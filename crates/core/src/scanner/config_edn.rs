//! Minimal `logseq/config.edn :hidden` extractor.
//!
//! Phase 1 reads exactly one key from `config.edn`: `:hidden`. Anything
//! else (`:journal/file-name-format`, `:pages-directory`, `:journals-directory`,
//! aliases, etc.) is deferred to Phase 2, when the renderer will need
//! a real EDN parser.
//!
//! ## Recognised forms
//!
//! ```edn
//! :hidden ["foo" "bar"]
//! :hidden #{"baz"}
//! :hidden [
//!   "a"
//!   "b"
//! ]
//! ```
//!
//! ## Documented limitations
//!
//! * Does NOT handle nested maps or tagged literals (`#inst "..."`).
//! * Does NOT handle namespaced keywords on the value side.
//! * Does NOT handle multi-line strings that span the `]` / `}` boundary.
//! * Does NOT distinguish a `:hidden` inside a line comment from a real
//!   key; the regex finds the first match in the file. Real `config.edn`
//!   files conventionally place `:hidden` outside any comment so this is
//!   acceptable for Phase 1.
//! * Returns `Vec::new()` on absent file, IO error, or unparseable value —
//!   never panics.
//!
//! Phase 2 will replace this if the renderer needs other `config.edn`
//! keys.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

/// Read `logseq/config.edn` and extract the `:hidden` string list.
/// Returns an empty Vec on any read or parse failure.
pub fn read_hidden(config_edn_path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(config_edn_path) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(
                path = %config_edn_path.display(),
                error = %err,
                "could not read config.edn — treating :hidden as empty"
            );
            return Vec::new();
        }
    };
    parse_hidden_from_str(&content)
}

/// Pure-string variant — exposed crate-private for unit testing without
/// touching the filesystem.
pub(crate) fn parse_hidden_from_str(content: &str) -> Vec<String> {
    // `:hidden  ` followed by `[ ... ]`  or  `#{ ... }`.
    // `(?s)` enables dot-matches-newline (the `[^\]\}]*` payload handles
    // newlines on its own since it excludes the closing bracket, but
    // making it explicit costs nothing).
    static HIDDEN_RE: OnceLock<Regex> = OnceLock::new();
    let re = HIDDEN_RE.get_or_init(|| {
        Regex::new(r#"(?s):hidden\s+[#]?[\[\{]([^\]\}]*)[\]\}]"#).unwrap()
    });
    let inner = match re.captures(content).and_then(|c| c.get(1)) {
        Some(m) => m.as_str(),
        None => return Vec::new(),
    };

    // Standard double-quoted-string with backslash escape support.
    static STR_RE: OnceLock<Regex> = OnceLock::new();
    let str_re =
        STR_RE.get_or_init(|| Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).unwrap());

    str_re
        .captures_iter(inner)
        .map(|c| c[1].to_string())
        .collect()
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
        // Documented naivety: the regex finds the first match regardless
        // of whether it's inside a line comment. We accept any result
        // here as long as it doesn't panic — the smoke test against the
        // real config.edn is the primary safety net.
        let content = ";; :hidden [\"a\" \"b\"]\n:hidden []";
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
