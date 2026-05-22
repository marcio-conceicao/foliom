//! `/api/journals*` handlers.
//!
//! - `GET /api/journals/today` — 302 redirect into the SPA hash route for
//!   today's `YYYY_MM_DD` journal. The backend computes today in the user's
//!   local timezone (D-25 server time = user time for a local app) and falls
//!   back to UTC if the offset cannot be determined.
//! - `GET /api/journals?from=YYYY-MM-DD&to=YYYY-MM-DD` — journal entries
//!   whose name is BETWEEN the bounds, with long-form titles.

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use rusqlite::params;
use time::OffsetDateTime;

use crate::cmd::serve::dto::{JournalEntry, JournalRange};
use crate::cmd::serve::format::{
    format_iso_date, format_journal_title, iso_to_filename, parse_journal_name,
};
use crate::cmd::serve::state::AppState;

/// `GET /api/journals/today` — manually-built 302 (axum's `Redirect::to`
/// emits 303 See Other by default; the wire contract calls for 302 Found).
pub async fn today() -> Response {
    let now = match OffsetDateTime::now_local() {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(error = %err, "now_local failed; falling back to UTC for /api/journals/today");
            OffsetDateTime::now_utc()
        }
    };
    let date = now.date();
    let name = format!(
        "{:04}_{:02}_{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    );
    let location = format!("/#/journals/{name}");

    let mut resp = (StatusCode::FOUND, "").into_response();
    if let Ok(val) = HeaderValue::from_str(&location) {
        resp.headers_mut().insert(header::LOCATION, val);
    }
    resp
}

/// `GET /api/journals?from=&to=` — range query over journal pages.
pub async fn range(
    State(state): State<AppState>,
    Query(q): Query<JournalRange>,
) -> Result<Json<Vec<JournalEntry>>, StatusCode> {
    // Convert ISO bounds to `YYYY_MM_DD` filename form so the BETWEEN
    // compares lexicographically (the same way the underscored stem sorts).
    let from_name = iso_to_filename(&q.from).ok_or(StatusCode::BAD_REQUEST)?;
    let to_name = iso_to_filename(&q.to).ok_or(StatusCode::BAD_REQUEST)?;

    let db = state.db.clone();
    let names = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<String>> {
        let guard = db.lock().expect("db poisoned");
        let conn = guard.conn();
        let mut stmt = conn.prepare(
            "SELECT name FROM pages \
             WHERE kind = 'journal' AND name BETWEEN ?1 AND ?2 \
             ORDER BY name",
        )?;
        let rows: rusqlite::Result<Vec<String>> = stmt
            .query_map(params![&from_name, &to_name], |r| r.get::<_, String>(0))?
            .collect();
        rows
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "join error in /api/journals");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        tracing::error!(error = %e, "db error in /api/journals");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let entries: Vec<JournalEntry> = names
        .into_iter()
        .filter_map(|name| {
            let date = parse_journal_name(&name)?;
            Some(JournalEntry {
                date: format_iso_date(date),
                name,
                formatted_title: format_journal_title(date),
            })
        })
        .collect();

    Ok(Json(entries))
}
