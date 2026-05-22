---
phase: 04-disk-sync
plan: 02
subsystem: frontend
tags: [svelte5, eventsource, sse, stores, conflict-banner, watcher-status, vitest, tdd]

# Dependency graph
requires:
  - phase: 04-disk-sync
    plan: 01
    provides: "GET /api/watch/events SSE endpoint; WatcherEvent enum (PagesUpdated/IndexReset/PageDeleted); PageUpdatedInfo camelCase DTO"
  - phase: 03-outliner-editor
    plan: 04
    provides: "staleConflict local state + reload() + StaleConflict banner in PageView.svelte"
provides:
  - "frontend/src/lib/stores/watcher.ts: watcherStatus + externalConflict stores"
  - "frontend/src/lib/watcher.ts: startWatcher/stopWatcher singleton EventSource"
  - "App.svelte: onMount startWatcher + beforeunload cleanup"
  - "PageView.svelte: $effect subscribes externalConflict → staleConflict banner or silent reload"
  - "Sidebar.svelte: watcher-status pill (green/amber-pulse/grey)"
  - "10 Vitest tests: status transitions, conflict path, silent-reload path, singleton guard"
affects:
  - "04-03 (CI integration smoke — watcher frontend now wired)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Module-level EventSource singleton with singleton guard (readyState !== CLOSED check)"
    - "10-second offline timer via setTimeout cleared on reconnect (open event)"
    - "externalConflict store as one-shot signal: watcher sets → PageView consumes + clears null"
    - "CSS-only animation for reconnecting state (watcher-pulse @keyframes, no JS)"
    - "vi.useFakeTimers() + MockEventSource class for deterministic SSE testing"

key-files:
  created:
    - "frontend/src/lib/stores/watcher.ts"
    - "frontend/src/lib/watcher.ts"
    - "frontend/src/__tests__/watcher.test.ts"
  modified:
    - "frontend/src/App.svelte (onMount startWatcher + beforeunload cleanup)"
    - "frontend/src/lib/pages/PageView.svelte ($effect for externalConflict)"
    - "frontend/src/lib/components/Sidebar.svelte (watcher-status pill + CSS)"

key-decisions:
  - "externalConflict one-shot signal pattern: watcher.ts sets the store; PageView.svelte always resets to null after handling — prevents perpetual banner (T-04-06)"
  - "EventSource singleton guard uses readyState !== CLOSED (not null check) so a browser-reconnecting ES (readyState=CONNECTING) is not replaced"
  - "App.svelte onMount for startWatcher (not module-level) so tests can control the EventSource mock before import"

# Metrics
duration: 3min
completed: 2026-05-22
---

# Phase 4 Plan 2: Frontend SSE Watcher Integration Summary

**Singleton EventSource subscribing to /api/watch/events; pages_updated drives conflict banner (SNC-06) or silent reload; Sidebar shows CSS-only watcher-status pill**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-22T12:49:55Z
- **Completed:** 2026-05-22T12:53:01Z
- **Tasks:** 2 (Task 1 TDD RED/GREEN; Task 2 wiring)
- **Files modified:** 6

## Accomplishments

- `stores/watcher.ts` provides `watcherStatus` (`reconnecting`→`connected`→`offline`) and `externalConflict` stores
- `watcher.ts` implements module-level singleton: `startWatcher`/`stopWatcher`; handles `open`/`error` (with 10s offline timer), `pages_updated` (conflict vs silent-reload dispatch), `index_reset` (unconditional `fetchPage`); T-04-05 try/catch wraps JSON.parse
- `App.svelte` calls `startWatcher()` in `onMount` and registers `beforeunload` cleanup — single EventSource per tab guaranteed
- `PageView.svelte` `$effect` reacts to `externalConflict` store: when a block is being edited → sets `staleConflict = true` (surfaces Phase 3 "External edit detected" banner); when idle → calls `reload()` silently. Consumes the store by resetting to null to avoid a perpetual banner
- `Sidebar.svelte` adds a watcher-status pill in the footer: 8px dot, green when connected, amber with `watcher-pulse` CSS animation when reconnecting, grey when offline — no JavaScript animation
- 10 new Vitest tests (full suite 177/177 passing); `npm run build` clean

## Task Commits

1. **RED: failing watcher tests** — `6d20839` (test)
2. **Task 1: stores + watcher singleton** — `9f78698` (feat)
3. **Task 2: App/PageView/Sidebar wiring** — `5695653` (feat)

## Files Created/Modified

- `frontend/src/lib/stores/watcher.ts` — watcherStatus + externalConflict stores
- `frontend/src/lib/watcher.ts` — startWatcher/stopWatcher singleton with full event handling
- `frontend/src/__tests__/watcher.test.ts` — 10 Vitest tests (MockEventSource, vi.useFakeTimers)
- `frontend/src/App.svelte` — onMount startWatcher + beforeunload cleanup
- `frontend/src/lib/pages/PageView.svelte` — $effect for externalConflict → staleConflict or reload
- `frontend/src/lib/components/Sidebar.svelte` — watcher-status pill with CSS-only animation

## Decisions Made

- **externalConflict one-shot signal:** Watcher sets the store; PageView always resets to null after handling. This is the correct pattern for Svelte 5 reactive effects — prevents the `$effect` from re-running perpetually on the same non-null value (T-04-06 mitigation).

- **Singleton guard uses readyState:** `if (es !== null && es.readyState !== EventSource.CLOSED)` catches both CONNECTING and OPEN states, preventing a duplicate connection while the browser is still connecting after a page navigation.

- **App.svelte onMount for startWatcher:** Module-level import of watcher.ts would execute `new EventSource()` before Vitest's `MockEventSource` mock is in place. Deferring to `onMount` means tests can set `globalThis.EventSource = MockEventSource` before the component mounts.

## Deviations from Plan

None — plan executed exactly as written. The backend emits `fileHash` camelCase (confirmed from `dto.rs` `#[serde(rename_all = "camelCase")]`) which aligns with the TypeScript interface in `watcher.ts`.

## Known Stubs

None — all data paths are wired. The `page_deleted` event type is received by EventSource but intentionally has no handler in this plan (noted in source comment as v1 out-of-scope per 04-CONTEXT).

## Threat Flags

No new threat surface beyond what was specified in the plan's threat model.

## Self-Check: PASSED

- [x] `frontend/src/lib/stores/watcher.ts` — exists
- [x] `frontend/src/lib/watcher.ts` — exists
- [x] `frontend/src/__tests__/watcher.test.ts` — exists
- [x] Commits 6d20839, 9f78698, 5695653 — exist in git log
- [x] 177 tests passing
- [x] `npm run build` clean
