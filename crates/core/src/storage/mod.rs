//! Storage layer — SQLite + migrations + DB-location resolution.
//!
//! Public surface for Phase 1:
//!   - [`Db`]              — owned handle over a single SQLite connection.
//!   - [`Db::open`]        — resolve DB path from notes-root + open + migrate.
//!   - [`Db::open_at`]     — explicit DB path (tests, CLI override).
//!   - [`StorageError`]    — unified error surface for the storage module.
//!   - [`resolve_db_path`] — re-export of the location resolver.
//!
//! Phase 1 owns the *schema* only — concrete inserts (files/pages/blocks/etc.)
//! live in Plan 01-06 (indexer). This module deliberately exposes the raw
//! [`rusqlite::Connection`] so the indexer can build its own statements.

pub mod location;

pub use location::resolve_db_path;

use std::path::Path;
use std::sync::OnceLock;

use rusqlite::{Connection, Transaction};
use rusqlite_migration::{M, Migrations};

/// Errors surfaced by the storage layer.
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("$HOME is not set — cannot resolve user data directory")]
    NoHomeDir,

    #[error("%LOCALAPPDATA% is not set — cannot resolve user data directory")]
    NoAppData,

    #[error("path resolution failed: {0}")]
    PathResolution(String),
}

impl From<rusqlite_migration::Error> for StorageError {
    fn from(e: rusqlite_migration::Error) -> Self {
        StorageError::Migration(e.to_string())
    }
}

/// The Phase 1 migration set: one migration, full schema.
///
/// `Migrations::new` is `const`, but the inner `Vec<M>` borrows `'static str`s from
/// `include_str!`. We materialize lazily via `OnceLock` so the static lives forever
/// and `to_latest` can mutate-borrow the `&Connection` it needs.
fn migrations() -> &'static Migrations<'static> {
    static MIGRATIONS: OnceLock<Migrations<'static>> = OnceLock::new();
    MIGRATIONS.get_or_init(|| {
        Migrations::new(vec![M::up(include_str!("migrations/001_init.sql"))])
    })
}

/// Apply the PRAGMA batch every freshly-opened connection needs.
///
/// Sourced from RESEARCH §PRAGMA setup on connection open. Notable choices:
///   - `journal_mode = WAL` — readers don't block the writer.
///   - `synchronous = NORMAL` — durable across process crashes (full only matters
///      across power-loss; not worth the cost for a single-user notes app).
///   - `foreign_keys = ON` — without this every FK in `001_init.sql` is decorative.
///   - `journal_size_limit = 64 MB` — caps -wal growth (T-04-05 mitigation).
pub fn configure_connection(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 268435456;
        PRAGMA wal_autocheckpoint = 1000;
        PRAGMA journal_size_limit = 67108864;
        ",
    )?;
    Ok(())
}

/// Open a connection at `db_path`, configure PRAGMAs, and run migrations to latest.
///
/// Public free function for callers that want a bare `Connection` rather than a `Db`.
pub fn open_db(db_path: &Path) -> Result<Connection, StorageError> {
    let mut conn = Connection::open(db_path)?;
    configure_connection(&conn)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

/// Owned handle over a SQLite connection backed by the migrated Phase 1 schema.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Resolve the DB path for `notes_root` (D-13), open, configure, migrate.
    pub fn open(notes_root: &Path) -> Result<Self, StorageError> {
        let db_path = resolve_db_path(notes_root)?;
        Self::open_at(&db_path)
    }

    /// Open at an explicit path. Used by integration tests and any future CLI
    /// override that bypasses the platform resolver.
    pub fn open_at(db_path: &Path) -> Result<Self, StorageError> {
        let conn = open_db(db_path)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.conn.transaction()
    }
}
