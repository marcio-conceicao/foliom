---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: Phase 2 plan 02-01 executed
last_updated: "2026-05-22T01:50:25.294Z"
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 15
  completed_plans: 9
  percent: 60
---

# Foliom — Project State

**Last updated:** 2026-05-22

---

## Project Reference

**Core value:** Cold start rápido e baixo uso de memória mesmo em grafos grandes, sem injetar metadados nos arquivos `.md`. Local-first markdown outliner (Logseq/Roam-style) where `.md` files are canonical and SQLite is a derivable cache.

**Current focus:** Roadmap initialized; ready to plan Phase 1.

---

## Current Position

- **Milestone:** v1
- **Phase:** 2 — Read-Only Web UI (in progress, plan 02-01 of 8 complete)
- **Plan:** 02-01 complete (HTTP scaffold); next is 02-02
- **Status:** Phase 2 plan 02-01 executed
- **Progress:** [██████░░░░] 60%

---

## Performance Metrics

| Metric | Target | Current |
|--------|--------|---------|
| Cold start (5k notes) | < 2 s | not measured |
| Idle RSS (5k notes) | < 300 MB | not measured |
| Installer size (desktop) | < 30 MB | not measured |
| Desktop idle RSS | < 150 MB | not measured |
| Round-trip CI (~600 sample files) | byte-identical | not implemented |

---
| Phase 01 P07 | 25 | 5 tasks | 17 files |
| Phase 02 P01 | 16m | 2 tasks | 11 files |
| Phase 02 P03 | 10 | 2 tasks | 23 files |

## Accumulated Context

### Decisions Logged

- Tech stack candidates: Rust (pulldown-cmark + rusqlite + notify-debouncer-full + axum + Svelte 5 + CM6 + Tauri 2) vs Go (Wails v3 alpha — disqualified). Lock in Phase 1.
- `.md` is canonical; SQLite is derivable cache stored outside notes folder.
- Two-stage parser: line-based outliner segmenter (TAB + 2-space continuation) → per-block CommonMark.
- Blocks materialized with both `raw` TEXT and `(byte_offset, byte_length)`; writeback via byte-splice, never whole-file re-serialize.
- (Plan 01-05) Scanner uses `walkdir 2.5` with `follow_links(false)` + `filter_entry`; ignore list is the 11-name hard-coded set + `:hidden` from `config.edn`. `regex 1` is added only for the config.edn module; segmenter/parser hot path stays regex-free.
- (Plan 01-05) Minimal `config.edn :hidden` reader is regex-based and NOT comment-aware — Phase 2 will upgrade if the renderer needs more keys.
- (Plan 01-06) Indexer uses single-pass page discovery: `ensure_unresolved_page` creates `pages` rows with `file_id = NULL` on demand (D-04); `ensure_self_page_row` claims unresolved rows on first backing-file insert. No second walk needed because order doesn't matter — verified by `delete_db_and_rebuild_reproduces_row_counts`.
- (Plan 01-06) Per-file SQLite transaction (AP-5) — failure of one file rolls back only that file's writes; orchestration continues for the rest of the corpus.
- (Plan 01-06) Full mode on unchanged corpus reports `mtime_touched` (not `unchanged`) because Full skips the (mtime,size) fast path by definition.
- (Plan 01-06) Synthetic fixture file count = 11 (10 pattern fixtures + README.md sibling). Real corpus = 620 files (locally verified).
- (Plan 02-01) HTTP scaffold: `foliom serve <root>` on 127.0.0.1:7345 via axum 0.7 + tokio current_thread + `Arc<Mutex<Db>>` shared state (D-22..D-25, D-38). Host-header allowlist rejects DNS rebinding with 421 Misdirected Request (T-02-01 mitigation). Graceful shutdown via `tokio::signal::ctrl_c`. AddrInUse on requested port falls back to OS-assigned :0.

### Open Decisions (PRD §12)

- §12.1 `#tag` vs `[[page]]` entity model (research recommends: same entity, two ref types).
- §12.3 block persistence (resolved by research: materialize with raw + byte offsets).
- §12.5 GFM scope (research recommends: tables YES Phase 2, code-fence highlight YES Phase 2 via Prism/starry-night).
- §12.8 `alias::` interpretation (v1: preserve opaque; v1.1: opt-in resolution).
- §12.9 TODO/DONE workflow markers (v1: plain text; v1.x: checkbox render).

### Todos

- (none — pending Phase 1 planning)

### Blockers

- (none)

---

## Session Continuity

**Last action:** Completed Phase 1 Plan 06 — indexer orchestrator (`reindex(&mut db, &root, mode)`) stitching scanner + parser + storage with per-file SQLite transactions. 3 task commits + metadata commit, 114 workspace tests green (including 12 new integration tests), AP-1/AP-2/AP-5 guards clean. IDX-02/03/04/05/07 and PRS-04/05/06 satisfied.
**Next action:** Plan 01-07 — CLI subcommands (`index`, `reindex`, `search`, `dump-tree`, `inventory`) wired to `indexer::reindex`, `storage::Db`, and parser. JSON output via `--json`. Pinned inventory snapshot against the synthetic corpus + CI matrix.
**Resumption hint:** `indexer::reindex(&mut Db, &Path, ReindexMode) -> Result<ReindexStats, IndexerError>` is the entry point. `ReindexStats { scanned, added, modified, unchanged, mtime_touched, deleted }` is the JSON shape (already derives Debug/Clone/PartialEq/Eq — add `serde::Serialize` when Plan 07 needs it). `Db::open(notes_root)` / `Db::open_at(db_path)` are the two ways to instantiate. Real-corpus run produces `ReindexStats { scanned: 620, added: 620, ... }` on first pass; second pass is idempotent.
