//! Integration test for plan 02-01 Task 2.
//!
//! Spawns `foliom serve <fixture> --port 0` as a child process, reads the
//! actual bound port from stdout, then validates:
//!   1. `GET /api/health` returns 200 with `{"ok": true}`.
//!   2. `GET /api/health` with `Host: evil.example.com` is rejected with
//!      421 Misdirected Request (DNS rebinding mitigation — T-02-01).
//!   3. SIGINT (Unix) or terminate (Windows) shuts the child cleanly.
//!
//! `--port 0` is used so the test never collides with a developer running
//! `foliom serve` locally on 7345; the fallback path is exercised in the
//! manual verification step (curl on 7345 twice in a row).

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

/// How long we wait for the server to print its banner before declaring
/// startup broken. Reindex over the synthetic fixture is fast (<1s on
/// dev hardware); this gives generous slack for cold CI runners.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// How long we wait for the child to exit after sending the shutdown
/// signal. axum's graceful-shutdown future should resolve immediately
/// once `ctrl_c` fires.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

/// Spawn the binary and wait for the startup banner; return (child, bound_port).
fn spawn_serve() -> (std::process::Child, u16) {
    let mut cmd = Command::cargo_bin("foliom").expect("locate foliom bin");
    cmd.arg("serve")
        .arg(fixture_root())
        .arg("--port")
        .arg("0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn foliom serve");

    // Read stdout on a worker thread so we can timeout the parent.
    let stdout = child.stdout.take().expect("stdout pipe");
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            // Forward every line; the parent only consumes until it sees
            // the banner, then leaves the channel to drain naturally.
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
        let line = rx.recv_timeout(remaining).expect(
            "timeout waiting for `Foliom servindo em ...` banner — server failed to start",
        );
        if let Some(port) = parse_port(&line) {
            return (child, port);
        }
    }
}

/// Parse the bound port from a banner line like:
///   `Foliom servindo em http://127.0.0.1:43217 — Ctrl+C para parar`
fn parse_port(line: &str) -> Option<u16> {
    let after = line.split("http://127.0.0.1:").nth(1)?;
    // The port runs until the first non-digit. Use take_while.
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Best-effort graceful shutdown: SIGINT on Unix, `kill()` on Windows
/// (Windows lacks a portable "send Ctrl+C to child" without attaching to
/// the same console; killing the process is acceptable here since the
/// graceful-shutdown semantics are exercised manually).
fn shutdown(mut child: std::process::Child) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let pid = child.id() as i32;
        // SAFETY: kill(2) with SIGINT on our own child PID is sound.
        unsafe {
            libc_kill(pid, SIGINT);
        }

        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // SIGINT-triggered graceful shutdown: tokio's ctrl_c
                    // future resolves and axum::serve returns Ok, so the
                    // process exits 0. Some platforms may still report
                    // signal termination if the handler is not registered
                    // in time — accept both shapes.
                    let ok = status.success()
                        || status.signal() == Some(SIGINT)
                        || status.code() == Some(130);
                    assert!(ok, "unexpected exit status after SIGINT: {status:?}");
                    return;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        panic!("child did not exit within {SHUTDOWN_TIMEOUT:?} of SIGINT");
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("try_wait failed: {e}"),
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Windows: just kill. Graceful Ctrl+C across processes is not
        // portable without attaching a console; the cross-platform
        // graceful path is exercised manually per the plan's verification.
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(unix)]
const SIGINT: i32 = 2;

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[test]
fn health_returns_ok_and_host_allowlist_rejects_evil_host() {
    let (child, port) = spawn_serve();
    let base = format!("http://127.0.0.1:{port}");

    // ---- 1. /api/health → 200 {"ok": true} ----
    let resp = ureq::get(&format!("{base}/api/health"))
        .call()
        .expect("GET /api/health");
    assert_eq!(resp.status(), 200, "health should return 200");
    let body: serde_json::Value = resp.into_json().expect("parse health JSON");
    assert_eq!(
        body,
        serde_json::json!({ "ok": true }),
        "health body shape"
    );

    // ---- 2. /api/health with Host: evil.example.com → 421 ----
    // Note: ureq lets us override the Host header. Servers MUST reject
    // this even though the socket-level connection is to 127.0.0.1.
    let result = ureq::get(&format!("{base}/api/health"))
        .set("Host", "evil.example.com:7345")
        .call();
    match result {
        Err(ureq::Error::Status(421, _)) => { /* expected */ }
        Err(ureq::Error::Status(code, _)) => {
            panic!("expected 421 for evil Host, got {code}")
        }
        Ok(resp) => panic!(
            "expected 421 rejection for evil Host, got success {}",
            resp.status()
        ),
        Err(other) => panic!("unexpected transport error: {other}"),
    }

    // ---- 3. graceful shutdown ----
    shutdown(child);
}
