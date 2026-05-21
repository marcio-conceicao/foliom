//! Integration tests for the Phase 1 storage layer (Plan 01-04 Task 3).
//!
//! These tests exercise the full open → configure → migrate → operate cycle:
//!   - PRAGMAs are applied and the values we asserted in RESEARCH stick.
//!   - All Phase 1 tables + the FTS5 virtual table exist after migration.
//!   - INSERT into every table works.
//!   - FTS5 triggers fire on INSERT / UPDATE / DELETE of `blocks`.
//!   - Foreign-key CASCADE and SET NULL behaviors match the schema.
//!   - Re-opening the DB is idempotent.
//!   - `Db::open(notes_root)` puts the DB OUTSIDE the notes folder (IDX-06).

use std::path::Path;

use foliom_core::storage::{Db, resolve_db_path};
use rusqlite::params;
use tempfile::TempDir;

/// Snapshot/restore an env var across a closure (used to point `XDG_DATA_HOME`
/// at a tempdir so `Db::open` doesn't pollute the developer's real ~/.local/share).
#[cfg(target_os = "linux")]
fn with_env<F: FnOnce()>(key: &str, value: &Path, f: F) {
    let prev = std::env::var(key).ok();
    // SAFETY: env mutation is ok here because each integration test is a separate
    // binary; cargo runs them serially per binary. Within this binary, the test
    // helpers that call `with_env` are not run concurrently.
    unsafe {
        std::env::set_var(key, value);
    }
    f();
    unsafe {
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

fn pragma_string(db: &Db, pragma: &str) -> String {
    db.conn()
        .query_row(&format!("PRAGMA {}", pragma), [], |r| r.get::<_, String>(0))
        .unwrap()
}

fn pragma_int(db: &Db, pragma: &str) -> i64 {
    db.conn()
        .query_row(&format!("PRAGMA {}", pragma), [], |r| r.get::<_, i64>(0))
        .unwrap()
}

#[test]
fn open_applies_pragmas_and_creates_schema() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("foliom.db");
    let db = Db::open_at(&db_path).unwrap();

    // PRAGMA assertions per RESEARCH §PRAGMA setup.
    assert_eq!(pragma_string(&db, "journal_mode").to_lowercase(), "wal");
    assert_eq!(pragma_int(&db, "foreign_keys"), 1);
    assert_eq!(pragma_int(&db, "synchronous"), 1); // NORMAL

    // Schema assertions: all 7 Phase 1 tables + the FTS5 virtual table.
    let tables: Vec<String> = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    for expected in [
        "files",
        "pages",
        "blocks",
        "block_props",
        "block_drawers",
        "tags",
        "refs",
        "blocks_fts",
    ] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {:?}; got {:?}",
            expected,
            tables
        );
    }
}

#[test]
fn full_row_lifecycle_in_every_table() {
    let tmp = TempDir::new().unwrap();
    let mut db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();

    // Single transaction exercising every table's INSERT path.
    {
        let tx = db.transaction().unwrap();

        // files
        tx.execute(
            "INSERT INTO files (id, path, mtime_ns, size, hash) VALUES (?, ?, ?, ?, ?)",
            params![1, "pages/Foo.md", 1_700_000_000_000_000_000_i64, 42, b"hash-bytes-32!!".as_slice()],
        )
        .unwrap();

        // pages
        tx.execute(
            "INSERT INTO pages (id, file_id, name, kind, journal_date) VALUES (?, ?, ?, ?, ?)",
            params![1, 1, "Foo", "page", Option::<String>::None],
        )
        .unwrap();

        // blocks (depth=-1 prelude + depth=0 bullet)
        tx.execute(
            "INSERT INTO blocks (id, page_id, parent_id, ord, depth, raw, byte_offset, byte_length, hash) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![1, 1, Option::<i64>::None, 0, -1, "", 0_i64, 0_i64, b"prelude-hash".as_slice()],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO blocks (id, page_id, parent_id, ord, depth, raw, byte_offset, byte_length, hash) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                2, 1, Option::<i64>::None, 1, 0, "- hello world\n",
                0_i64, 14_i64, b"hello-hash".as_slice()
            ],
        )
        .unwrap();

        // block_props
        tx.execute(
            "INSERT INTO block_props (block_id, key, value) VALUES (?, ?, ?)",
            params![2, "alias", "Greetings"],
        )
        .unwrap();

        // block_drawers
        tx.execute(
            "INSERT INTO block_drawers (block_id, name, byte_offset, byte_length) VALUES (?, ?, ?, ?)",
            params![2, "LOGBOOK", 14_i64, 30_i64],
        )
        .unwrap();

        // tags + refs
        tx.execute(
            "INSERT INTO tags (id, name) VALUES (?, ?)",
            params![1, "Crypto"],
        )
        .unwrap();
        // refs: the source block links at the page itself (D-03 — tag and link resolve to pages).
        tx.execute(
            "INSERT INTO refs (source_block, type, target_page) VALUES (?, ?, ?)",
            params![2, "page-link", 1],
        )
        .unwrap();

        tx.commit().unwrap();
    }

    // Read-back assertions.
    let files_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(files_count, 1);

    let blocks_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM blocks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(blocks_count, 2);

    // FTS5 trigger fired? blocks_fts should hold both rows now.
    let fts_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM blocks_fts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fts_count, 2, "INSERT triggers should populate blocks_fts");

    // FTS5 MATCH returns the inserted row.
    let fts_hit: i64 = db
        .conn()
        .query_row(
            "SELECT rowid FROM blocks_fts WHERE blocks_fts MATCH ?",
            params!["hello"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fts_hit, 2);
}

#[test]
fn fts5_update_trigger_swaps_indexed_text() {
    let tmp = TempDir::new().unwrap();
    let db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();

    db.conn()
        .execute(
            "INSERT INTO files (id, path, mtime_ns, size, hash) VALUES (1, 'p.md', 0, 0, X'00')",
            [],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO pages (id, file_id, name, kind) VALUES (1, 1, 'p', 'page')",
            [],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO blocks (id, page_id, ord, depth, raw, byte_offset, byte_length, hash) \
             VALUES (1, 1, 0, 0, 'original text', 0, 13, X'00')",
            [],
        )
        .unwrap();

    let before: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM blocks_fts WHERE blocks_fts MATCH 'original'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, 1);

    db.conn()
        .execute("UPDATE blocks SET raw = 'updated phrase' WHERE id = 1", [])
        .unwrap();

    let original_after: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM blocks_fts WHERE blocks_fts MATCH 'original'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let updated_after: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM blocks_fts WHERE blocks_fts MATCH 'updated'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(original_after, 0, "UPDATE trigger must invalidate the old text in FTS");
    assert_eq!(updated_after, 1, "UPDATE trigger must reindex the new text");
}

#[test]
fn fts5_delete_trigger_clears_indexed_row() {
    let tmp = TempDir::new().unwrap();
    let db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();

    db.conn().execute_batch(
        "INSERT INTO files (id, path, mtime_ns, size, hash) VALUES (1, 'p.md', 0, 0, X'00');
         INSERT INTO pages (id, file_id, name, kind) VALUES (1, 1, 'p', 'page');
         INSERT INTO blocks (id, page_id, ord, depth, raw, byte_offset, byte_length, hash)
            VALUES (1, 1, 0, 0, 'sayonara', 0, 8, X'00');",
    ).unwrap();
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM blocks_fts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    db.conn().execute("DELETE FROM blocks WHERE id = 1", []).unwrap();
    assert_eq!(
        db.conn()
            .query_row("SELECT COUNT(*) FROM blocks_fts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0,
        "DELETE trigger must remove the FTS row"
    );
}

#[test]
fn cascade_and_set_null_behavior() {
    let tmp = TempDir::new().unwrap();
    let db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();

    db.conn().execute_batch(
        "INSERT INTO files (id, path, mtime_ns, size, hash) VALUES (1, 'p.md', 0, 0, X'00');
         INSERT INTO pages (id, file_id, name, kind) VALUES (1, 1, 'P', 'page');
         INSERT INTO blocks (id, page_id, ord, depth, raw, byte_offset, byte_length, hash)
            VALUES (1, 1, 0, 0, 'b1', 0, 2, X'00');
         INSERT INTO block_props (block_id, key, value) VALUES (1, 'k', 'v');
         INSERT INTO block_drawers (block_id, name, byte_offset, byte_length) VALUES (1, 'D', 0, 1);
         INSERT INTO refs (source_block, type, target_page) VALUES (1, 'page-link', 1);",
    ).unwrap();

    // DELETE on files SETs NULL on pages.file_id (D-04 unresolved-page semantics).
    db.conn().execute("DELETE FROM files WHERE id = 1", []).unwrap();
    let file_id: Option<i64> = db
        .conn()
        .query_row("SELECT file_id FROM pages WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert!(
        file_id.is_none(),
        "files → pages should be ON DELETE SET NULL, got {:?}",
        file_id
    );

    // DELETE on pages CASCADEs to blocks (and from blocks to block_props / block_drawers / refs).
    db.conn().execute("DELETE FROM pages WHERE id = 1", []).unwrap();
    for table in ["blocks", "block_props", "block_drawers", "refs"] {
        let count: i64 = db
            .conn()
            .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "table {} should have cascaded to empty", table);
    }
}

#[test]
fn pages_name_unique_case_insensitive() {
    let tmp = TempDir::new().unwrap();
    let db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();

    db.conn()
        .execute(
            "INSERT INTO pages (id, name, kind) VALUES (1, 'Crypto', 'page')",
            [],
        )
        .unwrap();
    let dup = db.conn().execute(
        "INSERT INTO pages (id, name, kind) VALUES (2, 'crypto', 'page')",
        [],
    );
    assert!(
        dup.is_err(),
        "pages_name_idx COLLATE NOCASE should reject duplicate 'crypto' (D-03)"
    );
}

#[test]
fn refs_type_check_constraint() {
    let tmp = TempDir::new().unwrap();
    let db = Db::open_at(&tmp.path().join("foliom.db")).unwrap();
    db.conn().execute_batch(
        "INSERT INTO files (id, path, mtime_ns, size, hash) VALUES (1, 'p.md', 0, 0, X'00');
         INSERT INTO pages (id, file_id, name, kind) VALUES (1, 1, 'P', 'page');
         INSERT INTO blocks (id, page_id, ord, depth, raw, byte_offset, byte_length, hash)
            VALUES (1, 1, 0, 0, '', 0, 0, X'00');",
    ).unwrap();
    let bad = db.conn().execute(
        "INSERT INTO refs (source_block, type, target_page) VALUES (1, 'bogus', 1)",
        [],
    );
    assert!(bad.is_err(), "refs.type CHECK should reject 'bogus'");
}

#[test]
fn reopen_is_idempotent_and_preserves_user_version() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("foliom.db");

    let v1 = {
        let db = Db::open_at(&db_path).unwrap();
        db.conn()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap()
    };
    assert_eq!(v1, 1, "after first open, user_version should be 1 (one migration applied)");

    // Insert a sentinel; reopening must preserve it (no destructive re-migration).
    {
        let db = Db::open_at(&db_path).unwrap();
        db.conn()
            .execute(
                "INSERT INTO pages (id, name, kind) VALUES (99, 'sentinel', 'page')",
                [],
            )
            .unwrap();
    }

    // Reopen.
    let db = Db::open_at(&db_path).unwrap();
    let v2: i64 = db
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v2, 1, "reopen must not bump user_version (no new migrations)");
    let sentinel: i64 = db
        .conn()
        .query_row("SELECT id FROM pages WHERE name = 'sentinel'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sentinel, 99, "data must survive reopen");
}

#[test]
#[cfg(target_os = "linux")]
fn open_via_notes_root_resolver_places_db_outside_notes_folder() {
    let notes = TempDir::new().unwrap();
    let xdg = TempDir::new().unwrap();
    with_env("XDG_DATA_HOME", xdg.path(), || {
        // Open via the real resolver path.
        let db = Db::open(notes.path()).unwrap();
        // Sanity: a clean DB exposes the migrated schema.
        let tables: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(tables.contains(&"blocks".to_string()));
        // The resolved path is outside the notes folder (IDX-06 / T-04-01).
        let resolved = resolve_db_path(notes.path()).unwrap();
        let notes_canon = notes.path().canonicalize().unwrap();
        assert!(
            !resolved.starts_with(&notes_canon),
            "DB at {:?} must not live inside notes_root {:?}",
            resolved,
            notes_canon
        );
        assert!(
            resolved.starts_with(xdg.path().join("foliom")),
            "DB at {:?} should be under {:?}/foliom",
            resolved,
            xdg.path()
        );
    });
}
