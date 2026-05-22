---
phase: 01-headless-indexing-core
plan: 06
subsystem: indexer
tags: [rust, indexer, sqlite, blake3, transactional, idx-02, idx-03, idx-04, idx-05, idx-07, prs-04, prs-05, prs-06]

requires: [01-02, 01-03, 01-04, 01-05]
provides:
  - "crates/core/src/indexer/mod.rs — public surface: reindex(), ReindexMode, ReindexStats, IndexerError"
  - "crates/core/src/indexer/page_name.rs — derive_page_info(): %2F decode (case-insensitive), journal YYYY_MM_DD → ISO YYYY-MM-DD, NFC-passthrough via RelativePath"
  - "crates/core/src/indexer/write.rs — insert_file_tx / reparse_file_tx / delete_file_cascade / update_file_mtime + reindex_impl orchestrator"
  - "crates/core/tests/indexer_integration.rs — 12 end-to-end scenarios pinning the contract: first-pass, idempotence, mtime-touch, content-modify, file-delete, Full-mode, DB-delete-rebuild, page-05 refs, page-06 false-positives, journal kind, %2F page name, opt-in real corpus"
  - "Dev-only filetime 0.2 dep for mtime manipulation in tests"
affects: [01-07-cli, 01-08-inventory]

tech-stack:
  added:
    - "filetime 0.2 (dev-only) — cross-platform mtime setter for the mtime-touch regression test"
  patterns:
    - "Per-file SQLite transaction (AP-5): each file is its own tx; failure rolls back that file only, errors logged via tracing::warn! and indexing continues"
    - "Hash-resolved divergence: (mtime, size) fast-path for unchanged; on mismatch read+BLAKE3, then route to mtime_touched (hash matches) vs modified (hash differs)"
    - "ensure_self_page_row: NOCASE lookup → claim unresolved (file_id=NULL) row OR INSERT; warns on case-collision (two real files with same NOCASE name)"
    - "ensure_unresolved_page: D-04 — INSERT with file_id=NULL when a ref target has no backing file yet; backlinks resolve from day 1"
    - "Depth-stack parent_id derivation: walk blocks in source order, pop entries with depth ≥ new.depth, parent = stack.top.id or NULL, push new block"
    - "Per-block extract_refs only (AP-1) — never on whole-file text; per-block HashSet dedups duplicate (kind, target) before INSERT OR IGNORE"
    - "Single-pass orchestration: no separate page-discovery pass because ensure_unresolved_page creates rows on-demand inside the same transaction (RESEARCH Open Question 1 — answered single-pass)"

key-files:
  created:
    - crates/core/src/indexer/mod.rs
    - crates/core/src/indexer/page_name.rs
    - crates/core/src/indexer/write.rs
    - crates/core/tests/indexer_integration.rs
  modified:
    - crates/core/src/lib.rs
    - crates/core/src/parser/ast.rs
    - crates/core/Cargo.toml
    - Cargo.lock

key-decisions:
  - "Single-pass page discovery (RESEARCH Open Question 1): ensure_unresolved_page creates pages.id rows with file_id=NULL on first reference; when the backing file is later processed (in the same reindex), ensure_self_page_row claims the row by UPDATEing file_id. No second walk needed."
  - "Self-page collisions: when two real files (e.g. case-sensitive FS) hash to the same NOCASE page name, we log a warning and reuse the existing row; we do NOT error out. Phase 2 will surface this as a user-visible diagnostic if it ever fires."
  - "delete_file_cascade DELETEs the page row (not just the file row) for files that vanish from disk. Rationale: orphaned page-with-no-blocks is dead weight; if any other file links to that page, the next reindex's extract_refs will recreate it as unresolved."
  - "Full mode on unchanged corpus reports mtime_touched, not unchanged. Rationale: Full mode is defined as 'skip the (mtime,size) fast-path' — every file is read+hashed; when the hash matches, mtime_touched is the only stat that makes sense."
  - "Synthetic fixture count is 11, not 10 — the corpus root has a top-level README.md sibling to pages/ and journals/. The scanner walks all .md files under the root."
  - "u64 size cast to i64 at the rusqlite boundary (no `ToSql` impl for u64). Files >2^63 bytes would overflow; tracked as a non-issue for a notes app."

requirements-completed: [IDX-02, IDX-03, IDX-04, IDX-05, IDX-07, PRS-04, PRS-05, PRS-06]

duration: ~25min
completed: 2026-05-21
---

# Phase 1 Plan 06: Indexer Orchestrator + Per-File Transactional Writes Summary

**`reindex(db, root, mode)` stitches scanner + parser + storage together: walks `.md` files, diffs against the cached `files` table on `(mtime_ns, size)`, recomputes BLAKE3 on mismatch, and writes per-file in its own SQLite transaction. Distinguishes `mtime_touched` (hash matches cache) from `modified` (hash differs). Handles inserts, deletes, full re-reads, and cross-file `[[link]]` / `#tag` resolution to the same NOCASE page row (D-03) including unresolved D-04 pages with `file_id = NULL`.**

## Real-Corpus Stats on This Dev Machine

Running `cargo test real_corpus_smoke_if_present -- --nocapture` against `data-folder-sample/Logseq/` (gitignored, locally present on this dev machine):

```
ReindexStats { scanned: 620, added: 620, modified: 0, unchanged: 0, mtime_touched: 0, deleted: 0 }
```

620 files indexed on first pass; second pass reports `added: 0, modified: 0, unchanged: ...` (idempotent). Confirms assumption A1 (~619 ± 1) holds against the real base. Test runtime for the real-corpus first pass: ~1.3 s (dominated by per-file BLAKE3 + segmenter + ref extraction inside transactions).

## Performance

- **Duration:** ~25 min active work (3 task commits + this metadata commit)
- **Tasks:** 3 (page_name + scaffold, write helpers + orchestrator, integration test)
- **Files created:** 4 (`indexer/mod.rs`, `indexer/page_name.rs`, `indexer/write.rs`, `tests/indexer_integration.rs`)
- **Files modified:** 4 (`Cargo.toml`, `src/lib.rs`, `src/parser/ast.rs`, `Cargo.lock`)
- **Test suite runtime:** synthetic-corpus integration (12 tests) ~1.3 s; full workspace `cargo test --workspace --locked` ~3.5 s

## Task Commits

1. **Task 1:** `141c373` — `feat(01-06): page-name derivation + indexer module scaffold` (10 page_name unit tests + indexer module surface + stubs for Tasks 2-3)
2. **Task 2:** `b9c9d58` — `feat(01-06): per-file transactional write helpers + reindex orchestrator` (insert_file_tx / reparse_file_tx / delete_file_cascade / update_file_mtime + reindex_impl)
3. **Task 3:** `9dd4518` — `feat(01-06): end-to-end indexer integration test (12 scenarios)` (filetime dev-dep + 12 integration tests)

## Accomplishments

- `reindex(&mut Db, &Path, ReindexMode) -> Result<ReindexStats, IndexerError>` is fully wired and exercised end-to-end against the synthetic + real corpora.
- `derive_page_info`: 10 unit tests cover journal vs page, %2F decode (both cases), NFC accents, no-parent-dir, malformed journal dates, missing extension.
- `insert_file_tx` / `reparse_file_tx` / `delete_file_cascade` are the three public write helpers; all expect to run inside an already-open `Transaction` (AP-5).
- `insert_all_blocks` walks `segment(bytes)` output with a depth stack so `blocks.parent_id` is correctly populated for nested bullets.
- `insert_refs_for_block` is the AP-1-compliant ref pipeline: `extract_refs(&block.raw)` only (never on whole files), per-block dedup via HashSet, `ensure_unresolved_page` resolves the target to a `pages.id`, then `INSERT OR IGNORE INTO refs`.
- `ensure_self_page_row` claims unresolved page rows (D-04) on first backing-file insert and warns on case-collisions (D-03 boundary).
- D-03 satisfied: NOCASE collation on `pages.name` + `ensure_unresolved_page` looking up by `COLLATE NOCASE` mean `#Crypto` and `[[Crypto]]` always resolve to the same row.
- D-04 satisfied: `ensure_unresolved_page` INSERTs with `file_id = NULL` until/unless a backing file appears.
- D-14 satisfied: every block row has `raw`, `byte_offset`, `byte_length` populated.
- `blocks_fts` row count tracks `blocks` row count after every reindex (triggers wired by Plan 04 fire correctly through this code path).
- 12 integration tests pin the contract:
  1. First-pass adds every file (`stats.added == SYNTHETIC_FILE_COUNT = 11`)
  2. Second pass is idempotent (`stats.unchanged == 11`)
  3. mtime-touch without content change → `mtime_touched == 1`
  4. Content change → `modified == 1`, blocks replaced
  5. File delete → `deleted == 1`, blocks/refs CASCADE
  6. Full mode on unchanged corpus → `mtime_touched == 11`, row counts preserved
  7. Delete DB + reopen + reindex → reproduces baseline row counts
  8. Page 05 emits expected page-links + tags
  9. Page 06 false-positives (hex `#fff`/`#1a2b3c`, URL `#section-anchor`, in-code, in-fence) do NOT leak into refs
  10. Journal page gets `kind='journal'` and `journal_date='2024-03-15'`
  11. `Parent%2FChild.md` becomes page name `Parent/Child`
  12. Real-corpus smoke (opt-in, skipped silently in CI) — confirms 620 files index without panic

## Distinct Page / Ref Counts

Against the synthetic fixture (after first reindex):

- `SELECT COUNT(*) FROM files` → 11 (10 fixture files + README.md)
- `SELECT COUNT(*) FROM pages WHERE file_id IS NOT NULL` → 11 (one self-page per file)
- `SELECT COUNT(*) FROM pages WHERE file_id IS NULL` → distinct unresolved ref targets across all fixtures (varies by fixture content — verified via test `page_05_emits_expected_page_links_and_tags`).

For the real corpus (~620 files, locally measured): see real-corpus stats block above. Distinct page / tag / link counts will be reported by Plan 01-08's `inventory` subcommand, which is the canonical sink for those metrics.

## Decisions Made

### Single-pass page discovery (RESEARCH Open Question 1)

Chose single-pass over two-pass. Reasoning: `ensure_unresolved_page` INSERTs `pages` rows on demand with `file_id = NULL`. When file B's `[[Page A]]` ref is processed before file A is walked, the page row exists but is unresolved. When file A is later processed, `ensure_self_page_row` finds the existing NOCASE row and UPDATEs its `file_id`. The end-state is identical to two-pass discovery, with one less full corpus walk. Tested by `delete_db_and_rebuild_reproduces_row_counts` — order-independent.

### `delete_file_cascade` removes the page row too

When a file is deleted from disk, we DELETE the `pages` row (CASCADE clears blocks/refs/FTS) rather than relying on the `ON DELETE SET NULL` from `files → pages`. Rationale: an orphaned page row with no blocks and no incoming refs is dead weight; if another file mentions it, the next reindex's `extract_refs` recreates it as unresolved. This keeps the index lean and prevents stale page rows accumulating across delete/recreate cycles.

### Full mode semantics

Per-stat definition: Full mode skips the `(mtime, size)` fast path entirely — every file is read + hashed. When the recomputed hash matches the cached hash, the row is `mtime_touched` (we still UPDATE the mtime field even if it didn't actually change — defensive, since size could have changed too). When the hash differs, it's `modified`. When the file wasn't in the cache, it's `added`. This makes Full mode's stats discriminate "I re-verified everything" from Incremental's "I trusted (mtime, size)".

### `u64` size cast to `i64`

`rusqlite` does not implement `ToSql for u64`. SQLite integers are signed 64-bit. Cast `size as i64` at every call site. Files over 2^63 bytes would silently truncate; tracked as a non-issue for a markdown notes app.

### `RefKind` and `ExtractedRef` derive `Hash`

Required for the per-block dedup `HashSet<(RefKind, String)>` in `insert_refs_for_block`. Trivial derive, no semantic change.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Synthetic fixture count assumed to be 10, but is 11**
- **Found during:** Task 3 first integration-test run
- **Issue:** Plan/executor brief assumed `SYNTHETIC_FILE_COUNT = 10` (matching the fixture-count claim in `01-CONTEXT.md` D-08). The fixture root also contains a top-level `README.md`, which the scanner correctly walks. 7 of 12 tests failed with `expected 10, got 11`.
- **Fix:** Updated `SYNTHETIC_FILE_COUNT = 11` and documented inline that the README is a legitimate `.md` file that participates in indexing. CONTEXT D-08 says "10 small files isolating a pattern each" — that count remains accurate for the *pattern* fixtures; the README is purely descriptive.
- **Files modified:** `crates/core/tests/indexer_integration.rs`
- **Committed in:** `9dd4518` (Task 3 commit)

**2. [Rule 3 — Blocking issue] `RefKind` lacked `Hash` derive**
- **Found during:** Task 2 first `cargo build`
- **Issue:** `insert_refs_for_block` uses a `HashSet<(RefKind, String)>` for dedup; without `Hash` on `RefKind` the build fails.
- **Fix:** Added `Hash` to the existing `derive(Debug, Clone, PartialEq, Eq)` on both `RefKind` and `ExtractedRef` in `crates/core/src/parser/ast.rs`. No semantic change.
- **Files modified:** `crates/core/src/parser/ast.rs`
- **Committed in:** `b9c9d58` (Task 2 commit)

**3. [Rule 3 — Blocking issue] `u64: ToSql` not satisfied**
- **Found during:** Task 2 first `cargo build`
- **Issue:** `rusqlite::ToSql` is not implemented for `u64` (SQLite ints are signed i64). `params![size, …]` failed at three callsites.
- **Fix:** Cast `size as i64` at each rusqlite boundary. Documented in decisions.
- **Files modified:** `crates/core/src/indexer/write.rs`
- **Committed in:** `b9c9d58` (Task 2 commit)

**4. [Rule 1 — Bug] `read_hidden` returns `Vec<String>`, not `Result`**
- **Found during:** Task 2 first `cargo build`
- **Issue:** Wrote `match read_hidden(&path) { Ok(extra) => ..., Err(...) }` — but `read_hidden` returns `Vec<String>` directly.
- **Fix:** Replaced with `let extra = read_hidden(&path); if !extra.is_empty() { ignore.extend_from_config_edn(extra); }` and dropped the spurious error branch.
- **Files modified:** `crates/core/src/indexer/write.rs`
- **Committed in:** `b9c9d58` (Task 2 commit)

### CLAUDE.md Compliance

`./CLAUDE.md` mandates "GSD workflow enforcement: do not make direct repo edits outside a GSD workflow". This executor IS the GSD workflow — Plan 01-06 from the planning phase. Confirmed in scope.

No other deviations. Plan body executed as written modulo the four auto-fixes above.

## rusqlite 0.39 / API Gotchas Discovered

1. **`ToSql` is not implemented for `u64`.** SQLite stores 64-bit signed ints. Pass `size as i64` at the boundary; assume sizes fit (they always will for a notes app).
2. **`if let Some(x) = something.ok()` triggers clippy `redundant_pattern_matching`.** Use `if let Ok(x) = something` instead — same semantics, no allocation.
3. **`tx.last_insert_rowid()` returns the rowid of the most recent INSERT on that connection inside the current transaction.** Safe to call immediately after each INSERT — no race with concurrent writers because SQLite is single-writer.
4. **`Connection::transaction()` borrows `&mut self`.** `Db::conn_mut().transaction()` is the right access pattern; cannot hold a `&Connection` reference alongside a transaction.

## Pitfalls Discovered

- **Fixture README.md gets indexed.** Any test that pins file counts must include the README. Documented for future fixture changes.
- **Full mode + unchanged corpus reports `mtime_touched`, not `unchanged`.** The natural intuition is "Full mode on unchanged corpus → unchanged == file count" but that contradicts the *meaning* of Full mode ("skip the fast path"). Document this so Plan 01-07's CLI prints the right human-readable summary.
- **`HashSet<(K, V)>` requires both K and V be `Hash`.** Adding the derive to internal enum types is cheap; flag in code review when introducing dedup logic.

## Verification Results

- `cargo test --test indexer_integration --package foliom-core` — exits 0 (`12 passed`)
- `cargo test --package foliom-core --lib indexer::page_name` — exits 0 (`10 passed`)
- `cargo test --workspace --locked` — fully green: **114 tests** across 9+ binaries
  - 33 (lib unit including indexer::page_name) + 24 (ast_unit) + 12 (indexer_integration) + 9 (path_unit) + 9 (storage_integration) + 2 (roundtrip) + 10 (scanner_unit) + 15 (segment_unit) + 0 (server) + 0 (doctests) = 114
- AP-1 guard: `extract_refs` only called inside `insert_refs_for_block` (per-block) — verified via `grep -n extract_refs crates/core/src/indexer/write.rs`
- AP-2 guard: `grep -rE "fn (serialize|to_markdown|format_block)" crates/` — empty
- AP-5 guard: `db.conn_mut().transaction()` is called once per file inside the orchestrator loop, never wrapping the entire reindex
- Real-corpus smoke: 620 files indexed without panic; second pass idempotent

## TDD Gate Compliance

All three tasks were marked `tdd="true"`. Sequence in git log:

- **Task 1 (page_name + scaffold):** unit tests + implementation co-located (`#[cfg(test)] mod tests` in `page_name.rs`); single `feat(01-06)` commit `141c373` covers both. Same fold-pattern Plan 04 used for storage tests.
- **Task 2 (write helpers + orchestrator):** library code only; behavior exercised by Task 3's integration test. Single `feat(01-06)` commit `b9c9d58`.
- **Task 3 (integration test):** test file IS the behavioral spec; commit `9dd4518` adds the test file simultaneously with the dev-dep that makes it run (`filetime`). Since the implementation already exists from Task 2, this commit is effectively the GREEN gate for the plan as a whole.

The plan-level gate sequence is: RED was implicit (every test file written would have failed without Tasks 1+2); GREEN is the union of all three commits. No standalone RED commits were produced because the indexer's behavioral spec is the *integration* test, which depends on the entire write pipeline existing. Acceptable per Plan 04 / Plan 02 precedent (test + impl co-located when the test cannot compile without the impl's public surface).

## Threat Flags

None new. Threat register entries from the plan are addressed:

- **T-06-01 (SQL injection):** All inserts use `params![]` bindings throughout `write.rs`. No `format!` of SQL strings. Reviewed via `grep -n 'format!.*INSERT\|format!.*SELECT\|format!.*UPDATE' crates/core/src/indexer/` — empty.
- **T-06-02 (DoS via giant file):** Accept disposition; per-file transaction means a giant file's failure only rolls back that file.
- **T-06-03 (TOCTOU scanner→read):** Accept disposition; documented in RESEARCH §Security Domain.
- **T-06-04 (kill -9 mid-reindex):** Per-file transaction means a SIGKILL between file boundaries leaves the index in a consistent state with all completed files durable.
- **T-06-05 (path leak in logs):** Accept disposition; logs are local single-user.
- **T-06-06 (DB delete mid-reindex):** rusqlite holds open handle; on Unix the file is unlinked but still operable, on Windows the delete fails. Either way the in-flight reindex completes or errors with the transaction rolled back.
- **T-06-SC (supply chain):** One new dev-dep, `filetime 0.2` — standard cross-platform mtime setter, ~10M downloads/month, BSD-3-Clause license. HIGH legitimacy. No runtime deps added.

## Next Plan Readiness

- **Plan 01-07 (CLI):** Consumes `indexer::reindex(&mut db, &root, ReindexMode::{Incremental, Full})`. CLI subcommands `index <root>` (Full on first run) and `reindex` (Incremental on demand) map directly. JSON output uses `ReindexStats` — already derives Debug/Clone/PartialEq/Eq; add `serde::Serialize` when Plan 07 needs it.
- **Plan 01-08 (inventory):** Queries the now-populated `block_props`, `block_drawers`, `refs`, `pages.kind` directly. Distinct-tag count is `SELECT COUNT(*) FROM tags`. Distinct-page count is `SELECT COUNT(*) FROM pages`. Backlinks are `SELECT COUNT(*) FROM refs WHERE target_page = ? GROUP BY source_block`.
- **Phase 2 (watcher):** When the FS watcher fires for a single file, the indexer surface is already factored — call `reparse_file_tx` (or `insert_file_tx` for new files, `delete_file_cascade` for removes) inside a one-off transaction. No reindex-loop overhead needed for live edits.

## Known Stubs

None. `reindex`, `derive_page_info`, and all three write helpers are fully wired with no placeholder values, no `todo!()`, no hardcoded empties flowing to consumers.

## Self-Check: PASSED

- File `crates/core/src/indexer/mod.rs` — present (created)
- File `crates/core/src/indexer/page_name.rs` — present (created)
- File `crates/core/src/indexer/write.rs` — present (created)
- File `crates/core/tests/indexer_integration.rs` — present (created)
- Commit `141c373` — present in `git log` (Task 1)
- Commit `b9c9d58` — present in `git log` (Task 2)
- Commit `9dd4518` — present in `git log` (Task 3)
- `cargo test --workspace --locked` — green (114 tests across 9+ binaries)
- AP-1 guard — `extract_refs` only inside `insert_refs_for_block` per-block loop
- AP-2 guard — clean (no `serialize`/`to_markdown`/`format_block` functions)
- AP-5 guard — one transaction per file in the orchestrator loop
- `filetime` in `[dev-dependencies]` — confirmed
- Real-corpus smoke (620 files) — passes locally

---
*Phase: 01-headless-indexing-core*
*Plan: 06*
*Completed: 2026-05-21*
