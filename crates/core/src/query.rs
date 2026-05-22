//! Phase 1 / Plan 01-07 — read-only queries powering the CLI's
//! `search` and `dump-tree` subcommands.
//!
//! The structs derived `Serialize` here are part of the cross-phase
//! JSON contract (D-02). New fields are additive only.

use rusqlite::params;

use crate::indexer::IndexerError;
use crate::storage::Db;

/// One hit from [`search_blocks`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// Forward-slash relative path of the page file containing the hit.
    pub page_path: String,
    /// Rowid of the matching block (also the key into `blocks_fts`).
    pub block_id: i64,
    /// FTS5-generated snippet with `[`…`]` marking the match. Falls back
    /// to the leading `\u{2026}`-truncated raw text when FTS5 cannot
    /// produce one (e.g. the matched column is empty).
    pub snippet: String,
}

/// One node of the tree returned by [`dump_page_tree`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockNode {
    /// TAB-indent depth as stored in `blocks.depth`. `-1` is the page prelude.
    pub depth: i32,
    /// Raw block text — exactly what the segmenter produced.
    pub raw: String,
    /// Children, recursive. Ordered by `blocks.ord` ascending.
    pub children: Vec<BlockNode>,
}

/// FTS5 search across `blocks_fts`, joined back to `pages` → `files`
/// to surface human-readable paths.
///
/// The empty-query early-return is the T-07-04 mitigation (FTS5 `MATCH ''`
/// errors out, which would surface as `Sqlite` to the CLI — not what users
/// expect from "no input").
pub fn search_blocks(
    db: &Db,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, IndexerError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // `limit == 0` is "give me everything"; cap at a large signed value so
    // the SQL bind doesn't fail.
    let sql_limit: i64 = if limit == 0 {
        i64::MAX
    } else {
        i64::try_from(limit).unwrap_or(i64::MAX)
    };

    let conn = db.conn();
    let mut stmt = conn.prepare(
        r#"
        SELECT
            f.path                                                       AS page_path,
            b.id                                                         AS block_id,
            snippet(blocks_fts, 0, '[', ']', '\u{2026}', 12)             AS snippet
        FROM blocks_fts
        JOIN blocks b ON b.id = blocks_fts.rowid
        JOIN pages  p ON p.id = b.page_id
        JOIN files  f ON f.id = p.file_id
        WHERE blocks_fts MATCH ?1
        ORDER BY b.id ASC
        LIMIT ?2
        "#,
    )?;

    let rows = stmt
        .query_map(params![trimmed, sql_limit], |row| {
            Ok(SearchHit {
                page_path: row.get::<_, String>(0)?,
                block_id: row.get::<_, i64>(1)?,
                snippet: row.get::<_, String>(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Build the top-level block tree for `page_name` (lookup is NOCASE per D-03).
/// Empty result means the page is unknown OR has no blocks.
pub fn dump_page_tree(db: &Db, page_name: &str) -> Result<Vec<BlockNode>, IndexerError> {
    let conn = db.conn();

    // Resolve the page row. NOCASE collation lives on `pages_name_idx`.
    let page_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM pages WHERE name = ?1 COLLATE NOCASE LIMIT 1",
            params![page_name],
            |row| row.get::<_, i64>(0),
        )
        .ok();

    let Some(page_id) = page_id else {
        return Ok(Vec::new());
    };

    // Pull all blocks in deterministic source order.
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, depth, raw FROM blocks WHERE page_id = ?1 ORDER BY ord ASC",
    )?;
    let rows: Vec<(i64, Option<i64>, i32, String)> = stmt
        .query_map(params![page_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Build a parent_id → ordered children index. Source order is preserved
    // because `rows` is already ordered by `ord` (sibling order within parent).
    use std::collections::HashMap;
    let mut children_of: HashMap<Option<i64>, Vec<(i64, i32, String)>> = HashMap::new();
    for (id, parent_id, depth, raw) in rows {
        children_of
            .entry(parent_id)
            .or_default()
            .push((id, depth, raw));
    }

    fn build(
        parent_id: Option<i64>,
        children_of: &HashMap<Option<i64>, Vec<(i64, i32, String)>>,
    ) -> Vec<BlockNode> {
        let Some(direct) = children_of.get(&parent_id) else {
            return Vec::new();
        };
        direct
            .iter()
            .map(|(id, depth, raw)| BlockNode {
                depth: *depth,
                raw: raw.clone(),
                children: build(Some(*id), children_of),
            })
            .collect()
    }

    Ok(build(None, &children_of))
}
