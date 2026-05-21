//! `IgnoreSet` and the hard-coded Logseq ignore list (IDX-01).
//!
//! Implementation stub — see Task 1 GREEN.

use std::collections::HashSet;

pub const DEFAULT_LOGSEQ_IGNORES: &[&str] = &[];

#[derive(Debug, Clone, Default)]
pub struct IgnoreSet {
    #[allow(dead_code)]
    names: HashSet<String>,
}

impl IgnoreSet {
    pub fn default_logseq() -> Self {
        todo!("Task 1 GREEN")
    }

    pub fn extend_from_config_edn(&mut self, _hidden: Vec<String>) {
        todo!("Task 1 GREEN")
    }

    pub fn is_ignored(&self, _name: &str) -> bool {
        todo!("Task 1 GREEN")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_logseq_ignores_logseq_dir() {
        let set = IgnoreSet::default_logseq();
        assert!(set.is_ignored("logseq"));
    }

    #[test]
    fn default_logseq_ignores_every_documented_name() {
        let set = IgnoreSet::default_logseq();
        for name in [
            "logseq",
            "assets",
            "draws",
            "whiteboards",
            "bak",
            ".recycle",
            "version-files",
            ".git",
            ".obsidian",
            ".trash",
            "node_modules",
        ] {
            assert!(set.is_ignored(name), "expected {name:?} to be ignored");
        }
    }

    #[test]
    fn default_logseq_does_not_ignore_user_content_dirs() {
        let set = IgnoreSet::default_logseq();
        assert!(!set.is_ignored("journals"));
        assert!(!set.is_ignored("pages"));
        assert!(!set.is_ignored("Untitled.md"));
    }

    #[test]
    fn extend_from_config_edn_adds_entries() {
        let mut set = IgnoreSet::default_logseq();
        set.extend_from_config_edn(vec!["foo".to_string(), "bar".to_string()]);
        assert!(set.is_ignored("foo"));
        assert!(set.is_ignored("bar"));
        assert!(set.is_ignored("logseq"));
    }

    #[test]
    fn ignore_matching_is_case_sensitive() {
        let set = IgnoreSet::default_logseq();
        assert!(!set.is_ignored("LOGSEQ"));
        assert!(!set.is_ignored("Assets"));
    }

    #[test]
    fn default_set_has_expected_size() {
        let _ = IgnoreSet::default_logseq();
        assert_eq!(DEFAULT_LOGSEQ_IGNORES.len(), 11);
    }
}
