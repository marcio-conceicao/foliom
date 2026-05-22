---
phase: 04-disk-sync
phase_number: 4
created: 2026-05-22
mode: auto (--auto flag, all defaults selected)
---

# Phase 4 — Disk Sync: Context

**Goal (ROADMAP):** External edits (VS Code save, `git pull`, Syncthing storm) are detected, debounced, deduplicated against Foliom's own writes, and pushed to the UI live via SSE — and when an external edit collides with a foreground edit, the user gets a clear conflict choice.

**Requirements (3):** SNC-03, SNC-04, SNC-06

**Depends on:** Phase 3 (complete — SelfWriteSet shipped in plan 03-01, wired to mutation API in 03-03).

---

## Pre-locked Decisions (from research + prior phases)

| Area | Decision | Source |
|---|---|---|
| Watcher library | `notify 6.1 + notify-debouncer-full 0.3` with `RecommendedCache` (tracks file IDs across renames) | research/STACK.md, PRD §5.5 |
| Debounce mechanism | `notify-debouncer-full` built-in; per-path window | SNC-03, research PITFALLS #2 |
| Self-write suppression | `SelfWriteSet` from crates/core/src/sync/self_writes.rs (Phase 3 plan 03-01); registered by mutation API (03-03) | SNC-02, 03-01-SUMMARY |
| Watch level | Parent-directory recursive (NOT per-file) to avoid inotify exhaustion | SNC-04, research PITFALLS #2 |
| Rescan fallback triggers | Windows `ReadDirectoryChangesW` overflow + macOS `MustScanSubDirs` event | SNC-04 |
| SSE for live push | axum SSE already in workspace (`tower-http` + `axum::response::sse`); same pattern as Phase 2 health | architecture research |

---

## Decisions Locked in This Discussion (auto-selected)

### D-40-01: Debounce window — 300ms

`RecommendedDebouncer` configured with 300ms per-path window. Rationale: midpoint of the 250–500ms research range. Absorbs VS Code atomic rename (write-tmp → rename-over, ~50–150ms gap), Syncthing multi-file bursts, and Obsidian save-thrash. Configurable via a `FOLIOM_DEBOUNCE_MS` env var in the serve command for power users.

### D-40-02: SSE channel — single global broadcast

One `tokio::sync::broadcast::Sender<WatcherEvent>` held in `AppState`. Each SSE client (`GET /api/watch/events`) calls `broadcast_tx.subscribe()` to get a `Receiver`. On watcher callback, publish to broadcast. Browser uses a single `EventSource` instance per tab.

Event types emitted over SSE:
- `pages_updated` — `data: { pages: [{ name, fileHash }] }` (batch of changed page names + new hash for conflict detection)
- `page_deleted` — `data: { name }` (file removed from corpus)
- `index_reset` — `data: {}` (full reindex completed; frontend should refetch current page)

Keep-alive: send `event: ping` every 30s to prevent proxy timeouts.

### D-40-03: Bulk-change coalescing — batch + single reindex transaction

When a Syncthing storm hits (hundreds of dirty files), the debouncer emits one event per file after its 300ms window. The watcher pipeline:
1. Accept events from `notify-debouncer-full` in a background tokio task.
2. Coalesce into a `DirtySet` (DashMap<file_id, ()>).
3. Drain the dirty set on a 300ms coalescing tick (separate from per-path debounce) — reindex all dirty files in a single SQLite transaction.
4. Emit one `pages_updated` SSE event with the union of changed pages.

This prevents: per-file reindex transactions (300 sequential WAL commits on a Syncthing storm), UI flooding (1 SSE event vs 300).

### D-40-04: Conflict UI — non-blocking banner (foreground wins)

When `WatcherEvent.pages_updated` arrives for a page that has a block currently being edited (`currentlyEditing` store):
- Store the conflict internally: `conflictStore.set({ pageHash: newHash })`.
- Display a non-blocking banner **above** the CM6 editor: "File changed externally. [↺ Reload discards your edit]".
- Foreground edit wins by default — the user can keep typing.
- Clicking "↺ Reload" calls `GET /api/pages/:name`, replaces block tree, clears editor.
- On blur/Enter save: the `PUT /api/blocks/:id` call includes `prevHash` from the old page state → server returns 409 Stale → frontend receives 409 and shows the same banner (already in place from 03-04).

This connects Phase 3's `StaleConflict` banner to the watcher path — same component, two triggers.

### D-40-05: SSE reconnect — browser native + watcher status indicator

`EventSource` auto-reconnects on drop (browser default, ~3s). Frontend keeps a `watcherStatus: 'connected' | 'reconnecting' | 'offline'` Svelte store:
- `open` event → `'connected'`
- `error` event → `'reconnecting'`
- After 10s without reconnect → `'offline'`

`Sidebar.svelte` shows a subtle pill indicator (green dot / spinning dot / grey dot) in the bottom-left corner. No modal, no interruption.

---

## Scope Guardrails

**In scope:** SNC-03 (watcher → reindex → SSE push), SNC-04 (dir-level watch + rescan fallback), SNC-06 (conflict banner). Exactly 3 REQ-IDs.

**Out of scope:**
- SNC-02 (self-write set) → already in Phase 3.
- Sync protocol (Syncthing/git themselves) → delegated to external tools, never in scope.
- Conflict merge (three-way merge) → too complex for v1; foreground-wins + reload is the entire policy.
- Code-signing cert procurement for Phase 5 → noted: start admin process **during Phase 4** (research flagged weeks of lead time before Phase 5).

---

## Pre-existing Assets to Reuse

- `crates/core/src/sync/self_writes.rs` — `SelfWriteSet::contains(&hash)` for watcher deduplification.
- `crates/core/src/sync/atomic.rs` — `atomic_write_md` already called by mutation API; watcher reads files but doesn't write, so no overlap.
- `crates/core/src/indexer.rs` + `Db::open` — `reindex(mode, root, db)` for incremental reindex per-file; already tested at scale.
- `crates/cli/src/cmd/serve/state.rs` — `AppState`: add `watcher_tx: Arc<broadcast::Sender<WatcherEvent>>` alongside existing `db` + `self_writes`.
- Phase 2 `axum::response::Sse` pattern (already in scope via tower-http).
- Phase 3 `StaleConflict` banner in `frontend/src/lib/pages/PageView.svelte` — reuse for watcher-triggered conflict.

---

## Open Questions for Research

1. `notify-debouncer-full` API: does the Rust `RecommendedDebouncer::new()` constructor take a `Duration` for the debounce window? Confirm with current docs (notify 6.1 + debouncer-full 0.3).
2. `tokio::sync::broadcast` back-pressure: what happens when a slow SSE client accumulates messages? `RecvError::Lagged` — how to handle gracefully (skip + send `index_reset`)?
3. Linux inotify watch limit: the default is ~8192 watches per process. With 5k+ notes, the recursive-directory watch should be O(subdirectory-count), not O(file-count). Confirm this is the case for notify's `RecursiveMode::Recursive` implementation.
4. macOS `MustScanSubDirs` rescan: what does this look like in `notify` event types? Is it surfaced as a distinct event variant or must we detect it heuristically?
5. Windows `ReadDirectoryChangesW` buffer overflow: how does notify surface this? Is it an `Error` variant or a synthetic event? What's the recovery path in debouncer-full?

---

## Next Step

`/gsd-plan-phase 4` — spawn researcher (answers the 5 questions above) then planner (2-3 plans expected given only 3 REQs).
