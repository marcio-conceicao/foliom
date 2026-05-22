//! Static asset delivery for the Svelte SPA.
//!
//! Two profiles, one handler:
//!
//! - **Debug** (`cargo run` / `cargo build`): every miss past the `/api/*`
//!   surface 307-redirects to `http://localhost:5173/<path>`. The developer
//!   runs Vite separately and gets HMR, source maps, and ESM-on-the-fly. We
//!   pick 307 (Temporary Redirect) over 302 to preserve the HTTP method on
//!   redirect — important if a future debug session POSTs to a SPA path
//!   that turns out to be a Vite-served route.
//!
//! - **Release** (`cargo build --release`): `rust-embed` compiles
//!   `frontend/dist/` into the binary at build time. The handler looks up
//!   the requested path; on miss it falls back to `index.html` so deep
//!   hash-router links survive a hard reload (e.g. someone bookmarking
//!   `/random/non-matching/path` still loads the SPA which then routes via
//!   `window.location.hash`). `Content-Type` is derived from the path via
//!   `mime_guess`. `Cache-Control` is `no-cache` for `index.html`
//!   (so a redeploy invalidates the shell) and `public, max-age=3600` for
//!   any other asset (Vite emits content-hashed filenames, so a longer TTL
//!   is safe — and 1h is conservative; the CI can bump this later).
//!
//! `/api/*` routes are NEVER hit here — `Router::fallback` only runs on
//! misses, so registered `.route("/api/...", ...)` handlers always win.

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::Response,
};
// `IntoResponse` is only used in the release branch (`StatusCode::into_response`
// for the empty-bundle 404 path); gate the import so debug builds compile
// clean without an unused-import warning.
#[cfg(not(debug_assertions))]
use axum::response::IntoResponse;

#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;

/// Compile-time embed of the Svelte build output. Path is resolved
/// relative to `crates/cli/` (the crate manifest dir), so
/// `../../frontend/dist` points at the repo-root `frontend/dist/`.
///
/// Empty-dist tolerance: on a fresh clone `frontend/dist/` contains only
/// `.gitkeep`, which rust-embed silently ignores — `cargo check` succeeds
/// and the prod handler serves 404s until `npm run build` runs.
#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "../../frontend/dist"]
struct Assets;

/// Router fallback. Mounted via `Router::fallback(serve_static)`; never
/// shadows registered routes.
pub async fn serve_static(uri: Uri) -> Response {
    // Strip the leading slash so `Assets::get("index.html")` works (the
    // embed key is the path RELATIVE to `#[folder]`, no leading slash).
    let raw = uri.path().trim_start_matches('/');
    let path = if raw.is_empty() { "index.html" } else { raw };

    #[cfg(debug_assertions)]
    {
        // Dev: 307 to Vite. Preserve the path so /assets/foo.js round-trips.
        // Strip any query string from `uri.path_and_query()` and append it
        // ourselves so query params survive (rare for SPA assets, but cheap).
        let location = match uri.query() {
            Some(q) => format!("http://localhost:5173/{path}?{q}"),
            None => format!("http://localhost:5173/{path}"),
        };
        return Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(header::LOCATION, location)
            .body(Body::empty())
            .expect("static redirect response");
    }

    #[cfg(not(debug_assertions))]
    {
        // Prod: look up the requested asset; on miss, fall back to
        // index.html so SPA deep-links survive a hard reload.
        let (served_path, asset) = match Assets::get(path) {
            Some(a) => (path, a),
            None => match Assets::get("index.html") {
                Some(a) => ("index.html", a),
                None => return StatusCode::NOT_FOUND.into_response(),
            },
        };

        let mime = mime_guess::from_path(served_path).first_or_octet_stream();
        let cache = if served_path == "index.html" {
            // The shell must not be cached: rust-embed bakes whatever
            // index.html was on disk at compile time, and a redeploy
            // changes the hashed asset names embedded inside it. If a
            // browser caches the old shell it will request stale
            // /assets/index-<old>.js paths that no longer exist in the
            // new binary.
            "no-cache"
        } else {
            // Vite emits content-hashed asset names; long TTLs are safe.
            "public, max-age=3600"
        };

        Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache)
            .body(Body::from(asset.data.into_owned()))
            .expect("static asset response")
    }
}
