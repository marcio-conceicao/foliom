//! Plan 02-07 Task 2 — integration coverage for the dev/prod static-asset
//! toggle implemented in `crates/cli/src/cmd/serve/embed.rs`.
//!
//! ## Two profiles, two assertions
//!
//! - **Debug** (`cargo test -p foliom-cli`): `GET /` must 307 to
//!   `http://localhost:5173/`, proving the Vite hot-reload loop is
//!   preserved. The release embed path is NOT compiled in.
//!
//! - **Release** (`cargo test --release -p foliom-cli --test
//!   serve_prod_embed`): `GET /` must serve the embedded `index.html`
//!   (with `<div id="app"></div>` shell), `GET /assets/*.js` must come
//!   back with `Content-Type: application/javascript`, and an arbitrary
//!   unknown path must SPA-fallback to `index.html`.
//!
//! ## Self-skipping
//!
//! Release-mode tests SKIP (instead of failing) if `frontend/dist/index.html`
//! is missing on disk at compile time of this test binary, so a fresh clone
//! that hasn't run `npm run build` does not produce spurious red CI. The
//! plan 02-08 CI workflow runs `npm run build` first, exercising the real
//! assertion path.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

fn parse_port(line: &str) -> Option<u16> {
    let after = line.split("http://127.0.0.1:").nth(1)?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn spawn_serve() -> (Child, u16) {
    let mut cmd = Command::cargo_bin("foliom").expect("locate foliom bin");
    cmd.arg("serve")
        .arg(fixture_root())
        .arg("--port")
        .arg("0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn foliom serve");
    let stdout = child.stdout.take().expect("stdout pipe");
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO);
        let line = rx
            .recv_timeout(remaining)
            .expect("timeout waiting for serve banner");
        if let Some(p) = parse_port(&line) {
            return (child, p);
        }
    }
}

// ---------------------------------------------------------------------------
// Debug profile: SPA fallback must be a 307 redirect to Vite.
// ---------------------------------------------------------------------------

#[cfg(debug_assertions)]
#[test]
fn debug_root_redirects_to_vite_5173() {
    let (mut child, port) = spawn_serve();
    let url = format!("http://127.0.0.1:{port}/");

    // ureq follows redirects by default. Disable so we can inspect the 307.
    // ureq classifies 3xx with `redirects(0)` as a successful response
    // (NOT as an error variant), so match on `resp.status()` directly.
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let resp = match agent.get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(err) => {
            let _ = child.kill();
            panic!("transport error: {err}");
        }
    };
    assert_eq!(
        resp.status(),
        307,
        "GET / in debug must be 307 redirect to Vite, got {}",
        resp.status()
    );
    let location = resp
        .header("location")
        .expect("redirect must have Location header")
        .to_string();
    assert!(
        location.starts_with("http://localhost:5173/"),
        "expected redirect to localhost:5173, got {location}"
    );

    // Asset path also redirects.
    let asset_url = format!("http://127.0.0.1:{port}/assets/index.js");
    let resp = match agent.get(&asset_url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(err) => {
            let _ = child.kill();
            panic!("transport error fetching asset: {err}");
        }
    };
    assert_eq!(resp.status(), 307, "asset path must 307 in debug");
    let location = resp.header("location").unwrap_or("").to_string();
    assert_eq!(
        location, "http://localhost:5173/assets/index.js",
        "asset redirect mismatch"
    );

    // /api/* must NOT be intercepted: fallback only fires on misses.
    let health_url = format!("http://127.0.0.1:{port}/api/health");
    let resp = agent.get(&health_url).call().expect("health 200");
    assert_eq!(resp.status(), 200, "/api/health must remain 200");

    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Release profile: prod embed must serve index.html + assets + SPA fallback.
// ---------------------------------------------------------------------------

#[cfg(not(debug_assertions))]
#[test]
fn release_embeds_spa_at_root_and_falls_back_to_index_html() {
    // Self-skip if the frontend was not built.
    let dist_index = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("frontend")
        .join("dist")
        .join("index.html");
    if !dist_index.exists() {
        eprintln!(
            "SKIP: {} not found — run `cd frontend && npm run build` before \
             `cargo test --release --test serve_prod_embed`",
            dist_index.display()
        );
        return;
    }

    let (mut child, port) = spawn_serve();
    let base = format!("http://127.0.0.1:{port}");

    // ---- 1. GET / must return index.html (HTML, contains the SPA shell). ----
    let resp = ureq::get(&format!("{base}/")).call().expect("GET /");
    assert_eq!(resp.status(), 200, "GET / must be 200");
    let ctype = resp
        .header("content-type")
        .unwrap_or("")
        .to_ascii_lowercase();
    assert!(
        ctype.starts_with("text/html"),
        "GET / Content-Type must be text/html, got {ctype}"
    );
    let body = resp.into_string().expect("body string");
    assert!(
        body.contains(r#"<div id="app">"#),
        "index.html must contain <div id=\"app\"> shell; body was: {body}"
    );

    // ---- 2. Locate a hashed JS bundle under /assets/ and fetch it. ----
    let assets_dir = dist_index.with_file_name("assets");
    let mut js_name: Option<String> = None;
    if let Ok(read) = std::fs::read_dir(&assets_dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".js") {
                js_name = Some(name);
                break;
            }
        }
    }
    if let Some(js) = js_name {
        let url = format!("{base}/assets/{js}");
        let resp = ureq::get(&url).call().expect("GET asset");
        assert_eq!(resp.status(), 200, "asset must be 200: {url}");
        let ctype = resp
            .header("content-type")
            .unwrap_or("")
            .to_ascii_lowercase();
        assert!(
            ctype.contains("javascript"),
            "asset {url} must have javascript Content-Type, got {ctype}"
        );
    }

    // ---- 3. SPA fallback: arbitrary unknown path returns index.html. ----
    let resp = ureq::get(&format!("{base}/some/random/spa/path"))
        .call()
        .expect("GET random path");
    assert_eq!(resp.status(), 200, "SPA fallback must be 200");
    let body = resp.into_string().expect("body");
    assert!(
        body.contains(r#"<div id="app">"#),
        "SPA fallback must serve index.html with app shell"
    );

    // ---- 4. /api/* still works (fallback does not capture it). ----
    let resp = ureq::get(&format!("{base}/api/health"))
        .call()
        .expect("GET health");
    assert_eq!(resp.status(), 200, "/api/health must remain 200");

    let _ = child.kill();
    let _ = child.wait();
}
