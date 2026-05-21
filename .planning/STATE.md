---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: "awaiting `/gsd:plan-phase 1`"
last_updated: "2026-05-21T23:45:00.000Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 7
  completed_plans: 6
  percent: 86
---

# Foliom — Project State

**Last updated:** 2026-05-21

---

## Project Reference

**Core value:** Cold start rápido e baixo uso de memória mesmo em grafos grandes, sem injetar metadados nos arquivos `.md`. Local-first markdown outliner (Logseq/Roam-style) where `.md` files are canonical and SQLite is a derivable cache.

**Current focus:** Roadmap initialized; ready to plan Phase 1.

---

## Current Position

- **Milestone:** v1
- **Phase:** 1 — Headless Indexing Core (in progress, plans 01–05 of ~7 complete)
- **Plan:** 01-05 complete; next is 01-06 (indexer)
- **Status:** Phase 1 plans 01–05 executed
- **Progress:** [████████░░] 86%

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

## Accumulated Context

### Decisions Logged

- Tech stack candidates: Rust (pulldown-cmark + rusqlite + notify-debouncer-full + axum + Svelte 5 + CM6 + Tauri 2) vs Go (Wails v3 alpha — disqualified). Lock in Phase 1.
- `.md` is canonical; SQLite is derivable cache stored outside notes folder.
- Two-stage parser: line-based outliner segmenter (TAB + 2-space continuation) → per-block CommonMark.
- Blocks materialized with both `raw` TEXT and `(byte_offset, byte_length)`; writeback via byte-splice, never whole-file re-serialize.
- (Plan 01-05) Scanner uses `walkdir 2.5` with `follow_links(false)` + `filter_entry`; ignore list is the 11-name hard-coded set + `:hidden` from `config.edn`. `regex 1` is added only for the config.edn module; segmenter/parser hot path stays regex-free.
- (Plan 01-05) Minimal `config.edn :hidden` reader is regex-based and NOT comment-aware — Phase 2 will upgrade if the renderer needs more keys.

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

**Last action:** Completed Phase 1 Plan 05 — scanner + ignore list + minimal `config.edn :hidden` reader. 6 commits (3 RED + 3 GREEN), 92 workspace tests green, AP-2 clean. IDX-01 satisfied.
**Next action:** Plan 01-06 — incremental indexer that consumes `scanner::walk` and persists files / pages / blocks / refs into the storage layer.
**Resumption hint:** `scanner::walk(root, &ignore_set)` returns `Iterator<Item = ScanEntry>` where ScanEntry carries `(absolute path, mtime_ns, size)`. Indexer converts path via `RelativePath::from_filesystem(&path, root)` at the storage boundary. Read `config.edn :hidden` once at startup via `scanner::config_edn::read_hidden(root.join("logseq/config.edn"))` and feed to `IgnoreSet::extend_from_config_edn` before walking.
