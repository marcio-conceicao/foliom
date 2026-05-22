//! Plan 03-03 — Mutation REST handlers for `/api/blocks*`.
//!
//! Four mutation endpoints:
//!   - `PUT  /api/blocks/:id`            → edit block raw text (conflict-detected).
//!   - `POST /api/blocks`                → create a new block.
//!   - `PATCH /api/blocks/:id/structure` → indent / outdent / move.
//!   - `DELETE /api/blocks/:id`          → remove a block.
//!
//! All handlers follow the same pattern (per D-25):
//!   1. Clone the `Arc<Mutex<Db>>` from `AppState`.
//!   2. Run the entire DB+IO work inside `tokio::task::spawn_blocking` —
//!      rusqlite is synchronous.
//!   3. Return `Json<MutationResponse>` on success or `ApiError` on failure.
//!
//! Conflict detection (T-03-06): every mutating endpoint requires `prevHash`
//! (hex-encoded BLAKE3 of the current file contents). If `files.hash` no
//! longer matches, return 409 and ROLLBACK.
//!
//! Self-write registration (SNC-02): `atomic_write_md` registers the new
//! file hash in `AppState::self_writes` BEFORE the rename so the Phase 4
//! watcher cannot echo-trigger a redundant reindex.
//!
//! No-id-injection (D-13 / PRD §5.6): handlers ONLY write the `raw` string
//! supplied by the client. No Foliom-generated `id::` properties or `((uuid))`
//! block refs are ever appended. This invariant is pinned by the
//! `t2_no_id_injection_after_put` integration test.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use rusqlite::params;
use serde::Deserialize;

use foliom_core::indexer::write::insert_refs_for_block_tx;
use foliom_core::mutation::splice_block;
use foliom_core::sync::atomic_write_md;

use crate::cmd::serve::dto::{
    CreateBlockResponse, ErrorResponse, MutationResponse,
    PatchBlockStructureRequest, PostBlockRequest, PutBlockRequest,
};
use crate::cmd::serve::routes::pages::build_page_block_tree;
use crate::cmd::serve::state::AppState;

// ─── API error type ───────────────────────────────────────────────────────────

/// Thin error enum that maps handler failures to HTTP status codes.
/// Returned from every mutation handler via `Result<_, ApiError>`.
#[derive(Debug)]
pub enum ApiError {
    /// `404 Not Found` — block or page id not in the database.
    NotFound,
    /// `409 Conflict` — `prev_hash` doesn't match `files.hash`.
    Stale { current_file_hash: String },
    /// `400 Bad Request` — client supplied an invalid argument.
    BadRequest(String),
    /// `500 Internal Server Error` — unexpected DB / IO failure.
    Internal(String),
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<foliom_core::indexer::IndexerError> for ApiError {
    fn from(e: foliom_core::indexer::IndexerError) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "not_found".to_string(),
                    current_file_hash: None,
                }),
            )
                .into_response(),

            ApiError::Stale { current_file_hash } => (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "stale".to_string(),
                    current_file_hash: Some(current_file_hash),
                }),
            )
                .into_response(),

            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: msg,
                    current_file_hash: None,
                }),
            )
                .into_response(),

            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "mutation handler internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "internal".to_string(),
                        current_file_hash: None,
                    }),
                )
                    .into_response()
            }
        }
    }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Fetch block + file metadata needed by every mutation handler.
///
/// The `blocks` table has no direct `file_id` column — it reaches the file
/// through `blocks.page_id → pages.file_id`. This join resolves the full
/// chain.
///
/// Returns `(file_id, current_hash_bytes, abs_path, byte_offset, byte_length, page_id)`.
fn fetch_block_file_info(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
    block_id: i64,
) -> Result<(i64, Vec<u8>, std::path::PathBuf, i64, i64, i64), ApiError> {
    let row: Option<(i64, i64, i64, i64, Vec<u8>, String)> = conn
        .query_row(
            "SELECT p.file_id, b.byte_offset, b.byte_length, b.page_id, \
                    f.hash, f.path \
             FROM blocks b \
             JOIN pages  p ON p.id  = b.page_id \
             JOIN files  f ON f.id  = p.file_id \
             WHERE b.id = ?1",
            params![block_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .ok();
    let (file_id, byte_offset, byte_length, page_id, hash_bytes, rel_path) =
        row.ok_or(ApiError::NotFound)?;
    let abs_path = root.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    Ok((file_id, hash_bytes, abs_path, byte_offset, byte_length, page_id))
}

/// Verify that `prev_hash_hex` matches `current_hash`. Returns `Stale` on mismatch.
fn verify_prev_hash(current_hash: &[u8], prev_hash_hex: &str) -> Result<(), ApiError> {
    let supplied = hex::decode(prev_hash_hex)
        .map_err(|_| ApiError::BadRequest("invalid prev_hash hex".to_string()))?;
    if supplied != current_hash {
        return Err(ApiError::Stale {
            current_file_hash: hex::encode(current_hash),
        });
    }
    Ok(())
}

// ─── PUT /api/blocks/:id ──────────────────────────────────────────────────────

/// Edit a block's raw text. Body: `{ raw, prevHash }`.
pub async fn put_block(
    State(app): State<AppState>,
    Path(block_id): Path<i64>,
    Json(req): Json<PutBlockRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    let db = app.db.clone();
    let root = app.root.clone();
    let self_writes = app.self_writes.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<MutationResponse, ApiError> {
        let mut guard = db.lock().map_err(|_| ApiError::Internal("db poisoned".into()))?;
        let conn = guard.conn_mut();

        let (file_id, hash_bytes, abs_path, byte_offset, old_byte_length, page_id) =
            fetch_block_file_info(conn, &root, block_id)?;
        verify_prev_hash(&hash_bytes, &req.prev_hash)?;

        let original = std::fs::read(&abs_path)?;

        let new_raw_bytes = req.raw.as_bytes();
        let new_bytes = splice_block(
            &original,
            byte_offset as usize,
            old_byte_length as usize,
            new_raw_bytes,
        );
        let new_len = new_raw_bytes.len() as i64;
        let delta = new_len - old_byte_length;

        // Persist atomically — registers hash in SelfWriteSet BEFORE rename.
        let new_file_hash: [u8; 32] = atomic_write_md(&abs_path, &new_bytes, &self_writes)?;

        // SQL transaction AFTER disk write.
        let tx = conn.transaction().map_err(ApiError::from)?;

        tx.execute(
            "UPDATE files SET hash = ? WHERE id = ?",
            params![new_file_hash.as_slice(), file_id],
        )?;

        tx.execute(
            "UPDATE blocks SET raw = ?, byte_length = ? WHERE id = ?",
            params![&req.raw, new_len, block_id],
        )?;

        if delta != 0 {
            tx.execute(
                "UPDATE blocks SET byte_offset = byte_offset + ? \
                 WHERE page_id = ? AND byte_offset > ?",
                params![delta, page_id, byte_offset],
            )?;
        }

        tx.execute(
            "DELETE FROM refs WHERE source_block = ?",
            params![block_id],
        )?;
        insert_refs_for_block_tx(&tx, block_id, &req.raw)?;

        tx.commit()?;

        let block_subtree = build_page_block_tree(guard.conn(), page_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(MutationResponse {
            block_subtree,
            file_hash: hex::encode(new_file_hash),
            dirty_block_ids: vec![block_id],
        })
    })
    .await
    .map_err(|e| ApiError::Internal(format!("join error: {e}")))?;

    result.map(Json)
}

// ─── POST /api/blocks ─────────────────────────────────────────────────────────

/// Create a new block. Body: `{ pageId, parentId?, ord, depth, raw, prevHash }`.
pub async fn post_block(
    State(app): State<AppState>,
    Json(req): Json<PostBlockRequest>,
) -> Result<(StatusCode, Json<CreateBlockResponse>), ApiError> {
    let db = app.db.clone();
    let root = app.root.clone();
    let self_writes = app.self_writes.clone();

    let result =
        tokio::task::spawn_blocking(move || -> Result<CreateBlockResponse, ApiError> {
            let mut guard =
                db.lock().map_err(|_| ApiError::Internal("db poisoned".into()))?;
            let conn = guard.conn_mut();

            // Fetch file info for the page (conflict detection at page level).
            let row: Option<(i64, Vec<u8>, String)> = conn
                .query_row(
                    "SELECT f.id, f.hash, f.path FROM files f \
                     JOIN pages p ON p.file_id = f.id \
                     WHERE p.id = ?1",
                    params![req.page_id],
                    |r| Ok((r.get(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, String>(2)?)),
                )
                .ok();
            let (file_id, hash_bytes, rel_path) = row.ok_or(ApiError::NotFound)?;
            verify_prev_hash(&hash_bytes, &req.prev_hash)?;

            let abs_path =
                root.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let original = std::fs::read(&abs_path)?;

            // Find insertion byte offset: after the last block in the page.
            let insert_byte_offset: i64 = {
                let end: Option<i64> = conn
                    .query_row(
                        "SELECT MAX(byte_offset + byte_length) \
                         FROM blocks WHERE page_id = ?",
                        params![req.page_id],
                        |r| r.get::<_, Option<i64>>(0),
                    )
                    .ok()
                    .flatten();
                end.unwrap_or(original.len() as i64)
            };

            let new_raw_bytes = req.raw.as_bytes();
            let new_bytes = splice_block(
                &original,
                insert_byte_offset as usize,
                0,
                new_raw_bytes,
            );
            let new_len = new_raw_bytes.len() as i64;

            let new_file_hash: [u8; 32] = atomic_write_md(&abs_path, &new_bytes, &self_writes)?;

            let tx = conn.transaction().map_err(ApiError::from)?;

            // Shift blocks that were after the insertion point.
            tx.execute(
                "UPDATE blocks SET byte_offset = byte_offset + ? \
                 WHERE page_id = ? AND byte_offset >= ?",
                params![new_len, req.page_id, insert_byte_offset],
            )?;

            // Shift sibling ords >= req.ord.
            tx.execute(
                "UPDATE blocks SET ord = ord + 1 \
                 WHERE page_id = ? AND parent_id IS ? AND ord >= ?",
                params![req.page_id, req.parent_id, req.ord],
            )?;

            let block_hash = blake3::hash(req.raw.as_bytes());
            tx.execute(
                "INSERT INTO blocks \
                    (page_id, parent_id, ord, depth, raw, byte_offset, byte_length, hash) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    req.page_id,
                    req.parent_id,
                    req.ord,
                    req.depth,
                    &req.raw,
                    insert_byte_offset,
                    new_len,
                    block_hash.as_bytes().as_slice(),
                ],
            )?;
            let new_block_id = tx.last_insert_rowid();

            tx.execute(
                "UPDATE files SET hash = ? WHERE id = ?",
                params![new_file_hash.as_slice(), file_id],
            )?;

            insert_refs_for_block_tx(&tx, new_block_id, &req.raw)?;
            tx.commit()?;

            let block_subtree = build_page_block_tree(guard.conn(), req.page_id)
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            Ok(CreateBlockResponse {
                id: new_block_id,
                block_subtree,
                file_hash: hex::encode(new_file_hash),
            })
        })
        .await
        .map_err(|e| ApiError::Internal(format!("join error: {e}")))?;

    result.map(|r| (StatusCode::CREATED, Json(r)))
}

// ─── PATCH /api/blocks/:id/structure ─────────────────────────────────────────

/// Structural op: indent / outdent / move. Body: `{ op, prevHash, ... }`.
pub async fn patch_block_structure(
    State(app): State<AppState>,
    Path(block_id): Path<i64>,
    Json(req): Json<PatchBlockStructureRequest>,
) -> Result<Json<MutationResponse>, ApiError> {
    let db = app.db.clone();
    let root = app.root.clone();
    let self_writes = app.self_writes.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<MutationResponse, ApiError> {
        let mut guard = db.lock().map_err(|_| ApiError::Internal("db poisoned".into()))?;
        let conn = guard.conn_mut();

        let (file_id, hash_bytes, abs_path, byte_offset, byte_length, page_id) =
            fetch_block_file_info(conn, &root, block_id)?;
        verify_prev_hash(&hash_bytes, &req.prev_hash)?;

        let (current_depth, current_parent_id, current_ord): (i32, Option<i64>, i32) = conn
            .query_row(
                "SELECT depth, parent_id, ord FROM blocks WHERE id = ?1",
                params![block_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| ApiError::NotFound)?;

        let original = std::fs::read(&abs_path)?;
        let current_raw = String::from_utf8_lossy(
            &original[byte_offset as usize..(byte_offset + byte_length) as usize],
        )
        .to_string();

        match req.op.as_str() {
            "indent" => {
                let new_parent_id: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM blocks \
                         WHERE page_id = ? AND parent_id IS ? AND ord < ? \
                         ORDER BY ord DESC LIMIT 1",
                        params![page_id, current_parent_id, current_ord],
                        |r| r.get(0),
                    )
                    .ok();
                if new_parent_id.is_none() {
                    return Err(ApiError::BadRequest(
                        "no preceding sibling to indent into".to_string(),
                    ));
                }

                let new_raw = format!("\t{current_raw}");
                let new_raw_bytes = new_raw.as_bytes();
                let new_bytes = splice_block(
                    &original,
                    byte_offset as usize,
                    byte_length as usize,
                    new_raw_bytes,
                );
                let new_len = new_raw_bytes.len() as i64;
                let delta = new_len - byte_length;

                let new_file_hash: [u8; 32] =
                    atomic_write_md(&abs_path, &new_bytes, &self_writes)?;

                let tx = conn.transaction().map_err(ApiError::from)?;

                let new_ord: i32 = tx
                    .query_row(
                        "SELECT COALESCE(MAX(ord), -1) + 1 FROM blocks WHERE parent_id = ?",
                        params![new_parent_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);

                tx.execute(
                    "UPDATE blocks SET ord = ord - 1 \
                     WHERE page_id = ? AND parent_id IS ? AND ord > ?",
                    params![page_id, current_parent_id, current_ord],
                )?;

                tx.execute(
                    "UPDATE blocks SET raw = ?, byte_length = ?, depth = ?, \
                     parent_id = ?, ord = ? WHERE id = ?",
                    params![&new_raw, new_len, current_depth + 1, new_parent_id, new_ord, block_id],
                )?;

                if delta != 0 {
                    tx.execute(
                        "UPDATE blocks SET byte_offset = byte_offset + ? \
                         WHERE page_id = ? AND byte_offset > ?",
                        params![delta, page_id, byte_offset],
                    )?;
                }

                tx.execute(
                    "UPDATE files SET hash = ? WHERE id = ?",
                    params![new_file_hash.as_slice(), file_id],
                )?;

                tx.execute("DELETE FROM refs WHERE source_block = ?", params![block_id])?;
                insert_refs_for_block_tx(&tx, block_id, &new_raw)?;
                tx.commit()?;

                let block_subtree = build_page_block_tree(guard.conn(), page_id)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                Ok(MutationResponse {
                    block_subtree,
                    file_hash: hex::encode(new_file_hash),
                    dirty_block_ids: vec![block_id],
                })
            }

            "outdent" => {
                if current_depth == 0 || current_parent_id.is_none() {
                    return Err(ApiError::BadRequest(
                        "block is already at root depth".to_string(),
                    ));
                }

                let new_raw = if current_raw.starts_with('\t') {
                    current_raw[1..].to_string()
                } else {
                    current_raw.clone()
                };
                let new_raw_bytes = new_raw.as_bytes();
                let new_bytes = splice_block(
                    &original,
                    byte_offset as usize,
                    byte_length as usize,
                    new_raw_bytes,
                );
                let new_len = new_raw_bytes.len() as i64;
                let delta = new_len - byte_length;

                let new_file_hash: [u8; 32] =
                    atomic_write_md(&abs_path, &new_bytes, &self_writes)?;

                let (grandparent_id, parent_ord): (Option<i64>, i32) = conn
                    .query_row(
                        "SELECT parent_id, ord FROM blocks WHERE id = ?",
                        params![current_parent_id.unwrap()],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .map_err(|_| ApiError::NotFound)?;
                let new_ord = parent_ord + 1;

                let tx = conn.transaction().map_err(ApiError::from)?;

                tx.execute(
                    "UPDATE blocks SET ord = ord + 1 \
                     WHERE page_id = ? AND parent_id IS ? AND ord >= ?",
                    params![page_id, grandparent_id, new_ord],
                )?;
                tx.execute(
                    "UPDATE blocks SET ord = ord - 1 \
                     WHERE page_id = ? AND parent_id IS ? AND ord > ?",
                    params![page_id, current_parent_id, current_ord],
                )?;

                tx.execute(
                    "UPDATE blocks SET raw = ?, byte_length = ?, depth = ?, \
                     parent_id = ?, ord = ? WHERE id = ?",
                    params![&new_raw, new_len, current_depth - 1, grandparent_id, new_ord, block_id],
                )?;

                if delta != 0 {
                    tx.execute(
                        "UPDATE blocks SET byte_offset = byte_offset + ? \
                         WHERE page_id = ? AND byte_offset > ?",
                        params![delta, page_id, byte_offset],
                    )?;
                }

                tx.execute(
                    "UPDATE files SET hash = ? WHERE id = ?",
                    params![new_file_hash.as_slice(), file_id],
                )?;

                tx.execute("DELETE FROM refs WHERE source_block = ?", params![block_id])?;
                insert_refs_for_block_tx(&tx, block_id, &new_raw)?;
                tx.commit()?;

                let block_subtree = build_page_block_tree(guard.conn(), page_id)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                Ok(MutationResponse {
                    block_subtree,
                    file_hash: hex::encode(new_file_hash),
                    dirty_block_ids: vec![block_id],
                })
            }

            "move" => {
                let new_parent_id = req.new_parent_id;
                let new_ord = req.new_ord.ok_or_else(|| {
                    ApiError::BadRequest("newOrd required for move".to_string())
                })?;

                // Pure tree move: file bytes unchanged. We still call
                // atomic_write_md with the same bytes so the file's mtime and
                // the hash in SelfWriteSet stay consistent.
                let new_file_hash: [u8; 32] =
                    atomic_write_md(&abs_path, &original, &self_writes)?;

                let tx = conn.transaction().map_err(ApiError::from)?;

                tx.execute(
                    "UPDATE blocks SET ord = ord - 1 \
                     WHERE page_id = ? AND parent_id IS ? AND ord > ?",
                    params![page_id, current_parent_id, current_ord],
                )?;
                tx.execute(
                    "UPDATE blocks SET ord = ord + 1 \
                     WHERE page_id = ? AND parent_id IS ? AND ord >= ?",
                    params![page_id, new_parent_id, new_ord],
                )?;

                let new_depth: i32 = match new_parent_id {
                    Some(pid) => tx
                        .query_row(
                            "SELECT depth FROM blocks WHERE id = ?",
                            params![pid],
                            |r| r.get::<_, i32>(0),
                        )
                        .map(|d| d + 1)
                        .unwrap_or(0),
                    None => 0,
                };

                tx.execute(
                    "UPDATE blocks SET parent_id = ?, ord = ?, depth = ? WHERE id = ?",
                    params![new_parent_id, new_ord, new_depth, block_id],
                )?;

                tx.execute(
                    "UPDATE files SET hash = ? WHERE id = ?",
                    params![new_file_hash.as_slice(), file_id],
                )?;

                tx.commit()?;

                let block_subtree = build_page_block_tree(guard.conn(), page_id)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                Ok(MutationResponse {
                    block_subtree,
                    file_hash: hex::encode(new_file_hash),
                    dirty_block_ids: vec![block_id],
                })
            }

            other => Err(ApiError::BadRequest(format!("unknown op: {other}"))),
        }
    })
    .await
    .map_err(|e| ApiError::Internal(format!("join error: {e}")))?;

    result.map(Json)
}

// ─── DELETE /api/blocks/:id ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteQuery {
    #[serde(rename = "prevHash")]
    pub prev_hash: String,
}

/// Delete a block. Query param: `?prevHash=<hex>`.
pub async fn delete_block(
    State(app): State<AppState>,
    Path(block_id): Path<i64>,
    Query(q): Query<DeleteQuery>,
) -> Result<StatusCode, ApiError> {
    let db = app.db.clone();
    let root = app.root.clone();
    let self_writes = app.self_writes.clone();

    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        let mut guard = db.lock().map_err(|_| ApiError::Internal("db poisoned".into()))?;
        let conn = guard.conn_mut();

        let (file_id, hash_bytes, abs_path, byte_offset, byte_length, page_id) =
            fetch_block_file_info(conn, &root, block_id)?;
        verify_prev_hash(&hash_bytes, &q.prev_hash)?;

        let (parent_id, block_ord): (Option<i64>, i32) = conn
            .query_row(
                "SELECT parent_id, ord FROM blocks WHERE id = ?",
                params![block_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| ApiError::NotFound)?;

        let child_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM blocks WHERE parent_id = ? ORDER BY ord",
            )?;
            stmt.query_map(params![block_id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let n_children = child_ids.len() as i32;

        let original = std::fs::read(&abs_path)?;
        let new_bytes =
            splice_block(&original, byte_offset as usize, byte_length as usize, b"");

        let new_file_hash: [u8; 32] = atomic_write_md(&abs_path, &new_bytes, &self_writes)?;

        let tx = conn.transaction().map_err(ApiError::from)?;

        for (i, &cid) in child_ids.iter().enumerate() {
            tx.execute(
                "UPDATE blocks SET parent_id = ?, ord = ? WHERE id = ?",
                params![parent_id, block_ord + i as i32, cid],
            )?;
        }

        let shift = n_children - 1;
        tx.execute(
            "UPDATE blocks SET ord = ord + ? \
             WHERE page_id = ? AND parent_id IS ? AND ord > ? AND id != ?",
            params![shift, page_id, parent_id, block_ord, block_id],
        )?;

        let neg_len = -(byte_length as i64);
        tx.execute(
            "UPDATE blocks SET byte_offset = byte_offset + ? \
             WHERE page_id = ? AND byte_offset > ?",
            params![neg_len, file_id, byte_offset],
        )?;

        tx.execute("DELETE FROM blocks WHERE id = ?", params![block_id])?;

        tx.execute(
            "UPDATE files SET hash = ? WHERE id = ?",
            params![new_file_hash.as_slice(), file_id],
        )?;

        tx.commit()?;

        let _ = page_id;
        Ok(())
    })
    .await
    .map_err(|e| ApiError::Internal(format!("join error: {e}")))?
    .map(|_| StatusCode::NO_CONTENT)
}
