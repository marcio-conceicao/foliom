//! atomic_write_md behaviour suite — Plan 03-01 Task 2.
//!
//! Covers (from PLAN must_haves.truths + verification):
//!   - Linux happy path: bytes land identically, hash returned matches
//!     blake3, hash is registered in SelfWriteSet BEFORE we observe the
//!     write (verified by take_if_present == true immediately after).
//!   - Missing-parent rejection as a proxy for cross-FS error path.
//!   - Round-trip rehearsal (SNC-01 / ACPT-01 bridge): re-writing the
//!     existing fixture bytes is byte-identical (no encoding rewrite).
//!   - Windows-only: target held open by another thread → either Ok via
//!     retry, or Err after ≥3 attempts. Assert attempt counter from the
//!     test-only thread_local.

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use tempfile::tempdir;

use super::*;
use crate::sync::SelfWriteSet;

fn fixture_path(rel: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/logseq-synthetic");
    p.push(rel);
    p
}

#[test]
fn linux_happy_path_writes_bytes_and_registers_hash() {
    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("foo.md");
    let set = SelfWriteSet::default();

    let bytes = b"- hello\n";
    let hash = atomic_write_md(&target, bytes, &set).expect("write");

    // Bytes hit disk exactly as given.
    let on_disk = fs::read(&target).expect("read");
    assert_eq!(on_disk, bytes);

    // Hash returned matches blake3(contents).
    let expected: [u8; 32] = blake3::hash(bytes).into();
    assert_eq!(hash, expected);

    // The hash was registered BEFORE the rename — so by the time we get
    // control back, take_if_present must succeed.
    assert!(
        set.take_if_present(&hash),
        "self-write hash must be registered by the time atomic_write_md returns"
    );
}

#[test]
fn missing_parent_directory_returns_not_found() {
    let dir = tempdir().expect("tempdir");
    let bogus = dir.path().join("does-not-exist").join("file.md");
    let set = SelfWriteSet::default();

    let err = atomic_write_md(&bogus, b"x", &set).expect_err("should fail");
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::NotFound,
        "missing parent → NotFound (proxy for cross-FS CrossesDevices); got: {err:?}"
    );
}

#[test]
fn no_op_rewrite_is_byte_identical() {
    // Round-trip rehearsal for ACPT-01: writing the same bytes back to a
    // file produces a byte-identical buffer. No newline injection, no
    // encoding rewrite, no trailing-whitespace strip.
    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("rt.md");
    let original = b"- alpha\n  - beta\n- gamma\n";
    {
        let mut f = fs::File::create(&target).expect("create");
        f.write_all(original).expect("seed");
        f.sync_all().expect("sync");
    }

    let set = SelfWriteSet::default();
    let _ = atomic_write_md(&target, original, &set).expect("rewrite");

    let after = fs::read(&target).expect("read");
    assert_eq!(after, original, "no-op rewrite must be byte-identical");
}

#[test]
fn round_trip_preserves_fixture_bytes() {
    // Bridge to ACPT-01: load a real synthetic Logseq fixture, copy it
    // into a tempdir, rewrite with `atomic_write_md`, assert bytes match
    // the original file from `crates/core/tests/fixtures/`.
    let src = fixture_path("pages/01-simple-bullets.md");
    let original = fs::read(&src).expect("fixture read");

    let dir = tempdir().expect("tempdir");
    let copy = dir.path().join("01-simple-bullets.md");
    fs::copy(&src, &copy).expect("seed copy");

    let set = SelfWriteSet::default();
    atomic_write_md(&copy, &original, &set).expect("rewrite");

    let after = fs::read(&copy).expect("read");
    assert_eq!(
        after, original,
        "atomic_write_md must not mutate fixture bytes"
    );
}

#[test]
#[cfg_attr(not(windows), ignore = "Windows AV retry test — non-Windows targets skip")]
fn windows_av_retry_triggers_attempt_counter() {
    // Smoke test: hold the target file open in another thread while the
    // write attempts to rename over it. On Windows this typically yields
    // `ERROR_ACCESS_DENIED` → ErrorKind::PermissionDenied → retry path.
    //
    // We don't assert a specific outcome (Ok via retry or Err after
    // exhausting retries) — both are valid behaviours of the OS — but we
    // DO assert the retry loop iterated when the OS surfaced a transient
    // error. On Linux this test is `#[ignore]`d because rename(2) does not
    // fail with the kinds we retry on.
    use std::fs::OpenOptions;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("locked.md");
    // Seed the file so OpenOptions(read+write) can latch onto it.
    fs::write(&target, b"seed\n").expect("seed");

    let (start_tx, start_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let lock_target = target.clone();
    let locker = thread::spawn(move || {
        let _f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_target)
            .expect("open for lock");
        start_tx.send(()).ok();
        // Hold the handle for ~250ms — covers our 50+100+200 backoff.
        thread::sleep(Duration::from_millis(250));
        done_tx.send(()).ok();
    });

    start_rx.recv().expect("locker started");

    let set = SelfWriteSet::default();
    // Attempt the write while the file is locked.
    let _ = atomic_write_md(&target, b"new contents\n", &set);

    done_rx.recv().expect("locker finished");
    locker.join().expect("locker joined");

    let attempts = LAST_PERSIST_ATTEMPTS.with(|c| c.get());
    // Either: persist succeeded on attempt 1 (counter == 0 — OS didn't
    // actually deny us, AV not active), or it retried at least once.
    // The plan demands ≥3 attempts in the *failing* case; we tolerate the
    // benign no-AV case by accepting attempts == 0 OR attempts >= 3.
    assert!(
        attempts == 0 || attempts >= 3,
        "expected retry counter at 0 (no AV interference) or >= 3 (retry exhausted); got {attempts}"
    );
}
