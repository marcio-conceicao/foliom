//! Best-effort browser launcher for the `--open` flag (D-22).
//!
//! The `open` crate dispatches to `xdg-open` (Linux), `open` (macOS), and
//! `cmd /C start` (Windows). Failure is non-fatal — we log a warning and
//! let the user paste the printed URL manually.

/// Try to open `url` in the user's default browser. Any error is
/// downgraded to a `tracing::warn!` so a missing/blocked launcher never
/// brings the server down.
pub fn try_open(url: &str) {
    if let Err(err) = open::that(url) {
        tracing::warn!(
            url = url,
            error = %err,
            "não foi possível abrir o navegador (--open); cole a URL manualmente"
        );
    }
}
