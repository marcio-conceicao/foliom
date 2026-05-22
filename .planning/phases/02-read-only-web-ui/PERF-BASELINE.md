# Foliom — Phase 2 performance baseline

Pinned by **plan 02-08** as the regression reference for ACPT-02
(cold start < 2 s) and ACPT-03 (idle RSS < 300 MB) on the
5 000-file synthetic corpus.

Future plans must compare against the **CI ceiling** (target × 1.5 per
D-35), not against the dev-machine numbers below — the dev machine is
WSL2 on Windows 11, not the PRD's M1-class reference hardware.

---

## Reference hardware (PRD §RNF-01)

- **CPU class:** Apple M1 (or equivalent x86 8-core / 16 GB RAM laptop).
- **Disk:** NVMe SSD.
- **OS:** macOS 14 / Ubuntu 22.04 / Windows 11.

The PRD numbers (2 s cold start, 300 MB RSS) presume this class of
machine. CI runners are GitHub Actions `ubuntu-latest` (typically a
2-vCPU shared box) — slower and noisier; D-35 adds a × 1.5 tolerance
so the gate doesn't flake.

## CI ceilings (enforced by gates)

| Metric | Target | Ceiling (× 1.5) | Gate |
|--------|--------|-----------------|------|
| Cold start | 2 s | **3 s** (3 × 10⁹ ns) | `scripts/bench_assert.py target/criterion/cold_start_5k/db_open_reindex_full/new/estimates.json 3000000000` |
| Idle RSS | 300 MB | **450 MB** | `./target/release/foliom-bench-rss /tmp/synth-5k` |
| Frontend bundle | 300 KB gz | **600 KB** uncompressed | `du -sk frontend/dist` ≤ 600 |

---

## First-run baseline (2026-05-22)

### Dev machine (NOT reference hardware)

- **Host:** WSL2 (Ubuntu) on Windows 11.
- **Kernel:** Linux 6.6.87.1-microsoft-standard-WSL2.
- **Disk:** through WSL2 ext4 VHD (NOT a raw NVMe partition).
- **Note:** PRD numbers do NOT apply here. WSL2 file IO is markedly
  slower than native Linux on the same hardware, especially for the
  many-small-files workload Foliom indexes.

| Metric | Measured | CI ceiling | Verdict |
|--------|----------|------------|---------|
| Cold start (Db::open + reindex Full) | **12.20 s** (mean of 10 samples; 95% CI [12.14, 12.27]) | 3 s | ❌ above ceiling — **expected on WSL2** |
| Idle RSS after `foliom serve` + `/api/pages` warmup | **49 MB** | 450 MB | ✅ well below ceiling |
| Frontend `dist/` size | **248 KB** uncompressed (`du -sk frontend/dist`) | 600 KB | ✅ well below ceiling |

### CI runner (ubuntu-latest, GitHub Actions)

| Metric | Measured | CI ceiling | Verdict |
|--------|----------|------------|---------|
| Cold start | *to-be-recorded on first green `bench` job* | 3 s | — |
| Idle RSS | *to-be-recorded on first green `bench` job* | 450 MB | — |
| Frontend bundle | 248 KB | 600 KB | ✅ |

Operator: after the first PR landing 02-08 produces a green CI run,
update the table above with the actual `ubuntu-latest` numbers.

---

## Hardware caveat / Assumption A8

Plan 02-08 records the cold-start measurement on WSL2 as
**12.2 s** — about 6× the PRD's 2 s reference. The CI gate is set at
3 s and **will not pass on this dev machine**, by design. The bench is
gated only in the CI `bench` job (ubuntu-latest), not in pre-commit
hooks or `cargo nextest`. Local devs running `cargo bench --bench
cold_start` will see the actual machine cost and can use it to drive
their own optimisation work, but the merge gate is GHA-only.

**Action items if the gate flakes in CI:**
1. Inspect Criterion's regression plot in `target/criterion/cold_start_5k/db_open_reindex_full/report/index.html`.
2. Compare against the saved `phase2` baseline (`cargo bench -- --baseline phase2`).
3. If a single PR is the cause, optimise in that PR.
4. If the runner class drifted (GHA changed shared-pool hardware),
   open a follow-up plan to raise the ceiling to × 2 (= 4 s) per D-35
   escalation path. Do **not** silently widen the constant inside
   `scripts/bench_assert.py` — bump it in a tracked plan with
   reasoning recorded here.

---

## Frontend bundle composition (informational)

`frontend/dist/` total: 248 KB (uncompressed) at the close of Phase 2.

Major contributors are inventoried per Plan 02-05 SUMMARY decisions:
- Svelte 5 runtime (compiled output, very small per-component).
- markdown-it (read-only block renderer) — biggest single dep.
- CodeMirror 6 modular bundle — lazy-loaded into the active block on
  focus, so the cold path doesn't pay for it.
- Prism (deferred from 02-06; not in the bundle yet — landing later).

Headroom against the 600 KB ceiling: **352 KB** (1.4×). Plenty of room
for Prism + a few more components in Phase 3.

---

## Future tuning (recorded as drift)

When a future change moves any of these numbers ≥ 10 %, append a row
below with date, commit, and a one-line cause analysis. Do not edit
the historical rows — they're the regression record.

| Date | Commit | Metric | Old | New | Cause |
|------|--------|--------|-----|-----|-------|
| 2026-05-22 | (this plan) | cold start (WSL2) | — | 12.20 s | initial baseline; not the reference platform |
| 2026-05-22 | (this plan) | idle RSS (WSL2) | — | 49 MB | initial baseline; comfortably below target |
| 2026-05-22 | (this plan) | bundle (uncompressed) | — | 248 KB | initial baseline after 02-07 SPA embed |
