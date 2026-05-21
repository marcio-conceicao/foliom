---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: "awaiting `/gsd:plan-phase 1`"
last_updated: "2026-05-21T17:02:22.764Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
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
- **Phase:** 1 — Headless Indexing Core (not started)
- **Plan:** none yet
- **Status:** awaiting `/gsd:plan-phase 1`
- **Progress:** Phase 0/5 complete `[          ] 0%`

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

**Last action:** Roadmap created via `/gsd:new-project` orchestrator.
**Next action:** `/gsd:plan-phase 1` to decompose Phase 1 (Headless Indexing Core) into executable plans.
**Resumption hint:** Phase 1 must produce the ACPT-01 round-trip CI gate and IDX-08 inventory script BEFORE any storage/indexer/watcher work.
