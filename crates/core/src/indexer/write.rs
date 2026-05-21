//! Per-file transactional writes and the reindex orchestrator body.
//!
//! Wired in Task 2 (write helpers) and Task 3 (`reindex_impl`).

use std::path::Path;

use crate::storage::Db;

use super::{IndexerError, ReindexMode, ReindexStats};

/// Stub orchestrator body — populated in Task 3.
pub(crate) fn reindex_impl(
    _db: &mut Db,
    _root: &Path,
    _mode: ReindexMode,
) -> Result<ReindexStats, IndexerError> {
    unimplemented!("reindex_impl lands in Task 3 of plan 01-06")
}
