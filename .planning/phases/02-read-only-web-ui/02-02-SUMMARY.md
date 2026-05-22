---
phase: 02-read-only-web-ui
plan: 02
subsystem: cli-http-routes
tags: [phase-2, axum, rest-api, fts5, backlinks, journals, unicode, sanitization]
requires:
  - foliom-core::storage::Db (pages, blocks, refs, block_props, block_drawers, blocks_fts)
  - foliom-cli::serve::state::AppState (Arc<Mutex<Db>>)
provides:
  - "GET /api/pages                 — PageSummary[]"
  - "GET /api/pages/:name           — PageDetail (nested-tree blocks)"
  - "GET /api/pages/:name/backlinks — Backlink[]"
  - "GET /api/page-titles           — string[]"
  - "GET /api/journals/today        — 302 Location: /#/journals/YYYY_MM_DD"
  - "GET /api/journals?from&to      — JournalEntry[] with English-ordinal title"
  - "GET /api/search?q&kind&limit   — SearchHit[] (FTS5 bm25 OR tag-refs)"
  - "format_journal_title / parse_journal_name (`YYYY_MM_DD` ↔ `Month Dth, YYYY`)"
  - "DTO module (PageSummary, PageDetail, Block, DrawerRef, Backlink, JournalEntry, SearchHit) with camelCase serde"
affects:
  - crates/cli (8 new files: dto.rs, format.rs, routes/{pages,journals,search,titles}.rs, tests/serve_routes.rs, mod.rs/state.rs touch-up)
  - crates/core/tests/fixtures/logseq-synthetic (+ pages/Avaliação.md, journals/2024_03_15.md edits)
  - crates/cli/tests/cli_integration.rs (pinned inventory counts bumped by +1 scanned / +1 page)
tech-stack:
  added: []  # all new deps came from 02-01
  patterns:
    - "Path<String> percent-decoding handled by axum at extractor boundary; SQL receives canonical names (D-37)"
    - "rusqlite params![] for every bind — zero string-formatted SQL (T-02-04)"
    - "spawn_blocking per handler (D-25) — DB lock never held across .await"
    - "Search sanitization: empty after trim → []; unquoted ':' → reject; backslashes stripped"
    - "FTS5 snippet(blocks_fts, 0, '<mark>', '</mark>', '…', 16) ORDER BY rank"
    - "Tag-kind search routes through refs (NOT FTS5) — pages.refs join with kind='tag'"
    - "Page-kind search is rejected with 400 — clients use /api/page-titles instead"
    - "Tree assembly: depth stack walk over (id, ord, depth, raw); properties/drawers prefetched per-page (no N+1)"
key-files:
  created:
    - crates/cli/src/cmd/serve/dto.rs
    - crates/cli/src/cmd/serve/format.rs
    - crates/cli/src/cmd/serve/routes/pages.rs
    - crates/cli/src/cmd/serve/routes/journals.rs
    - crates/cli/src/cmd/serve/routes/search.rs
    - crates/cli/src/cmd/serve/routes/titles.rs
    - crates/cli/tests/serve_routes.rs
    - crates/core/tests/fixtures/logseq-synthetic/pages/Avaliação.md
    - .planning/phases/02-read-only-web-ui/02-02-SUMMARY.md
  modified:
    - crates/cli/src/cmd/serve/routes/mod.rs (+ 5 routes; axum-0.7 colon syntax for path params)
    - crates/cli/src/cmd/serve/mod.rs (re-export adjustments)
    - crates/cli/src/cmd/serve/state.rs (no-op cleanup)
    - crates/cli/tests/cli_integration.rs (pinned counts bumped: scanned 11→12, pages 10→11)
    - crates/core/tests/fixtures/logseq-synthetic/journals/2024_03_15.md (added nested children for tree-shape test)
    - crates/core/tests/indexer_integration.rs (one fixture count touch)
    - crates/core/tests/roundtrip.rs (one fixture count touch)
requirements: [LNK-02, LNK-03, LNK-05, LNK-06, SCH-01, SCH-02]
threats_mitigated: [T-02-04, T-02-05, T-02-06, T-02-07]
verification:
  automated:
    - "cargo test -p foliom-cli --test serve_routes — 15/15 green"
    - "cargo test --workspace — full suite green"
  manual_pending:
    - "Headed manual run vs synthetic corpus once frontend (02-04..02-06) lands"
---

## Outcome

All seven read-only endpoints from D-24 are live against the Phase 1 SQLite index. Wire contract for the frontend (02-04..02-06) is locked.

## Notes for Future Plans

- **`properties_json` / `drawers_json` storage shape** — Phase 1's schema uses normalized side tables (`block_props`, `block_drawers`), NOT JSON columns. The detail handler joins these per-page (one query each, prefetched) to assemble `properties: [[k, v], ...]` and `drawers: [DrawerRef]` arrays in the nested-tree response. Future mutation work (02-07 / Phase 3) should write back through these tables, not the JSON columns hypothesized in 02-RESEARCH.

- **axum path-param syntax** — workspace pins `axum = "0.7"` which uses matchit 0.7 (`:name`), NOT `{name}` (matchit 0.8 / axum 0.8+). The first implementation used `{name}` and produced "passing" tests that were actually hitting axum's fallback 404 instead of the handler — including `pages_detail_returns_404_on_missing` which was a false positive. Fix: revert to `:name`. If we ever upgrade axum to 0.8, this is the one breaking-change to remember.

- **Search query sanitization is conservative** — bare backslashes are stripped (some legitimate queries like `\d` lose precision; documented as accepted in T-02-05) and any unquoted `:` rejects the whole query (FTS5 column-filter injection guard). Both reject paths return `[]`, never an error.

- **Pitfall 6 (UTF-8 in FTS5 snippet)** — `pages/Avaliação.md` was added specifically so that `GET /api/search?q=Avalia%C3%A7%C3%A3o` exercises non-ASCII tokenization. The snippet round-trips with no `�` replacement glyphs. This becomes a regression guard for any FTS tokenizer config change.

- **Inventory regression counts bumped** — adding `Avaliação.md` shifted `EXPECTED_SCANNED` 11 → 12 and `EXPECTED_PAGES` 10 → 11 in `crates/cli/tests/cli_integration.rs`. Pattern counts (`alias::`, `id::`, `LOGBOOK`, `#[[...]]`, `SCHEDULED:`, `code-fence-in-bullet`, `%2F-in-filename`) were untouched — the new fixture is plain prose. Phase 1's inventory contract stays green.

- **2024_03_15.md fixture** — the original minimal version had only top-level bullets; the test `pages_detail_journal_has_formatted_title_and_prelude_root` requires nested children to verify tree assembly. Added two TAB-nested children under "Pair on parser segmenter" without introducing new inventory patterns.

## Open Items Surfaced

- **02-04 markdown renderer** depends on the JSON wire shape locked here. If anything in `Block` / `DrawerRef` ergonomics needs adjusting (e.g., flattening `[[k, v]]` to `{key, value}`), do it before 02-04 ships, not after.
- **bm25 ordering** is enforced via `ORDER BY rank`, but the synthetic corpus is too small to differentiate scores meaningfully. Plan 02-08 (perf gates) should add a query-quality assertion on a larger corpus.
- **Journal title locale** — currently English long-form only. The PRD lets the user override via `config.edn :journal/page-title-format`; that wiring lives in 02-06 (sidebar / journal navigator) not here.
