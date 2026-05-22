---
phase: 02-read-only-web-ui
plan: 06
subsystem: ui
tags: [phase-2, frontend, svelte, search, palette, fts5, keyboard, xss]

requires:
  - phase: 02-02
    provides: "/api/search FTS5 endpoint with sanitized query + <mark> snippet"
  - phase: 02-03
    provides: "searchPalette store + /search route placeholder + svelte-spa-router config"
  - phase: 02-04
    provides: "lib/zoom.ts hashchange listener that consumes #block=N"
  - phase: 02-05
    provides: "Sidebar footer slot + App.svelte 02-06 marker"
provides:
  - "Global Ctrl/Cmd+K keymap (lib/keys.ts) with disposer"
  - "SearchPalette modal + inline body (single component, mode prop)"
  - "Snippet sanitizer (lib/sanitize.ts) — allow-list of <mark> only"
  - "Sidebar footer Buscar trigger button"
  - "SearchView inline mode for #/search?q= deep links"
affects: [02-07, 02-08]

tech-stack:
  added: []
  patterns:
    - "Hand-rolled HTML allow-list sanitizer (escape-all + reintroduce-tokens) for narrow surfaces — avoids DOMPurify dep"
    - "Single component / two render modes via `mode: 'modal' | 'inline'` prop"
    - "Module-level memoized fetch promise for slow-changing endpoints (page-titles)"
    - "AbortController per debounced run to cancel stale fetches (race-safe results)"
    - "rAF-after-push hash rewrite to layer foliom sub-fragments on top of svelte-spa-router hash routes"

key-files:
  created:
    - frontend/src/lib/keys.ts
    - frontend/src/lib/sanitize.ts
    - frontend/src/lib/components/SearchPalette.svelte
    - frontend/src/lib/components/SearchResult.svelte
    - frontend/src/lib/components/__tests__/keys.test.ts
    - frontend/src/lib/components/__tests__/palette.test.ts
    - frontend/src/styles/palette.css
  modified:
    - frontend/src/App.svelte
    - frontend/src/main.ts
    - frontend/src/lib/components/Sidebar.svelte
    - frontend/src/lib/pages/SearchView.svelte

key-decisions:
  - "Snippet sanitization: hand-rolled allow-list (escapeHtml then reintroduce literal <mark>/</mark>). Rationale: T-02-20 surface is two tokens; DOMPurify (~20 KB gz) is overkill and the bundle budget matters for cold-start."
  - "Empty-state copy: \"Sem resultados para '<query>'.\" (resolves 02-RESEARCH Open Question 3, in Portuguese to match the rest of the UI)."
  - "AbortController-based cancellation IS in. Each debounced run aborts the prior in-flight fetch so a slow /api/search response can never overwrite results from a newer keystroke (T-02-22)."
  - "Debounce locked at 150ms per plan; no in-flight evidence to retune yet."
  - "[[ branch caches page-titles in a module-level Promise so subsequent [[ queries don't re-hit the network. First [[ keystroke triggers the fetch; later filters are pure client-side."
  - "Single SearchPalette.svelte with `mode='modal'|'inline'` prop instead of extracting a SearchPanel.svelte — avoids prop-drilling refs/handlers across two layers and keeps the test surface small."
  - "Non-numeric blockId (T-02-21) coerced via Number(); when NaN OR <= 0 we omit the #block= fragment entirely. blockId=0 is a sentinel used by the [[ branch (means \"page top\") so the same coercion guards it cleanly."
  - "Esc keybinding split between global and palette: lib/keys.ts owns global Esc but defers to native input clearing when focus is inside an editable. Inside the palette modal, the input's own onkeydown handler reasserts Esc-to-close so the user doesn't get stuck."
  - "Cmd+K modifier OVERRIDES the input gate by design — the palette must always be summonable regardless of focus context."

patterns-established:
  - "lib/keys.ts: single window keydown listener for app-level shortcuts; returns disposer; gated by `isEditableTarget()`"
  - "lib/sanitize.ts: tiny escape-then-reintroduce allow-list — the pattern to reuse when other backend HTML payloads need defense-in-depth"

requirements-completed: [SCH-01, SCH-02, SCH-03, LNK-07]

duration: 14min
completed: 2026-05-22
---

# Phase 02 Plan 06: Search Palette (Ctrl+K) Summary

**Global Ctrl/Cmd+K search palette consuming /api/search FTS5 with kind routing (#tag, [[page, content), allow-list-sanitized <mark> snippets, keyboard nav, and click-to-block deep linking via #/pages/<page>#block=<blockId>.**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-22T00:50:00Z
- **Completed:** 2026-05-22T00:56:00Z
- **Tasks:** 2 (each TDD: RED + GREEN)
- **Files modified:** 11 (7 created, 4 modified)
- **Frontend bundle:** 212.57 kB JS (87.74 kB gzip) + 12.60 kB CSS (2.98 kB gzip) — +5.3 kB JS / +1.9 kB CSS vs 02-05 baseline
- **Tests:** 73/73 green (17 new: 8 keys + 9 palette)

## Accomplishments

- SCH-03 shipped: Ctrl+K (any OS) and Cmd+K (macOS) toggle the palette regardless of focus; Esc closes; input-target gating handled correctly (modifier overrides, plain Esc defers to native input clear).
- SCH-01 shipped: palette consumes `/api/search?q=&kind=&limit=` with branch-aware routing — `#prefix` → `kind=tag`, `[[prefix` → cached `/api/page-titles` client-filter, otherwise → `kind=content`.
- SCH-02 shipped: snippets render with `<mark>` highlights preserved via a 5-line escape-then-reintroduce allow-list sanitizer (T-02-20). `<script>`/`<img>`/all other tags pass through escaped as text.
- LNK-07 contract exercised end-to-end: Enter on a result navigates to `#/pages/<page>#block=<blockId>` and the existing zoom listener (from plan 02-04) scrolls + highlights the target block.
- `#/search?q=…` deep links land in `SearchView.svelte` rendering the SAME palette body in `mode='inline'` (no backdrop, no centering) — single component, two render modes.
- Sidebar footer gained a "Buscar" trigger button that opens the palette via the store, giving the keyboard-averse a click-path entry too.

## Task Commits

Each task was committed atomically (TDD: RED then GREEN):

1. **Task 1 RED: failing test for global Ctrl/Cmd+K keymap** — `dfe8a95` (test)
2. **Task 1 GREEN: lib/keys.ts + App.svelte wiring + SearchPalette stub** — `0d6a1ed` (feat)
3. **Task 2 RED: failing palette tests (debounce, kind routing, sanitize, kbd nav, deep-link)** — `c9d2dc2` (test)
4. **Task 2 GREEN: SearchPalette body + SearchResult + sanitize + Sidebar trigger + styles** — `d60e4be` (feat)

**Plan metadata commit:** (this commit — includes SUMMARY + STATE + ROADMAP updates).

## Files Created/Modified

- `frontend/src/lib/keys.ts` — `bindGlobalShortcuts()` window keydown registrar with disposer; Cmd/Ctrl+K toggle, Esc close (input-aware).
- `frontend/src/lib/sanitize.ts` — `sanitizeSnippet()` allow-list: escape everything then reintroduce literal `<mark>`/`</mark>` only.
- `frontend/src/lib/components/SearchPalette.svelte` — modal + inline palette body. 150ms debounce, AbortController cancellation, three-branch query routing, keyboard navigation, click-to-block.
- `frontend/src/lib/components/SearchResult.svelte` — single result row consuming `sanitizeSnippet`; `role="option"` + `aria-selected`.
- `frontend/src/styles/palette.css` — themed modal chrome (backdrop+blur, centered panel) + result list (hover/active + `<mark>` highlight color).
- `frontend/src/lib/pages/SearchView.svelte` — now mounts `SearchPalette mode="inline"` and pre-populates the store query from the hash `?q=`.
- `frontend/src/App.svelte` — wires `bindGlobalShortcuts()` via `$effect` and renders `SearchPalette` conditionally on `searchPalette.open`.
- `frontend/src/lib/components/Sidebar.svelte` — footer "Buscar (Ctrl+K)" trigger button that calls `searchPalette.set({ open: true, query: '' })`.
- `frontend/src/main.ts` — imports `palette.css`.
- `frontend/src/lib/components/__tests__/keys.test.ts` — 8 cases covering modifier toggle, input gating, Esc behavior, disposer cleanup.
- `frontend/src/lib/components/__tests__/palette.test.ts` — 9 cases: debounce window, `kind=tag` prefix, `[[` page-titles branch, `<mark>` allow-list (T-02-20 XSS payload), Enter navigation, ArrowDown+Enter, empty result copy, whitespace-only no-op, NaN blockId coercion (T-02-21).

## Decisions Made

See `key-decisions` in frontmatter. Highlights:

- **Sanitizer is hand-rolled** (escape-then-reintroduce). The DOMPurify alternative would cost ~20 KB gzipped for a two-token allow-list — fails the cold-start budget for no real safety win since the snippet contract is fixed at the backend.
- **AbortController IS wired in** (plan asked us to decide). Each debounced run aborts the prior in-flight `/api/search`, so a slow response can't clobber results from a newer keystroke. AbortError on the awaited fetch is silently swallowed.
- **Empty state in Portuguese**: `Sem resultados para '<query>'.` Resolves Open Question 3 in 02-RESEARCH.
- **Single SearchPalette component, two modes** rather than the plan's tentative "extract SearchPanel.svelte" — simpler diff, fewer files, single test surface.

## Deviations from Plan

None - plan executed exactly as written. Some minor implementation refinements (single-component two-mode rendering instead of extracting SearchPanel; AbortController added as the plan's open question explicitly invited) are documented in Decisions.

## Issues Encountered

- A pre-existing `svelte-check` warning on `routes.ts` (Component params type mismatch for hash-routed pages) is NOT introduced by this plan and is OUT OF SCOPE for 02-06. Logged for future cleanup (likely 02-07 or 02-08 hygiene pass).
- One `npm run build` invocation during verification accidentally touched `frontend/dist/.gitkeep`; reverted before commit.

## Threat Model Compliance

All three mitigations in the plan's threat register are implemented and covered by tests:

- **T-02-20 (XSS in snippet)** — `sanitizeSnippet` test passes a `<script>alert(1)</script>` payload through the rendered DOM and asserts no `<script>` element appears and the literal text is escaped.
- **T-02-21 (Tampering with #block=N fragment)** — `Number(hit.blockId)` coercion; non-numeric or `<= 0` blockIds OMIT the `#block=` fragment entirely (test asserts `#/pages/Broken` when blockId is `'evil'`).
- **T-02-22 (DoS via rapid typing)** — 150ms debounce verified by test (no fetch at 100ms, fetch fires at 160ms). AbortController cancels stale fetches inflight.

## Known Stubs

None.

## Self-Check: PASSED

- Files exist:
  - `frontend/src/lib/keys.ts` ✓
  - `frontend/src/lib/sanitize.ts` ✓
  - `frontend/src/lib/components/SearchPalette.svelte` ✓
  - `frontend/src/lib/components/SearchResult.svelte` ✓
  - `frontend/src/lib/components/__tests__/keys.test.ts` ✓
  - `frontend/src/lib/components/__tests__/palette.test.ts` ✓
  - `frontend/src/styles/palette.css` ✓
- Commits in `git log`: `dfe8a95`, `0d6a1ed`, `c9d2dc2`, `d60e4be` ✓
- Tests: 73/73 green ✓
- Build: `vite build` succeeds ✓

## Next Phase Readiness

- Wave 4 of Phase 2 is complete (this plan was the only Wave 4 member after 02-05 absorbed the wave-3 scope).
- Wave 5 = plan 02-07 (integration + smoke E2E). The palette is now wired into App.svelte, store, Sidebar, and SearchView — 02-07's E2E can drive the full Ctrl+K → result → block-scroll flow against a live backend.
- Wave 6 = plan 02-08 (perf gates). Bundle is 212.57 kB / 87.74 kB gz JS; well below typical SPA budgets and leaves headroom for 02-07 fixes.

---
*Phase: 02-read-only-web-ui*
*Completed: 2026-05-22*
