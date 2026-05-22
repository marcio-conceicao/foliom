---
phase: 05-desktop-packaging
phase_number: 5
created: 2026-05-22
mode: auto (--auto flag, all defaults selected)
---

# Phase 5 — Desktop Packaging: Context

**Goal (ROADMAP):** Ship Foliom as a signed, notarized Tauri 2 desktop binary on macOS and Windows that wraps the same Svelte UI consuming the in-process axum server — with footprint materially smaller than an Electron equivalent.

**Requirements (3):** DSK-01, DSK-02, DSK-03

**Depends on:** Phase 2 (rust-embed SPA already in release binary, 02-07), Phase 4 (complete watcher lifecycle ready).

---

## Pre-locked Decisions (from research + CLAUDE.md + prior phases)

| Area | Decision | Source |
|---|---|---|
| Desktop framework | Tauri 2.9 (`tauri-plugin-localhost`) — native WebView (WebView2/WKWebView/WebKitGTK), ~3–10 MB installer | research/STACK.md, PRD §5.4 |
| NOT Electron | Electron is the anti-pattern Foliom is built to escape (~80 MB installer, ~150–300 MB idle RAM) | PRD §2, §3.2, CLAUDE.md |
| NOT Wails v3 | `v3.0.0-alpha.1` as of May 2026 — API churn | research/STACK.md |
| axum server pattern | `tauri-plugin-localhost` spawns the same axum server in-process; webview loads `http://127.0.0.1:<port>/` | CLAUDE.md Architecture §1 |
| SPA embed | `rust-embed` already in release binary (Phase 2 plan 02-07) — no separate `frontend/dist/` needed at runtime | 02-07-SUMMARY |
| Watcher lifecycle | `spawn_watcher` started alongside axum in `serve::run()`; Tauri shell calls the same `run()` (no code fork) | 04-01-SUMMARY |

---

## Decisions Locked in This Discussion (auto-selected)

### D-50-01: Tauri workspace structure — `src-tauri/` crate added to workspace

Add `src-tauri/` at workspace root (`Cargo.toml` workspace members). `src-tauri/Cargo.toml` depends on `foliom-cli` (not foliom-core directly). Tauri's `setup` hook calls `foliom_cli::cmd::serve::run(ServeArgs { root, port: 0, open: false, force_reindex: false })` in a background thread, then opens the window on the bound URL.

No Tauri IPC commands — the HTTP interface is already the API (keeps the "same UI runs in plain browser" property per CLAUDE.md Architecture §1).

### D-50-02: Port discovery — bound port surfaced via a `once_cell::sync::OnceLock`

`serve::run()` writes the bound port into a `static PORT: OnceLock<u16>`. The Tauri `setup` hook reads the port after startup, constructs `http://127.0.0.1:<port>/` and navigates the webview to it.

### D-50-03: Native folder picker — Tauri `dialog::open` plugin

On first launch with no stored root path, show a Tauri native folder-picker dialog (`tauri-plugin-dialog`). Store the chosen path in `tauri-plugin-store` (JSON config at `$APPDATA/foliom/config.json`). On subsequent launches, auto-open to the stored root.

### D-50-04: Code signing CI — GitHub Actions secrets + Tauri action

- **macOS:** `macos-latest`, `APPLE_CERTIFICATE` (base64 .p12) + `APPLE_CERTIFICATE_PASSWORD` + `APPLE_SIGNING_IDENTITY` + `APPLE_ID` + `APPLE_TEAM_ID` + `APPLE_ID_PASSWORD` secrets. Tauri's GitHub Action (`tauri-apps/tauri-action@v0`) handles notarization via `xcrun notarytool` automatically.
- **Windows:** `windows-latest`, code-signing cert via `WINDOWS_CERTIFICATE` + `WINDOWS_CERTIFICATE_PASSWORD`. `signtool.exe` called by Tauri action.
- **Linux:** no signing (no requirement for Linux packaging in v1).
- Signing is only active when secrets are present (for forks / PRs without secrets, the build still succeeds but produces unsigned artifacts).

### D-50-05: Footprint CI gate — cargo test binary check

Footprint assertions added to CI:
- Installer size: `du -sh foliom_*.dmg` / `.msi` < 30 MB checked in the release workflow.
- Idle RSS: existing `foliom-bench-rss` binary (Phase 2 plan 02-08) re-used against the Tauri build; target < 150 MB (vs Phase 2's 49 MB baseline — extra budget for the WebView process).

---

## Scope Guardrails

**In scope:** DSK-01 (Tauri binary + localhost plugin), DSK-02 (code signing + notarization), DSK-03 (footprint gate). Exactly 3 REQ-IDs.

**Out of scope:**
- Linux packaging / `.deb` / `.AppImage` — not required in v1.
- Auto-update mechanism — v1 ships manually downloaded binaries.
- Sparkle / WinSparkle — deferred to v1.x.
- Multiple vault support — single root per launch in v1.

---

## Pre-existing Assets to Reuse

- `crates/cli/src/cmd/serve/mod.rs::run()` — the function Tauri setup hook will call.
- `crates/cli/src/bin/bench-rss.rs` (Phase 2) — footprint measurement.
- `frontend/dist/.gitkeep` + `rust-embed` bundle (Phase 2) — already in release binary.
- `.github/workflows/ci.yml` (Phase 4) — extend with release build + footprint check jobs.

---

## Operator Note (from research PITFALLS §8)

**Code-signing certs have weeks of administrative lead time:**
- Apple Developer Program: $99/yr, ~24–48h approval.
- Windows OV cert: ~$300/yr, 2–5 business days identity verification.

These should have been started during Phase 4 (per the research warning). If they are not yet in hand, the CI release job can be added with signing disabled initially and activated once certs arrive. Phase 5 build tasks do not block on cert availability.

---

## Open Questions for Research

1. `tauri-plugin-localhost` API: what's the exact Rust snippet to spawn a Tauri window that loads `http://127.0.0.1:<port>/`? Confirm for Tauri 2.9.
2. `OnceLock` pattern: does `serve::run()` need to return the port, or can a static work across threads in the Tauri context?
3. `tauri-plugin-dialog` + `tauri-plugin-store` versions for Tauri 2.9 — confirm they are stable (not alpha).
4. Tauri `tauri-action@v0` — confirm it handles both signing and notarization in a single step; confirm the environment variable names.
5. Universal macOS binary (arm64 + x86_64) — does Tauri 2 support `--target universal-apple-darwin`? Required for M1 + Intel coverage.
