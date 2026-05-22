//! RED-phase smoke test for `foliom-bench-rss` (plan 02-08, task 2).
//!
//! Asserts:
//!   1. The `foliom-bench-rss` binary builds and is callable via cargo.
//!   2. Run against a tiny ad-hoc corpus, it prints `RSS: <N> MB`
//!      where N is parseable as an integer.
//!   3. Exits 0 when RSS is well below the 450 MB ACPT-03 ceiling
//!      (a 10-file corpus served by foliom should be a fraction of
//!      that on any platform — A4 unit sanity check).
//!
//! This test purposefully uses a *small* corpus (not /tmp/synth-5k)
//! so it can run inside `cargo nextest` without the 5k-generator
//! prerequisite. CI runs the real 5k probe in the dedicated `bench`
//! job.

use std::fs;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

#[test]
fn bench_rss_prints_rss_and_exits_zero_on_tiny_corpus() {
    // Tiny corpus under tempdir — bench-rss spawns `foliom serve` on it.
    let tmp = tempfile::tempdir().expect("tempdir");
    let pages = tmp.path().join("pages");
    fs::create_dir_all(&pages).expect("pages dir");
    for i in 0..10 {
        fs::write(pages.join(format!("Page {i}.md")),
                  format!("- hello block {i} [[Page 0]] #tag\n"))
            .expect("write page");
    }

    // Use a high port to avoid colliding with any dev `foliom serve`.
    let output = Command::cargo_bin("foliom-bench-rss")
        .expect("foliom-bench-rss binary")
        .arg(tmp.path())
        .env("FOLIOM_BENCH_PORT", "17350")
        .output()
        .expect("spawn bench-rss");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("RSS:"),
            "expected `RSS:` in stdout, got stdout={stdout:?} stderr={stderr:?}");

    // Parse `RSS: <N> MB` (tolerant of trailing parenthetical).
    let n_mb: u64 = stdout
        .lines()
        .find_map(|l| l.strip_prefix("RSS:"))
        .and_then(|rest| rest.trim().split_whitespace().next())
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("could not parse RSS MB from: {stdout}"));

    // A 10-file corpus must not approach 450 MB — A4 unit sanity check:
    // if sysinfo silently switched back to KB units, n_mb would be in
    // the millions and this assertion catches it.
    assert!(n_mb < 450,
            "RSS {n_mb} MB unexpectedly high for tiny corpus — \
             possible sysinfo unit drift (A4). stderr={stderr}");
    assert!(output.status.success(),
            "bench-rss exited {:?} stdout={stdout} stderr={stderr}",
            output.status.code());
}
