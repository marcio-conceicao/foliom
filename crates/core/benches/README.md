# Foliom benchmarks (Phase 2 perf gates)

This directory hosts the ACPT-02 / ACPT-03 performance harness wired
in plan **02-08** (see `.planning/phases/02-read-only-web-ui/02-08-PLAN.md`
and `02-RESEARCH.md §Performance Harness`).

## Quick start

```bash
# 1. Generate the synthetic 5k corpus (deterministic; ~30 s).
cargo run --release --bin foliom-bench-gen -- \
    --out /tmp/synth-5k --count 5000 --seed 42

# 2. Run the cold-start bench (Criterion; ~30 s measurement window).
cargo bench --bench cold_start

# 3. Gate the measured mean against the CI ceiling (3 s = target 2 s × 1.5).
python3 scripts/bench_assert.py \
    target/criterion/cold_start_5k/*/new/estimates.json \
    3000000000

# 4. Measure idle RSS after `foliom serve` boots on the corpus.
cargo build --release --bin foliom --bin foliom-bench-rss
./target/release/foliom-bench-rss /tmp/synth-5k
```

## What each piece measures

| Tool | Requirement | Ceiling | Source |
|------|-------------|---------|--------|
| `cargo bench --bench cold_start` | **ACPT-02** cold start on 5k corpus | mean < 3 s (target 2 s × 1.5) | `crates/core/benches/cold_start.rs` |
| `foliom-bench-rss` | **ACPT-03** idle RSS on 5k corpus | RSS < 450 MB (target 300 × 1.5) | `crates/cli/src/bin/bench-rss.rs` |
| `scripts/bench_assert.py` | CI gate parser | non-zero exit if mean ≥ ceiling | `scripts/bench_assert.py` |

## Reading Criterion output

Criterion writes per-bench statistics under
`target/criterion/<group>/<function>/`:

```
cold_start_5k/db_open_reindex_full/
├── new/
│   ├── estimates.json   ← bench_assert.py reads `mean.point_estimate` (ns)
│   ├── sample.json
│   └── ...
├── change/              ← present once a baseline exists
└── report/index.html    ← open in a browser for the violin plot
```

> **Why the underscore name:** Criterion sanitizes `:` and `::` from
> bench function names when building the output directory (e.g.
> `Db::open` becomes `Db__open`). Using `db_open_reindex_full`
> keeps the path predictable for CI globs and `bench_assert.py`.

`estimates.json::mean::point_estimate` is the mean wall-clock time in
**nanoseconds**. The `--save-baseline phase2` flag (used in CI) names the
saved baseline so subsequent runs can diff against it.

## Why × 1.5 ceiling (D-35 / A8)

The PRD's reference hardware is an M1-class laptop (2 s cold start,
300 MB RSS). CI runners on GitHub Actions (`ubuntu-latest`) are slower
and noisier; D-35 budgets a × 1.5 tolerance:

- ACPT-02 ceiling: 2 s × 1.5 = **3 s** (3 × 10⁹ ns).
- ACPT-03 ceiling: 300 MB × 1.5 = **450 MB**.

If GHA runners drift slower than 1.5 × reference (Assumption A8), the
plan-02-08 SUMMARY records the actual numbers and recommends raising
to × 2 in a follow-up plan — do **not** silently widen the ceiling
inside this file.

WSL2 / Windows-native dev environments are likewise out-of-spec versus
the M1 reference; record any local deviation in `PERF-BASELINE.md`
rather than tuning the constants here.

## Corpus contents (5000 files)

The generator emits a 70 / 30 journal-vs-page split:

- `journals/YYYY_MM_DD.md` × 3500 — date-stamped from 2010-01-01.
- `pages/Topic N.md` × 1500 — flat topical namespace.

Each file mixes:

- TAB-indented bullets (depth 0–5, skewed to 0–2).
- ~5 % bullets carry a fenced code block (`rust` / `python` / `sql`).
- ~5 % blocks carry `id:: <hex>` or `collapsed:: true` properties.
- ~10 % journal pages carry a `:LOGBOOK: … :END:` drawer.
- ~4 link/tag refs per block on average — mix of `[[Topic N]]`,
  `[[YYYY_MM_DD]]`, `#tag`, and `#[[multi word tag]]`.
- File sizes follow a log-normal distribution: median ≈ 2 KB, tail
  out to 30–50 KB.

Reproducible: same `--seed` + `--count` + bench-gen version → identical
file contents byte-for-byte (mtimes excluded).

## Skipping vs failing

`cold_start.rs` **skips gracefully** if the corpus is missing
(`eprintln!` then early return) so a fresh-clone `cargo bench` doesn't
fail before the developer has run `bench-gen`. CI generates the corpus
in a dedicated step before `cargo bench --bench cold_start`.
