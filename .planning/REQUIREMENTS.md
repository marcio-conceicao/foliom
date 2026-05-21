# Foliom — v1 Requirements

**Source:** Synthesized from `PRD-outliner-markdown.md` (RF-01..56) + `.planning/research/SUMMARY.md` (promoted gaps). REQ-IDs follow `[CATEGORY]-NN` convention.

---

## v1 Requirements

### Indexing & Storage (IDX)

- [ ] **IDX-01**: Foliom can scan a root folder recursively and discover all `.md` files, respecting an ignore list (`logseq/`, `assets/`, `draws/`, `whiteboards/`, `bak/`, `.recycle/`, `version-files/`, plus `:hidden` entries from `config.edn` when present).
- [ ] **IDX-02**: Foliom builds a SQLite index (files, pages, blocks, tags, refs, FTS5) derived from the `.md` files; deleting the index never causes data loss.
- [ ] **IDX-03**: On startup, Foliom reindexes incrementally — only files whose `mtime`+`hash` changed are reparsed.
- [ ] **IDX-04**: User can trigger a full reindex via CLI command (e.g. `foliom reindex`).
- [ ] **IDX-05**: The blocks table stores `raw` text **plus** `(byte_offset, byte_length)` so write-back can splice changed bytes into the original file without re-serializing the AST.
- [ ] **IDX-06**: The SQLite database lives outside the notes folder (default `$XDG_DATA_HOME/foliom/<root-hash>.db`) to avoid cloud-sync corruption.
- [ ] **IDX-07**: All stored paths are normalized to NFC + forward slashes so macOS NFC/NFD and Windows backslash differences don't duplicate or lose entries.
- [ ] **IDX-08**: A one-shot inventory CLI command reports counts of Logseq-specific patterns (`alias::`, `id::`, `:LOGBOOK:`, `#[[...]]`, `%2F`, `template::`, code-fence-in-bullet, `SCHEDULED:`/`DEADLINE:`) over the user's real base — this gates M0 parser sign-off.

### Parser (PRS)

- [ ] **PRS-01**: A page is parsed into a tree of blocks (bullets nested by TAB indentation).
- [ ] **PRS-02**: Block ≠ line. Lines beneath a bullet with **2-space continuation** belong to the same block; the parser preserves code fences (` ``` `) embedded inside a bullet.
- [ ] **PRS-03**: Parsing is two-stage: (a) line-based outliner segmenter respecting TAB indent and 2-space continuation; (b) CommonMark/GFM per-block. Each block is a mini-document.
- [ ] **PRS-04**: Tag/link extraction reads the CommonMark AST and considers only text nodes — ignoring ATX headings (`# Título`), code blocks, hex colors (`#fff`), and URLs.
- [ ] **PRS-05**: Block properties (`key:: value`, including `id::`, `collapsed::`, `alias::`, `template::`, `logseq.order-list-type::`, `file::`) are preserved opaquely — parsed into a per-block `properties` slot, never rendered, written back at canonical position.
- [ ] **PRS-06**: Logseq drawers (`:LOGBOOK:` / `:END:`) are preserved opaquely attached to the parent block — never reformatted, never dropped.
- [ ] **PRS-07**: The parser round-trips byte-identical on the entire `data-folder-sample/Logseq/` corpus (~600 files); this is a CI gate. (See ACPT-01.)

### Linking & Navigation (LNK)

- [ ] **LNK-01**: Foliom recognizes `[[page]]`, `#tag`, and `#[[multi-word tag]]`; all three render inline as clickable chips/links (mid-sentence, not only at block start/end).
- [ ] **LNK-02**: Filename encoding handles spaces, accents, `&`, and `%2F` for Logseq-style namespace pages (`Parent%2FChild.md` ↔ `[[Parent/Child]]`).
- [ ] **LNK-03**: Each page renders a backlinks panel listing blocks that reference it, queried via the `refs` index.
- [ ] **LNK-04**: Clicking a link/tag navigates to the target page; clicking an unresolved link offers to create it.
- [ ] **LNK-05**: Journal pages live in `journals/` as `YYYY_MM_DD.md` and display a formatted title (default English long form, e.g. "May 21st, 2026", configurable; reads `:journal/page-title-format` from `config.edn` when present).
- [ ] **LNK-06**: A sidebar lists pages + recents + favorites; a journal navigator opens to today and lets the user jump to any date.
- [ ] **LNK-07**: User can zoom into a single block (focus mode); identity is ephemeral via URL fragment (`#block=<indent-path>`) so it survives reload but does not require IDs in the file.

### Editor — Outliner (EDT)

- [ ] **EDT-01**: At any moment **at most one block is in edit**. The focused block shows raw markdown (CodeMirror 6 textarea); all other blocks render read-only.
- [ ] **EDT-02**: Transition render → edit on focus/click; edit → render on `blur` or `Enter`, reparsing only that block's raw text.
- [ ] **EDT-03**: The raw text of a block is the only source of truth; HTML is a discardable projection. Foliom never reconstructs markdown from HTML.
- [ ] **EDT-04**: `Enter` creates a sibling block; `Shift+Enter` inserts a newline within the same block (code fence stays multi-line).
- [ ] **EDT-05**: `Tab` / `Shift+Tab` indent/outdent the current block (hierarchy mutation).
- [ ] **EDT-06**: `Backspace` at the start of a block merges with the previous block.
- [ ] **EDT-07**: `Arrow ↑` / `Arrow ↓` at block edges navigate to the neighboring block, entering edit mode.
- [ ] **EDT-08**: Block folding (collapse children) is supported per-block; UI-only by default, but persists to the `collapsed::` block property when the user explicitly toggles persistence.
- [ ] **EDT-09**: Autocomplete for `[[page]]` and `#tag` triggers on `[[` and `#` respectively, suggesting from the indexed page/tag set.
- [ ] **EDT-10**: Undo/redo works at the block-edit granularity (Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z). In-memory transaction log is acceptable for v1.
- [ ] **EDT-11**: User can copy/cut/paste blocks preserving their hierarchy (multi-line clipboard format with bullet indentation).
- [ ] **EDT-12**: A block context menu exposes the common operations (cut, copy, duplicate, fold, zoom, copy as markdown).
- [ ] **EDT-13**: IME composition is preserved on every save path (`view.composing` guard), so dead-key sequences like Pt-BR `~` + `a` → `ã` and CJK IMEs work correctly.

### Persistence & Sync (SNC)

- [ ] **SNC-01**: Edits are persisted by byte-splicing the changed block's bytes into the original file buffer at `(byte_offset, byte_length)`; unchanged portions of the file remain byte-identical.
- [ ] **SNC-02**: File writes are atomic (write to temp + rename) and registered in a self-write set (hash of the just-written content with TTL) so the watcher does not re-trigger reindex for own writes.
- [ ] **SNC-03**: A filesystem watcher (`notify-debouncer-full` `RecommendedCache`) detects external changes with ~250–500ms per-path debounce and refreshes both the index and the UI; survives atomic-rename saves from VS Code/Obsidian and bulk events from Syncthing/git.
- [ ] **SNC-04**: Recursive watch is at the parent-directory level (not per-file) to avoid Linux inotify exhaustion; Windows `ReadDirectoryChangesW` overflow and macOS `MustScanSubDirs` trigger a rescan fallback.
- [ ] **SNC-05**: Renaming a page rewrites all `[[oldname]]` and `[[oldname|alias]]` references across the corpus in one atomic transaction; backlinks survive.
- [ ] **SNC-06**: When an external edit and an in-flight foreground edit collide on the same block, the user is shown a conflict UI (foreground edit wins by default, with a one-click "discard mine / reload" option).

### Search (SCH)

- [ ] **SCH-01**: Full-text search uses SQLite FTS5 (external-content rows per block) — no requirement to hold content in memory.
- [ ] **SCH-02**: Search results show a snippet with highlighted match and navigate directly to the matching block on click.
- [ ] **SCH-03**: A global `Ctrl/Cmd+K` palette opens a unified search across pages, tags, and block content.

### Rendering & UX (UI)

- [ ] **UI-01**: Read-only blocks render GFM (CommonMark + tables + code fences with syntax highlighting via a lightweight library like Prism/starry-night + bold/italic/links). Callouts and other extensions are NOT in v1.
- [ ] **UI-02**: Dark mode is supported with a toggle; default follows the OS theme.
- [ ] **UI-03**: Indentation guide lines render between nested bullets (Logseq-style visual hierarchy).
- [ ] **UI-04**: Code fences render with detected language label and line numbers.

### Acceptance Tests / Quality Gates (ACPT)

- [ ] **ACPT-01**: Round-trip stability CI test: for every file in `data-folder-sample/Logseq/`, `read → parse → splice-noop → write` produces a byte-identical buffer. This test ships before any storage code is built and must stay green.
- [ ] **ACPT-02**: Performance CI: cold start with a 5,000-note generated corpus completes in under 2 seconds on a reference laptop (M1-class).
- [ ] **ACPT-03**: Memory CI: RSS at idle (after open + first journal) is under 300 MB on the 5,000-note corpus.
- [ ] **ACPT-04**: Cross-platform CI: parser + watcher tests run on Linux, macOS, and Windows runners.
- [ ] **ACPT-05**: Portability check: `.md` files written by Foliom open without warnings or visible diffs in Obsidian and VS Code.

### Desktop Packaging (DSK)

- [ ] **DSK-01**: Foliom ships a Tauri 2 desktop binary that wraps the same Svelte UI consuming the in-process axum server via `tauri-plugin-localhost`.
- [ ] **DSK-02**: macOS and Windows installers are code-signed; macOS is notarized.
- [ ] **DSK-03**: Desktop binary RSS + disk footprint is materially smaller than an Electron equivalent (target: < 30 MB installer, < 150 MB RSS at idle).

---

## v2 / Later (Deferred)

- [ ] Drag-and-drop block reorder — cut/paste covers it for v1.
- [ ] Slash commands / command palette beyond `Ctrl/Cmd+K`.
- [ ] TODO/DONE/DOING/LATER/NOW workflow markers as block state (with filter/agenda views). v1 preserves them as plain text.
- [ ] `SCHEDULED:` / `DEADLINE:` timestamps as queryable state.
- [ ] `alias::` block property interpretation in `[[link]]` resolution. v1 preserves opaquely.
- [ ] Right-side reference pane (open multiple pages simultaneously).
- [ ] Page graph view.
- [ ] Spaced repetition (`{{cloze}}` rendering).
- [ ] Excalidraw/whiteboard embed rendering.

## Out of Scope (v1 explicit "no")

- **Logseq plugin compatibility** — out of scope of the v1 product.
- **Block references `((uuid))` and any IDs of blocks injected into `.md`** — breaks file portability, contradicts §5.6.
- **WYSIWYG round-trip (reconstructing markdown from HTML)** — source of truth is always the raw text.
- **Own sync protocol or real-time collaboration** — delegated to Syncthing/git/Dropbox.
- **Mobile app** — single-user desktop/web only in v1.
- **Org-mode files** — markdown only in v1.
- **AI / LLM features (semantic search, summarization, chat)** — explicitly deferred.
- **`((block-uuid))` migration tool for existing Logseq bases** — preserve verbatim but do not interpret.

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| IDX-01 | Phase 1 | Pending |
| IDX-02 | Phase 1 | Pending |
| IDX-03 | Phase 1 | Pending |
| IDX-04 | Phase 1 | Pending |
| IDX-05 | Phase 1 | Pending |
| IDX-06 | Phase 1 | Pending |
| IDX-07 | Phase 1 | Pending |
| IDX-08 | Phase 1 | Pending |
| PRS-01 | Phase 1 | Pending |
| PRS-02 | Phase 1 | Pending |
| PRS-03 | Phase 1 | Pending |
| PRS-04 | Phase 1 | Pending |
| PRS-05 | Phase 1 | Pending |
| PRS-06 | Phase 1 | Pending |
| PRS-07 | Phase 1 | Pending |
| ACPT-01 | Phase 1 | Pending |
| ACPT-04 | Phase 1 | Pending |
| LNK-01 | Phase 2 | Pending |
| LNK-02 | Phase 2 | Pending |
| LNK-03 | Phase 2 | Pending |
| LNK-05 | Phase 2 | Pending |
| LNK-06 | Phase 2 | Pending |
| LNK-07 | Phase 2 | Pending |
| SCH-01 | Phase 2 | Pending |
| SCH-02 | Phase 2 | Pending |
| SCH-03 | Phase 2 | Pending |
| UI-01 | Phase 2 | Pending |
| UI-02 | Phase 2 | Pending |
| UI-03 | Phase 2 | Pending |
| UI-04 | Phase 2 | Pending |
| EDT-08 | Phase 2 | Pending |
| ACPT-02 | Phase 2 | Pending |
| ACPT-03 | Phase 2 | Pending |
| EDT-01 | Phase 3 | Pending |
| EDT-02 | Phase 3 | Pending |
| EDT-03 | Phase 3 | Pending |
| EDT-04 | Phase 3 | Pending |
| EDT-05 | Phase 3 | Pending |
| EDT-06 | Phase 3 | Pending |
| EDT-07 | Phase 3 | Pending |
| EDT-09 | Phase 3 | Pending |
| EDT-10 | Phase 3 | Pending |
| EDT-11 | Phase 3 | Pending |
| EDT-12 | Phase 3 | Pending |
| EDT-13 | Phase 3 | Pending |
| SNC-01 | Phase 3 | Pending |
| SNC-02 | Phase 3 | Pending |
| SNC-05 | Phase 3 | Pending |
| LNK-04 | Phase 3 | Pending |
| ACPT-05 | Phase 3 | Pending |
| SNC-03 | Phase 4 | Pending |
| SNC-04 | Phase 4 | Pending |
| SNC-06 | Phase 4 | Pending |
| DSK-01 | Phase 5 | Pending |
| DSK-02 | Phase 5 | Pending |
| DSK-03 | Phase 5 | Pending |

**Coverage:** 56/56 v1 requirements mapped to exactly one phase. No orphans, no duplicates.

---
*Generated 2026-05-21 from PRD + research synthesis.*
