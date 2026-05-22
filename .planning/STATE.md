---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: Phase 2 plan 02-05 executed
last_updated: "2026-05-22T03:50:00.000Z"
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 15
  completed_plans: 13
  percent: 87
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
- **Phase:** 2 — Read-Only Web UI (in progress, 5 of 8 plans complete: 02-01, 02-02, 02-03, 02-04, 02-05)
- **Plan:** 02-05 complete (Sidebar + JournalNavigator + ThemeToggle + BacklinksPanel + unresolved chip styling); next is 02-06 (search palette Ctrl+K)
- **Status:** Phase 2 plan 02-05 executed
- **Progress:** [████████▊░] 87%

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
| Phase 02 P05 | 7m | 2 tasks | 16 files |

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
- (Plan 02-05) Did NOT add `@testing-library/svelte` — raw `mount`/`unmount` from svelte + DOM `querySelector` is consistent with 02-04's test style and avoids a ~70KB devDep.
- (Plan 02-05) ThemeToggle eagerly sets `<html data-theme>` on click in addition to writing the `theme` store. App.svelte's `$effect` remains the authoritative resolver (with `prefers-color-scheme` change listener + cleanup), but the eager apply keeps tests + isolated mounts self-sufficient.
- (Plan 02-05) Anti-FOUC: pure ES5 IIFE in `index.html` reads `localStorage('theme')` + `matchMedia` and writes `<html data-theme>` BEFORE Svelte hydrates. `try/catch` falls back to `'light'` with a `console.warn` on private-mode quota errors.
- (Plan 02-05) Block.svelte unresolved-chip styling runs as a post-render `$effect` over `contentEl` + `resolvedSet`. Empty `sidebarPages` is treated as "don't know yet" — chips render neutrally instead of all looking unresolved during the initial fetch.
- (Plan 02-05) BacklinksPanel guards against late-arriving responses on rapid page switches via `current === name` check at effect-resolution time.
- (Plan 02-05) JournalNavigator `initialMonth` prop intentionally non-reactive (`svelte-ignore state_referenced_locally`) — once mounted the user navigates via prev/next/Hoje; parent re-mutating the prop would be surprising. Prop exposed mainly for deterministic testing.
- (Plan 02-05) `/api/journals/today` returns a 302 to `/api/pages/YYYY_MM_DD`; the Hoje button reads `response.url` after the follow-redirect and converts `YYYY_MM_DD` → `YYYY-MM-DD` for the router shape.
- (Plan 02-05) Frontend: 56/56 tests green, bundle 207.23 kB JS (85.89 kB gzip) + 10.68 kB CSS (2.55 kB gzip).

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

**Last action:** Completed Phase 2 Plan 05 — Sidebar (debounced filter + Pages/Journals grouped sections + Favorites/Recents placeholders + ThemeToggle footer), JournalNavigator (month grid, prev/next, Hoje button), ThemeToggle (tri-state Claro/Auto/Escuro), BacklinksPanel (collapsible <details> under PageView with #block= deep links), Block.svelte retroactive unresolved chip styling via post-render `$effect` over `sidebarPages`-derived Set, anti-FOUC inline script in index.html, App.svelte `prefers-color-scheme` change listener. 2 task commits + summary. 56 frontend tests green (12 new). Delivers LNK-03 + LNK-06 + UI-02. Closes long-deferred unresolved page-chip styling from 02-04.
**Next action:** Plan 02-06 — search palette (Ctrl/Cmd+K) consuming `/api/search?q&kind&limit`. Insertion points already marked: `App.svelte` (palette modal slot below `.layout`), `Sidebar.svelte` footer (Ctrl+K trigger button). `searchPalette` store from 02-03 is the open/query state; `searchResults` store is 02-06's to introduce.
**Resumption hint:** Existing surfaces — `stores.ts` already has `searchPalette: { open, query }`. `routes.ts` has `/search` → `SearchView.svelte` (placeholder from 02-03, ready to be wired). Tag-chip clicks already route to `/search?q=#<tag>&kind=tag` (Block.svelte delegated handler from 02-04). Backend `/api/search` ships from 02-02 with `kind=content|tag` + `limit` + snippet. 02-05 left no overlapping work — App.svelte and Sidebar.svelte have explicit `// 02-06 will add:` markers.
