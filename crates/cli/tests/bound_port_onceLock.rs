//! Integration test for plan 05-01 Task 1 — BOUND_PORT OnceLock (D-50-02).
//!
//! Verifies that after `serve::run()` starts, `BOUND_PORT` OnceLock is set
//! with a valid non-zero port number. This is the exact behaviour the Tauri
//! setup hook depends on to construct `http://127.0.0.1:<port>/`.
//!
//! RED commit: test exists, imports BOUND_PORT from foliom_cli.
//! GREEN commit: BOUND_PORT.set() added to serve::run(), test passes.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use foliom_cli::cmd::serve::{BOUND_PORT, ServeArgs, run as serve_run};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

/// Spawn serve::run() in a background OS thread (matching the Tauri shell
/// pattern — never inside a tokio runtime to avoid nested-runtime panics).
/// Poll BOUND_PORT for up to 5 s (matches Tauri shell timeout).
/// Assert the port is non-zero and that the HTTP server is reachable.
#[test]
fn bound_port_is_set_before_rt_block_on() {
    // Spawn serve in a background thread (same pattern as Tauri main.rs).
    thread::spawn(|| {
        let _ = serve_run(ServeArgs {
            root: fixture_root(),
            port: 0, // OS-assigned port — same as Tauri shell
            open: false,
            full: false,
        });
    });

    // Poll BOUND_PORT for up to 5 seconds — mirrors the Tauri setup hook.
    let deadline = Instant::now() + Duration::from_secs(5);
    let port = loop {
        if let Some(p) = BOUND_PORT.get() {
            break *p;
        }
        assert!(
            Instant::now() < deadline,
            "BOUND_PORT was not set within 5s — serve::run() missing OnceLock.set()"
        );
        thread::sleep(Duration::from_millis(20));
    };

    assert_ne!(port, 0, "BOUND_PORT should be a real OS-assigned port, not 0");

    // Verify the server is actually listening on the bound port.
    let url = format!("http://127.0.0.1:{port}/api/health");
    let resp = ureq::get(&url).call().expect("GET /api/health");
    assert_eq!(resp.status(), 200, "axum server must be reachable on BOUND_PORT");
}
