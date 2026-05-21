-- Phase 1 schema, applied in a single migration via `rusqlite_migration`.
--
-- Sources of truth:
--   - CONTEXT.md  D-03  (`#tag` and `[[page]]` resolve to the same pages.id)
--   - CONTEXT.md  D-04  (unresolved pages: file_id = NULL)
--   - CONTEXT.md  D-05  (block properties → auxiliary table; queryable by key)
--   - CONTEXT.md  D-06  (`:LOGBOOK:` drawers → opaque blobs keyed by byte_offset)
--   - CONTEXT.md  D-14  (blocks materialized with raw AND byte_offset+byte_length)
--   - CONTEXT.md  D-15  (files.path is NFC + forward-slash, relative to notes root)
--   - CONTEXT.md  D-16  (BLAKE3 used for files.hash and blocks.hash, 32 bytes)
--   - RESEARCH.md §Schema + Migrations (canonical DDL)
--
-- This migration is referenced by `include_str!` from `crates/core/src/storage/mod.rs`.
-- DO NOT edit in place once shipped; future schema changes go to 002_*.sql etc.

PRAGMA foreign_keys = ON;

-- files: every markdown file on disk we know about, keyed by NFC + forward-slash
-- relative path. mtime_ns + size + hash form the incremental-reindex decision key.
CREATE TABLE files (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,            -- D-15: NFC + forward-slash, relative to notes root
    mtime_ns  INTEGER NOT NULL,                -- nanoseconds since Unix epoch
    size      INTEGER NOT NULL,
    hash      BLOB NOT NULL                    -- BLAKE3(file_bytes), 32 bytes (D-16)
);
CREATE INDEX files_hash_idx ON files(hash);

-- pages: a canonical page name. May or may not have a backing file (D-04 unresolved pages).
-- name is COLLATE NOCASE so `#Crypto` and `[[Crypto]]` resolve to the same row (D-03).
CREATE TABLE pages (
    id            INTEGER PRIMARY KEY,
    file_id       INTEGER REFERENCES files(id) ON DELETE SET NULL,  -- NULL = unresolved page (D-04)
    name          TEXT NOT NULL,               -- canonical page name (after %2F decode)
    kind          TEXT NOT NULL,               -- 'journal' | 'page'
    journal_date  TEXT                         -- ISO 8601 'YYYY-MM-DD' for journals, NULL otherwise
);
CREATE UNIQUE INDEX pages_name_idx ON pages(name COLLATE NOCASE);
CREATE INDEX pages_file_idx ON pages(file_id);

-- blocks: D-14 materialization. `raw` is the verbatim block text (used for FTS and cheap
-- reads); `byte_offset` + `byte_length` index back into the source file for Phase 3
-- byte-splice writeback. Both pairs coexist by design. depth = -1 is the page prelude.
CREATE TABLE blocks (
    id           INTEGER PRIMARY KEY,
    page_id      INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    parent_id    INTEGER REFERENCES blocks(id) ON DELETE CASCADE,
    ord          INTEGER NOT NULL,             -- sibling order within parent
    depth        INTEGER NOT NULL,             -- TAB count; 0 = top-level; -1 = page prelude
    raw          TEXT NOT NULL,                -- D-14: full block text (FTS + cheap reads)
    byte_offset  INTEGER NOT NULL,             -- D-14: for byte-splice writeback (Phase 3)
    byte_length  INTEGER NOT NULL,
    hash         BLOB NOT NULL                 -- BLAKE3(raw)
);
CREATE INDEX blocks_page_ord_idx ON blocks(page_id, ord);
CREATE INDEX blocks_parent_idx   ON blocks(parent_id);

-- D-05: block properties as a small auxiliary table. Indexed by key so future queries
-- like "all blocks where template:: is set" don't need JSON1.
CREATE TABLE block_props (
    block_id INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    value    TEXT NOT NULL,
    PRIMARY KEY (block_id, key)
);
CREATE INDEX block_props_key_idx ON block_props(key);

-- D-06: drawers preserved as opaque blobs anchored to a parent block; addressed by
-- byte range in the source file so writeback never reorders/normalizes drawer content.
CREATE TABLE block_drawers (
    block_id    INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    byte_offset INTEGER NOT NULL,
    byte_length INTEGER NOT NULL,
    PRIMARY KEY (block_id, byte_offset)
);

-- tags: kept for symmetry with refs.type and future query patterns (Phase 1 plan 04
-- chose KEEP rather than OMIT per RESEARCH note; cost is one usually-empty table).
CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

-- refs: D-03 — both `#tag` and `[[link]]` resolve to a page row; `type` preserves the
-- syntactic distinction at the source so UI can render chip-vs-link.
CREATE TABLE refs (
    source_block INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    type         TEXT NOT NULL CHECK (type IN ('tag', 'page-link')),
    target_page  INTEGER REFERENCES pages(id) ON DELETE CASCADE,
    PRIMARY KEY (source_block, type, target_page)
);
CREATE INDEX refs_target_idx ON refs(target_page, type);

-- blocks_fts: D-14 external-content FTS5 over blocks.raw. Inverted index lives in FTS;
-- canonical text stays in `blocks.raw`. unicode61 + remove_diacritics=2 matches the
-- "café" ≡ "cafe" search behavior expected by RF-30.
CREATE VIRTUAL TABLE blocks_fts USING fts5(
    raw,
    content='blocks', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- Triggers keep blocks_fts in lockstep with blocks. The `('delete', rowid, raw)`
-- form is the canonical external-content FTS5 invalidate-row command.
CREATE TRIGGER blocks_ai AFTER INSERT ON blocks BEGIN
    INSERT INTO blocks_fts(rowid, raw) VALUES (new.id, new.raw);
END;
CREATE TRIGGER blocks_ad AFTER DELETE ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, raw) VALUES('delete', old.id, old.raw);
END;
CREATE TRIGGER blocks_au AFTER UPDATE ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, raw) VALUES('delete', old.id, old.raw);
    INSERT INTO blocks_fts(rowid, raw) VALUES (new.id, new.raw);
END;
