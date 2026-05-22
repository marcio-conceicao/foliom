//! `GET /api/health` — liveness probe. Returns `{"ok": true}` once the
//! server has finished startup reindex and bound its listener.
//!
//! Phase 2 plan 02-01 keeps this minimal; later plans may add index
//! counts (page/block totals) once the storage layer exposes cheap
//! aggregate accessors.

use axum::Json;
use serde_json::{Value, json};

pub async fn get() -> Json<Value> {
    Json(json!({ "ok": true }))
}
