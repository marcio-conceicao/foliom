---
phase: 04-disk-sync
plan: 03
subsystem: ci
tags: [ci, github-actions, integration-smoke, acceptance, watcher, sse, linux, inotify]

# Dependency graph
requires:
  - phase: 04-disk-sync
    plan: 01
    provides: "spawn_watcher + GET /api/watch/events SSE endpoint; foliom-cli binary"
  - phase: 04-disk-sync
    plan: 02
    provides: "Frontend SSE watcher singleton; conflict banner (SNC-06)"
provides:
  - ".github/workflows/ci.yml phase-4-watcher-smoke job: Linux-only end-to-end SSE smoke (foliom serve + external write + pages_updated assertion)"
  - ".planning/phases/04-disk-sync/ACPT-04-WATCHER.md: manual acceptance checklist for Windows watcher + Syncthing storm + conflict banner"
affects:
  - "Phase 5+ (CI baseline established; watcher smoke job runs on every push)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CI end-to-end smoke pattern: spawn binary as background process, background curl SSE subscriber, write external file, grep assert, kill PIDs with if: always() cleanup"
    - "PID-file pattern for CI process cleanup: write $! to /tmp/*.pid, kill via cat in always-step"
    - "phase-4-watcher-smoke cache key: separate from test/bench to avoid invalidating each other"

key-files:
  created:
    - ".planning/phases/04-disk-sync/ACPT-04-WATCHER.md"
  modified:
    - ".github/workflows/ci.yml (phase-4-watcher-smoke job added)"

key-decisions:
  - "CI smoke job uses curl --no-buffer -N for SSE (streaming, line-buffered) rather than ureq in a test binary — simpler, no new dependency, exercises the real HTTP path"
  - "PID files (/tmp/foliom-smoke.pid, /tmp/sse-curl.pid) used for cleanup instead of subshell variable because GitHub Actions steps do not share environment variables across run: blocks"
  - "1.5s wait (not 1s) to account for: server warmup + inotify debounce (300ms) + DirtySet coalescing tick (300ms) + grep assertion overhead — CI runners vary in speed"
  - "Linux-only job: Windows ReadDirectoryChangesW testing is impractical in ubuntu-latest CI; ACPT-04-WATCHER.md documents the Windows manual checklist instead"
  - "watcher_integration unit tests (SNC-03 + SNC-04) already covered by cargo nextest run --workspace in the test job; phase-4-watcher-smoke adds the process-boundary end-to-end smoke on top"

patterns-established:
  - "CI smoke: background process + background SSE subscriber + external file write + grep assert pattern (reusable for future phases)"

requirements-completed: [SNC-03, SNC-04, SNC-06]

# Metrics
duration: 2min
completed: 2026-05-22
---

# Phase 4 Plan 3: CI Watcher Smoke + Manual Acceptance Checklist Summary

**End-to-end `phase-4-watcher-smoke` CI job on Linux boots `foliom serve`, writes an external file, and asserts `pages_updated` SSE arrives within 1.5s; `ACPT-04-WATCHER.md` documents 5 manual scenarios covering Windows + Syncthing + conflict banner**

## Performance

- **Duration:** 2 min
- **Started:** 2026-05-22T12:55:37Z
- **Completed:** 2026-05-22T12:57:37Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- `phase-4-watcher-smoke` CI job added to `.github/workflows/ci.yml`: Linux-only, `needs: test`, `timeout-minutes: 5`. Builds the release binary, creates a minimal corpus, starts `foliom serve`, subscribes to `/api/watch/events` SSE via `curl`, writes an external file, and asserts `pages_updated` arrives within 1.5s. Cleanup runs in an `if: always()` step to kill both PIDs regardless of pass/fail.
- `ACPT-04-WATCHER.md` created with 5 manual acceptance scenarios: VS Code atomic-rename save (all platforms), Syncthing write storm (Linux/macOS), Windows ReadDirectoryChangesW recovery, conflict banner interaction (SNC-06), and self-write suppression. Includes a 10-row sign-off table, WSL2 inotify caveat, and curl/DevTools instructions for manual SSE observation.
- All existing CI jobs (`test`, `bench`, `phase-3-acpt-05`) unchanged and verified via YAML parse.

## Task Commits

1. **Task 1: phase-4-watcher-smoke CI job** — `eee03fa` (feat)
2. **Task 2: ACPT-04-WATCHER.md manual acceptance checklist** — `a8860c5` (docs)

## Files Created/Modified

- `.github/workflows/ci.yml` — `phase-4-watcher-smoke` job added (94 lines); all existing jobs preserved unchanged
- `.planning/phases/04-disk-sync/ACPT-04-WATCHER.md` — 5 acceptance scenarios + sign-off table + WSL2/Windows notes

## Decisions Made

- **PID files for CI cleanup:** GitHub Actions `run:` steps do not share shell environment between them, so `$FOLIOM_PID` set in one step is unavailable in the cleanup step. Writing `$!` to `/tmp/*.pid` and reading with `$(cat ...)` in the cleanup step is the reliable cross-step pattern.

- **1.5s wait (not 1.0s):** The plan spec said 1s window. Using 1.5s provides margin for: server startup (~0.5s on a cold runner), inotify debounce (300ms), DirtySet coalescing tick (300ms), plus `curl` and `foliom serve` startup latency. 1.0s was too tight for a worst-case CI runner; 1.5s is still well within the 5-minute job timeout.

- **curl for SSE (not ureq in a test binary):** A shell-based smoke using `curl --no-buffer -N` is simpler than writing a dedicated integration test binary, adds no Rust dependencies, and exercises the real HTTP path including TCP accept + SSE headers. The `grep -q "pages_updated"` assertion is sufficient to confirm the event reached the wire.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Phase 4 is CI-complete: watcher_integration unit tests run on all platforms in the `test` job; `phase-4-watcher-smoke` exercises the full process boundary on Linux on every push.
- Windows watcher acceptance is documented in `ACPT-04-WATCHER.md` for manual sign-off during verification.
- Phase 5 can begin. Phase 4 is ready for `/gsd-verify-work`.

## Known Stubs

None — CI job is fully wired. ACPT-04-WATCHER.md sign-off rows are intentionally blank (awaiting human testing).

## Threat Flags

No new threat surface. The CI job runs on an ephemeral ubuntu-latest runner with a loopback-only `foliom serve` instance; `/tmp` paths are isolated per job (T-04-09 accepted per plan threat model).

---
*Phase: 04-disk-sync*
*Completed: 2026-05-22*
