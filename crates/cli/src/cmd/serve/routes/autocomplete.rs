//! `GET /api/autocomplete` — prefix completion for `[[page]]` and `#tag` triggers.
//!
//! # Query parameters
//! - `prefix` (required): the text typed after `[[` or `#`. Empty string OK.
//! - `kind`   (required): `page` | `tag` | `all`.
//! - `limit`  (optional): max results, default 20, clamped to 100.
//!
//! # Response shapes
//! - `kind=page`: `string[]` of page names.
//! - `kind=tag`:  `string[]` of distinct `target_page` from `refs WHERE kind='tag'`.
//! - `kind=all`:  `[{ name: string, kind: "tag" | "page" }]` — union with dedup
//!                (tags first on collision per D-30-06).
//!
//! # Security
//! - T-03-15: `params![]` binding + LIKE wildcard escape (`%` → `\%`, `_` → `\_`)
//!   with `ESCAPE '\'` clause.
//! - T-03-16: limit clamped to 100 server-side.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use crate::cmd::serve::state::AppState;

/// Raw query params from the request URL.
#[derive(Debug, Deserialize)]
pub struct AutocompleteParams {
    prefix: Option<String>,
    kind: Option<String>,
    limit: Option<u32>,
}

/// A labelled item returned for `kind=all`.
#[derive(Debug, Serialize)]
pub struct LabelledItem {
    pub name: String,
    pub kind: &'static str, // "tag" | "page"
}

/// Response body variant.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AutocompleteResponse {
    Strings(Vec<String>),
    Labelled(Vec<LabelledItem>),
}

/// Escape LIKE wildcards so user-supplied prefix is treated as a literal prefix.
/// `%` → `\%`, `_` → `\_`. The LIKE clause must use `ESCAPE '\'`.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub async fn get(
    State(state): State<AppState>,
    Query(params): Query<AutocompleteParams>,
) -> Result<Json<AutocompleteResponse>, StatusCode> {
    // T-03-15/T-03-16: require prefix, validate kind, clamp limit.
    let prefix = match &params.prefix {
        Some(p) => p.clone(),
        None => return Err(StatusCode::BAD_REQUEST),
    };
    let kind = params.kind.as_deref().unwrap_or("page");
    let limit = params.limit.unwrap_or(20).min(100) as i64;

    let escaped = escape_like(&prefix);
    let pattern = format!("{escaped}%");

    let db = state.db.clone();

    match kind {
        "page" => {
            let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<String>> {
                let guard = db.lock().expect("db poisoned");
                let conn = guard.conn();
                let mut stmt = conn.prepare(
                    "SELECT name FROM pages \
                     WHERE LOWER(name) LIKE LOWER(?1) ESCAPE '\\' \
                     ORDER BY name COLLATE NOCASE \
                     LIMIT ?2",
                )?;
                stmt.query_map(rusqlite::params![pattern, limit], |r| r.get::<_, String>(0))?
                    .collect()
            })
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "join error in /api/autocomplete?kind=page");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "db error in /api/autocomplete?kind=page");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(AutocompleteResponse::Strings(rows)))
        }

        "tag" => {
            // refs.type = 'tag' (not 'kind'); refs.target_page is a page ID.
            // Join with pages to get the name string.
            let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<String>> {
                let guard = db.lock().expect("db poisoned");
                let conn = guard.conn();
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT p.name FROM refs r \
                     JOIN pages p ON p.id = r.target_page \
                     WHERE r.type='tag' AND LOWER(p.name) LIKE LOWER(?1) ESCAPE '\\' \
                     ORDER BY p.name COLLATE NOCASE \
                     LIMIT ?2",
                )?;
                stmt.query_map(rusqlite::params![pattern, limit], |r| r.get::<_, String>(0))?
                    .collect()
            })
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "join error in /api/autocomplete?kind=tag");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "db error in /api/autocomplete?kind=tag");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(AutocompleteResponse::Strings(rows)))
        }

        "all" => {
            // Union of tags + pages. Tags appear first; dedup by name.
            let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<LabelledItem>> {
                let guard = db.lock().expect("db poisoned");
                let conn = guard.conn();

                // Fetch tags first (they win on collision per D-30-06).
                let mut items: Vec<LabelledItem> = Vec::new();
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

                {
                    // refs.type = 'tag'; join to get page name string.
                    let mut stmt = conn.prepare(
                        "SELECT DISTINCT p.name FROM refs r \
                         JOIN pages p ON p.id = r.target_page \
                         WHERE r.type='tag' AND LOWER(p.name) LIKE LOWER(?1) ESCAPE '\\' \
                         ORDER BY p.name COLLATE NOCASE \
                         LIMIT ?2",
                    )?;
                    for row in
                        stmt.query_map(rusqlite::params![pattern.clone(), limit], |r| {
                            r.get::<_, String>(0)
                        })?
                    {
                        let name = row?;
                        if seen.insert(name.clone()) {
                            items.push(LabelledItem { name, kind: "tag" });
                        }
                    }
                }

                {
                    let mut stmt = conn.prepare(
                        "SELECT name FROM pages \
                         WHERE LOWER(name) LIKE LOWER(?1) ESCAPE '\\' \
                         ORDER BY name COLLATE NOCASE \
                         LIMIT ?2",
                    )?;
                    for row in
                        stmt.query_map(rusqlite::params![pattern, limit], |r| r.get::<_, String>(0))?
                    {
                        let name = row?;
                        if seen.insert(name.clone()) {
                            items.push(LabelledItem { name, kind: "page" });
                        }
                    }
                }

                // Trim to limit (the two queries individually respect limit,
                // but the combined might exceed it if both are near limit).
                items.truncate(limit as usize);
                Ok(items)
            })
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "join error in /api/autocomplete?kind=all");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map_err(|e| {
                tracing::error!(error = %e, "db error in /api/autocomplete?kind=all");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(AutocompleteResponse::Labelled(rows)))
        }

        _ => Err(StatusCode::BAD_REQUEST),
    }
}
