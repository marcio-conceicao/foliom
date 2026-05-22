//! Shared application state injected into every axum handler via
//! `axum::extract::State`. Per D-38, `Db` is wrapped in `Arc<Mutex<...>>`
//! for the Phase 2 single-writer + few-readers profile. A connection pool
//! would only be justified once contention shows up in benchmarks
//! (deferred to Phase 3 if needed).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foliom_core::rename::Journal;
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;

use std::path::Path;
use foliom_core::rename::RenameState;

/// Cloneable handle to the shared backend state.
///
/// `Clone` is cheap (just bumps the `Arc` refcount) — required by
/// `axum::Router::with_state`, which clones the state per request.
#[derive(Clone)]
pub struct AppState {
    /// Shared SQLite handle. Lock on read; lock-and-mutate on write.
    /// Read by every plan-02-02 handler (pages, journals, search, titles).
    pub db: Arc<Mutex<Db>>,
    /// Notes root the server was launched against. Used by mutation handlers
    /// to resolve absolute file paths from `files.path` (relative to root).
    pub root: PathBuf,
    /// Lock-free registry of BLAKE3 hashes Foliom has written so the Phase 4
    /// watcher can suppress its own write echoes (`take_if_present`). Cloning
    /// is cheap — the inner `Arc<DashMap>` is shared across all clones.
    ///
    /// Plan 03-03 mutation handlers call `self_writes.register` (via
    /// `atomic_write_md`) BEFORE the rename so the watcher cannot race ahead.
    pub self_writes: Arc<SelfWriteSet>,
    /// Write-ahead journal for rename operations. Shared across handlers.
    /// Plan 03-06 crash recovery: appended before SQL commit, replayed on boot.
    pub journal: Arc<Journal>,
}

impl RenameState for AppState {
    fn db(&self) -> &Arc<Mutex<Db>> {
        &self.db
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn journal(&self) -> &Arc<Journal> {
        &self.journal
    }
    fn self_writes(&self) -> &Arc<SelfWriteSet> {
        &self.self_writes
    }
}
