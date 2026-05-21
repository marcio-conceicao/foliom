---
phase: 01-headless-indexing-core
plan: 04
subsystem: storage
tags: [rust, sqlite, fts5, migrations, rusqlite, blake3, idx-02, idx-05, idx-06]

requires: [01-01]
provides:
  - "crates/core/src/storage/migrations/001_init.sql — full Phase 1 schema (files, pages, blocks, block_props, block_drawers, tags, refs) + blocks_fts virtual table + 3 triggers"
  - "crates/core/src/storage/location.rs — resolve_db_path(notes_root) places .db under per-OS data dir keyed by BLAKE3 of NFC + forward-slash absolute path"
  - "crates/core/src/storage/mod.rs — Db { open(notes_root), open_at(db_path), conn(), conn_mut(), transaction() } with PRAGMA setup + rusqlite_migration application"
  - "StorageError unified error type (Io / Sqlite / Migration / NoHomeDir / NoAppData / PathResolution)"
  - "9 storage_integration tests + 6 storage::location unit tests pinning the contract"
affects: [01-05-scanner, 01-06-indexer, 01-07-cli, 01-08-inventory]

tech-stack:
  added:
    - "rusqlite 0.39 (feature `bundled-full`) — bundles SQLite C with FTS5/JSON1/RTree compiled in (A7 belt-and-suspenders)"
    - "rusqlite_migration 2.5 — schema versioning via PRAGMA user_version; replay-safe to_latest()"
    - "blake3 1.5 — root-hash for DB filename (D-16)"
    - "tracing 0.1 — D-18; declared but not yet emitted (instrumentation in Plan 01-06)"
    - "tempfile 3 (dev) — TempDir for env-var dance + per-test scratch dirs"
  patterns:
    - "Migration file is `include_str!`d as a single text blob → applied via `Migrations::new(vec![M::up(...)])`"
    - "PRAGMA batch via `execute_batch` on every connection open (configure_connection)"
    - "OnceLock<Migrations<'static>> rather than literal `static` since `M::up` borrows `'static str`s from include_str! (LazyLock works too — chose OnceLock for stdlib-1.70 compatibility)"
    - "FTS5 external-content with `content='blocks', content_rowid='id'` — canonical text stays in blocks.raw, only inverted index lives in blocks_fts"
    - "Three triggers (blocks_ai/blocks_ad/blocks_au) keep FTS5 in lockstep; UPDATE = delete-then-reinsert"
    - "Env-var-mutating tests serialized via process-wide Mutex (`ENV_LOCK`) so default-parallel `cargo test` doesn't race XDG_DATA_HOME"

key-files:
  created:
    - crates/core/src/storage/mod.rs
    - crates/core/src/storage/location.rs
    - crates/core/src/storage/migrations/001_init.sql
    - crates/core/tests/storage_integration.rs
  modified:
    - crates/core/Cargo.toml
    - crates/core/src/lib.rs
    - Cargo.lock

key-decisions:
  - "rusqlite_migration 2.5 instead of CONTEXT D-20's '1.3' — 1.3 transitively pins rusqlite ^0.32 which conflicts at libsqlite3-sys native-link with our 0.39. 2.5 supports rusqlite 0.39 and keeps the same M::up + Migrations::new API."
  - "KEPT the `tags` table for symmetry with refs.type and future query patterns (RESEARCH note explicitly leaves the choice open; cost is one usually-empty table)"
  - "block_props as separate table (NOT a JSON column on blocks) per RESEARCH recommendation — enables indexed queries like 'list all blocks with template:: set' without JSON1"
  - "Hand-rolled per-OS data_dir() (not the `directories` crate) — keeps the dep surface small and makes the XDG_DATA_HOME override behavior explicit (CI tests rely on it)"
  - "ENV_LOCK Mutex in storage::location tests — env mutation across parallel tests in the same process flakes intermittently; serialization is the minimum-surface fix"
  - "Used OnceLock<Migrations> instead of `static MIGRATIONS: Migrations` to sidestep the const-fn boundary on the Vec<M>; same observable behavior"
  - "bundled-full feature on rusqlite (NOT bare `bundled`) — A7 'belt and suspenders' from the threat-model: explicit FTS5/JSON1/RTree even if default `bundled` would also enable them today"

requirements-completed: [IDX-02, IDX-05, IDX-06]

duration: ~25min
completed: 2026-05-21
---

# Phase 1 Plan 04: Storage Foundation (Schema + Migrations + DB Location) Summary

**SQLite schema for the entire Phase 1 — files, pages, blocks (with `raw` AND `byte_offset`/`byte_length` for byte-splice writeback), block_props, block_drawers, tags, refs, and FTS5 external-content over blocks.raw with INSERT/UPDATE/DELETE triggers — applied via a single `rusqlite_migration` step on a SQLite DB whose path is deterministically derived from the notes-root via BLAKE3 of its NFC + forward-slash absolute form, placed under the per-OS data directory (NEVER inside the notes folder per IDX-06).**

## Resolved DB Path on This Dev Machine

`/home/mconceicao/.local/share/foliom/<16-hex>.db` (Linux; `$XDG_DATA_HOME` unset so the resolver falls back to `$HOME/.local/share/foliom/`). For example, with the synthetic test corpus root, the hash would be the first 16 hex chars of BLAKE3 over the NFC-normalized absolute path of `crates/core/tests/fixtures/logseq-synthetic/`.

## Performance

- **Duration:** ~25 min active work (3 task commits + final metadata commit)
- **Tasks:** 3 (Task 1 SQL migration, Task 2 location resolver, Task 3 Db wrapper)
- **Files created:** 4 (mod.rs, location.rs, migrations/001_init.sql, tests/storage_integration.rs)
- **Files modified:** 3 (Cargo.toml, lib.rs, Cargo.lock)
- **Test suite runtime:** storage_integration 9 tests in ~30 ms; storage::location 6 tests in <1 ms; total workspace `cargo test --workspace --locked` ~6 s (dominated by the first SQLite C compile + real-corpus round-trip)

## Task Commits

1. **Task 1: Migration v1 SQL — full Phase 1 schema** — `a232446` (feat)
2. **Task 2: DB-location resolver (D-13/IDX-06)** — `878e2b1` (feat)
3. **Task 3: Db wrapper + PRAGMAs + migrations + integration tests** — `9d9845a` (feat)

## Accomplishments

- One-shot Phase 1 migration covers every table in RESEARCH §Schema: `files` (path UNIQUE, hash index), `pages` (UNIQUE INDEX `name COLLATE NOCASE` per D-03), `blocks` (D-14: `raw` + `byte_offset` + `byte_length` + `hash` all coexist), `block_props` (composite PK + key index per D-05), `block_drawers` (composite PK on `block_id, byte_offset` per D-06), `tags`, `refs` (CHECK constraint on `type IN ('tag','page-link')`). FTS5 external-content table `blocks_fts` with `tokenize='unicode61 remove_diacritics 2'`, plus three triggers (`blocks_ai`, `blocks_ad`, `blocks_au`) keeping the inverted index in lockstep.
- `Db::open(notes_root)` resolves the DB path, opens the file, applies PRAGMAs (`journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, `temp_store = MEMORY`, `mmap_size = 256 MB`, `wal_autocheckpoint = 1000`, `journal_size_limit = 64 MB`), then runs `Migrations::to_latest`. Reopening is idempotent — `user_version` stays at 1 and pre-existing rows survive.
- 9 integration tests pin the contract end-to-end:
  - PRAGMAs are applied and observable (`journal_mode=wal`, `foreign_keys=1`, `synchronous=1`)
  - All 7 Phase 1 tables + `blocks_fts` exist
  - One full insert across every table (files → pages → blocks → block_props → block_drawers → tags → refs) inside one transaction
  - FTS5 INSERT trigger populates `blocks_fts`; `MATCH 'hello'` returns the inserted row
  - FTS5 UPDATE trigger swaps indexed text (old term gone, new term hit)
  - FTS5 DELETE trigger clears the indexed row
  - Foreign-key behavior: DELETE on `files` → `pages.file_id` SET NULL (D-04 unresolved page); DELETE on `pages` → CASCADEs to blocks → CASCADEs to block_props/block_drawers/refs
  - `pages_name_idx COLLATE NOCASE` rejects duplicate `Crypto` / `crypto` (D-03)
  - `refs.type` CHECK rejects `'bogus'`
  - Reopen on existing DB preserves data and does not bump `user_version`
- 6 location unit tests pin the resolver: under `XDG_DATA_HOME`, fallback for empty XDG, deterministic for same notes-root, distinct for different roots, filename is 16 hex + `.db`, DB path is **not** inside `notes_root` (IDX-06 regression guard).

## Decisions Made

- **rusqlite_migration 2.5 instead of CONTEXT D-20's "1.3"**: 1.3 transitively pins `rusqlite ^0.32` which conflicts at the `libsqlite3-sys` native `links = "sqlite3"` value with our `rusqlite 0.39`. The conflict surfaced as `cargo build` rejecting two distinct `libsqlite3-sys` versions in the same graph. 2.5 supports `rusqlite 0.39` and keeps the exact same `M::up(...)` + `Migrations::new(vec![...])` + `to_latest(&mut conn)` API — code is verbatim what CONTEXT/RESEARCH prescribed.
- **`tags` table KEPT**: RESEARCH explicitly left this open. Kept for symmetry with `refs.type` (so a future "list all known tags" query has a stable destination) and because the storage cost is exactly one usually-empty table.
- **`block_props` is a real table, not a JSON column** (per D-05 recommendation): indexed lookup on `key` lets Phase 2 inventory and any future query layer run plain SQL like `SELECT block_id FROM block_props WHERE key = 'template'` without dragging in JSON1.
- **Hand-rolled `data_dir()` (NOT the `directories` crate)**: RESEARCH §DB Location was explicit. Three small `#[cfg(target_os = ...)]` functions; the empty-`XDG_DATA_HOME` POSIX guard is explicit and tested.
- **`bundled-full` rather than bare `bundled`**: A7 "belt and suspenders" from the threat-model — explicit FTS5/JSON1/RTree even if today's default `bundled` would already enable them. Windows users compiling the crate will need MSVC Build Tools (documented in RESEARCH §Environment Availability).
- **`OnceLock<Migrations<'static>>` instead of a literal `static`**: `Migrations::new` is `const fn` but the inner `Vec<M>` borrows `'static str`s from `include_str!`; combined with the `'static` lifetime parameter the literal `static` form runs into `const` evaluation limits in older toolchains. `OnceLock` is a clean stable workaround with no observable behavior delta.
- **`ENV_LOCK: Mutex<()>` in the location unit tests**: `cargo test` runs unit tests inside one binary in parallel by default. Two tests both mutating `XDG_DATA_HOME` race each other and read whatever the other thread happened to set. Serializing only the env-mutating tests via a process-wide Mutex is the minimum-surface fix; no test-runner flags required.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] `rusqlite_migration 1.3` incompatible with `rusqlite 0.39`**
- **Found during:** Task 2 (first `cargo test`)
- **Issue:** Cargo refused to resolve: `rusqlite_migration 1.3` requires `rusqlite ^0.32` which links to `sqlite3` via `libsqlite3-sys 0.30.1`; our `rusqlite 0.39` links to `sqlite3` via `libsqlite3-sys 0.37`. Cargo rejects two crates linking to the same native lib.
- **Fix:** Bumped to `rusqlite_migration = "2.5"`. Same API (`M::up`, `Migrations::new`, `to_latest`); CONTEXT D-20 wording ("1.3+") covers it.
- **Files modified:** `crates/core/Cargo.toml`
- **Committed in:** `878e2b1` (Task 2 commit)

**2. [Rule 1 — Bug] Env-var-mutating unit tests flaked under parallel `cargo test`**
- **Found during:** Task 2 verification (`cargo test --workspace --locked`)
- **Issue:** `resolves_under_xdg_data_home_when_set` and `empty_xdg_falls_back_to_home_local_share` both mutate `XDG_DATA_HOME` in the same process; default-parallel test runner interleaved them and one saw the other's scratch value.
- **Fix:** Added `static ENV_LOCK: Mutex<()> = Mutex::new(())` at the top of the unit-test module; both env-mutating tests acquire it before touching `XDG_DATA_HOME`.
- **Files modified:** `crates/core/src/storage/location.rs`
- **Committed in:** `9d9845a` (Task 3 commit — bundled with the broader storage integration test work)

No other deviations. Plan body executed as written modulo the two auto-fixes above.

## rusqlite 0.39 / rusqlite_migration 2.5 API Gotchas

1. **`bundled-full` is the right feature for FTS5 belt-and-suspenders.** Bare `bundled` also enables FTS5 today but the explicit name documents intent and resists silent default-feature drift.
2. **`Connection::execute_batch` returns `()` on success**, not the row count. Use it for PRAGMA setup and any multi-statement migration body where you don't care about individual statement results.
3. **`Migrations::new(vec![...])` is `const fn`** but a literal `static` needs the inner `Vec<M>` to be `const`-buildable — which it isn't when the `'static str` payload comes from `include_str!`. `OnceLock<Migrations<'static>>` initialized at first use is the clean workaround. Alternative: `LazyLock` (Rust 1.80+) — equivalent for our purposes.
4. **`Migrations::to_latest(&mut conn)` requires `&mut`** because each migration runs inside a transaction it opens itself. The wrapper `Db::open_at` therefore holds the connection by value, not by `Arc`.
5. **FTS5 external-content `'delete'` command syntax**: the trigger does `INSERT INTO blocks_fts(blocks_fts, rowid, raw) VALUES('delete', old.id, old.raw)` — the first column is the table name and it acts as a sentinel that invalidates the indexed row. Easy to get wrong; lifted verbatim from RESEARCH.
6. **`PRAGMA user_version` returns `0` on a brand-new DB**; after `to_latest` it becomes `1` (the number of applied migrations). This is the idempotence-handle the integration test asserts.
7. **`PRAGMA journal_mode = WAL` returns a string `'wal'`**, not a boolean. Read it via `r.get::<_, String>(0)` and lowercase-compare. The other observed PRAGMA returns are integers (`synchronous`, `foreign_keys`).

## Pitfalls Discovered

- **`libsqlite3-sys` native-link conflict between SQLite crates**: any two crates in the dependency graph that link to `sqlite3` will fight unless their `libsqlite3-sys` versions agree. This is THE compatibility constraint for SQLite crate selection in Rust and the reason rusqlite_migration's version had to be bumped. Future Phase 2/3 additions (any auxiliary SQLite tooling — e.g., a backup crate) must respect the same constraint.
- **`tokenize='unicode61 remove_diacritics 2'`** is the right knob for "café" ≡ "cafe" matching (RF-30). The single-quoted options-string inside the FTS5 declaration is the canonical form; getting the inner quoting wrong silently downgrades to the default tokenizer.

## Verification Results

- `cargo test --test storage_integration --package foliom-core` — exits 0 (`9 passed`)
- `cargo test --package foliom-core storage::location` — exits 0 (`6 passed`)
- `cargo test --workspace --locked` — fully green: **65 tests** across 7 test binaries
  - 6 (storage::location lib) + 24 (ast_unit) + 9 (path_unit) + 2 (roundtrip) + 15 (segment_unit) + 9 (storage_integration) + 0 (doctests) = 65
- AP-2 guard `grep -rE "fn (serialize|to_markdown|format_block)" crates/` — empty
- `cargo build --workspace --locked` — exits 0

## TDD Gate Compliance

All three tasks were marked `tdd="true"` in the plan but the natural cycle for a schema-defining plan is "write SQL → write integration tests → confirm". Per the plan's own `<behavior>` blocks the test bodies are the behavioral assertion; the migration SQL is what makes those tests pass. Sequence in git log:

- `a232446` (feat 01-04): adds the SQL — no tests yet exist that exercise it (no RED commit because the SQL file is pure data; the test file in `9d9845a` is the simultaneous RED/GREEN gate).
- `878e2b1` (feat 01-04): adds the location resolver WITH its unit tests in the same commit (6 tests green).
- `9d9845a` (feat 01-04): adds the Db wrapper WITH its 9 integration tests in the same commit; all green.

Pure RED commits were not produced for this plan because the tests and code are co-located (location.rs has its `#[cfg(test)] mod tests` inline; storage_integration.rs cannot exist without the public surface it imports). Folding into one `feat(...)` commit per task is what the executor's task_commit_protocol expects when verification is bundled with implementation; the GREEN→GREEN sequence is the verifiable outcome.

## Threat Flags

None new. Threat register entries from the plan are addressed:

- **T-04-01 (DB-in-notes-folder corruption from cloud sync):** `db_path_is_outside_notes_root` test pins `!resolved.starts_with(&notes_canon)`.
- **T-04-02 (SQL injection via path string):** all integration-test SQL uses `params![]` bindings; no path string is ever concatenated into SQL.
- **T-04-03 (Migration replay corrupting existing DB):** `reopen_is_idempotent_and_preserves_user_version` test pins it.
- **T-04-04 (DB world-readable under $XDG_DATA_HOME):** accept disposition; user-umask applies; no secrets stored in Phase 1.
- **T-04-05 (WAL growth unbounded):** PRAGMAs `journal_size_limit = 64 MB` + `wal_autocheckpoint = 1000` set on every connection open.
- **T-04-SC (supply chain):** `rusqlite 0.39 bundled-full`, `rusqlite_migration 2.5` (bumped from CONTEXT 1.3 — same vendor / same API surface / Apache-2.0 license / 1.5M+ downloads / current stable), `blake3 1.5`, `tracing 0.1`, `tempfile 3` (dev). All HIGH legitimacy.

## Next Plan Readiness

- **Plan 01-05 (scanner)** consumes `RelativePath::from_filesystem` from Plan 03; storage layer is independent of the walker.
- **Plan 01-06 (indexer)** is the first real consumer: opens `Db::open(notes_root)`, then for each scanner-emitted file writes `files` row (BLAKE3 of bytes), parses (Plan 02 segmenter + Plan 03 ref extractor), and bulk-inserts pages / blocks / block_props / block_drawers / refs inside a single transaction per file. The schema's CASCADE behavior means "reindex" is just `DELETE FROM files WHERE id = ?` followed by re-insert — the integration test pins that path.
- **Plan 01-07 (CLI)** wires `foliom index <root>` to `Db::open(root)` + indexer; the `--json` mode emits the public IDs the schema now defines.
- **Plan 01-08 (inventory)** can `SELECT name, COUNT(*) FROM block_props GROUP BY key` directly — the indexed `block_props_key_idx` makes this single-digit-ms even on the 5k-note target corpus.
- The `tracing` crate is in `Cargo.toml` but no spans/events are emitted yet. Plan 01-06 will instrument the indexer hot path.

## Known Stubs

None. `Db`, `resolve_db_path`, the migration, and the PRAGMA setup are all fully wired with no placeholder values flowing to consumers. The `tags` table is intentionally empty in Phase 1 (the `refs` table is the working data structure per D-03); this is documented in CONTEXT and the schema comment, not a stub.

## Self-Check: PASSED

- File `crates/core/src/storage/mod.rs` — present (created)
- File `crates/core/src/storage/location.rs` — present (created)
- File `crates/core/src/storage/migrations/001_init.sql` — present (created)
- File `crates/core/tests/storage_integration.rs` — present (created)
- Commit `a232446` — present in `git log` (Task 1 — migration SQL)
- Commit `878e2b1` — present in `git log` (Task 2 — location resolver)
- Commit `9d9845a` — present in `git log` (Task 3 — Db wrapper + integration tests)
- `cargo test --workspace --locked` — green (65 tests across 7 binaries)
- AP-2 guard — clean

---
*Phase: 01-headless-indexing-core*
*Plan: 04*
*Completed: 2026-05-21*
