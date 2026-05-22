---
phase: 03-outliner-editor
phase_number: 3
created: 2026-05-22
mode: standard
---

# Phase 3 — Outliner Editor: Context

**Goal (ROADMAP):** A user can click any block, edit its raw markdown in a CodeMirror 6 textarea, and save via byte-splice writeback that leaves the rest of the file byte-identical — with undo/redo, autocomplete, rename-with-backlinks, IME safety, and the round-trip gate still green after real edits.

**Requirements (17):** EDT-01..07, EDT-09, EDT-10, EDT-11, EDT-12, EDT-13, SNC-01, SNC-02, SNC-05, LNK-04, ACPT-05

**Depends on:** Phase 2 (complete — frontend Block.svelte renderer, REST API, sidebar, palette, embed all shipped).

---

## Pre-locked Decisions (from PRD + research, not re-asked)

| Area | Decision | Source |
|---|---|---|
| Source of truth | Raw text per block. HTML render is a discardable projection. **Never** reconstruct markdown from HTML. | RF-13, EDT-03 |
| Write-back model | Byte-splice into `(byte_offset, byte_length)` of the original file via atomic temp + rename. Index stores `raw` + offsets in parallel (Phase 1 IDX-05). | SNC-01, research SUMMARY tension resolution |
| Self-write suppression | Hash of just-written content registered in a TTL set so Phase 4 watcher can suppress its own echo. The set must already exist in Phase 3, even though no watcher consumes it yet. | SNC-02, research PITFALLS #2 |
| CM6 mount pattern | **Single instance per focused block**, mounted on focus / destroyed on blur. **Never** DOM-reparent an existing `EditorView` (breaks IME composition — catastrophic for Pt-BR `~`+`a`→`ã`). | research PITFALLS #5 |
| IME guard | Every save path checks `view.composing` before reading `view.state.doc.toString()`. | EDT-13, research PITFALLS #5 |
| Boundary key interception | ArrowUp/Down/Backspace at block edges intercepted with `Prec.highest` so CM6 default behavior doesn't fire first. | EDT-05, EDT-06, EDT-07 |
| Caret on click | v1 accepts "end of block" fallback if `view.posAtCoords()` is fragile. | PRD §12.2 |

---

## Decisions Locked in This Discussion

### D-30-01: Save trigger model — Blur + Enter

Save (byte-splice writeback) fires on:
1. The focused block loses focus (`blur` event on the CM6 view's container).
2. The user presses `Enter` (which creates a sibling — implicitly commits the current block first).

**Not** debounced autosave while typing. **Not** explicit `Ctrl+S`.

Implications for planning:
- Mutation API in `crates/core` takes `(file_id, block_id, new_raw)` and returns the new `(byte_offset, byte_length)`.
- Frontend `Block.svelte` switches to CM6 on focus, reads `view.state.doc.toString()` on blur, calls `PUT /api/blocks/:id` (or whatever the route ends up being), then re-renders the read-only HTML from the response.
- Risk: crashed tab loses mid-edit text. Acceptable for v1.0. v1.x may add a draft-to-localStorage safety net.

### D-30-02: Rename UX — Click on PageHeader title

Clicking the page title in `PageHeader.svelte` transforms it into an editable input. `Enter` confirms, `Esc` cancels.

If the page has backlinks, show a confirmation modal: "Rewrite N references to `[[oldname]]` across M files?" with `Rewrite all` / `Rename without rewriting` / `Cancel`. Default = Rewrite all.

Implications:
- `PageHeader.svelte` already exists from Phase 2 plan 02-04 — gets an in-place edit affordance.
- Backend needs a `POST /api/pages/:name/rename { newName }` endpoint that runs an atomic SQL transaction over `refs` + filesystem rename + backlinks rewrite via byte-splice on every referencing file.
- Collision (target name exists): reject with 409 + clear error in the modal.

### D-30-03: Unresolved-link click — Silent create

Clicking a `[[link]]` whose target page does not exist creates an empty `<name>.md` in `pages/` (or `journals/` if the name matches `YYYY_MM_DD`) and navigates to it. No modal.

Risk: typos materialize as ghost pages. Mitigation: the existing `.page-link.unresolved` styling (red-ish from Phase 2 plan 02-05) gives a visual hint **before** the click — user can hover/check spelling.

Implications:
- Frontend route handler on `[[link]]` click: if `unresolved`, hit `POST /api/pages` `{ name }` then navigate.
- Backend creates the file with a single empty bullet `-\n` (or just an empty file? — to be decided in planning; tests can pin one).
- Indexer must pick up the new file on next reindex. Phase 4 watcher will surface it live; Phase 3 may need a synchronous reindex of the single new file.

### D-30-04: Block context menu — Bullet click → popover

Clicking the bullet `•` of any read-only block opens a popover with: `Cut block` / `Copy block` / `Duplicate` / `Fold` / `Zoom` / `Copy as markdown`. Clicking outside closes it.

Right-click stays as the browser's native context menu (preserves "select text", "search Google for...", etc).

Implications:
- `Block.svelte` already renders the bullet — wire a click handler.
- Popover positioning: simple absolute-positioned `<menu>` element. No portal-to-body unless overflow clipping bites; defer that complexity.
- Keyboard equivalent: `Ctrl+.` opens the menu for the focused block? — defer to planning.

### D-30-05: Undo/redo scope — Hybrid (CM6 history per block + custom log for tree ops)

- **While a block is in edit mode:** Ctrl+Z / Ctrl+Shift+Z use CM6's native `history()` extension. Granularity is per-input-action (CM6 default).
- **When no block is in edit mode (or for tree-shape operations like Tab/Shift+Tab/Backspace-merge/move-block):** a separate frontend transaction log records each tree op as one undoable step. Ctrl+Z outside edit mode pops the last tree-op.

Implications:
- Frontend state: a `treeOpLog: TreeOp[]` store with capacity (200 entries?) — to be sized in planning.
- TreeOp variants: `Indent { blockId }`, `Outdent { blockId }`, `Merge { blockId, mergedInto }`, `Split { blockId, atOffset }`, `Move { blockId, newParent, newOrd }`, `Delete { blockId, parentSnapshot }`.
- Server-side: each tree op already maps to a single mutation API call; undo replays the inverse op. No server-side undo log needed.
- Risk: divergence between CM6 history and tree-op log. Document a clear rule: **Ctrl+Z while focused in CM6 always uses CM6 history; Ctrl+Z while focus is on a read-only block (or document body) uses tree-op log.** Visually distinct focus state already exists.

### D-30-06: Tag autocomplete source — Combined (tags + page names)

On `#` trigger: fetch from a backend endpoint that returns both:
- Distinct `target_page` from `refs` table where `kind='tag'` (= tags actually in use).
- Page names from `pages` (so `#[[Page Name]]` can be quickly created without leaving the keyboard).

Display in the autocomplete with type labels (e.g., `🏷 work` for tag, `📄 Speech Analytics` for page).

On `[[` trigger: fetch only page names (already exposed via `/api/page-titles` from Phase 2 plan 02-02).

Implications:
- New endpoint or extension: `/api/autocomplete?prefix=&kind=tag|page|all`. Reuses `/api/page-titles` cache for pages.
- Frontend wires CM6 autocomplete extension (`@codemirror/autocomplete`) — already pinned as a transitive dep via `@codemirror/lang-markdown`; verify in planning.

### D-30-07: Paste behavior — Detect bullet hierarchy

On paste into a CM6 view:
1. Read `event.clipboardData.getData('text/plain')`.
2. If the content matches `^(\t*)- ` on at least 2 lines, parse it as a bullet tree (using the same line-segmenter logic the backend uses — port it to TS or call a backend endpoint to parse).
3. Insert the resulting tree as siblings/children of the current block.
4. Otherwise: insert as raw text into the current CM6 view.

Implications:
- Need TS port of the line-segmenter — already trivial (10 lines: count leading TABs, group continuation 2-space lines). Defer the heavy CommonMark per-block parsing; just bullet detection here.
- Pasting from Foliom-to-Foliom (cut/copy block ops) uses the same format on the clipboard — that's the round-trip guarantee.

### D-30-08: Backspace on empty block — Delete block

When `Backspace` is pressed inside an empty block (CM6 doc.length === 0):
- Delete the block from the tree.
- Move focus to the previous block (or parent if no previous sibling), positioning cursor at end of its content.
- This is one entry in the tree-op log (`Delete`).

Different from Backspace-at-start of a non-empty block (which is `Merge` with previous).

Implications:
- One additional TreeOp variant. Undo restores the empty block at its position.

---

## Decisions Deferred to Planning / Research

These don't block CONTEXT.md but should be settled by the gsd-planner:

1. **Mutation API surface** — REST endpoints: `PUT /api/blocks/:id` (edit text)? `POST /api/blocks` (insert)? `PATCH /api/blocks/:id` (move/indent)? Or a single batched `POST /api/mutations`? Planning to pick the shape consistent with the byte-splice contract.
2. **Conflict during long edit** — if the file changes on disk while a block is mid-edit (no watcher yet, but `mtime`/`hash` check on save can catch this). v1: refuse save with 409, surface in UI "External edit detected — reload?". Confirm in planning.
3. **Empty-file create on unresolved-link click** — `-\n` vs truly empty file? Indexer must accept both.
4. **CM6 autocomplete extension** — vendored sub-dep vs add `@codemirror/autocomplete` explicitly to frontend package.json.
5. **Tree-op log capacity** — 200 entries? Stored in memory only (lost on reload)?
6. **Live re-render after save** — does the backend mutation response include the parsed block (props/drawers/raw) so the frontend doesn't need a follow-up GET? Probably yes for latency.
7. **Bullet popover positioning** — absolute vs floating-ui. Pick lightest viable in planning.
8. **Keyboard shortcuts NOT yet decided:** `Ctrl+Enter`? `Alt+Shift+ArrowUp/Down` to move block? List in 03-RESEARCH and pin in plans.

---

## Scope Guardrails (Phase 3 boundaries)

**In scope:** Everything in the 17 REQs above + the 8 decisions in this CONTEXT.md.

**Out of scope (defer to phase backlog):**
- File-system watcher and SSE live updates → Phase 4.
- Conflict-resolution UI for external edits during foreground edit → Phase 4 (SNC-06).
- Drag-and-drop block reorder → v1.x (REQUIREMENTS deferred).
- Slash commands beyond Ctrl+K palette → v1.x.
- `((block-uuid))` references → v1.x (explicitly out per PRD §3.2 and not in REQUIREMENTS).
- Workflow markers as state (`TODO`/`DONE`/`SCHEDULED:`) → v1.x.
- `alias::` block-property resolution → v1.x.

---

## Open Questions for Research Phase

The gsd-phase-researcher should investigate these before planning starts:

1. **CodeMirror 6 markdown extension config** — which combinations of `@codemirror/lang-markdown`, `@lezer/markdown`, history, autocomplete, key-bindings produce the IME-safe + boundary-key-controllable behavior we need? Cite specific docs/issues.
2. **Atomic file rename + self-write suppression on Windows** — what guarantees does `tempfile::NamedTempFile` + `persist` give vs `std::fs::rename`? Confirm on Windows specifically (no antivirus surprises).
3. **Backlinks rewrite atomicity** — rename a page that has 100 backlinks across 50 files. SQL transaction in `refs` is easy; the 50 file rewrites are not transactional. What's the recovery model if rewrite-N fails mid-batch? Two-phase commit with a journal? Best-effort with detailed error?
4. **CM6 history reset on mount/unmount** — does each fresh `EditorView` get its own clean history, or does state need explicit reset? Confirm so Ctrl+Z doesn't reach across blocks.
5. **Pt-BR IME (XCompose / dead-key) test fixture** — what's the actual sequence of `compositionstart` / `compositionupdate` / `compositionend` events for `~` + `a` → `ã` on Linux/Windows/macOS? Need this to write a regression test.
6. **`copy as markdown` block format** — TAB indent? Just bullets? Preserve properties/drawers or render them out? Lock the format so paste-back round-trips.

---

## Project Skills / Patterns to Reuse

Reusable assets already in tree (do not re-build):
- `crates/core/src/parser/segment.rs` — line-based segmenter (use to validate TS-port of paste detection).
- `crates/core/src/parser/ast.rs::strip_segmenter_prefix` — block raw text helpers.
- `crates/core/src/path.rs` — NFC + forward-slash normalization (any new file create/rename must go through this).
- `crates/cli/src/cmd/serve/dto.rs` — extend `Block` DTO with whatever mutation response shape is decided.
- `crates/cli/src/cmd/serve/routes/pages.rs::assemble_tree` — same tree-walk shape used in the mutation response.
- `frontend/src/lib/components/Block.svelte` — main extension point (add CM6 mount + bullet popover + autocomplete).
- `frontend/src/lib/components/PageHeader.svelte` — main extension point for rename UX.

---

## Next Step

`/gsd-plan-phase 3` — should spawn gsd-phase-researcher first (research enabled in config.json), then gsd-planner to produce 6-8 plans matching the granularity of Phase 2.
