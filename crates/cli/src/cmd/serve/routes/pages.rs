//! `/api/pages*` handlers.
//!
//! Three routes:
//!   - `GET /api/pages`                 → flat list (`PageSummary[]`).
//!   - `GET /api/pages/:name`           → nested block tree (`PageDetail`).
//!   - `GET /api/pages/:name/backlinks` → `Backlink[]`.
//!
//! All DB work runs inside `tokio::task::spawn_blocking` per D-25 — rusqlite
//! is synchronous and would otherwise stall the single-threaded runtime.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use rusqlite::{Connection, params};

use crate::cmd::serve::dto::{Backlink, Block, DrawerRef, PageDetail, PageSummary};
use crate::cmd::serve::format::{format_journal_title, parse_journal_name};
use crate::cmd::serve::state::AppState;

/// `GET /api/pages` — list every known page (resolved or otherwise), sorted
/// case-insensitively by name.
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<PageSummary>>, StatusCode> {
    let db = state.db.clone();
    let rows = tokio::task::spawn_blocking(move || {
        let guard = db.lock().expect("db poisoned");
        let conn = guard.conn();
        let mut stmt = conn.prepare(
            "SELECT name, kind = 'journal', file_id IS NOT NULL \
             FROM pages \
             ORDER BY name COLLATE NOCASE",
        )?;
        let rows: rusqlite::Result<Vec<PageSummary>> = stmt
            .query_map([], |row| {
                Ok(PageSummary {
                    name: row.get(0)?,
                    is_journal: row.get::<_, i64>(1)? != 0,
                    is_resolved: row.get::<_, i64>(2)? != 0,
                })
            })?
            .collect();
        rows
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "join error in /api/pages");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        tracing::error!(error = %e, "db error in /api/pages");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

/// `GET /api/pages/:name` — nested block tree.
///
/// axum decodes `%2F` at the path-extractor boundary, so `name` arrives as
/// the canonical page name (`Parent/Child`, not `Parent%2FChild`). Per
/// D-37 we never re-decode.
pub async fn detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<PageDetail>, StatusCode> {
    let db = state.db.clone();
    let detail = tokio::task::spawn_blocking(move || -> rusqlite::Result<Option<PageDetail>> {
        let guard = db.lock().expect("db poisoned");
        let conn = guard.conn();

        // Resolve the page row first. Missing → 404 from the outer layer.
        // Also join files.hash so mutation clients can use it as prev_hash.
        let page_row: Option<(i64, String, String, Option<Vec<u8>>)> = conn
            .query_row(
                "SELECT p.id, p.kind, p.name, f.hash \
                 FROM pages p \
                 LEFT JOIN files f ON f.id = p.file_id \
                 WHERE p.name = ?1 COLLATE NOCASE",
                params![&name],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        let Some((page_id, kind, canonical_name, file_hash_bytes)) = page_row else {
            return Ok(None);
        };
        let is_journal = kind == "journal";
        let formatted_title = if is_journal {
            parse_journal_name(&canonical_name).map(format_journal_title)
        } else {
            None
        };
        // Hex-encode the 32-byte BLAKE3 hash for the wire response. None for
        // unresolved pages (file_id IS NULL → no backing file).
        let file_hash: Option<String> = file_hash_bytes
            .filter(|h| h.len() == 32)
            .map(hex::encode);

        // Pull all blocks for this page in `ord` order.
        let mut block_stmt = conn.prepare(
            "SELECT id, ord, depth, raw FROM blocks WHERE page_id = ?1 ORDER BY ord",
        )?;
        let raw_blocks: Vec<(i64, i64, i32, String)> = block_stmt
            .query_map(params![page_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?;

        // Prefetch properties and drawers in two batched queries; build
        // per-block lookups keyed by block id. (Avoids N+1 over `block_props`.)
        let mut props_stmt = conn.prepare(
            "SELECT bp.block_id, bp.key, bp.value \
             FROM block_props bp \
             JOIN blocks b ON b.id = bp.block_id \
             WHERE b.page_id = ?1",
        )?;
        let mut props_map: std::collections::HashMap<i64, Vec<[String; 2]>> =
            std::collections::HashMap::new();
        for row in props_stmt.query_map(params![page_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })? {
            let (bid, k, v) = row?;
            props_map.entry(bid).or_default().push([k, v]);
        }

        let mut draw_stmt = conn.prepare(
            "SELECT bd.block_id, bd.name, bd.byte_offset, bd.byte_length \
             FROM block_drawers bd \
             JOIN blocks b ON b.id = bd.block_id \
             WHERE b.page_id = ?1",
        )?;
        let mut draw_map: std::collections::HashMap<i64, Vec<DrawerRef>> =
            std::collections::HashMap::new();
        for row in draw_stmt.query_map(params![page_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })? {
            let (bid, name, off, len) = row?;
            draw_map.entry(bid).or_default().push(DrawerRef {
                name,
                byte_offset: off,
                byte_length: len,
            });
        }

        // Build the nested tree by walking blocks in `ord` order and
        // maintaining a depth stack. Each stack frame stores the block's
        // depth and a path of indices into `roots` we can descend back into.
        let blocks = assemble_tree(raw_blocks, &mut props_map, &mut draw_map);

        Ok(Some(PageDetail {
            name: canonical_name,
            is_journal,
            formatted_title,
            blocks,
            file_hash,
            id: page_id,
        }))
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "join error in /api/pages/:name");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        tracing::error!(error = %e, "db error in /api/pages/:name");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    detail.map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// Build the full block tree for `page_id` from the database.
///
/// Used by both the read-only `detail` handler and the mutation handlers in
/// `routes/blocks.rs` to return the updated subtree after each write.
pub(crate) fn build_page_block_tree(
    conn: &Connection,
    page_id: i64,
) -> rusqlite::Result<Vec<Block>> {
    let mut block_stmt = conn.prepare(
        "SELECT id, ord, depth, raw FROM blocks WHERE page_id = ?1 ORDER BY ord",
    )?;
    let raw_blocks: Vec<(i64, i64, i32, String)> = block_stmt
        .query_map(params![page_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut props_stmt = conn.prepare(
        "SELECT bp.block_id, bp.key, bp.value \
         FROM block_props bp \
         JOIN blocks b ON b.id = bp.block_id \
         WHERE b.page_id = ?1",
    )?;
    let mut props_map: std::collections::HashMap<i64, Vec<[String; 2]>> =
        std::collections::HashMap::new();
    for row in props_stmt.query_map(params![page_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })? {
        let (bid, k, v) = row?;
        props_map.entry(bid).or_default().push([k, v]);
    }

    let mut draw_stmt = conn.prepare(
        "SELECT bd.block_id, bd.name, bd.byte_offset, bd.byte_length \
         FROM block_drawers bd \
         JOIN blocks b ON b.id = bd.block_id \
         WHERE b.page_id = ?1",
    )?;
    let mut draw_map: std::collections::HashMap<i64, Vec<DrawerRef>> =
        std::collections::HashMap::new();
    for row in draw_stmt.query_map(params![page_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })? {
        let (bid, name, off, len) = row?;
        draw_map.entry(bid).or_default().push(DrawerRef {
            name,
            byte_offset: off,
            byte_length: len,
        });
    }

    Ok(assemble_tree(raw_blocks, &mut props_map, &mut draw_map))
}

/// Walk the flat list and produce the nested `Vec<Block>` shape.
///
/// Indices instead of `&mut` references because Rust's borrow checker
/// rejects the equivalent reference-stack form (the compiler cannot prove
/// the parent borrows are still live across pushes into siblings).
fn assemble_tree(
    flat: Vec<(i64, i64, i32, String)>,
    props_map: &mut std::collections::HashMap<i64, Vec<[String; 2]>>,
    draw_map: &mut std::collections::HashMap<i64, Vec<DrawerRef>>,
) -> Vec<Block> {
    let mut roots: Vec<Block> = Vec::new();
    // Path of indices from `roots` down to the current cursor. Empty = next
    // push goes to `roots`.
    let mut path: Vec<(i32, usize)> = Vec::new();

    for (id, _ord, depth, raw) in flat {
        // Pop frames with depth >= this block's depth.
        while let Some(&(top_depth, _)) = path.last() {
            if top_depth >= depth {
                path.pop();
            } else {
                break;
            }
        }

        let new_block = Block {
            id,
            depth,
            raw,
            properties: props_map.remove(&id).unwrap_or_default(),
            drawers: draw_map.remove(&id).unwrap_or_default(),
            children: Vec::new(),
        };

        // Find the parent's children Vec by walking the path, then push.
        let parent_children: &mut Vec<Block> = walk_to_children(&mut roots, &path);
        parent_children.push(new_block);
        let new_idx = parent_children.len() - 1;
        path.push((depth, new_idx));
    }

    roots
}

/// Descend `roots` following `path` and return the `children` Vec where the
/// next sibling should be appended. Empty path → return `roots` itself.
fn walk_to_children<'a>(
    roots: &'a mut Vec<Block>,
    path: &[(i32, usize)],
) -> &'a mut Vec<Block> {
    let mut current: &mut Vec<Block> = roots;
    for &(_, idx) in path {
        current = &mut current[idx].children;
    }
    current
}

/// `GET /api/pages/:name/backlinks` — blocks that reference this page via
/// `refs.target_page`, joined back to source page + block.
pub async fn backlinks(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Backlink>>, StatusCode> {
    let db = state.db.clone();
    let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<Backlink>> {
        let guard = db.lock().expect("db poisoned");
        let conn = guard.conn();
        let mut stmt = conn.prepare(
            "SELECT p.name, b.id, substr(b.raw, 1, 200) \
             FROM refs r \
             JOIN blocks b ON b.id = r.source_block \
             JOIN pages  p ON p.id = b.page_id \
             JOIN pages  tp ON tp.id = r.target_page \
             WHERE tp.name = ?1 COLLATE NOCASE \
             ORDER BY p.name, b.ord \
             LIMIT 500",
        )?;
        let rows: rusqlite::Result<Vec<Backlink>> = stmt
            .query_map(params![&name], |r| {
                Ok(Backlink {
                    page: r.get(0)?,
                    block_id: r.get(1)?,
                    snippet: r.get(2)?,
                })
            })?
            .collect();
        rows
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "join error in /api/pages/:name/backlinks");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        tracing::error!(error = %e, "db error in /api/pages/:name/backlinks");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}
