---
phase: 03-outliner-editor
plan: 02
subsystem: mutation
tags: [phase-3, backend, mutation, byte-splice, tree-ops, pure-logic, snc-01, d-30-05]

# Dependency graph
requires:
  - phase: 02-parser-storage
    provides: "RawBlock segmenter with byte_offset/byte_length contract; round-trip ACPT-01 gate"
provides:
  - "splice_block(original, byte_offset, byte_length, new_raw) -> Vec<u8> — byte-identical splice into a file buffer"
  - "compute_shifted_offsets(blocks, changed_block_id, old_len, new_len) — pure offset bookkeeping for downstream blocks"
  - "BlockOffset trait — generic offset access; impls for RawBlock and (i64,usize,usize) tuple"
  - "TreeOp enum (Indent/Outdent/Merge/Split/Move/Delete) with invertible apply()"
  - "MutableTree / MutableBlock — flat in-memory tree representation"
  - "BlockSnapshot with reparented_children for invertible Delete"
affects: ["03-03 mutation REST handlers", "03-04 undo stack", "03-05 paste/drag/drop"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure-function mutation core — no IO, no async, no SQL — composed by HTTP layer in plan 03-03"
    - "Invertible ops: every apply() returns the inverse TreeOp for client-side undo (D-30-05)"
    - "Test-only Vec ordering normalization (assert_trees_equal) to compare semantically-equal MutableTrees that differ in storage order"

key-files:
  created:
    - "crates/core/src/mutation/mod.rs — module wiring + re-exports"
    - "crates/core/src/mutation/splice.rs — splice_block + BlockOffset + compute_shifted_offsets"
    - "crates/core/src/mutation/tree_ops.rs — TreeOp/MutableTree/BlockSnapshot + per-variant apply"
    - "crates/core/src/mutation/__tests__/splice_test.rs — 13 tests including round-trip noop over 11 fixtures"
    - "crates/core/src/mutation/__tests__/tree_ops_test.rs — 15 tests (invertibility, edges, serde)"
  modified:
    - "crates/core/src/lib.rs — added `pub mod mutation;`"

key-decisions:
  - "BlockSnapshot carries an explicit `reparented_children: Vec<i64>` (Rule 2 deviation) — necessary for invertible Delete on a block with children. Cleaner than encoding the child count inside the existing `raw` field via Unicode sentinels."
  - "Merge concatenates raw bytes verbatim (no '\\n' separator inserted) and Split is the pure inverse via byte_offset slicing. Earlier prototype inserted a separator newline; it broke Split↔Merge invertibility for blocks whose raw already ended in '\\n'."
  - "Merge closes the ord gap left by the removed sibling; Split re-opens it on the inverse. Without gap-closure on Merge, Split→Merge round-trips diverged on sibling ord values."
  - "RawBlock's BlockOffset::id() returns 0 — RawBlock has no SQL id until plan 03-03 wraps it. Sufficient for offset-only call sites (the splice tests). Production callers always use storage-row wrappers carrying real ids."

patterns-established:
  - "TreeOp::apply consumes self and returns inverse — Rust ownership encodes 'one op, one inversion' invariant"
  - "BlockOffset trait abstracts over any (id, offset, length) triple — keeps splice math storage-agnostic; SQL row types in plan 03-03 just impl it"

requirements-completed: [SNC-01]

# Metrics
duration: ~25 min
completed: 2026-05-22
---

# Phase 3 Plan 02: Mutation Core (byte-splice + invertible TreeOps) Summary

**Pure-logic mutation primitives — byte-splice with offset bookkeeping and six invertible TreeOp variants — composed by plan 03-03's HTTP layer for SNC-01 writeback.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-22T05:50:00Z (approx)
- **Completed:** 2026-05-22T06:15:25Z
- **Tasks:** 2
- **Files modified:** 6 (5 created, 1 modified)
- **Test count:** 28 mutation tests (13 splice + 15 tree_ops); 0 failures
- **ACPT-01 round-trip gate:** unchanged, 2 passing as before

## Accomplishments

- `splice_block` — 6-line pure function with byte-identity guarantee, exercised over every non-prelude block in 11 synthetic fixtures (49 splice instances total) — every result byte-identical to the input.
- `compute_shifted_offsets` — handles shrink, grow, last-block, prelude-changed, and no-op cases; bounded O(n) over a single pass; downstream-offset underflow detected and panics loudly.
- `TreeOp::apply` for all 6 variants (Indent/Outdent/Merge/Split/Move/Delete) — every test asserts `inv.apply(...)` reproduces the original tree (sorted-by-id) including raw, depth, parent_id, ord.
- 3 documented error cases return the correct `TreeOpError`: `FirstChildCannotIndent`, `AlreadyAtRoot`, `CannotMergeIntoPrelude`, plus `InvalidSplitOffset` for OOB / non-char-boundary splits.
- Serde round-trip is bit-stable for every TreeOp variant (`serde_json::from_str(&serde_json::to_string(&op)?)? == op`).
- ACPT-01 round-trip CI gate stays green — this plan touched zero bytes of `segment.rs` / `ast.rs`.

## Task Commits

1. **Task 1: splice_block + compute_shifted_offsets + round-trip tests** — `a113c88` (feat; combined commit, see note below)
2. **Task 2: TreeOp + invertible apply() + edge tests** — `8c44793` (feat)

**Note on Task 1's commit:** the parallel-wave 03-01 agent's commit `a113c88` ("feat(03-01): add SelfWriteSet ...") inadvertently bundled all of plan 03-02 Task 1's files (splice.rs, mod.rs, tree_ops.rs stub, splice_test.rs, lib.rs edit) into the same commit. Files are byte-identical to what this plan would have committed standalone; only the commit message conflates the two plans. No data loss, no rework. Documented here so the verifier doesn't flag the missing `feat(03-02)` Task 1 commit as a gap. Surface area: `crates/core/src/mutation/{mod,splice,tree_ops}.rs` and `__tests__/splice_test.rs` originate from this plan.

## Files Created/Modified

- `crates/core/src/lib.rs` — added `pub mod mutation;`
- `crates/core/src/mutation/mod.rs` — module wiring + public re-exports
- `crates/core/src/mutation/splice.rs` — splice_block, BlockOffset trait, compute_shifted_offsets (128 LoC incl. docs)
- `crates/core/src/mutation/tree_ops.rs` — MutableTree, MutableBlock, BlockSnapshot, TreeOp, TreeOpError, per-variant apply_* helpers (~430 LoC incl. docs)
- `crates/core/src/mutation/__tests__/splice_test.rs` — 13 tests (6 literal byte ops, 1 round-trip noop over 11 fixtures, 5 shift edge cases, 1 trait-impl smoke)
- `crates/core/src/mutation/__tests__/tree_ops_test.rs` — 15 tests (invertibility for 6 variants, 4 error edge cases, serde round-trip, MutableTree helpers)

## Decisions Made

- **BlockSnapshot.reparented_children added (Rule 2 deviation, see below).** Considered encoding the child count inside `raw` via PUA Unicode sentinels — rejected as too clever; explicit field is clearer and serde-friendly.
- **Merge uses pure byte concat (no '\\n' separator).** The semantic '\\n' separator belongs to the *callers* who construct the raw strings; the tree op should be a pure string concatenation so Split is the exact inverse via `str::split_at`.
- **assert_trees_equal helper sorts blocks by id before comparison.** `Vec::push` in apply_split and apply_restore puts new blocks at the end of the storage vec, but logical tree equality must ignore storage order. Production callers (plan 03-03) re-serialize to SQL by ord/parent, so storage order is never load-bearing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added `reparented_children: Vec<i64>` to BlockSnapshot**
- **Found during:** Task 2 (Delete inverse)
- **Issue:** Plan's `<interfaces>` block specified `BlockSnapshot { raw, depth, parent_id, ord }` — but inverting a `Delete` that re-parented N children requires knowing which N child ids to pull back. Without this field, the inverse op cannot distinguish "the slot at snapshot.ord now contains an original sibling that shifted into the gap" from "the slot contains a formerly-reparented child". Heuristic-based detection (e.g., walk siblings forward until depth changes) was tried but ambiguous in the general case.
- **Fix:** Added `reparented_children: Vec<i64>` with `#[serde(default)]` so the wire shape is a strict superset. apply_delete populates it; apply_restore consumes it. Documented in module doc comment.
- **Files modified:** crates/core/src/mutation/tree_ops.rs
- **Verification:** `delete_block_with_children_reparents_to_grandparent` test asserts inverse restores the tree exactly; ten other invertibility tests pass alongside.
- **Committed in:** 8c44793 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Necessary for invertibility correctness. No scope creep; serde-compat preserved via `#[serde(default)]`.

## Issues Encountered

- **Initial Merge↔Split round-trip diverged** because Merge inserted a separator `\n` and Split tried to strip a trailing `\n`. Fix: removed both behaviors; Merge does pure concat, Split does pure slice. Caught by `split_then_inverse_restores_tree`.
- **Sibling ord shift not closed on Merge.** Removing a block from the middle of a sibling list left a gap (e.g., siblings ord [0, 1, 2] became [0, 2] instead of [0, 1]). Split inverse then shifted later siblings further right and the inverse diverged. Fix: Merge now closes the gap. Caught by `merge_then_inverse_restores_tree`.
- **MutableTree storage-order sensitivity in PartialEq.** Tests initially used the derived `PartialEq` which compared `Vec` order — false negatives when `Vec::push` appended new blocks at the end. Resolved with a test-only `assert_trees_equal` that sorts by id before compare.

## User Setup Required

None - no external service configuration required.

## Threat Flags

None - no new threat surface introduced. The module is pure logic with no IO, no syscalls, no network. T-03-04 (splice math off-by-one) is mitigated by the round-trip noop test over every synthetic fixture (49 splice instances, 0 divergences). T-03-05 (TreeOp::Delete DoS on wide subtrees) remains accepted per the plan's threat-register disposition.

## Next Phase Readiness

- Plan 03-03 (mutation REST handlers) can now compose `splice_block` + `atomic_write` (plan 03-01 wave 1 sibling) without any further pure-logic work.
- `TreeOp` is `serde::Serialize + Deserialize` — ready for `treeOpLog` HTTP wire format in plan 03-04 (D-30-05 hybrid undo).
- `MutableTree` is internal to mutation for now. Plan 03-03 will write the SQL-row → MutableTree adapter (likely in `crates/server/src/mutation_adapter.rs`).
- No blockers.

## Self-Check: PASSED

- FOUND: crates/core/src/mutation/splice.rs
- FOUND: crates/core/src/mutation/tree_ops.rs
- FOUND: crates/core/src/mutation/mod.rs
- FOUND: crates/core/src/mutation/__tests__/splice_test.rs
- FOUND: crates/core/src/mutation/__tests__/tree_ops_test.rs
- FOUND: commit a113c88 (Task 1 — combined w/ 03-01, see note above)
- FOUND: commit 8c44793 (Task 2)
- VERIFIED: `cargo test -p foliom-core --lib mutation` → 28 passed, 0 failed
- VERIFIED: `cargo test -p foliom-core --test roundtrip` → 2 passed (ACPT-01 unaffected)
- VERIFIED: `cargo build --workspace --locked` → green

---
*Phase: 03-outliner-editor*
*Completed: 2026-05-22*
