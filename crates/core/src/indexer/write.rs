//! Per-file transactional writes + the reindex orchestrator body.
//!
//! Three public functions are exposed:
//!   - [`insert_file_tx`]    — first-time insert of a file's full content.
//!   - [`reparse_file_tx`]   — drop existing blocks/refs and re-insert.
//!   - [`delete_file_cascade`] — remove a file (CASCADE clears blocks/refs/FTS).
//!
//! Per AP-5 every function expects to be called inside an *already-open*
//! `rusqlite::Transaction` and does NOT commit on its own.
//!
//! Per AP-1 `extract_refs` is called per-block inside `insert_blocks`,
//! never on whole-file text.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use rusqlite::{Transaction, params};

use crate::parser::ast::{ExtractedRef, RefKind, extract_refs};
use crate::parser::segment::segment;
use crate::path::RelativePath;
use crate::scanner::{IgnoreSet, ScanEntry, walk};
use crate::scanner::config_edn::read_hidden;
use crate::storage::Db;

use super::page_name::{PageInfo, PageKind, derive_page_info};
use super::{IndexerError, ReindexMode, ReindexStats};

const PRELUDE_DEPTH_SENTINEL: i64 = -1;

/// Insert a file row, its self-page row, and all block/prop/drawer/ref rows.
/// Returns the new `files.id`.
pub fn insert_file_tx(
    tx: &Transaction<'_>,
    rel: &RelativePath,
    bytes: &[u8],
    mtime_ns: i64,
    size: u64,
    hash: &[u8],
) -> Result<i64, IndexerError> {
    tx.execute(
        "INSERT INTO files (path, mtime_ns, size, hash) VALUES (?, ?, ?, ?)",
        params![rel.as_str(), mtime_ns, size as i64, hash],
    )?;
    let file_id = tx.last_insert_rowid();

    let info = derive_page_info(rel);
    let page_id = ensure_self_page_row(tx, file_id, &info)?;
    insert_all_blocks(tx, page_id, bytes)?;

    Ok(file_id)
}

/// Drop existing blocks (CASCADE handles props/drawers/refs/FTS), update
/// the `files` row, and re-insert blocks for the new bytes.
pub fn reparse_file_tx(
    tx: &Transaction<'_>,
    file_id: i64,
    rel: &RelativePath,
    bytes: &[u8],
    mtime_ns: i64,
    size: u64,
    hash: &[u8],
) -> Result<(), IndexerError> {
    // Clear all blocks for the page(s) this file backs. The CASCADE on
    // `blocks.page_id → pages.id` is only triggered when the page row is
    // deleted, not when the file row is updated — so we explicitly DELETE
    // FROM blocks scoped to the page that this file backs.
    tx.execute(
        "DELETE FROM blocks WHERE page_id IN (SELECT id FROM pages WHERE file_id = ?)",
        params![file_id],
    )?;

    tx.execute(
        "UPDATE files SET mtime_ns = ?, size = ?, hash = ? WHERE id = ?",
        params![mtime_ns, size as i64, hash, file_id],
    )?;

    // Page name might have changed if the file was renamed. For Phase 1
    // we do not handle in-place renames (the scanner sees them as
    // delete-then-insert), so we trust the existing page row's name.
    let info = derive_page_info(rel);
    let page_id = ensure_self_page_row(tx, file_id, &info)?;
    insert_all_blocks(tx, page_id, bytes)?;

    Ok(())
}

/// Touch only the mtime/size on the `files` row — content is unchanged.
pub fn update_file_mtime(
    tx: &Transaction<'_>,
    file_id: i64,
    mtime_ns: i64,
    size: u64,
) -> Result<(), IndexerError> {
    tx.execute(
        "UPDATE files SET mtime_ns = ?, size = ? WHERE id = ?",
        params![mtime_ns, size as i64, file_id],
    )?;
    Ok(())
}

/// Delete a file row by its relative path. The page row's `file_id` is set
/// to NULL via ON DELETE SET NULL (D-04 — unresolved page survives), but
/// blocks belonging to that page also need to be wiped because there is no
/// longer a backing file.
pub fn delete_file_cascade(
    tx: &Transaction<'_>,
    rel: &RelativePath,
) -> Result<(), IndexerError> {
    // Look up the file_id (and any backed page) before deletion so we can
    // also delete the now-orphaned page row. We treat an unresolved page
    // (file gone) as "delete the page entirely" because Phase 1 has no
    // separate UI to manage orphaned-page-with-no-blocks; backlinks to it
    // will be recreated on the next reindex if some other file mentions it.
    let file_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM files WHERE path = ?",
            params![rel.as_str()],
            |row| row.get(0),
        )
        .ok();

    if let Some(fid) = file_id {
        // Delete the page row that this file backs (CASCADEs blocks/refs/FTS).
        tx.execute("DELETE FROM pages WHERE file_id = ?", params![fid])?;
        // Delete the file row itself.
        tx.execute("DELETE FROM files WHERE id = ?", params![fid])?;
    }

    Ok(())
}

// -- private helpers --------------------------------------------------------

/// Ensure the `pages` row backing `file_id` exists.
///
/// Lookup strategy:
///   1. Look up by NOCASE name (D-03 — `Foo` and `foo` resolve to one row).
///   2. If found with `file_id IS NULL` → UPDATE to set this file as the backing.
///   3. If found with a *different* `file_id` → log warning (case collision
///      between two real files on a case-sensitive filesystem) and reuse the row.
///   4. Otherwise INSERT.
fn ensure_self_page_row(
    tx: &Transaction<'_>,
    file_id: i64,
    info: &PageInfo,
) -> Result<i64, IndexerError> {
    let existing: Option<(i64, Option<i64>)> = tx
        .query_row(
            "SELECT id, file_id FROM pages WHERE name = ? COLLATE NOCASE",
            params![&info.name],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .ok();

    let kind_str = match info.kind {
        PageKind::Page => "page",
        PageKind::Journal => "journal",
    };

    match existing {
        Some((page_id, None)) => {
            // Unresolved — claim it.
            tx.execute(
                "UPDATE pages SET file_id = ?, kind = ?, journal_date = ? WHERE id = ?",
                params![file_id, kind_str, &info.journal_date, page_id],
            )?;
            Ok(page_id)
        }
        Some((page_id, Some(existing_fid))) if existing_fid == file_id => {
            // Already pointing at us — refresh kind/journal_date in case
            // the file moved between pages/ and journals/.
            tx.execute(
                "UPDATE pages SET kind = ?, journal_date = ? WHERE id = ?",
                params![kind_str, &info.journal_date, page_id],
            )?;
            Ok(page_id)
        }
        Some((page_id, Some(other))) => {
            tracing::warn!(
                page_id,
                this_file = file_id,
                other_file = other,
                name = %info.name,
                "case collision — two files share a NOCASE page name; reusing existing row"
            );
            Ok(page_id)
        }
        None => {
            tx.execute(
                "INSERT INTO pages (file_id, name, kind, journal_date) VALUES (?, ?, ?, ?)",
                params![file_id, &info.name, kind_str, &info.journal_date],
            )?;
            Ok(tx.last_insert_rowid())
        }
    }
}

/// Look up or INSERT an unresolved page row (`file_id = NULL`, D-04).
fn ensure_unresolved_page(
    tx: &Transaction<'_>,
    name: &str,
) -> Result<i64, IndexerError> {
    if let Ok(id) = tx.query_row(
        "SELECT id FROM pages WHERE name = ? COLLATE NOCASE",
        params![name],
        |row| row.get::<_, i64>(0),
    ) {
        return Ok(id);
    }
    // INSERT then return rowid. Use 'page' kind by default — if a backing
    // file appears later, ensure_self_page_row will UPDATE the kind.
    tx.execute(
        "INSERT INTO pages (file_id, name, kind, journal_date) VALUES (NULL, ?, 'page', NULL)",
        params![name],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Segment the file, insert every block (with parent linkage from a depth
/// stack), then insert per-block properties / drawers / refs.
fn insert_all_blocks(
    tx: &Transaction<'_>,
    page_id: i64,
    bytes: &[u8],
) -> Result<(), IndexerError> {
    let blocks = segment(bytes);

    // depth_stack[i] = (db_depth, last_inserted_block_id_at_that_depth).
    // We use the DB depth (i64, prelude = -1) so the comparison is in the
    // same domain.
    let mut depth_stack: Vec<(i64, i64)> = Vec::new();

    for (ord, blk) in blocks.iter().enumerate() {
        let db_depth: i64 = if blk.depth == u8::MAX {
            PRELUDE_DEPTH_SENTINEL
        } else {
            blk.depth as i64
        };

        // Pop stack entries with depth >= db_depth.
        while let Some(&(top_depth, _)) = depth_stack.last() {
            if top_depth >= db_depth {
                depth_stack.pop();
            } else {
                break;
            }
        }
        let parent_id: Option<i64> = depth_stack.last().map(|&(_, id)| id);

        let hash = blake3::hash(blk.raw.as_bytes());
        let hash_bytes = hash.as_bytes();

        tx.execute(
            "INSERT INTO blocks
                (page_id, parent_id, ord, depth, raw, byte_offset, byte_length, hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                page_id,
                parent_id,
                ord as i64,
                db_depth,
                &blk.raw,
                blk.byte_offset as i64,
                blk.byte_length as i64,
                hash_bytes.as_slice(),
            ],
        )?;
        let block_id = tx.last_insert_rowid();

        depth_stack.push((db_depth, block_id));

        // Properties (D-05).
        for (k, v) in &blk.properties {
            tx.execute(
                "INSERT OR REPLACE INTO block_props (block_id, key, value) VALUES (?, ?, ?)",
                params![block_id, k, v],
            )?;
        }
        // Drawers (D-06).
        for d in &blk.drawers {
            tx.execute(
                "INSERT OR REPLACE INTO block_drawers
                    (block_id, name, byte_offset, byte_length) VALUES (?, ?, ?, ?)",
                params![block_id, &d.name, d.byte_offset as i64, d.byte_length as i64],
            )?;
        }
        // Refs per block (AP-1 — never on whole file).
        insert_refs_for_block(tx, block_id, &blk.raw)?;
    }

    Ok(())
}

/// Public alias for use by plan 03-03 mutation handlers.
///
/// Extracts `[[link]]`, `#tag`, and `#[[composite tag]]` from `raw`, resolves
/// (or creates) the target page rows, and inserts `refs` rows.
///
/// Callers must have already `DELETE FROM refs WHERE source_block = block_id`
/// before calling this so re-indexing is idempotent.
pub fn insert_refs_for_block_tx(
    tx: &Transaction<'_>,
    block_id: i64,
    raw: &str,
) -> Result<(), IndexerError> {
    insert_refs_for_block(tx, block_id, raw)
}

fn insert_refs_for_block(
    tx: &Transaction<'_>,
    block_id: i64,
    raw: &str,
) -> Result<(), IndexerError> {
    let refs: Vec<ExtractedRef> = extract_refs(raw);
    // Dedup within a block: refs PK is (source_block, type, target_page).
    // INSERT OR IGNORE handles that, but we also pre-dedup to skip the
    // per-target page lookup when an identical ref repeats.
    let mut seen: HashSet<(RefKind, String)> = HashSet::new();

    for r in refs {
        if !seen.insert((r.kind.clone(), r.target.clone())) {
            continue;
        }
        let target_id = ensure_unresolved_page(tx, &r.target)?;
        let kind_str = match r.kind {
            RefKind::Tag => "tag",
            RefKind::PageLink => "page-link",
        };
        tx.execute(
            "INSERT OR IGNORE INTO refs (source_block, type, target_page) VALUES (?, ?, ?)",
            params![block_id, kind_str, target_id],
        )?;
        // Populate the discoverability tags index for tag-kind refs.
        if matches!(r.kind, RefKind::Tag) {
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?)",
                params![&r.target],
            )?;
        }
    }

    Ok(())
}

// -- reindex orchestrator ---------------------------------------------------

/// Internal `reindex` body. See `mod.rs::reindex` for the public entry point.
pub(crate) fn reindex_impl(
    db: &mut Db,
    root: &Path,
    mode: ReindexMode,
) -> Result<ReindexStats, IndexerError> {
    let mut stats = ReindexStats::default();

    // Load IgnoreSet + extend from logseq/config.edn :hidden if present.
    let mut ignore = IgnoreSet::default_logseq();
    let hidden_path = root.join("logseq/config.edn");
    if hidden_path.exists() {
        let extra = read_hidden(&hidden_path);
        if !extra.is_empty() {
            ignore.extend_from_config_edn(extra);
        }
    }

    // Load cache from the `files` table.
    // Map: RelativePath → (file_id, mtime_ns, size, hash[32]).
    let mut known: HashMap<String, (i64, i64, i64, Vec<u8>)> = HashMap::new();
    {
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT id, path, mtime_ns, size, hash FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Vec<u8>>(4)?,
            ))
        })?;
        for r in rows {
            let (id, path, mtime, size, hash) = r?;
            known.insert(path, (id, mtime, size, hash));
        }
    }

    let mut seen: HashSet<String> = HashSet::new();

    let scan_entries: Vec<ScanEntry> = walk(root, &ignore).collect();
    stats.scanned = scan_entries.len();

    for entry in scan_entries {
        let rel = match RelativePath::from_filesystem(&entry.path, root) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(path = %entry.path.display(), error = %err, "skipping file with invalid path");
                continue;
            }
        };
        let rel_str = rel.as_str().to_string();
        seen.insert(rel_str.clone());

        let cached = known.get(&rel_str).cloned();

        // Fast-path: incremental + (mtime, size) match → unchanged.
        if mode == ReindexMode::Incremental {
            if let Some((_, cached_mtime, cached_size, _)) = &cached {
                if *cached_mtime == entry.mtime_ns && *cached_size as u64 == entry.size {
                    stats.unchanged += 1;
                    continue;
                }
            }
        }

        // Need to read the bytes and hash.
        let bytes = match fs::read(&entry.path) {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(path = %entry.path.display(), error = %err, "read failed; skipping");
                continue;
            }
        };
        let new_hash = blake3::hash(&bytes);
        let new_hash_bytes = new_hash.as_bytes();

        let tx = db.conn_mut().transaction()?;
        let result: Result<(), IndexerError> = (|| {
            match cached {
                Some((file_id, _cached_mtime, _cached_size, cached_hash)) => {
                    if cached_hash.as_slice() == new_hash_bytes.as_slice() {
                        // Content unchanged — just touch the metadata row.
                        update_file_mtime(&tx, file_id, entry.mtime_ns, entry.size)?;
                        stats.mtime_touched += 1;
                    } else {
                        // Content changed — full reparse.
                        reparse_file_tx(
                            &tx,
                            file_id,
                            &rel,
                            &bytes,
                            entry.mtime_ns,
                            entry.size,
                            new_hash_bytes.as_slice(),
                        )?;
                        stats.modified += 1;
                    }
                }
                None => {
                    insert_file_tx(
                        &tx,
                        &rel,
                        &bytes,
                        entry.mtime_ns,
                        entry.size,
                        new_hash_bytes.as_slice(),
                    )?;
                    stats.added += 1;
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                tx.commit()?;
            }
            Err(err) => {
                // Transaction is rolled back on drop.
                drop(tx);
                tracing::warn!(
                    path = %entry.path.display(),
                    error = %err,
                    "file failed to index; rolled back"
                );
            }
        }
    }

    // Deletions: anything in `known` but not in `seen`.
    let to_delete: Vec<String> = known
        .keys()
        .filter(|k| !seen.contains(k.as_str()))
        .cloned()
        .collect();
    for path in to_delete {
        let rel = RelativePath::from_storage_str(&path);
        let tx = db.conn_mut().transaction()?;
        delete_file_cascade(&tx, &rel)?;
        tx.commit()?;
        stats.deleted += 1;
    }

    Ok(stats)
}
