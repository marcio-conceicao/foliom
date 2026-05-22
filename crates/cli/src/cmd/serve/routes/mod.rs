//! HTTP routing. Plan 02-01 wired `/api/health`; plan 02-02 adds the seven
//! read-only endpoints from D-24 (the SearchKind variants count as one
//! route with query-driven routing). Subsequent plans (02-03..02-08) add
//! the embedded SPA on `/`. Plan 03-03 adds mutation endpoints.

pub mod blocks;
pub mod health;
pub mod journals;
pub mod pages;
pub mod search;
pub mod titles;

use axum::{Router, middleware as axum_middleware, routing::{delete, get, patch, post, put}};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

use crate::cmd::serve::embed;
use crate::cmd::serve::middleware::host_allowlist;
use crate::cmd::serve::state::AppState;

/// Build the axum router with all middleware layers attached.
///
/// Layer order matters in axum: the `host_allowlist` layer is applied
/// last so it runs *first* on the request (rejecting hostile Host
/// headers before any handler or compression work happens).
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Liveness probe (plan 02-01).
        .route("/api/health", get(health::get))
        // Page surface (plan 02-02).
        .route("/api/pages", get(pages::list))
        .route("/api/pages/:name", get(pages::detail))
        .route("/api/pages/:name/backlinks", get(pages::backlinks))
        .route("/api/page-titles", get(titles::list))
        // Journals (plan 02-02).
        .route("/api/journals/today", get(journals::today))
        .route("/api/journals", get(journals::range))
        // Search (plan 02-02).
        .route("/api/search", get(search::search))
        // Plan 03-03: mutation endpoints.
        .route("/api/blocks", post(blocks::post_block))
        .route("/api/blocks/:id", put(blocks::put_block))
        .route("/api/blocks/:id/structure", patch(blocks::patch_block_structure))
        .route("/api/blocks/:id", delete(blocks::delete_block))
        // Plan 02-07: SPA static-asset fallback. Misses on `/api/*` cannot
        // reach this — `Router::fallback` runs only when no `.route(...)`
        // matches. In debug: 307 to Vite (`localhost:5173`). In release:
        // serve from the rust-embed bundle with SPA fallback to index.html.
        .fallback(embed::serve_static)
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(axum_middleware::from_fn(host_allowlist))
}
