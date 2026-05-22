---
phase: 01-headless-indexing-core
plan: 07
subsystem: cli
tags: [rust, clap, cli, serde-json, fts5, inventory, idx-04, idx-08, acpt-04]

requires: [01-01, 01-02, 01-03, 01-04, 01-05, 01-06]
provides:
  - "foliom CLI binary with 5 subcommands (index, reindex, search, dump-tree, inventory) + --json on every one"
  - "crates/core/src/inventory.rs — pure aggregator: walks corpus via scanner::walk, reuses parser::segment, counts 9 Logseq patterns + property/drawer file totals; never touches the DB"
  - "crates/core/src/query.rs — search_blocks (FTS5 → pages → files join) and dump_page_tree (NOCASE lookup + recursive children-by-parent)"
  - "JSON contract for Phase 2 frontend: InventoryReport, PatternCount, SearchHit, BlockNode all serde::Serialize + camelCase"
  - "ReindexStats now serde::Serialize so the existing indexer surface flows straight through to --json"
  - "crates/cli/tests/cli_integration.rs — 4 end-to-end tests via assert_cmd: pinned inventory counts (regression gate), search returns hits with full JSON contract, dump-tree shape, reindex idempotence + Full-mode semantics"
  - ".github/workflows/ci.yml — inventory smoke gated against synthetic fixture on all three OSes, continue-on-error removed"
  - "README.md — Phase 1 close-out doc with quick start, JSON contract pointer, DB locations, cross-OS smoke procedure"
affects: [02-watcher, 02-server]

tech-stack:
  added:
    - "clap 4 (derive) — subcommand tree dispatch"
    - "anyhow 1 — binary-side error context (D-19)"
    - "serde 1 (derive) + serde_json 1 — promoted to runtime dep in foliom-core; JSON contract for inventory / query / ReindexStats"
    - "tracing-subscriber 0.3 (env-filter feature) — RUST_LOG-style log filtering (D-18)"
    - "assert_cmd 2 (dev) — CLI process driver in integration tests"
  patterns:
    - "Logs to stderr, JSON to stdout: tracing_subscriber configured with_writer(io::stderr) so --json output stays a clean stream consumable by jq / serde_json::from_slice"
    - "Per-test isolated cache: TempDir + XDG_DATA_HOME / HOME / LOCALAPPDATA env override so concurrent integration tests cannot collide on the per-root <hash>.db file"
    - "Capture-once pinned counts protocol: FOLIOM_REGEN_INVENTORY=1 prints the current report in copy-paste-ready Rust array form, panics; developer pastes new values and commits"
    - "Pure aggregator: inventory_report runs without opening the DB at all — fast, side-effect-free, safe in CI"

key-files:
  created:
    - crates/core/src/inventory.rs
    - crates/core/src/query.rs
    - crates/cli/src/cmd/mod.rs
    - crates/cli/src/cmd/index.rs
    - crates/cli/src/cmd/reindex.rs
    - crates/cli/src/cmd/search.rs
    - crates/cli/src/cmd/dump_tree.rs
    - crates/cli/src/cmd/inventory.rs
    - crates/cli/tests/cli_integration.rs
    - README.md
  modified:
    - crates/core/Cargo.toml
    - crates/core/src/lib.rs
    - crates/core/src/indexer/mod.rs
    - crates/cli/Cargo.toml
    - crates/cli/src/main.rs
    - .github/workflows/ci.yml
    - Cargo.lock

key-decisions:
  - "JSON contract is the Phase 1 → Phase 2 boundary. New struct fields are additive only; camelCase via #[serde(rename_all)] is the long-lived shape."
  - "Inventory aggregator is DB-free. Walks files, runs the segmenter, counts patterns in-memory. Keeps the subcommand cheap (no DB open / migration) and impossible to drift from the segmenter."
  - "Tracing logs go to stderr explicitly. Default tracing_subscriber::fmt() writes to stdout, which silently contaminates JSON consumers. Configured with_writer(io::stderr) — caught by the integration test on the first run."
  - "CI inventory smoke target switched to the synthetic fixture (committed, no PII). The real corpus data-folder-sample/Logseq/ is gitignored and only used locally. Plan revision banner is the canonical wording; this plan implements it."
  - "FTS5 snippet ellipsis is the actual U+2026 char inside the SQL literal. Initial attempt used '\\u{2026}' which Rust passes verbatim into the SQL string — SQLite then prints the literal escape, not the char. Fixed before the integration test commit."

requirements-completed: [IDX-04, IDX-08, ACPT-04]

duration: ~25min
completed: 2026-05-21
---

# Phase 1 Plan 07: CLI Binary + Inventory JSON Contract Summary

**Wired the public-facing `foliom` binary with five clap subcommands (index, reindex, search, dump-tree, inventory), each defaulting to a human-readable terminal table and switching to a serde-driven camelCase JSON contract under `--json`. Built the inventory aggregator that gates parser sign-off (IDX-08), pinned its counts as a CI regression test against the synthetic fixture, and removed `continue-on-error` from the GitHub Actions workflow so drift fails the build.**

## Pinned Inventory Counts (Synthetic Corpus)

Captured 2026-05-21 from `foliom inventory crates/core/tests/fixtures/logseq-synthetic --json`:

| Pattern | Files With | Occurrences |
|---|---:|---:|
| `alias::` | 2 | 2 |
| `id::` | 2 | 2 |
| `template::` | 2 | 2 |
| `LOGBOOK` | 3 | 4 |
| `#[[...]]` | 4 | 6 |
| `SCHEDULED:` | 3 | 3 |
| `DEADLINE:` | 0 | 0 |
| `code-fence-in-bullet` | 3 | 3 |
| `%2F-in-filename` | 1 | 1 |

Plus: `scannedFiles = 11`, `journalFiles = 1`, `pageFiles = 10`, `blockPropertyFiles = 2`, `drawerFiles = 3`, `totalSizeBytes = 5051`.

These are now locked in `crates/cli/tests/cli_integration.rs::EXPECTED_PATTERNS`. Any future parser-semantics change that drifts a count fails CI on this assertion. The DEADLINE count is `0` because the synthetic fixture only exercises SCHEDULED today — adding a DEADLINE fixture will require a re-pin via `FOLIOM_REGEN_INVENTORY=1`.

Real corpus (locally, 620 files): not pinned in CI (PII / gitignored). Real-corpus check still works via `./target/release/foliom inventory data-folder-sample/Logseq --json` as a manual sanity step.

## CI Matrix Status

The workflow is ready for the next push to validate. Three legs: `ubuntu-latest`, `macos-latest`, `windows-latest`. Each runs:
1. `cargo build --workspace --locked`
2. `cargo nextest run --workspace --no-fail-fast` (includes the 4 new CLI integration tests)
3. `cargo build --release --bin foliom --locked`
4. `./target/release/foliom inventory crates/core/tests/fixtures/logseq-synthetic --json > inv.json` + assert `scannedFiles > 0`

The executor cannot directly verify the third leg (Windows) from this WSL session — the workflow file is correct and pushed-equivalent; the next GitHub Actions run is the canonical verifier.

## Performance

- **Duration:** ~25 min active work (4 task commits)
- **Tasks:** 5 (consolidated into 4 commits — Tasks 1 & 2 were both small library-only additions and shared a commit)
- **Files created:** 10 (1 inventory module, 1 query module, 5 cmd modules, cmd/mod.rs, integration test, README)
- **Files modified:** 7 (both Cargo.toml files, lib.rs, indexer/mod.rs derive, main.rs, ci.yml, Cargo.lock)
- **Test suite runtime:** workspace `cargo test --workspace --locked` ~4 s; 121 tests across 11 binaries (was 114 before this plan)

## Task Commits

1. **Tasks 1+2:** `0664ae8` — `feat(01-07): inventory aggregator + query module (JSON contract for CLI)` — inventory.rs + query.rs + serde deps + Serialize derives. 3 inventory unit tests green.
2. **Task 3:** `ae98cd7` — `feat(01-07): foliom CLI binary with five subcommands + --json flag` — clap derive tree, cmd/{index,reindex,search,dump_tree,inventory}.rs, tracing-subscriber on stderr, ReindexStats Serialize derive.
3. **Task 4:** `72a46d4` — `test(01-07): end-to-end CLI integration tests with pinned inventory counts` — 4 assert_cmd-driven tests; inventory regression gate.
4. **Task 5:** `420f780` — `ci(01-07): enforce inventory smoke on synthetic corpus + README` — CI workflow drift-enforced + Phase 1 README.

## Accomplishments

- `foliom --help` lists all five subcommands; each `<subcmd> --help` documents its flags.
- `foliom index <root>` and `foliom reindex <root> [--full]` produce a populated SQLite cache outside the notes folder (D-13).
- `foliom search <root> <query>` joins `blocks_fts` → `blocks` → `pages` → `files` and returns `SearchHit` rows with FTS5 snippets (U+2026 ellipsis, `[…]` match markers).
- `foliom dump-tree <root> <page>` does NOCASE page lookup (D-03) and recursive children-by-parent assembly with depth metadata preserved (prelude depth -1 surfaces correctly).
- `foliom inventory <root>` walks the corpus once via `scanner::walk`, reuses `parser::segment` block-by-block, and emits 9 pattern counts + per-file totals. No DB involvement.
- Every subcommand: human-readable default (aligned table / indented tree) **or** `--json` for the contract output.
- Logs flow to stderr; JSON flows to stdout. The integration tests catch any future regression of this separation.
- 4 CLI integration tests pin the contract end-to-end:
  1. Inventory `--json` output matches PINNED counts exactly (regression gate for parser semantics).
  2. `search "alias"` returns ≥1 hit with all three contract keys (`pagePath`, `blockId`, `snippet`).
  3. `dump-tree 05-links-and-tags` returns ≥1 node with all three contract keys (`depth`, `raw`, `children`).
  4. `reindex` lifecycle: first run adds 11, second run reports unchanged 11, `--full` reports `mtimeTouched 11` (Plan 06 documented this semantics; now exercised end-to-end).
- CI: `continue-on-error: true` is gone from `.github/workflows/ci.yml` (count is 0). The synthetic fixture is the matrix target; the smoke step builds the release binary once and asserts `scannedFiles > 0` via Python (Unix) / PowerShell (Windows) — no jq dependency.
- README documents the five subcommands, JSON contract pointer, per-platform DB locations, round-trip guarantee, and the Windows 11 native / WSL2 cross-OS testing caveat (the latter persisted because it's the primary dev's real testing matrix).

## Decisions Made

### Logs to stderr, JSON to stdout
Default `tracing_subscriber::fmt()` writes to stdout. Caught by the very first reindex-idempotent integration test run, which failed parsing JSON because the first stdout line was `INFO Database migrated to version 1`. Fixed by `.with_writer(std::io::stderr)`. Pattern documented inline in `main.rs` so Phase 2's HTTP server doesn't repeat the mistake.

### Pure-data inventory aggregator
`inventory_report` does not call `Db::open`. It walks the filesystem once with `scanner::walk`, segments each file with `parser::segment`, and counts patterns in memory. Rationale: the inventory subcommand is meant to be fast and side-effect-free (e.g. running it on a notes folder shouldn't create a stale cache under `$XDG_DATA_HOME` just for the diagnostic). Also keeps the aggregator faithful to what the indexer would actually store, because it reuses the exact same segmenter — no risk of two parallel "what counts as a LOGBOOK drawer?" definitions drifting.

### Code-fence-in-bullet counted once per block
The plan said "occurrences", but every non-prelude block whose `raw` contains ` ``` ` is counted once (not per-fence-pair). Rationale: the segmenter has already balanced fences; a block containing a fence is *by construction* one bullet that owns one or more fenced regions. Counting fence pairs would require re-parsing, which contradicts the "reuse segmenter output" principle. The pinned count is the *block* count, which is the meaningful gate metric: "how many bullets own a code fence" is what RF-21 / PRS-04 are actually about.

### CI uses Python (not jq) for the smoke assertion
Three options were considered: install `jq` on every matrix leg (Windows extra step), use `cargo run` against a tiny one-off Rust helper, or use `python3` which is preinstalled on `ubuntu-latest`/`macos-latest` and `pwsh` (which has `ConvertFrom-Json`) on `windows-latest`. The Python route was simplest and added zero new install steps to the matrix.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] FTS5 snippet ellipsis was a literal `\u{2026}` string**
- **Found during:** Task 3 manual smoke test (`foliom search ... | head`)
- **Issue:** The SQL string in `query.rs` had `'\u{2026}'` as the FTS5 snippet ellipsis. Rust passes that 8-byte text verbatim into the SQL literal; SQLite then prints it as `\u{2026}` instead of the ellipsis character.
- **Fix:** Replaced with the literal `…` (U+2026) inside the SQL string. Rust source-file is already UTF-8.
- **Files modified:** `crates/core/src/query.rs`
- **Committed in:** `ae98cd7` (Task 3 commit) — caught before commit so no fix-up commit needed.

**2. [Rule 1 — Bug] tracing logs polluting JSON stdout**
- **Found during:** Task 4 first integration-test run (`test_reindex_idempotent_unchanged_count` panicked on `serde_json::from_slice` because stdout's first line was `INFO Database migrated to version 1`).
- **Issue:** `tracing_subscriber::fmt().init()` writes to stdout by default. With `--json` set, every CLI invocation prefixed the JSON output with a log line.
- **Fix:** Added `.with_writer(std::io::stderr)` to the subscriber builder. Documented inline so the pattern survives into Phase 2's HTTP server.
- **Files modified:** `crates/cli/src/main.rs`
- **Committed in:** `72a46d4` (Task 4 commit, alongside the integration test that caught it).

### Plan Revision Honored

Per the REVISION 2026-05-21 banner at the top of PLAN.md: all CI inventory pins target the synthetic corpus (`crates/core/tests/fixtures/logseq-synthetic/`), not the real PII corpus. The integration test uses `synthetic_corpus()` exclusively; the workflow YAML targets the same path. No hardcoded `data-folder-sample/Logseq/` paths remain in CI or tests.

### CLAUDE.md Compliance

`./CLAUDE.md` mandates "GSD workflow enforcement". This executor IS the GSD workflow — Plan 01-07 from the planning phase. Confirmed in scope. The forced `cargo` / `rustc` / `clap` choice in this plan matches CLAUDE.md's recommended Rust stack with HIGH confidence.

## clippy Status

`cargo clippy --workspace --no-deps -- -D warnings` fails — **but every error is pre-existing** in `crates/core/src/parser/{ast,segment}.rs`, `crates/core/src/scanner/walk.rs`, and `crates/core/src/storage/{location,mod}.rs`. None are introduced by this plan. Per the executor's scope boundary rule, pre-existing lints in unrelated files are out of scope; logged for a future cleanup pass.

My new code (`inventory.rs`, `query.rs`, all `cmd/*.rs`, `main.rs`, integration test) produces zero clippy errors or warnings on Rust 1.95 stable.

## Verification Results

- `cargo test --workspace --locked` → **121 passed; 0 failed** across 11 binaries (was 114 before this plan).
- `cargo build --release --bin foliom` → succeeds in ~1 min cold, ~3 s warm.
- `foliom --help` → lists all 5 subcommands.
- `foliom inventory crates/core/tests/fixtures/logseq-synthetic --json` → returns the pinned report.
- `foliom search crates/core/tests/fixtures/logseq-synthetic "alias" --json` → returns hits with the contract keys.
- `foliom dump-tree crates/core/tests/fixtures/logseq-synthetic "05-links-and-tags" --json` → returns the tree.
- `foliom reindex crates/core/tests/fixtures/logseq-synthetic --full --json` → reports `mtimeTouched: 11` after a fresh index.
- `.github/workflows/ci.yml` → `continue-on-error: true` count is **0**.
- README.md exists at repo root with the five subcommand examples and Phase 1 status line.
- AP-2 guard (no serialize/to_markdown/format_block): green (`grep -rE` returns no matches).

## TDD Gate Compliance

All five tasks were marked `tdd="true"`. Sequence:

- **Task 1 (inventory):** unit tests co-located in `inventory.rs` (3 tests — smoke, pattern detection, count-occurrences-edge-cases) committed in the same commit as the implementation. Same fold pattern as Plan 04/06.
- **Task 2 (query):** library code only; behavior exercised by Task 4's integration test (search + dump-tree end-to-end). No standalone unit tests.
- **Task 3 (CLI):** subcommand glue; exercised by Task 4. Manual smoke during dev (`foliom --help`, `foliom inventory ...`).
- **Task 4 (integration test):** THE behavioral spec for the whole plan. 4 tests that drive the binary end-to-end; commit `72a46d4` is effectively the GREEN gate for Tasks 2-4 collectively.
- **Task 5 (CI + README):** docs/CI changes — no behavioral test (the GitHub Actions run is the canonical verifier).

Plan-level gate sequence (per Plan 04 / Plan 06 precedent when test depends on existing impl surface): test + impl co-located in the same commit when the test cannot compile without the impl. Acceptable.

## Phase 1 Retrospective

Phase 1 (Headless Indexing Core) closes here. Seven plans:

1. **01-01** — Cargo workspace scaffold, edition 2024, CI matrix baseline, CLI stub.
2. **01-02** — Stage 1 segmenter (line-based, fence-aware, drawer-aware, TAB + 2-space continuation).
3. **01-03** — Path normalization (`RelativePath`: NFC + forward-slash boundary).
4. **01-04** — SQLite storage layer (schema + migrations + FTS5 triggers).
5. **01-05** — Scanner walk + ignore list + minimal `config.edn :hidden` reader.
6. **01-06** — Indexer orchestrator: per-file transactional writes, hash-resolved divergence, single-pass page discovery.
7. **01-07** — CLI binary with five subcommands + --json contract + inventory regression gate + CI enforcement (this plan).

### RESEARCH assumptions that held
- **A1: ~619 markdown files in the real corpus.** Confirmed at 620 in Plan 06's smoke test.
- **A4: pulldown-cmark `into_offset_iter()` gives reliable byte spans.** Confirmed across all PRS-* tests.
- **A5: rusqlite `bundled-full` ships FTS5.** Confirmed at the migration stage; no system-SQLite linkage needed.
- **A7: walkdir's `filter_entry` prunes ignored subtrees before descent.** Confirmed via the scanner unit tests.
- **A8: BLAKE3 is fast enough to hash every file during a Full reindex.** Confirmed: 620-file Full reindex in ~1.3 s on the real corpus (Plan 06 stat).

### RESEARCH assumptions that needed refinement
- **A2: "10 small files" in the synthetic fixture.** Actually 11 — the corpus root README.md is also a `.md` file the scanner walks. Plan 06 documented this; Plan 07's pins use 11 throughout.
- **A3: "Stage 2 CommonMark per block".** Not yet implemented — the parser surface today is Stage 1 segmenter only + per-block regex/event extraction for `[[link]]` / `#tag` / `#[[composite]]`. Stage 2 (full CommonMark per block) is deferred to Phase 2's renderer, where it actually matters. No correctness loss in Phase 1.

### PITFALLS that bit during execution
- **Tracing-on-stdout-contaminates-JSON.** Not in PITFALLS.md; should be added. Bit on the first integration test.
- **rusqlite::ToSql not implemented for u64** (Plan 06).
- **HashSet<(K, V)> requires both Hash** (Plan 06).
- **`'\u{2026}'` in SQL is verbatim text, not escape.** Specific to Rust string handling at the SQL boundary; could go to PITFALLS for the watcher/server work.

### What flows into Phase 2
- The JSON contract (`InventoryReport`, `SearchHit`, `BlockNode`, `ReindexStats`) is now frozen and immutable except for additive field additions.
- The CLI's tracing-to-stderr pattern is documented inline; Phase 2's HTTP server (axum) must do the same when it emits SSE/WebSocket events alongside structured logs.
- The synthetic corpus is the canonical CI target; Phase 2's watcher tests should use it too, and only opt into the real corpus locally.

## Threat Flags

None new. Threat register entries from the plan are addressed:

- **T-07-01 (FTS5 query injection / DoS):** Accept disposition. Query goes through `?` parameter; malformed FTS5 returns a SQLite error which surfaces cleanly. No SQL injection vector.
- **T-07-02 (Path traversal):** Mitigated by `RelativePath::from_filesystem` rejecting `..` components (verified in Plan 03 tests). CLI does not write outside `XDG_DATA_HOME`.
- **T-07-03 (JSON output discloses file paths):** Accept disposition. Single-user local app.
- **T-07-04 (Empty FTS5 query):** Mitigated. `search_blocks` early-returns `Vec::new()` on empty/whitespace input.
- **T-07-05 (JSON contract breakage):** Mitigated. Structs documented in README as additive-only; cross-phase consumers can rely on the existing field set.
- **T-07-SC (supply chain):** All new deps are Rust ecosystem cornerstones (`clap`, `serde`, `serde_json`, `anyhow`, `tracing-subscriber`, `assert_cmd`). No suspect packages.

## Known Stubs

None. Every subcommand is fully wired with real implementations. The aggregator, query helpers, and CLI dispatch all flow to real data. No `todo!()`, no hardcoded empty results, no placeholder return values.

## Self-Check: PASSED

- File `crates/core/src/inventory.rs` — present (created)
- File `crates/core/src/query.rs` — present (created)
- File `crates/cli/src/cmd/mod.rs` — present (created)
- File `crates/cli/src/cmd/index.rs` — present (created)
- File `crates/cli/src/cmd/reindex.rs` — present (created)
- File `crates/cli/src/cmd/search.rs` — present (created)
- File `crates/cli/src/cmd/dump_tree.rs` — present (created)
- File `crates/cli/src/cmd/inventory.rs` — present (created)
- File `crates/cli/tests/cli_integration.rs` — present (created)
- File `README.md` — present (created)
- Commit `0664ae8` — present in `git log` (Tasks 1+2)
- Commit `ae98cd7` — present in `git log` (Task 3)
- Commit `72a46d4` — present in `git log` (Task 4)
- Commit `420f780` — present in `git log` (Task 5)
- `cargo test --workspace --locked` — green (121 tests across 11 binaries)
- AP-2 guard — clean (no `serialize`/`to_markdown`/`format_block` functions)
- `continue-on-error: true` in ci.yml — count is 0
- README mentions `foliom inventory` — confirmed

---
*Phase: 01-headless-indexing-core*
*Plan: 07*
*Completed: 2026-05-21*
