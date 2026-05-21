//! `IgnoreSet` and the hard-coded Logseq ignore list (IDX-01).
//!
//! The ignore list is exact-match, case-sensitive on the directory name.
//! [`super::walk::walk`] consults [`IgnoreSet::is_ignored`] at every
//! directory entry via `walkdir::filter_entry`, so an ignored directory is
//! never even descended into — its children are not enumerated at all.
//!
//! [`IgnoreSet::extend_from_config_edn`] folds the `:hidden [...]` strings
//! from the user's `logseq/config.edn` into the set. The Logseq syntax
//! allows the entries to be path-like (e.g. `"/archived"`,
//! `"../assets/archived"`), but Phase 1's matcher is a single segment
//! name — anything with a path separator simply will not match and is
//! stored verbatim. Phase 2 may revisit if real-world configs demand
//! path-based suppression.

use std::collections::HashSet;

/// Hard-coded ignore list for the Logseq folder layout (IDX-01). Reproduced
/// verbatim from RESEARCH §Ignore List.
pub const DEFAULT_LOGSEQ_IGNORES: &[&str] = &[
    "logseq",        // Logseq metadata folder
    "assets",        // images / attachments
    "draws",         // Excalidraw drawings (.excalidraw)
    "whiteboards",   // Logseq whiteboards
    "bak",           // Logseq edit-history backups
    ".recycle",      // Logseq trash
    "version-files", // Logseq versioning
    ".git",          // version control
    ".obsidian",     // if user also opened the folder in Obsidian
    ".trash",        //
    "node_modules",  // safety net
];

/// A case-sensitive set of directory names the scanner must not descend
/// into.
#[derive(Debug, Clone, Default)]
pub struct IgnoreSet {
    names: HashSet<String>,
}

impl IgnoreSet {
    /// Build an `IgnoreSet` pre-populated with [`DEFAULT_LOGSEQ_IGNORES`].
    pub fn default_logseq() -> Self {
        Self {
            names: DEFAULT_LOGSEQ_IGNORES.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// Fold `:hidden` entries from `config.edn` into the set. Duplicates
    /// are no-ops.
    pub fn extend_from_config_edn(&mut self, hidden: Vec<String>) {
        self.names.extend(hidden);
    }

    /// Return `true` if `name` is in the ignore set (exact match,
    /// case-sensitive).
    pub fn is_ignored(&self, name: &str) -> bool {
        self.names.contains(name)
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
