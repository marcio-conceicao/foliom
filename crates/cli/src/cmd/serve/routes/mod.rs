//! HTTP routing. Task 2 of plan 02-01 wires `/api/health`; subsequent
//! plans (02-02..02-06) extend this router with the page/journal/search
//! endpoints from D-24.

pub mod health;

use axum::{Router, middleware as axum_middleware, routing::get};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

use crate::cmd::serve::middleware::host_allowlist;
use crate::cmd::serve::state::AppState;

/// Build the axum router with all middleware layers attached.
///
/// Layer order matters in axum: the `host_allowlist` layer is applied
/// last so it runs *first* on the request (rejecting hostile Host
/// headers before any handler or compression work happens).
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health::get))
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(axum_middleware::from_fn(host_allowlist))
}
