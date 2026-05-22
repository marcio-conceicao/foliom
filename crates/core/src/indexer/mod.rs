//! Indexer — orchestrates scanner + parser + storage into a coherent
//! "scan disk → diff against `files` → reparse dirty → write" pipeline.
//!
//! Phase 1 milestone: provides the [`reindex`] entry point consumed by
//! Plan 01-07's CLI. Per AP-5 every file is its own SQLite transaction;
//! per AP-7 incremental mode trusts `(mtime_ns, size)` first and only
//! reads + hashes on mismatch.

pub mod page_name;
pub mod write;

use std::path::Path;

use crate::path::PathError;
use crate::storage::{Db, StorageError};

/// Reindex mode — `Incremental` trusts the cached `(mtime_ns, size)` to
/// avoid disk reads; `Full` re-reads every file regardless of cache state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReindexMode {
    Incremental,
    Full,
}

/// Statistics returned by [`reindex`].
///
/// Plan 01-07 added the `Serialize` derive (`camelCase`) so the CLI can
/// emit it as the JSON contract for `foliom index --json` / `reindex --json`.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexStats {
    /// Total entries visited by the scanner.
    pub scanned: usize,
    /// New files inserted into the index.
    pub added: usize,
    /// Files whose hash differed from the cached one and were reparsed.
    pub modified: usize,
    /// Files whose `(mtime_ns, size)` matched the cache — no IO performed.
    pub unchanged: usize,
    /// Files whose mtime changed but content hash matched — mtime touched only.
    pub mtime_touched: usize,
    /// Files that were in the DB but no longer on disk — cascade-deleted.
    pub deleted: usize,
}

/// Errors surfaced by the indexer.
#[derive(thiserror::Error, Debug)]
pub enum IndexerError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("path error: {0}")]
    Path(#[from] PathError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("non-UTF-8 block content in file {0}")]
    NonUtf8Block(String),
}

/// Orchestrator entry point — implementation lives in this module.
///
/// Wired in Task 3 of plan 01-06.
pub fn reindex(
    db: &mut Db,
    root: &Path,
    mode: ReindexMode,
) -> Result<ReindexStats, IndexerError> {
    crate::indexer::write::reindex_impl(db, root, mode)
}
