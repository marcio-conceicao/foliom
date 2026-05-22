---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: Phase 2 plan 02-04 executed
last_updated: "2026-05-22T02:55:00.000Z"
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 15
  completed_plans: 12
  percent: 80
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
- **Phase:** 2 — Read-Only Web UI (in progress, 4 of 8 plans complete: 02-01, 02-02, 02-03, 02-04)
- **Plan:** 02-04 complete (per-block markdown renderer + foliom inline rules + Prism + Block zoom); next is 02-05 (sidebar + dark-mode toggle + backlinks panel)
- **Status:** Phase 2 plan 02-04 executed
- **Progress:** [████████░░] 80%

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
| Phase 02 P04 | 12m | 2 tasks | 16 files |

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
- (Plan 02-02) REST surface live: 7 read-only endpoints (`/api/pages`, `/api/pages/:name`, `/api/pages/:name/backlinks`, `/api/page-titles`, `/api/journals/today`, `/api/journals?from&to`, `/api/search?q&kind&limit`). All handlers run DB work in `spawn_blocking` and bind via `params![]` (T-02-04). Search sanitization: empty after trim → `[]`, unquoted `:` rejected, backslashes stripped (T-02-05). `properties_json`/`drawers_json` are stored as normalized side tables (`block_props`/`block_drawers`), NOT JSON columns — detail handler joins per-page with prefetch (no N+1). `pages/Avaliação.md` fixture added for FTS5 UTF-8 snippet integrity (Pitfall 6); inventory pinned counts bumped scanned 11→12, pages 10→11.
- (Plan 02-02) **axum 0.7 path-param syntax bug fixed**: workspace pins `axum = "0.7"` (matchit 0.7, `:name` syntax) but initial implementation used `{name}` (axum 0.8 / matchit 0.8 syntax) so detail/backlinks routes were treated as literal paths and every request hit axum's fallback 404 with content-length 0. Reverted to `:name`. Note for future: any axum 0.8 upgrade must flip this back.
- (Plan 02-04) Heading suppression mechanism: chose **option 3** (post-process core ruler walking `heading_open → inline.children` and rewriting chip tokens to `text`). Option 1 (env flag in a block-rule wrapper) failed because markdown-it's inline pass runs after the block wrapper's `finally` clears the flag.
- (Plan 02-04) Tag chip click destination = **search-by-tag** (`/search?q=#<tag>&kind=tag`) — Open Question 3 in 02-RESEARCH. No dedicated tag-page view in v1.
- (Plan 02-04) `stripForRender` strips ALL leading TABs (matching `ast.rs::strip_segmenter_prefix` which is unbounded). `depth` arg only used for prelude detection (depth < 0).
- (Plan 02-04) Vitest config needs `resolve.conditions = ['browser','svelte','development']` + `test.server.deps.inline = ['svelte']` so `mount()` resolves to Svelte's client entry under happy-dom. Without these, vitest picks the SSR entry and throws `lifecycle_function_unavailable`.
- (Plan 02-04) Line-numbers: pure-CSS gutter band (no JS plugin). Per-line digits deferred. UI-04's `.line-numbers` class contract is satisfied so a future enhancement can layer numbers on top without touching markdown.
- (Plan 02-04) Unresolved page-link styling deferred to plan 02-05 (waits for `sidebarPages`).
- (Plan 02-04) 1000-block render time in happy-dom = 705ms vs 2000ms ceiling. Real-browser aspirational target is <100ms per A7.

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

**Last action:** Completed Phase 2 Plan 04 — per-block markdown renderer with foliom inline rules (`compositeTag` / `pageLink` / `bareTag`), segmenter-prefix + property + drawer stripping, ATX heading suppression via post-process core ruler, Prism syntax highlighting with lang label + line-numbers gutter, recursive `Block.svelte` with fold (UI-only D-34) + delegated chip click handler, `PageHeader.svelte`, block zoom (LNK-07) via second-`#` sub-fragment, 1000-block soft perf gate (705ms vs 2000ms ceiling). 2 task commits + summary. 44 frontend tests green.
**Next action:** Plan 02-05 — Sidebar + journal navigator + dark mode toggle + backlinks panel. Wires `sidebarPages` store to `<aside>`, adds the long-deferred unresolved `.page-link.unresolved` styling once that set is loaded, persistent theme toggle, backlinks panel under main content.
**Resumption hint:** Block contract is locked: `Block.svelte` props `{ id, depth, raw, properties, drawers, children }`; rendering pipeline is `stripForRender → md.render → {@html}` (see `frontend/src/lib/components/Block.svelte`). Tag click destination = `/search?q=#<tag>&kind=tag` (plan 02-06 will own the search palette behavior side). `sidebarPages` Svelte store exists (plan 02-03) but is unused until 02-05. Block.svelte's `.page-link` class is currently neutral — toggle to `.page-link.unresolved` based on `sidebarPages` membership lookup.
