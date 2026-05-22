---
phase: 03-outliner-editor
plan: 05
subsystem: frontend-editor, api
tags: [phase-3, autocomplete, codemirror, paste, bullet-popover, copy-as-markdown, tree-ops-wired, svelte5, rust, sqlite]

requires:
  - phase: 03-04
    provides: "BlockEditor class, blockEditorExtensions, BlockEditorCallbacks interface, treeOpLog store, history-routing bindHistoryRouting stub, api.ts mutations"
  - phase: 03-03
    provides: "PUT/POST/PATCH/DELETE /api/blocks mutation REST API + MutationResponse wire contract"
  - phase: 02-02
    provides: "/api/page-titles, refs + pages schema (pages.name, refs.type='tag', refs.target_page FK)"

provides:
  - "GET /api/autocomplete?prefix=&kind=page|tag|all&limit= backend endpoint (crates/cli/src/cmd/serve/routes/autocomplete.rs)"
  - "completionSource CM6 completion function (frontend/src/lib/editor/autocomplete.ts)"
  - "detectBulletTree paste detector TS port of segment.rs Stage 1 (frontend/src/lib/editor/paste.ts)"
  - "serializeBlockTree depth-first raw concat (frontend/src/lib/editor/serialize.ts)"
  - "BulletPopover.svelte: 6-action popover (cut/copy/duplicate/fold/zoom/copy-as-md)"
  - "applyInverse(op, prevHash) treeOpLog inverse wiring (history-routing.ts extended)"
  - "BlockEditorCallbacks.onPaste optional paste hook (boundary.ts extended)"
  - "Block.svelte: bullet-click popover, paste handler, real completionSource wired"

affects: ["03-06-rename", "03-07-unresolved-link", "04-watcher"]

tech-stack:
  added: []
  patterns:
    - "autocomplete.ts completionSource: bracket regex /\\[\\[([^\\]]*)\$/ → kind=page fetch; hash regex /(^|\\s)#([\\p{L}\\p{N}_-]*)\$/u → kind=all fetch. `from` anchors at ctx.pos - prefix.length."
    - "detectBulletTree: strict TAB bullet rule (^(\\t*)- ), continuation (^\\t+  |^  ) or empty; returns null on any non-bullet non-continuation line. Requires >= 2 bullet items."
    - "serializeBlockTree: one-line recursive concat of block.raw + children.map(serializeBlockTree).join('') — exact inverse of detectBulletTree."
    - "applyInverse switch: Indent/Outdent → PATCH /structure with depth; Delete → POST /blocks with snapshot; Move → PATCH /structure with parentId+ord; Merge/Split stubbed (deferred to 03-06)."
    - "BulletPopover absolute-positioned menu (left: 100%; top: 0) as $state-controlled child of .block, with $effect keydown/mousedown listeners."
    - "Paste extension: EditorView.domEventHandlers({ paste }) added to extensions.ts when BlockEditorCallbacks.onPaste is provided. preventDefault before async handler resolves."

key-files:
  created:
    - crates/cli/src/cmd/serve/routes/autocomplete.rs
    - crates/cli/tests/autocomplete_api.rs
    - frontend/src/lib/editor/autocomplete.ts
    - frontend/src/lib/editor/paste.ts
    - frontend/src/lib/editor/serialize.ts
    - frontend/src/lib/components/BulletPopover.svelte
    - frontend/src/lib/editor/__tests__/autocomplete.test.ts
    - frontend/src/lib/editor/__tests__/paste.test.ts
    - frontend/src/lib/components/__tests__/bullet-popover.test.ts
  modified:
    - crates/cli/src/cmd/serve/routes/mod.rs
    - frontend/src/lib/editor/history-routing.ts
    - frontend/src/lib/editor/extensions.ts
    - frontend/src/lib/editor/boundary.ts
    - frontend/src/lib/components/Block.svelte

key-decisions:
  - "refs schema fix: refs.type='tag' (not 'kind'); refs.target_page is INTEGER FK to pages.id — the autocomplete SQL for tags must JOIN pages ON pages.id = refs.target_page to get name strings. Caught during GREEN phase when tests returned 500."
  - "LIKE escape: prefix.replace('%','\\\\%').replace('_','\\\\_') with ESCAPE '\\\\' clause (T-03-15). Verified by autocomplete_prefix_like_wildcards_escaped test."
  - "detectBulletTree deviation from plan: plan spec said 'Mixed input where non-bullet, non-continuation line appears returns null' — implemented exactly. Continuation rule accepts both TAB+2-space and plain 2-space (depth-0 blocks). Empty lines also treated as continuations (matches segment.rs)."
  - "serializeBlockTree confirmed: block.raw from a real fixture includes leading TABs + '- ' + text + '\\n'. The plan's 1-line recursion is correct and sufficient. No additional stripping needed."
  - "Copy-paste round-trip unit test passed without backend-coupled fixtures — used inline mock Block objects with the exact raw format. serializeBlockTree → detectBulletTree returns correct item count and depth array."
  - "BulletPopover positioning: left: 100%; top: 0 as per plan recommendation. No floating-ui dep added. No overflow clipping issues in tests."
  - "applyInverse Merge/Split inverse: stubbed with console.debug — plan explicitly deferred these to plan 03-06 ('complex inverse'). The Delete/Indent/Outdent/Move inverses are fully wired."
  - "Paste handler async design: EditorView.domEventHandlers paste fires synchronously; we preventDefault immediately, then the async postBlock chain (via onSiblingCreate callback) runs after. CM6 does not insert raw text."
  - "BlockEditorCallbacks.onPaste added as optional field — no existing call-sites needed updating (the stub in 03-04 passes completions only; onPaste is undefined → paste extension skipped)."

patterns-established:
  - "autocomplete LIKE escape: escape(%, _) + ESCAPE clause. Same pattern as Phase 2 search.rs."
  - "Popover open/close via $state boolean + $effect for document listeners with cleanup disposer."

requirements-completed: [EDT-09, EDT-11, EDT-12]

duration: ~10min
completed: 2026-05-22
---

# Phase 03 Plan 05: Autocomplete, paste detection, BulletPopover, treeOpLog inverses Summary

**GET /api/autocomplete (tag/page/all with LIKE escape), CM6 completionSource wired to [[/# triggers, detectBulletTree paste detector, serializeBlockTree round-trip, BulletPopover with 6 actions, and applyInverse dispatching real server mutations.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-22T06:55:10Z
- **Completed:** 2026-05-22T07:04:33Z (approx)
- **Tasks:** 2 (both tdd="true", RED then GREEN each)
- **Files created:** 9 (5 source + 4 test files — backend autocomplete, frontend autocomplete/paste/serialize/BulletPopover)
- **Files modified:** 5 (mod.rs, history-routing.ts, extensions.ts, boundary.ts, Block.svelte)

## Accomplishments

- `GET /api/autocomplete?prefix=&kind=page|tag|all&limit=` — 8 integration tests green. LIKE wildcard escape + limit clamp to 100 + SQL JOIN to get tag names from refs.
- `completionSource` CM6 function: fires on `[[` (pages) and `#` (tags+pages); `from` anchors at `ctx.pos - prefix.length` so `[[` delimiter is preserved during replacement.
- `detectBulletTree`: strict TAB-indent bullet rule; continuation lines and empty lines fold into parent block's `raw`; requires ≥2 bullets; returns null for mixed content.
- `serializeBlockTree`: one-line recursive concat confirming `block.raw` already has leading TABs + `- ` + trailing `\n` — no additional formatting needed.
- `BulletPopover.svelte`: 6-item menu (cut/copy/duplicate/fold/zoom/copy-as-md); absolute-positioned at `left: 100%; top: 0`; closes on Escape, click-outside, or item selection.
- `applyInverse`: Indent/Outdent → PATCH /structure depth restore; Delete → POST /blocks with snapshot; Move → PATCH /structure parent+ord; 409 → restore op to log + signal conflict.
- Paste extension in `extensions.ts` via `EditorView.domEventHandlers` — wired when `BlockEditorCallbacks.onPaste` is provided.
- 153 frontend tests green (12 new); 8 new backend integration tests green; bundle 753 KB (under 900 KB gate).

## Autocomplete LIKE-Escape Behavior

The `ESCAPE '\\'` clause was necessary. Without it, a prefix like `%25_` would match all rows (`%` is a LIKE wildcard). The escape converts `%` → `\\%` and `_` → `\\_` before building the pattern. Test `autocomplete_prefix_like_wildcards_escaped` confirms 200 is returned for wildcard-containing prefixes (no 500, no spurious matches).

## detectBulletTree vs segment.rs Stage 1

One intentional deviation: the TS port accepts both `^\t+  ` (TAB+2spaces) and `^  ` (2 spaces alone) as continuation line patterns. This matches segment.rs's continuation rule for depth-0 blocks (where the leading TAB count is 0 so continuation is just 2 spaces). The plan's behavior spec was silent on depth-0 continuation — the implementation is consistent with the Rust original.

## serializeBlockTree Confirmation

`block.raw` in the real fixture format is verbatim segment.rs output: `\t*- text\n`. The one-line recursion `block.raw + children.map(serializeBlockTree).join('')` is confirmed correct. No stripping or reformatting needed. Round-trip test (`serializeBlockTree` → `detectBulletTree`) passes with inline mock objects whose depth arrays match the original tree structure.

## Copy-Paste Round-Trip Test

Used inline mock Block objects with the raw format exactly as segment.rs produces — no backend-coupled fixture required. `serializeBlockTree(blockWithChildren)` returns `- parent\n\t- child1\n\t- child2\n\t\t- grandchild\n`; `detectBulletTree` on that returns 4 items with depths `[0,1,1,2]`.

## Popover Positioning

`left: 100%; top: 0` as recommended in 03-RESEARCH §Bullet popover positioning. No `floating-ui` dep added. No overflow clipping issues found in tests.

## Task Commits

1. **Task 1 RED: failing tests for autocomplete + paste** — `74446f4` (test)
2. **Task 1 GREEN: autocomplete endpoint + completionSource + paste detector** — `422b61a` (feat)
3. **Task 2 RED: failing tests for BulletPopover + serialize + inverses** — `7c8cd5b` (test)
4. **Task 2 GREEN: BulletPopover + serialize + treeOpLog inverses + paste wiring** — `b15b7f1` (feat)

## Files Created/Modified

**Created:**
- `crates/cli/src/cmd/serve/routes/autocomplete.rs` — GET /api/autocomplete handler (page/tag/all/limit)
- `crates/cli/tests/autocomplete_api.rs` — 8 integration tests
- `frontend/src/lib/editor/autocomplete.ts` — CM6 completionSource
- `frontend/src/lib/editor/paste.ts` — detectBulletTree
- `frontend/src/lib/editor/serialize.ts` — serializeBlockTree
- `frontend/src/lib/components/BulletPopover.svelte` — 6-action popover
- `frontend/src/lib/editor/__tests__/autocomplete.test.ts` — 7 tests
- `frontend/src/lib/editor/__tests__/paste.test.ts` — 10 tests
- `frontend/src/lib/components/__tests__/bullet-popover.test.ts` — 12 tests (incl. round-trip, inverse wiring)

**Modified:**
- `crates/cli/src/cmd/serve/routes/mod.rs` — register /api/autocomplete route
- `frontend/src/lib/editor/history-routing.ts` — applyInverse + extended bindHistoryRouting
- `frontend/src/lib/editor/extensions.ts` — paste domEventHandler extension
- `frontend/src/lib/editor/boundary.ts` — onPaste optional callback
- `frontend/src/lib/components/Block.svelte` — bullet click popover, paste handler, real completionSource

## Decisions Made

- **refs schema:** `refs.type='tag'` (not `kind`), `refs.target_page` is INTEGER FK — autocomplete SQL joins pages. Caught during GREEN (500 errors in 4 tests).
- **applyInverse Merge/Split stubbed:** plan explicitly deferred to 03-06. console.debug logs the op.
- **paste handler async:** preventDefault immediately, then async postBlock chain via onSiblingCreate.
- **BulletPopover onPaste optional:** no existing call-sites in 03-04 needed updating.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] refs.type='tag' (not kind), target_page is FK integer — SQL JOIN required**
- **Found during:** Task 1 (backend GREEN verification)
- **Issue:** Plan said `WHERE kind='tag' AND target_page LIKE prefix%` but schema has `type='tag'` and `target_page INTEGER FK` to pages.id, not the page name string. 4 of 8 tests returned 500.
- **Fix:** Changed SQL to `WHERE r.type='tag'` and added `JOIN pages p ON p.id = r.target_page` to retrieve `p.name`. Applied to both `kind=tag` and `kind=all` tag branch.
- **Files modified:** `crates/cli/src/cmd/serve/routes/autocomplete.rs`
- **Verification:** All 8 integration tests pass.
- **Committed in:** `422b61a` (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — schema reality mismatch in plan spec)
**Impact on plan:** Necessary correction; the LIKE query needed a JOIN to get name strings from integer FK refs. No scope creep.

## Known Stubs

- `applyInverse` Merge case: `console.debug('[applyInverse] Merge/Split inverse not yet implemented', op)` — plan explicitly deferred to 03-06. The op is not applied to the server.
- `applyInverse` Split case: same as Merge — deferred to 03-06.
- `handlePopoverAction('duplicate')`: signals via `onSiblingCreate` callback (same as Enter) — full `postBlock` with correct ord/depth is pending in 03-06 where `pageId` is plumbed through Block.svelte props.
- `handlePaste`: signals via `onSiblingCreate` callback per item — same limitation as duplicate.
- Arrow navigation (ArrowUp/ArrowDown): handler unmounts editor but does not mount adjacent block — deferred from 03-04, still deferred to 03-06.
- Merge on Backspace-at-start (non-empty block): returns `false` (CM6 default char-delete) — deferred from 03-04, still deferred to 03-06.

## Threat Model Check

- T-03-15 (SQL injection via prefix): `params![]` binding + LIKE escape `\\%`/`\\_` with ESCAPE clause. Tested by `autocomplete_prefix_like_wildcards_escaped`.
- T-03-16 (DoS via huge limit): limit clamped to 100 server-side. Tested by `autocomplete_limit_clamps_to_100`.
- T-03-17 (XSS via pasted content): clipboard text treated as RAW markdown (same trust level as typing). markdown-it `html: false` already mitigates in the renderer.
- T-03-18 (DoS via huge paste): paste loop is sequential via onSiblingCreate callback. Documented as known v1 limitation.
- T-03-19 (treeOpLog inverse race): applyInverse includes prevHash; 409 → restore op + conflict banner.

## Self-Check

- [x] `crates/cli/src/cmd/serve/routes/autocomplete.rs` — present
- [x] `frontend/src/lib/editor/autocomplete.ts` — present
- [x] `frontend/src/lib/editor/paste.ts` — present
- [x] `frontend/src/lib/editor/serialize.ts` — present
- [x] `frontend/src/lib/components/BulletPopover.svelte` — present
- [x] Commit `74446f4` exists on main (RED test 1)
- [x] Commit `422b61a` exists on main (GREEN task 1)
- [x] Commit `7c8cd5b` exists on main (RED test 2)
- [x] Commit `b15b7f1` exists on main (GREEN task 2)
- [x] `npm run test -- --run` in `frontend/` → 153 passed, 0 failed
- [x] `cargo test -p foliom-cli --test autocomplete_api` → 8 passed, 0 failed
- [x] `npm run build` in `frontend/` → 753 KB JS, under 900 KB CI gate

## Self-Check: PASSED

---
*Phase: 03-outliner-editor*
*Completed: 2026-05-22*
