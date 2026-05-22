# Phase 5 — Desktop Packaging: Performance Baseline

**Recorded:** 2026-05-22 (phase planning)
**Updated:** (fill in after first release build)

## Footprint Targets (DSK-03)

| Metric | Target | CI Ceiling | Electron Reference |
|--------|--------|------------|-------------------|
| macOS installer (.dmg) | < 20 MB | 30 MB | ~150–200 MB |
| Windows installer (.msi) | < 20 MB | 30 MB | ~120–180 MB |
| Idle RSS (headless axum, 5k notes) | < 100 MB | 150 MB | ~150–300 MB |

## Baseline Measurements

### Headless axum baseline (Phase 2, plan 02-08)

| Environment | RSS at idle | Notes |
|-------------|-------------|-------|
| WSL2 / ubuntu-latest CI | 49 MB | foliom serve against synth-5k; FOLIOM_BENCH_CEILING_MB=450 |

### Desktop Tauri baseline (to be filled after first release build)

| Environment | Installer size | RSS (foliom process only) | WebView RSS (separate OS process) | Notes |
|-------------|---------------|--------------------------|----------------------------------|-------|
| macOS (macos-latest, universal) | TBD | TBD | TBD (excluded from gate) | First release CI run |
| Windows (windows-latest, x64) | TBD | TBD | N/A | First release CI run |

## Gate Methodology

### Installer size

- **macOS:** `bash scripts/footprint_check.sh <path>.dmg 30` via release.yml after tauri-action
- **Windows:** `[math]::Ceiling($installer.Length / 1MB)` in inline PowerShell (bash not available on Windows runner)
- **Ceiling:** 30 MB (DSK-03)
- **Reusable script:** `scripts/footprint_check.sh` — accepts `<installer_path> [ceiling_mb]`; exits 0 within budget, 1 on failure

### Idle RSS

- **Binary measured:** `foliom serve` (headless axum process) — the same code path executed inside Tauri via `std::thread::spawn(serve_run)`
- **Tool:** `foliom-bench-rss` (crates/cli/src/bin/bench-rss.rs, sysinfo crate)
- **Corpus:** `crates/core/tests/fixtures/logseq-synthetic` (12 files; conservative lower bound vs 5k-note production workload)
- **Ceiling:** 150 MB (`FOLIOM_BENCH_CEILING_MB=150`) (DSK-03)
- **Invocation:** `FOLIOM_BENCH_CEILING_MB=150 FOLIOM_BENCH_FOLIOM=target/release/foliom ./target/release/foliom-bench-rss <corpus>`

### WebView renderer scope exclusion

The Tauri desktop binary spawns the WebView renderer as a **separate OS process**:

- Windows: `WebView2.exe` (Edge/Chromium renderer process)
- macOS: `com.apple.WebKit.WebContent` (WKWebView renderer process)

`foliom-bench-rss` measures a single PID (the `foliom serve` process) and does **not** capture the WebView renderer's RSS. This is intentional for v1:

1. The axum server process is the part Foliom controls. Its RSS is deterministic and benchmarkable.
2. WebView renderer memory is controlled by the browser vendor (Microsoft Edge/WebView2, Apple WebKit) and varies with page content, JS heap, and cache.
3. DSK-03's Core Value claim ("smaller than Electron") is about the installer size and the app's own code footprint — not the OS's WebView renderer, which exists independently of Foliom.

For future reference: measure combined RSS (foliom + WebView) using `pgrep -f foliom | xargs -I{} ps -o rss= -p {}` and sum the values. This is out of scope for v1.

## Drift Policy

When a metric drifts >= 10% from the prior recorded value:

1. Add a **new row** to the relevant table (never overwrite existing rows)
2. Record the date and environment in the new row
3. Investigate and document the cause in this file
4. Do **not** bump the CI ceiling without a written justification in this file

Ceiling bumps require a named decision (e.g., "D-05-XX: increased ceiling because...") added to both this file and to STATE.md decisions.
