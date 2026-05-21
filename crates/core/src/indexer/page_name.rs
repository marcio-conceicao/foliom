//! Filename → canonical page name derivation.
//!
//! Rules (Phase 1):
//!   1. Take the last `/`-separated component of the `RelativePath`.
//!   2. Strip a trailing `.md` extension if present.
//!   3. Decode `%2F` / `%2f` (literal substring replace) to `/` — LNK-02.
//!   4. NFC normalization is already guaranteed by `RelativePath`; no work.
//!   5. If the first component is `journals/`, validate the basename matches
//!      `YYYY_MM_DD` (10 chars, digits in positions 0..4, 5..7, 8..10 and
//!      underscores at positions 4 and 7). On match, `kind = Journal` and
//!      `journal_date = Some("YYYY-MM-DD")`. On mismatch the kind downgrades
//!      to `Page`.
//!
//! Phase 2 will turn the raw `YYYY_MM_DD` name into a display title like
//! "May 21st, 2026" (RF-55). Phase 1 keeps the name verbatim.

use crate::path::RelativePath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Page,
    Journal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    pub name: String,
    pub kind: PageKind,
    pub journal_date: Option<String>,
}

/// Derive a [`PageInfo`] from a [`RelativePath`].
pub fn derive_page_info(rel: &RelativePath) -> PageInfo {
    let s = rel.as_str();
    let mut parts = s.split('/');

    // Examine the first component — it flags journal kind iff `journals`.
    let first = parts.next().unwrap_or("");
    let is_journal_dir = first == "journals";

    // Take the last component (may equal `first` if there is only one part).
    let last = s.rsplit('/').next().unwrap_or(s);

    // Strip a trailing `.md` (case-sensitive — the scanner already filters to .md).
    let base = strip_md_suffix(last);

    // Decode %2F / %2f → '/'.
    let decoded = decode_percent_2f(base);

    // Journal date validation.
    if is_journal_dir {
        if let Some(iso) = validate_journal_date(&decoded) {
            return PageInfo {
                name: decoded,
                kind: PageKind::Journal,
                journal_date: Some(iso),
            };
        }
    }

    PageInfo {
        name: decoded,
        kind: PageKind::Page,
        journal_date: None,
    }
}

fn strip_md_suffix(s: &str) -> &str {
    // Strip literally `.md` — the scanner already filters by extension, so
    // anything else is "should not happen in practice" but we still don't crash.
    if let Some(stem) = s.strip_suffix(".md") {
        stem
    } else {
        s
    }
}

fn decode_percent_2f(s: &str) -> String {
    // Case-insensitive literal replace: %2F or %2f → '/'.
    // Single forward pass to avoid an extra allocation when no match.
    if !s.contains("%2") {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 2 < bytes.len()
            && bytes[i] == b'%'
            && bytes[i + 1] == b'2'
            && (bytes[i + 2] == b'F' || bytes[i + 2] == b'f')
        {
            out.push('/');
            i += 3;
        } else {
            // Safe: we're indexing on a byte-by-byte basis but only push
            // single-byte ASCII OR resync at UTF-8 boundaries. Since `%2`
            // detection uses ASCII, we can safely push the byte if it's
            // ASCII, otherwise reach into the str slice for the rest.
            //
            // Simpler approach: walk by char index.
            //
            // Re-do with chars iterator below; this branch only fires if
            // the byte isn't part of %2F so just emit it via str slicing.
            let ch_end = next_utf8_boundary(bytes, i);
            out.push_str(&s[i..ch_end]);
            i = ch_end;
        }
    }
    out
}

fn next_utf8_boundary(bytes: &[u8], i: usize) -> usize {
    // Returns the next index after the UTF-8 char starting at byte `i`.
    let b = bytes[i];
    let width = if b < 0x80 {
        1
    } else if b < 0xC0 {
        // Continuation byte — shouldn't be a starting position; recover by 1.
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    };
    (i + width).min(bytes.len())
}

/// Validate that `name` matches `YYYY_MM_DD` and convert to ISO `YYYY-MM-DD`.
fn validate_journal_date(name: &str) -> Option<String> {
    let b = name.as_bytes();
    if b.len() != 10 {
        return None;
    }
    if b[4] != b'_' || b[7] != b'_' {
        return None;
    }
    for &i in &[0, 1, 2, 3, 5, 6, 8, 9] {
        if !b[i].is_ascii_digit() {
            return None;
        }
    }
    // Build the ISO string by swapping underscores for hyphens.
    let mut iso = String::with_capacity(10);
    for (idx, ch) in name.chars().enumerate() {
        if idx == 4 || idx == 7 {
            iso.push('-');
        } else {
            iso.push(ch);
        }
    }
    Some(iso)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::RelativePath;

    fn rel(s: &str) -> RelativePath {
        RelativePath::from_storage_str(s)
    }

    #[test]
    fn journal_yyyy_mm_dd_parsed() {
        let info = derive_page_info(&rel("journals/2023_11_09.md"));
        assert_eq!(info.name, "2023_11_09");
        assert_eq!(info.kind, PageKind::Journal);
        assert_eq!(info.journal_date.as_deref(), Some("2023-11-09"));
    }

    #[test]
    fn simple_page_in_pages_dir() {
        let info = derive_page_info(&rel("pages/Sleep.md"));
        assert_eq!(info.name, "Sleep");
        assert_eq!(info.kind, PageKind::Page);
        assert_eq!(info.journal_date, None);
    }

    #[test]
    fn percent_2f_decoded_uppercase() {
        let info = derive_page_info(&rel("pages/Parent%2FChild.md"));
        assert_eq!(info.name, "Parent/Child");
        assert_eq!(info.kind, PageKind::Page);
    }

    #[test]
    fn percent_2f_decoded_lowercase() {
        let info = derive_page_info(&rel("pages/Foo%2fBar.md"));
        assert_eq!(info.name, "Foo/Bar");
    }

    #[test]
    fn nfc_passthrough_for_accents() {
        // RelativePath guarantees NFC. We just need to confirm we don't mangle.
        let info = derive_page_info(&rel("pages/Avaliação.md"));
        assert_eq!(info.name, "Avaliação");
        assert_eq!(info.kind, PageKind::Page);
    }

    #[test]
    fn untitled_no_parent_dir() {
        let info = derive_page_info(&rel("Untitled.md"));
        assert_eq!(info.name, "Untitled");
        assert_eq!(info.kind, PageKind::Page);
        assert_eq!(info.journal_date, None);
    }

    #[test]
    fn malformed_journal_date_downgrades_to_page() {
        // Wrong shape — should NOT parse as a journal.
        let info = derive_page_info(&rel("journals/foo_bar_baz.md"));
        assert_eq!(info.name, "foo_bar_baz");
        assert_eq!(info.kind, PageKind::Page);
        assert_eq!(info.journal_date, None);
    }

    #[test]
    fn journal_date_wrong_length_downgrades() {
        let info = derive_page_info(&rel("journals/2024_1_15.md"));
        assert_eq!(info.kind, PageKind::Page);
        assert_eq!(info.journal_date, None);
    }

    #[test]
    fn no_md_suffix_still_strips_nothing() {
        // Defensive: scanner only feeds us .md, but verify we don't crash
        // on a missing extension.
        let info = derive_page_info(&rel("pages/NoExt"));
        assert_eq!(info.name, "NoExt");
    }

    #[test]
    fn no_2f_no_alloc_path_correct() {
        let info = derive_page_info(&rel("pages/Plain.md"));
        assert_eq!(info.name, "Plain");
    }
}
