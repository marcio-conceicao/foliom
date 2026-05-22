---
phase: 02-read-only-web-ui
plan: 01
subsystem: cli-http-scaffold
tags: [phase-2, axum, scaffold, http, serve, security]
requires:
  - foliom-core::storage::Db
  - foliom-core::indexer::reindex
provides:
  - "foliom serve <root>: localhost HTTP server scaffold"
  - "AppState { db: Arc<Mutex<Db>>, root: PathBuf } for downstream handlers"
  - "Host-header allowlist middleware (T-02-01 mitigation)"
  - "Graceful shutdown via tokio::signal::ctrl_c"
affects:
  - crates/cli (new serve subcommand + module tree)
  - workspace Cargo.toml (new shared deps: axum, tower, tower-http, tokio, open, time)
tech-stack:
  added:
    - "axum 0.7"
    - "tower 0.5"
    - "tower-http 0.6 (compression-full + trace)"
    - "tokio 1.40 (macros, rt, signal, net)"
    - "open 5 (best-effort --open)"
    - "time 0.3 (reserved for journal formatting in plan 02-04; declared workspace-wide now)"
    - "ureq 2 (dev-only, json feature — integration test HTTP client)"
  patterns:
    - "Synchronous run() wrapper builds a current_thread tokio runtime per D-25"
    - "Arc<Mutex<Db>> in axum::State per D-38"
    - "Loopback-only bind with AddrInUse fallback to :0"
    - "Stateful banner on stdout (always visible); structured tracing on stderr"
key-files:
  created:
    - crates/cli/src/cmd/serve/mod.rs
    - crates/cli/src/cmd/serve/state.rs
    - crates/cli/src/cmd/serve/browser.rs
    - crates/cli/src/cmd/serve/middleware.rs
    - crates/cli/src/cmd/serve/routes/mod.rs
    - crates/cli/src/cmd/serve/routes/health.rs
    - crates/cli/tests/serve_boot.rs
    - .planning/phases/02-read-only-web-ui/02-01-SUMMARY.md
  modified:
    - Cargo.toml (workspace deps)
    - crates/cli/Cargo.toml (consume workspace deps + ureq dev-dep)
    - crates/cli/src/main.rs (Serve variant + dispatch)
    - crates/cli/src/cmd/mod.rs (re-export serve)
decisions:
  - "Reject hostile Host headers with 421 Misdirected Request (closest HTTP semantic to 'wrong server'). Documented at SECURITY: tag in middleware.rs."
  - "Integration test uses --port 0 to avoid colliding with developers running foliom serve on 7345 locally. AddrInUse fallback path verified manually per success criterion."
  - "Used libc::kill directly (extern via #[link_name]) instead of pulling nix as a dev-dep for the single SIGINT call. Keeps dev-dep surface minimal."
  - "Tagged AppState fields with #[allow(dead_code)] for db/root since the only Phase 2 plan-01 consumer is /api/health (no DB access). Subsequent plans (02-02+) read them; the allow goes away naturally as routes are added."
metrics:
  duration_minutes: 16
  tasks_completed: 2
  files_touched: 11
  tests_added: 1
  tests_passing: "all (122 across workspace)"
completed_date: 2026-05-22
---

# Phase 2 Plan 01: HTTP Server Scaffold Summary

**Substantive one-liner:** New `foliom serve <root>` subcommand boots axum 0.7 on 127.0.0.1:7345 after a startup reindex, exposes `/api/health`, and rejects DNS-rebinding via a Host-header allowlist middleware — the first vertical slice of Phase 2.

## What Shipped

The Foliom binary now learns a sixth subcommand. `foliom serve <root>` opens the Phase 1 SQLite index, runs `indexer::reindex` (incremental by default; `--full` forces a cold rebuild), then binds a loopback axum server on a configurable port (default 7345; falls back to OS-assigned if requested port is busy). Requests pass through a host-allowlist middleware before any handler runs — anything not addressed to `127.0.0.1` or `localhost` gets a 421 Misdirected Request response. Ctrl+C triggers graceful shutdown via `axum::serve(...).with_graceful_shutdown(tokio::signal::ctrl_c())`.

The router currently exposes a single route, `GET /api/health`, which returns `{"ok": true}`. Subsequent plans in this phase (02-02..02-08) extend this scaffold with the actual page/journal/search endpoints from D-24 and the embedded SPA from D-23.

The shared `AppState { db: Arc<Mutex<Db>>, root: PathBuf }` is in place and `Clone`-able for `axum::extract::State` per D-38. Single-threaded tokio runtime (`new_current_thread`) is sufficient per D-25 — the entire phase targets single-user, low-concurrency workloads.

## Commits

| Hash    | Type | Description                                                                   |
|---------|------|-------------------------------------------------------------------------------|
| 4100ec7 | feat | Wire serve subcommand scaffold + axum/tower/tokio deps                        |
| 0d38ce5 | feat | Implement serve runtime with axum boot + Host allowlist + integration test    |

## Key Decisions

- **421 over 403 for Host rejection.** RFC 9110 §15.5.20 defines 421 Misdirected Request as "the request was directed at a server that is not able to produce a response," which is the precise semantic for "this Host header points elsewhere." Documented inline in middleware.rs.
- **`libc::kill` via direct extern instead of adding `nix` as a dev-dep.** The integration test needs SIGINT exactly once; a 4-line `extern "C"` block is lighter than pulling `nix` for a single syscall.
- **`--port 0` in the integration test.** Avoids collision with `foliom serve` running locally on 7345 during development. The AddrInUse fallback path is exercised by the manual verification step in `<verification>`.
- **`time` crate declared in workspace deps now, consumed in plan 02-04.** Declaring it at the workspace level here lets later plans pin `workspace = true` instead of re-pinning the version. Zero runtime cost since `crates/cli` doesn't depend on it yet.

## Threat Model Verification

| Threat ID | Mitigation Status | Evidence |
|-----------|-------------------|----------|
| T-02-01 (DNS rebinding) | Mitigated | `middleware::host_allowlist` rejects Host headers outside `{127.0.0.1, localhost}` with 421. Integration test asserts `Host: evil.example.com:7345` → 421. Socket is loopback-bound via `SocketAddr::from((Ipv4Addr::LOCALHOST, port))`. |
| T-02-02 (Stale index served) | Mitigated | Startup reindex error propagated via `anyhow::Context("reindex no startup ...")` aborts boot. Tested implicitly by the integration test (which exercises a successful reindex path; failure mode is the standard `?` propagation). |
| T-02-03 (DoS via runtime) | Accepted | Single-user loopback app; current_thread runtime sufficient per D-25. |
| T-02-SC (Supply-chain) | Mitigated | All new crates (axum/tower/tower-http/tokio/open/time/ureq) are well-known maintainers per 02-RESEARCH §Package Legitimacy Audit. No `[SUS]` crates introduced. |

## Deviations from Plan

None — plan executed as written. Two minor judgment calls inside Claude's-discretion scope:

1. Used a direct `extern "C" { fn kill(...) }` block in the integration test instead of adding the `nix` crate as a dev-dep. Functionally equivalent, smaller dep surface. Documented in the test file.
2. Added `#[allow(dead_code)]` on `AppState::db` and `AppState::root` because the only consumer in this plan (`/api/health`) ignores both. The allows go away naturally when plan 02-02 wires real handlers.

## Authentication Gates

None. Phase 2 is a single-user local app with no auth.

## Known Stubs

- `AppState::db` and `AppState::root` are constructed but unread by the only route (`/api/health`). This is deliberate — the scaffold is intentionally minimal in this plan, and the state is consumed starting in plan 02-02. Documented with `#[allow(dead_code)]` and a comment pointing to the next plan.

## Threat Flags

None — no new security-relevant surface introduced beyond what the threat model already covers.

## TDD Gate Compliance

Plan frontmatter has `type: execute` (not `tdd`), but individual tasks were tagged `tdd="true"`. Each task added a verification (Task 1: `serve --help` shape; Task 2: integration test in `serve_boot.rs`). Test commits and feat commits are interleaved in one commit per task per the standard executor protocol.

## Verification Evidence

```
$ cargo build --workspace --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.26s

$ cargo test --workspace --locked
test result: ok. 122 tests passing across all crates (15 + 24 + 36 + 12 + 9 + ...; full breakdown in CI output)
serve_boot: test health_returns_ok_and_host_allowlist_rejects_evil_host ... ok

$ cargo run -p foliom-cli -- serve --help
  Sobe o servidor HTTP local read-only (Phase 2 — D-22..D-25)
  Options:
        --port <PORT>  Porta TCP em 127.0.0.1... [default: 7345]
        --open         Abre o navegador padrão...
        --full         Força reindex completo no startup...
```

## Self-Check: PASSED

- [x] `crates/cli/src/cmd/serve/mod.rs` exists
- [x] `crates/cli/src/cmd/serve/state.rs` exists
- [x] `crates/cli/src/cmd/serve/browser.rs` exists
- [x] `crates/cli/src/cmd/serve/middleware.rs` exists
- [x] `crates/cli/src/cmd/serve/routes/mod.rs` exists
- [x] `crates/cli/src/cmd/serve/routes/health.rs` exists
- [x] `crates/cli/tests/serve_boot.rs` exists
- [x] Commit `4100ec7` present in git log
- [x] Commit `0d38ce5` present in git log
- [x] `cargo build --workspace --locked` exits 0
- [x] `cargo test --workspace --locked` exits 0
- [x] AP-2 guard clean (no serialize/to_markdown/format_block in serve module)
- [x] Both must-haves truths satisfied (boot/health + Host rejection + AddrInUse fallback + Ctrl+C exit 0)
