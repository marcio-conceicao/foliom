---
phase: 05-desktop-packaging
verified: 2026-05-22T11:10:00Z
status: human_needed
score: 9/10
overrides_applied: 1
overrides:
  - must_have: "A Tauri 2 desktop binary launches the same Svelte UI against the in-process axum server via tauri-plugin-localhost"
    reason: "ROADMAP SC #1 wording is stale — REQUIREMENTS.md DSK-01 explicitly requires WebviewUrl::External (not tauri-plugin-localhost, which runs its own tiny_http server and would create a duplicate server). The plan frontmatter, research notes, and implementation all correctly use WebviewUrl::External. The ROADMAP SC wording predates the research finding that resolved this. Implementation matches DSK-01 as defined in REQUIREMENTS.md."
    accepted_by: "verifier (documented intentional deviation — REQUIREMENTS.md overrides stale ROADMAP SC wording)"
    accepted_at: "2026-05-22T11:10:00Z"
human_verification:
  - test: "Tauri desktop binary launches and shows UI"
    expected: "foliom-desktop launches, shows native folder picker on first run, opens Svelte UI pointing at localhost axum server, window title 'Foliom'"
    why_human: "webkit2gtk-4.1 not installed in WSL2 dev environment; cargo build -p foliom-tauri requires system library. Code is type-correct (verified from API source). Needs macOS or Windows machine (or CI runner) with native WebView."
  - test: "macOS code signing and notarization produces signed DMG"
    expected: "tauri-apps/tauri-action@v0 on macos-latest with APPLE_CERTIFICATE secrets produces a signed, notarized .dmg; codesign --verify passes"
    why_human: "Requires Apple Developer Program certificate (DSK-02). CI workflow is plumbed correctly but signing secrets not yet procured. Cannot verify signing without real certs."
  - test: "Windows code signing produces signed installer"
    expected: "tauri-apps/tauri-action@v0 on windows-latest with WINDOWS_CERTIFICATE produces a signed .msi or .exe"
    why_human: "Requires Windows code-signing certificate (DSK-02). Same as macOS — workflow correct, secrets not yet acquired."
  - test: "Footprint gate passes on first real release build"
    expected: "DMG < 30 MB on macOS runner; installer < 30 MB on Windows runner; foliom-bench-rss exits 0 with FOLIOM_BENCH_CEILING_MB=150 on macOS/Linux runner"
    why_human: "First actual Tauri build on CI required to produce real measurements. PERF-BASELINE.md desktop rows are intentionally TBD until first v* tag push."
  - test: "Folder picker appears on first launch and vault root is persisted"
    expected: "On first launch (no config.json) a native folder picker dialog opens; selecting a folder starts the server; second launch auto-starts without dialog"
    why_human: "Requires running the Tauri binary on macOS or Windows with a real WebView."
---

# Phase 5: Desktop Packaging — Verification Report

**Phase Goal:** Ship Foliom as a signed, notarized Tauri 2 desktop binary on macOS and Windows that wraps the same Svelte UI consuming the in-process axum server — with footprint materially smaller than an Electron equivalent.
**Verified:** 2026-05-22T11:10:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Tauri 2 desktop binary wraps the same Svelte UI via in-process axum server | PASSED (override) | `WebviewUrl::External` used (not tauri-plugin-localhost). DSK-01 in REQUIREMENTS.md explicitly requires `WebviewUrl::External`. ROADMAP SC #1 wording is stale — see override. |
| 2 | `src-tauri/src/main.rs` exists and uses `WebviewUrl::External` | VERIFIED | File exists at `src-tauri/src/main.rs` line 114: `WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))` |
| 3 | `BOUND_PORT: OnceLock<u16>` exported from serve::run(); set BEFORE rt.block_on | VERIFIED | `crates/cli/src/cmd/serve/mod.rs` line 47: `pub static BOUND_PORT: OnceLock<u16>`. Line 156: `BOUND_PORT.set(bound.port())`. Line 171: `rt.block_on(...)`. Ordering confirmed. |
| 4 | No `tauri-plugin-localhost` in src-tauri/ or .github/ | VERIFIED | `grep -rn "tauri-plugin-localhost" src-tauri/ .github/` returns 0 matches |
| 5 | `.github/workflows/release.yml` exists, valid YAML, uses `tauri-apps/tauri-action@v0` with macOS + Windows matrix | VERIFIED | File exists; `python3 yaml.safe_load` passes; step `tauri-apps/tauri-action@v0` present; matrix has `universal-apple-darwin` and `windows-latest` entries |
| 6 | Conditional signing guards use correct expressions; `APPLE_PASSWORD` not `APPLE_ID_PASSWORD` | VERIFIED | Line 56: `if: runner.os == 'macOS' && secrets.APPLE_CERTIFICATE != ''`; line 78: Windows equivalent; `APPLE_PASSWORD` at line 106; `APPLE_ID_PASSWORD` returns 0 matches |
| 7 | `scripts/footprint_check.sh` exists, is executable, exits 0 for file within ceiling, exits 1 for file over ceiling | VERIFIED | File executable (`-rwxr-xr-x`); `bash -n` syntax check passes; behavioral spot-check: 1 MB file against 30 MB ceiling → exit 0 (PASS); 35 MB file against 30 MB ceiling → exit 1 (FAIL, correct) |
| 8 | Release workflow asserts installer size < 30 MB and RSS < 150 MB | VERIFIED | `release.yml` has 5 "Footprint gate" steps; `FOLIOM_BENCH_CEILING_MB=150` present; `bash scripts/footprint_check.sh "$DMG" 30` wired for macOS gate |
| 9 | `PERF-BASELINE.md` exists with targets table, headless baseline (49 MB), and desktop methodology | VERIFIED | File in phase dir; targets table with 30 MB installer / 150 MB RSS ceilings; headless axum 49 MB baseline recorded; WebView scope exclusion documented |
| 10 | All workspace tests (excluding foliom-tauri) remain green | VERIFIED | `cargo test --workspace --exclude foliom-tauri` → all 147 Rust tests pass, 0 failed; `npx vitest run` (frontend) → 177 tests pass, 23 files, 0 failed |

**Score:** 9/10 truths verified (includes 1 override); 5 items require human testing on real hardware

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src-tauri/src/main.rs` | Tauri entry point: setup hook, serve::run thread, OnceLock poll, WebviewUrl::External | VERIFIED | All 5 steps present: store read (line 34), folder picker (line 54), std::thread::spawn (line 80), OnceLock poll with 5s timeout (lines 94-108), WebviewWindowBuilder (line 114) |
| `src-tauri/Cargo.toml` | tauri 2, tauri-plugin-dialog 2, tauri-plugin-store 2, foliom-cli path dep | VERIFIED | All 4 deps confirmed |
| `src-tauri/tauri.conf.json` | productName=Foliom, identifier=dev.foliom.app, bundle targets | VERIFIED | File exists with correct fields |
| `src-tauri/capabilities/default.json` | Remote origin allowlist for http://127.0.0.1:*/** | VERIFIED | `"remote": { "urls": ["http://127.0.0.1:*/**"] }` present |
| `src-tauri/build.rs` | tauri_build::build() | VERIFIED | File exists |
| `src-tauri/icons/` | 4 icon files (32x32.png, 128x128.png, icon.icns, icon.ico) | VERIFIED | All 4 files present (placeholder icons — documented as out-of-scope for DSK-01) |
| `crates/cli/src/cmd/serve/mod.rs` | pub static BOUND_PORT: OnceLock<u16> + BOUND_PORT.set() before rt.block_on | VERIFIED | Lines 47, 156, 171 confirmed in correct order |
| `.github/workflows/release.yml` | Release workflow with tauri-action@v0, conditional signing, matrix | VERIFIED | Exists, valid YAML, all required steps present |
| `scripts/footprint_check.sh` | Executable bash; exits 0 within budget, 1 exceeded | VERIFIED | Behavioral spot-check confirms both exit codes |
| `.planning/phases/05-desktop-packaging/PERF-BASELINE.md` | Baseline targets + methodology | VERIFIED | File exists with complete content |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src-tauri/src/main.rs` setup hook | `crates/cli/src/cmd/serve/mod.rs::run()` | `std::thread::spawn` (line 80) | VERIFIED | `std::thread::spawn` confirmed; no `tauri::async_runtime::spawn` in non-comment code |
| `src-tauri/src/main.rs` | BOUND_PORT OnceLock | poll loop with 20ms sleep (lines 94-108) | VERIFIED | `BOUND_PORT.get()` polled; 250-iteration (5s) timeout present |
| WebviewWindowBuilder | axum server | `WebviewUrl::External(http://127.0.0.1:{port}/)` (line 114) | VERIFIED | `WebviewUrl::External` confirmed |
| `.github/workflows/release.yml` footprint gate | `scripts/footprint_check.sh` | `bash scripts/footprint_check.sh "$DMG" 30` (line 129) | VERIFIED | Grep confirms call present |
| `release.yml` RSS gate | `foliom-bench-rss` binary | `cargo build --release --bin foliom-bench-rss` + `FOLIOM_BENCH_CEILING_MB=150` | VERIFIED | Both confirmed in release.yml |
| macOS signing step | APPLE_CERTIFICATE secret | `if: runner.os == 'macOS' && secrets.APPLE_CERTIFICATE != ''` (line 56) | VERIFIED | Correct conditional guard |
| Windows signing step | WINDOWS_CERTIFICATE secret | `if: runner.os == 'Windows' && secrets.WINDOWS_CERTIFICATE != ''` (line 78) | VERIFIED | Correct conditional guard |

---

### Data-Flow Trace (Level 4)

Not applicable for this phase — deliverables are Rust binary scaffold, CI workflow, and bash scripts. No dynamic data-rendering components introduced.

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `footprint_check.sh` exits 0 for 1 MB file (ceiling 30 MB) | `bash scripts/footprint_check.sh /tmp/1mb.dmg 30` | "OK: installer size within budget" (exit 0) | PASS |
| `footprint_check.sh` exits 1 for 35 MB file (ceiling 30 MB) | `bash scripts/footprint_check.sh /tmp/35mb.dmg 30` | "::error::Installer 35 MB exceeds 30 MB budget (DSK-03)" (exit 1) | PASS |
| `cargo build -p foliom-cli` compiles | `cargo build -p foliom-cli` | `Finished dev profile` (exit 0) | PASS |
| `cargo test --workspace --exclude foliom-tauri` all green | `cargo test --workspace --exclude foliom-tauri` | 147 Rust tests passed, 0 failed | PASS |
| `npx vitest run` all green | `cd frontend && npx vitest run` | 177 tests passed (23 files), 0 failed | PASS |
| `cargo build -p foliom-tauri` compiles | Skipped — webkit2gtk-4.1 not installed in WSL2 | N/A | SKIP (human_needed) |

---

### Probe Execution

No conventional `scripts/*/tests/probe-*.sh` files declared or found for this phase. Skip.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DSK-01 | 05-01-PLAN.md | Tauri 2 binary wrapping axum via WebviewUrl::External | SATISFIED | `src-tauri/src/main.rs` with `WebviewUrl::External`; BOUND_PORT OnceLock wired; no tauri-plugin-localhost |
| DSK-02 | 05-02-PLAN.md | macOS and Windows code-signed installers | SATISFIED (CI) / NEEDS HUMAN (actual signing) | CI workflow plumbing complete; conditional signing guards present; actual certs not yet procured |
| DSK-03 | 05-03-PLAN.md | Installer < 30 MB, idle RSS < 150 MB | SATISFIED (gate) / NEEDS HUMAN (first measurement) | `footprint_check.sh` gate wired in `release.yml`; RSS gate with `FOLIOM_BENCH_CEILING_MB=150` present; actual measurements TBD on first release CI run |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `PERF-BASELINE.md` | 26-27 | `TBD` in desktop baseline table | Info | Intentional placeholder — desktop build measurements populated on first `v*` tag push. Not a code debt marker. Planning artifact, not executable code. |

No `TBD`, `FIXME`, or `XXX` markers found in any modified code file (`src-tauri/src/main.rs`, `crates/cli/src/cmd/serve/mod.rs`, `.github/workflows/release.yml`, `scripts/footprint_check.sh`).

---

### Human Verification Required

#### 1. Tauri Desktop Binary Launches and Shows UI

**Test:** Install `webkit2gtk-4.1` on WSL2 (`sudo pacman -S webkit2gtk-4.1`) or run on macOS/Windows. Build with `cargo build -p foliom-tauri`. Launch `./target/debug/foliom-desktop`. On first run, verify native folder picker appears. Select the `crates/core/tests/fixtures/logseq-synthetic` folder. Verify the Svelte UI loads in the WebView window (title "Foliom"). Kill and relaunch — verify it auto-starts without the picker.
**Expected:** Folder picker shown on first launch, Svelte UI rendered, vault auto-starts on subsequent launches.
**Why human:** webkit2gtk-4.1 system library not available in WSL2 dev environment. Code is type-correct (verified against tauri-plugin-dialog 2.7.1 and tauri-plugin-store 2.4.3 source in ~/.cargo/registry). Needs native WebView.

#### 2. macOS Code Signing and Notarization (DSK-02)

**Test:** Add Apple Developer secrets to GitHub repository secrets (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`). Push a `v0.1.0-test` tag. Verify `release.yml` runs on `macos-latest`, `tauri-apps/tauri-action@v0` produces a `.dmg`, and `codesign --verify` + `spctl --assess` pass on the result.
**Expected:** Signed, notarized `.dmg` uploaded to GitHub Release draft. Keychain import step runs and finds the signing identity.
**Why human:** Requires Apple Developer Program enrollment and cert procurement. Weeks of lead time. CI workflow is correctly plumbed (conditional on `secrets.APPLE_CERTIFICATE != ''`).

#### 3. Windows Code Signing (DSK-02)

**Test:** Add `WINDOWS_CERTIFICATE` and `WINDOWS_CERTIFICATE_PASSWORD` to GitHub secrets. Trigger release workflow. Verify Windows installer is signed (`signtool verify /pa` passes).
**Expected:** Signed `.msi` or `.exe` uploaded as release asset.
**Why human:** Requires Windows OV certificate purchase. Post-June 2023 CA/B Forum requirement — EV certs require hardware token or Azure Key Vault. Workflow plumbing is correct.

#### 4. Footprint Gate on First Real Release Build (DSK-03)

**Test:** Push a `v*` tag to trigger release workflow. Observe the "Footprint gate — DMG size < 30 MB (macOS)" and "Footprint gate — Idle RSS < 150 MB" steps in CI. Record actual measurements in PERF-BASELINE.md desktop table.
**Expected:** DMG < 30 MB; RSS < 150 MB; both gate steps exit 0; PERF-BASELINE.md TBD rows filled in.
**Why human:** Requires macOS CI runner to produce the actual Tauri bundle. Cannot measure installer size without building the bundle.

#### 5. Folder Picker and Vault Persistence UX

**Test:** On macOS or Windows, run `foliom-desktop` twice. Verify: (a) first launch opens native OS folder picker, (b) selected path is persisted to OS app-data `config.json`, (c) second launch auto-starts server without dialog.
**Expected:** Picker appears once; config.json written with `vault_root` key; subsequent launch skips picker.
**Why human:** Requires real Tauri binary with native WebView and OS filesystem access.

---

### Gaps Summary

No automated gaps. All code-verifiable must-haves are VERIFIED or PASSED (override). The ROADMAP SC #1 wording (`tauri-plugin-localhost`) is stale relative to REQUIREMENTS.md DSK-01 (`WebviewUrl::External`); implementation correctly follows REQUIREMENTS.md — override applied.

Five items require real hardware (macOS/Windows with WebView and signing certs). These are `human_needed`, not FAILED — the code structure is correct and the CI workflow is properly plumbed.

---

_Verified: 2026-05-22T11:10:00Z_
_Verifier: Claude (gsd-verifier)_
