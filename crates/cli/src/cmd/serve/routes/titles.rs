//! `GET /api/page-titles` — flat `String[]` of every page name.
//!
//! Phase 2 uses this only for the search palette's page-name fuzzy matcher
//! (D-26 / 02-RESEARCH §Page-name search). Phase 3 reuses the same shape for
//! `[[` autocomplete inside the editor.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;

use crate::cmd::serve::state::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let db = state.db.clone();
    let rows = tokio::task::spawn_blocking(move || -> rusqlite::Result<Vec<String>> {
        let guard = db.lock().expect("db poisoned");
        let conn = guard.conn();
        let mut stmt =
            conn.prepare("SELECT name FROM pages ORDER BY name COLLATE NOCASE")?;
        let rows: rusqlite::Result<Vec<String>> =
            stmt.query_map([], |r| r.get::<_, String>(0))?.collect();
        rows
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "join error in /api/page-titles");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .map_err(|e| {
        tracing::error!(error = %e, "db error in /api/page-titles");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}
