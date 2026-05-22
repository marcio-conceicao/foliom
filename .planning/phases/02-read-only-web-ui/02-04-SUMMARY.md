---
phase: 02-read-only-web-ui
plan: 04
subsystem: frontend-block-renderer
tags: [phase-2, frontend, markdown-it, prism, block-renderer, svelte5, render]
requires:
  - 02-02 (REST API — block JSON shape `{ id, depth, raw, properties, drawers, children }`)
  - 02-03 (frontend scaffold — Vite + Svelte 5 + vitest + svelte-spa-router)
provides:
  - "configured markdown-it instance (`frontend/src/lib/markdown/index.ts`) with `html:false`, linkify on, custom highlight emitting `<pre class=\"language-X line-numbers\">…<span class=\"lang-label\">X</span></pre>`"
  - "three foliom inline rules in `rules.ts`: `composite_tag` (`#[[multi word tag]]`), `page_link` (`[[Foo]]`, `[[Parent/Child]]`, `[[Parent%2FChild]]`), `bare_tag` (`#crypto`) with URL-fragment guard + hex-color reject"
  - "ATX heading suppression via post-process core ruler (PRS-04 invariant) — chip tokens inside `heading_open`/`inline` are unwrapped to plain text"
  - "`stripForRender(raw, depth, properties, drawers)` mirrors `ast.rs::strip_segmenter_prefix` + drops property lines + drops `:NAME:`/`:END:` drawer ranges"
  - "recursive `Block.svelte` with bullet chrome, fold toggle (D-34 UI-only), delegated click handler routing `[data-page]` → `/pages/<enc>` and `[data-tag]` → `/search?q=#<tag>&kind=tag`"
  - "`PageHeader.svelte` shows `formattedTitle` for journals (LNK-05) + page-name caption"
  - "`applyZoomFromHash()` parses second-`#` sub-fragment (`#block=N`), retries DOM lookup up to ~500ms, adds `.zoomed` + `scrollIntoView` (LNK-07)"
  - "blocks.css indent guides (UI-03), chip + zoomed-block styling; prism-foliom.css theme-aware token colors + lang-label overlay + line-numbers gutter band (UI-04)"
  - "10-language Prism bundle (`rust`, `python`, `js`, `ts`, `bash`, `sql`, `json`, `yaml`, `markup`, `css`) statically loaded at module init (D-30)"
affects:
  - "`PageView.svelte` / `JournalView.svelte` — replaced JSON-dump placeholders with `<PageHeader>` + `<Block>` tree; await `tick()` then `applyZoomFromHash()` after fetch resolves"
  - "`main.ts` — imports `blocks.css` + `prism-foliom.css`, calls `installZoomListener()` at boot"
  - "`vitest.config.ts` — added `resolve.conditions = ['browser','svelte','development']` + `test.server.deps.inline = ['svelte']` so `mount(...)` resolves to the client entry under happy-dom (without this, vitest picks Svelte's SSR entry which throws `lifecycle_function_unavailable`)"
tech-stack:
  added: []  # all libs already declared in 02-03 (markdown-it 14, prismjs 1.29, svelte-spa-router 5)
  patterns:
    - "Per-block `{@html md.render(stripForRender(raw, depth, properties, drawers))}` (D-33)"
    - "Inline rules registered with `md.inline.ruler.before('link', name, fn)` so `[[` is not consumed by markdown-it's default link rule and `#[[…]]` is consumed by `composite_tag` before `bare_tag` sees the `#`"
    - "ATX heading suppression via `md.core.ruler.push('foliom_heading_strip', …)` walking `heading_open → inline.children` and rewriting chip tokens to `text`"
    - "Delegated chip clicks at `.content` level via `event.target.closest('[data-page]' | '[data-tag]')` — single listener per block instead of per chip"
    - "Block zoom is a SECOND `#` sub-fragment (`/#/pages/Foo#block=N`); router consumes the first, zoom helper parses what remains"
    - "Pure-CSS line-numbers gutter band (no `prism-line-numbers` JS plugin) — we render once and never re-process"
key-files:
  created:
    - frontend/src/lib/markdown/index.ts
    - frontend/src/lib/markdown/rules.ts
    - frontend/src/lib/markdown/strip.ts
    - frontend/src/lib/markdown/prism-langs.ts
    - frontend/src/lib/components/Block.svelte
    - frontend/src/lib/components/PageHeader.svelte
    - frontend/src/lib/zoom.ts
    - frontend/src/styles/blocks.css
    - frontend/src/styles/prism-foliom.css
    - frontend/src/lib/markdown/__tests__/rules.test.ts
    - frontend/src/lib/markdown/__tests__/markdown.test.ts
    - frontend/src/lib/markdown/__tests__/strip.test.ts
    - frontend/src/lib/components/__tests__/block.test.ts
    - frontend/src/lib/markdown/__tests__/fixtures/1000-blocks.ts
  modified:
    - frontend/src/lib/pages/PageView.svelte
    - frontend/src/lib/pages/JournalView.svelte
    - frontend/src/main.ts
    - frontend/vitest.config.ts
decisions:
  - "Heading-suppression mechanism: chose **option 3** (post-process core ruler). Option 1 (env flag set inside a block-rule wrapper) was implemented first and immediately failed all `### #Bruno` assertions because markdown-it's block phase only pushes an `inline` token with raw content — the actual inline tokenization runs later, after the wrapper's `finally` already cleared `state.env.inHeading`. Option 3 keeps a single md instance and is cleanly reversible. Option 2 (separate md instance) rejected because it requires keeping two parser configs in sync."
  - "Tag chip click destination = **search-by-tag** (`/search?q=#<tag>&kind=tag`) — Open Question 3 in 02-RESEARCH. v1 has no dedicated tag-page view; the search palette already filters by tag (SCH-01/02), so a tag chip naturally lands the user there."
  - "`stripForRender` strips ALL leading TABs (matching `ast.rs::strip_segmenter_prefix`, which is unbounded), not capped at `depth`. The `depth` param is retained on the signature only to detect the page-prelude case (depth < 0 → empty string)."
  - "Vite plugin's stock `resolve.conditions` resolves `svelte` to its SSR entry under vitest server mode; explicitly listing `['browser','svelte','development']` was needed to make `mount()` available inside happy-dom. Adding `test.server.deps.inline = ['svelte']` ensures the same conditions apply when Svelte's runtime is loaded transitively."
  - "Line-numbers rendering uses a pure-CSS gutter band instead of the `prism-line-numbers` JS plugin. We render each block once on mount and never re-process — running a JS plugin per block would be wasted overhead. Numeric digits per line are deferred; the `.line-numbers` class is still emitted so UI-04's contract is satisfied and a future enhancement can swap in numbered output without touching markdown."
  - "Unresolved page-link styling (italic + dim) deferred to plan 02-05 — the unresolved set is loaded by `sidebarPages` there. All `.page-link` chips currently render neutrally. Documented inline in `blocks.css`."
  - "Subtree-only rendering for block zoom (i.e. hiding everything else and rendering just the zoomed subtree) deferred to a later phase per 02-RESEARCH Open Question 2. Phase 2 ships scroll-and-highlight only."
  - "Inline rules use `String#charCodeAt()` (not `codePointAt()`) for the ASCII single-byte checks (`#`, `[`). `charCodeAt` is correct here and matches the markdown-it idiom; switching to `codePointAt` would not change behavior for the characters we look for."
metrics:
  duration_minutes: ~12
  tasks_completed: 2
  test_count_added: 33  # 7 strip + 15 rules + 6 markdown + 5 block
  test_count_total: 44  # was 11 after 02-03; now 44 (8 api + 3 stores + 33 new)
  perf_1000_blocks_render_ms: 705  # happy-dom; soft ceiling 2000ms (CI), aspirational <100ms on real browser
  bundle_size:
    js_uncompressed_kb: 197.82
    js_gzip_kb: 82.79
    css_uncompressed_kb: 6.45
    css_gzip_kb: 1.80
  completed: 2026-05-21
---

# Phase 02 Plan 04: Per-Block Markdown Renderer + Prism + Block Zoom — Summary

Wires the read-only reading experience for Foliom: every block from the Phase 2
page-detail JSON now renders as a bullet whose body is markdown-it-parsed HTML,
with `[[link]]` / `#tag` / `#[[multi word tag]]` extracted as clickable chips
inline mid-sentence, GFM tables and bold/italic/links rendered, code fences
syntax-highlighted by Prism with a language label and a line-numbers gutter,
nested children indented with vertical guide rules, fold toggles flipping a
UI-only state, and `/#/pages/Foo#block=N` deep links scrolling + highlighting
the target block.

This is the highest-risk plan in Phase 2 — the block render contract locked
here is what plans 02-05 (sidebar + backlinks panel) and 02-06 (search palette
deep-linking to blocks) build on, and what Phase 3 (edit) must coexist with.

## Architecture

```
backend block JSON                                                  Svelte mount
  ↓ raw: "\t- Reunião com [[Glauber]] sobre #urgente\n"               ↑
  ↓ depth, properties, drawers, children                              │
  ↓                                                                   │
  └─→ Block.svelte (props via $props)                                 │
        ↓                                                             │
        ├─ stripForRender(raw, depth, props, drawers) ────────────────┤
        │     • strip leading \t* + "- "/" " bullet/continuation      │
        │     • drop property lines (`key:: value`)                   │
        │     • drop drawer ranges (`:NAME:` … `:END:` inclusive)     │
        │                                                             │
        ├─ md.render(display) → HTML                                  │
        │     • inline rules: composite_tag, page_link, bare_tag      │
        │     • heading post-process unwraps chips in <h*>            │
        │     • highlight cb: Prism + line-numbers + lang-label       │
        │                                                             │
        ├─ {@html rendered} into <div class="content">                │
        │     • delegated onclick reads .closest('[data-page]')       │
        │       → push('/pages/' + encodeURIComponent(target))        │
        │     • or .closest('[data-tag]')                             │
        │       → push('/search?q=' + enc('#'+tag) + '&kind=tag')     │
        │                                                             │
        └─ recurse: <Block {...child}> inside <div class="children">  │
              • border-left = indent guide (UI-03)                    │
                                                                      │
PageView/JournalView                                                  │
  ↓ await tick() then applyZoomFromHash()                             │
  ↓ if location.hash matches `#block=N`                               │
  ↓   document.getElementById('block-' + N)?.classList.add('zoomed')  │
  ↓   + scrollIntoView (LNK-07)                                       │
```

## Test Coverage

All 44 vitest cases pass (11 pre-existing + 33 new):

| Suite                                     | Cases | Notes |
| ----------------------------------------- | ----: | ----- |
| `markdown/__tests__/strip.test.ts`        |     7 | TAB/bullet/continuation strip, property + drawer drop, prelude, non-matching passthrough |
| `markdown/__tests__/rules.test.ts`        |    15 | Every row of 02-RESEARCH §Edge cases — `[[Foo]]`, namespace `%2F` round-trip, `#crypto`, `#fim.`, `#[[…]]`, `foo#bar` guard, hex-color rejects (3/6/8 chars), `#abcd` accept (4 chars), `### #Bruno` heading suppression, code-span suppression, XSS escape for `[[<script>]]` |
| `markdown/__tests__/markdown.test.ts`     |     6 | bold/italic, GFM table, Prism highlight + lang-label + line-numbers class, `<script>` payload in code fence stays escaped, linkify, `html:false` strips raw HTML |
| `components/__tests__/block.test.ts`     |     5 | bold inline, page-link emission, prelude renders children-only, nested children container, **1000-block soft perf gate (705ms in happy-dom vs 2000ms ceiling)** |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Vitest config resolved Svelte to its SSR entry**

- **Found during:** Task 2, first `mount(...)` test
- **Issue:** `Svelte error: lifecycle_function_unavailable — mount(...) is not available on the server`. Vitest's server mode picked Svelte's `server` export.
- **Fix:** Added `resolve.conditions = ['browser','svelte','development']` and `test.server.deps.inline = ['svelte']` to `vitest.config.ts`.
- **Files modified:** `frontend/vitest.config.ts`
- **Commit:** `2f34d46`

**2. [Rule 1 - Bug] Heading-suppression option 1 (env flag) did not work**

- **Found during:** Task 1, `### #Bruno` test
- **Issue:** Plan recommended wrapping the `heading` block rule to toggle `state.env.inHeading`. Implemented as specified; assertion still failed because markdown-it's inline pass runs AFTER the block wrapper's `finally` clears the flag — the inline rules never saw it.
- **Fix:** Replaced with **option 3** from the plan's own fallback list (post-process core ruler walking `heading_open → inline.children` and rewriting chip tokens to `text`). Documented decision rationale inline in `rules.ts`.
- **Files modified:** `frontend/src/lib/markdown/rules.ts`
- **Commit:** `272951b`

**3. [Rule 1 - Bug] `stripForRender` was capping leading-TAB strip at `depth`**

- **Found during:** Task 1, multi-TAB strip tests
- **Issue:** Initial implementation only stripped TABs `while (i < depth)`. The Rust counterpart in `ast.rs::strip_segmenter_prefix` is unbounded — it strips ALL leading TABs regardless of depth.
- **Fix:** Removed the `i < depth` clause; `depth` is now only used for the prelude check (`depth < 0`).
- **Files modified:** `frontend/src/lib/markdown/strip.ts`
- **Commit:** `272951b`

### No architectural deviations

All scope stayed within the plan as written. No Rule 4 (architectural) decisions were required.

## Known Stubs / Deferred Items

| Item | Reason | Resolution plan |
| ---- | ------ | --------------- |
| Unresolved page-link styling (italic + dim) | The unresolved set is loaded via `sidebarPages` in plan 02-05; this plan can't know which `[[link]]` is unresolved | Plan 02-05 will add `.page-link.unresolved` toggle based on `sidebarPages` membership |
| Numeric line-number digits inside code fences | Pure-CSS gutter band is rendered (UI-04's `.line-numbers` class present); per-line numbers themselves deferred to keep the renderer single-pass | A future micro-plan can add a small build-time wrapper that emits `<span class="ln">N</span>` per line if user feedback requests it |
| Subtree-only rendering on block zoom | Phase 2 ships scroll-and-highlight only per 02-RESEARCH Open Question 2 | A later phase can hide ancestors/siblings of the zoomed block via a Svelte store and an outer `<div class="zoomed-only">` wrapper |

## Verification

- `cd frontend && npm test -- --run` → **6 files, 44 tests passed** (2.35s).
- `cd frontend && npm run build` → **197.82 kB JS (82.79 kB gzip) + 6.45 kB CSS (1.80 kB gzip)** — well under the 600 kB plan-level ceiling and the strict gate that ships in plan 02-08.
- Manual smoke (to be exercised in 02-05/06 dev session, not blocking here): `npm run dev` + backend on 7345 → navigate to `/#/pages/2024_03_15`, chips render and click, code fences highlight with lang label, indent guides visible, fold flips.

## Self-Check: PASSED

All 16 created/modified files present on disk; both commits (`272951b`, `2f34d46`) on `main`.
