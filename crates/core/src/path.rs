//! `RelativePath` newtype — stub. Real impl in Plan 01-03 Task 2 GREEN.

use std::path::{Path, PathBuf};

use thiserror::Error;

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
    pub fn from_filesystem(_abs: &Path, _root: &Path) -> Result<Self, PathError> {
        todo!("plan 01-03 task 2 GREEN")
    }

    pub fn from_storage_str(_s: &str) -> Self {
        todo!("plan 01-03 task 2 GREEN")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_filesystem(&self, _root: &Path) -> PathBuf {
        todo!("plan 01-03 task 2 GREEN")
    }
}
