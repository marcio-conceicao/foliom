---
phase: 02-read-only-web-ui
plan: 07
subsystem: infra
tags: [phase-2, backend, rust-embed, mime_guess, single-binary, dev-prod-toggle, vite, axum-fallback]

# Dependency graph
requires:
  - phase: 02-read-only-web-ui
    provides: HTTP server scaffold (02-01), REST surface (02-02), Vite SPA build (02-03), per-block renderer (02-04), sidebar/journals/backlinks (02-05), search palette (02-06)
provides:
  - Single-binary distribution model (cargo build --release ships frontend/dist via rust-embed)
  - Debug-mode fallback that 307-redirects to http://localhost:5173/<path> (Vite hot-reload preserved)
  - SPA deep-link fallback in release builds (unknown paths return index.html)
  - mime_guess-driven Content-Type + cache-control policy on embedded assets
affects: [02-08, phase-3, phase-4]

# Tech tracking
tech-stack:
  added:
    - "rust-embed 8.11 (default-features off + compression feature; compile-time gzip on embedded bytes)"
    - "mime_guess 2.x (Content-Type lookup for embedded asset paths)"
  patterns:
    - "Router-level fallback (.fallback(embed::serve_static)) catches every miss past /api/*; /api/* routes are unaffected because Router::fallback only fires when no .route(...) matches."
    - "Debug vs release split via #[cfg(debug_assertions)] inside a single handler — same function signature in both profiles; no feature flag, no second binary."
    - "SPA shell served with Cache-Control: no-cache (index.html); hashed assets served with public, max-age=3600 (Vite-emitted content-hashed names are safe to cache long)."

key-files:
  created:
    - crates/cli/src/cmd/serve/embed.rs
    - crates/cli/tests/serve_prod_embed.rs
  modified:
    - Cargo.toml (workspace deps: rust-embed, mime_guess)
    - Cargo.lock (lock new transitive deps)
    - crates/cli/Cargo.toml (workspace = true refs)
    - crates/cli/src/cmd/serve/mod.rs (pub mod embed)
    - crates/cli/src/cmd/serve/routes/mod.rs (.fallback(embed::serve_static))
    - frontend/README.md (prod build sequence + dev/release behavior table)

key-decisions:
  - "rust-embed 8.11.0 pinned (not 6.x fallback): builds cleanly on edition 2024 / rustc 1.95 — A2 in 02-RESEARCH Assumptions Log is resolved positively."
  - "Picked 307 Temporary Redirect (not 302) for the dev-mode Vite redirect: preserves HTTP method on the redirected request. Cheap insurance against future debug sessions where the SPA route might POST."
  - "SPA fallback in release: missing path -> index.html (not 404). Supports hard reloads on hash-router deep links AND arbitrary unknown URLs (e.g. someone pasting /random/path). Same Cache-Control: no-cache because the served body is the shell."
  - "Empty-dist tolerance verified: cargo check on a fresh clone (only frontend/dist/.gitkeep present) succeeds — rust-embed silently embeds an empty assets directory and prod runtime serves 404 until npm run build runs. Recommended option from 02-RESEARCH §Empty-dist fallback chosen."
  - "Test infrastructure NOT extracted into crates/cli/tests/common/mod.rs: spawn-binary helpers stay inline per-file for parity with serve_boot.rs and serve_routes.rs. Plan allowed either choice; inline matches existing convention."
  - "rust-embed compression feature ENABLED: gzips embedded bytes at compile time, smaller release binary. Decompression cost on each request is negligible for SPA assets (<1MB total) on localhost."

patterns-established:
  - "Single handler, two profiles: future static-asset additions go through embed::serve_static, not parallel routes."
  - "Test files self-skip when build prerequisite is missing (release leg checks for frontend/dist/index.html; emits SKIP line and returns)."

requirements-completed: []

# Metrics
duration: ~18m
completed: 2026-05-22
---

# Phase 02 Plan 07: Embed + Dev/Prod Toggle Summary

**Single-binary distribution: `cargo build --release` produces a `foliom` binary that serves the Svelte SPA at `/` with no `frontend/dist/` required at runtime, while debug builds 307-redirect to `localhost:5173` so the Vite hot-reload dev loop is preserved.**

## Performance

- **Duration:** ~18 min (incl. one full release rebuild from clean)
- **Started:** 2026-05-22T03:50Z
- **Completed:** 2026-05-22T04:07Z
- **Tasks:** 2 (Task 1 fused with Task 2 test file via TDD RED/GREEN)
- **Files modified:** 8 (2 created, 6 modified)

## Accomplishments

- Release binary embeds the Vite-built SPA via rust-embed; manual smoke-tested by running the release binary from `/tmp/foliom-smoke/` with no `frontend/dist/` in cwd — `GET /` returns `text/html` with the 1585-byte SPA shell, `GET /foo/bar/baz` SPA-falls back to the same shell.
- Debug builds preserve the Vite dev loop unchanged: `GET /` 307s to `http://localhost:5173/`; `/assets/index.js` 307s to `http://localhost:5173/assets/index.js`; `/api/*` is unaffected because `Router::fallback` only runs on misses.
- `cargo test -p foliom-cli` (debug, 16 tests): all green, including new `debug_root_redirects_to_vite_5173`.
- `cargo test --release -p foliom-cli --test serve_prod_embed`: green; verifies `<div id="app">` shell at `/`, `application/javascript` Content-Type on `/assets/*.js`, SPA fallback on arbitrary paths, and `/api/health` still 200.
- `cargo build --release -p foliom-cli --locked` builds cleanly (1m 29s from cold, 10s incremental).
- `--open` wiring confirmed in `serve/mod.rs` (already present from plan 02-01); release build invokes `browser::try_open` after the boot banner.

## Task Commits

1. **Task 1 (RED): Failing dev-redirect + prod-embed integration test** — `fb7da2d` (test)
2. **Task 1 (GREEN): Single-binary embed with dev/prod static-asset toggle** — `5e64687` (feat) — also satisfies Task 2 (test file authored as RED for Task 1, both profiles assert in the same file).

**Plan metadata:** _(this commit)_ (docs)

## Files Created/Modified

- `crates/cli/src/cmd/serve/embed.rs` — Shared fallback handler. Debug: 307 to Vite (query string preserved). Release: rust-embed lookup → mime_guess Content-Type → cache-control policy → SPA fallback.
- `crates/cli/tests/serve_prod_embed.rs` — Two cfg-gated tests: debug profile asserts 307 + Location header; release profile asserts embedded shell, JS Content-Type, SPA fallback, `/api/health` unaffected. Release leg self-skips on missing `frontend/dist/index.html`.
- `Cargo.toml` — Workspace pins `rust-embed = { version = "8", default-features = false, features = ["compression"] }` and `mime_guess = "2"`.
- `Cargo.lock` — Locks transitive deps (`include-flate`, `include-flate-codegen`, `compression-codecs`, etc).
- `crates/cli/Cargo.toml` — `workspace = true` refs for the new deps.
- `crates/cli/src/cmd/serve/mod.rs` — `pub mod embed;`.
- `crates/cli/src/cmd/serve/routes/mod.rs` — `use crate::cmd::serve::embed;` + `.fallback(embed::serve_static)` between the last `.route(...)` and `.with_state(...)`.
- `frontend/README.md` — Prod build sequence (`npm ci && npm run build && cargo build --release`) and a comparison table of dev vs release fallback behavior for `/`, `/assets/*`, arbitrary SPA paths, and `/api/*`.

## Decisions Made

- **Pinned `rust-embed` 8.11.0 with the `compression` feature** rather than the bare default — gzip-compresses embedded bytes at compile time, shrinking the release binary. Decompression cost on each request is negligible for localhost-only SPA delivery.
- **307 over 302 for the dev-mode redirect** — preserves the HTTP method, future-proofing the dev loop against POSTs to SPA-served paths.
- **Single shared handler with `#[cfg(debug_assertions)]` split** — avoids the complexity of a `--features embed` cargo feature; identical signature in both profiles; same fallback wired in `routes/mod.rs`.
- **SPA fallback returns index.html, not 404, on any miss** — required for hash-router deep links to survive a hard reload AND for users pasting arbitrary URLs (`/foo/bar/baz` → SPA shell loads → routes to default view).
- **Test files inline their spawn-binary helper** rather than extracting `crates/cli/tests/common/mod.rs` — keeps `serve_prod_embed.rs` parallel to `serve_boot.rs`/`serve_routes.rs`. Plan permitted either; chose the lower-churn path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ureq classifies 307 with `redirects(0)` as Ok, not Err**
- **Found during:** Task 1 (first run of `debug_root_redirects_to_vite_5173`)
- **Issue:** Initial test assumed `ureq::AgentBuilder::new().redirects(0).build()` would return `Err(ureq::Error::Status(307, ..))` for a 307 response. ureq actually treats 3xx as success when redirects are disabled — only 4xx/5xx land in the `Status` error variant. The test panicked with `"expected 307 redirect, got success 307"`.
- **Fix:** Rewrote the test to fold `Ok(resp)` and `Err(ureq::Error::Status(_, resp))` into a single `resp` binding, then assert `resp.status() == 307` and inspect the `Location` header directly. Pattern applied to both the `/` assertion and the `/assets/index.js` assertion.
- **Files modified:** `crates/cli/tests/serve_prod_embed.rs`
- **Verification:** Test passes; both 307 redirects are observed end-to-end.
- **Committed in:** `5e64687` (folded into Task 1 GREEN).

**2. [Rule 3 - Blocking] Unused-import warning on `IntoResponse` in debug builds**
- **Found during:** Task 1 (`cargo check -p foliom-cli`)
- **Issue:** `IntoResponse` was imported unconditionally in `embed.rs` but only used in the `#[cfg(not(debug_assertions))]` branch (for the empty-bundle 404 path). Triggered `warning: unused import: IntoResponse` in debug builds.
- **Fix:** Gated the `IntoResponse` import with `#[cfg(not(debug_assertions))]`. Kept `Response` unconditional since both branches construct one.
- **Files modified:** `crates/cli/src/cmd/serve/embed.rs`
- **Verification:** `cargo check -p foliom-cli` clean (no warnings); `cargo build --release -p foliom-cli --locked` clean.
- **Committed in:** `5e64687`.

---

**Total deviations:** 2 auto-fixed (1 test bug, 1 build hygiene blocker).
**Impact on plan:** Both were minor mechanics that didn't change scope or surface. No scope creep.

## Issues Encountered

- **Open Question A9 from 02-RESEARCH (dev-mode redirect `fetch('/api/...')` origin headaches)** — not exercised in this plan because no live `fetch('/api/...')` was triggered through the redirected origin in the test suite. The dev loop in normal use opens `http://localhost:5173/` directly (not via a redirect from 7345), so the origin is consistently `localhost:5173`, the Vite proxy forwards `/api/*` → 7345, and same-origin policy holds. The 307 redirect from 7345 → 5173 is a corner case (someone hitting the backend port directly in the browser); their session continues from `5173` origin where the proxy is in play. No headaches observed. If a future symptom appears, switch to a reverse-proxy implementation per A9.
- **`--open` cross-OS behavior observed locally:** Implementation environment is WSL2 (Linux). `open` crate uses `xdg-open` which on WSL2 hands off to Windows via `wslview`/Edge if configured; if not, it logs a warning and exits cleanly. Best-effort contract holds. Windows-native and macOS verification deferred to plan 02-08 CI matrix.

## User Setup Required

None — no external services or env vars touched by this plan.

## Next Phase Readiness

- **Ready for plan 02-08 (perf gates):** Single-binary distribution model is locked in; ACPT-02 (cold start <2s on 5k notes) and ACPT-03 (RSS <300MB) can now be measured against the release binary directly. The release-test plumbing (`cargo test --release ... --test serve_prod_embed`) gives 02-08 a known-good template for any benchmark/CI-asserted test it needs to gate on release builds.
- **No outstanding blockers.**

## Self-Check: PASSED

- `crates/cli/src/cmd/serve/embed.rs` — present (verified).
- `crates/cli/tests/serve_prod_embed.rs` — present (verified).
- Commit `fb7da2d` (test RED) — present in `git log`.
- Commit `5e64687` (feat GREEN) — present in `git log`.
- `cargo test -p foliom-cli` — 16 tests pass.
- `cargo test --release -p foliom-cli --test serve_prod_embed` — 1 test passes.
- Manual smoke (release binary, no `frontend/dist/` in cwd): `GET /` returns 200 + `<div id="app">` HTML; `GET /foo/bar/baz` SPA-fallback returns same HTML.

---
*Phase: 02-read-only-web-ui*
*Completed: 2026-05-22*
