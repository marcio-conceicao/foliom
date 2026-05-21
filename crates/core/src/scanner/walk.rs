//! Stub — implemented in Task 2.
#![allow(dead_code)]

use std::path::PathBuf;

use super::ignore::IgnoreSet;

#[derive(Debug, Clone)]
pub struct ScanEntry {
    pub path: PathBuf,
    pub mtime_ns: i64,
    pub size: u64,
}

pub fn walk<'a>(
    _root: &std::path::Path,
    _ignore: &'a IgnoreSet,
) -> impl Iterator<Item = ScanEntry> + 'a {
    std::iter::empty()
}
