# Foliom — Roadmap

**Project:** Foliom (local-first markdown outliner)
**Granularity:** coarse
**Mode:** standard (horizontal layers — M0 headless → M1 read UI → M2 editor → M3 sync → M4 desktop)
**Coverage:** 49/49 v1 requirements mapped
**Last updated:** 2026-05-22

---

## Phases

- [x] **Phase 1: Headless Indexing Core** — Round-trip CI gate + inventory script + scanner/parser/indexer/CLI; no UI. (completed 2026-05-22)
- [x] **Phase 2: Read-Only Web UI** — Svelte frontend over axum HTTP serving rendered pages, navigation, backlinks, search. (code-complete 2026-05-22 — awaiting `/gsd-verify-work`)
- [ ] **Phase 3: Outliner Editor** — CodeMirror 6 single-block editor with byte-splice writeback, undo/redo, rename-with-backlinks.
- [x] **Phase 4: Disk Sync** — Filesystem watcher with self-write suppression, SSE live updates, conflict UI. (completed 2026-05-22)
- [ ] **Phase 5: Desktop Packaging** — Tauri 2 shell, signed installers, footprint gate.

---

## Phase Details

### Phase 1: Headless Indexing Core
**Goal**: A headless Rust core can scan a real Logseq folder, byte-stably round-trip every file, and answer index/search/dump queries via CLI — proving the foundation before any UI exists.
**Depends on**: Nothing (foundation phase)
**Requirements**: IDX-01, IDX-02, IDX-03, IDX-04, IDX-05, IDX-06, IDX-07, IDX-08, PRS-01, PRS-02, PRS-03, PRS-04, PRS-05, PRS-06, PRS-07, ACPT-01, ACPT-04
**Success Criteria** (what must be TRUE):
  1. The round-trip CI gate (ACPT-01) is green: `read → segment → splice-noop → write` produces byte-identical output for (a) every file in the committed synthetic corpus `crates/core/tests/fixtures/logseq-synthetic/` (CI gate, 10 fixtures covering all §6.6 patterns, no PII) and (b) every file in the opt-in real corpus `data-folder-sample/Logseq/` when present locally (gitignored — PII). Test was written BEFORE any storage/indexer/watcher implementation. Revised 2026-05-21 after PII concern split the corpus.
  2. The inventory CLI (IDX-08) runs over the real Logseq base and reports counts of `alias::`, `id::`, `:LOGBOOK:`, `#[[...]]`, `%2F`, `template::`, code-fence-in-bullet, and `SCHEDULED:`/`DEADLINE:` — gating parser sign-off.
  3. `foliom index <root>` builds a SQLite index (files, pages, blocks with `raw` + `(byte_offset, byte_length)`, tags, refs, FTS5) stored outside the notes folder (`$XDG_DATA_HOME/foliom/<root-hash>.db`); deleting the DB and re-running reproduces the same index.
  4. `foliom reindex` does incremental work — only files whose `mtime`/`hash` changed are reparsed.
  5. Parser + scanner tests run green on Linux, macOS, and Windows CI runners (ACPT-04) with NFC + forward-slash path normalization.
**Plans**: 7 plans
- [x] 01-01-PLAN.md — Workspace skeleton + RawBlock type + failing round-trip CI gate (ACPT-01 RED)
- [x] 01-02-PLAN.md — Stage 1 segmenter (TAB + 2-space continuation, fence-aware, drawer-aware) → flips ACPT-01 GREEN
- [x] 01-03-PLAN.md — Stage 2 parser (CommonMark + ref extraction) + RelativePath newtype (NFC + forward-slash)
- [x] 01-04-PLAN.md — Storage schema (migration v1), DB-location resolver, Db wrapper with PRAGMAs
- [x] 01-05-PLAN.md — Scanner with walkdir + ignore list + minimal config.edn :hidden reader
- [x] 01-06-PLAN.md — Indexer orchestrator (incremental + full reindex, per-file transactions)
- [x] 01-07-PLAN.md — CLI subcommands (index/reindex/search/dump-tree/inventory) + pinned inventory regression + green CI matrix

### Phase 2: Read-Only Web UI
**Goal**: A user can point Foliom at their Logseq folder, open `localhost` in a browser, navigate the graph by `[[links]]`/`#tags`, see backlinks, browse journals, and run full-text search — all read-only, all lazy-loaded, hitting the 5k-note performance budget.
**Depends on**: Phase 1
**Requirements**: LNK-01, LNK-02, LNK-03, LNK-05, LNK-06, LNK-07, SCH-01, SCH-02, SCH-03, UI-01, UI-02, UI-03, UI-04, EDT-08, ACPT-02, ACPT-03
**Success Criteria** (what must be TRUE):
  1. User can launch the local server, open the web UI, and navigate from any page to any other via clickable `[[page]]`, `#tag`, and `#[[multi-word tag]]` chips rendered inline mid-sentence.
  2. Every page shows a backlinks panel listing referencing blocks; journal pages display a formatted long-form title ("May 21st, 2026") and a journal navigator opens to today.
  3. `Ctrl/Cmd+K` opens a unified search palette across pages, tags, and block content via SQLite FTS5; results show highlighted snippets and click-navigate to the matching block.
  4. Read-only blocks render GFM (CommonMark + tables + syntax-highlighted code fences with line numbers + bold/italic/links) with indentation guide lines and dark-mode toggle (default follows OS).
  5. Cold start on a 5,000-note generated corpus completes in under 2 seconds (ACPT-02) and idle RSS stays under 300 MB (ACPT-03) on the reference laptop, with only visible content held in memory.
**Plans**: 8 plans
- [x] 02-01-PLAN.md — `foliom serve` scaffold: axum 0.7 on 127.0.0.1, startup reindex, Host allowlist, /api/health
- [x] 02-02-PLAN.md — REST API: pages, page detail (nested tree), backlinks, journals (today + range), search (FTS5 + tag-refs), page-titles
- [x] 02-03-PLAN.md — Frontend scaffold: Svelte 5 + Vite + TS + svelte-spa-router + stores + api wrappers + vitest
- [x] 02-04-PLAN.md — Block renderer: markdown-it custom rules ([[link]]/#tag/#[[tag]]) + Prism + GFM + indent guides + fold + block zoom
- [x] 02-05-PLAN.md — Sidebar + journal navigator + theme toggle + backlinks panel + unresolved chip styling
- [x] 02-06-PLAN.md — Search palette (Ctrl/Cmd+K) with FTS5 + tag-refs + page-title routing + click-to-block deep links
- [x] 02-07-PLAN.md — rust-embed prod build + dev-redirect to Vite + --open flag + SPA fallback
- [x] 02-08-PLAN.md — Perf harness (Criterion cold-start + sysinfo RSS) + 5k corpus gen + CI matrix update + ACPT-02/03 gates
**UI hint**: yes

### Phase 3: Outliner Editor
**Goal**: A user can click any block, edit its raw markdown in a CodeMirror 6 textarea, and save via byte-splice writeback that leaves the rest of the file byte-identical — with undo/redo, autocomplete, rename-with-backlinks, IME safety, and the round-trip gate still green after real edits.
**Depends on**: Phase 2
**Requirements**: EDT-01, EDT-02, EDT-03, EDT-04, EDT-05, EDT-06, EDT-07, EDT-09, EDT-10, EDT-11, EDT-12, EDT-13, SNC-01, SNC-02, SNC-05, LNK-04, ACPT-05
**Success Criteria** (what must be TRUE):
  1. At any moment at most one block is in edit mode showing raw markdown in a single mounted-then-unmounted CodeMirror 6 instance; all others render read-only and transition on focus/blur/Enter reparsing only that block.
  2. Keyboard model works end-to-end: Enter creates sibling, Shift+Enter inserts newline within block (code fence stays multi-line), Tab/Shift+Tab indent/outdent, Backspace-at-start merges with previous, arrows at edges navigate-and-edit; undo/redo (Ctrl/Cmd+Z, +Shift+Z) at block-edit granularity; IME composition is preserved (Pt-BR `~`+`a`→`ã` and CJK IMEs work).
  3. Saving an edited block splices its bytes into the original file at `(byte_offset, byte_length)` via atomic temp+rename; the unchanged remainder of the file is byte-identical, and the self-write hash is registered so future watcher phases can suppress echo.
  4. Renaming a page rewrites all `[[oldname]]` and `[[oldname|alias]]` references across the corpus in one atomic transaction; backlinks survive; clicking an unresolved `[[link]]` offers to create the target.
  5. Autocomplete on `[[` and `#` suggests from the indexed page/tag set; copy/cut/paste preserves block hierarchy via multi-line bullet-indented clipboard; a block context menu exposes cut/copy/duplicate/fold/zoom/copy-as-markdown.
  6. Portability check (ACPT-05): `.md` files written by Foliom open without warnings or visible diffs in Obsidian and VS Code, on a corpus exercised by real edits across this phase.
**Plans**: 7 plans
- [x] 03-01-PLAN.md — Atomic write + SelfWriteSet (sync foundation; dashmap + tempfile promotion)
- [x] 03-02-PLAN.md — Mutation engine pure logic (splice_block + TreeOp invertible apply)
- [x] 03-03-PLAN.md — Mutation REST endpoints (PUT/POST/PATCH/DELETE /api/blocks + conflict detection)
- [x] 03-04-PLAN.md — CM6 frontend: mount/unmount, IME guard, boundary keymap, history per-instance, treeOpLog
- [x] 03-05-PLAN.md — Autocomplete + bullet popover + paste detection + treeOp inverse wiring
- [x] 03-06-PLAN.md — Page rename WAL + PageHeader UX + unresolved-link silent create
- [x] 03-07-PLAN.md — ACPT-05 portability acceptance test (automated + manual checklist)
**UI hint**: yes

### Phase 4: Disk Sync
**Goal**: External edits (VS Code save, `git pull`, Syncthing storm) are detected, debounced, deduplicated against Foliom's own writes, and pushed to the UI live via SSE — and when an external edit collides with a foreground edit, the user gets a clear conflict choice.
**Depends on**: Phase 3
**Requirements**: SNC-03, SNC-04, SNC-06
**Success Criteria** (what must be TRUE):
  1. The filesystem watcher (`notify-debouncer-full` with `RecommendedCache`) detects external changes within ~250–500ms per-path debounce, survives atomic-rename saves from VS Code/Obsidian, and refreshes both the index and the open UI via SSE — with zero feedback loop from Foliom's own writes (hash self-write set with TTL).
  2. Recursive watch is registered at the parent-directory level (not per-file) to avoid Linux inotify exhaustion at ~8k files; Windows `ReadDirectoryChangesW` overflow and macOS `MustScanSubDirs` trigger a rescan fallback without dropping events.
  3. A Syncthing-style bulk-change burst (hundreds of files rewritten in seconds) is processed without UI freeze, lost events, or runaway reindex; only touched files are reparsed.
  4. When an external edit and an in-flight foreground edit collide on the same block, the user sees a conflict UI with foreground-edit-wins as the default and a one-click "discard mine / reload" option.
**Plans**: 3 plans
- [x] 04-01-PLAN.md — Backend watcher (notify-debouncer-full + DirtySet coalescing + SSE endpoint)
- [x] 04-02-PLAN.md — Frontend SSE subscription + live reload + watcher-status pill + conflict banner wire
- [x] 04-03-PLAN.md — CI smoke job (phase-4-watcher-smoke) + ACPT-04-WATCHER.md manual checklist

### Phase 5: Desktop Packaging
**Goal**: Ship Foliom as a signed, notarized Tauri 2 desktop binary on macOS and Windows that wraps the same Svelte UI consuming the in-process axum server — with footprint materially smaller than an Electron equivalent.
**Depends on**: Phase 4
**Requirements**: DSK-01, DSK-02, DSK-03
**Success Criteria** (what must be TRUE):
  1. A Tauri 2 desktop binary launches the same Svelte UI against the in-process axum server via `tauri-plugin-localhost`, with a folder picker for selecting the notes root; identical code path between web dev and desktop ship.
  2. macOS and Windows installers are code-signed; macOS is notarized; CI produces release artifacts for both platforms.
  3. Footprint CI gate passes: installer < 30 MB, idle RSS < 150 MB on the 5,000-note corpus — materially smaller than an Electron-equivalent shell.
**Plans**: 3 plans
- [ ] 05-01-PLAN.md — src-tauri/ Tauri shell: BOUND_PORT, WebviewUrl::External, folder picker, config store
- [ ] 05-02-PLAN.md — Release CI: macOS universal + Windows x64, conditional signing + notarization, GitHub Release
- [ ] 05-03-PLAN.md — Footprint gate: installer < 30 MB + idle RSS < 150 MB CI assertions
**UI hint**: yes

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Headless Indexing Core | 7/7 | Complete   | 2026-05-22 |
| 2. Read-Only Web UI | 0/8 | Not started | - |
| 3. Outliner Editor | 4/7 | In Progress|  |
| 4. Disk Sync | 3/3 | Complete   | 2026-05-22 |
| 5. Desktop Packaging | 0/3 | Not started | - |

---

## Coverage Notes

**Quality gates that span phases — assigned to earliest testable phase:**
- **ACPT-01** (round-trip CI gate over ~600-file sample) → **Phase 1**: the highest-leverage early task; must be green before any storage/indexer/watcher code lands; stays green for the life of the project (regression gate for every later phase).
- **ACPT-02** (cold start < 2s on 5k notes) → **Phase 2**: first phase with a startup path exercising index + lazy-load + UI render; performance budget becomes testable end-to-end here.
- **ACPT-03** (RSS < 300 MB idle on 5k notes) → **Phase 2**: same rationale — first phase with a steady-state memory profile to measure.
- **ACPT-04** (cross-platform CI on Linux/macOS/Windows) → **Phase 1**: parser + scanner + path normalization are the surfaces with platform divergence (NFC/NFD, backslash, MAX_PATH); CI matrix established with the foundation.
- **ACPT-05** (Obsidian/VS Code portability — files open without warnings or visible diffs) → **Phase 3**: first phase that actually writes files via byte-splice; portability becomes meaningfully testable on real edits.

**Phase 1 ordering invariant:** ACPT-01 (round-trip gate) and IDX-08 (inventory script) ship BEFORE storage schema, indexer, or watcher implementation work — per research SUMMARY, this is the single highest-leverage early task and gates parser sign-off.
