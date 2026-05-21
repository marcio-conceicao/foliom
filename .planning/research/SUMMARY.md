# Project Research Summary

**Project:** Foliom
**Domain:** Local-first markdown outliner / PKM (Logseq/Roam alternative)
**Researched:** 2026-05-21
**Confidence:** HIGH (stack, architecture, pitfalls); MEDIUM-HIGH (feature gap analysis)

## Executive Summary

Foliom is a single-user, local-first markdown outliner whose core differentiator is **not features but performance and file purity**: sub-second cold start on a 5k+ note graph, RAM proportional to what's open, and byte-stable `.md` round-trip (no `id::` pollution, no indentation drift). The recommended way to build this is a **Rust core** (`pulldown-cmark` + `rusqlite` w/ bundled FTS5 + `notify-debouncer-full`) exposing a localhost HTTP/SSE API consumed by a **Svelte 5 + CodeMirror 6** frontend, later wrapped in **Tauri 2** — same code path in browser dev and desktop ship. Go + Wails v2 is a viable runner-up but cedes the cold-start narrative that justifies the project's existence.

The architecture is a strict inward-pointing dependency graph: `desktop → server → core`, where `core` (scanner, two-stage parser, indexer, watcher, storage) owns all IO and has no knowledge of HTTP. The parser is deliberately **two-stage** — a bespoke line-based outliner segmenter that respects TAB indentation and 2-space continuation, followed by per-block CommonMark — because letting CommonMark drive segmentation will misparse Logseq's TAB-indented bullets with embedded code fences (validated against `data-folder-sample/Logseq/journals/2023_11_09.md`). Blocks are **materialized in SQLite** with per-block FTS5 (external-content) so search hits and backlinks resolve directly to a `block_id` without re-parsing at query time.

The dominant risk is **lossy markdown round-trip**: any edit-then-save flow that re-serializes from a CommonMark AST will silently mutate TAB indents, drawer formatting, bullet markers, and trailing whitespace across the user's entire 600-file Logseq base on first save. The mitigation is non-negotiable and dictates the entire M0 design: **the index stores byte-range tuples `(file_id, byte_offset, byte_length)` for every block alongside the `raw` text**; on edit, only the changed block's bytes are spliced into the original file buffer — the unchanged 99% of the file is byte-identical to what was read. A property-based round-trip CI gate against all ~600 sample files is the single highest-leverage early task in the project. Combined with hash-based watcher self-write suppression and per-block FTS, this is the foundation that everything else stands on.

## Key Findings

### Recommended Stack

Rust backend + Svelte 5/CM6 frontend + Tauri 2 desktop shell. Same axum HTTP server serves the UI in `cargo run` dev mode (via Vite proxy) and inside the Tauri webview (via `tauri-plugin-localhost`) — zero code fork between web and desktop builds. See `STACK.md` for full rationale.

**Core technologies:**
- **Rust 1.85+** — zero-GC cold start, smallest binary, same language across core and Tauri shell.
- **pulldown-cmark 0.13** — event stream with `into_offset_iter()` exposes byte spans (load-bearing for RF-21 tag/link extraction and byte-range block storage).
- **rusqlite 0.39 `bundled`** — embedded SQLite with FTS5 compiled in; no system-libsqlite hunt across distros.
- **notify 6 + notify-debouncer-full 0.3** — only correct cross-platform watcher; `RecommendedCache` tracks file identity across atomic-save renames.
- **axum 0.7 + SSE** — REST for queries/mutations, SSE for server→client live updates (no WebSocket needed).
- **Svelte 5.37 (runes)** — smallest runtime for matched feature set.
- **CodeMirror 6 (modular)** — single editor instance mounted/unmounted on focus transitions (never reparented).
- **Tauri 2.9** — desktop wrapper; Wails v3 is alpha and disqualified.

### Expected Features

The PRD covers the read/write/index baseline well, but research surfaced **critical table-stakes gaps** that a Logseq refugee would notice on day 1. See `FEATURES.md` for the full 20+ gap list.

**Must have (table stakes — gaps to promote into REQUIREMENTS.md):**
- **Undo/redo (RF-32)** — **CRITICAL** PRD gap; non-negotiable for any editor.
- **Page rename with backlink rewrite (RF-37)** — **CRITICAL** PRD gap; without it, rename corrupts the graph.
- Block folding (RF-17), zoom into block (RF-18), bullet-click navigation (RF-19).
- `[[page]]` and `#tag` autocomplete (RF-24, RF-25) — linking unusable at scale without it.
- Journal navigation + "open to today" (RF-26, RF-27).
- Sidebar with page list + recents (RF-28).
- Block context menu, copy/paste blocks with hierarchy (RF-31, RF-33).
- Dark mode, indentation guides, `Ctrl/Cmd+K` search (RF-35, RF-36, RF-48).
- Round-trip stability acceptance test (RF-44) — proves Logseq-compat actually works.

**Should have (differentiators that justify switching from Logseq):**
- Sub-second cold start + <300MB RAM on 5k notes (the wedge).
- Resilient watcher that survives Syncthing rename storms (RF-45).
- Byte-stable `.md` (no `id::` pollution).
- Search results with block-in-tree context.

**Defer (v1.x or v2+):**
- Drag-and-drop block reorder (RF-34) — cut/paste covers it for M2.
- Slash commands, command palette, right-pane (RF-38, RF-39, RF-29).
- TODO/DONE checkbox rendering (RF-49) — answer §12.9 post-M2.
- Block refs `((uuid))`, plugins, mobile, real-time sync, graph view, AI — **anti-features**, document the "no".

### Architecture Approach

Inward-pointing dependency graph (`desktop → server → core`) with the watcher inside `core` because it's a file-IO concern. The backend owns all IO; the frontend is identical between web and Tauri builds, both consuming `http://127.0.0.1:<port>` over REST+SSE. The schema materializes blocks in SQLite (per-block FTS5 row, external-content) so search and backlinks resolve to `block_id` without re-parsing. See `ARCHITECTURE.md` §1, §3.Q2, §5.

**Major components (M0→M4 build order):**
1. **Storage** (schema + migrations + WAL) — M0 first.
2. **Scanner** (walk + ignore lists + stat) — M0.
3. **Two-stage parser** — Stage 1: line-based outliner segmenter (TAB + 2-space continuation, fence-aware); Stage 2: CommonMark per block. M0.
4. **Indexer** (orchestrates scan→parse→transactional write) — M0.
5. **CLI** (`index`, `search`, `dump-tree`) — M0 gate.
6. **Query layer** — M1 (read-only HTTP).
7. **Mutation layer** (byte-splice writeback, NOT whole-file re-serialization) — M2.
8. **Watcher** (notify + debouncer + hash-based self-write suppressor) — M3.
9. **Tauri shell** — M4.

### Critical Pitfalls

Top items from `PITFALLS.md`. Most must be addressed in **M0 design**, not deferred.

1. **Lossy markdown round-trip (CRITICAL, M0).** All CommonMark parsers are lexers, not formatters; re-serializing from AST silently mutates TAB indents, `:LOGBOOK:` drawers, `key:: value` properties, trailing newlines. **Mitigation:** byte-range splice on edit (see Block Storage Model resolution below); property-based round-trip CI test on all ~600 sample files as the **first test written**.
2. **Watcher loop (CRITICAL, M0 design / M3 impl).** Naive time-based dedup fails under clock skew, atomic-rename saves, and Syncthing bulk writes. **Mitigation:** hash-of-just-written set with TTL; per-path debounce (~250–500ms); handle Windows ReadDirectoryChangesW overflow + macOS `MustScanSubDirs` explicitly with rescan fallback; watch only `.md`; recursive parent-dir watch (per-file exhausts inotify at 8k files).
3. **CommonMark TAB-quirk collision (CRITICAL, M0).** CommonMark says 4-space indent = code block; Logseq nests with TAB. Single-pass CommonMark misparses deeply-nested bullets and code fences inside bullets. **Mitigation:** two-stage parser (segmenter strips TABs before CommonMark sees the block content); golden fixtures for every quirk.
4. **Logseq compat — drawers, `%2F` namespaces, `id::`/`alias::` properties (CRITICAL, M0).** PRD §6.6 underspecifies these. **Mitigation:** inventory script over the real 600 files BEFORE finalizing M0 parser; treat `:LOGBOOK:`/`:END:` drawers and `key:: value` lines as opaque (parsed into a `properties: []` slot per block, never rendered, written back at canonical position); decode/encode `%2F` for namespace pages; defer `alias::` resolution to v1.1 but preserve verbatim from day 1.
5. **CodeMirror 6 focus / IME / boundary keys (HIGH, M2).** Single CM6 instance mount/unmount (never DOM-reparent — breaks IME composition, catastrophic for Pt-BR `~` + `a` → `ã`); guard every save path with `view.composing`; intercept boundary keys with `Prec.highest`.
6. **Cross-platform path handling (HIGH, M0).** macOS NFC/NFD, Windows case-insensitivity + reserved names + 260-char MAX_PATH, forward-slash paths in SQLite. Normalize all stored paths to NFC + forward slashes at storage boundary; CI matrix Linux/macOS/Windows.
7. **DB in cloud-synced folder (M0).** Default DB location must be **outside** the notes folder (`$XDG_DATA_HOME/foliom/<root-hash>.db`).

## Key Tensions Resolved

### Block Storage Model (resolves PRD §12.3)

ARCHITECTURE recommended materializing `blocks` with a `raw` TEXT column; PITFALLS argued byte-range tuples `(file_id, offset, length)` are mandatory to avoid re-serialization drift. **Both are correct and not in conflict.** The `blocks` table stores **both**:

```sql
CREATE TABLE blocks (
  id           INTEGER PRIMARY KEY,
  page_id      INTEGER NOT NULL,
  parent_id    INTEGER,
  ord          INTEGER NOT NULL,
  depth        INTEGER NOT NULL,
  raw          TEXT NOT NULL,        -- for FTS indexing & cheap reads
  byte_offset  INTEGER NOT NULL,     -- for safe write-back via splice
  byte_length  INTEGER NOT NULL,
  hash         BLOB NOT NULL
);
```

- **On indexing / query / FTS / backlinks:** use `raw`.
- **On write-back (mutation):** load original file bytes, splice the changed block's bytes into `[byte_offset .. byte_offset+byte_length)`, atomic-write. The unchanged 99% of the file is byte-identical.
- **On external change detected by watcher:** invalidate the affected file's blocks, reparse, recompute offsets.

Preserves the "cache is derivable, can be deleted" invariant (PRD §5.1) while making byte-stable round-trip the default code path. **Roadmap impact:** M0 must build both the byte-range parser API AND the splice-based mutation layer (M2) on top of it — never a whole-file `serialize(tree)` function. The single highest-leverage early test is `parse → splice-noop → assert byte-equal` against all ~600 sample files.

### PRD Critical Gaps to Promote to REQUIREMENTS.md

1. **Undo/redo (RF-32)** — non-negotiable for an editor; M2 ship.
2. **Page rename with backlink rewrite (RF-37)** — without it, rename is graph-corrupting. M2.
3. **Round-trip stability acceptance test (RF-44)** — CI gate that proves RF-50/51/54 + byte-splice architecture work.

### Logseq Compat Is Wider Than PRD §6.6 Admits

- `:LOGBOOK:` / `:END:` drawers (Org-mode style) — preserve opaquely, attached to parent block, never re-formatted.
- `%2F` URL-encoded namespace separators (`Parent%2FChild.md` ↔ `[[Parent/Child]]`) — decode for display, encode for write.
- Drawer-vs-2-space-continuation interaction — naively, drawer lines look like orphan paragraphs and get dropped on save.
- `config.edn` (Clojure EDN, not JSON/TOML) — vendored minimal EDN reader or documented-scope regex extraction.
- `id::` block-property lines — parsed into `properties` slot, never rendered, written back at canonical position.

**REQUIREMENTS.md adds an inventory-script deliverable:** scanner over the real ~600 files reporting counts of each pattern (`alias::`, `id::`, `:LOGBOOK:`, `#[[...]]`, `%2F`, `template::`, code-fence-in-bullet, `SCHEDULED:`/`DEADLINE:`). Gates M0 parser sign-off.

## Implications for Roadmap

The PRD's M0–M4 phasing is correct; research sharpens the gates.

### Phase M0 — Headless Indexing Core
**Rationale:** Carries the most pitfall-prevention weight. Decisions here (byte-range parser API, hash-based watcher dedup, blocks schema with both `raw` and byte offsets, DB-outside-notes-folder, NFC path normalization, transactional reindex) cascade into every later milestone.
**Delivers:** CLI (`foliom index`, `search`, `dump-tree`); SQLite schema with migrations; two-stage parser; scanner with ignore lists; round-trip CI gate over all ~600 sample files; inventory script.
**Addresses:** RF-01..04, RF-10, RF-16, RF-21, RF-50..56.
**Critical early task:** byte-range round-trip parser tested against all ~600 sample files BEFORE storage/indexer/watcher work.

### Phase M1 — Read-Only Web UI
**Rationale:** Splitting read from write lets indexer/parser stabilize before edit complexity. Independently useful as a "fast Logseq viewer" for dogfooding.
**Delivers:** axum HTTP server (REST queries), Svelte + markdown-it renderer, navigation by `[[link]]`/`#tag`, backlinks panel, FTS search with snippets, journal navigation, sidebar, dark mode, block folding, zoom into block.
**Addresses:** RF-20..23, RF-30..31; promoted RF-17, RF-18, RF-19, RF-26, RF-27, RF-28, RF-35, RF-36, RF-48; resolve §12.5.

### Phase M2 — Outliner Editor
**Rationale:** Edit is the highest-risk UX surface. Arrives after M1 has proven read paths and M0's byte-splice writeback is exercised.
**Delivers:** CM6 single-block editor with mount/unmount discipline, IME-safe save path, boundary-key handling, autocomplete for `[[page]]`/`#tag`, block context menu, copy/cut/paste with hierarchy, **undo/redo**, **page rename with backlink rewrite**, atomic byte-splice write-back, round-trip CI gate as ship blocker.
**Addresses:** RF-11..16, RF-40..41; promoted RF-24, RF-25, RF-31, **RF-32**, RF-33, **RF-37**, RF-44, RF-46, RF-47.

### Phase M3 — Disk Sync
**Rationale:** Watcher complexity is its own milestone. Built after M2 so the mutation path it coordinates with already exists.
**Delivers:** notify + notify-debouncer-full integration, hash-based self-write filter, SSE event broker, frontend SSE subscription, sync-conflict UI, bulk-change resilience.
**Addresses:** RF-40, RF-05; RF-45 (Syncthing storm survival).
**Note:** Code-signing cert procurement starts in late M3 (lead time before M4).

### Phase M4 — Desktop Packaging
**Rationale:** Last by design — Tauri shell over already-proven web stack via `tauri-plugin-localhost`.
**Delivers:** Tauri 2 shell, folder picker, universal macOS binary, signed Win/macOS installers, footprint CI gate (RNF-05).

### Phase Ordering Rationale

- **M0 first** is non-negotiable: byte-range parser, hash-based watcher fence design, blocks schema, DB location all dictate downstream interfaces.
- **Read before Write (M1 before M2)** is dictated by lazy-loading architecture.
- **Editor before Watcher (M2 before M3)** is dictated by self-write suppression coordinating with a mutation layer.
- **Desktop last (M4)** — Tauri is a thin wrapper.

### Research Flags

Phases likely needing deeper research during planning:

- **M0** — Logseq base inventory script before locking parser behavior; `config.edn` EDN parsing scope.
- **M2** — CM6 IME boundary-key test matrix on a 3-block toy with Pt-BR dead-keys AND a CJK IME before integrating.
- **M4** — Code-signing logistics (Apple Developer Program $99/yr, Windows OV cert ~$300/yr) have weeks-long lead times; start admin work during M3.

Standard patterns (skip extra research):
- **M1** — axum + Svelte + markdown-it + SSE is well-documented.
- **M3** — `notify-debouncer-full` `RecommendedCache` is the documented pattern.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Crates verified via Context7; Tauri 2.9 confirmed; Wails v3 disqualified as alpha. |
| Features | MEDIUM-HIGH | Feature parity drawn from training data; user should sanity-check differentiator calls against own workflow. |
| Architecture | MEDIUM-HIGH | Parser strategy validated against `2023_11_09.md`. Some library-behavior specifics MEDIUM — re-verify in M0 spike. |
| Pitfalls | MEDIUM-HIGH | De-facto failure modes of this product category. Direct inspection of sample base confirms patterns. |

**Overall confidence:** HIGH for the recommended path.

### Gaps to Address

- **Live verification of library versions** at M0 kickoff.
- **Real-base inventory pending** — M0 produces it before locking parser.
- **Open PRD §12 decisions** to answer during REQUIREMENTS.md:
  - §12.1 `#tag` vs `[[page]]` entity model (recommendation: same entity, two reference types).
  - §12.2 caret on click (recommendation: end-of-block for v1).
  - §12.5 GFM scope (recommendation: tables YES M1; code-fence highlighting YES M1 via lightweight Prism/starry-night).
  - §12.8 `alias::` interpretation (recommendation: preserve opaque v1, opt-in resolution v1.1).
  - §12.9 TODO/DONE (recommendation: checkbox render v1.x; no agenda/scheduling in v1).
- **User workflow sanity check** — daily reliance on slash commands, graph view, or `((block refs))`? Anti-feature calls assume "no".

## Sources

### Primary (HIGH confidence)
- Context7 `/websites/rs_tauri_2_9_5`, `/websites/v2_tauri_app` — Tauri 2.9 stable + `tauri-plugin-localhost`.
- Context7 `/websites/v3_wails_io` — Wails v3 alpha (disqualifying).
- Context7 `/pulldown-cmark/pulldown-cmark` — `into_offset_iter()` byte-span API.
- Context7 `/notify-rs/notify` — `notify-debouncer-full` + `RecommendedCache`.
- Context7 `/websites/rs_rusqlite_0_39_0_rusqlite` — `bundled` / `bundled-full` FTS5.
- Context7 `/sveltejs/svelte`, `/codemirror/website` — current stable APIs.
- Direct inspection `data-folder-sample/Logseq/journals/2023_11_09.md` — validates two-stage parser strategy.
- Direct inspection `PRD-outliner-markdown.md` — RF baseline and open decisions.

### Secondary (MEDIUM confidence)
- Training-data knowledge of Logseq, Roam, Workflowy, Dynalist, Obsidian, Tana.
- Training-data knowledge of CommonMark spec quirks, CM6 internals (`view.composing`, `Prec`), SQLite WAL/FTS5 patterns.

### Tertiary (LOW confidence — verify in M0 spike)
- Specific GitHub-issue references for `pulldown-cmark-to-cmark` drift and comrak `format_commonmark` caveat.
- Exact debounce window tuning (250–500ms) — needs empirical pass under Syncthing load.
- inotify watch limit (~8192) — distro-dependent.

---
*Research completed: 2026-05-21*
*Ready for roadmap: yes*
