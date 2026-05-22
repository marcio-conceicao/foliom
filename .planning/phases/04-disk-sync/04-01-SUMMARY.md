---
phase: 04-disk-sync
plan: 01
subsystem: backend
tags: [rust, notify, notify-debouncer-full, sse, tokio-broadcast, watcher, self-write-suppression, inotify, fsevent]

# Dependency graph
requires:
  - phase: 03-outliner-editor
    provides: "SelfWriteSet::take_if_present (self-write suppression), AppState shape, atomic_write_md"
provides:
  - "spawn_watcher: notify-debouncer-full bridge with DirtySet coalescing, self-write suppression, Flag::Rescan handling, Windows Err recovery"
  - "GET /api/watch/events: BroadcastStream SSE endpoint; Lagged → index_reset; 30s keep-alive"
  - "AppState.watcher_tx: Arc<broadcast::Sender<WatcherEvent>> (capacity 64)"
  - "WatcherEvent enum: PagesUpdated/PageDeleted/IndexReset + PageUpdatedInfo DTO"
  - "4 integration tests (SNC-03 + SNC-04): external write, self-write dedup, bulk coalesce, rescan"
affects:
  - "04-02 (frontend SSE subscription — depends on this endpoint)"
  - "04-03 (CI integration smoke — depends on this endpoint)"

# Tech tracking
tech-stack:
  added:
    - "notify 6.1.1 (pinned stable — 9.0.0-rc.4 is RC)"
    - "notify-debouncer-full 0.3.2 (pinned stable — 0.8.0-rc.2 is RC)"
    - "tokio-stream 0.1 with sync feature (BroadcastStream adapter)"
    - "dashmap workspace dep added to crates/cli (DirtySet for coalescing)"
  patterns:
    - "mpsc::Sender as DebounceEventHandler — bridges notify blocking thread to tokio broadcast"
    - "Local suppressed-hash DashMap cache absorbs multi-event inotify storms for same write"
    - "DirtySet (DashMap<PathBuf,()>) drained on 300ms coalescing tick — prevents per-file SSE events"
    - "spawn_watcher_injectable constructor exposes mpsc channel for test event injection without real OS events"
    - "sse_event_from_result pure function exposed as pub for unit testability"

key-files:
  created:
    - "crates/cli/src/cmd/serve/watcher.rs"
    - "crates/cli/src/cmd/serve/routes/watch.rs"
    - "crates/cli/tests/watcher_integration.rs"
  modified:
    - "crates/cli/src/cmd/serve/dto.rs (WatcherEvent + PageUpdatedInfo added)"
    - "crates/cli/src/cmd/serve/state.rs (watcher_tx field added)"
    - "crates/cli/src/cmd/serve/mod.rs (broadcast channel + spawn_watcher wired)"
    - "crates/cli/src/cmd/serve/routes/mod.rs (/api/watch/events route registered)"
    - "Cargo.toml (tokio sync feature + axum tokio feature)"
    - "crates/cli/Cargo.toml (notify + notify-debouncer-full + tokio-stream + dashmap)"
    - "4 existing test files (watcher_tx dummy channel added to AppState construction)"

key-decisions:
  - "Local suppressed-hash cache (DashMap<[u8;32], Instant>, TTL 600ms) added to absorb inotify multi-event storms per write — take_if_present consumes the SelfWriteSet entry but subsequent events for same path would bypass suppression without the cache"
  - "spawn_watcher_injectable exposes mpsc channel for test injection — no real OS events needed for rescan test (avoids platform-specific test timing)"
  - "sse_event_from_result is a pure pub fn — unit-testable without spinning up an HTTP server"
  - "Full root incremental reindex used for dirty path set (vs per-file variant) — correct, hash-gated, avoids private indexer API"

patterns-established:
  - "notify mpsc bridge: std::sync::mpsc::Sender implements DebounceEventHandler; event loop runs in std::thread (not tokio) to avoid runtime context requirements"
  - "Suppressed hash cache: after take_if_present succeeds, cache hash in local DashMap with TTL to dedup follow-up inotify events"
  - "Test-only constructor pattern: spawn_watcher_injectable returns the mpsc tx for synthetic event injection"

requirements-completed: [SNC-03, SNC-04]

# Metrics
duration: 12min
completed: 2026-05-22
---

# Phase 4 Plan 1: Backend Watcher Integration Summary

**notify-debouncer-full watcher bridged to tokio broadcast SSE with DirtySet coalescing, self-write suppression, and Flag::Rescan + Windows Err recovery; GET /api/watch/events endpoint live**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-22T12:30:45Z
- **Completed:** 2026-05-22T12:42:45Z
- **Tasks:** 2 (both TDD)
- **Files modified:** 11

## Accomplishments

- `spawn_watcher` wires `notify-debouncer-full` (300ms per-path debounce, FileIdMap rename tracking) via mpsc bridge into a blocking OS thread that coalesces dirty `.md` paths with a 300ms tick, suppresses Foliom's own writes via `SelfWriteSet::take_if_present` + local hash cache, and fans out `WatcherEvent` to all SSE clients via a capacity-64 broadcast channel
- `GET /api/watch/events` SSE endpoint uses `BroadcastStream` to map each `WatcherEvent` (or `Lagged` error) to the correct SSE event name; `index_reset` on Lagged prevents client state divergence; 30s keep-alive prevents proxy timeouts
- 4 integration tests green: `external_write_detected` (700ms budget), `own_write_not_echoed` (self-write dedup), `bulk_coalesced` (10 files → ≤3 events), `rescan_triggers_index_reset` (synthetic Flag::Rescan injection)

## Task Commits

1. **RED: failing watcher integration tests** — `bfbe823` (test)
2. **Task 1: Cargo deps + WatcherEvent + AppState + spawn_watcher** — `c1239dc` (feat)
3. **Task 2: GET /api/watch/events SSE endpoint** — `7a06a1d` (feat)

## Files Created/Modified

- `crates/cli/src/cmd/serve/watcher.rs` — spawn_watcher, spawn_watcher_with_channel, spawn_watcher_injectable; full event loop with DirtySet coalescing, SelfWriteSet suppression + local cache, Flag::Rescan handling, Windows Err recovery
- `crates/cli/src/cmd/serve/routes/watch.rs` — watch_events_handler (SSE), sse_event_from_result (pure, testable), 3 unit tests
- `crates/cli/tests/watcher_integration.rs` — 4 integration tests covering SNC-03 + SNC-04
- `crates/cli/src/cmd/serve/dto.rs` — WatcherEvent enum + PageUpdatedInfo struct
- `crates/cli/src/cmd/serve/state.rs` — watcher_tx: Arc<broadcast::Sender<WatcherEvent>>
- `crates/cli/src/cmd/serve/mod.rs` — broadcast channel creation + spawn_watcher call + FOLIOM_DEBOUNCE_MS env var
- `crates/cli/src/cmd/serve/routes/mod.rs` — /api/watch/events route registered
- `Cargo.toml` — tokio sync + axum tokio features
- `crates/cli/Cargo.toml` — notify 6.1.1 + notify-debouncer-full 0.3.2 + tokio-stream 0.1 + dashmap

## Decisions Made

- **Local suppressed-hash cache:** inotify emits multiple events per atomic rename (MOVED_TO + MODIFY + CLOSE_WRITE). `take_if_present` consumes the SelfWriteSet entry on the first event; subsequent events for the same path bypass suppression. Fixed by maintaining a `DashMap<[u8;32], Instant>` with 600ms TTL in the event loop — after `take_if_present` succeeds, the hash is cached locally to absorb follow-up events.

- **spawn_watcher_injectable:** The `rescan_triggers_index_reset` test cannot rely on real OS events for `Flag::Rescan` (platform-specific, timing-sensitive). Exposing a test-only constructor that returns the raw mpsc sender lets the test inject synthetic `DebounceEventResult` values directly into the event loop — deterministic and fast.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added local suppressed-hash cache for multi-event inotify dedup**
- **Found during:** Task 1 (own_write_not_echoed test failure)
- **Issue:** `SelfWriteSet::take_if_present` consumes the hash entry on first call. inotify emits multiple events per atomic rename. The second event bypasses suppression and emits a spurious `PagesUpdated`.
- **Fix:** Added `DashMap<[u8; 32], Instant>` with 600ms TTL in the watcher event loop. After `take_if_present` succeeds, hash is inserted into the local cache; subsequent events check the local cache first.
- **Files modified:** `crates/cli/src/cmd/serve/watcher.rs`
- **Verification:** `own_write_not_echoed` test passes; all 4 integration tests green.
- **Committed in:** `c1239dc` (Task 1 feat commit)

**2. [Rule 3 - Blocking] Added watcher_tx to 4 existing test AppState constructions**
- **Found during:** Task 1 — full workspace test suite broke when watcher_tx field was added to AppState
- **Issue:** `blocks_api.rs`, `rename_api.rs`, `autocomplete_api.rs`, `portability_acpt_05.rs` all construct `AppState` directly and were missing the new required field.
- **Fix:** Added `tokio::sync::broadcast::channel(64)` dummy channel + `watcher_tx: Arc::new(watcher_tx)` to each test's `build_state` helper.
- **Files modified:** 4 test files listed above
- **Verification:** All workspace tests pass.
- **Committed in:** `c1239dc` (Task 1 feat commit)

**3. [Rule 3 - Blocking] Added dashmap to crates/cli/Cargo.toml**
- **Found during:** Task 1 — `watcher.rs` uses `dashmap::DashMap` which was a workspace dep but not declared in crates/cli
- **Fix:** Added `dashmap = { workspace = true }` to crates/cli/Cargo.toml
- **Files modified:** `crates/cli/Cargo.toml`
- **Verification:** `cargo check -p foliom-cli` passes.
- **Committed in:** `c1239dc`

---

**Total deviations:** 3 auto-fixed (1 missing critical, 2 blocking)
**Impact on plan:** All auto-fixes necessary for correctness or compilability. No scope creep. The local hash cache is a correctness requirement for self-write suppression on any platform with multi-event FS notifications.

## Issues Encountered

- inotify emits multiple events per atomic rename on Linux (MOVED_TO + MODIFY + CLOSE_WRITE). `take_if_present` is a consume-once API, so a per-event design needs a local cache to handle the duplicate events. Resolved with the suppressed-hash DashMap.

## Next Phase Readiness

- Backend SSE endpoint live at `GET /api/watch/events`
- `WatcherEvent` enum available for frontend consumption
- Phase 04-02 can start: frontend SSE subscription + `watcherStatus` store + conflict banner wiring
- The `AppState.watcher_tx` is correctly wired — frontend just needs `EventSource('/api/watch/events')`

---
*Phase: 04-disk-sync*
*Completed: 2026-05-22*
