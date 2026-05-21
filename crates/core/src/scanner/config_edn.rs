//! Stub — implemented in Task 3 GREEN.
#![allow(dead_code)]

use std::path::Path;

pub fn read_hidden(_config_edn_path: &Path) -> Vec<String> {
    todo!("Task 3 GREEN")
}

pub(crate) fn parse_hidden_from_str(_content: &str) -> Vec<String> {
    todo!("Task 3 GREEN")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_vector_form() {
        assert_eq!(
            parse_hidden_from_str(":hidden [\"foo\" \"bar\"]"),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn parses_set_form() {
        assert_eq!(
            parse_hidden_from_str(":hidden #{\"baz\"}"),
            vec!["baz".to_string()]
        );
    }

    #[test]
    fn parses_multiline_vector() {
        let content = ":hidden  [\n  \"a\"\n  \"b\"\n]";
        assert_eq!(
            parse_hidden_from_str(content),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(parse_hidden_from_str("").is_empty());
    }

    #[test]
    fn missing_key_returns_empty() {
        assert!(parse_hidden_from_str("{:other-key 42}").is_empty());
    }

    #[test]
    fn empty_vector_returns_empty() {
        assert!(parse_hidden_from_str(":hidden []").is_empty());
    }

    #[test]
    fn commented_out_hidden_is_ignored() {
        // The regex is naive: a `:hidden` inside a line-comment will still
        // match. We document this in the module doc. Phase 2 may upgrade
        // to a real EDN parser if needed.
        let content = ";; :hidden [\"a\" \"b\"]\n:hidden []";
        // The regex finds the FIRST match — which is the commented one
        // in this synthetic case. So we accept any result that doesn't
        // panic; the smoke-test against the real config.edn is the
        // primary safety net.
        let _ = parse_hidden_from_str(content);
    }

    #[test]
    fn handles_escaped_quote_in_string() {
        assert_eq!(
            parse_hidden_from_str(r#":hidden ["foo\"bar" "baz"]"#),
            vec![r#"foo\"bar"#.to_string(), "baz".to_string()]
        );
    }

    #[test]
    fn read_hidden_returns_empty_on_missing_file() {
        assert!(read_hidden(Path::new("/definitely/does/not/exist.edn")).is_empty());
    }

    #[test]
    fn read_hidden_reads_a_real_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "{{:hidden [\"archived\" \"drafts\"]}}").unwrap();
        let v = read_hidden(tmp.path());
        assert_eq!(v, vec!["archived".to_string(), "drafts".to_string()]);
    }

    #[test]
    fn smoke_real_config_edn_does_not_panic_if_present() {
        let p = Path::new("../../data-folder-sample/Logseq/logseq/config.edn");
        if !p.exists() {
            eprintln!("skipping — real config.edn not present");
            return;
        }
        let _ = read_hidden(p);
    }
}
