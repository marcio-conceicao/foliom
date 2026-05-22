# Phase 4: Disk Sync — Research

**Researched:** 2026-05-22
**Domain:** notify-debouncer-full watcher integration, tokio broadcast SSE, cross-platform OS event recovery, Svelte 5 watcher-status UI
**Confidence:** HIGH (all 5 open questions answered from primary sources: notify/debouncer-full source code and docs.rs, tokio-stream docs, fsevent.rs and inotify.rs inspected directly)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

| Area | Decision | Source |
|---|---|---|
| Watcher library | `notify 6.1 + notify-debouncer-full 0.3` with `RecommendedCache` | research/STACK.md, PRD §5.5 |
| Debounce mechanism | `notify-debouncer-full` built-in; per-path window | SNC-03 |
| Self-write suppression | `SelfWriteSet::take_if_present` from `crates/core/src/sync/self_writes.rs` | SNC-02 (Phase 3) |
| Watch level | Parent-directory recursive (NOT per-file) | SNC-04 |
| Rescan fallback triggers | Windows ReadDirectoryChangesW overflow + macOS MustScanSubDirs | SNC-04 |
| SSE for live push | axum SSE, one global broadcast::Sender in AppState | D-40-02 |
| Debounce window | 300ms, env-overridable via `FOLIOM_DEBOUNCE_MS` | D-40-01 |
| Bulk coalescing | DashMap DirtySet + 300ms coalescing tick | D-40-03 |
| Conflict UI | Non-blocking banner above CM6 editor; foreground wins | D-40-04 |
| SSE reconnect | EventSource auto-reconnect; `watcherStatus` Svelte store | D-40-05 |

### Claude's Discretion

None explicitly stated — all key decisions locked in CONTEXT.md.

### Deferred Ideas (OUT OF SCOPE)

- SNC-02 (self-write set) — already shipped in Phase 3
- Three-way merge conflict resolution — too complex for v1; foreground-wins + reload is the policy
- Sync protocol (Syncthing/git themselves)
- Code-signing cert procurement for Phase 5 (but start admin process during Phase 4)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SNC-03 | FS watcher detects external edits, runs incremental reindex, pushes SSE event to all connected clients | notify-debouncer-full callback → DirtySet coalescing → tokio broadcast → axum Sse stream |
| SNC-04 | Directory-level recursive watch, rescan fallback on MustScanSubDirs/ReadDirectoryChangesW overflow, inotify budget within limits | Verified: `EventKind::Other + Flag::Rescan` emitted by both FSEvents and inotify overflow; Windows missing Rescan — must fallback on `Error` paths |
| SNC-06 | Conflict banner when external edit lands on a page with an active CM6 editor | Re-uses Phase 3 `staleConflict` state in PageView.svelte; watcher SSE event triggers same banner path |
</phase_requirements>

---

## Summary

Phase 4 wires three systems together: (1) the `notify-debouncer-full` filesystem watcher running in a background tokio task, (2) a `tokio::sync::broadcast` channel that fans out `WatcherEvent` messages to all SSE clients, and (3) a Svelte 5 `EventSource` listener that drives live-reload and the watcher-status indicator.

The 5 open questions from CONTEXT.md are fully answered by primary source inspection of `notify` and `notify-debouncer-full` source code and docs.rs. The key practical findings: `new_debouncer` uses a **callback** (not a direct channel), but `std::sync::mpsc::Sender<DebounceEventResult>` is a first-class `DebounceEventHandler` implementation — so the bridge is `mpsc` callback → tokio task reads from `mpsc::Receiver` in `spawn_blocking` loop → publishes to `broadcast::Sender`. The `Flag::Rescan` signal is the unified recovery trigger on both macOS (FSEvents `MustScanSubDirs`) and Linux (inotify `IN_Q_OVERFLOW`); Windows does NOT emit Rescan on `ReadDirectoryChangesW` buffer overflow — recovery there is driven by catching watcher errors. The inotify watch count for a Foliom corpus is O(directory count), not O(file count), so a flat `pages/` + `journals/` structure generates only ~2–10 watches against the 8192 default limit.

**Primary recommendation:** Use the `mpsc`-based handler pattern (`std::sync::mpsc::channel()`, pass `tx` to `new_debouncer`, receive in `std::thread::spawn` → convert to broadcast fan-out). This keeps the debouncer's internal thread isolated from the tokio runtime while still feeding the async broadcast channel. The axum SSE endpoint converts a `broadcast::Receiver` into a `tokio_stream::wrappers::BroadcastStream`, maps each item to an `axum::response::sse::Event`, and calls `.keep_alive(KeepAlive::new().interval(Duration::from_secs(30)))`.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| FS event detection + debounce | Backend (Rust, OS thread spawned by notify) | — | Watcher must own IO; browser FS API forbidden (PRD §5.4) |
| Self-write suppression | Backend (SelfWriteSet from Phase 3) | — | Hash comparison must happen at the same layer that wrote the file |
| Incremental reindex on dirty file | Backend (existing `indexer::reindex`) | — | SQLite ownership is backend-only |
| SSE fan-out | Backend (tokio broadcast + axum Sse) | — | Server pushes; client subscribes |
| Live-reload on page change | Frontend (Svelte store + `fetchPage`) | — | Frontend owns rendering decisions |
| Conflict banner | Frontend (PageView.svelte `staleConflict`) | — | Re-uses Phase 3 stale path; no new backend logic |
| Watcher status indicator | Frontend (Sidebar.svelte pill) | — | UI-only state, driven by EventSource events |
| Coalescing dirty writes under Syncthing storm | Backend (DirtySet + 300ms tick) | — | Prevents UI flooding and per-file SQLite transactions |

---

## Answers to the 5 Open Questions

### Q1: notify-debouncer-full constructor signature + event delivery

**Answer:** [VERIFIED: docs.rs/notify-debouncer-full/0.3.2]

The public constructor is `new_debouncer`, not `RecommendedDebouncer::new`:

```rust
pub fn new_debouncer<F: DebounceEventHandler>(
    timeout: Duration,
    tick_rate: Option<Duration>,
    event_handler: F,
) -> Result<Debouncer<RecommendedWatcher, FileIdMap>, Error>
```

- `timeout`: the per-path debounce window (pass `Duration::from_millis(300)` per D-40-01)
- `tick_rate`: how often the internal thread flushes the debounce queue; `None` = auto (¼ of timeout = 75ms)
- `event_handler`: any type implementing `DebounceEventHandler` — this is the callback-or-channel bridge point

**Channel bridge (recommended for Foliom):** `std::sync::mpsc::Sender<DebounceEventResult>` implements `DebounceEventHandler` — pass it directly:

```rust
let (tx, rx) = std::sync::mpsc::channel::<DebounceEventResult>();
let mut debouncer = new_debouncer(
    Duration::from_millis(debounce_ms),
    None,
    tx,  // mpsc Sender IS the handler — stdlib impl, no trait impl needed
)?;
// Attach the watch AFTER creating the debouncer:
debouncer.watcher().watch(&root, RecursiveMode::Recursive)?;
// ALSO register root in the FileIdMap cache (required for rename tracking):
debouncer.cache().add_root(&root, RecursiveMode::Recursive);
```

**Event delivery:** The internal thread calls `handle_event` on the mpsc Sender. In Foliom's watcher task, consume from `rx` in a **blocking** `std::thread::spawn` loop (NOT tokio-async):

```rust
// Runs in its own OS thread — does NOT need tokio context
std::thread::spawn(move || {
    for result in &rx {   // blocking iteration over mpsc Receiver
        match result {
            Ok(events) => { /* publish to broadcast channel */ }
            Err(errors) => { /* log + handle Rescan if present */ }
        }
    }
});
```

**Why blocking, not async:** The debouncer's internal thread is not a tokio task. `mpsc::Receiver` is sync. Use `tokio::sync::broadcast::Sender::send` (which is sync, not async) to forward events to the broadcast channel from the blocking thread.

**`DebounceEventResult` type:**
```rust
type DebounceEventResult = Result<Vec<DebouncedEvent>, Vec<notify::Error>>;
```

**`DebouncedEvent` structure:** wraps a `notify::Event` with a `time: Instant` field.

**Rescan event handling in debouncer-full:** When the underlying watcher emits `Flag::Rescan`, the debouncer calls `cache.rescan(roots)` (refreshes FileIdMap) and stores the event as a `DebouncedEvent`. It is NOT passed through raw — it will be emitted with the debounce delay applied. The Foliom watcher loop should check `event.need_rescan()` and trigger a full incremental reindex of the watched root.

**Confidence:** HIGH — constructor and channel-impl verified from docs.rs and source inspection.

---

### Q2: tokio::sync::broadcast back-pressure — Lagged recovery

**Answer:** [VERIFIED: docs.rs/tokio/latest/tokio/sync/broadcast]

`RecvError::Lagged(n)` is returned when the receiver missed `n` messages because the broadcast channel was at capacity and the sender dropped the oldest message. After the error, the receiver's position is **automatically advanced** to the oldest message still in the buffer — the next `recv()` returns that value (not another error).

**Recommended capacity for Foliom:** 64 messages. At 300ms debounce + 300ms coalescing tick, the maximum burst rate is ~3 SSE events/second. 64 slots = ~21 seconds of buffer — much more than any plausible SSE client lag.

**Recovery path (confirmed correct per CONTEXT.md D-40-02):**

```rust
loop {
    match rx.recv().await {
        Ok(event) => { yield Ok(sse_event_from(event)); }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            tracing::warn!(skipped = n, "SSE client lagged — sending index_reset");
            yield Ok(Event::default()
                .event("index_reset")
                .data("{}"));
            // receiver position is already advanced by tokio — next recv() is safe
        }
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```

**Why this is the right recovery:** After `Lagged`, the client's UI state may be stale by an unknown number of page changes. `index_reset` instructs the client to `fetchPage(currentPageName)` unconditionally — equivalent to a "force refresh from server". The client does not need to know which pages changed; it re-fetches the one it's viewing. This matches D-40-02's declared event types.

**Alternative rejected:** Draining all buffered messages on lag and sending a `pages_updated` with the union — this requires iterating the buffer, which the broadcast API does not expose. `index_reset` is the correct primitive.

**tokio-stream bridge:** To avoid hand-rolling the `recv()` loop, use `tokio_stream::wrappers::BroadcastStream`:

```rust
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

let stream = BroadcastStream::new(broadcast_tx.subscribe())
    .map(|result| match result {
        Ok(event) => Ok(sse_event_from(event)),
        Err(_lagged) => Ok(Event::default().event("index_reset").data("{}")),
    });
Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(30)))
```

`BroadcastStream` from `tokio-stream 0.1` maps `RecvError::Lagged` to `BroadcastStreamRecvError::Lagged`. The `.map()` converts it to the `index_reset` event.

**Confidence:** HIGH — broadcast channel behavior verified from tokio docs; BroadcastStream verified from tokio-stream docs.rs.

---

### Q3: Linux inotify watch count under RecursiveMode::Recursive

**Answer:** [VERIFIED: github.com/notify-rs/notify/blob/main/notify/src/inotify.rs]

`RecursiveMode::Recursive` registers **one inotify watch per directory** (not per file). The implementation uses `WalkDir` to enumerate all subdirectories and calls `inotify.watches().add()` for each.

**For a Foliom corpus:**
- `pages/` (1 watch) + `journals/` (1 watch) + vault root (1 watch) + typical Logseq subdirs (`assets/`, `draws/`, `whiteboards/`, `bak/`, `.recycle/`, `version-files/`, `logseq/`) = ~10 watches, even counting all subdirectories.
- Even a vault with 50 topic subdirectories = ~55 watches — well under the 8192 Linux default.
- The `walkdir`-based ignore list in Phase 1 (`crates/core/src/scanner/ignore.rs`) already skips Logseq's non-content directories. The watcher should mirror that ignore list to avoid adding watches for `assets/`, `.git/`, etc.

**`IN_Q_OVERFLOW` handling (kernel event queue overflow):** When inotify's kernel-side event queue overflows, `inotify.rs` emits:

```rust
// inotify.rs source (verified):
if event.mask.contains(EventMask::Q_OVERFLOW) {
    let ev = Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan));
    self.event_handler.handle_event(ev);
}
```

This is `EventKind::Other + Flag::Rescan` — the same signal as macOS MustScanSubDirs (Q4). The Foliom watcher loop handles both cases identically.

**`ENOSPC` watch-limit exhaustion:** If `add_watch` fails with `ENOSPC` (limit reached), `notify` converts it to `ErrorKind::MaxFilesWatch` and delivers it as an `Err(Error)` to the event handler. Foliom should log a clear warning: "inotify watch limit reached — increase fs.inotify.max_user_watches" and continue with the watches that did succeed (partial coverage better than crash).

**WSL2 caveat** (from memory/project_dev_targets.md): Marcelo develops on WSL2. inotify on WSL2 works correctly for files on the WSL2 filesystem (`/home/...`). For notes stored on the Windows filesystem (`/mnt/c/...`), `notify` falls back to polling or may miss events. The existing startup warning about `/mnt/` paths (PITFALLS.md §6, per memory document) covers this.

**Confidence:** HIGH — inotify.rs source inspected directly.

---

### Q4: macOS FSEvents MustScanSubDirs event representation

**Answer:** [VERIFIED: github.com/notify-rs/notify/blob/main/notify/src/fsevent.rs]

When FSEvents fires `kFSEventStreamEventFlagMustScanSubDirs`, `fsevent.rs` emits:

```rust
// fsevent.rs source (verified):
if flags.contains(StreamFlags::MUST_SCAN_SUBDIRS) {
    let e = Event::new(EventKind::Other).set_flag(Flag::Rescan);
    // also adds info string: "rescan: user dropped" or "rescan: kernel dropped"
    self.event_handler.handle_event(Ok(e));
}
```

**Representation in the Foliom watcher loop:**
- `event.kind` = `EventKind::Other`
- `event.flag()` = `Some(Flag::Rescan)`
- Detection predicate: `event.need_rescan()` (convenience method on `Event`)

**debouncer-full behavior:** The debouncer intercepts the Rescan event, calls `cache.rescan(roots)` to refresh the FileIdMap, then stores the event as a `DebouncedEvent` subject to the normal debounce window. It is NOT forwarded raw — it arrives in the Foliom callback as a `DebouncedEvent` whose inner `Event` has `Flag::Rescan`.

**Recovery in Foliom (SNC-04):** Detect `DebouncedEvent.event.need_rescan()` in the watcher loop and trigger a full incremental reindex of the watched root (not just a single file). Emit `index_reset` over SSE so all clients refresh.

```rust
// In the debouncer event loop:
for result in &rx {
    match result {
        Ok(events) => {
            let needs_rescan = events.iter().any(|e| e.event.need_rescan());
            if needs_rescan {
                // Full incremental reindex of entire root
                do_full_reindex(&state);
                broadcast_tx.send(WatcherEvent::IndexReset).ok();
            } else {
                // Normal per-file path
                for ev in events.iter().filter(|e| is_md_file(&e.event)) {
                    dirty_set.insert(ev.event.paths[0].clone());
                }
                // drain dirty_set on coalescing tick
            }
        }
        Err(errors) => {
            // errors may include Rescan signals from debouncer's error pass-through
            for e in &errors {
                tracing::warn!(error = %e, "watcher error");
            }
        }
    }
}
```

**Confidence:** HIGH — fsevent.rs source inspected directly.

---

### Q5: Windows ReadDirectoryChangesW overflow

**Answer:** [VERIFIED: github.com/notify-rs/notify/blob/main/notify/src/windows.rs — with important caveat]

**Finding (from source inspection):** The `windows.rs` implementation in notify 6.x does **NOT** emit `Flag::Rescan` on ReadDirectoryChangesW buffer overflow. The buffer is statically sized at 16 KB. When an overflow occurs (equivalent to `ERROR_NOTIFY_ENUM_DIR`), the implementation logs an error and unwatches the directory:

```
log::error!("unknown error in ReadDirectoryChangesW for directory {}: {}", ...);
// then calls unwatch() on the affected path
```

This means the Foliom watcher receives:
1. No more events from the affected directory (silently dropped until re-watched), OR
2. An `Err(Vec<notify::Error>)` in the `DebounceEventResult` error path

**Recovery path for Foliom (SNC-04):** The Foliom watcher must handle the `Err` path from the debouncer as a rescan trigger on Windows, not just log and continue:

```rust
Err(errors) => {
    for e in &errors {
        tracing::warn!(error = %e, "watcher error");
        // On Windows, errors from ReadDirectoryChangesW mean events were lost
        // Re-trigger a full reindex as the safe fallback
    }
    // Treat any watcher error as a potential missed-event scenario
    do_full_reindex(&state);
    broadcast_tx.send(WatcherEvent::IndexReset).ok();
}
```

**Practical risk for Foliom's corpus:** Buffer overflow requires many simultaneous file changes (e.g., `git checkout` of 500 files). The 16 KB buffer holds roughly 200–400 typical file-change notifications. A normal Syncthing sync of a few dozen files will NOT overflow. The risk is real only for large `git pull` operations on a 5k-file corpus. The 300ms coalescing tick (D-40-03) drains the DirtySet in bulk, so normal Syncthing storms are already absorbed before reaching the Windows buffer limit.

**Cross-reference:** debouncer-full does NOT add its own Rescan wrapping for Windows errors — errors are passed through the `Err(Vec<Error>)` branch of `DebounceEventResult`.

**Confidence:** HIGH for the "no Rescan on Windows" finding (source inspected). MEDIUM for the exact overflow trigger threshold (16 KB static buffer, docs.rs only; not tested on live Windows system).

---

## Standard Stack

### Core (new additions for Phase 4)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `notify` | `6.1` | FS event backend | Already pinned in STACK.md; not yet in Cargo.lock (new dep) |
| `notify-debouncer-full` | `0.3` | Debounce + FileIdMap rename tracking | Already pinned in STACK.md; not yet in Cargo.lock |
| `tokio-stream` | `0.1.18` | `BroadcastStream` wrapper for broadcast::Receiver → Stream for SSE | Part of tokio project; slopcheck [OK] |

### Already in workspace (no changes needed)

| Library | Version | Note |
|---------|---------|------|
| `tokio::sync::broadcast` | (tokio 1.40) | Already in workspace; add `"sync"` feature to tokio workspace dep |
| `axum` | `0.7` | SSE via `axum::response::sse::{Sse, Event, KeepAlive}` — requires `tokio` feature |
| `dashmap` | `6.2.1` | DirtySet for coalescing |

### Feature flag changes needed

```toml
# Cargo.toml (workspace)
# tokio: add "sync" feature (already has "rt", "macros", "signal", "net")
tokio = { version = "1.40", features = ["macros", "rt", "signal", "net", "sync"] }

# axum: add "tokio" feature for SSE support
axum = { version = "0.7", features = ["tokio"] }
```

**Note on axum SSE feature:** The current workspace pins `axum = "0.7"` without explicit features. `axum::response::sse` is gated on the `tokio` cargo feature. Verify whether the current axum dependency already enables it transitively (likely yes via tokio runtime dependency), but the explicit feature flag is the safe path.

**Installation for Phase 4:**
```toml
# crates/cli/Cargo.toml additions
notify               = "6.1"
notify-debouncer-full = "0.3"
tokio-stream          = { version = "0.1", features = ["sync"] }
```

---

## Package Legitimacy Audit

| Package | Registry | Age | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|
| `notify` | crates.io | ~8 yrs (notify-rs org) | [OK] | Approved |
| `notify-debouncer-full` | crates.io | ~2 yrs (notify-rs org) | [OK] | Approved |
| `tokio-stream` | crates.io | ~4 yrs (tokio project) | [OK] | Approved |
| `async-stream` | crates.io | ~5 yrs (tokio project) | [OK] | Approved (optional alternative) |

**Packages removed due to slopcheck [SLOP]:** none
**Packages flagged [SUS]:** none

*slopcheck run: 2026-05-22 — all 4 packages rated [OK].*

---

## Architecture Patterns

### System Architecture Diagram

```
[OS Filesystem Events]
       │
       ▼
[notify RecommendedWatcher]  (native: inotify/FSEvents/ReadDirectoryChangesW)
       │  (raw events)
       ▼
[notify-debouncer-full]      (per-path 300ms window, FileIdMap rename tracking)
       │  DebounceEventResult via mpsc::Sender
       ▼
[std::thread watcher_loop]   (blocking OS thread, outside tokio)
       │
       ├─── Flag::Rescan / Err branch ──→ spawn_blocking(full_reindex) → broadcast(IndexReset)
       │
       └─── normal events ──→ filter .md, SelfWriteSet.take_if_present()
                              │  (non-echo events only)
                              ▼
                         [DashMap DirtySet]
                              │  (300ms coalescing tick)
                              ▼
                    spawn_blocking(reindex dirty files)
                              │
                              ▼
                    broadcast::Sender<WatcherEvent>  (in AppState)
                              │
                ┌─────────────┼─────────────┐
                ▼             ▼             ▼
           [SSE client 1] [SSE client 2] [SSE client N]
           BroadcastStream  BroadcastStream  ...
                │
          EventSource (browser)
                │
        ┌───────┴──────────┐
        ▼                  ▼
  pages_updated        index_reset
  → update fileHash    → fetchPage()
  → conflict banner    → full reload
    if editing
```

### Recommended Project Structure (new files)

```
crates/cli/src/cmd/serve/
├── watcher.rs           # spawn_watcher(root, self_writes, broadcast_tx) → JoinHandle
├── routes/
│   └── watch.rs         # GET /api/watch/events → SSE stream handler
frontend/src/lib/
├── stores/
│   └── watcher.ts       # watcherStatus store ('connected'|'reconnecting'|'offline')
├── watcher.ts           # startWatcher(), stopWatcher(), onWatcherEvent() composable
└── components/
    └── Sidebar.svelte   # add watcherStatus pill indicator (already exists, add pill)
```

### Pattern 1: Watcher spawn (backend)

```rust
// crates/cli/src/cmd/serve/watcher.rs
// Source: docs.rs/notify-debouncer-full/0.3.2 + CONTEXT.md D-40-01/D-40-03

use std::sync::{Arc, mpsc};
use std::time::Duration;
use notify_debouncer_full::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use tokio::sync::broadcast;

pub fn spawn_watcher(
    root: PathBuf,
    self_writes: Arc<SelfWriteSet>,
    broadcast_tx: Arc<broadcast::Sender<WatcherEvent>>,
    db: Arc<Mutex<Db>>,
    debounce_ms: u64,   // from FOLIOM_DEBOUNCE_MS env or 300
) -> anyhow::Result<()> {
    let (mpsc_tx, mpsc_rx) = mpsc::channel::<DebounceEventResult>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None,           // tick_rate = auto (1/4 of timeout)
        mpsc_tx,        // std::sync::mpsc::Sender implements DebounceEventHandler
    )?;

    // Watch the root directory recursively (one inotify watch per subdir on Linux)
    debouncer.watcher().watch(&root, RecursiveMode::Recursive)?;
    // Register root in FileIdMap cache for rename tracking
    debouncer.cache().add_root(&root, RecursiveMode::Recursive);

    // Spawn blocking OS thread for the event loop
    std::thread::spawn(move || {
        let _debouncer = debouncer; // keep alive
        let dirty: dashmap::DashMap<PathBuf, ()> = dashmap::DashMap::new();
        let coalesce_window = Duration::from_millis(300);
        let mut last_drain = std::time::Instant::now();

        for result in &mpsc_rx {
            match result {
                Ok(events) => {
                    let needs_rescan = events.iter().any(|e| e.event.need_rescan());
                    if needs_rescan {
                        // macOS MustScanSubDirs or Linux IN_Q_OVERFLOW
                        do_full_reindex_blocking(&root, &db);
                        let _ = broadcast_tx.send(WatcherEvent::IndexReset);
                        continue;
                    }
                    for ev in &events {
                        for path in &ev.event.paths {
                            if !is_md_path(path) { continue; }
                            // Self-write suppression: read file, check hash
                            if let Ok(bytes) = std::fs::read(path) {
                                let hash = blake3::hash(&bytes).into();
                                if self_writes.take_if_present(&hash) { continue; }
                                dirty.insert(path.clone(), ());
                            }
                        }
                    }
                }
                Err(errors) => {
                    // Windows ReadDirectoryChangesW overflow or other errors
                    for e in &errors {
                        tracing::warn!(error = %e, "watcher error — treating as potential missed events");
                    }
                    do_full_reindex_blocking(&root, &db);
                    let _ = broadcast_tx.send(WatcherEvent::IndexReset);
                }
            }

            // Coalescing drain
            if last_drain.elapsed() >= coalesce_window && !dirty.is_empty() {
                let paths: Vec<PathBuf> = dirty.iter().map(|e| e.key().clone()).collect();
                dirty.clear();
                let changed_names = reindex_dirty_files_blocking(&paths, &root, &db);
                if !changed_names.is_empty() {
                    let _ = broadcast_tx.send(WatcherEvent::PagesUpdated(changed_names));
                }
                last_drain = std::time::Instant::now();
            }
        }
    });

    Ok(())
}
```

### Pattern 2: SSE handler (axum)

```rust
// crates/cli/src/cmd/serve/routes/watch.rs
// Source: docs.rs/axum/0.7.9/axum/response/sse + docs.rs/tokio-stream/0.1.18

use axum::response::sse::{Event, KeepAlive, Sse};
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub async fn watch_events_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.watcher_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|result| {
        let event = match result {
            Ok(WatcherEvent::PagesUpdated(pages)) => {
                let data = serde_json::to_string(&pages).unwrap_or_default();
                Event::default().event("pages_updated").data(data)
            }
            Ok(WatcherEvent::PageDeleted(name)) => {
                Event::default().event("page_deleted")
                    .data(format!(r#"{{"name":"{}"}}"#, name))
            }
            Ok(WatcherEvent::IndexReset) => {
                Event::default().event("index_reset").data("{}")
            }
            Err(_lagged) => {
                // RecvError::Lagged — client missed events, force full refresh
                Event::default().event("index_reset").data("{}")
            }
        };
        Ok::<_, std::convert::Infallible>(event)
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(30)))
}
```

### Pattern 3: AppState extension

```rust
// Add to crates/cli/src/cmd/serve/state.rs

use tokio::sync::broadcast;

pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub root: PathBuf,
    pub self_writes: Arc<SelfWriteSet>,
    pub journal: Arc<Journal>,
    // NEW for Phase 4:
    pub watcher_tx: Arc<broadcast::Sender<WatcherEvent>>,
}
```

### Pattern 4: Frontend EventSource (Svelte 5)

```typescript
// frontend/src/lib/watcher.ts
import { watcherStatus } from './stores/watcher';
import { currentPage } from './stores';
import { fetchPage } from './api';

let es: EventSource | null = null;
let offlineTimer: ReturnType<typeof setTimeout> | null = null;

export function startWatcher(): void {
  es = new EventSource('/api/watch/events');

  es.addEventListener('open', () => {
    watcherStatus.set('connected');
    clearOfflineTimer();
  });

  es.addEventListener('pages_updated', (e) => {
    const updated: { name: string; fileHash: string }[] = JSON.parse(e.data);
    const cp = get(currentPage);
    if (cp && updated.some(p => p.name === cp.name)) {
      // Check if this page is currently being edited
      const editing = get(currentlyEditing);
      if (editing !== null) {
        // Conflict: user is editing — set conflict store, show banner
        externalConflict.set({ newFileHash: updated.find(p => p.name === cp.name)!.fileHash });
      } else {
        // No active edit — silently reload
        fetchPage(cp.name).then(fresh => currentPage.set(fresh));
      }
    }
  });

  es.addEventListener('index_reset', () => {
    const cp = get(currentPage);
    if (cp) fetchPage(cp.name).then(fresh => currentPage.set(fresh));
  });

  es.addEventListener('error', () => {
    watcherStatus.set('reconnecting');
    scheduleOfflineTimer();
  });
}
```

### Anti-Patterns to Avoid

- **DO NOT** pass a tokio-async channel (e.g., `tokio::sync::mpsc`) as the debouncer event handler. The debouncer's internal thread is NOT a tokio task; calling `tokio_sender.send()` from a blocking thread requires `tokio::runtime::Handle::current().spawn_blocking()` ceremony and breaks easily.
- **DO NOT** call `debouncer.watcher().watch()` before calling `new_debouncer()` — the debouncer must be created first.
- **DO NOT** call `debouncer.cache().add_root()` with the wrong `RecursiveMode` relative to what was passed to `.watch()` — this will break rename tracking.
- **DO NOT** filter `.md` files inside the debouncer callback at the event kind level (`Modify(Data)` only) — macOS FSEvents and Windows ReadDirectoryChangesW sometimes only emit `EventKind::Any`, not `EventKind::Modify(ModifyKind::Data(...))`. Filter by file extension on `event.paths`, not by `EventKind`.
- **DO NOT** perform reindex synchronously on the OS thread running the debouncer callback — this blocks event delivery. Use a coalescing DirtySet drained by a background thread, as specified in D-40-03.
- **DO NOT** use `axum`'s SSE feature without adding `tokio` to axum's feature list — `axum::response::sse` is gated on the `tokio` feature.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-path debounce + rename tracking | Custom timer per path + inode map | `notify-debouncer-full` with `FileIdMap` | Handles VS Code atomic rename (write-tmp → rename-over), macOS FSEvents coarseness, Linux IN_MOVED_FROM/IN_MOVED_TO pairing |
| Broadcast fan-out to SSE clients | Custom `Vec<mpsc::Sender>` with manual cleanup | `tokio::sync::broadcast` | Back-pressure handled via `Lagged`; disconnected receivers automatically cleaned up; no manual cleanup code |
| Stream adapter for broadcast receiver | Manual `poll_fn` future | `tokio_stream::wrappers::BroadcastStream` | Idiomatic; integrates with `.map()/.filter()` chain that axum SSE expects |
| File ID tracking across renames | `HashMap<(dev, ino), PathBuf>` | `FileIdMap` from notify-debouncer-full | Already handles cross-platform file IDs (inode on Linux/macOS, file index on Windows) |

---

## Common Pitfalls

### Pitfall 1: Watcher echo not suppressed on the first event after startup
**What goes wrong:** The startup `reindex()` call in `serve/mod.rs` does not write files, so `SelfWriteSet` is empty. The watcher starts immediately after `reindex()` and may emit a stale event for a file that was modified between `Db::open` and `watcher.watch()`. This causes a spurious reindex of 0-delta files.
**Why it happens:** There is a TOCTOU window between `Db::open` and `watch()` registration.
**How to avoid:** Start the watcher BEFORE the startup `reindex()`, or accept the spurious reindex (it's a no-op for the index since `mtime` matches). The latter is simpler and correct — a no-op reindex costs < 1ms for a few files.
**Warning signs:** Log shows "reindex: 1 modified" immediately after startup with no external changes.

### Pitfall 2: debouncer dropped before events are drained
**What goes wrong:** `let mut debouncer = new_debouncer(...)` is local to `spawn_watcher`. If `debouncer` is dropped, the background thread stops. The OS thread's `for result in &mpsc_rx` loop exits because the `mpsc_tx` sender (held inside `debouncer`) is dropped.
**How to avoid:** Move `_debouncer` into the OS thread closure: `let _debouncer = debouncer;` — keep it alive for the lifetime of the thread.

### Pitfall 3: Svelte EventSource leaks on route navigation
**What goes wrong:** Navigating away from a page triggers Svelte component unmount. If `EventSource` is created per-component, the old instance is not closed, and a new one is opened — leading to duplicate event handlers and `watcherStatus` flapping.
**How to avoid:** Create a single global `EventSource` in `watcher.ts` (module-level), started once in `App.svelte`'s `onMount`. Never create per-component EventSources.

### Pitfall 4: `externalConflict` store and `staleConflict` state are two different things
**What goes wrong:** Phase 3 uses `staleConflict = $state(false)` in `PageView.svelte` (a local component state, triggered by `PUT /api/blocks` returning 409). Phase 4 adds a watcher-triggered conflict path. Conflating them leads to one banner for two causes OR two separate banners.
**How to avoid:** Per D-40-04, reuse the same `staleConflict` state and `reload()` function in `PageView.svelte`. The watcher SSE handler sets `staleConflict = true` via a callback passed down from `PageView`. No new banner component needed.

### Pitfall 5: Windows watcher silently stops after ReadDirectoryChangesW error
**What goes wrong:** As documented in Q5, `notify/src/windows.rs` calls `unwatch()` on the affected directory after an unrecognized error. The watcher is now blind to that directory.
**How to avoid:** In the `Err` branch of the debouncer callback, after triggering a full reindex, also call `debouncer.watcher().watch(&root, RecursiveMode::Recursive)` to re-register the watch. This re-arms coverage.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest 0.9 (already in CI) |
| Config file | `.config/nextest.toml` (if exists) or workspace default |
| Quick run command | `cargo nextest run -p foliom-cli --test watcher_integration` |
| Full suite command | `cargo nextest run --workspace --no-fail-fast` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | Notes |
|--------|----------|-----------|-------------------|-------|
| SNC-03 | Watcher detects external `.md` write, reindexes, SSE pushes `pages_updated` | Integration | `cargo nextest run -p foliom-cli --test watcher_integration::external_write_detected` | Uses `tempfile` corpus + write + sleep(400ms) |
| SNC-03 | Self-write echo suppressed | Integration | `cargo nextest run -p foliom-cli --test watcher_integration::own_write_not_echoed` | atomic_write_md → verify no SSE event |
| SNC-03 | Syncthing storm (10 files) coalesced into 1 SSE event | Integration | `cargo nextest run -p foliom-cli --test watcher_integration::bulk_coalesced` | Write 10 files within 100ms, wait 600ms, assert 1 SSE event |
| SNC-04 | Linux inotify Q_OVERFLOW → IndexReset emitted | Integration (Linux-only) | `cargo nextest run -p foliom-cli --test watcher_integration::rescan_triggers_index_reset` | Simulate `Flag::Rescan` event |
| SNC-04 | macOS MustScanSubDirs → IndexReset emitted | Integration (macOS-only) | Same test, cfg-gated | `#[cfg(target_os = "macos")]` |
| SNC-06 | External edit on open page shows conflict banner | Frontend unit | `npm run test -- watcher.test.ts` | Mock EventSource, fire `pages_updated`, assert `staleConflict` |
| SNC-06 | External edit on page with no active editor → silent reload | Frontend unit | `npm run test -- watcher.test.ts` | `currentlyEditing = null` path |

### Key Test Fixtures

| Fixture | Purpose |
|---------|---------|
| `tempdir` with 3 `.md` files | Atomic rename simulation: write tmp, rename over target |
| `tempdir` with 10 `.md` files written in <100ms | Bulk dirty coalescing (D-40-03 verification) |
| Mock `DebounceEventResult::Ok(vec![Event::new(EventKind::Other).set_flag(Flag::Rescan)])` | Rescan path test without needing real OS events |
| Frontend: mock `EventSource` in Vitest | SSE event → store/UI assertions without backend |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p foliom-cli --test watcher_integration` + `npm run test`
- **Per wave merge:** `cargo nextest run --workspace --no-fail-fast`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps (new files needed before implementation)

- [ ] `crates/cli/tests/watcher_integration.rs` — covers SNC-03, SNC-04
- [ ] `frontend/src/__tests__/watcher.test.ts` — covers SNC-06 frontend paths

---

## Plan Breakdown Recommendation

### Plan 04-01: Backend watcher integration (SNC-03 + SNC-04)

**Scope:** `crates/cli/src/cmd/serve/watcher.rs` (new) + `state.rs` change + `mod.rs` spawn call + `routes/watch.rs` (new SSE endpoint) + `crates/cli/Cargo.toml` additions + new `WatcherEvent` enum in `dto.rs`.

**Tasks:**
1. Add `notify`, `notify-debouncer-full`, `tokio-stream` deps + feature flags for `axum/tokio` + `tokio/sync`
2. Define `WatcherEvent` enum (`PagesUpdated`, `PageDeleted`, `IndexReset`) in `dto.rs`
3. Add `watcher_tx: Arc<broadcast::Sender<WatcherEvent>>` to `AppState` + wire `broadcast::channel::<WatcherEvent>(64)` in `mod.rs`
4. Implement `spawn_watcher` in `watcher.rs` — mpsc bridge, DirtySet coalescing, self-write suppression, Rescan recovery, Windows error recovery
5. Implement `GET /api/watch/events` SSE handler in `routes/watch.rs` using `BroadcastStream` + Lagged→IndexReset
6. Register route in `routes/mod.rs`
7. Spawn watcher in `mod.rs::run()` after `AppState` is built
8. Tests: `crates/cli/tests/watcher_integration.rs` covering external write, own-write suppression, bulk coalescing, Rescan path

**Wave dependency:** Can start immediately (depends only on Phase 3 assets already shipped).

**TDD fixture:** `tempdir` + write `.md` file externally → assert `broadcast_tx` receives `PagesUpdated` within 700ms (300ms debounce + 300ms coalesce + 100ms margin).

---

### Plan 04-02: Frontend SSE subscription + live reload + watcher status (SNC-03 + SNC-06 partial)

**Scope:** `frontend/src/lib/watcher.ts` (new) + `frontend/src/lib/stores/watcher.ts` (new) + `App.svelte` (start watcher) + `PageView.svelte` (handle watcher events + conflict banner wire) + `Sidebar.svelte` (status pill).

**Depends on:** 04-01 (SSE endpoint must exist for frontend integration tests to pass).

**Tasks:**
1. `stores/watcher.ts` — `watcherStatus` Svelte store (`'connected' | 'reconnecting' | 'offline'`)
2. `stores/watcher.ts` — `externalConflict` store (`{ newFileHash: string } | null`) for watcher-triggered conflict
3. `watcher.ts` — `startWatcher()` / `stopWatcher()` — single global EventSource, event handlers for `pages_updated`, `index_reset`, `error`, `open`
4. `App.svelte` — call `startWatcher()` in `onMount`, `stopWatcher()` on page unload
5. `PageView.svelte` — subscribe to `externalConflict` store; if conflict arrives and `currentlyEditing !== null`, set `staleConflict = true` (reuse existing banner); if `currentlyEditing === null`, silently reload
6. `Sidebar.svelte` — add watcherStatus pill (green/spinning/grey dot, bottom-left, CSS-only)
7. Tests: `watcher.test.ts` — mock EventSource, assert store transitions + PageView behavior

**TDD fixture:** Vitest mock EventSource implementation that fires synthetic `MessageEvent` objects.

---

### Plan 04-03: Integration smoke + ACPT-04 gate

**Scope:** E2E smoke test in `ci.yml` + optional `ACPT-04-WATCHER.md` acceptance doc.

**Depends on:** 04-01 + 04-02.

**Tasks:**
1. Add E2E watcher smoke to `ci.yml` (Unix only): boot `foliom serve`, subscribe to `/api/watch/events`, write a `.md` file externally, assert `pages_updated` arrives within 1s
2. Document manual acceptance test steps for Windows watcher + Syncthing storm scenario

**Note:** This plan is lightweight — 04-01 and 04-02 are where the implementation lives. 04-03 is the CI wire and acceptance gate.

### Wave Dependencies

```
Wave 1: 04-01 (backend watcher + SSE endpoint)
Wave 2: 04-02 (frontend SSE + stores + PageView wire)  [depends on 04-01]
Wave 3: 04-03 (CI integration smoke)                   [depends on 04-01 + 04-02]
```

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `notify` crate | SNC-03/04 watcher | ✓ (new dep, will add) | 6.1 | — |
| `notify-debouncer-full` crate | SNC-03 debounce | ✓ (new dep, will add) | 0.3 | — |
| `tokio-stream` crate | SSE BroadcastStream | ✓ (new dep, will add) | 0.1.18 | hand-roll `poll_fn` loop (not recommended) |
| inotify (Linux) | SNC-04 watcher backend | ✓ (WSL2 kernel 6.6) | — | — |
| Windows `ReadDirectoryChangesW` | SNC-04 Windows watcher | N/A (dev on WSL2) | — | CI matrix covers it |
| macOS FSEvents | SNC-04 macOS watcher | N/A (dev on WSL2) | — | CI matrix covers it |

**Missing dependencies with no fallback:** none — all are addable crates.

---

## Security Domain

| ASVS Category | Applies | Control |
|---------------|---------|---------|
| V5 Input Validation | yes | SSE endpoint takes no user input; watcher reads only from the configured notes root |
| V4 Access Control | yes | `GET /api/watch/events` is loopback-only (inherited from existing Host-header allowlist middleware); no auth needed (single-user) |
| V5 Path traversal | yes | `watcher.rs` must verify that every dirty path is under `root` before reindexing — use `path.starts_with(&root)` guard |

**Path-traversal guard (load-bearing):** Symlinks in the notes root can point outside the vault. The watcher receives events for paths that may resolve outside root. Always check `canonical_path.starts_with(root)` before triggering reindex. See PITFALLS.md §11 (symlink traversal).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `axum = "0.7"` workspace dep enables `axum::response::sse` when `tokio` feature is added | Standard Stack | Planner must verify with `cargo check` in Wave 0; if not, add `features = ["tokio"]` to axum workspace dep |
| A2 | `tokio` workspace dep at `features = ["rt", "macros", "signal", "net"]` does NOT include `sync` yet | Standard Stack | If sync is already included (e.g., transitive), the feature addition is a no-op; verify with `cargo tree` |
| A3 | `notify 6.1` and `notify-debouncer-full 0.3` are compatible (debouncer pins notify version) | Stack | Pinned versions are known compatible per STACK.md; confirmed from docs.rs both reference the same notify 6.x |
| A4 | Windows `ReadDirectoryChangesW` buffer overflow does NOT emit `Flag::Rescan` | Q5 answer | If wrong (i.e., a newer notify release adds Rescan on Windows), the `Err` branch recovery is still safe (over-triggers reindex rather than under-triggers) |

---

## Open Questions

1. **`axum` feature flags in workspace:** The workspace `axum = "0.7"` has no explicit features. Confirm whether `axum::response::sse` compiles with the current dep or requires adding `features = ["tokio"]` to the workspace manifest.
   - What we know: SSE is behind the `tokio` feature per docs.rs
   - What's unclear: Whether the current transitive tokio dependency already enables it
   - Recommendation: Add `features = ["tokio"]` explicitly in Wave 0 task; `cargo check -p foliom-cli` confirms

2. **`tokio/sync` feature already in workspace tokio dep?** Current: `features = ["macros", "rt", "signal", "net"]`. `broadcast` is in `tokio::sync`, gated on the `sync` feature.
   - Recommendation: Add `"sync"` to tokio workspace features as part of Plan 04-01 Wave 0.

3. **Keep-alive ping format:** D-40-02 specifies `event: ping` every 30s. The axum `KeepAlive` type sends an SSE comment (`: keepalive`) by default. Confirm whether the browser `EventSource` stays alive on comments vs named events.
   - What we know: SSE spec requires any non-empty line to prevent proxy timeout; comments (`: ...`) are valid keep-alive
   - Recommendation: Use `KeepAlive::new().interval(30s).text("ping")` to send a comment-style ping — this is simpler than a named `ping` event which the frontend would need to explicitly ignore.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-file inotify watch | Recursive directory watch with notify-debouncer-full | notify 5.x → 6.x | Eliminates O(N-files) watch exhaustion |
| Timestamp-based self-write dedup | Hash-based dedup via SelfWriteSet | Phase 3 (already shipped) | Eliminates race conditions on clock skew |
| SSE via polling (`setInterval GET /api/changes`) | True push via `EventSource` + axum Sse | Phase 4 | No poll overhead; instant notification |

---

## Sources

### Primary (HIGH confidence)
- `github.com/notify-rs/notify/blob/main/notify/src/fsevent.rs` — MustScanSubDirs → `EventKind::Other + Flag::Rescan` (inspected)
- `github.com/notify-rs/notify/blob/main/notify/src/inotify.rs` — per-directory watch, `IN_Q_OVERFLOW` → `Flag::Rescan` (inspected)
- `github.com/notify-rs/notify/blob/main/notify/src/windows.rs` — no Rescan on overflow, error path (inspected)
- `github.com/notify-rs/notify/blob/main/notify-debouncer-full/src/lib.rs` — Rescan handling, mpsc Sender impl (inspected)
- `docs.rs/notify-debouncer-full/0.3.2` — `new_debouncer` constructor signature (verified)
- `docs.rs/tokio-stream/0.1.18` — `BroadcastStream` + `BroadcastStreamRecvError` (verified)
- `docs.rs/tokio/latest/tokio/sync/broadcast` — `RecvError::Lagged` behavior (verified)
- `docs.rs/axum/0.7.9/axum/response/sse` — SSE handler pattern, `tokio` feature gate (verified)
- `crates/core/src/sync/self_writes.rs` — `take_if_present` API (read directly)
- `crates/cli/src/cmd/serve/state.rs` — current `AppState` fields (read directly)
- `crates/cli/src/cmd/serve/mod.rs` — serve startup sequence (read directly)
- `.planning/phases/03-outliner-editor/03-01-SUMMARY.md` — SelfWriteSet API confirmed shipped
- `.planning/phases/03-outliner-editor/03-03-SUMMARY.md` — mutation API + AppState.self_writes confirmed
- `frontend/src/lib/pages/PageView.svelte` — `staleConflict` + `reload()` (read directly)
- `frontend/src/lib/stores/editing.ts` — `currentlyEditing` store (read directly)

### Secondary (MEDIUM confidence)
- slopcheck 0.6.1 — all 4 new packages rated [OK]

---

## Metadata

**Confidence breakdown:**
- Q1 (debouncer API): HIGH — constructor signature from docs.rs, mpsc impl from source
- Q2 (broadcast Lagged): HIGH — tokio docs + tokio-stream BroadcastStream verified
- Q3 (inotify watch count): HIGH — inotify.rs source inspected for per-directory behavior
- Q4 (macOS MustScanSubDirs): HIGH — fsevent.rs source, exact code snippet found
- Q5 (Windows overflow): HIGH for "no Rescan emitted"; MEDIUM for overflow threshold (16 KB buffer seen in source, not tested live)
- Plan breakdown: HIGH — based on locked CONTEXT.md decisions and Phase 3 asset inventory

**Research date:** 2026-05-22
**Valid until:** 2026-06-22 (30 days; notify 6.x is stable; debouncer-full 0.3 is current stable vs. 0.8.0-rc.2 pre-release — pin `"0.3"` to stay on stable)

---

## notify Version Warning

`cargo search notify-debouncer-full` currently shows `0.8.0-rc.2` as the latest version — this is a **pre-release** (RC). The locked decision in CONTEXT.md pins `0.3` (stable). Do NOT upgrade to 0.8.0-rc.2. When adding to `Cargo.toml`, use exact version `= "0.3"` or `"0.3.2"` to prevent accidental RC upgrade.

Similarly, `notify` search shows `9.0.0-rc.4` as latest. Pin `notify = "6.1"` explicitly.
