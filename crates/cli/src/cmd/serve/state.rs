//! Shared application state injected into every axum handler via
//! `axum::extract::State`. Per D-38, `Db` is wrapped in `Arc<Mutex<...>>`
//! for the Phase 2 single-writer + few-readers profile. A connection pool
//! would only be justified once contention shows up in benchmarks
//! (deferred to Phase 3 if needed).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foliom_core::storage::Db;

/// Cloneable handle to the shared backend state.
///
/// `Clone` is cheap (just bumps the `Arc` refcount) — required by
/// `axum::Router::with_state`, which clones the state per request.
#[derive(Clone)]
pub struct AppState {
    /// Shared SQLite handle. Lock on read; lock-and-mutate on write.
    pub db: Arc<Mutex<Db>>,
    /// Notes root the server was launched against. Useful for
    /// future `/api/inventory` and reindex-on-demand endpoints.
    pub root: PathBuf,
}
