---
phase: 01-headless-indexing-core
plan: 02
subsystem: parser
tags: [rust, segmenter, state-machine, round-trip, acpt-01, prs-07]

requires: [01-01]
provides:
  - "Real fn segment(&[u8]) -> Vec<RawBlock> implementing the Stage 1 line-based state machine"
  - "Round-trip CI gate (ACPT-01 / PRS-07) flipped from RED to GREEN against synthetic AND real corpus"
  - "Block properties (key:: value) parsed into RawBlock.properties while preserved verbatim in raw"
  - ":LOGBOOK:/:END: drawers captured as opaque RawDrawer entries on the parent block"
  - "Code-fence-inside-bullet correctly swallows inner dash-lines (RESEARCH §Don't Hand-Roll)"
  - "15 hand-crafted unit tests covering empty input, prelude, bullets, continuation, fences, drawers, properties"
affects: [01-03-parser-refs, 01-04-storage, 01-05-scanner, 01-06-indexer, 01-07-cli]

tech-stack:
  added: []
  patterns:
    - "Zero-dependency segmenter: pure stdlib, no regex, no memchr — keeps core crate slim"
    - "State machine variables (fence/drawer) gate bullet detection — prevents false bullet splits inside code"
    - "Continuation prefix = depth TABs + 2 spaces; blank lines auto-glue to current block"
    - "Splice-noop by construction: every byte is consumed into exactly one block range"
    - "UTF-8 assumption surfaces loud panic per T-02-03 if violated"

key-files:
  created:
    - crates/core/tests/segment_unit.rs
  modified:
    - crates/core/src/parser/segment.rs

key-decisions:
  - "Empty input returns single prelude block with byte_length=0 (not empty Vec) — keeps the 'always at least one block' invariant trivial for downstream consumers"
  - "Skipped memchr crate dependency — std slice iteration is fast enough and zero new deps is cheaper than the perf delta at this scale"
  - "Property regex implemented manually (not via regex crate) — keeps `crates/core` dep-free until pulldown-cmark lands in plan 01-03"
  - "Drawer is closed at EOF if :END: is missing (T-02-04) — drawer.byte_length reaches source.len() so splice-noop still holds on malformed files"
  - "Empty-bullet line (just '-\\n') is recognized as a bullet — fixture 08 emits these"

patterns-established:
  - "Module-grouped unit tests (mod prelude / bullets / continuation / fences / drawers / properties) keep the corner-case matrix discoverable"
  - "Shared assert_splice_noop helper makes the round-trip invariant the universal assertion across every unit test"

requirements-completed: [PRS-01, PRS-02, PRS-03, PRS-05, PRS-06, PRS-07, ACPT-01]

duration: ~20min
completed: 2026-05-21
---

# Phase 1 Plan 02: Stage 1 Segmenter State Machine Summary

**Hand-rolled line-based segmenter that owns TAB-indent + 2-space continuation + fence-awareness + drawer-awareness; flips the round-trip CI gate from RED to GREEN against both the committed synthetic corpus (10 fixtures) and the local real Logseq base (620 files).**

## Performance

- **Duration:** ~20 min
- **Tasks:** 2
- **Files created:** 1 (`tests/segment_unit.rs`)
- **Files modified:** 1 (`src/parser/segment.rs`)
- **Round-trip test runtime:** ~150 ms for synthetic corpus, ~3 s for real corpus (620 files)

## Accomplishments

- `segment()` walks the source byte-by-byte via a small state machine: state vars are `current: RawBlock`, `fence: Option<FenceState>`, `drawer: Option<DrawerState>`. New-bullet detection is gated on `fence.is_none() && drawer.is_none()` — that single guard is what makes RESEARCH §Don't Hand-Roll's anti-pattern (off-the-shelf CommonMark mis-splitting fenced bullets) impossible by construction.
- Splice-noop invariant holds by construction: every byte of the source is appended to exactly one block's `byte_length`. The cumulative cursor over `byte_offset + byte_length` advances monotonically with no gaps.
- Round-trip test result: **2/2 passing** — both `roundtrip_byte_identical_for_synthetic_corpus` (10 files) and `roundtrip_byte_identical_for_real_corpus_if_present` (620 files locally) green.
- Unit tests: **15/15 passing** covering empty input, prelude-only, single bullet, nested 3-level, empty bullet (`-\n`), 2-space continuation, blank-line-inside, two siblings, fence-eats-dashes, nested-fence-then-sibling, LOGBOOK drawer, two drawers in one block, single property, multi-property, dotted-key property.
- AP-2 guard remains clean: `grep -rE "fn (serialize|to_markdown|format_block)" crates/` returns nothing.
- `cargo build --workspace --locked` exits 0.

## Task Commits

1. **Task 1: Implement the Stage 1 segmenter state machine** — `b5f4cda` (feat)
2. **Task 2: Hand-crafted unit tests for state-machine corners** — `14691a6` (test)

## Canonical Regression Fixture (data-folder-sample/Logseq/journals/2023_11_09.md)

Traced output for the canonical fence-in-bullet fixture (the file RESEARCH §Stage 1 validates against):

```
file: 5958 bytes; block count: 6
  block 0: depth=PRELUDE offset=0    len=0    props=0 drawers=0
  block 1: depth=0       offset=0    len=77   props=0 drawers=0
  block 2: depth=1       offset=77   len=58   props=0 drawers=0
  block 3: depth=1       offset=135  len=65   props=0 drawers=0
  block 4: depth=1       offset=200  len=211  props=0 drawers=0
  block 5: depth=1       offset=411  len=5547 props=0 drawers=0
```

Six blocks total (1 empty prelude + 5 bullets), with block 5 holding the entire ~150-line SQL code fence as a single 5547-byte slice. Splice-noop holds: `0 + 77 + 58 + 65 + 211 + 5547 = 5958` = file size. (RESEARCH §Stage 1 said "block 5 at depth 2" — the actual canonical file has the fence-hosting bullet at TAB-depth 1, which is what we observe. Block count, structure, and fence-preservation match the design intent.)

## Decisions Made

- **No memchr dependency.** Considered adding `memchr = "2"` per the plan guidance, but plain `iter().position(|&b| b == b'\n')` is fast enough at corpus scale (real-corpus round-trip runs in ~3 s for 620 files / ~17 MB total). Keeping `crates/core`'s dependency surface at exactly zero non-dev deps is more valuable than the perf delta.

- **No regex dependency.** Property detection (`key:: value`), drawer detection (`:LOGBOOK:`), and fence detection are all simple-enough byte patterns to recognize by hand-coded scanners. Adding `regex` would pull in ~10 transitive crates for what's three tiny matchers; deferred until something genuinely needs it.

- **Empty bullet `-\n` recognized as bullet.** Fixture 08 (`empty-and-deep.md`) emits `-\n` for empty bullets. `detect_bullet_depth` accepts both `- ...` and an exact `-` line.

- **Drawer EOF behavior.** If `:END:` is missing (malformed file), the drawer is closed at EOF with `byte_length` reaching `source.len() - drawer_offset`. This preserves the splice-noop invariant on malformed files (T-02-04 disposition: mitigate).

- **UTF-8 panic, not lossy.** Per T-02-03, `finalize()` uses `from_utf8(...).expect(...)` with a clear message. Phase 5 (Tauri) revisits if real-world bases surface non-UTF-8 content.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug fix] Test assertion `byte_length == 12` was actually 13**
- **Found during:** Task 2 (first run of `two_bullets_continuation_only_on_first`)
- **Issue:** I asserted `"- foo\n  cont\n"` was 12 bytes; it is 13 (6 + 7).
- **Fix:** Updated assertion to 13 and corrected the comment.
- **Files modified:** `crates/core/tests/segment_unit.rs`
- **Committed in:** `14691a6` (folded into Task 2 commit)

No other deviations. Plan body executed as written.

## Pitfalls Discovered

None new — the design directly applies the patterns already enumerated in PITFALLS Pitfall 1 (lossy round-trip) and RESEARCH §Don't Hand-Roll. The state-machine bullet-detection-gate (fence/drawer must both be closed) is the load-bearing piece, exactly as anticipated.

## Verification Results

- `cargo test --test roundtrip --package foliom-core` — exits 0 (`2 passed`)
- `cargo test --test segment_unit --package foliom-core` — exits 0 (`15 passed`)
- `cargo test --workspace --locked` — fully green
- `cargo build --workspace --locked` — exits 0
- AP-2 guard `grep -rE "fn (serialize|to_markdown|format_block)" crates/` — empty
- Real-corpus round-trip on this machine: 620 files, all byte-identical

## TDD Gate Compliance

Both tasks have `tdd="true"`. Plan 01-01 shipped the RED gate (`4e7a9ce`); plan 01-02 supplies the GREEN gate.

- **RED gate (plan 01-01):** `4e7a9ce` — `test(01-01): add failing round-trip property test ...`
- **GREEN gate (this plan):** `b5f4cda` — `feat(01-02): implement Stage 1 line-based segmenter state machine` makes the RED test pass
- **REFACTOR:** Not needed; segmenter is small, single-pass, and the tests are stable

Plan-level TDD cycle complete across the 01-01 / 01-02 plan boundary.

## Threat Flags

None new. Threat register entries T-02-01 .. T-02-04 are addressed:
- T-02-01 (deep TABs): depth is u8; lines beyond u8::MAX-1 fall back to "not a bullet" (continuation).
- T-02-02 (unclosed fence): linear scan; the whole file becomes one bullet block; splice-noop holds.
- T-02-03 (non-UTF-8): loud panic at `finalize()` with clear message.
- T-02-04 (unclosed drawer): drawer closes at EOF; splice-noop holds.
- T-02-SC (supply chain): zero new deps added.

## Next Plan Readiness

- `segment()` is now the GREEN target every downstream plan depends on. Public signature unchanged.
- `RawBlock.properties` and `RawBlock.drawers` are populated; plan 01-04 (storage) can persist them via D-05/D-06.
- Plan 01-03 (parser refs) feeds `block.raw` to pulldown-cmark for Stage 2 link/tag extraction.
- Plan 01-08 (inventory) can now report code-fence-inside-bullet counts, drawer counts, property-bearing-blocks — the underlying data is already on `RawBlock`.

## Self-Check: PASSED

- File `crates/core/src/parser/segment.rs` — present (modified)
- File `crates/core/tests/segment_unit.rs` — present (created)
- Commit `b5f4cda` — present in `git log`
- Commit `14691a6` — present in `git log`
- `cargo test --workspace --locked` — green
- AP-2 guard — clean

---
*Phase: 01-headless-indexing-core*
*Plan: 02*
*Completed: 2026-05-21*
