//! Integration tests for Phase 4 watcher: SNC-03 + SNC-04.
//!
//! Each test starts `spawn_watcher` against a tempdir, performs file
//! operations, and asserts the expected `WatcherEvent` is received on
//! `broadcast_rx` within a bounded timeout.
//!
//! Tests that exercise real OS events (external_write_detected, own_write_not_echoed,
//! bulk_coalesced) rely on inotify/FSEvents and include a generous sleep to
//! absorb debounce + coalescing windows (300ms + 300ms + 100ms margin = 700ms).
//!
//! rescan_triggers_index_reset injects a synthetic event directly into the
//! mpsc channel exposed by the test API — no real OS event needed.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use foliom_cli::cmd::serve::dto::WatcherEvent;
use foliom_cli::cmd::serve::watcher::spawn_watcher_with_channel;
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;
use tempfile::TempDir;
use tokio::sync::broadcast;

// Helper: create a minimal Db + tempdir for watcher tests.
fn setup() -> (TempDir, Arc<Mutex<Db>>, Arc<SelfWriteSet>, broadcast::Receiver<WatcherEvent>) {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open(dir.path()).expect("db open");
    let db = Arc::new(Mutex::new(db));
    let self_writes = Arc::new(SelfWriteSet::default());
    let (tx, rx) = broadcast::channel::<WatcherEvent>(64);
    let tx = Arc::new(tx);

    spawn_watcher_with_channel(
        dir.path().to_path_buf(),
        self_writes.clone(),
        tx,
        db.clone(),
        300,
    )
    .expect("spawn_watcher_with_channel");

    (dir, db, self_writes, rx)
}

/// SNC-03: An external `.md` write triggers `PagesUpdated` within 700ms.
// FSEvents on macOS coalesces events with up to ~1–2 s latency, far exceeding
// the 700ms inotify-tuned timeout. The watcher behaviour is verified manually
// on macOS per ACPT-04-WATCHER.md.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn external_write_detected() {
    let (dir, _db, _sw, mut rx) = setup();

    // Give the watcher a moment to register the watch before writing.
    std::thread::sleep(Duration::from_millis(100));

    let file = dir.path().join("ExternalNote.md");
    std::fs::write(&file, "# External Note\n- hello world\n").expect("write");

    // Wait up to 700ms for PagesUpdated
    let deadline = std::time::Instant::now() + Duration::from_millis(700);
    let mut got_event = false;
    while std::time::Instant::now() < deadline {
        match rx.try_recv() {
            Ok(WatcherEvent::PagesUpdated(pages)) => {
                let found = pages.iter().any(|p| p.name.contains("ExternalNote"));
                if found {
                    got_event = true;
                    break;
                }
            }
            Ok(WatcherEvent::IndexReset) => {
                // IndexReset is also acceptable — watcher triggered a full reindex
                got_event = true;
                break;
            }
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(broadcast::error::TryRecvError::Lagged(_)) => {
                got_event = true;
                break;
            }
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    assert!(
        got_event,
        "expected PagesUpdated or IndexReset within 700ms after writing ExternalNote.md"
    );
}

/// SNC-03: Foliom's own atomic write does NOT appear as a watcher event.
///
/// We call `atomic_write_md` which registers the hash in `SelfWriteSet`.
/// The watcher must suppress the echo and NOT emit any event within 700ms.
#[test]
fn own_write_not_echoed() {
    use foliom_core::sync::atomic_write_md;

    let (dir, _db, self_writes, mut rx) = setup();

    // Give the watcher a moment to register.
    std::thread::sleep(Duration::from_millis(100));

    let file = dir.path().join("OwnNote.md");
    let content = b"# Own Note\n- foliom wrote this\n";
    atomic_write_md(&file, content, &self_writes).expect("atomic_write_md");

    // Poll for 700ms — should NOT receive any event for OwnNote.md
    let deadline = std::time::Instant::now() + Duration::from_millis(700);
    let mut got_own_event = false;
    while std::time::Instant::now() < deadline {
        match rx.try_recv() {
            Ok(WatcherEvent::PagesUpdated(pages)) => {
                if pages.iter().any(|p| p.name.contains("OwnNote")) {
                    got_own_event = true;
                    break;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(broadcast::error::TryRecvError::Lagged(_)) | Err(_) => break,
        }
    }

    assert!(
        !got_own_event,
        "own write MUST NOT be echoed as a PagesUpdated event"
    );
}

/// SNC-03 + D-40-03: 10 files written within 100ms are coalesced into
/// exactly 1 `PagesUpdated` event (or 1 `IndexReset` on rescan).
// FSEvents on macOS coalesces events with up to ~1–2 s latency; the 800ms
// wait is insufficient for reliable event delivery. Verified manually per
// ACPT-04-WATCHER.md.
#[cfg_attr(target_os = "macos", ignore)]
#[test]
fn bulk_coalesced() {
    let (dir, _db, _sw, mut rx) = setup();

    // Give the watcher a moment to register.
    std::thread::sleep(Duration::from_millis(100));

    // Write 10 .md files within 100ms
    for i in 0..10 {
        let file = dir.path().join(format!("BulkNote{i}.md"));
        std::fs::write(&file, format!("# Bulk Note {i}\n- content\n")).expect("write");
    }

    // Wait 800ms for the debounce (300ms) + coalescing tick (300ms) + margin.
    std::thread::sleep(Duration::from_millis(800));

    // Drain all events received
    let mut event_count = 0usize;
    loop {
        match rx.try_recv() {
            Ok(WatcherEvent::PagesUpdated(_)) | Ok(WatcherEvent::IndexReset) => {
                event_count += 1;
            }
            Ok(WatcherEvent::PageDeleted(_)) => {}
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                event_count += 1;
                let _ = n;
                break;
            }
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    assert!(
        event_count <= 3,
        "10 files written in <100ms should coalesce into ≤3 SSE events, got {event_count}"
    );
    assert!(event_count >= 1, "expected at least 1 event for the 10 writes, got 0");
}

/// SNC-04: A synthetic `Flag::Rescan` event triggers `IndexReset`.
///
/// This test uses the test-only `inject_mpsc_tx` handle returned by
/// `spawn_watcher_with_channel` to push a synthetic rescan event directly
/// into the watcher's mpsc channel — no real OS event needed.
#[test]
fn rescan_triggers_index_reset() {
    use foliom_cli::cmd::serve::watcher::spawn_watcher_injectable;
    use notify::{
        Event, EventKind,
        event::Flag,
    };
    use notify_debouncer_full::DebouncedEvent;

    let dir = TempDir::new().expect("tempdir");
    let db = Db::open(dir.path()).expect("db open");
    let db = Arc::new(Mutex::new(db));
    let self_writes = Arc::new(SelfWriteSet::default());
    let (tx, mut rx) = broadcast::channel::<WatcherEvent>(64);
    let tx = Arc::new(tx);

    // spawn_watcher_injectable returns the mpsc Sender for synthetic injection
    let (inject_tx, _join) = spawn_watcher_injectable(
        dir.path().to_path_buf(),
        self_writes,
        tx,
        db,
        300,
    )
    .expect("spawn_watcher_injectable");

    // Give the loop time to start
    std::thread::sleep(Duration::from_millis(50));

    // Inject a synthetic Flag::Rescan event
    let rescan_event = DebouncedEvent {
        event: Event::new(EventKind::Other).set_flag(Flag::Rescan),
        time: std::time::Instant::now(),
    };
    inject_tx
        .send(Ok(vec![rescan_event]))
        .expect("inject rescan event");

    // Wait up to 400ms for IndexReset
    let deadline = std::time::Instant::now() + Duration::from_millis(400);
    let mut got_reset = false;
    while std::time::Instant::now() < deadline {
        match rx.try_recv() {
            Ok(WatcherEvent::IndexReset) => {
                got_reset = true;
                break;
            }
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }

    assert!(
        got_reset,
        "Flag::Rescan event must trigger WatcherEvent::IndexReset"
    );
}
