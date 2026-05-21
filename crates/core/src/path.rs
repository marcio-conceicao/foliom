//! `RelativePath` — newtype owning the NFC + forward-slash storage
//! invariant (IDX-07, D-15).
//!
//! Every path that crosses the IO boundary into SQLite goes through
//! `from_filesystem`, which:
//!
//!   1. Strips the notes-folder root prefix.
//!   2. Walks the remaining components, accepting **only** `Component::Normal`
//!      — `..`, `.`, prefix, and root components are rejected with
//!      `UnexpectedPathComponent` (T-03-02 path-traversal mitigation).
//!   3. NFC-normalizes each component string via `unicode-normalization`, so
//!      a macOS-Finder NFD filename and a Linux NFC filename produce
//!      byte-identical storage keys.
//!   4. Joins components with `/` — never the platform separator.
//!
//! `from_storage_str` is the trusting constructor used when reading paths
//! back out of SQLite; it does no normalization.
//!
//! `to_filesystem` rebuilds a platform-native `PathBuf` by joining each
//! `/`-separated component onto the supplied root.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelativePath(String);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path is not under root")]
    PathOutsideRoot,
    #[error("path component is not valid UTF-8")]
    NonUtf8Path,
    #[error("unexpected path component (only Normal components allowed)")]
    UnexpectedPathComponent,
}

impl RelativePath {
    /// Build a `RelativePath` from an absolute path under `root`. Rejects
    /// non-Normal components (T-03-02 traversal mitigation), non-UTF-8
    /// components, and paths outside `root`. NFC-normalizes each component.
    pub fn from_filesystem(abs: &Path, root: &Path) -> Result<Self, PathError> {
        let rel = abs.strip_prefix(root).map_err(|_| PathError::PathOutsideRoot)?;

        let mut parts: Vec<String> = Vec::new();
        for c in rel.components() {
            match c {
                Component::Normal(os) => {
                    let s = os.to_str().ok_or(PathError::NonUtf8Path)?;
                    let nfc: String = s.nfc().collect();
                    parts.push(nfc);
                }
                _ => return Err(PathError::UnexpectedPathComponent),
            }
        }

        Ok(RelativePath(parts.join("/")))
    }

    /// Trusting constructor for values read back out of SQLite. No
    /// normalization — the value was already canonical when stored.
    pub fn from_storage_str(s: &str) -> Self {
        RelativePath(s.to_string())
    }

    /// Borrow the canonical `/`-separated string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Rebuild a platform-native absolute path by joining each
    /// `/`-separated component onto `root`.
    pub fn to_filesystem(&self, root: &Path) -> PathBuf {
        let mut out = root.to_path_buf();
        if !self.0.is_empty() {
            for part in self.0.split('/') {
                out.push(part);
            }
        }
        out
    }
}
