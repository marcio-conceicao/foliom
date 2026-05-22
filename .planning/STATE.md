---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: Phase 2 plan 02-07 executed
last_updated: "2026-05-22T04:10:00.000Z"
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 15
  completed_plans: 14
  percent: 93
---

# Foliom — Project State

**Last updated:** 2026-05-22 (Plan 02-07 executed)

---

## Project Reference

**Core value:** Cold start rápido e baixo uso de memória mesmo em grafos grandes, sem injetar metadados nos arquivos `.md`. Local-first markdown outliner (Logseq/Roam-style) where `.md` files are canonical and SQLite is a derivable cache.

**Current focus:** Roadmap initialized; ready to plan Phase 1.

---

## Current Position

- **Milestone:** v1
- **Phase:** 2 — Read-Only Web UI (in progress, 7 of 8 plans complete: 02-01, 02-02, 02-03, 02-04, 02-05, 02-06, 02-07)
- **Plan:** 02-07 complete (rust-embed 8.11 + mime_guess 2; debug 307→Vite, release single-binary embed with SPA fallback; --open wired); next is 02-08 (perf gates ACPT-02 cold start + ACPT-03 RAM)
- **Status:** Phase 2 plan 02-07 executed
- **Progress:** [█████████▎] 93%

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
| Phase 02 P06 | 14m | 2 tasks | 11 files |
| Phase 02 P07 | 18m | 2 tasks | 8 files |

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
- (Plan 02-06) Snippet sanitization for `/api/search` results is a hand-rolled allow-list (`lib/sanitize.ts`): escape all HTML then reintroduce only literal `<mark>`/`</mark>`. Rationale: T-02-20 surface is two tokens; DOMPurify (~20 KB gz) would dent the cold-start budget for no real safety win. Test passes `<script>alert(1)</script>` through the snippet and asserts no script element appears.
- (Plan 02-06) `lib/keys.ts` owns ALL app-level shortcuts — single `window.keydown` listener returning a disposer. Cmd/Ctrl+K modifier OVERRIDES the editable-target gate by design so the palette is always summonable. Plain Esc defers to native input clearing inside `<input>`/`<textarea>`/`[contenteditable]`; outside those it closes the palette. Inside the palette modal the input's own onkeydown handler reasserts Esc-to-close so the user doesn't get stuck.
- (Plan 02-06) Palette `[[` branch caches `/api/page-titles` in a module-level `Promise<string[]>` so subsequent `[[` keystrokes are pure client-side filtering. Cache lives for the page session; titles change rarely and the branch is forgiving of staleness.
- (Plan 02-06) AbortController IS wired into each debounced run (T-02-22). A slow `/api/search` response from an older keystroke cannot clobber results from a newer one; AbortError is silently swallowed.
- (Plan 02-06) `Number(hit.blockId)` coercion + `>0` guard (T-02-21). Non-numeric or zero/negative blockId → omit `#block=` fragment entirely → user lands at page top. blockId=0 is a documented sentinel used by the `[[` page-titles branch.
- (Plan 02-06) svelte-spa-router treats `#` as its own route boundary; foliom layers the `#block=N` sub-fragment on top via `push(target)` then a `requestAnimationFrame` callback that rewrites `window.location.hash` to include both segments. The 02-04 zoom listener picks up the hashchange and scrolls.
- (Plan 02-06) Single `SearchPalette.svelte` with `mode='modal'|'inline'` prop instead of extracting a SearchPanel — keeps the test surface small and avoids prop-drilling. `SearchView.svelte` mounts it in inline mode and pre-populates the store query from `window.location.hash`.
- (Plan 02-06) Frontend: 73/73 tests green (17 new), bundle 212.57 kB JS (87.74 kB gzip) + 12.60 kB CSS (2.98 kB gzip). +5.3 kB JS / +1.9 kB CSS vs 02-05 baseline.
- (Plan 02-07) Single-binary distribution: workspace pins `rust-embed = { version = "8", default-features = false, features = ["compression"] }` + `mime_guess = "2"`. A2 (Assumptions Log — rust-embed 8.x on edition 2024) is RESOLVED positively at v8.11.0 / rustc 1.95.
- (Plan 02-07) Static asset fallback wired via `Router::fallback(embed::serve_static)` in `routes/mod.rs`. `/api/*` is unaffected because `.fallback` only fires when no `.route(...)` matches. Single handler, two profiles via `#[cfg(debug_assertions)]` — no `--features embed` cargo gate.
- (Plan 02-07) Debug profile: `GET /` (and any non-`/api` miss) → 307 Temporary Redirect to `http://localhost:5173/<path>` (query string preserved). Chose 307 over 302 to preserve HTTP method on the redirect.
- (Plan 02-07) Release profile: rust-embed reads `crates/cli/../../frontend/dist` at compile time. SPA fallback (missing path → `index.html`) supports both hash-router deep links AND arbitrary URL paste-reloads. Cache-Control = `no-cache` for `index.html` (shell can change between deploys) and `public, max-age=3600` for hashed assets (Vite emits content-hashed names).
- (Plan 02-07) Empty-dist tolerance verified: `cargo check` on a fresh clone with only `frontend/dist/.gitkeep` succeeds — rust-embed silently embeds an empty assets directory and runtime serves 404 on `/` until `npm run build` runs. Chose option 1 from 02-RESEARCH §Empty-dist fallback (no separate feature flag).
- (Plan 02-07) `--open` flag already wired in `serve/mod.rs` from plan 02-01 (`browser::try_open(&url)` after boot banner). Best-effort; non-fatal on headless environments. Cross-OS verification deferred to plan 02-08 CI matrix.
- (Plan 02-07) Integration test `crates/cli/tests/serve_prod_embed.rs` covers BOTH profiles via `#[cfg(debug_assertions)]` / `#[cfg(not(debug_assertions))]` split. Release leg self-skips when `frontend/dist/index.html` is absent so fresh-clone runs of `cargo test --release` don't produce spurious red CI. ureq treats 307 with `redirects(0)` as `Ok` (not `Err`), so the assertion folds `Ok` and `Err(Status(_, resp))` into a single response binding before checking status + Location.
- (Plan 02-07) Manual smoke: release binary run from `/tmp/foliom-smoke/` with no `frontend/dist/` in cwd serves the 1585-byte SPA shell at `/` and SPA-falls back on `/foo/bar/baz` — single-binary distribution confirmed end-to-end.

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

**Last action:** Completed Phase 2 Plan 06 — search palette (`SCH-01`/`SCH-02`/`SCH-03`/`LNK-07`). `lib/keys.ts` global keymap, `SearchPalette.svelte` modal + inline body, `SearchResult.svelte` row, `lib/sanitize.ts` `<mark>` allow-list, Sidebar "Buscar (Ctrl+K)" footer trigger, `SearchView.svelte` rewired to render the palette body in inline mode for `#/search?q=` deep links. 150ms debounce, AbortController cancellation, three-branch routing (`#`→tag, `[[`→page-titles cached, else content), keyboard nav + click-to-block via `#/pages/<page>#block=<blockId>`. 4 commits (2 RED, 2 GREEN). 73/73 tests green (17 new: 8 keys + 9 palette). Bundle 212.57 kB / 87.74 kB gz.
**Next action:** Plan 02-07 — integration + smoke E2E. With wave 4 done, the full UI (sidebar + page view + backlinks + palette + zoom + theme) is on the same store/route surface; 02-07 should drive end-to-end flows against a live `foliom serve` and pin the Ctrl+K → result → block-scroll path as a regression check. Plan 02-08 (Wave 6) is perf gates; bundle headroom is healthy.
**Resumption hint:** Phase 2 progress: 6 of 8 plans complete. Frontend stack confirmed working: Svelte 5 runes + Vite + happy-dom vitest; 73 tests across 12 suites. App.svelte mounts `<SearchPalette>` on `searchPalette.open`. The zoom listener installed in `main.ts` (from 02-04) consumes the `#block=N` fragments the palette emits. `lib/keys.ts` is the canonical place to add future app-wide shortcuts. Pre-existing `svelte-check` warnings on `routes.ts` (Component params type mismatch for hash routes) are NOT a 02-06 regression — log as a hygiene item for 02-07 or 02-08.
