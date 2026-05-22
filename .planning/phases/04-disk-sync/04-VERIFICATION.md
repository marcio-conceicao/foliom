---
phase: 04-disk-sync
verified: 2026-05-22T13:06:07Z
status: human_needed
score: 4/4 must-haves verified
overrides_applied: 0
human_verification:
  - test: "ACPT-04-WATCHER.md — Windows ReadDirectoryChangesW re-arm"
    expected: "After a large git pull that overflows the 16 KB RDC buffer, foliom continue watching without a restart; server log shows re-armed watcher; subsequent single-file edits trigger pages_updated normally"
    why_human: "Windows-native test not executable on Linux CI or WSL2; requires Windows 11 runner and notify 6.1 Windows backend"
  - test: "ACPT-04-WATCHER.md — VS Code atomic-rename save (all platforms)"
    expected: "VS Code Cmd+S causes page to reload in Foliom browser tab within ~1s; no conflict banner; no duplicate SSE events"
    why_human: "Requires a real VS Code process performing an atomic rename; cannot be reproduced with plain echo > file"
  - test: "ACPT-04-WATCHER.md — Syncthing storm simulation"
    expected: "Scripted concurrent 100+ file writes produce 1-2 SSE events, not 100; no UI freeze; no runaway reindex log entries"
    why_human: "Race-condition-sensitive; requires observing DevTools Network tab + server logs simultaneously"
  - test: "ACPT-04-WATCHER.md — Conflict banner (SNC-06) interaction"
    expected: "Banner appears within ~1s; user can continue typing; Reload restores external content"
    why_human: "UI interaction flow; cannot grep for user-visible banner appearance"
  - test: "ACPT-04-WATCHER.md — Self-write suppression end-to-end"
    expected: "Saving a block in the editor does NOT trigger the conflict banner; watcher log shows suppressed hash"
    why_human: "Full stack user interaction: click block, type, blur, verify no banner"
---

# Phase 4: Disk Sync — Verification Report

**Phase Goal:** External edits (VS Code save, `git pull`, Syncthing storm) are detected, debounced, deduplicated against Foliom's own writes, and pushed to the UI live via SSE — and when an external edit collides with a foreground edit, the user gets a clear conflict choice.
**Verified:** 2026-05-22T13:06:07Z
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | FS watcher detects external changes within ~250-500ms debounce, survives atomic-rename saves, refreshes index+UI via SSE with zero feedback loop from own writes | ✓ VERIFIED | `spawn_watcher_with_channel` in `watcher.rs:332`; `SelfWriteSet::take_if_present` called at line 242; `COALESCE_WINDOW = 300ms`; 4 integration tests green |
| 2 | Recursive watch at parent-directory level; Windows RDC overflow and macOS MustScanSubDirs trigger rescan fallback without dropping events | ✓ VERIFIED | `RecursiveMode::Recursive` at `watcher.rs:350`; `e.event.need_rescan()` at line 192; `Err(errors)` branch at line 270 triggers `IndexReset + full reindex`; note: explicit re-arm call absent (see WARNING below) |
| 3 | Syncthing-style bulk-change burst processed without UI freeze, lost events, or runaway reindex; only touched files reparsed | ✓ VERIFIED | `DirtySet: DashMap<PathBuf, ()>` at `watcher.rs:181`; coalescing drain at lines 287-300; `bulk_coalesced` test asserts ≤3 events for 10 files in 100ms — passes |
| 4 | Conflict UI shown when external edit and in-flight foreground edit collide; user sees banner with foreground-edit-wins default + one-click reload | ✓ VERIFIED | `externalConflict` store in `frontend/src/lib/stores/watcher.ts:25`; `$effect` in `PageView.svelte:29-38` sets `staleConflict = true` when `$currentlyEditing !== null`; banner at line 235; reload button at line 238 |

**Score:** 4/4 ROADMAP success criteria verified

---

### SNC-03 Detail Verification

| Check | File | Line | Status |
|-------|------|------|--------|
| `spawn_watcher` function exists | `crates/cli/src/cmd/serve/watcher.rs` | 318 | ✓ VERIFIED |
| `SelfWriteSet::take_if_present` called in pipeline | `watcher.rs` | 242 | ✓ VERIFIED |
| `DirtySet` (`DashMap<PathBuf,()>`) coalescing present | `watcher.rs` | 181, 287-300 | ✓ VERIFIED |
| `GET /api/watch/events` SSE route registered | `routes/mod.rs` | 53 | ✓ VERIFIED |
| `watcher_tx` in `AppState` | `state.rs` | 46 | ✓ VERIFIED |
| `spawn_watcher` called in `mod.rs` | `mod.rs` | 126 | ✓ VERIFIED |
| `FOLIOM_DEBOUNCE_MS` env override | `mod.rs` | 121-124 | ✓ VERIFIED |

### SNC-04 Detail Verification

| Check | File | Line | Status |
|-------|------|------|--------|
| `RecursiveMode::Recursive` watch registered | `watcher.rs` | 350 | ✓ VERIFIED |
| `debouncer.cache().add_root()` for rename tracking | `watcher.rs` | 354 | ✓ VERIFIED |
| `event.need_rescan()` handled → full reindex + `IndexReset` | `watcher.rs` | 192-199 | ✓ VERIFIED |
| Windows `Err` branch → full reindex + `IndexReset` | `watcher.rs` | 270-284 | ✓ VERIFIED |
| Windows `Err` branch explicit `watch()` re-arm call | `watcher.rs` | — | ⚠️ WARNING (see below) |
| `rescan_triggers_index_reset` integration test | `tests/watcher_integration.rs` | 182 | ✓ VERIFIED |

### SNC-06 Detail Verification

| Check | File | Line | Status |
|-------|------|------|--------|
| `externalConflict` store exported | `frontend/src/lib/stores/watcher.ts` | 25 | ✓ VERIFIED |
| `watcherStatus` store exported | `frontend/src/lib/stores/watcher.ts` | 18 | ✓ VERIFIED |
| `PageView.svelte` `$effect` reacts to `externalConflict` | `PageView.svelte` | 29-38 | ✓ VERIFIED |
| `staleConflict = true` when editing block | `PageView.svelte` | 33 | ✓ VERIFIED |
| Silent `reload()` when not editing | `PageView.svelte` | 35 | ✓ VERIFIED |
| `externalConflict.set(null)` consumed after handling | `PageView.svelte` | 37 | ✓ VERIFIED |
| Watcher-status pill in Sidebar | `Sidebar.svelte` | 146-147 | ✓ VERIFIED |
| CSS-only animation for `reconnecting` state | `Sidebar.svelte` | 257-261 | ✓ VERIFIED |
| `startWatcher()` called in `App.svelte` `onMount` | `App.svelte` | 57-64 | ✓ VERIFIED |
| `stopWatcher()` on `beforeunload` | `App.svelte` | 59 | ✓ VERIFIED |
| Singleton guard (no duplicate EventSource) | `watcher.ts` | 111-114 | ✓ VERIFIED |

---

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/cli/src/cmd/serve/watcher.rs` | ✓ VERIFIED | 382 lines; `spawn_watcher`, `spawn_watcher_with_channel`, `spawn_watcher_injectable`, `run_event_loop`; DirtySet coalescing + suppressed-hash cache |
| `crates/cli/src/cmd/serve/routes/watch.rs` | ✓ VERIFIED | 164 lines; `watch_events_handler`, `sse_event_from_result`; 30s keep-alive; 3 unit tests inline |
| `crates/cli/tests/watcher_integration.rs` | ✓ VERIFIED | 241 lines; 4 test functions: `external_write_detected`, `own_write_not_echoed`, `bulk_coalesced`, `rescan_triggers_index_reset` |
| `frontend/src/lib/stores/watcher.ts` | ✓ VERIFIED | `watcherStatus` + `externalConflict` writable stores |
| `frontend/src/lib/watcher.ts` | ✓ VERIFIED | `startWatcher` / `stopWatcher` singleton; all event handlers present |
| `frontend/src/__tests__/watcher.test.ts` | ✓ VERIFIED | 10 tests covering all 7 plan-specified behaviors + 3 additional edge cases |
| `.github/workflows/ci.yml` | ✓ VERIFIED | `phase-4-watcher-smoke` job at line 247; `needs: [test]`; `runs-on: ubuntu-latest` |
| `.planning/phases/04-disk-sync/ACPT-04-WATCHER.md` | ✓ VERIFIED | 5 acceptance scenarios; sign-off table; WSL2 caveat |

---

### Key Link Verification

| From | To | Via | Status |
|------|----|-----|--------|
| `watcher.rs` | `foliom_core::sync::SelfWriteSet` | `self_writes.take_if_present(&hash)` | ✓ WIRED — line 242 |
| `routes/watch.rs` | `state.rs` | `state.watcher_tx.subscribe()` | ✓ WIRED — `rx = state.watcher_tx.subscribe()` at line 75 |
| `mod.rs` | `watcher.rs` | `spawn_watcher(root, self_writes, watcher_tx, db_arc, debounce_ms)` | ✓ WIRED — lines 126-133 |
| `App.svelte` | `watcher.ts` | `onMount(() => startWatcher())` | ✓ WIRED — line 57-64 |
| `watcher.ts` | `stores/watcher.ts` | `watcherStatus.set('connected' \| 'reconnecting')` | ✓ WIRED — lines 51, 57 |
| `watcher.ts` | `PageView.svelte` | `externalConflict` store | ✓ WIRED — store subscription via `$effect` in PageView |
| `PageView.svelte` | `stores/watcher.ts` | `externalConflict` import + `$externalConflict` rune | ✓ WIRED — lines 11, 30 |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `watch_events_handler` | `BroadcastStream<WatcherEvent>` | `watcher_tx.subscribe()` → watcher thread | Real reindex results from `reindex_dirty_files_blocking` | ✓ FLOWING |
| `PageView.svelte` `$externalConflict` | `externalConflict` store | `watcher.ts handlePagesUpdated` → `externalConflict.set(...)` | Real SSE `pages_updated` data parsed from backend | ✓ FLOWING |
| `Sidebar.svelte` `$watcherStatus` | `watcherStatus` store | `watcher.ts handleOpen/handleError` | Real EventSource lifecycle events | ✓ FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `cargo test --workspace` passes | `cargo test --workspace` | 4/4 watcher_integration + all other tests green | ✓ PASS |
| Frontend 177 tests pass | `cd frontend && npx vitest run` | 177/177 passed (23 test files) | ✓ PASS |
| 10 watcher.test.ts tests pass | included above | 10/10 in `watcher.test.ts` | ✓ PASS |
| CI YAML syntactically valid | Python yaml parse | `phase-4-watcher-smoke` job present at line 247 | ✓ PASS |

---

### Requirements Coverage

| Requirement | Plan | Description | Status | Evidence |
|-------------|------|-------------|--------|----------|
| SNC-03 | 04-01 | FS watcher detects external edits, reindexes, SSE push to clients | ✓ SATISFIED | `spawn_watcher` + `GET /api/watch/events` + 4 integration tests |
| SNC-04 | 04-01 | Directory-level recursive watch + rescan fallback on overflow | ✓ SATISFIED | `RecursiveMode::Recursive` + `need_rescan()` + `Err` branch handling |
| SNC-06 | 04-02 | Conflict banner when external edit collides with foreground edit | ✓ SATISFIED | `externalConflict` store → `staleConflict = true` in PageView |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `watcher.rs` | 281-283 | Comment: "re-arm is handled by the debouncer holding the watcher alive" with NO explicit `debouncer.watcher().watch()` call | ⚠️ WARNING | Research Pitfall 5 states that `notify/src/windows.rs` calls `unwatch()` after unrecognized errors; the debouncer guard keeps the struct alive but the underlying watch registration may be dropped. The Err branch does NOT call `debouncer.watcher().watch(&root, RecursiveMode::Recursive)` as specified in the plan and research. This is only observable on Windows native — not testable on Linux CI. |

**No `TBD`, `FIXME`, or `XXX` markers found in phase-modified files.**

---

### Human Verification Required

The following items are in ACPT-04-WATCHER.md and require a human tester on the appropriate platform. All automated checks pass.

#### 1. Windows watcher re-arm after ReadDirectoryChangesW error

**Test:** Run `foliom serve` on Windows 11 native. Perform a `git checkout` that modifies 500+ `.md` files simultaneously. Observe server logs.
**Expected:** Server log shows "watcher: debouncer error — potential missed events" followed by "full reindex triggered". Browser tab receives `index_reset` event. Subsequent single-file edits (after the storm) still trigger `pages_updated` — confirming the watcher is still active.
**Why human:** Requires Windows 11 native with `notify`'s Windows backend (`ReadDirectoryChangesW`). Cannot be tested on Linux CI or WSL2. The code comment defers re-arming to the debouncer guard keeping the watcher alive — this assumption needs human confirmation on Windows.

#### 2. VS Code atomic-rename save (all platforms)

**Test:** Open any `.md` file in VS Code. Edit one line. Save with Cmd+S / Ctrl+S.
**Expected:** Foliom browser tab reloads the page within ~1s. No "File changed externally" banner appears (no block was in edit). No duplicate SSE events in DevTools Network.
**Why human:** VS Code performs a temp-file → atomic rename sequence. The `RecommendedCache` in notify-debouncer-full is the rename-tracking mechanism. Cannot be replicated with `echo >` writes in CI.

#### 3. Syncthing-style storm simulation

**Test:** Open browser DevTools Network filtered to `/api/watch/events`. Run: `for i in $(seq 1 10); do echo "- Sync $i" > notes/pages/sync_$i.md; done`
**Expected:** 1–2 `pages_updated` SSE events arrive in DevTools Network within 1s — not 10. No UI freeze. No runaway reindex log entries.
**Why human:** Requires observing DevTools Network tab and server logs simultaneously while scripts run.

#### 4. Conflict banner interaction (SNC-06)

**Test:** Open a page in Foliom. Click a block to enter CM6 edit mode. Without blurring, write the same `.md` file externally (VS Code or `echo`).
**Expected:** A non-blocking banner appears above the editor: "File changed externally. Reload". User can continue typing. Clicking Reload closes editor and reloads with external content.
**Why human:** Requires clicking UI elements in sequence; tests both the appearance timing and that the editor remains functional during conflict.

#### 5. Self-write suppression end-to-end

**Test:** Open a page in Foliom. Click a block, type text, blur (or press Enter to save).
**Expected:** No "File changed externally" banner appears after Foliom saves. Watcher server log shows hash suppressed.
**Why human:** Requires a full round-trip: CM6 editor → `PUT /api/blocks` → `atomic_write_md` → watcher event. The `own_write_not_echoed` integration test covers the hash path but not the full HTTP → file → watcher loop.

---

### Notes on Windows Re-arm Warning

**Context:** Research Pitfall 5 explicitly states that `notify/src/windows.rs` calls `unwatch()` on the affected directory after an unrecognized error, making the watcher "blind" to that directory. The prescribed fix is to explicitly call `debouncer.watcher().watch(&root, RecursiveMode::Recursive)` in the Err branch.

**What the code does instead:** The Err branch (line 270-284) triggers full reindex + IndexReset but relies on the `_debouncer_guard` (the debouncer struct moved into the thread) to keep the watcher alive. This keeps the Rust struct alive but does not re-register the watch path if `notify` internally unwatched it.

**Classification:** WARNING, not BLOCKER. Reasons:
1. This affects Windows native only — Linux CI and the `watcher_integration` tests cover the Err path on Linux where no unwatch-on-error occurs.
2. The ACPT-04-WATCHER.md scenario 3 explicitly covers this manual test case for human sign-off.
3. The ROADMAP success criterion 2 says "trigger a rescan fallback without dropping events" — on Linux/macOS this is met; Windows correctness is deferred to manual acceptance per the existing WSL2 caveat in project memory.
4. If this were confirmed broken on Windows, it would require a one-line fix (add the `.watch()` call in the Err branch) — low complexity, well-understood location.

---

## Summary

All 4 ROADMAP success criteria are VERIFIED in the codebase. All 3 requirements (SNC-03, SNC-04, SNC-06) have substantive, wired, and data-flowing implementations. The `cargo test --workspace` suite is fully green (4 new watcher integration tests + all pre-existing tests). The frontend Vitest suite passes 177/177 tests including 10 new watcher tests.

The `human_needed` status is due to 5 acceptance scenarios in ACPT-04-WATCHER.md that require a human tester on the target platform — principally the Windows watcher re-arm behavior and the VS Code atomic-rename flow. One WARNING exists (Windows Err branch re-arm — code keeps debouncer struct alive rather than explicitly calling `.watch()` again) which should be confirmed during Windows acceptance testing.

---

_Verified: 2026-05-22T13:06:07Z_
_Verifier: Claude (gsd-verifier)_
