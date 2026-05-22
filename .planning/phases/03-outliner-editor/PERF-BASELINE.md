# ACPT-05 CI Pass Baseline

**Recorded:** 2026-05-22
**Plan:** 03-07
**Test:** `cargo test -p foliom-cli --test portability_acpt_05`

## Corpus

| Metric | Value |
|--------|-------|
| Total corpus files (post-edit) | 16 .md files |
| Base corpus (logseq-synthetic) | 11 files (pages/ + journals/) |
| Curated ACPT-05 fixtures added | 2 files (journal_2026_05_22.md, page_with_code_drawer_props.md) |
| Files created during edit sequence | 2 (ACPT05BrandNewPage.md, ACPT05Renamed.md) |
| Files renamed during edit sequence | 1 (ACPT05Target → ACPT05Renamed) |

## Edit Scenarios

| Scenario | Operation | Result |
|----------|-----------|--------|
| 1 | PUT /api/blocks/:id (edit existing) | 200 OK |
| 2 | POST /api/blocks (insert sibling) | 201 Created |
| 3 | PATCH /api/blocks/:id/structure (indent) | 200 OK |
| 4 | PATCH /api/blocks/:id/structure (outdent) | 200 OK |
| 5 | DELETE /api/blocks/:id | 204 No Content |
| 6 | POST /api/blocks ×3 (paste tree simulation) | 201 Created ×3 |
| 7 | POST /api/pages (create via unresolved link) | 201 Created |
| 8 | POST /api/pages/:name/rename (rewrite backlinks) | 200 OK |

Total edit operations: 11 HTTP mutations across 8 scenarios.

## Assertions Executed

| Assertion | Scope | Result |
|-----------|-------|--------|
| No CRLF introduced | All 16 .md files | PASS |
| No BOM injected | All 16 .md files | PASS |
| Valid UTF-8 | All 16 .md files | PASS |
| `id::` count unchanged per file | All 16 .md files | PASS |
| `((` count unchanged per file | All 16 .md files | PASS |
| `<!-- foliom` count unchanged | All 16 .md files | PASS |
| `.foliom-` count unchanged | All 16 .md files | PASS |
| `foliom_uuid` count unchanged | All 16 .md files | PASS |
| CommonMark parseable (no panic) | All 16 .md files | PASS |
| ACPT-01 segment round-trip | All 16 .md files | PASS |
| `id:: 6f7a3c9e-...` preserved verbatim | page_with_code_drawer_props.md | PASS |
| `:LOGBOOK:` / `:END:` preserved | page_with_code_drawer_props.md | PASS |
| TAB indentation integrity | pages/01-simple-bullets.md | PASS |

## Wall-clock Performance

| Metric | Value |
|--------|-------|
| Test suite wall time | ~0.09–0.11 s (in-process, no socket overhead) |
| Operations per test | 11 HTTP mutations + 16-file corpus scan ×5 assertion passes |

## Notes

- The `id:: 6f7a3c9e-1d4b-4a12-9b8c-2f4e5d6a7c1f` in `page_with_code_drawer_props.md`
  is a Logseq-format property from the fixture. The Foliom-metadata grep compares
  pre-edit vs post-edit counts, so this pre-existing `id::` line does NOT trigger a
  false positive. This is the correct behavior: Foliom preserves Logseq metadata, it
  does not inject its own.
- The manual Obsidian / VS Code verification is PENDING (fills in during /gsd-verify-work).
