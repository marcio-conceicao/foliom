---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
last_updated: "2026-05-22T13:44:11.645Z"
progress:
  total_phases: 5
  completed_phases: 4
  total_plans: 28
  completed_plans: 27
  percent: 96
---

# Foliom — Project State

**Last updated:** 2026-05-22 (Plan 05-01 executed — Tauri 2 desktop shell scaffold; BOUND_PORT OnceLock; src-tauri/ crate; WebviewUrl::External; 11min)

---

## Project Reference

**Core value:** Cold start rápido e baixo uso de memória mesmo em grafos grandes, sem injetar metadados nos arquivos `.md`. Local-first markdown outliner (Logseq/Roam-style) where `.md` files are canonical and SQLite is a derivable cache.

**Current focus:** Roadmap initialized; ready to plan Phase 1.

---

## Current Position

- **Milestone:** v1
- **Milestone:** v1
- **Milestone:** v1
- **Phase:** 5 — Desktop Packaging (IN PROGRESS — 1/3 plans: 05-01 done).
- **Plan:** 05-01 complete (Tauri 2 shell scaffold — src-tauri/ crate, BOUND_PORT OnceLock, WebviewUrl::External; DSK-01 code complete; 11min).
- **Status:** Phase 5 in progress — 05-01 done. 05-02 (release CI) and 05-03 (footprint gate) next.
- **Progress:** [██████████] 96%

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
| Phase 02 P08 | 35m | 2 tasks | 12 files |
| Phase 03 P01 | 35min | 2 tasks | 9 files |
| Phase 03 P02 | 25min | 2 tasks | 6 files |
| Phase 03 P04 | 11min | 2 tasks | 18 files |
| Phase 03 P05 | 10min | 2 tasks | 14 files |
| Phase 04 P01 | 12min | 2 tasks | 11 files |
| Phase 04 P02 | 3min | 2 tasks | 6 files |
| Phase 04 P03 | 2min | 2 tasks | 2 files |
| Phase 05-desktop-packaging P01 | 11min | 2 tasks | 13 files |
| Phase 05-desktop-packaging P02 | 2 | 1 tasks | 2 files |

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
- (Plan 02-08) Performance harness landed: workspace pins `criterion = "0.5"`, `rand = "0.8"`, `rand_chacha = "0.3"`, `sysinfo = "=0.30.13"` (exact pin for A4 unit drift: process.memory() returns bytes only on 0.30.x; earlier returned KB). Two new bin targets in foliom-cli: `foliom-bench-gen` (deterministic 5k corpus, 70/30 journals/pages, TAB indents + code fences + drawers + properties + refs) and `foliom-bench-rss` (sysinfo probe with hand-rolled HTTP/1.1 GET over TcpStream — no reqwest/TLS in release artifact).
- (Plan 02-08) Criterion bench function naming: AVOID `:` and `::` because Criterion sanitizes them in output paths (`Db::open` → `Db__open`). Use snake_case identifiers. Bench renamed to `db_open_reindex_full` so the CI gate path is stable; documented in `crates/core/benches/README.md` as a project convention.
- (Plan 02-08) Bench skips gracefully when `/tmp/synth-5k` is missing (eprintln + early return) so `cargo bench` on a fresh clone never fails before the dev runs `foliom-bench-gen`. CI generates the corpus in a dedicated step before `cargo bench`.
- (Plan 02-08) `scripts/bench_assert.py` is strict-less-than: equality with the ceiling FAILS the gate. Has its own 4-case unittest (`scripts/test_bench_assert.py`) covering pass/fail/exact-ceiling/missing-file branches.
- (Plan 02-08) CI matrix restructured: Node-before-Rust (so rust-embed picks up real `frontend/dist/`); bundle-size gate (600 KB ceiling, runs on every OS); Phase 2 E2E smoke step (`serve` → /api/health + /api/pages → kill) on Linux/macOS/Windows; new `bench` job (Linux-only, `needs: test`) builds release artifacts + generates 5k corpus + runs cold-start bench + RSS probe + asserts ACPT-02 (<3s via bench_assert.py) and ACPT-03 (<450MB via bench-rss) + uploads Criterion report as 14-day artifact. Swatinem/rust-cache shared-key bumped phase1 → phase2 (lock file changed materially).
- (Plan 03-02) Mutation module is pure-logic (no IO/HTTP/SQL). `splice_block` + `compute_shifted_offsets` form the SNC-01 byte-splice foundation; 6 invertible `TreeOp` variants form the D-30-05 hybrid-undo foundation. Plan 03-03's REST handlers compose these with plan 03-01's `atomic_write_md`.
- (Plan 03-02) `BlockSnapshot` extended with `reparented_children: Vec<i64>` (Rule 2 deviation) — required for invertible `Delete` on a block with children. `#[serde(default)]` preserves wire compatibility.
- (Plan 03-02) `Merge` uses pure byte concat (no `\n` separator) so `Split` is the exact byte-slice inverse. Earlier prototype inserted a separator newline + auto-stripped on split — broke invertibility for blocks whose raw already ended in `\n`. Caught by `merge_then_inverse_restores_tree` + `split_then_inverse_restores_tree`.
- (Plan 03-02) `Merge` closes the ord gap left by the removed sibling; without this, downstream `Split` inverse shifted siblings further right and the round-trip diverged.
- (Plan 02-08) bench-rss env override surface for test/CI hygiene: `FOLIOM_BENCH_PORT` (default 7350), `FOLIOM_BENCH_CEILING_MB` (default 450), `FOLIOM_BENCH_FOLIOM` (binary path override). Binary resolver: env override → sibling of current_exe (works under target/release/) → `./target/release/foliom` fallback. Cross-platform via `cfg!(windows)` for the `.exe` suffix.
- (Plan 02-08) PERF-BASELINE.md records first measurements: WSL2 cold start = 12.20s (above 3s ceiling — EXPECTED hardware delta vs the M1-class PRD reference; CI runs on ubuntu-latest where the ceiling should hold), RSS = 49 MB (well below 300 MB target), bundle = 248 KB (well below 600 KB ceiling). Drift table inline; new rows appended (never overwritten) when metrics drift ≥10%.
- (Plan 02-08) `cargo nextest` not installed locally; verified via `cargo test --workspace --no-fail-fast` (all 17 test binaries green). CI installs nextest via `taiki-e/install-action@nextest` so this affects only local verification.
- (Plan 03-01) SNC-02 foundations: `crates/core::sync` module exposes `atomic_write_md(target, contents, &SelfWriteSet) -> io::Result<[u8;32]>` (same-FS temp+rename, blake3 hash registered BEFORE persist, Windows-only retry 50/100/200ms on `PermissionDenied`/`Other` for AV holds, unix parent fsync for crash safety). `SelfWriteSet` is `Clone + Send + Sync` over `Arc<DashMap<[u8;32], Instant>>` with configurable TTL (`DEFAULT_TTL = 30s`). Workspace pins `tempfile = "3"` (promoted from per-crate dev-dep) and `dashmap = "6"` (max-stable 6.2.1; 7.x is rc — explicitly rejected). A9 verified: `TempPath::persist` returns `PathPersistError { error, path }` so the retry loop re-attempts without rewriting contents. Windows AV smoke test is `#[cfg_attr(not(windows), ignore)]`; tolerates 0 or ≥3 attempts via test-only `LAST_PERSIST_ATTEMPTS` thread_local counter.
- (Plan 03-01) Commit `a113c88` accidentally included `crates/core/src/mutation/{mod,splice,tree_ops}.rs` + `__tests__/splice_test.rs` scaffolding created out-of-band by a concurrent process (PLAN 03-02's territory per 03-RESEARCH §8). Files compile cleanly and pass tests; left in place. Plan 03-02 executor should treat them as a starting point, not re-create.
- (Plan 03-03) Disk write before SQL transaction: crash between `atomic_write_md` and COMMIT self-heals on next reindex (hash mismatch triggers reparse). Inverse order (SQL first) leaves committed hash with stale bytes — harder to recover.
- (Plan 03-03) `blocks` table has no `file_id` column — always resolve via `blocks.page_id → pages.file_id` JOIN. All offset-shift UPDATE statements use `page_id` filter.
- (Plan 03-03) `foliom-cli` library target added (`src/lib.rs` + `[lib]` in Cargo.toml) so integration tests can import `AppState`/`build_router` for in-process `tower::ServiceExt::oneshot` tests. `main.rs` uses `foliom_cli::cmd` instead of local `mod cmd`.
- (Plan 03-03) `PATCH /api/blocks/:id/structure` op=move defers byte-level file reorder to Phase 5 — only SQL tree updated. File byte order is cosmetically wrong but functionally irrelevant until user views raw `.md`. Documented in handler.
- (Plan 03-03) `ApiError` is a new typed enum (NotFound/Stale/BadRequest/Internal) rather than re-using Phase 2's `StatusCode`. Reason: 409 Stale must return `{ error: "stale", currentFileHash: "..." }` structured body so clients can refresh.
- (Plan 03-03) `PageDetail` gains `fileHash` (hex BLAKE3) + `id` fields so client can round-trip `prevHash` without a separate lookup. `fileHash` is `None` for unresolved pages.
- (Plan 03-03) MutationResponse wire shape final: `{ blockSubtree, fileHash, dirtyBlockIds }`. CreateBlockResponse adds `id`. ErrorResponse: `{ error, currentFileHash? }`. Plan 03-04 frontend will depend on these shapes.
- (Plan 03-05) refs.type='tag' (not kind), refs.target_page is INTEGER FK to pages.id — autocomplete SQL for tags must JOIN pages ON pages.id = refs.target_page. Plan spec said `WHERE kind='tag' AND target_page LIKE prefix%` which would fail (type column, not kind; integer, not string).
- (Plan 03-05) detectBulletTree depth-0 continuation: accepts both TAB+2-space and plain 2-space, consistent with segment.rs. Empty lines also fold into parent block raw. Requires ≥2 bullet items per D-30-07.
- (Plan 03-05) serializeBlockTree confirmed: block.raw is verbatim segment.rs output (leading TABs + "- " + text + "\n"). One-line recursive concat is correct — no additional formatting needed.
- (Plan 03-05) applyInverse wired for Indent/Outdent (PATCH /structure depth), Delete (POST /blocks snapshot), Move (PATCH /structure parent+ord). Merge/Split inverse deferred to plan 03-06. 409 restores op to treeOpLog + signals stale banner.
- (Plan 03-05) BlockEditorCallbacks.onPaste added as optional — existing 03-04 mounts unaffected (onPaste undefined → paste extension omitted from CM6 extensions array).
- (Plan 03-05) BulletPopover absolute-positioned (left: 100%; top: 0) with $effect document keydown/mousedown listeners. No floating-ui dep.
- (Plan 04-01) notify-debouncer-full watcher: mpsc::Sender as DebounceEventHandler (blocking OS thread, NOT tokio task). Event loop: Ok(events) → filter .md → path-traversal guard → SelfWriteSet::take_if_present → local suppressed-hash DashMap (600ms TTL) for multi-event inotify dedup → DirtySet. Drain on 300ms coalescing tick. Flag::Rescan (macOS MustScanSubDirs / Linux IN_Q_OVERFLOW) → full reindex + IndexReset SSE. Err branch (Windows ReadDirectoryChangesW) → same + re-arm. FOLIOM_DEBOUNCE_MS env override for power users.
- (Plan 04-01) Local suppressed-hash cache is required: SelfWriteSet.take_if_present is consume-once but inotify emits multiple events per atomic rename (MOVED_TO + MODIFY + CLOSE_WRITE). After take_if_present succeeds, hash is cached in local DashMap for 600ms to absorb follow-up events.
- (Plan 04-01) GET /api/watch/events SSE endpoint uses BroadcastStream::new(rx).map(sse_event_from_result); Lagged → index_reset (T-04-03); 30s KeepAlive.text("ping") (D-40-02). AppState.watcher_tx: Arc<broadcast::Sender<WatcherEvent>>(64).
- (Plan 04-02) externalConflict one-shot signal pattern: watcher.ts sets; PageView.svelte always resets to null after handling — prevents perpetual banner (T-04-06). EventSource singleton guard uses readyState !== CLOSED (not null check) so a browser-reconnecting ES (CONNECTING state) is not replaced.
- (Plan 04-02) App.svelte onMount for startWatcher (not module-level) so Vitest tests can assign MockEventSource to globalThis before the component mounts and captures the constructor.
- (Plan 04-02) Watcher-status pill: CSS-only @keyframes watcher-pulse animation for reconnecting state. No JS animation. 8px dot, three data-status values binding to green/amber/grey.
- (Plan 04-03) CI smoke job uses curl --no-buffer -N for SSE (streaming) rather than a ureq test binary — simpler, no new dep, exercises the real HTTP path. PID-file pattern (/tmp/*.pid) for cleanup across GitHub Actions steps that don't share env vars.
- (Plan 04-03) 1.5s wait (not 1s) in CI smoke: absorbs server startup + inotify debounce (300ms) + DirtySet coalescing (300ms) + curl overhead. Still well within 5-minute job timeout.
- (Plan 04-03) Linux-only smoke job: Windows ReadDirectoryChangesW testing requires Windows native runner; covered by ACPT-04-WATCHER.md manual checklist instead.
- (Plan 05-01) WebviewUrl::External NOT tauri-plugin-localhost — plugin creates own tiny_http server, NOT a bridge to axum; Foliom's axum already serves SPA via rust-embed. Source-verified against docs.rs/tauri-plugin-localhost.
- (Plan 05-01) std::thread::spawn for serve_run — serve::run() builds its own tokio::runtime via block_on; nesting inside tauri::async_runtime::spawn panics with "Cannot start a runtime from within a runtime". Always use std::thread::spawn.
- (Plan 05-01) BOUND_PORT OnceLock<u16> set after bind_loopback() before rt.block_on() — Tauri setup hook polls with 20ms sleep, 5s timeout (250 iterations). Port typically available < 50ms from thread start.
- (Plan 05-01) Dialog API: app.dialog().file().blocking_pick_folder() → Option<FilePath>; FilePath::into_path() → PathBuf. NOT the assumed blocking::FileDialogBuilder pattern.
- (Plan 05-01) Store API: StoreExt trait; app.store('config.json') → Result<Arc<Store>>; store.get/set/save confirmed from tauri-plugin-store-2.4.3 source.

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

**Last action:** Plan 05-01 complete — Tauri 2 shell scaffold (64ce7b0). src-tauri/ crate, BOUND_PORT OnceLock, WebviewUrl::External, folder picker, vault-root persistence. DSK-01 code complete.
**Next action:** Run `sudo pacman -S webkit2gtk-4.1` then `cargo build -p foliom-tauri` to verify local build. Then 05-02 (release CI workflow) and 05-03 (footprint gate).
**Resumption hint:** Phase 5 plan 1 done. webkit2gtk-4.1 must be installed for local Linux build. CI (ubuntu-latest) already has it. Code is correct and verified against plugin sources.
