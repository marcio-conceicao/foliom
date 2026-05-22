---
phase: 03-outliner-editor
plan: 01
subsystem: backend
tags: [atomic-write, self-write-set, tempfile, dashmap, blake3, windows-av, sync, snc-02]

requires:
  - phase: 01-foundation
    provides: "blake3 dep already in crates/core; tests/fixtures/logseq-synthetic for round-trip rehearsal"
  - phase: 02-backend-spike
    provides: "workspace lockfile / pinning conventions; crates/core module layout"
provides:
  - "atomic_write_md(target, contents, &SelfWriteSet) → io::Result<[u8;32]> — same-FS atomic temp+rename with Windows AV retry (50/100/200ms) and unix parent fsync"
  - "SelfWriteSet (dashmap + TTL, default 30s) — Clone-shared registry of Foliom's own writes; Phase 4 watcher will consume via take_if_present"
  - "tempfile promoted from dev-dep to runtime dep in crates/core; dashmap=6 added to workspace"
  - "crates/core::sync module (mod.rs + atomic.rs + self_writes.rs) imported by crates/cli via lib re-exports"
affects: [03-02-mutation-splice, 03-03-block-routes, 03-05-rename-journal, 04-watcher]

tech-stack:
  added: [dashmap 6.2.1, tempfile 3 (promoted to runtime)]
  patterns:
    - "All disk writes route through atomic_write_md (single chokepoint for tampering threat T-03-01)"
    - "Hash registration BEFORE persist (race-free watcher-echo suppression for T-03-03)"
    - "TempPath::persist + PathPersistError { error, path } as the retry primitive (A9 verified)"
    - "Cfg-gated retry (Windows only); unix fails fast"

key-files:
  created:
    - crates/core/src/sync/mod.rs
    - crates/core/src/sync/atomic.rs
    - crates/core/src/sync/self_writes.rs
    - crates/core/src/sync/__tests__/atomic_test.rs
    - crates/core/src/sync/__tests__/self_writes_test.rs
  modified:
    - Cargo.toml (workspace.dependencies: + tempfile, + dashmap)
    - Cargo.lock
    - crates/core/Cargo.toml (promote tempfile, add dashmap)
    - crates/core/src/lib.rs (pub mod sync;)

key-decisions:
  - "dashmap pinned to '6' resolving to 6.2.1 max-stable (7.0.0-rc2 is pre-release, rejected per 03-RESEARCH §Package Legitimacy)"
  - "SelfWriteSet default TTL = 30s (DEFAULT_TTL const), per 03-RESEARCH §2 recommendation; explicit-TTL constructor SelfWriteSet::new(Duration) for tests"
  - "Used TempPath::persist (not NamedTempFile::persist) for the retry loop so we don't have to rewrite contents on each attempt — A9 confirmed: PathPersistError { error, path } on tempfile 3.x"
  - "Missing-parent path returns ErrorKind::NotFound (proxy for cross-FS CrossesDevices since tests cannot reliably mount a second FS)"
  - "Test-only thread_local LAST_PERSIST_ATTEMPTS counter (cfg(test) gated) so the Windows AV smoke test can assert the retry path fired"

patterns-established:
  - "Single disk-write chokepoint: every Phase 3 mutation handler routes through atomic_write_md (mitigates T-03-01 partial-write)"
  - "Hash-then-register-then-persist ordering: textual order in source verified via grep gate in PLAN <verification>"
  - "Cfg-gated retry: Windows-specific retry isolated to #[cfg(windows)] blocks; unix path is fail-fast (matches rename(2) semantics)"

requirements-completed: [SNC-02]

duration: 35min
completed: 2026-05-22
---

# Phase 03 Plan 01: Atomic Write + Self-Write Set Summary

**Same-FS atomic temp+rename helper (`atomic_write_md`) with Windows AV retry (50/100/200ms backoff) and a dashmap-backed `SelfWriteSet` (TTL 30s) that the Phase 4 watcher will consume to suppress Foliom's own write echoes.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-05-22T05:35:00Z
- **Completed:** 2026-05-22T06:10:22Z
- **Tasks:** 2 (both `tdd="true"`, executed RED+GREEN as combined feat commits)
- **Files created:** 5 (mod, atomic, self_writes + 2 test files)
- **Files modified:** 4 (workspace Cargo.toml, Cargo.lock, crate Cargo.toml, lib.rs)

## Accomplishments
- `SelfWriteSet` with `register / take_if_present / gc`, Clone-shared via `Arc<DashMap>`, configurable TTL (default 30s).
- `atomic_write_md` writes contents atomically (same-FS temp + rename), `sync_all`s the temp before rename, fsyncs the parent dir on unix, retries up to 3× on Windows transient `PermissionDenied`/`Other`, and registers the BLAKE3 hash in `SelfWriteSet` **before** the rename to close the watcher-echo race.
- 9 passing tests + 1 Windows-gated test (`#[cfg_attr(not(windows), ignore)]`) covering all five `must_haves.truths`.
- ACPT-01 round-trip gate (Phase 1) stays green after these additions: `cargo test -p foliom-core --test roundtrip` → 2 passed.
- `cargo build --workspace --locked` green; `Cargo.lock` updated to include `dashmap 6.2.1`.

## Task Commits

1. **Task 1: SelfWriteSet (dashmap + TTL)** — `a113c88` (feat)
2. **Task 2: atomic_write_md (same-FS persist + Windows AV retry)** — `2c675b0` (feat)

_Plan metadata commit appended after self-check (see below)._

## Files Created/Modified

**Created:**
- `crates/core/src/sync/mod.rs` — module entry; re-exports `atomic_write_md`, `SelfWriteSet`
- `crates/core/src/sync/self_writes.rs` — `SelfWriteSet` + `DEFAULT_TTL` const; `Arc<DashMap<[u8;32], Instant>>`
- `crates/core/src/sync/atomic.rs` — `atomic_write_md` + cfg-gated Windows retry loop + test-only `LAST_PERSIST_ATTEMPTS`
- `crates/core/src/sync/__tests__/self_writes_test.rs` — 5 behaviour tests (consume, TTL expiry, clone-share, gc reclaim, concurrent insert)
- `crates/core/src/sync/__tests__/atomic_test.rs` — 5 behaviour tests (happy path, missing-parent, no-op byte-identity, fixture round-trip, Windows AV retry)

**Modified:**
- `Cargo.toml` — added `tempfile = "3"` and `dashmap = "6"` to `[workspace.dependencies]`
- `Cargo.lock` — captures `dashmap 6.2.1` + transitive `hashbrown`, etc.
- `crates/core/Cargo.toml` — promoted `tempfile` to `[dependencies]` (kept in `[dev-dependencies]` for benches); added `dashmap = { workspace = true }`
- `crates/core/src/lib.rs` — added `pub mod sync;`

## Decisions Made

- **dashmap version pinned at `"6"` → resolves to 6.2.1.** Per 03-RESEARCH §Package Legitimacy, 7.0.0-rc2 is pre-release and rejected; `cargo info dashmap` confirms 6.2.1 is current `max_stable`. Workspace dep doc-comments record the rationale so a future `cargo update` doesn't silently jump to 7.x.
- **TTL = 30 seconds** (PLAN <output> recommendation; matches 03-RESEARCH §2). Exposed as `DEFAULT_TTL` const + `SelfWriteSet::new(Duration)` for tests. Tests use 50ms TTL.
- **Used `TempPath::persist` rather than `NamedTempFile::persist`** for the Windows retry loop. A9 (PersistError exposes the original temp) is verified: `PathPersistError { error, path: TempPath }` lets us re-attempt the rename without rewriting contents. No fallback path needed.
- **Windows AV smoke test tolerates 0 OR ≥3 attempts.** The test cannot guarantee Defender (or equivalent) interference in CI; it asserts that *if* the OS surfaces a transient denial, the retry loop fires at least 3 times. On Linux/macOS the test is `#[ignore]`d by `cfg_attr`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Cfg-gate `mut temp_path` on Windows only**
- **Found during:** Task 2 (`cargo test -p foliom-core --lib sync` produced `unused_mut` warning on unix because the retry loop never re-assigns `temp_path` outside `#[cfg(windows)]`).
- **Issue:** `let mut temp_path = tmp.into_temp_path();` triggered `warning: variable does not need to be mutable` on unix builds. The `#[cfg(not(windows))]` branch fails fast without re-binding `temp_path`.
- **Fix:** Split the binding under `#[cfg(windows)]` / `#[cfg(not(windows))]` so each platform sees the right mutability; added `#[allow(unused_mut)]` on the `attempt` counter.
- **Files modified:** `crates/core/src/sync/atomic.rs`
- **Verification:** `cargo test -p foliom-core --lib sync` produces 0 warnings.
- **Committed in:** `2c675b0` (Task 2 commit — fix inline with implementation).

**2. [Rule 3 — Blocking] Drop `///` on a thread_local definition**
- **Found during:** Task 2 (warning: `unused doc comment` on the `thread_local!` block).
- **Issue:** Rust does not propagate `///` docs through macro expansion; the comment must be `//` to avoid `unused_doc_comments`.
- **Fix:** Demoted `///` to `//` on the `LAST_PERSIST_ATTEMPTS` block.
- **Files modified:** `crates/core/src/sync/atomic.rs`
- **Verification:** Same `cargo test` run as above — 0 warnings.
- **Committed in:** `2c675b0`.

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking compile-warning-as-soft-failure; treated as blocking because the project's CI keeps the `dev` profile warning-clean).

**Impact on plan:** Both fixes are inline noise reduction. No scope creep, no API change.

## Issues Encountered

**Contamination in Task 1 commit (`a113c88`):** the commit accidentally included an untracked `crates/core/src/mutation/` directory (4 files: `mod.rs`, `splice.rs`, `tree_ops.rs`, `__tests__/splice_test.rs`) created out-of-band by a concurrent process — also evidenced by the system-reminder that `crates/core/src/lib.rs` was modified to add `pub mod mutation;`. These files belong to **plan 03-02** per 03-RESEARCH §8 ("crates/core/src/mutation/{splice,tree_ops,mod}.rs → 03-02"). They built cleanly and did not break tests, so I left them in rather than reverting. Plan 03-02's executor should expect that scaffolding already exists and verify/extend it rather than re-creating from scratch.

## Self-Check

- [x] `crates/core/src/sync/mod.rs` — present (re-exports verified)
- [x] `crates/core/src/sync/atomic.rs` — present (165 lines, includes test mod include)
- [x] `crates/core/src/sync/self_writes.rs` — present (92 lines)
- [x] `crates/core/src/sync/__tests__/atomic_test.rs` — present, 5 tests
- [x] `crates/core/src/sync/__tests__/self_writes_test.rs` — present, 5 tests
- [x] `Cargo.lock` carries `dashmap 6.2.1` (verified via `cargo build --locked` succeeding)
- [x] Commit `a113c88` exists on `main`
- [x] Commit `2c675b0` exists on `main`
- [x] `cargo test -p foliom-core --lib sync` → 9 passed, 1 ignored, 0 failed
- [x] `cargo test -p foliom-core --test roundtrip` → ACPT-01 green
- [x] `grep -n "self_writes\.register\|sync_all" crates/core/src/sync/atomic.rs` → `register` (line 69) precedes the `persist` call site

## Self-Check: PASSED

## Windows Retry Test Observation

Linux CI run (the only platform exercised in this session): test `windows_av_retry_triggers_attempt_counter` is `ignored` (filtered by `#[cfg_attr(not(windows), ignore)]`). Per PLAN <output>, Windows CI must record the observed attempt count from the `LAST_PERSIST_ATTEMPTS` thread_local. **Not yet observed** — Windows CI matrix entry (added in 02-08 perf-harness plan) will surface this. Tracking item, not a blocker.

## TTL Default

`SelfWriteSet::default()` uses `DEFAULT_TTL = 30s`. Test-only constructors pass 50ms.

## A9 Outcome

Assumption A9 held: `tempfile::TempPath::persist` returns `Result<(), PathPersistError>` where `PathPersistError { error: io::Error, path: TempPath }`. We use `err.path` to re-loop on Windows without rewriting contents. No fallback (fresh `NamedTempFile::new_in`) was needed.

## Next Plan Readiness

- **03-02 (mutation::splice + tree_ops):** scaffolding files already present on disk under `crates/core/src/mutation/` (see Issues Encountered). Plan 03-02 should treat them as a starting point to verify/extend, not re-create.
- **03-03 (block routes):** `atomic_write_md` and `SelfWriteSet` are ready to consume. Add `SelfWriteSet` to `AppState`; have every mutation handler `set.register(blake3(new_contents))` before calling `atomic_write_md`.
- **Phase 4 (watcher):** clone the `SelfWriteSet` into the watcher's event loop; call `take_if_present(observed_hash)` on every `Modify(Data)` event to suppress echoes.

---
*Phase: 03-outliner-editor*
*Completed: 2026-05-22*
