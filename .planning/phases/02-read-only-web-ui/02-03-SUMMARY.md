---
phase: 02-read-only-web-ui
plan: 03
subsystem: frontend-scaffold
tags: [phase-2, frontend, svelte5, vite, scaffold, router]
requires:
  - 02-01 (HTTP server scaffold — for the future `/api/*` proxy target; not blocking this plan's build)
provides:
  - frontend toolchain (Vite 5 + Svelte 5 + TS 5 + vitest 2)
  - typed REST wrappers in `frontend/src/lib/api.ts` for the Phase 2 contract
  - Svelte stores (`currentPage`, `sidebarPages`, `theme`, `searchPalette`)
  - svelte-spa-router hash routing with placeholder views
  - empty-dist `.gitkeep` so plan 02-07 (`rust-embed`) compiles on fresh clones
affects:
  - root `.gitignore` (defense-in-depth `frontend/node_modules`, `frontend/dist/*`, `synthetic-5k`)
tech-stack:
  added:
    - svelte@5.37 (Svelte 5 with runes — `$state`, `$effect`, `$props`)
    - svelte-spa-router@5.1.0 (hash routing — confirmed Svelte ^5 peer dep)
    - markdown-it@14 + prismjs@1.29 (declared; rendering arrives in plan 02-04)
    - vite@5.4, @sveltejs/vite-plugin-svelte@4
    - vitest@2 + happy-dom@15
    - typescript@5.5, svelte-check@4, @types/markdown-it, @types/prismjs, tslib
  patterns:
    - "Hash-based SPA routing (D-28): /#/pages/:name, /#/journals/:date, /#/search"
    - "Same-origin `/api/*` calls — Vite proxy in dev, rust-embed in prod (D-23, D-37)"
    - "RelativePath-style URL encoding at the boundary (encodeURIComponent on every path segment)"
    - "Theme persisted in localStorage with allowlist coercion (T-02-10)"
key-files:
  created:
    - frontend/package.json
    - frontend/package-lock.json
    - frontend/tsconfig.json
    - frontend/vite.config.ts
    - frontend/vitest.config.ts
    - frontend/index.html
    - frontend/.gitignore
    - frontend/.gitattributes
    - frontend/README.md
    - frontend/dist/.gitkeep
    - frontend/src/main.ts
    - frontend/src/App.svelte
    - frontend/src/routes.ts
    - frontend/src/lib/api.ts
    - frontend/src/lib/stores.ts
    - frontend/src/lib/pages/PageView.svelte
    - frontend/src/lib/pages/JournalView.svelte
    - frontend/src/lib/pages/SearchView.svelte
    - frontend/src/lib/pages/NotFound.svelte
    - frontend/src/lib/pages/RedirectToday.svelte
    - frontend/src/styles/global.css
    - frontend/src/__tests__/stores.test.ts
    - frontend/src/__tests__/api.test.ts
  modified:
    - .gitignore (added frontend/node_modules, frontend/dist/*, synthetic-5k)
decisions:
  - "Router: svelte-spa-router 5.1.0 — peerDependencies confirmed `svelte: ^5.0.0` via `npm view`. No fallback needed."
  - "Node 24.14.1 + npm 11.11.0 used on the dev machine; package.json targets Node 20+ per README."
  - "Vite proxy uses `changeOrigin: false` — Foliom backend binds 127.0.0.1 same-origin; Host header should remain `localhost:5173` to match the future axum allowlist (Plan 02-01 trust boundary)."
  - "RedirectToday issues a plain `fetch('/api/journals/today')` and parses `res.url` after the browser follows the 302 (Plan 02-02's contract). If 02-02 returns a JSON `{ name }` instead, the wrapper will be updated then — for now both shapes are easy to swap."
  - "Placeholder pages render `<pre>{JSON.stringify(...)}</pre>` only — no `{@html ...}` until plan 02-04 (T-02-11)."
metrics:
  duration_minutes: ~10
  tasks_completed: 2 (Task 1 checkpoint auto-resolved via npm registry lookup; Tasks 2-3 implemented)
  test_count: 11 (8 api + 3 stores)
  bundle_size:
    js_uncompressed: 45.29 KB
    js_gzip: 17.22 KB
    css_uncompressed: 1.65 KB
    css_gzip: 0.59 KB
    dist_total_uncompressed: ~47 KB (well below the 600 KB plan-level ceiling; strict gate ships in plan 02-08)
  completed: 2026-05-21
---

# Phase 02 Plan 03: Frontend Scaffold (Svelte 5 + Vite + TS) — Summary

Bootstraps the Foliom SPA at `frontend/` with Svelte 5 runes, Vite 5 dev server, the
`/api → 127.0.0.1:7345` proxy, svelte-spa-router hash routing, typed REST wrappers
mirroring the Phase 2 backend contract, four Svelte stores, placeholder views that
JSON-dump fetched data, and a vitest+happy-dom smoke suite — all so plan 02-04 can
slot in the real Block renderer and plan 02-07 can rust-embed `dist/` without
scaffolding ceremony.

## What Shipped

### Toolchain
- `frontend/package.json` + `package-lock.json` materialized via `npm install`.
  91 packages installed in 15 s, lockfile committed (cross-platform — no absolute paths).
- `vite.config.ts` proxies `/api/*` → `http://127.0.0.1:7345` with `changeOrigin: false`.
- `vitest.config.ts` uses happy-dom and shares the Svelte plugin (HMR disabled in test mode).
- `tsconfig.json`: ES2022 / ESNext modules / bundler resolution / strict / verbatimModuleSyntax / isolatedModules.
- `.gitattributes` pins LF line endings for `.svelte`, `.ts`, `.json`, etc. (Windows CI hygiene, Pitfall 5).
- Root `.gitignore` gains `frontend/node_modules/`, `frontend/dist/*` (with `!frontend/dist/.gitkeep`),
  and `crates/core/benches/fixtures/synthetic-5k/`.

### App Shell + Routing
- `main.ts` uses Svelte 5's `mount(App, { target })` — never `new App({ target })` (legacy v4 API).
- `App.svelte` is a two-column grid: 260 px sidebar stub (literal "Sidebar (plan 02-05)") +
  main column hosting `<Router {routes} />`. A `$effect` resolves `theme === 'auto'`
  via `window.matchMedia('(prefers-color-scheme: dark)')` and sets `:root[data-theme]`.
- `routes.ts` maps `/`, `/pages/:name`, `/journals/:date`, `/search`, `*`.
- `RedirectToday.svelte` calls `resolveJournalToday()`, then `replace('/journals/<name>')`.
- Each placeholder view (`PageView`, `JournalView`, `SearchView`, `NotFound`) renders
  a Portuguese heading and a `<pre>` JSON dump of the fetched payload.

### Stores
- `currentPage: Writable<PageDetail | null>` (starts null; set by `PageView` / `JournalView`).
- `sidebarPages: Writable<PageSummary[]>` (lazy-loaded by plan 02-05).
- `theme: Writable<'light' | 'dark' | 'auto'>` — initial value coerced from
  `localStorage.getItem('theme')` through a 3-value allowlist (T-02-10); writes
  flow back to localStorage via a subscription.
- `searchPalette: Writable<{ open: boolean; query: string }>` (Ctrl+K state, plan 02-06).

### API Wrappers
All wrappers in `frontend/src/lib/api.ts` use **relative URLs** (`/api/...`) so dev (Vite
proxy) and prod (rust-embed) share the same code path (D-23, D-37). Every path segment
goes through `encodeURIComponent` (T-02-09).

| Function                | URL                                                  |
| ----------------------- | ---------------------------------------------------- |
| `fetchPages()`          | `GET /api/pages`                                     |
| `fetchPage(name)`       | `GET /api/pages/{enc}`                               |
| `fetchBacklinks(name)`  | `GET /api/pages/{enc}/backlinks`                     |
| `fetchPageTitles()`     | `GET /api/page-titles`                               |
| `fetchSearch(q, kind?)` | `GET /api/search?q=…&kind=…&limit=20`                |
| `fetchJournalsRange`    | `GET /api/journals?from=…&to=…`                      |
| `resolveJournalToday()` | `GET /api/journals/today` (reads final URL)          |

TypeScript interfaces (`PageSummary`, `Block`, `PageDetail`, `Backlink`, `JournalEntry`,
`SearchHit`, `SearchKind`) mirror the JSON contract from plan 02-02 verbatim.

### Tests
- `api.test.ts` (8 tests): URL building, encodeURIComponent for `Parent/Child` and `A B`,
  query-param assembly, default limit, non-2xx throws.
- `stores.test.ts` (3 tests): `currentPage` is null on init, `theme` persists to
  localStorage, `searchPalette` initial shape.
- Total: **11/11 passing** in 949 ms.

### Production Build
```
dist/index.html                  0.39 KB │ gzip:  0.27 KB
dist/assets/index-*.css          1.65 KB │ gzip:  0.59 KB
dist/assets/index-*.js          45.29 KB │ gzip: 17.22 KB
```
Total dist ~47 KB uncompressed — comfortably under the 600 KB plan ceiling. The strict
bundle gate is enforced in plan 02-08.

## Task 1 Checkpoint Resolution

The plan flagged `svelte-spa-router` `[SUS]` for Svelte 5 compatibility (audit could not
verify automatically). Resolved by querying npm directly:

```
$ npm view svelte-spa-router peerDependencies version --json
{
  "peerDependencies": { "svelte": "^5.0.0" },
  "version": "5.1.0"
}
```

The current published version (5.1.0) **explicitly requires Svelte 5**, so the
fallback options (`@hsorby/svelte-spa-router`, `tinro`, hand-rolled router) were
not needed. Locked at `^5.1.0` in `package.json`.

Per auto-mode and the executor's standing instruction ("No need to ask the user —
your call based on the npm metadata"), this was treated as a routine technical
decision and resolved without a human round-trip.

## Deviations from Plan

### [Rule 3 — Blocking] Pinned router to `^5.1.0` not `^4`

- **Found during:** Task 1 (registry lookup).
- **Issue:** Plan suggested `svelte-spa-router ^4` as the default. The 4.x line predates
  Svelte 5; only 5.x onward declares `svelte: ^5.0.0` peer dep.
- **Fix:** Pinned `svelte-spa-router: ^5.1.0`. Same package, same API surface
  (`Router`, `routes` object, `replace`).
- **Files modified:** `frontend/package.json`, `frontend/package-lock.json`
- **Commit:** 1103d10

### [Rule 2 — Critical, npm audit]

`npm install` reports 8 vulnerabilities (7 moderate, 1 critical) — all in dev-only
transitive deps (`esbuild` ≤0.24.2 dev-server CORS, vitest mocker, vite-plugin-svelte
v4, etc.). Auto-fix would require semver-major bumps that violate the plan's pinned
versions (`vite ^5.4`, `vitest ^2`, `@sveltejs/vite-plugin-svelte ^4`). Documented
here per Rule 4 (architectural change requires user decision); not auto-bumped.

**Mitigation status today:** none of these affect a production bundle — they touch the
dev server only. Recommendation for plan 02-08 (CI): include `npm audit --omit=dev`
in the gate, not `npm audit`, so prod-only severity surfaces.

## Manual Verification Steps Not Automated Here

These are deferred to local dogfooding once plan 02-02's REST endpoints are merged:

1. `cargo run -p foliom-cli -- serve crates/core/tests/fixtures/logseq-synthetic --port 7345`
2. `cd frontend && npm run dev` in a second terminal
3. Open `http://localhost:5173/#/pages/Some%20Page` and confirm JSON dump of that page
4. Open `http://localhost:5173/#/search?q=Glauber` and confirm JSON dump of hits
5. In the browser console: `localStorage.setItem('theme', 'dark'); location.reload();` →
   `<html data-theme="dark">` is set and CSS variables flip

These are documented in the plan's `<verification>` block; not automatable inside a
single agent because they require concurrent backend + frontend processes.

## Forward Wiring

- **Plan 02-04** replaces `<pre>{JSON.stringify(...)}</pre>` in `PageView` with a
  `Block` renderer using `markdown-it` + Prism.
- **Plan 02-05** turns the sidebar stub into a real page list + journal navigator,
  using `sidebarPages` and `fetchPages()`.
- **Plan 02-06** wires `Ctrl+K` → `searchPalette` store → `SearchView`.
- **Plan 02-07** introduces `rust-embed` against `frontend/dist/`. The `.gitkeep`
  file committed here means `cargo check` works even before someone runs
  `npm run build` for the first time.
- **Plan 02-08** adds a strict bundle-size gate to CI (placeholder: 300 KB JS).

## Self-Check: PASSED

- Files created: all 23 frontend/src files + 10 frontend/ root files + .gitignore exist on disk.
- Commits exist: `1103d10` (Task 2 scaffold) and `9f68b2f` (Task 3 shell+stores+tests).
- `npm run build` clean (854 ms, 0 errors).
- `npm run test -- --run` clean (11/11 in 949 ms).
- `frontend/dist/.gitkeep` is tracked (`git ls-files frontend/dist/.gitkeep` returns the path).
