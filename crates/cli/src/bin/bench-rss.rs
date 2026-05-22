//! ACPT-03 RSS probe (plan 02-08).
//!
//! Spawns `./foliom serve <corpus> --port <port>` as a child process,
//! waits for steady state (boot + first-page fetch), reads RSS via
//! sysinfo by pid, kills the child, prints
//! `RSS: N MB (target: 300 MB, CI ceiling: 450 MB)`, and exits non-zero
//! if N exceeds the ceiling.
//!
//! Usage:
//!     foliom-bench-rss <corpus-path>
//!
//! Environment overrides:
//!     FOLIOM_BENCH_PORT       — bind port (default 7350)
//!     FOLIOM_BENCH_CEILING_MB — fail threshold in MB (default 450)
//!     FOLIOM_BENCH_FOLIOM     — path to the `foliom` binary
//!                                 (default: sibling of this exe, then
//!                                 ./target/release/foliom fallback)
//!
//! HTTP probe is hand-rolled over `std::net::TcpStream` so we do not
//! drag reqwest + rustls into the release artifact for a one-shot
//! localhost GET (per 02-RESEARCH §Performance Harness recommendation).
//!
//! Unit safety (A4): sysinfo 0.30 returns RSS in **bytes**. Earlier
//! versions returned KB. The workspace pins `sysinfo = "=0.30.13"`
//! exactly to prevent silent unit drift across upgrades.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

const DEFAULT_PORT: u16 = 7350;
const DEFAULT_CEILING_MB: u64 = 450;
const BOOT_TIMEOUT: Duration = Duration::from_secs(30);
const STEADY_STATE_GRACE: Duration = Duration::from_secs(2);

fn main() {
    if let Err(e) = run() {
        eprintln!("bench-rss: {e:#}");
        std::process::exit(2);
    }
}

fn run() -> anyhow::Result<()> {
    let corpus = std::env::args().nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: foliom-bench-rss <corpus>"))?;
    let corpus = PathBuf::from(corpus);
    anyhow::ensure!(corpus.exists(),
                    "corpus path does not exist: {}", corpus.display());

    let port: u16 = std::env::var("FOLIOM_BENCH_PORT")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_PORT);
    let ceiling_mb: u64 = std::env::var("FOLIOM_BENCH_CEILING_MB")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_CEILING_MB);

    let foliom_bin = resolve_foliom_binary()?;
    eprintln!("bench-rss: foliom binary = {}", foliom_bin.display());
    eprintln!("bench-rss: corpus       = {}", corpus.display());
    eprintln!("bench-rss: port         = {port}");
    eprintln!("bench-rss: ceiling      = {ceiling_mb} MB");

    let mut child = Command::new(&foliom_bin)
        .args(["serve"])
        .arg(&corpus)
        .args(["--port", &port.to_string()])
        // Silence the server's own log output so it doesn't interleave
        // with our `RSS:` line on stdout. stderr stays attached for
        // crash diagnostics.
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let pid = Pid::from_u32(child.id());

    // Guard: kill child even if we early-return on a probe failure.
    let result = measure(&mut child, pid, port, ceiling_mb);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn measure(_child: &mut Child, pid: Pid, port: u16,
           ceiling_mb: u64) -> anyhow::Result<()> {
    // Wait for boot — poll /api/health until 200 or BOOT_TIMEOUT.
    let deadline = Instant::now() + BOOT_TIMEOUT;
    let mut ready = false;
    while Instant::now() < deadline {
        if let Ok(body) = http_get(port, "/api/health") {
            if body.contains("\"ok\":true") {
                ready = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    anyhow::ensure!(ready, "foliom serve did not become ready within {:?}",
                    BOOT_TIMEOUT);

    // Warm a real handler (page list) to trigger the index path.
    let _ = http_get(port, "/api/pages")?;
    thread::sleep(STEADY_STATE_GRACE);

    // sysinfo 0.30: `process.memory()` returns RSS in BYTES (A4).
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new()
            .with_memory()),
    );
    sys.refresh_process(pid);
    let rss_bytes = sys.process(pid)
        .ok_or_else(|| anyhow::anyhow!("process {pid:?} not found by sysinfo"))?
        .memory();
    let rss_mb = rss_bytes / 1024 / 1024;

    println!("RSS: {rss_mb} MB (target: 300 MB, CI ceiling: {ceiling_mb} MB)");

    if rss_mb > ceiling_mb {
        anyhow::bail!("RSS {rss_mb} MB exceeds CI ceiling {ceiling_mb} MB \
                       (ACPT-03)");
    }
    Ok(())
}

/// Resolve the `foliom` binary path with three fallbacks (in order):
///   1. `$FOLIOM_BENCH_FOLIOM` — explicit override (used by tests).
///   2. Sibling of `current_exe()` — `target/release/foliom` when
///      bench-rss itself lives under `target/release/`.
///   3. `./target/release/foliom` — last-resort path relative to CWD.
fn resolve_foliom_binary() -> anyhow::Result<PathBuf> {
    if let Some(v) = std::env::var_os("FOLIOM_BENCH_FOLIOM") {
        return Ok(PathBuf::from(v));
    }
    let exe_name = if cfg!(windows) { "foliom.exe" } else { "foliom" };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join(exe_name);
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }
    let fallback = PathBuf::from(format!("./target/release/{exe_name}"));
    if fallback.exists() {
        return Ok(fallback);
    }
    anyhow::bail!("could not locate `foliom` binary — set FOLIOM_BENCH_FOLIOM \
                   or run from the workspace root after `cargo build --release \
                   --bin foliom`");
}

/// Bare-bones HTTP/1.1 GET against `127.0.0.1:<port>` returning the
/// response body. Sufficient for hitting localhost JSON endpoints — no
/// chunked-encoding support, no keep-alive, no TLS.
fn http_get(port: u16, path: &str) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let req = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: 127.0.0.1\r\n\
         Connection: close\r\n\
         Accept: application/json\r\n\
         \r\n",
    );
    stream.write_all(req.as_bytes())?;
    let mut buf = Vec::with_capacity(8192);
    stream.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    // Strip headers — body starts after the first CRLFCRLF.
    if let Some(idx) = text.find("\r\n\r\n") {
        Ok(text[idx + 4..].to_string())
    } else {
        Ok(text)
    }
}
