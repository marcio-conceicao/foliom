//! Storage layer — SQLite + migrations + DB-location resolution.
//!
//! Stub for Plan 01-04 Task 2: only `StorageError` is wired so `location.rs`
//! compiles standalone. Task 3 adds the `Db` wrapper, PRAGMA setup, and
//! migration application.

pub mod location;

pub use location::resolve_db_path;

/// Errors surfaced by the storage layer.
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("$HOME is not set — cannot resolve user data directory")]
    NoHomeDir,

    #[error("%LOCALAPPDATA% is not set — cannot resolve user data directory")]
    NoAppData,
}
