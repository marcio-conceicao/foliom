---
phase: 02-read-only-web-ui
plan: 05
subsystem: frontend-shell-sidebar-backlinks-theme
tags: [phase-2, frontend, sidebar, backlinks, journal-nav, theme]
requires:
  - 02-03 (frontend scaffold — Vite + Svelte 5 + vitest + svelte-spa-router; stores + api wrappers)
  - 02-04 (per-block markdown renderer — needed because Block.svelte is where unresolved-chip styling is now wired)
provides:
  - "Sidebar.svelte: left rail with debounced (100ms) search filter, alphabetical NOCASE grouped sections (Pages, Journals), placeholder sections (Favoritos, Recentes), JournalNavigator embedded in Journals section, ThemeToggle in footer"
  - "JournalNavigator.svelte: month-grid calendar with prev/next month buttons, day-click navigation to #/journals/YYYY-MM-DD, Hoje button calling /api/journals/today and converting page-name shape YYYY_MM_DD to router shape YYYY-MM-DD"
  - "ThemeToggle.svelte: tri-state segment control (Claro / Auto / Escuro) bound to `theme` store, eagerly applies <html data-theme=...> on pick so feedback is instant in tests + outside App.svelte"
  - "BacklinksPanel.svelte: <details>-based collapsible panel (open by default) under main page content, groups backlinks by source page, each entry links to #/pages/<src>#block=<id>; snippet rendered as escaped text only (T-02-17 mitigation)"
  - "PageLinkChip.svelte: small reusable chip component for template-space rendering (Sidebar/Backlinks); honors `resolved` prop with .unresolved class"
  - "Block.svelte retroactive .unresolved chip styling: post-render $effect walks .page-link elements inside .content, consults sidebarPages-derived Set, toggles .unresolved class. Empty store = render neutrally (avoids false unresolved styling before sidebar fetch completes)"
  - "App.svelte theme $effect now also subscribes to `prefers-color-scheme` change events so OS-level theme flips propagate when user is on 'Auto'"
  - "Anti-FOUC inline script in index.html: resolves theme + writes <html data-theme> BEFORE Svelte hydrates"
  - "sidebar.css: 280px desktop rail, collapses to top strip below 720px viewport (no off-canvas drawer in Phase 2 per deferred list)"
  - "global.css .page-link.unresolved styling: italic + opacity 0.6 (matches 02-CONTEXT §Specifics)"
affects:
  - "App.svelte — replaced 'Sidebar (plan 02-05)' stub with <Sidebar/>; theme handling moved to mqlMatches reactive + matchMedia change listener with cleanup"
  - "main.ts — imports sidebar.css after global.css"
  - "PageView.svelte — mounts <BacklinksPanel name={detail.name}/> after the block tree"
  - "Block.svelte — bind:this on .content + post-render $effect for retroactive unresolved chip styling; no behavior change to chip click delegation"
tech-stack:
  added: []  # no new deps — used native <details>, native Date math, native Svelte 5 runes
  patterns:
    - "Per-component eager theme application (ThemeToggle.pick) + authoritative App-level $effect with prefers-color-scheme listener cleanup — toggle feels instant outside App context (notably in vitest), but App.svelte remains source of truth at runtime"
    - "Late-arriving response guard in BacklinksPanel: capture `current = name` at effect start, only commit results if `current === name` at resolution — prevents stale backlinks rendering on rapid page switches"
    - "Retroactive DOM class toggle for unresolved chips (Block.svelte $effect over contentEl + resolvedSet): cleaner than re-running markdown-it with a context arg, and lets sidebarPages load asynchronously without forcing a re-render of every Block"
    - "Anti-FOUC inline script in index.html: pure ES5 IIFE reads localStorage('theme') + matchMedia, writes <html data-theme> before module scripts execute. Wrapped in try/catch so a broken localStorage (private mode) silently falls back to 'light'"
    - "Native <details> for backlinks collapse (no JS, accessible by default, browser-persisted state via summary semantics)"
key-files:
  created:
    - frontend/src/lib/components/Sidebar.svelte
    - frontend/src/lib/components/JournalNavigator.svelte
    - frontend/src/lib/components/ThemeToggle.svelte
    - frontend/src/lib/components/BacklinksPanel.svelte
    - frontend/src/lib/components/PageLinkChip.svelte
    - frontend/src/styles/sidebar.css
    - frontend/src/lib/components/__tests__/sidebar.test.ts
    - frontend/src/lib/components/__tests__/theme.test.ts
    - frontend/src/lib/components/__tests__/journal-nav.test.ts
    - frontend/src/lib/components/__tests__/backlinks.test.ts
  modified:
    - frontend/src/App.svelte
    - frontend/src/main.ts
    - frontend/src/styles/global.css
    - frontend/src/lib/components/Block.svelte
    - frontend/src/lib/pages/PageView.svelte
    - frontend/index.html
decisions:
  - "Did NOT add @testing-library/svelte — existing 02-04 tests use raw svelte `mount`/`unmount`; consistent with codebase and avoids a 70KB devDep that adds no leverage for the simple assertions Task 2 needs."
  - "ThemeToggle eagerly sets <html data-theme> on click (in addition to writing the store). Reason: in vitest happy-dom, App.svelte's $effect doesn't run inside an isolated ThemeToggle mount — the toggle needs to be self-sufficient or tests get brittle. App.svelte remains authoritative for prefers-color-scheme propagation."
  - "JournalNavigator uses native `Date` math (no `date-fns`) — keeps bundle tiny; calendar grid is O(31) entries so any overhead is negligible. `toLocaleDateString('pt-BR', {month: 'long', year: 'numeric'})` for the header label gives a localized 'março de 2024' for free."
  - "JournalNavigator's `initialMonth` prop is consumed in a non-reactive snapshot (`svelte-ignore state_referenced_locally`) — once mounted, the user navigates via prev/next/today; parent re-mutating the prop would be surprising. Prop is exposed mainly for deterministic testing."
  - "BacklinksPanel uses a guard `current === name` at effect-resolution time to drop late-arriving responses for previous page names. Without this, rapid sidebar clicks would briefly show Foo's backlinks under Bar's heading."
  - "Block.svelte's unresolved-chip $effect runs on the post-render DOM rather than re-running markdown-it with a chip-resolution callback. Reason: chip rendering happens in raw HTML via `{@html}` (data-page already attached by 02-04's inline rule); a DOM walk is O(chips-per-block) and only runs when rendered HTML or resolvedSet change. Re-rendering through markdown-it would invalidate the perf gate set in 02-04."
  - "Anti-FOUC script uses inline ES5 (var, function expressions) so it runs WITHOUT module transformation. Cannot import anything; must use globalThis.localStorage / globalThis.matchMedia. Catch handler defaults to 'light' (and console.warn for diagnosis) rather than silently swallowing — caught a SonarLint S2486 complaint by handling the exception meaningfully."
  - "02-06 surface area: left a // 02-06 will add: comment in App.svelte (search palette modal slot) and in Sidebar.svelte (Ctrl+K trigger button) so the next plan has clear insertion points without overlapping work."
metrics:
  duration_minutes: 7
  tasks_completed: 2
  test_count_added: 12  # 2 sidebar + 4 theme + 3 journal-nav + 3 backlinks
  test_count_total: 56  # was 44 after 02-04; now 44+12 = 56
  bundle_size:
    js_uncompressed_kb: 207.23
    js_gzip_kb: 85.89
    css_uncompressed_kb: 10.68
    css_gzip_kb: 2.55
  completed: 2026-05-22
---

# Phase 02 Plan 05: Sidebar + JournalNavigator + ThemeToggle + BacklinksPanel — Summary

Closes the visible-shell gaps left as stubs in plan 02-03. The three-zone
layout now has a real left rail (page list with debounced filter, journals
section anchored by a month-grid calendar, Favorites/Recents placeholders,
ThemeToggle in the footer); page content has a collapsible Backlinks panel
beneath the block tree; and the `[[NonExistent]]` chips that 02-04 left
neutral now render italic + dim once `sidebarPages` is loaded.

Delivers requirements **LNK-03** (backlinks panel), **LNK-06** (sidebar +
journal navigator with "today" button), and **UI-02** (dark mode with OS
default + persistence + anti-FOUC).

## Architecture

```
App.svelte
  ├─ <html data-theme=...> ← Anti-FOUC inline script (index.html, runs pre-hydration)
  │                       ← App.svelte $effect (post-hydration, subscribes to prefers-color-scheme)
  │                       ← ThemeToggle.pick() (eager on user click, in addition to store write)
  │
  ├─ <aside class="sidebar"> Sidebar.svelte
  │      ├─ search input (debounced 100ms → debouncedQuery)
  │      ├─ Favoritos / Recentes (empty placeholders, v1.x)
  │      ├─ Journals section
  │      │    ├─ JournalNavigator (month grid, prev/next, Hoje)
  │      │    └─ <ul data-section="journals"> filtered + alphabetized
  │      ├─ Páginas section
  │      │    └─ <ul data-section="pages"> filtered + alphabetized,
  │      │       .unresolved class on isResolved=false entries
  │      └─ footer: <ThemeToggle />
  │
  └─ <main> Router
       └─ PageView / JournalView
            ├─ <PageHeader />
            ├─ <Block> tree
            │    └─ post-render $effect: walk .page-link chips, toggle
            │       .unresolved against sidebarPages-derived Set
            └─ <BacklinksPanel name={detail.name} />
                 ├─ fetchBacklinks() with stale-response guard
                 ├─ groupBy(b.page)
                 ├─ snippet → stripForRender → text-bind (escaped)
                 └─ href = #/pages/<src>#block=<id>  → zoom listener picks up
```

## Test Coverage

All 56 vitest cases pass (44 pre-existing + 12 new):

| Suite                                              | Cases | Notes |
| -------------------------------------------------- | ----: | ----- |
| `components/__tests__/sidebar.test.ts`             |     2 | grouped Pages/Journals, alphabetical NOCASE, unresolved class, debounced filter |
| `components/__tests__/theme.test.ts`               |     4 | three buttons (Claro/Auto/Escuro), click → store + localStorage + data-theme sync, aria-pressed state |
| `components/__tests__/journal-nav.test.ts`         |     3 | day-click → #/journals/YYYY-MM-DD, prev/next month edge cases (Feb 29 leap year, April 30), Hoje → fetch + nav |
| `components/__tests__/backlinks.test.ts`           |     3 | grouped by source page with #block= hrefs, "Sem backlinks" empty state, snippet text escaped (T-02-17) |

## Sidebar layout choices

- **Desktop width:** 280px (was 260px stub; bumped to give the calendar grid breathing room)
- **Narrow viewport (≤ 720px):** collapses to a top strip (max-height 40vh) above the main content. No off-canvas drawer in Phase 2 — deferred to v1.x per 02-CONTEXT §Deferred.
- **Sections (top→bottom):** brand → filter input → Favoritos (empty) → Recentes (empty) → Journals (with calendar) → Páginas → ThemeToggle footer.
- **Sort:** `localeCompare(undefined, { sensitivity: 'base' })` for case-insensitive ordering matching the spec's "NOCASE".
- **Debounce:** 100ms via `setTimeout` (no third-party lib).

## Anti-FOUC approach

`index.html` has a pre-hydration `<script>` (plain ES5 IIFE, no module
imports possible) that:

1. Reads `globalThis.localStorage.getItem('theme')`.
2. If `'light' | 'dark'`, uses it.
3. Otherwise reads `globalThis.matchMedia('(prefers-color-scheme: dark)').matches`.
4. Writes `document.documentElement.dataset.theme = resolved`.
5. Catches all errors (e.g. private-mode storage quota), `console.warn`s, and falls back to `'light'`.

This runs BEFORE the Svelte bundle loads, so CSS variables that key on
`[data-theme=...]` are correct on first paint — no flash.

`App.svelte`'s `$effect` is still authoritative once mounted: it
subscribes to the `theme` store AND adds a `matchMedia` change listener
(with cleanup) so an OS-level theme flip propagates when the user is on
"Auto" without requiring a reload.

## @testing-library/svelte decision

**Not added.** The plan called it out as "add if missing", but the
existing block.test.ts (from 02-04) uses raw svelte `mount`/`unmount`
and reaches into the DOM via `querySelector` for assertions. That pattern
is concise and self-contained for the assertions Task 1 + Task 2 need
(presence of elements, text content, attribute values, click dispatch).
Pulling in `@testing-library/svelte` would have added ~70KB of devDeps
and a second testing idiom for no measurable leverage. If future plans
introduce more elaborate component interaction tests we can revisit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Anti-FOUC script lint cleanup**

- **Found during:** Task 1, after IDE diagnostics surfaced 4 SonarLint warnings on the inline script
- **Issue:** `window.*` references (S7764), `setAttribute` instead of `dataset` (S7761), empty catch (S2486), void operator (S3735)
- **Fix:** Switched to `globalThis.*`, used `dataset.theme = ...`, replaced empty catch with a meaningful handler (`console.warn` + safe default to `'light'`)
- **Files modified:** `frontend/index.html`
- **Commit:** `8d57cf8` (folded into Task 1 GREEN)

**2. [Rule 1 - Bug] Svelte `state_referenced_locally` compiler warning on JournalNavigator**

- **Found during:** Task 1 first GREEN run (warning, not test failure)
- **Issue:** `parseInitial(initialMonth)` called at script top-level captured `initialMonth` non-reactively, triggering Svelte 5's state warning
- **Fix:** Added `// svelte-ignore state_referenced_locally` with explanatory comment — intentionally non-reactive (parent shouldn't mutate `initialMonth` after mount; user navigates via prev/next/today)
- **Files modified:** `frontend/src/lib/components/JournalNavigator.svelte`
- **Commit:** `8d57cf8` (folded into Task 1 GREEN)

### No architectural deviations

All scope stayed within the plan. No Rule 4 architectural decisions required. No auth gates encountered.

## Coordination with 02-06

Plan 02-06 (search palette) is also in wave 4 and will touch `App.svelte`
and `stores.ts`. Left two surface markers:

- `App.svelte`: `// 02-06 will add: search palette modal slot here (mounted as a sibling of .layout, controlled by searchPalette store; visible via Ctrl/Cmd+K).`
- `Sidebar.svelte`: `// 02-06 will add: "Buscar (Ctrl+K)" trigger button that opens the searchPalette modal — leave the footer slot open for that.`

Did not pre-empt 02-06's `searchResults` store or palette modal — those
surfaces are clean.

## Known Stubs / Deferred Items

| Item | Reason | Resolution plan |
| ---- | ------ | --------------- |
| Favoritos sidebar section | Per 02-CONTEXT §Deferred — v1.x feature | Future micro-plan introduces favorites store + UI to add/remove |
| Recentes sidebar section | Per 02-CONTEXT §Deferred — v1.x feature | Could be wired soon via a `recentPages` store tracking `currentPage` writes |
| Off-canvas drawer on mobile | Phase 2 ships a simple top-strip collapse only | v1.x will replace with a hamburger toggle + `position: fixed` drawer |
| `@testing-library/svelte` | Not added — raw mount/unmount sufficed | Revisit if future plans need fireEvent / queries with rich semantics |

## Verification

- `cd frontend && npm run test -- --run` → **10 files, 56 tests passed** (2.57s)
- `cd frontend && npm run build` → **207.23 kB JS (85.89 kB gzip) + 10.68 kB CSS (2.55 kB gzip)** — comfortably within the strict gate planned for 02-08
- Manual smoke (deferred to live 02-05/06 dev session, non-blocking):
  - sidebar visible with three sections; filter narrows pages live
  - ThemeToggle switches `data-theme` instantly; localStorage updates
  - JournalNavigator: arrow buttons change month, day click navigates, Hoje opens today's journal
  - BacklinksPanel: visit a referenced page (e.g. `Glauber`); panel populated, click an entry, lands on source page with target block highlighted/scrolled into view
  - Unresolved chip styling: open a page containing `[[DoesNotExist]]`; chip renders italic + dim once sidebar finishes loading

## Self-Check: PASSED

All 16 created/modified files present on disk; 4 commits (`6f22201`,
`8d57cf8`, `be8eeb7`, `a653732`) on `main`.
