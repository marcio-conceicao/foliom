---
phase: 01-headless-indexing-core
plan: 03
subsystem: parser
tags: [rust, pulldown-cmark, ast, ref-extraction, relative-path, nfc, prs-04, idx-07]

requires: [01-02]
provides:
  - "extract_refs(raw: &str) -> Vec<ExtractedRef> via pulldown-cmark event stream + suppression stack"
  - "Hand-rolled inline scanner for [[link]], #[[composite tag]], and #bare-tag with hex-color and URL-fragment rejection"
  - "RelativePath newtype owning the NFC + forward-slash storage invariant"
  - "PathError enum (PathOutsideRoot, NonUtf8Path, UnexpectedPathComponent) for path-traversal mitigation (T-03-02)"
  - "Segmenter-prefix stripping so pulldown-cmark sees plain paragraph text, not indented code blocks"
affects: [01-04-storage, 01-05-scanner, 01-06-indexer]

tech-stack:
  added:
    - "pulldown-cmark 0.13 (default-features = false) — CommonMark + GFM event stream parser"
    - "unicode-normalization 0.1 — UnicodeNormalization::nfc()/nfd() iterator adapters"
    - "thiserror 1 — derive macro for PathError variants"
  patterns:
    - "Suppression-depth counter — increment on Tag::Heading/CodeBlock/Link/Image, decrement on matching TagEnd, scan Event::Text only at depth 0"
    - "Text-run buffering — pulldown-cmark splits `[`, `[`, `Foo`, `]`, `]` into 5 separate Text events; concatenate consecutive Text events and scan the joined buffer, flushing on structural boundaries"
    - "Strict-prefix order for `#[[…]]` vs `#bare` — recognize composite tag BEFORE bare tag scanner runs, otherwise `#[[Foo]]` parses as `#` + literal `[[Foo]]`"
    - "Hex-color rejection by length: token of exactly 3 / 6 / 8 hex digits is dropped (covers #fff, #1a2b3c, #deadbeef alpha)"
    - "URL-fragment guard: `#` immediately preceded by ASCII alphanumeric is never a tag (mitigates `https://x.com/page#anchor` and `text#fragment`)"
    - "RelativePath::from_filesystem walks `Component::Normal` only — non-Normal (CurDir, ParentDir, RootDir, Prefix) is auto-rejected"

key-files:
  created:
    - crates/core/src/parser/ast.rs
    - crates/core/src/path.rs
    - crates/core/tests/ast_unit.rs
    - crates/core/tests/path_unit.rs
  modified:
    - crates/core/Cargo.toml
    - crates/core/src/lib.rs
    - crates/core/src/parser/mod.rs
    - Cargo.lock

key-decisions:
  - "Strip the segmenter's structural prefix (TABs + `- ` on first line, TABs + 2 spaces on continuation lines) BEFORE feeding to pulldown-cmark — otherwise `\\t- text` is interpreted as an indented code block and every ref is suppressed"
  - "%2F decoder is a literal substring replacer, not a general percent-decoder (LNK-02 scope)"
  - "Trailing `.` on a bare tag is stripped (`#Tag2.` → `Tag2`); only `.` is stripped, not `,` or `;` (kept the rule minimal until corpus tells us otherwise)"
  - "Hex-color guard rejects tokens of exactly 3 / 6 / 8 hex digits — `#beef` (4 hex chars) WOULD survive as a tag; the disambiguating syntax for a hex tag is `#[[beef]]`"
  - "Bare-tag first char must be alphanumeric or underscore (or non-ASCII) — `#-foo` and `#/foo` are not tags"
  - "v1 conservative: anything inside a markdown link `[text](url)` is suppressed for refs, including refs in the link text. Re-evaluate if real usage shows people typing `[see [[Page]]](url)`"

requirements-completed: [PRS-04]
requirements-partial: [IDX-07]

duration: ~25min
completed: 2026-05-21
---

# Phase 1 Plan 03: Stage 2 Parser (Refs) + RelativePath Summary

**pulldown-cmark event-stream walker with suppression-depth counter and a hand-rolled inline scanner extracts `[[link]]`, `#[[composite tag]]`, and `#bare-tag` from CommonMark text nodes only; `RelativePath` newtype owns the NFC + forward-slash storage invariant and rejects non-Normal path components for path-traversal safety.**

## Performance

- **Duration:** ~25 min of active work (4 commits in sequence; no checkpoints)
- **Tasks:** 2
- **Files created:** 4 (`src/parser/ast.rs`, `src/path.rs`, `tests/ast_unit.rs`, `tests/path_unit.rs`)
- **Files modified:** 4 (`Cargo.toml`, `src/lib.rs`, `src/parser/mod.rs`, `Cargo.lock`)
- **Test suite runtime:** AST 24 tests <1 ms, Path 9 tests <1 ms, total workspace test ~3 s (dominated by real-corpus round-trip, unchanged)

## Accomplishments

- `extract_refs(raw: &str) -> Vec<ExtractedRef>` walks pulldown-cmark events with `Options::ENABLE_TABLES | ENABLE_STRIKETHROUGH`, maintains a `suppress_depth` counter for headings / code blocks / links / images, buffers consecutive `Event::Text` payloads, and scans the concatenated buffer on each structural boundary. The 24 AST unit tests cover: page links, bare tags, composite tags, suppression (heading + fenced code + inline code + link), hex-color rejection (3 and 6 hex digits), URL-fragment suppression, %2F decoding (upper + lower case, in both link and composite-tag forms), NFC≡NFD equivalence, and two positive cases lifted from `fixtures/logseq-synthetic/pages/05-links-and-tags.md`.
- `RelativePath::from_filesystem` walks `Path::components()` and accepts only `Component::Normal`, NFC-normalizing each component string and joining with `/`. Anything else — `..`, `.`, `RootDir`, `Prefix` — is rejected with `UnexpectedPathComponent`. Combined with `strip_prefix` (which rejects paths outside `root` via `PathOutsideRoot`), this covers T-03-02 path traversal end-to-end. Path-unit test `from_filesystem_rejects_dotdot_component_traversal` is the regression guard.
- NFC-equals-NFD test (`from_filesystem_nfd_normalizes_to_nfc`) constructs two PathBufs from `"Avaliação".nfc()` and `"Avaliação".nfd()`, asserts they differ byte-wise, then asserts both produce identical `RelativePath::as_str()`. This is the IDX-07 invariant in test form.
- Round-trip CI gate (ACPT-01) from Plan 02 STILL passes — this plan added new modules but touched no segmenter code. AP-2 guard `grep -rE "fn (serialize|to_markdown|format_block)" crates/` returns nothing.
- `cargo test --workspace --locked` fully green: 24 (ast) + 9 (path) + 2 (roundtrip) + 15 (segment) = 50 tests passing.

## Task Commits

1. **Task 1 RED — failing AST ref extraction tests:** `c44ce77`
2. **Task 1 GREEN — Stage 2 ref extractor:** `7489342`
3. **Task 2 RED — failing RelativePath tests:** `2fc3ca4`
4. **Task 2 GREEN — RelativePath newtype:** `a4c1971`

## Decisions Made

- **Segmenter-prefix stripping at the boundary, not at the parser core.** The first failing test (`page_05_continuation_extracts_tag_and_composite`) revealed that pulldown-cmark interprets `\t- Mencionou ...` as an indented code block (any line with 4+ spaces or a TAB at column 0 is "indented code" in CommonMark). The Stage 1 segmenter contract preserves the leading `\t…\t- ` prefix in `RawBlock.raw` (D-12, Plan 02). Two options were considered:
  1. Change the segmenter to expose a "stripped" raw view (rejected — would break the splice-noop invariant).
  2. Strip the prefix inside `extract_refs` before feeding to pulldown-cmark (chosen — keeps Stage 1 untouched and centralizes the knowledge of the bullet-prefix shape).
  The chosen approach: `strip_segmenter_prefix(raw)` walks lines, drops `\t*- ` on the first line and `\t*  ` on continuation lines. Non-matching lines pass through verbatim so the function is safe on edge inputs (page-prelude blocks, malformed blocks).

- **Text-run buffering across pulldown-cmark text events.** Probing with a throwaway harness (`/tmp/cmark-probe`) showed that pulldown-cmark 0.13 emits punctuation like `[`, `[`, `Foo`, `]`, `]` as **five separate** `Event::Text` events. Scanning each event individually means `[[Foo]]` never matches. The buffer-and-flush approach (concatenate consecutive Text events, scan on structural boundary) is the minimal correct walker for this shape. This is a real pulldown-cmark API quirk worth flagging for future plans that consume Event::Text — they need the same buffering.

- **Hex-color rejection by length-3/6/8 only.** The threat register (T-03-05) listed `#deadbeef` as a hex-color false-positive. With strict 3/6/8 lengths only, `#deadbeef` (8 hex chars, common 8-digit RGBA) is rejected, but `#beef` (4 hex chars) survives. Documenting in PRD/spec: users who want `#beef` as a literal tag use the unambiguous `#[[beef]]` form.

- **URL-fragment guard via "preceded by ASCII alphanumeric".** The cheapest reliable signal that a `#` is a URL fragment is "the character immediately before it is alphanumeric" (e.g., `example.com/page#anchor`). This rejects every URL fragment that came through pulldown-cmark as plain text (rare — most URLs become `Tag::Link` and are suppressed wholesale by the depth stack). The `text#NoSpace` case from the plan's negative-list is also handled by this rule.

- **`thiserror` derives on `PathError` include `PartialEq`/`Eq`.** Required by the unit-test pattern `matches!(err, PathError::PathOutsideRoot)`. Cheap to derive; no allocations.

## pulldown-cmark 0.13 API Quirks Discovered (assumption A6 / Tertiary source confidence)

1. **Text-event splitting:** As above — punctuation is emitted as individual `Text` events. Code consuming the event stream MUST buffer across consecutive `Text` events to see multi-character patterns.

2. **`Tag::Heading` shape:** v0.13 uses `Tag::Heading { level, id, classes, attrs }` (struct variant), not the v0.10-style `Tag::Heading(HeadingLevel, ...)`. Pattern: `Event::Start(Tag::Heading { .. })`. Matched by `TagEnd::Heading(HeadingLevel)` (tuple variant).

3. **`Tag::Link` / `Tag::Image` are struct variants:** `Tag::Link { link_type, dest_url, title, id }`. Matched by `TagEnd::Link` / `TagEnd::Image` (unit variants — no fields).

4. **`Event::Code` for inline code, `Tag::CodeBlock` for fenced/indented blocks.** Inline code never enters a `Start`/`End` pair — it's one self-contained event. Our walker treats it as a flush-boundary that contributes no text.

5. **No `Options::ENABLE_GFM` constant.** GFM features are opt-in piecemeal via `ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`, etc. We enable Tables + Strikethrough only — sufficient for this corpus, and avoids the perf cost of features the segmenter doesn't even let through.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Initial walker scanned each Text event individually, missing `[[…]]`**
- **Found during:** Task 1 GREEN (first cargo test run; 12 tests failed)
- **Issue:** pulldown-cmark splits `[[Foo]]` into 5 separate Text events; scanning each one in isolation never matches the `[[…]]` pattern.
- **Fix:** Added text-run buffering — accumulate consecutive Text events into a `String`, flush on each non-Text event (structural start/end, inline code, soft/hard break). Scan the flushed buffer.
- **Files modified:** `crates/core/src/parser/ast.rs`
- **Committed in:** `7489342` (folded into Task 1 GREEN before commit — single GREEN commit)

**2. [Rule 1 — Bug] Tab-indented bullet `\t- Foo` was being eaten as indented code**
- **Found during:** Task 1 GREEN (final failing test after the buffering fix)
- **Issue:** Stage 1 segmenter emits `RawBlock.raw` starting with `\t…\t- `; pulldown-cmark interprets any line indented by a TAB at column 0 as an indented code block, suppressing every ref inside.
- **Fix:** Added `strip_segmenter_prefix()` that runs over the raw text before parsing; drops the `\t*- ` prefix on the first line and the `\t*  ` continuation prefix on subsequent lines. Non-matching lines pass through (page-prelude safety).
- **Files modified:** `crates/core/src/parser/ast.rs`
- **Committed in:** `7489342` (same GREEN commit as above)

### CLAUDE.md Compliance

`./CLAUDE.md` mandates "GSD workflow enforcement: do not make direct repo edits outside a GSD workflow". This executor IS the GSD workflow — Plan 01-03 from the planning phase. Confirmed in scope.

## Pitfalls Discovered

- **pulldown-cmark splits punctuation into individual Text events** — load-bearing for any future code that does pattern-matching on text content (the inventory CLI in Plan 01-08 will need the same buffering, and so will the renderer in Phase 2 if it wants to colorize tags inline).
- **`\t` at column 0 = indented code in CommonMark** — every consumer of `RawBlock.raw` that feeds it to pulldown-cmark must strip the structural prefix first. Cannot be done generically inside pulldown-cmark because it doesn't know about Logseq's TAB-bullet convention.

## Verification Results

- `cargo test --test ast_unit --package foliom-core` — exits 0 (`24 passed`)
- `cargo test --test path_unit --package foliom-core` — exits 0 (`9 passed`)
- `cargo test --test roundtrip --package foliom-core` — exits 0 (`2 passed` — regression guard for ACPT-01 holds)
- `cargo test --test segment_unit --package foliom-core` — exits 0 (`15 passed`)
- `cargo test --workspace --locked` — fully green
- AP-2 guard `grep -rE "fn (serialize|to_markdown|format_block)" crates/` — empty
- AP-1 guard: `extract_refs` is documented per-block-only at the top of `parser/ast.rs`

## TDD Gate Compliance

Both tasks have `tdd="true"`. Both observe the RED→GREEN sequence in git log:

- **Task 1 RED:** `c44ce77` — `test(01-03): add failing AST ref extraction tests` (24 tests, all panicking on `todo!`)
- **Task 1 GREEN:** `7489342` — `feat(01-03): implement Stage 2 ref extractor (PRS-04)` (24 / 24 passing)
- **Task 2 RED:** `2fc3ca4` — `test(01-03): add failing RelativePath tests` (9 tests, all panicking on `todo!`)
- **Task 2 GREEN:** `a4c1971` — `feat(01-03): implement RelativePath newtype (IDX-07)` (9 / 9 passing)

No REFACTOR commits needed; the GREEN code is small and direct.

## Threat Flags

None new. Threat register entries T-03-01 .. T-03-SC are addressed:

- **T-03-01 (DoS via giant `[[…]]`):** linear-scan inline parser; per-block size already capped by segmenter.
- **T-03-02 (path traversal):** `RelativePath::from_filesystem` rejects any non-Normal `Component`; regression test `from_filesystem_rejects_dotdot_component_traversal` is permanent.
- **T-03-03 (symlink leak):** not addressed here by design — Phase 1 scanner (Plan 05) sets `follow_links(false)`. `from_filesystem` does not call `canonicalize()`, so it cannot leak symlink targets.
- **T-03-04 (non-UTF-8 filename):** `from_filesystem` returns `NonUtf8Path`; indexer (Plan 06) will log and skip.
- **T-03-05 (hex-color false-positive):** rejected for exact 3 / 6 / 8 hex digits; `#beef` (4) survives — disambiguation is `#[[beef]]`. Documented.
- **T-03-SC (supply chain):** three new deps. All HIGH legitimacy:
  - `pulldown-cmark 0.13` (D-10, RESEARCH §Don't Hand-Roll)
  - `unicode-normalization 0.1` (D-15, RESEARCH §RelativePath)
  - `thiserror 1` (D-19)
  No `[ASSUMED]`/`[SUS]` packages.

## Next Plan Readiness

- `extract_refs` returns `Vec<ExtractedRef>` per block, ready for Plan 01-06 (indexer) to insert into `refs` table.
- `RelativePath` is the type Plan 01-04 (storage) should use for `files.path` — every insert and every query parameter goes through `RelativePath`.
- The text-run buffering pattern in `parser/ast.rs::extract_refs` is reusable by Plan 01-08 (inventory) for counting `#[[…]]` and code-fence-inside-bullet patterns.
- The `strip_segmenter_prefix` helper currently lives inside `parser/ast.rs`. If Plan 01-08 (inventory) or a future renderer also needs to feed `RawBlock.raw` to pulldown-cmark, it should be hoisted to a shared helper. Not done now — premature factoring.

## Known Stubs

None. `extract_refs` and `RelativePath` are fully wired with no placeholders or hardcoded empties flowing to consumers.

## Self-Check: PASSED

- File `crates/core/src/parser/ast.rs` — present (created)
- File `crates/core/src/path.rs` — present (created)
- File `crates/core/tests/ast_unit.rs` — present (created)
- File `crates/core/tests/path_unit.rs` — present (created)
- Commit `c44ce77` — present in `git log` (Task 1 RED)
- Commit `7489342` — present in `git log` (Task 1 GREEN)
- Commit `2fc3ca4` — present in `git log` (Task 2 RED)
- Commit `a4c1971` — present in `git log` (Task 2 GREEN)
- `cargo test --workspace --locked` — green (50 tests across 4 test binaries)
- AP-2 guard — clean
- `pulldown-cmark`, `unicode-normalization`, `thiserror` in `crates/core/Cargo.toml` — confirmed via `cat`

---
*Phase: 01-headless-indexing-core*
*Plan: 03*
*Completed: 2026-05-21*
