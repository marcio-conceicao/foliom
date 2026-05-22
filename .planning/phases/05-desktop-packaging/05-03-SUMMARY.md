---
phase: 05-desktop-packaging
plan: 03
subsystem: infra
tags: [tauri2, footprint, ci-gate, bash, github-actions, rss, installer-size, perf-baseline, dsk-03]

requires:
  - phase: 05-01
    provides: "src-tauri/ Tauri 2 crate + BOUND_PORT OnceLock pattern"
  - phase: 05-02
    provides: ".github/workflows/release.yml — matrix release workflow with installer size gates (partial)"
  - phase: 02-08
    provides: "foliom-bench-rss binary — sysinfo-based RSS measurement tool (49 MB baseline)"

provides:
  - "scripts/footprint_check.sh — bash installer size gate (portable macOS/Linux, exits 1 if > ceiling)"
  - "release.yml extended: macOS gate calls footprint_check.sh; Windows inline PowerShell"
  - "release.yml RSS gate: foliom-bench-rss with FOLIOM_BENCH_CEILING_MB=150 (non-Windows)"
  - "PERF-BASELINE.md — headless baseline 49 MB + desktop targets + methodology + scope note"

affects:
  - "Future regression detection — PERF-BASELINE.md drift policy applies when any metric shifts >= 10%"
  - "Phase 5 verification — DSK-03 complete; verifier can confirm gate steps present in release.yml"

tech-stack:
  added:
    - "scripts/footprint_check.sh — new reusable bash script for installer size assertion"
  patterns:
    - "Installer size gate: bash script called from CI, accepts path + ceiling as args, exits 0/1"
    - "RSS gate: FOLIOM_BENCH_FOLIOM env var selects binary; FOLIOM_BENCH_CEILING_MB overrides ceiling"
    - "WebView renderer scope exclusion: bench-rss measures only the foliom serve PID, not the OS WebView renderer"
    - "Drift policy: append rows only, never overwrite; require named decision to bump CI ceiling"

key-files:
  created:
    - "scripts/footprint_check.sh — installer size assertion (executable, bash, 30 MB default ceiling)"
    - ".planning/phases/05-desktop-packaging/PERF-BASELINE.md — baseline targets + methodology"
  modified:
    - ".github/workflows/release.yml — macOS gate refactored to call footprint_check.sh; Windows gate updated; RSS gate added"

key-decisions:
  - "footprint_check.sh is bash-only (macOS/Linux); Windows footprint check remains inline PowerShell — consistent with runner OS capabilities"
  - "RSS gate skips Windows (runner.os != Windows) because foliom-bench-rss uses Linux/macOS proc inspection"
  - "du -sm chosen over stat -c for portability across macOS (BSD du) and Linux (GNU du); ~5% measurement difference is within 30 MB ceiling headroom"
  - "WebView renderer excluded from RSS gate: it is a separate OS process (WebView2.exe / com.apple.WebKit.WebContent) outside foliom's control"
  - "Synthetic fixture (12-file logseq-synthetic) used for RSS gate — always present, starts in < 1s; conservative lower bound"

requirements-completed: [DSK-03]

duration: 3min
completed: "2026-05-22"
---

# Phase 5 Plan 03: Footprint Gate — Installer Size + RSS Assertion Summary

**Bash installer size script (footprint_check.sh), foliom-bench-rss RSS gate (150 MB ceiling), and PERF-BASELINE.md recording the 49 MB headless axum baseline and desktop footprint targets for DSK-03**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-22T13:45:18Z
- **Completed:** 2026-05-22T13:48:45Z
- **Tasks:** 1
- **Files modified:** 3 (1 created, 1 new script, 1 extended)

## Accomplishments

- Created `scripts/footprint_check.sh` — reusable installer size gate, accepts `<installer_path> [ceiling_mb]`, exits 0 within budget and 1 on failure; verified against 5 MB (pass), 35 MB (fail), and missing-file (fail) cases
- Extended `.github/workflows/release.yml` with RSS gate step: builds `foliom-bench-rss`, runs against `logseq-synthetic` fixture, asserts `FOLIOM_BENCH_CEILING_MB=150`; macOS installer gate refactored to delegate to `footprint_check.sh`
- Created `PERF-BASELINE.md` documenting headless axum baseline (49 MB, Phase 2), desktop targets (< 30 MB installer, < 150 MB RSS), gate methodology, WebView scope exclusion rationale, and drift policy

## Task Commits

1. **Task 1: footprint_check.sh + release.yml RSS gate + PERF-BASELINE.md** - `afd64d3` (feat)

**Plan metadata:** (set after SUMMARY commit)

## Files Created/Modified

- `/home/mconceicao/work-others/foliom/scripts/footprint_check.sh` — Installer size gate script; portable bash; `du -sm` for macOS/Linux; default ceiling 30 MB; exits 1 on missing file or exceeded budget; executable (`chmod +x`)
- `/home/mconceicao/work-others/foliom/.github/workflows/release.yml` — macOS gate refactored to call `bash scripts/footprint_check.sh "$DMG" 30`; Windows gate step name updated; new "Footprint gate — Idle RSS < 150 MB" step added (non-Windows, cargo build foliom-bench-rss + run against logseq-synthetic)
- `/home/mconceicao/work-others/foliom/.planning/phases/05-desktop-packaging/PERF-BASELINE.md` — Phase 5 performance baseline document with targets table, headless axum baseline measurement, TBD desktop measurements, gate methodology, WebView scope exclusion note, and drift policy

## Decisions Made

- **du -sm for portability:** `stat -c` is Linux-only; `wc -c` gives byte count (less readable). `du -sm` works on both macOS (BSD) and Linux (GNU) CI runners. The ~5% SI vs binary MB difference is within the 30 MB ceiling headroom for a ~10–15 MB Tauri installer.
- **RSS gate skips Windows:** `foliom-bench-rss` uses sysinfo process RSS which works on Linux/macOS. Windows runner skips via `if: runner.os != 'Windows'`. This is acceptable for v1 — Linux CI is the authoritative gate for RSS regression.
- **footprint_check.sh is bash-only:** Windows uses inline PowerShell in the workflow. No bash shell available on `windows-latest` by default for external scripts; PowerShell is the correct choice there.

## Deviations from Plan

None — plan executed exactly as written. The release.yml already had inline installer size checks from plan 05-02; these were preserved with minor refactoring (macOS gate now delegates to `footprint_check.sh`; Windows gate step name aligned with plan spec).

## Issues Encountered

- `cargo nextest` not installed locally — used `cargo test --workspace --exclude foliom-tauri` instead. The foliom-tauri exclusion is required because WebKitGTK system libs are not available in WSL2 (pre-existing from Phase 5 plan 01); all other workspace tests pass.

## User Setup Required

None — no external service configuration required. The footprint gates run automatically in CI on `v*` tag pushes.

## Next Phase Readiness

- **Phase 5 complete:** All three plans (05-01 Tauri shell, 05-02 release CI, 05-03 footprint gate) have committed deliverables and SUMMARY files. Phase 5 is ready for verification.
- **v1 milestone complete:** This is the final plan of the v1 milestone. All phases (01–05) have shipped.
- **Footprint gate active:** Pushing a `v*` tag will trigger the full release workflow including installer size + RSS assertions. First real measurements will populate the TBD rows in PERF-BASELINE.md.
- **Cert procurement still needed:** Code signing requires Apple Developer Program enrollment and Windows cert purchase. The CI workflow is ready; secrets are plugged in when certs are acquired.

## Known Stubs

- `PERF-BASELINE.md` "Desktop Tauri baseline" table rows are TBD — they will be filled after the first release CI run on `macos-latest` + `windows-latest`. This is intentional: the baseline document exists now so the first run can record into it.

## Threat Flags

No new security surface introduced. `footprint_check.sh` reads only file size via `du` and does not execute the installer (T-05-12 mitigated). The RSS gate uses `foliom-bench-rss` against a synthetic fixture with no PII (T-05-14 accepted).

## Self-Check: PASSED

- `scripts/footprint_check.sh`: FOUND
- `bash -n scripts/footprint_check.sh`: SYNTAX OK
- `ls -la scripts/footprint_check.sh | grep "^-rwx"`: EXECUTABLE OK
- `grep -c "Footprint gate" .github/workflows/release.yml`: 5 (>= 2, OK)
- `grep "foliom-bench-rss" .github/workflows/release.yml`: 3 matches
- `grep "FOLIOM_BENCH_CEILING_MB=150" .github/workflows/release.yml`: 1 match
- `.planning/phases/05-desktop-packaging/PERF-BASELINE.md`: FOUND
- `python3 yaml.safe_load + step presence check`: PASSED (tauri-action@v0 + Footprint gates present)
- `cargo test --workspace --exclude foliom-tauri`: all 9 tests pass, 0 failed

---
*Phase: 05-desktop-packaging*
*Completed: 2026-05-22*
