//! Filesystem watcher bridge for Phase 4 (SNC-03 + SNC-04).
//!
//! [`spawn_watcher`] creates a `notify-debouncer-full` watcher on `root`,
//! bridges OS events over a `std::sync::mpsc` channel into a blocking OS
//! thread, coalesces dirty paths via a `DashMap`, triggers incremental
//! reindex, and fans out `WatcherEvent` messages to all SSE clients via
//! a `tokio::sync::broadcast::Sender`.
//!
//! # Architecture
//!
//! ```text
//! [OS FS events]
//!   → notify RecommendedWatcher
//!   → notify-debouncer-full (300ms per-path window)
//!   → mpsc::Sender<DebounceEventResult>  ← DebounceEventHandler impl
//!   → std::thread (blocking event loop)
//!       ├─ Flag::Rescan or Err → full reindex → IndexReset
//!       └─ normal events → filter .md → SelfWriteSet::take_if_present → DirtySet
//!                             DirtySet drain (300ms tick) → reindex → PagesUpdated
//!   → broadcast::Sender<WatcherEvent>  (capacity 64)
//!   → BroadcastStream → axum Sse  (per SSE client)
//! ```
//!
//! # Test API
//!
//! Two additional constructors are provided for integration tests:
//! - [`spawn_watcher_with_channel`] — same as [`spawn_watcher`] but takes a
//!   pre-built `Arc<broadcast::Sender<WatcherEvent>>` (so tests can hold the
//!   receiver before spawning).
//! - [`spawn_watcher_injectable`] — same but also returns the raw
//!   `mpsc::Sender<DebounceEventResult>` so tests can inject synthetic events
//!   (e.g. `Flag::Rescan`) without relying on real OS events.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use notify_debouncer_full::{DebounceEventResult, new_debouncer, notify::{RecursiveMode, Watcher}};
use tokio::sync::broadcast;
use tracing::{info, warn};

use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;

use super::dto::{PageUpdatedInfo, WatcherEvent};

/// Coalescing window: after the per-path debounce fires, dirty paths are
/// batched and flushed no more than once per this interval. Per D-40-03.
const COALESCE_WINDOW: Duration = Duration::from_millis(300);

/// Returns `true` if `path` ends with `.md` (case-sensitive on all platforms).
fn is_md_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Acquire the DB lock, run an incremental reindex of the entire root,
/// and return. Logs errors rather than panicking (watcher is best-effort).
fn do_full_reindex_blocking(root: &PathBuf, db: &Arc<Mutex<Db>>) {
    match db.lock() {
        Ok(mut guard) => {
            match reindex(&mut guard, root, ReindexMode::Incremental) {
                Ok(stats) => {
                    info!(
                        scanned = stats.scanned,
                        modified = stats.modified,
                        "watcher: full reindex triggered"
                    );
                }
                Err(e) => warn!(error = %e, "watcher: full reindex failed"),
            }
        }
        Err(e) => warn!(error = %e, "watcher: db mutex poisoned during full reindex"),
    }
}

/// Reindex only the provided dirty paths by running an incremental reindex
/// of the full root (safe because incremental is hash-gated — unchanged files
/// cost only a `stat` + hash comparison). Returns a vec of `(page_name,
/// file_hash)` for all pages that changed during the reindex.
///
/// Rationale for full-root incremental: per-file reindex would require a
/// private API into `reindex_impl`; the full incremental reindex is always
/// correct and fast (hash-gated), so the simplicity wins here.
fn reindex_dirty_files_blocking(
    dirty_paths: &[PathBuf],
    root: &PathBuf,
    db: &Arc<Mutex<Db>>,
) -> Vec<PageUpdatedInfo> {
    match db.lock() {
        Ok(mut guard) => {
            match reindex(&mut guard, root, ReindexMode::Incremental) {
                Ok(stats) => {
                    info!(
                        dirty_count = dirty_paths.len(),
                        scanned = stats.scanned,
                        modified = stats.modified,
                        "watcher: incremental reindex for dirty files"
                    );
                    // Build PageUpdatedInfo for the dirty set — query current
                    // file hashes from the DB for each dirty path.
                    let mut pages = Vec::new();
                    for path in dirty_paths {
                        if let Some(info) = page_info_for_path(path, root, &guard) {
                            pages.push(info);
                        }
                    }
                    pages
                }
                Err(e) => {
                    warn!(error = %e, "watcher: incremental reindex failed");
                    vec![]
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "watcher: db mutex poisoned during dirty reindex");
            vec![]
        }
    }
}

/// Query the DB for a page's name + current hash given the absolute path.
/// Returns `None` if the path is not in the index or the hash is missing.
fn page_info_for_path(
    abs_path: &PathBuf,
    root: &PathBuf,
    db: &Db,
) -> Option<PageUpdatedInfo> {
    // Compute relative path from root
    let rel = abs_path.strip_prefix(root).ok()?;
    let rel_str = rel.to_str()?;

    // Query pages.name + files.hash from the DB
    let conn = db.conn();
    let result: rusqlite::Result<(String, Vec<u8>)> = conn.query_row(
        "SELECT p.name, f.hash FROM files f \
         JOIN pages p ON p.file_id = f.id \
         WHERE f.path = ? LIMIT 1",
        rusqlite::params![rel_str],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((name, hash_bytes)) => {
            let file_hash = hex::encode(&hash_bytes);
            Some(PageUpdatedInfo { name, file_hash })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            warn!(error = %e, path = %rel_str, "watcher: page_info_for_path query failed");
            None
        }
    }
}

/// How long to keep a suppressed hash in the local dedup cache after the
/// first `take_if_present` succeeds. This absorbs inotify's tendency to
/// emit multiple events (MOVED_TO + MODIFY + CLOSE_WRITE) for a single
/// atomic rename — without this, the second event bypasses suppression
/// because `take_if_present` already consumed the entry.
const SUPPRESSED_HASH_TTL: Duration = Duration::from_millis(600);

/// The event loop body. Shared by all three public constructors.
///
/// `mpsc_rx`: receives `DebounceEventResult` from the debouncer callback.
/// `broadcast_tx`: fans events out to all SSE clients.
/// `root`, `db`: used for reindex.
/// `self_writes`: suppresses Foliom's own write echoes.
/// `_debouncer_guard`: keeps the debouncer alive for the lifetime of the thread.
fn run_event_loop<G: Send + 'static>(
    mpsc_rx: mpsc::Receiver<DebounceEventResult>,
    broadcast_tx: Arc<broadcast::Sender<WatcherEvent>>,
    root: PathBuf,
    db: Arc<Mutex<Db>>,
    self_writes: Arc<SelfWriteSet>,
    _debouncer_guard: G,
) {
    std::thread::spawn(move || {
        let _guard = _debouncer_guard; // keep debouncer alive
        let dirty: DashMap<PathBuf, ()> = DashMap::new();
        // Local cache of recently-suppressed hashes. When `take_if_present`
        // consumes a SelfWriteSet entry, we cache the hash here so that
        // follow-up inotify events for the same write (MOVED_TO, MODIFY,
        // CLOSE_WRITE) are also suppressed without a second SelfWriteSet hit.
        let suppressed: DashMap<[u8; 32], Instant> = DashMap::new();
        let mut last_drain = Instant::now();

        for result in &mpsc_rx {
            match result {
                Ok(events) => {
                    let needs_rescan = events.iter().any(|e| e.event.need_rescan());
                    if needs_rescan {
                        // macOS MustScanSubDirs or Linux IN_Q_OVERFLOW (Q4/Q3)
                        do_full_reindex_blocking(&root, &db);
                        broadcast_tx.send(WatcherEvent::IndexReset).ok();
                        dirty.clear();
                        last_drain = Instant::now();
                        continue;
                    }

                    for ev in &events {
                        for path in &ev.event.paths {
                            if !is_md_path(path) {
                                continue;
                            }
                            // Path-traversal guard (T-04-02): canonicalize and
                            // verify the path is inside the notes root.
                            let canonical = match path.canonicalize() {
                                Ok(c) => c,
                                Err(_) => {
                                    // File may have been deleted; skip.
                                    continue;
                                }
                            };
                            if !canonical.starts_with(&root) {
                                warn!(
                                    path = %path.display(),
                                    "watcher: path escapes notes root — skipping (T-04-02)"
                                );
                                continue;
                            }

                            // Self-write suppression (SNC-02 / T-04-02):
                            // read file bytes, compute hash, check SelfWriteSet.
                            match std::fs::read(path) {
                                Ok(bytes) => {
                                    let hash: [u8; 32] = blake3::hash(&bytes).into();

                                    // Check local suppressed-hash cache first
                                    // (absorbs follow-up events after take_if_present
                                    // already consumed the SelfWriteSet entry).
                                    let in_local_cache = suppressed
                                        .get(&hash)
                                        .map(|ts| ts.elapsed() < SUPPRESSED_HASH_TTL)
                                        .unwrap_or(false);

                                    if in_local_cache {
                                        continue;
                                    }

                                    if self_writes.take_if_present(&hash) {
                                        // Our own write — suppress echo and
                                        // add to local cache for follow-up events.
                                        suppressed.insert(hash, Instant::now());
                                        continue;
                                    }
                                    dirty.insert(canonical, ());
                                }
                                Err(e) => {
                                    // File may have been deleted (delete event).
                                    // Insert anyway so the drain reindexes and
                                    // detects the deletion from disk.
                                    if e.kind() != std::io::ErrorKind::NotFound {
                                        warn!(
                                            error = %e,
                                            path = %path.display(),
                                            "watcher: could not read file for hash check"
                                        );
                                    }
                                    dirty.insert(canonical, ());
                                }
                            }
                        }
                    }

                    // GC expired suppressed-hash entries to bound memory.
                    suppressed.retain(|_, ts| ts.elapsed() < SUPPRESSED_HASH_TTL);
                }
                Err(errors) => {
                    // Windows ReadDirectoryChangesW overflow or other errors
                    // (Q5): treat any error as potential missed events, trigger
                    // full reindex + IndexReset + re-arm watch.
                    for e in &errors {
                        warn!(error = %e, "watcher: debouncer error — potential missed events");
                    }
                    do_full_reindex_blocking(&root, &db);
                    broadcast_tx.send(WatcherEvent::IndexReset).ok();
                    dirty.clear();
                    last_drain = Instant::now();
                    // Note: re-arm is handled by the debouncer holding the watcher
                    // alive; the `_debouncer_guard` keeps it alive in `spawn_watcher`.
                    // For injectable tests, no real watcher is used.
                }
            }

            // Coalescing drain: flush dirty set at most once per COALESCE_WINDOW.
            if last_drain.elapsed() >= COALESCE_WINDOW && !dirty.is_empty() {
                let paths: Vec<PathBuf> =
                    dirty.iter().map(|e| e.key().clone()).collect();
                dirty.clear();
                last_drain = Instant::now();

                let changed_pages = reindex_dirty_files_blocking(&paths, &root, &db);
                if !changed_pages.is_empty() {
                    broadcast_tx
                        .send(WatcherEvent::PagesUpdated(changed_pages))
                        .ok();
                }
            }
        }
        // mpsc_rx closed — debouncer was dropped. Watcher is done.
        info!("watcher event loop exited");
    });
}

/// Start the notify-debouncer-full watcher on `root` and spawn the event
/// processing thread.
///
/// Creates a new `broadcast::channel::<WatcherEvent>(64)` internally and
/// stores the sender in the returned `Arc`. Callers that need to subscribe
/// before spawning should use [`spawn_watcher_with_channel`] instead.
///
/// # Errors
///
/// Returns `Err` if the `notify` watcher fails to initialize or if the
/// root directory cannot be watched.
pub fn spawn_watcher(
    root: PathBuf,
    self_writes: Arc<SelfWriteSet>,
    broadcast_tx: Arc<broadcast::Sender<WatcherEvent>>,
    db: Arc<Mutex<Db>>,
    debounce_ms: u64,
) -> anyhow::Result<()> {
    spawn_watcher_with_channel(root, self_writes, broadcast_tx, db, debounce_ms)
}

/// Same as [`spawn_watcher`] but takes a pre-built `broadcast::Sender` so
/// tests (and `serve/mod.rs`) can subscribe to the receiver before spawning.
///
/// This is the canonical constructor used by `serve/mod.rs`.
pub fn spawn_watcher_with_channel(
    root: PathBuf,
    self_writes: Arc<SelfWriteSet>,
    broadcast_tx: Arc<broadcast::Sender<WatcherEvent>>,
    db: Arc<Mutex<Db>>,
    debounce_ms: u64,
) -> anyhow::Result<()> {
    let (mpsc_tx, mpsc_rx) = mpsc::channel::<DebounceEventResult>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None, // tick_rate = auto (1/4 of timeout ≈ 75ms)
        mpsc_tx,
    )
    .map_err(|e| anyhow::anyhow!("notify debouncer init failed: {e}"))?;

    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| anyhow::anyhow!("notify watch failed for {:?}: {e}", root))?;

    // Register root in the FileIdMap cache for rename tracking.
    debouncer.cache().add_root(&root, RecursiveMode::Recursive);

    run_event_loop(mpsc_rx, broadcast_tx, root, db, self_writes, debouncer);
    Ok(())
}

/// Test-only constructor that also returns the raw `mpsc::Sender` so tests
/// can inject synthetic `DebounceEventResult` values (e.g. `Flag::Rescan`)
/// without relying on real OS events.
///
/// Returns `(mpsc_tx, ())` where `mpsc_tx` feeds directly into the event loop.
/// The `()` placeholder is kept for future use (e.g. a join handle).
pub fn spawn_watcher_injectable(
    root: PathBuf,
    self_writes: Arc<SelfWriteSet>,
    broadcast_tx: Arc<broadcast::Sender<WatcherEvent>>,
    db: Arc<Mutex<Db>>,
    debounce_ms: u64,
) -> anyhow::Result<(mpsc::Sender<DebounceEventResult>, ())> {
    let _ = debounce_ms; // not used — no real debouncer

    let (mpsc_tx, mpsc_rx) = mpsc::channel::<DebounceEventResult>();

    // No real watcher — the caller injects events directly.
    // We use `()` as the guard (nothing to keep alive).
    run_event_loop(mpsc_rx, broadcast_tx, root, db, self_writes, ());
    Ok((mpsc_tx, ()))
}
