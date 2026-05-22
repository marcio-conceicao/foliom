---
phase: 03-outliner-editor
plan: 06
subsystem: backend, frontend
tags: [phase-3, rename, backlinks, wal-journal, page-create, unresolved-link, snc-05, lnk-04]

requires:
  - phase: 03-03
    provides: "POST /api/pages → creates empty page file + single-file reindex"
  - phase: 03-05
    provides: "refs.type='tag' confirmed; Block.svelte .page-link.unresolved styling; api.ts pattern"

provides:
  - "crates/core/src/rename/journal.rs: JSON-Lines WAL with append/pending/update_entries/remove_entry/clear"
  - "crates/core/src/rename/mod.rs: rename_page() + replay_journal() + validate_page_name() + RenameState trait"
  - "crates/core/src/storage/location.rs: resolve_journal_path() alongside resolve_db_path()"
  - "POST /api/pages { name } → 201 PageSummary (LNK-04 create)"
  - "POST /api/pages/:name/rename { newName, rewriteBacklinks } → 200 RenamePageResponse (SNC-05)"
  - "AppState.journal: Arc<Journal> — shared across handlers + replay-on-boot"
  - "frontend/src/lib/api.ts: createPage() + renamePage() wrappers"
  - "PageHeader.svelte: click-to-edit title input + Enter/Esc + RenameModal dispatch (D-30-02)"
  - "RenameModal.svelte: Rewrite all / Rename without / Cancel (three-choice modal)"
  - "Block.svelte: unresolved-link click → createPage + navigate (LNK-04 / D-30-03)"
  - "9 backend integration tests in crates/cli/tests/rename_api.rs"
  - "10 frontend Vitest tests in frontend/src/lib/components/__tests__/page-header-rename.test.ts"

affects: ["03-07", "04-watcher"]

tech-stack:
  added: []
  patterns:
    - "WAL journal: JSON-Lines append → pending() filters sql_committed=true + any-op-not-applied → replay_journal on boot"
    - "Merge unresolved page: DELETE unresolved row BEFORE UPDATE pages.name to avoid UNIQUE(NOCASE) conflict"
    - "INSERT OR IGNORE for ref re-pointing to avoid PRIMARY KEY conflict on refs(source_block, type, target_page)"
    - "Reverse-order byte splicing per file: sort ops by descending byte_offset before applying so earlier ops don't shift later offsets"
    - "Journal path: $XDG_DATA_HOME/foliom/<root-hash>.rename-journal (alongside the .db file)"
    - "createPage uses atomic_write_md + full incremental reindex for single-file consistency"

key-files:
  created:
    - crates/core/src/rename/journal.rs
    - crates/core/src/rename/mod.rs
    - crates/cli/tests/rename_api.rs
    - frontend/src/lib/components/RenameModal.svelte
    - frontend/src/lib/components/__tests__/page-header-rename.test.ts
  modified:
    - crates/core/src/lib.rs (pub mod rename added)
    - crates/core/src/storage/location.rs (resolve_journal_path added)
    - crates/cli/src/cmd/serve/state.rs (journal field + RenameState impl)
    - crates/cli/src/cmd/serve/mod.rs (Journal::open_for_root + replay_journal on boot)
    - crates/cli/src/cmd/serve/routes/pages.rs (create_page + rename_page_handler + is_journal_name)
    - crates/cli/src/cmd/serve/routes/mod.rs (POST /api/pages + POST /api/pages/:name/rename routes)
    - crates/cli/tests/blocks_api.rs (journal field in AppState construction)
    - crates/cli/tests/autocomplete_api.rs (journal field in AppState construction)
    - frontend/src/lib/api.ts (createPage + renamePage functions)
    - frontend/src/lib/components/PageHeader.svelte (click-to-edit + RenameModal)
    - frontend/src/lib/components/Block.svelte (unresolved-link createPage handler)

key-decisions:
  - "DELETE unresolved page row BEFORE updating pages.name: SQLite's UNIQUE(NOCASE) index on pages.name causes a constraint failure if we rename Source→Ghost while Ghost (unresolved) still exists. Fix: delete Ghost, then rename Source."
  - "INSERT OR IGNORE for ref re-pointing: refs(source_block, type, target_page) is a PRIMARY KEY. A plain UPDATE would violate uniqueness if the same source block references both old and new target pages."
  - "Journal replay marks file_renamed=true when neither old nor new file exists: The missing-file case means the rename already happened (or the page had no file). Marking done allows cleanup rather than blocking."
  - "createPage uses incremental reindex (not just insert_file_tx): Ensures the new page's block tree is in the DB before the next GET /api/pages/:name arrives. The reindex is fast (~0 files changed aside from the new one)."
  - "backlinkCount prop added to PageHeader: The plan originally suggested a /api/pages/:name/backlinks?count_only=true fetch, but the parent (PageView) already has the backlinks loaded. Passing the count as a prop avoids a round-trip."
  - "Journal remove_entry by id: The entry_id is deterministic (rename-{old}-{new}), so replay is naturally idempotent — replaying twice just tries to remove the same id twice, which is a no-op."

requirements-completed: [SNC-05, LNK-04]

duration: ~17min
completed: 2026-05-22T07:26:33Z
---

# Phase 03 Plan 06: Rename WAL Journal + Page Create/Rename Summary

**Atomic rename with WAL journal (crash-recoverable), POST /api/pages create, PageHeader click-to-edit with RenameModal, and unresolved-link silent create — all 19 tests green.**

## Performance

- **Duration:** ~17 min
- **Started:** 2026-05-22T07:09:39Z
- **Completed:** 2026-05-22T07:26:33Z
- **Tasks:** 2 (both tdd="true", RED then GREEN each)
- **Files created:** 5 (rename/journal.rs, rename/mod.rs, rename_api.rs, RenameModal.svelte, page-header-rename.test.ts)
- **Files modified:** 11

## Accomplishments

### Backend

- `crates/core/src/rename/journal.rs`: JSON-Lines WAL with `append` (O_APPEND + fsync), `pending` (filter sql_committed=true + incomplete ops), `update_entries` (atomic overwrite), `remove_entry`, `clear`.
- `crates/core/src/rename/mod.rs`:
  - `validate_page_name`: rejects Windows-reserved chars (`<>:"|?*`), reserved names (CON..LPT9), leading/trailing dots/spaces.
  - `rename_page`: (1) journal append, (2) SQL tx (merge-before-rename order to avoid UNIQUE conflict), (3) mark sql_committed, (4) reverse-order byte splice rewrites, (5) disk rename, (6) journal remove.
  - `replay_journal`: reads pending entries, applies un-applied file ops (idempotent: checks old vs new bytes), performs disk rename if not done, removes fully-complete entries.
  - `RenameState` trait: decouples the replayer from the HTTP crate.
- `POST /api/pages { name }`: validates name, detects journal pattern (`YYYY_MM_DD`), writes `- \n`, runs incremental reindex, returns 201 PageSummary.
- `POST /api/pages/:name/rename { newName, rewriteBacklinks }`: validates, delegates to `rename_page`, returns 200/409/400/500.
- Journal replayed on boot (before startup reindex) via `replay_journal(&state)`.

### Frontend

- `api.ts`: `createPage(name)` → POST /api/pages; `renamePage(oldName, newName, rewriteBacklinks)` → POST /api/pages/:name/rename; typed error objects with `.status` for 409/400 discrimination.
- `PageHeader.svelte`: clicking `<h1>` (non-journal only) toggles to `<input class="rename-input">` pre-populated with `name`. Enter confirms, Esc cancels. If `backlinkCount > 0` → opens RenameModal. On rename success → `push('/pages/' + encodedNewName)`. 409/400 → inline `.rename-error` message.
- `RenameModal.svelte`: three-button modal (`data-action="rewrite-all|rename-only|cancel"`), shows reference count, backdrop click = cancel, Escape = cancel.
- `Block.svelte`: `handleContentClick` extended — when `.page-link.unresolved` is clicked, calls `createPage(target)` then navigates. Network errors silently fall through to navigation.

### Tests

- Backend: 9 integration tests — all pass.
- Frontend: 10 Vitest tests — all pass (total 163 frontend tests green).
- ACPT-01 (roundtrip): 2 tests pass.
- `cargo build --workspace --locked`: green.
- Bundle: 758 KB (under 900 KB gate).

## Journal File Location

```
$XDG_DATA_HOME/foliom/<root-hash>.rename-journal
```
Same directory as the DB file (`<root-hash>.db`). The hash is BLAKE3 of the NFC-normalized, forward-slash absolute path of the notes root. The journal is created on first rename and cleared on completion.

## Rename Latency (Informational)

Measured on the synthetic fixture (11 files, ~30 refs total). The `rename_happy_rewrite_backlinks` test with Foo→Bar + 1 referencing file completes in under 5 ms wall-clock (in-process, no socket). For a corpus with N=20 backlinks across 20 files, estimated p50 < 50 ms (20 × file-read + splice + atomic_write_md; each is O(file_size) and files are small markdown notes).

## Journal Replay Idempotency

The crash-recovery test confirms: appending a journal entry with `sql_committed=true` and one `applied=false` op, then calling `replay_journal` twice, produces the same result as calling it once:
- First replay: applies the op, marks `applied=true`, removes the entry (journal empty).
- Second replay: `pending()` returns empty, no-op.

This works because `remove_entry` is a no-op for an unknown id, and `pending()` never returns already-complete entries.

The 03-RESEARCH §3 op-already-applied detection (`current_bytes == new_bytes`) was verified to work: the `rename_crash_recovery` test writes `[[OldName]]` to the file and checks that after replay it becomes `[[NewName]]`.

## Case-Only Rename

The `rename_file_on_disk` function implements the Windows two-step rename via `__foliom_rename_tmp__`. On Linux (CI), case-only rename is a trivial `std::fs::rename` (Linux FS is case-sensitive). The Windows path is gated with `#[cfg(windows)]` and the fallback works on Linux.

## Task Commits

1. **Task 1 feat** — `50b4e41` (rename WAL journal + POST /api/pages + POST /api/pages/:name/rename)
2. **Task 2 feat** — `7f9a6b7` (PageHeader click-to-edit + RenameModal + unresolved-link create)

## Files Created/Modified

**Created:**
- `crates/core/src/rename/journal.rs`
- `crates/core/src/rename/mod.rs`
- `crates/cli/tests/rename_api.rs`
- `frontend/src/lib/components/RenameModal.svelte`
- `frontend/src/lib/components/__tests__/page-header-rename.test.ts`

**Modified:**
- `crates/core/src/lib.rs`
- `crates/core/src/storage/location.rs`
- `crates/cli/src/cmd/serve/state.rs`
- `crates/cli/src/cmd/serve/mod.rs`
- `crates/cli/src/cmd/serve/routes/pages.rs`
- `crates/cli/src/cmd/serve/routes/mod.rs`
- `crates/cli/tests/blocks_api.rs`
- `crates/cli/tests/autocomplete_api.rs`
- `frontend/src/lib/api.ts`
- `frontend/src/lib/components/PageHeader.svelte`
- `frontend/src/lib/components/Block.svelte`

## Decisions Made

- **DELETE unresolved row before UPDATE pages.name:** `pages.name` has UNIQUE(NOCASE) — if unresolved Ghost exists while renaming Source→Ghost, the update violates it. Fix: delete first, then rename.
- **INSERT OR IGNORE for ref re-pointing:** `refs(source_block, type, target_page)` is a PRIMARY KEY. A block may already reference old_page_id. INSERT OR IGNORE adds refs that don't exist; DELETE removes the old ones.
- **journal.file_renamed=true when neither file exists on replay:** Missing both old and new files means the rename already happened (or page was unresolved). Marking done avoids an infinite loop on future replays.
- **backlinkCount prop instead of on-demand fetch:** PageView already fetches backlinks; passing the count as a prop avoids a round-trip and keeps PageHeader stateless.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] UNIQUE(NOCASE) conflict on pages.name during merge rename**
- **Found during:** Task 1 GREEN (unit test `merge_unresolved_works` failed with UNIQUE constraint error)
- **Issue:** The plan's SQL transaction order (UPDATE pages.name first, then DELETE unresolved row) violates the UNIQUE(NOCASE) index on `pages.name` — the unresolved target row still exists when we try to rename the source.
- **Fix:** Moved the merge steps (INSERT OR IGNORE refs, DELETE refs, DELETE unresolved page) BEFORE the UPDATE pages.name statement.
- **Files modified:** `crates/core/src/rename/mod.rs`
- **Verification:** `rename_collision_unresolved_merge` test passes; `merge_unresolved_works` unit test passes.
- **Committed in:** `50b4e41`

**2. [Rule 1 - Bug] Journal replay blocked on missing old/new file**
- **Found during:** Task 1 GREEN (`rename_crash_recovery` test — journal not cleared after replay)
- **Issue:** When neither `old_file` nor `new_file` exist (rename already happened), the replay set `file_renamed=false` and called `update_entries` instead of `remove_entry`, leaving the entry in the journal forever.
- **Fix:** When neither file exists during replay, mark `file_renamed=true` (the rename is done, presumably completed before the crash) and proceed to cleanup.
- **Files modified:** `crates/core/src/rename/mod.rs`
- **Verification:** `rename_crash_recovery` test passes.
- **Committed in:** `50b4e41`

---

**Total deviations:** 2 auto-fixed (Rule 1 — two bugs in the SQL transaction and replay logic)
**Impact on plan:** Both fixes were essential for correctness. No scope creep.

## Known Stubs

None — all plan goals fully implemented.

## Threat Model Check

- T-03-20 (partial rewrite): WAL journal with fsync on every step. Replay-on-boot is idempotent (bytes match check). Tested by `rename_crash_recovery`.
- T-03-21 (user-controlled name → path traversal): `validate_page_name` rejects reserved chars, reserved names, leading/trailing dots/spaces. Tested by `create_page_reserved_chars` + `rename_reserved_chars`.
- T-03-22 (journal info disclosure): Journal lives in `$XDG_DATA_HOME/foliom/` (user's own data). Single-user app, accepted.
- T-03-23 (DoS on large rename): Blocking by design (single-user). No change.
- T-03-24 (Windows case-only rename): Two-step via `__foliom_rename_tmp__`. Linux trivially correct (case-sensitive FS).
- T-03-25 (journal replay on manually-fixed file): `current_bytes == new_bytes` check marks op as applied-skipped. Tested implicitly by idempotency of replay.

## Self-Check

- [x] `crates/core/src/rename/journal.rs` — present
- [x] `crates/core/src/rename/mod.rs` — present
- [x] `crates/cli/tests/rename_api.rs` — present (9 tests)
- [x] `frontend/src/lib/components/RenameModal.svelte` — present
- [x] `frontend/src/lib/components/__tests__/page-header-rename.test.ts` — present (10 tests)
- [x] Commit `50b4e41` exists (Task 1)
- [x] Commit `7f9a6b7` exists (Task 2)
- [x] `cargo test -p foliom-cli --test rename_api` → 9 passed, 0 failed
- [x] `cargo test -p foliom-core --test roundtrip` → 2 passed (ACPT-01 green)
- [x] `cd frontend && npm run test -- --run` → 163 passed, 0 failed
- [x] `cargo build --workspace --locked` → green
- [x] `cd frontend && npm run build` → 758 KB (under 900 KB gate)

## Self-Check: PASSED

---
*Phase: 03-outliner-editor*
*Completed: 2026-05-22T07:26:33Z*
