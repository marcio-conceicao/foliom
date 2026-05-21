//! Stage 2 ref extractor — unit tests.
//!
//! Covers PRS-04: extract `[[link]]`, `#tag`, `#[[multi word tag]]` only from
//! CommonMark text nodes. Headings, code blocks, inline code, link targets,
//! hex colors, and URL fragments must NOT produce refs.

use foliom_core::parser::ast::{extract_refs, ExtractedRef, RefKind};

fn page(target: &str) -> ExtractedRef {
    ExtractedRef {
        kind: RefKind::PageLink,
        target: target.to_string(),
    }
}

fn tag(target: &str) -> ExtractedRef {
    ExtractedRef {
        kind: RefKind::Tag,
        target: target.to_string(),
    }
}

mod page_links {
    use super::*;

    #[test]
    fn single_page_link() {
        let refs = extract_refs("[[Foo]]");
        assert_eq!(refs, vec![page("Foo")]);
    }

    #[test]
    fn two_page_links_in_sentence() {
        let refs = extract_refs("see [[Foo Bar]] and [[Baz]]");
        assert_eq!(refs, vec![page("Foo Bar"), page("Baz")]);
    }

    #[test]
    fn namespace_path_slash() {
        let refs = extract_refs("[[Parent/Child]]");
        assert_eq!(refs, vec![page("Parent/Child")]);
    }
}

mod tags {
    use super::*;

    #[test]
    fn single_tag() {
        let refs = extract_refs("#Crypto");
        assert_eq!(refs, vec![tag("Crypto")]);
    }

    #[test]
    fn two_tags_with_period_terminator() {
        let refs = extract_refs("#Tag1 and #Tag2.");
        assert_eq!(refs, vec![tag("Tag1"), tag("Tag2")]);
    }

    #[test]
    fn tag_glued_to_preceding_alphanumeric_rejected() {
        // URL-fragment-like: `ends with#NoSpace` — # is preceded by alphanumeric.
        let refs = extract_refs("ends with#NoSpace");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn tag_with_dash_and_slash() {
        let refs = extract_refs("see #conector-com-parceiro at end");
        assert_eq!(refs, vec![tag("conector-com-parceiro")]);
    }
}

mod multiword_tags {
    use super::*;

    #[test]
    fn composite_tag_extracted_as_tag() {
        let refs = extract_refs("#[[multi word tag]]");
        assert_eq!(refs, vec![tag("multi word tag")]);
    }

    #[test]
    fn composite_tag_inside_sentence() {
        let refs = extract_refs("mentioned #[[Monitoria de Qualidade]] today");
        assert_eq!(refs, vec![tag("Monitoria de Qualidade")]);
    }

    #[test]
    fn composite_tag_takes_precedence_over_bare_hash() {
        // `#[[X]]` must not be parsed as bare `#` followed by `[[X]]`.
        let refs = extract_refs("#[[Foo]] and [[Bar]]");
        assert_eq!(refs, vec![tag("Foo"), page("Bar")]);
    }
}

mod suppression {
    use super::*;

    #[test]
    fn atx_heading_suppressed() {
        let refs = extract_refs("# Heading with [[Link]] and #tag");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn fenced_code_block_suppressed() {
        let refs = extract_refs("```\n[[InCode]] and #InTag\n```");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn inline_code_suppressed() {
        let refs = extract_refs("see `[[InCode]] and #InTag`");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn link_text_and_target_suppressed() {
        // v1 conservative: anything inside a link Tag::Link is suppressed.
        let refs = extract_refs("see [link text with #tag](https://x.com)");
        assert_eq!(refs, vec![]);
    }
}

mod hex_and_urls {
    use super::*;

    #[test]
    fn three_digit_hex_color_rejected() {
        let refs = extract_refs("color is #fff");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn six_digit_hex_color_rejected() {
        let refs = extract_refs("accent #1a2b3c here");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn url_autolink_fragment_suppressed() {
        // pulldown-cmark recognizes plain URLs in autolink form? Not always —
        // but `[txt](url)` definitely suppresses. Use explicit md link form.
        let refs = extract_refs("see [docs](https://example.com/page#section-anchor)");
        assert_eq!(refs, vec![]);
    }

    #[test]
    fn real_legit_tag_after_text() {
        let refs = extract_refs("Real tag at the end works: #legit");
        assert_eq!(refs, vec![tag("legit")]);
    }
}

mod percent_decoding {
    use super::*;

    #[test]
    fn percent_2f_decoded_in_link() {
        let refs = extract_refs("[[Parent%2FChild]]");
        assert_eq!(refs, vec![page("Parent/Child")]);
    }

    #[test]
    fn percent_2f_decoded_lowercase() {
        let refs = extract_refs("[[Foo%2fBar]]");
        assert_eq!(refs, vec![page("Foo/Bar")]);
    }

    #[test]
    fn percent_2f_decoded_in_composite_tag() {
        let refs = extract_refs("#[[Foo%2FBar]]");
        assert_eq!(refs, vec![tag("Foo/Bar")]);
    }
}

mod nfc {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    #[test]
    fn nfc_and_nfd_link_targets_match() {
        let nfc_form: String = "Avaliação".nfc().collect();
        let nfd_form: String = "Avaliação".nfd().collect();
        assert_ne!(nfc_form, nfd_form, "sanity: NFC and NFD differ byte-wise");

        let refs_nfc = extract_refs(&format!("[[{}]]", nfc_form));
        let refs_nfd = extract_refs(&format!("[[{}]]", nfd_form));

        assert_eq!(refs_nfc.len(), 1);
        assert_eq!(refs_nfd.len(), 1);
        assert_eq!(refs_nfc[0].target, refs_nfd[0].target);
        assert_eq!(refs_nfc[0].target, nfc_form);
    }
}

mod logseq_fixture_positive {
    use super::*;

    #[test]
    fn page_05_first_bullet_extracts_two_page_links() {
        let refs = extract_refs("- Reunião com [[Glauber]] sobre [[Speech Analytics]]\n");
        assert_eq!(refs, vec![page("Glauber"), page("Speech Analytics")]);
    }

    #[test]
    fn page_05_continuation_extracts_tag_and_composite() {
        let refs = extract_refs("\t- Mencionou #urgente e #[[Monitoria de Qualidade]] no meio\n");
        assert_eq!(
            refs,
            vec![tag("urgente"), tag("Monitoria de Qualidade")]
        );
    }
}
