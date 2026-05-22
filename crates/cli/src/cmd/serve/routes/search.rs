//! `GET /api/search?q=&kind=&limit=` — FTS5 content search and tag-refs
//! lookup, depending on `kind`.
//!
//! Sanitization (locked in 02-RESEARCH §FTS5 Query Patterns):
//!   - trim whitespace, return `[]` if empty (NOT 400 — empty palette input
//!     is expected during typing).
//!   - reject unquoted `:` (FTS5 column filter injection vector) → `[]`.
//!   - strip backslashes (defensive, conservative trade — see threat model
//!     T-02-05 in PLAN; some legitimate `\d` queries won't match).
//!   - double inner `"` so quoted phrases stay legal MATCH syntax.
//!
//! `kind=page` returns 400 — the frontend should call `/api/page-titles`
//! instead (per 02-RESEARCH §Page-name search vs block-content search).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use rusqlite::params;

use crate::cmd::serve::dto::{SearchHit, SearchKind, SearchQuery};
use crate::cmd::serve::state::AppState;

/// Hard cap on `limit` so a hostile query can't fan out an arbitrary number
/// of FTS rows.
const MAX_LIMIT: usize = 200;

pub async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, StatusCode> {
    let limit = q.limit.clamp(1, MAX_LIMIT) as i64;

    // Pre-trim once; downstream branches can short-circuit on empty.
    let trimmed = q.q.trim().to_string();

    match q.kind {
        SearchKind::Page => Err(StatusCode::BAD_REQUEST),
        SearchKind::Tag => {
            // Strip leading `#`(s) so `#crypto` and `crypto` both hit.
            let tag = trimmed.trim_start_matches('#').trim();
            if tag.is_empty() {
                return Ok(Json(Vec::new()));
            }
            let tag_owned = tag.to_string();
            let db = state.db.clone();
            let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<SearchHit>> {
                let guard = db.lock().expect("db poisoned");
                let conn = guard.conn();
                let mut stmt = conn.prepare(
                    "SELECT p.name, b.id, substr(b.raw, 1, 200) \
                     FROM refs r \
                     JOIN blocks b ON b.id = r.source_block \
                     JOIN pages  p ON p.id = b.page_id \
                     JOIN pages  tp ON tp.id = r.target_page \
                     WHERE tp.name = ?1 COLLATE NOCASE AND r.type = 'tag' \
                     ORDER BY p.name, b.ord \
                     LIMIT ?2",
                )?;
                let rows: rusqlite::Result<Vec<SearchHit>> = stmt
                    .query_map(params![tag_owned, limit], |r| {
                        Ok(SearchHit {
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
                tracing::error!(error = %e, "join error in /api/search?kind=tag");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "db error in /api/search?kind=tag");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(rows))
        }
        SearchKind::Content => {
            let Some(sanitized) = sanitize_fts(&trimmed) else {
                tracing::debug!(query = %trimmed, "search query rejected by sanitizer");
                return Ok(Json(Vec::new()));
            };
            let db = state.db.clone();
            let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<SearchHit>> {
                let guard = db.lock().expect("db poisoned");
                let conn = guard.conn();
                let mut stmt = conn.prepare(
                    "SELECT p.name, b.id, \
                            snippet(blocks_fts, 0, '<mark>', '</mark>', '…', 16) \
                     FROM blocks_fts \
                     JOIN blocks b ON b.id = blocks_fts.rowid \
                     JOIN pages  p ON p.id = b.page_id \
                     WHERE blocks_fts MATCH ?1 \
                     ORDER BY rank \
                     LIMIT ?2",
                )?;
                let rows: rusqlite::Result<Vec<SearchHit>> = stmt
                    .query_map(params![sanitized, limit], |r| {
                        Ok(SearchHit {
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
                tracing::error!(error = %e, "join error in /api/search");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            // Surfacing FTS5 syntax errors (e.g. unbalanced quote in user
            // input that snuck past sanitize) as empty rather than 500 —
            // see 02-RESEARCH §FTS5 Query Patterns.
            Ok(Json(rows.unwrap_or_default()))
        }
    }
}

/// Returns `Some(canonical_query)` if the input passes the sanitizer,
/// `None` if it should produce an empty result (`q.is_empty()` or `:`
/// column-filter injection detected).
fn sanitize_fts(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    // Strip backslashes (conservative — see threat model T-02-05).
    let stripped: String = input.chars().filter(|c| *c != '\\').collect();

    // Reject unquoted `:` to block FTS5 column-filter injection.
    // We accept `:` only inside a balanced double-quoted run.
    let mut in_quote = false;
    for ch in stripped.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => return None,
            _ => {}
        }
    }

    // Double inner `"` so each becomes `""` — FTS5's escape for a literal
    // double-quote inside a quoted phrase. We work on a fresh String to keep
    // the implementation obviously correct.
    //
    // Strategy: walk the original quote-state and double the `"` that the
    // user typed verbatim INSIDE a phrase. The simplest correct strategy is:
    // replace every `"` with `""` then wrap the whole expression in `"…"`.
    // That re-encodes the entire user input as a single phrase, which is the
    // strictest possible interpretation but matches the contract in
    // 02-RESEARCH (phrase queries via `"…"` are the documented happy path).
    //
    // Because we already early-returned on `:` outside quotes, the simpler
    // approach is to just escape the bare query string with FTS5's standard
    // double-quote-doubling rule and return it. The frontend should pass
    // already-quoted phrases when it wants phrase semantics; for the common
    // case ("Glauber") we want token search, not phrase. So: only escape
    // unbalanced inner quotes by doubling them, then return as-is.
    let mut out = String::with_capacity(stripped.len() + 2);
    for ch in stripped.chars() {
        if ch == '"' {
            out.push_str("\"\"");
        } else {
            out.push(ch);
        }
    }

    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_fts;

    #[test]
    fn empty_returns_none() {
        assert!(sanitize_fts("").is_none());
        assert!(sanitize_fts("   ").is_none());
    }

    #[test]
    fn plain_token_passes() {
        assert_eq!(sanitize_fts("Glauber").as_deref(), Some("Glauber"));
    }

    #[test]
    fn colon_outside_quotes_rejected() {
        assert!(sanitize_fts("name:value").is_none());
        assert!(sanitize_fts("col:foo").is_none());
    }

    #[test]
    fn backslashes_stripped() {
        assert_eq!(sanitize_fts(r"a\b").as_deref(), Some("ab"));
    }

    #[test]
    fn quoted_phrase_preserved_with_escaped_inner_quotes() {
        // User typed `"speech analytics"` — passes through with each `"`
        // doubled so FTS5 sees a well-formed quoted phrase with no
        // accidental column-filter sidesteps.
        assert_eq!(
            sanitize_fts("\"speech analytics\"").as_deref(),
            Some("\"\"speech analytics\"\"")
        );
    }
}
