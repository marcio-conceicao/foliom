//! Wire-level DTOs for the Phase 2/3 REST API.
//!
//! Schema source of truth: 02-RESEARCH §REST API Schema and 03-RESEARCH §7.
//! All types serialize with `#[serde(rename_all = "camelCase")]` so the
//! frontend can consume them as TypeScript interfaces with natural field names.
//! Phase 3 adds mutation DTOs (plan 03-03) alongside the existing read-only
//! surface.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSummary {
    pub name: String,
    pub is_journal: bool,
    /// `false` when a page is referenced by `[[...]]` or `#...` but has no
    /// backing file on disk (D-04 unresolved page).
    pub is_resolved: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDetail {
    pub name: String,
    pub is_journal: bool,
    /// `Some("March 15th, 2024")` for journals, `None` for ordinary pages.
    pub formatted_title: Option<String>,
    /// Root-level blocks. Per the segmenter contract, a `depth = -1` page
    /// prelude block is always first; its `children` are the page's
    /// top-level (`depth = 0`) bullets.
    pub blocks: Vec<Block>,
    /// BLAKE3 hash of the backing `.md` file, hex-encoded. Used by mutation
    /// handlers as `prev_hash` for conflict detection (plan 03-03).
    /// `None` for unresolved pages (no backing file).
    pub file_hash: Option<String>,
    /// SQL `pages.id`. Exposed so the POST /api/blocks frontend payload can
    /// include `pageId` without an extra round-trip.
    pub id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub id: i64,
    pub depth: i32,
    /// Verbatim block text from `blocks.raw` (includes the segmenter prefix
    /// `\t...\t- `). The frontend strips that prefix for rendering.
    pub raw: String,
    /// `[[key, value], ...]` from `block_props`. Tuple shape keeps JSON
    /// stable and avoids accidental key-ordering changes.
    pub properties: Vec<[String; 2]>,
    pub drawers: Vec<DrawerRef>,
    pub children: Vec<Block>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawerRef {
    pub name: String,
    pub byte_offset: i64,
    pub byte_length: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backlink {
    pub page: String,
    pub block_id: i64,
    /// First 200 chars of `blocks.raw`. Frontend strips the segmenter prefix.
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
pub struct JournalRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    /// ISO 8601 `YYYY-MM-DD`.
    pub date: String,
    /// Page name as stored on disk: `YYYY_MM_DD`.
    pub name: String,
    /// Long-form English title, e.g. `"March 15th, 2024"`.
    pub formatted_title: String,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    #[default]
    Content,
    Tag,
    Page,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub kind: SearchKind,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub page: String,
    pub block_id: i64,
    pub snippet: String,
}

// ─── Plan 03-03 mutation DTOs ────────────────────────────────────────────────

/// Response body for successful mutation endpoints (PUT/POST/PATCH/DELETE).
///
/// `block_subtree` mirrors the shape of `PageDetail.blocks` — it is the full
/// updated tree for the affected page, built by `assemble_tree`. Plan 03-04
/// (frontend) will diff this against the cached tree to update only changed
/// blocks.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResponse {
    /// Full updated block tree for the page (same shape as `PageDetail.blocks`).
    pub block_subtree: Vec<Block>,
    /// New BLAKE3 file hash after the write, hex-encoded. The client must
    /// pass this as `prevHash` in subsequent mutations.
    pub file_hash: String,
    /// SQL ids of blocks whose `raw` / offsets changed in this mutation.
    pub dirty_block_ids: Vec<i64>,
}

/// Body for `PUT /api/blocks/:id` (edit block text).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PutBlockRequest {
    /// New block raw text (full block including segmenter prefix, ending `\n`).
    pub raw: String,
    /// Hex-encoded BLAKE3 hash of the current file contents the client last
    /// read. If this doesn't match `files.hash`, the server returns 409.
    pub prev_hash: String,
}

/// Body for `POST /api/blocks` (create new block).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockRequest {
    pub page_id: i64,
    pub parent_id: Option<i64>,
    pub ord: i32,
    pub depth: i32,
    pub raw: String,
    pub prev_hash: String,
}

/// Body for `PATCH /api/blocks/:id/structure` (indent/outdent/move).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBlockStructureRequest {
    /// `"indent"` | `"outdent"` | `"move"`.
    pub op: String,
    pub prev_hash: String,
    /// For `op = "move"`: target parent id (null = top-level).
    pub new_parent_id: Option<i64>,
    /// For `op = "move"`: target sibling ordinal.
    pub new_ord: Option<i32>,
}

/// Response body for `POST /api/blocks` (includes new block id).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockResponse {
    pub id: i64,
    pub block_subtree: Vec<Block>,
    pub file_hash: String,
}

/// Error response body returned by mutation endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_file_hash: Option<String>,
}

// ─── Plan 04-01 watcher DTOs ─────────────────────────────────────────────────

/// Single page entry in a `WatcherEvent::PagesUpdated` payload.
///
/// `file_hash` is the new BLAKE3 hex hash after the external edit — the
/// frontend uses it to detect whether the page being viewed was changed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageUpdatedInfo {
    /// Page name as stored in `pages.name` (no `.md` extension).
    pub name: String,
    /// New BLAKE3 hash of the file contents, hex-encoded.
    pub file_hash: String,
}

/// Events broadcast from the watcher to all SSE clients via
/// `GET /api/watch/events`. Capacity-64 `tokio::sync::broadcast` channel;
/// `Lagged` mapped to `IndexReset` by the SSE handler (D-40-02, T-04-03).
#[derive(Debug, Clone)]
pub enum WatcherEvent {
    /// One or more `.md` files changed externally. Contains the union of all
    /// dirty pages from the current coalescing window (D-40-03).
    PagesUpdated(Vec<PageUpdatedInfo>),
    /// A `.md` file was deleted from disk (no longer in corpus).
    PageDeleted(String),
    /// A full rescan was triggered (macOS `MustScanSubDirs`, Linux
    /// `IN_Q_OVERFLOW`, Windows `ReadDirectoryChangesW` error — Q4/Q5).
    /// Frontend must refetch the current page unconditionally.
    IndexReset,
}
