//! Wire-level response DTOs for the Phase 2 read-only REST API.
//!
//! Schema source of truth: 02-RESEARCH §REST API Schema. All types serialize
//! with `#[serde(rename_all = "camelCase")]` so the frontend can consume them
//! as TypeScript interfaces with natural field names. Phase 3 will add the
//! mutation counterparts (PUT/PATCH); Phase 2 keeps the wire surface
//! read-only.

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
