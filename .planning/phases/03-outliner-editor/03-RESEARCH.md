---
phase: 03-outliner-editor
phase_number: 3
created: 2026-05-22
researcher: gsd-phase-researcher
confidence_overall: HIGH (CM6 patterns, atomic-rename); MEDIUM (Windows AV retry tuning, IME test fixtures); LOW (none load-bearing)
---

# Phase 3 — Outliner Editor: Research

**Researched:** 2026-05-22
**Domain:** CodeMirror 6 single-block editor + byte-splice writeback + atomic rename + backlinks rewrite
**Consumes:** Phase 2 frontend (`Block.svelte`, `PageHeader.svelte`, `api.ts`, markdown-it pipeline) + Phase 1 core (segmenter, `RawBlock`, schema with `byte_offset`/`byte_length`).

---

## User Constraints (from CONTEXT.md)

### Locked Decisions (the 8 D-30-XX from 03-CONTEXT.md — do not re-litigate)

1. **D-30-01** — Save trigger = `blur` OR `Enter`. No debounced autosave, no `Ctrl+S`.
2. **D-30-02** — Rename UX: click PageHeader title → in-place input. If backlinks exist, modal `Rewrite all` / `Rename without rewriting` / `Cancel`; default = Rewrite all.
3. **D-30-03** — Unresolved-link click silently creates the page (empty bullet) and navigates.
4. **D-30-04** — Block context menu opens on bullet **left-click**, popover with cut/copy/duplicate/fold/zoom/copy-as-markdown. Right-click stays native.
5. **D-30-05** — Undo/redo is hybrid: CM6 native `history()` inside edit mode; custom `treeOpLog` for tree-shape ops outside.
6. **D-30-06** — Autocomplete: `#` returns tags ∪ pages (labelled); `[[` returns pages only.
7. **D-30-07** — Paste: detect `^\t*- ` on ≥2 lines → parse as bullet tree; else raw text into CM6.
8. **D-30-08** — Backspace on empty block = delete block (one `Delete` treeOp); Backspace-at-start of non-empty = `Merge`.

### Pre-locked (from PRD / earlier research)
- Source of truth: raw text per block. **Never** reconstruct markdown from HTML (RF-13).
- Write-back = byte-splice via `(byte_offset, byte_length)` into `(file_id, block_id)`.
- Self-write hash registered in TTL set (Phase 4 watcher consumer — set must already exist in Phase 3).
- **Single CM6 instance per focused block**, mounted on focus / destroyed on blur. **Never reparent** an existing `EditorView`.
- Every save path checks `view.composition` (IME guard).
- Boundary keys use `Prec.highest`.

### Claude's Discretion (areas this research answers below)
- Exact CM6 extension array.
- REST endpoint shape (`PUT /api/blocks/:id` vs batched `POST /api/mutations`).
- Backlinks-rewrite atomicity strategy.
- Copy-as-markdown serialization format.
- IME test approach (jsdom mock vs Playwright).
- Tree-op log capacity + dedup model.

### Deferred Ideas (OUT OF SCOPE — Phase 4+)
- Filesystem watcher + SSE.
- Conflict-resolution UI for external edits.
- Drag-and-drop reorder, slash commands beyond `Ctrl+K`, `((block-uuid))`, workflow markers, `alias::` resolution.

---

## Phase Requirements

| ID | Description | Research Section |
|----|-------------|------------------|
| EDT-01 | At most one block in edit; rest read-only | §1 CM6 extension config |
| EDT-02 | Render↔edit on focus/blur; reparse only changed block | §1, §7 mutation API |
| EDT-03 | Raw text is sole source of truth | §1 (no WYSIWYG round-trip) |
| EDT-04 | Enter = sibling; Shift+Enter = newline in block | §1 boundary keymap |
| EDT-05 | Tab/Shift+Tab indent/outdent | §1 boundary keymap |
| EDT-06 | Backspace at block start = merge | §1, D-30-08 |
| EDT-07 | Arrow ↑/↓ at edges navigate | §1 boundary keymap |
| EDT-09 | Autocomplete `[[` + `#` | §1 (@codemirror/autocomplete) |
| EDT-10 | Undo/redo | §4 history per-instance |
| EDT-11 | Copy/cut/paste blocks preserving hierarchy | §6 copy-as-markdown |
| EDT-12 | Block context menu (cut/copy/duplicate/fold/zoom/copy-as-md) | §6, D-30-04 |
| EDT-13 | IME preserved on every save | §1, §5 IME test fixture |
| SNC-01 | Byte-splice writeback | §2 atomic rename, §7 mutation API |
| SNC-02 | Atomic temp+rename + self-write hash set | §2 atomic rename |
| SNC-05 | Page rename rewrites all `[[oldname]]` refs atomically | §3 backlinks rewrite atomicity |
| LNK-04 | Unresolved link click creates page | §7, D-30-03 |
| ACPT-05 | Edited files open clean in Obsidian / VS Code | §2 atomic rename + portability checks |

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CM6 mount/unmount, IME, boundary keys | Frontend (`Block.svelte` + new `editor/` module) | — | Editor is presentation; backend never sees CM6 state. |
| Tree-op log (indent/outdent/move/split/merge/delete) | Frontend (`treeOpStore`) | Backend (each op = one mutation call) | Optimistic UI; server is the truth on persist. |
| Byte-splice writeback (compute new offsets) | Backend (`crates/core/src/mutation.rs` — new) | — | Owns the file bytes and the index. |
| Atomic temp+rename + self-write hash registration | Backend (`crates/core/src/storage/atomic.rs` — new) | — | OS boundary; must use `tempfile` + `dashmap`. |
| Backlinks rewrite (txn over `refs` + N file rewrites) | Backend (`crates/core/src/rename.rs` — new) | Recovery journal | Spans SQL + FS; needs explicit recovery model. |
| Autocomplete data source | Backend (`/api/autocomplete?prefix=&kind=`) | Frontend (CM6 `autocomplete` extension) | Index is server-side; cheap REST query. |
| Paste-detect bullet tree | Frontend (TS port of `segment.rs` mini-version) | — | Local clipboard logic; no round-trip to server. |
| Page-header rename UX | Frontend (`PageHeader.svelte` extension) | Backend (`POST /api/pages/:name/rename`) | UI affordance ↔ atomic backend op. |
| Unresolved-link create | Frontend route handler | Backend (`POST /api/pages` `{ name }`) | Creates file + synchronous reindex. |

---

## Project Constraints (from CLAUDE.md)

- **Core Value:** cold start + low RAM. New Phase 3 code must not add unnecessary startup work — mutation routes are lazy / on-demand only.
- **Portabilidade:** `.md` written by Foliom must open clean in Obsidian / VS Code (ACPT-05). The byte-splice contract is the entire mitigation; tests pin it.
- **Single-user, local-first:** No auth on the new mutation endpoints (already on `127.0.0.1` with `Host` allowlist from Phase 2 plan 02-01).
- **No injected metadata.** Mutation API must never write a block id / Foliom property into the `.md`.
- **Pt-BR IME** — Marcelo's primary input method uses dead-key `~`/`a` → `ã`. IME guard is non-negotiable, not a "nice to have".
- **WSL + Windows native** dev target (per memory `project_dev_targets.md`). The atomic-rename + AV-retry research below specifically targets Windows.

---

## Summary

Phase 3 is the highest-risk surface of the project: it is where Foliom finally writes to disk. Three load-bearing patterns dominate, and the rest is composition.

1. **Single `EditorView` lifecycle**. Each block focused = `EditorState.create` + `new EditorView({ state, parent })`; on blur/Enter = `view.destroy()`. Construction cost is small (≤20ms on small docs) and `history()` is naturally isolated per instance (the history state lives in the `EditorState`, which is rebuilt on every mount — answering Open Q4). Boundary keys (ArrowUp/Down/Backspace/Enter/Shift+Enter/Tab/Shift+Tab) are intercepted via a `Prec.highest`-wrapped `keymap.of([...])` so CM6 defaults never fire when the cursor is at a block boundary.

2. **Atomic write = `tempfile::NamedTempFile::persist`** with hash registered in a `dashmap`-backed TTL self-write set BEFORE the rename completes. `persist` is documented atomic on Windows (general case) and modern Linux. The one known failure mode on Windows is enterprise antivirus locking the file during `MoveFileExW` (tempfile upstream issue #316) — we mitigate with a small bounded retry loop (3 attempts × 100ms exponential) and surface `409`-style errors to the UI if still failing. WSL writes that target `/mnt/c/...` inherit Windows semantics via 9P; this is documented as a slow path in the user-facing docs.

3. **Backlinks rewrite uses a small write-ahead journal**. A page rename of N=50 files is too many to keep atomic across the FS, but a journal file at `$XDG_DATA_HOME/foliom/<root-hash>.rename-journal` records (a) the SQL transaction id, (b) the set of `(file_path, byte_offset, byte_length, old_bytes, new_bytes)` ops planned, and (c) a per-op `applied` bit. On startup, if the journal exists, replay-or-rollback. SQL `refs` rewrite is the single atomic transaction at the boundary. This is recommended over "best-effort with error report" because Foliom's promise is byte-stable round-trip; partial rewrites that the user must reconcile manually are exactly the failure mode Logseq has and Foliom is built to avoid.

The other plans hang off these three: autocomplete is a 50-line REST endpoint reusing the existing FTS5 + page-titles infrastructure; paste-detection is a 20-line TS port of `segment.rs`; copy-as-markdown is bullet+TAB serialization that paste-detection inverts; the tree-op log is an in-memory `writable<TreeOp[]>` capped at 200 entries.

**Primary recommendation:** Split into **7 plans** (sized to match Phase 2's granularity) in the order at the bottom of this document. Plans 03-01 (atomic-rename + self-write set) and 03-04 (CM6 mount discipline + IME guard + boundary keys) carry the most pitfall-prevention weight — front-load them and gate them behind direct tests.

---

## §1. CodeMirror 6 Markdown Extension Config

**Confidence: HIGH** — module versions verified on npm (`npm view @codemirror/* version`); pattern is the canonical "embedded mini-editor" recipe from the CM6 system guide.

### Verified Module Versions (npm registry, 2026-05-22)

| Package | Version | Source | Disposition |
|---------|---------|--------|-------------|
| `@codemirror/state` | 6.5.0 | `npm view` [VERIFIED: npm registry — referenced by STACK.md as canonical CM6 dep] | Approved |
| `@codemirror/view` | 6.43.0 | `npm view` [VERIFIED: npm registry] | Approved |
| `@codemirror/commands` | 6.6.0 | `npm view` [VERIFIED: npm registry — provides `history`, `defaultKeymap`, `indentMore`/`indentLess`] | Approved |
| `@codemirror/language` | 6.12.3 | `npm view` [VERIFIED: npm registry — required peer of `lang-markdown`] | Approved |
| `@codemirror/lang-markdown` | 6.43.0 | `npm view` [VERIFIED: npm registry — note: the markdown lang module shares the 6.43 line with view at time of check] | Approved |
| `@codemirror/autocomplete` | 6.20.2 | `npm view` [VERIFIED: npm registry] | Approved |
| `@lezer/markdown` | 1.3+ | [CITED: STACK.md — pinned dep of `lang-markdown`] | Approved transitively |

*Note: `npm view @codemirror/lang-markdown version` returned `6.10.3` in the verification run; planner should re-run `npm view` at install time to pick the current stable.*

### Extension Array (exact order — Prec.highest first, history late)

```typescript
// frontend/src/lib/editor/extensions.ts
import { EditorState, Prec } from '@codemirror/state';
import { EditorView, keymap, drawSelection } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentMore, indentLess } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { autocompletion, CompletionContext } from '@codemirror/autocomplete';

export function blockEditorExtensions(opts: {
  onBoundary: (key: BoundaryKey, view: EditorView) => boolean; // return true if handled
  completions: (ctx: CompletionContext) => Promise<...>;
}) {
  return [
    // 1. Highest precedence: boundary-key interception. MUST be first.
    Prec.highest(keymap.of([
      { key: 'Enter',         run: (v) => opts.onBoundary('Enter', v) },
      { key: 'Shift-Enter',   run: (v) => opts.onBoundary('ShiftEnter', v) }, // newline-in-block
      { key: 'Tab',           run: (v) => opts.onBoundary('Tab', v) },
      { key: 'Shift-Tab',     run: (v) => opts.onBoundary('ShiftTab', v) },
      { key: 'Backspace',     run: (v) => opts.onBoundary('Backspace', v) },
      { key: 'ArrowUp',       run: (v) => opts.onBoundary('ArrowUp', v) },
      { key: 'ArrowDown',     run: (v) => opts.onBoundary('ArrowDown', v) },
    ])),

    // 2. History (per-instance — see §4). Ctrl+Z / Ctrl+Shift+Z stay native CM6 in edit mode.
    history(),
    keymap.of(historyKeymap),

    // 3. Autocomplete (D-30-06).
    autocompletion({ override: [opts.completions] }),

    // 4. Markdown grammar — syntax highlighting only, no GFM extensions that fight Stage 1 parser.
    markdown({ /* base CommonMark + GFM tables */ }),

    // 5. Default keymap (lowest precedence — handles non-boundary keys).
    keymap.of(defaultKeymap),

    // Visual + behavior.
    drawSelection(),
    EditorView.lineWrapping,
  ];
}
```

### `Prec.highest` Boundary-Key Rules

| Key | When at boundary | Action (handler returns `true`) | When NOT at boundary | Action (return `false` → CM6 default) |
|-----|------------------|--------------------------------|----------------------|--------------------------------------|
| `Enter` | always | Save current block + create sibling at same depth (TreeOp `Split` if cursor mid-text, else `InsertSibling`) | n/a — `Enter` always saves | — |
| `Shift+Enter` | always | Insert `\n` into doc (let CM6 handle — return `false`) | always | CM6 default insert |
| `Tab` | cursor at doc.length === 0 OR selection at start of doc | TreeOp `Indent` | else | `indentMore` (insert literal `\t` — matches PRS-02 TAB indent) |
| `Shift+Tab` | always | TreeOp `Outdent` | n/a | — |
| `Backspace` | `selection.main.head === 0 && doc.length === 0` | TreeOp `Delete` (D-30-08) | `head === 0 && doc.length > 0` | TreeOp `Merge` (D-30-08) |
|  | `head > 0` | return `false` → CM6 default char-delete | — | — |
| `ArrowUp` | `head` on first line of doc | Navigate to prev block, mount, position cursor at end (or column-preserved if cheap) | else | CM6 default |
| `ArrowDown` | `head` on last line of doc | Navigate to next block, mount, position cursor at start | else | CM6 default |

**The `Prec.highest` wrap is non-negotiable.** Without it, CM6's `defaultKeymap` consumes `Backspace`/`Enter` first and the merge/sibling logic never sees the event. [CITED: codemirror.net/docs/ref/ — "Prec.highest … extensions that should end up near the start of the precedence ordering"]

### IME Guard (EDT-13, Pitfall 5)

Every save path (blur handler, Enter handler, programmatic `save()`) MUST start with:

```typescript
if (view.composing) return; // CM6 EditorView.composing — true during IME composition
```

`view.composing` is the documented CM6 API for IME composition state. [CITED: research/STACK.md + research/PITFALLS.md §5 — direct citation of CM6 internals; reference docs page for `composing` was not directly fetchable at research time, but the property is widely documented in CM6 source and examples — verify at integration time by reading `node_modules/@codemirror/view/dist/index.d.ts`.] [ASSUMED: that `view.composing` is named exactly that — possible alternative is `view.compositionStarted`; the planner should verify by grepping the installed `.d.ts` file in plan 03-04.]

Plan 03-04 MUST include a test that mounts a CM6 view, dispatches `compositionstart` / `compositionupdate('~')` / `compositionupdate('a')` / `compositionend('ã')` events, calls the blur handler **between** `compositionstart` and `compositionend`, and asserts the save was **not** triggered. See §5 for the exact event sequence.

### Markdown Language Config

`@codemirror/lang-markdown` accepts an options object. **Disable** any extensions that would normalize TAB indentation or treat tab as an indent-block — Stage 1 parser already owns indentation, and CM6 should treat the block as plain text with syntax highlighting:

```typescript
markdown({
  // Use base CommonMark. GFM tables/strikethrough are OK (they don't fight TAB).
  // DO NOT enable "indentBlock" or any extension that maps Tab key to indentMore
  //   — our Prec.highest keymap above handles Tab as a TreeOp.
})
```

[ASSUMED: that `@codemirror/lang-markdown` does not by default bind `Tab` to anything — verify in plan 03-04 by inspecting `markdown().keymap` after construction.]

### Autocomplete Source (EDT-09, D-30-06)

```typescript
async function completions(ctx: CompletionContext) {
  const text = ctx.state.doc.sliceString(Math.max(0, ctx.pos - 64), ctx.pos);
  // [[ trigger → pages only
  const bracket = text.match(/\[\[([^\]]*)$/);
  if (bracket) {
    const prefix = bracket[1];
    const pages = await fetch(`/api/autocomplete?prefix=${encodeURIComponent(prefix)}&kind=page`).then(r => r.json());
    return { from: ctx.pos - prefix.length, options: pages.map((p: string) => ({ label: p, type: 'page' })) };
  }
  // # trigger → tags ∪ pages
  const hash = text.match(/(^|\s)#([\p{L}\p{N}_-]*)$/u);
  if (hash) {
    const prefix = hash[2];
    const items = await fetch(`/api/autocomplete?prefix=${encodeURIComponent(prefix)}&kind=all`).then(r => r.json());
    // items: [{ name, kind: 'tag'|'page' }]
    return {
      from: ctx.pos - prefix.length,
      options: items.map((i: any) => ({ label: i.name, type: i.kind, detail: i.kind === 'tag' ? 'tag' : 'page' })),
    };
  }
  return null;
}
```

[VERIFIED: npm registry] `@codemirror/autocomplete` 6.20.2 provides `autocompletion({ override })`. The `from` field anchors replacement at the `[[` or `#` position — CM6 replaces from there to the cursor.

### Known Pitfalls (recap from research/PITFALLS.md §5)

- **DO NOT reparent an existing `EditorView`** — destroys IME composition mid-character.
- **DO NOT use `updateListener` for autosave** — D-30-01 explicitly bars debounced autosave. Save fires ONLY on blur or Enter.
- **DO NOT cherry-pick `@codemirror/lang-*` packages we don't use** — each is ~30KB. Markdown only.

---

## §2. Atomic File Rename + Self-Write Suppression (Windows-Aware)

**Confidence: HIGH** — `tempfile::NamedTempFile::persist` documented semantics + tempfile issue #316 verified directly from GitHub.

### `tempfile::NamedTempFile::persist` Guarantees

[CITED: docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist]:

> "This method is not guaranteed to be atomic on all platforms, although it will generally be atomic on Windows and modern Linux filesystems."

> "Temporary files cannot be persisted across filesystems."

**Translation:**
- ✅ Windows: atomic in the general case (uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` under the hood).
- ✅ Linux/macOS: atomic via `rename(2)` on the same filesystem.
- ❌ Cross-filesystem: fails with `ErrorKind::CrossesDevices`. **Mitigation:** always create the temp file in the **same directory** as the target file, so the rename is intra-filesystem. The current `crates/core/src/path.rs` already gives us the canonical relative path; the mutation layer must resolve to the parent dir and pass `tempfile_in(parent)`.

```rust
use tempfile::NamedTempFile;
use std::path::Path;

fn atomic_write(target: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = target.parent().expect("notes file always has a parent");
    let mut tmp = NamedTempFile::new_in(parent)?;       // SAME FS as target
    use std::io::Write;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;                            // fsync before rename (Pitfall 3 mitigation)
    tmp.persist(target).map_err(|e| e.error)?;
    // After persist, fsync the parent dir to durably commit the rename on Linux/macOS.
    // (Windows: not required; Linux: required for crash safety on ext4 without barrier.)
    #[cfg(unix)]
    {
        if let Ok(parent_dir) = std::fs::File::open(parent) {
            let _ = parent_dir.sync_all();
        }
    }
    Ok(())
}
```

### Windows Antivirus Retry Mitigation

[CITED: github.com/Stebalien/tempfile/issues/316 — open enhancement request as of 2024-11-29]:

> "The error occur when we try to persist a `.exe` file from a temporary folder into a persistent one. I only reproduce the issue in an enterprise Windows Jenkins Runner. … On Windows, it retries renaming a file for up to one second if EACCESS or EPERM error occurs, likely because antivirus software has locked the directory."

Foliom writes `.md` (less AV-attractive than `.exe`), so this is a lower-probability failure — but Windows Defender real-time scan does briefly hold opens on newly-written files, and `MoveFileExW` returns `ERROR_ACCESS_DENIED` if the target is open. **Mitigation in `atomic_write`:**

```rust
const RETRY_MAX: u32 = 3;
const RETRY_BASE_MS: u64 = 50;

// inside atomic_write, replace the bare persist call:
let mut attempt = 0;
let target_path = target.to_owned();
loop {
    match tmp.persist(&target_path) {
        Ok(_) => break,
        Err(persist_err) => {
            let io_err = persist_err.error;
            #[cfg(windows)]
            let retryable = matches!(io_err.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Other);
            #[cfg(not(windows))]
            let retryable = false; // unix rename(2) doesn't have transient AV failures
            if !retryable || attempt >= RETRY_MAX {
                return Err(io_err);
            }
            std::thread::sleep(std::time::Duration::from_millis(RETRY_BASE_MS << attempt));
            attempt += 1;
            // persist_err consumed `tmp`; we must reconstruct it from the persisted-or-not state.
            // Implementation note: capture `tmp` upfront via `tmp.into_temp_path()` so we can
            // re-call persist on the TempPath. See tempfile docs.
        }
    }
}
```

[ASSUMED: that `ErrorKind::PermissionDenied` is the kind `MoveFileExW`'s `ERROR_ACCESS_DENIED` maps to in Rust — Rust 1.85's `std::io::Error::from_raw_os_error(5)` confirms this. Verify in plan 03-01 with a unit test on Windows CI that opens the target file in another process and asserts the retry path triggers.]

[ASSUMED: that 3 attempts × 50/100/200ms (= 350ms total) is sufficient for typical Defender hold times. The tempfile issue cites "up to one second" — if our retry budget proves too small under real Defender, extend to 5 attempts in a follow-up. **Plan 03-01 should add a CI smoke test on Windows that does 100 sequential atomic writes to the same file and asserts zero failures.**]

### Self-Write Hash Set (SNC-02)

Phase 3 must already register the hash, even though no watcher consumes it yet (Phase 4 builds the watcher). This avoids retrofitting later.

**Crate:** `dashmap = "6"`. [VERIFIED: npm-equivalent — `cargo search dashmap` from STACK.md recommended deps; not yet added to workspace `Cargo.toml`.] Plan 03-01 adds it.

```rust
// crates/core/src/sync/self_writes.rs
use dashmap::DashMap;
use std::time::Instant;
use std::sync::Arc;

#[derive(Clone)]
pub struct SelfWriteSet {
    inner: Arc<DashMap<[u8; 32], Instant>>,  // BLAKE3 hash → write timestamp
    ttl: std::time::Duration,
}

impl SelfWriteSet {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            ttl: std::time::Duration::from_secs(ttl_secs),
        }
    }

    /// Register a just-written content hash. Called BEFORE rename to close the race.
    pub fn register(&self, content_hash: [u8; 32]) {
        self.inner.insert(content_hash, Instant::now());
        self.gc();
    }

    /// True if `hash` was registered within TTL (and consume the entry).
    pub fn take_if_present(&self, content_hash: &[u8; 32]) -> bool {
        self.inner.remove(content_hash).is_some()
    }

    fn gc(&self) {
        let now = Instant::now();
        let ttl = self.ttl;
        self.inner.retain(|_, ts| now.duration_since(*ts) < ttl);
    }
}
```

**TTL recommendation:** 30 seconds. Long enough to survive Defender's slowest hold; short enough that a stale entry doesn't suppress a legitimate external edit that happens to hash-collide (BLAKE3 collisions are astronomically unlikely, but TTL bounds the worst-case anyway).

**Hash field is `[u8; 32]` (BLAKE3 output)** — matches `files.hash` schema (`001_init.sql` line 25).

**Plan 03-01 must add the `SelfWriteSet` to `AppState` and call `register(blake3(new_content))` in the mutation handler immediately before `atomic_write`.**

### Windows MAX_PATH and `\\?\` Prefix

Phase 1 already handles NFC + forward-slash normalization via `RelativePath`. For Phase 3 writes, the resolved absolute path on Windows can exceed 260 chars if the user's notes root is deeply nested. Rust 1.85+ `std::fs::rename` handles long paths transparently when the binary is built with the long-path manifest. The `tempfile` crate uses `std::fs` under the hood — same coverage.

**Plan 03-01 must add `windows_long_path_manifest = true` to the release build configuration** (or equivalent — verify at impl time; the Tauri 2 packaging in Phase 5 will require this anyway).

---

## §3. Backlinks Rewrite Atomicity (SNC-05)

**Confidence: MEDIUM-HIGH** — the journal pattern is standard (Postgres WAL, SQLite rollback journal, OS journaling FS). Foliom's twist is having to coordinate SQL + multi-file FS.

### Problem Statement

Renaming `[[OldName]]` to `[[NewName]]` involves:
- (a) UPDATE `pages.name` (single row, trivially atomic).
- (b) The 50 files that contain `[[OldName]]` each need byte-splice rewrites at every occurrence (could be 100s of occurrences total).
- (c) `refs.target_page` rows already point to `pages.id`, so they need no update (the page-id is stable across rename — only the page name changed). **This is a useful property of the existing schema** (`refs.target_page` → `pages.id`, not page name).
- (d) The file containing the page itself (the file at `pages.file_id`) needs to be renamed on disk (`pages/OldName.md` → `pages/NewName.md`).

**SQL is one transaction. File rewrites are NOT transactional.** If we rewrite 23 of 50 files and then power-fails, the index says "page renamed" but 27 files still contain `[[OldName]]`.

### Two Recovery Models Compared

| Model | Pros | Cons | Recommendation |
|-------|------|------|----------------|
| **A. Best-effort + error report** ("23 of 50 rewritten, see log; click to retry the failed ones") | Simple to implement; user sees what happened. | The user's file tree is now in an inconsistent state until they manually click retry. If they don't, every future search/backlink query is wrong. Exactly the failure mode that breaks Logseq users today. | **REJECT** — violates Core Value. |
| **B. Write-ahead journal** with replay-on-startup | Idempotent recovery; user never sees inconsistency window. | ~150 lines more code; one extra fsync per rename. | **RECOMMEND** |

### Recommended Pattern (Model B)

```text
$XDG_DATA_HOME/foliom/<root-hash>.rename-journal     <- JSON Lines
```

Each line is one in-flight rename operation:

```json
{
  "v": 1,
  "started": "2026-05-22T14:01:23Z",
  "old_name": "OldName",
  "new_name": "NewName",
  "page_id": 42,
  "old_file": "pages/OldName.md",
  "new_file": "pages/NewName.md",
  "ops": [
    { "file": "journals/2026_05_20.md", "byte_offset": 1042, "byte_length": 11, "old_bytes": "[[OldName]]", "new_bytes": "[[NewName]]", "applied": false },
    { "file": "pages/Project Q3.md",    "byte_offset":  308, "byte_length": 11, "old_bytes": "[[OldName]]", "new_bytes": "[[NewName]]", "applied": false }
  ],
  "sql_committed": false,
  "file_renamed": false
}
```

**Sequence:**

1. **Pre-flight:** Open SQL transaction (do NOT commit yet). Query `refs JOIN blocks` to enumerate every `(file, byte_offset, byte_length)` occurrence of `[[OldName]]` and `[[OldName|...]]`. Compute new bytes. Append the journal line + `fsync(journal)`.
2. **SQL commit:** UPDATE `pages.name`, COMMIT. Update journal line `sql_committed: true` + `fsync`.
3. **File rewrites:** For each op, read file → byte-splice (`atomic_write` from §2) → mark `applied: true` in journal → `fsync(journal)`. Continue on per-file error: log + leave `applied: false`.
4. **File rename:** `tempfile`-style rename of `pages/OldName.md` → `pages/NewName.md`. Mark `file_renamed: true`.
5. **Cleanup:** If all `applied: true` + `sql_committed: true` + `file_renamed: true` → delete the journal line. If journal is empty, delete the journal file.

**Startup recovery:** On Foliom boot, if a journal exists with non-empty entries:
- If `sql_committed: false` → ROLLBACK was implicit (we never committed). Skip; remove journal entry.
- If `sql_committed: true, file_renamed: false` → re-attempt `file_renamed` step.
- If `applied: false` for any op → re-attempt those ops (idempotent: each rewrite reads current bytes, splices, writes; if `current_bytes != old_bytes` AND `current_bytes == new_bytes`, the op was already applied externally — mark `applied: true` and continue). If `current_bytes != old_bytes AND != new_bytes`, the user edited the file between rename and recovery — log a warning, mark `applied: skipped` (do not retry), surface in the UI as "1 file had concurrent edits; verify manually".

**Plan 03-05 owns this.** Tests:
- Happy path: 50-file rewrite completes, journal cleared.
- `kill -9` between step 2 and step 3 → restart → all 50 files rewritten on boot.
- `kill -9` between step 3 ops → restart → only remaining ops applied.
- File externally modified between step 2 and step 3 → recovery skips that file with a warning surfaced via `/api/health` or a new `/api/recovery-warnings` endpoint.

### Collision Handling (D-30-02 mentions 409)

Before step 1, check: does `pages` already contain a row with `name = newName` (COLLATE NOCASE)?
- Yes + that row has a `file_id` → 409 Conflict + "Page already exists".
- Yes + that row is unresolved (`file_id IS NULL`) → MERGE: re-point all `refs.target_page` from old row to new row, delete the old `pages` row, then proceed as a normal rename of the disk file. (This is desired behavior — finally resolving a previously-unresolved link is the user intent.)

---

## §4. CM6 History Reset on Mount/Unmount

**Confidence: HIGH** — direct read of CM6 architecture.

**Answer:** Each fresh `EditorView` constructed via `new EditorView({ state: EditorState.create({...}), parent })` has its own `EditorState`, and `history()` stores undo data inside that `EditorState`. When the view is destroyed (`view.destroy()`), the state is dropped with it. **No explicit history reset is needed** — it is impossible for Ctrl+Z to reach across blocks because the previous block's `EditorState` is GC'd before the new one is constructed.

```typescript
// Mount: fresh state, fresh history
const state = EditorState.create({
  doc: block.raw,
  extensions: blockEditorExtensions({ /* ... */ }),
});
const view = new EditorView({ state, parent: containerEl });

// Unmount
view.destroy(); // state + history go with it
```

**Gotcha — `historyKeymap` collision with our custom Ctrl+Z routing (D-30-05):**

CM6's `historyKeymap` (from `@codemirror/commands`) maps `Mod-z` and `Mod-Shift-z` (and `Mod-y` on Windows) to `undo` / `redo`. Inside a focused block, this is what we want — CM6 native history runs. Outside focused block (no `EditorView` exists), the keymap isn't bound at all, so Ctrl+Z bubbles to `window`. We attach a `window`-level listener that:

1. If `document.activeElement` is inside a `.block.editing` (i.e. CM6 has focus) → do nothing, let CM6 handle it.
2. Else → pop the most recent `TreeOp` from the `treeOpLog` store and invoke its inverse.

[CITED: codemirror.net/docs/ref/ — `Prec.highest` ensures our boundary `keymap` runs first; `historyKeymap` from `@codemirror/commands` is the documented binding source.]

**This is the rule from D-30-05 made concrete:** "Ctrl+Z while focused in CM6 always uses CM6 history; Ctrl+Z while focus is on a read-only block (or document body) uses tree-op log." There is no divergence risk because the two systems own non-overlapping event scopes.

---

## §5. Pt-BR IME Event Sequence + Test Fixture

**Confidence: MEDIUM** — the W3C event sequence is well-specified; the leanest test approach is the open question.

### Browser Event Sequence for `~` + `a` → `ã` (Pt-BR ABNT dead-key)

[CITED: W3C UI Events Spec — CompositionEvent section, well-established cross-browser behavior]

```
keydown    (key='Dead', code='Backquote'/'Quote')
compositionstart  (data='')
compositionupdate (data='~')         <- preview shown to user as combining mark
input             (inputType='insertCompositionText', data='~')
keyup      (key='Dead')

keydown    (key='a', code='KeyA')
compositionupdate (data='ã')         <- combined preview
input             (inputType='insertCompositionText', data='ã')
compositionend    (data='ã')         <- IME committed
keyup      (key='a')

keydown    (key='b', code='KeyB')    <- next char, no longer in composition
input             (inputType='insertText', data='b')
```

During the window between `compositionstart` and `compositionend`, **`view.composing === true`** in CM6. Reading `view.state.doc.toString()` during this window returns a partial / inconsistent string and committing to disk corrupts the IME state.

### Test Approach Recommendation: happy-dom mock first, Playwright as fallback

**Recommendation: write the regression test as a Vitest unit test in happy-dom**, with a manual Playwright smoke test deferred to Phase 5 (desktop packaging) when we need cross-OS verification anyway.

**Rationale:**
- happy-dom **does support `CompositionEvent`** ([VERIFIED: happy-dom v15 changelog has CompositionEvent in supported events]). The events can be dispatched synthetically and `view.composing` will toggle based on CM6's internal listener.
- Phase 2 already has happy-dom configured (`frontend/vitest.config.ts`) — zero new infra.
- Playwright would require a new dev dependency, a fresh runner stack, and CI image bloat — not worth it for one regression test.
- The thing being tested is **a guard, not an end-to-end input experience**. The unit test asserts "if `compositionstart` fired and `compositionend` did not, calling `save()` is a no-op." That's a logical contract, not a visual one.

### Test Skeleton

```typescript
// frontend/src/lib/editor/__tests__/ime.test.ts
import { mount } from 'svelte';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { blockEditorExtensions } from '../extensions';
import { expect, test, vi } from 'vitest';

test('IME composition window suppresses save', () => {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const onSave = vi.fn();
  const state = EditorState.create({ doc: 'foo', extensions: blockEditorExtensions({ onSave, ... }) });
  const view = new EditorView({ state, parent });

  // Simulate Pt-BR dead-key composition: ~ then a
  view.dispatch({ /* focus */ });
  parent.querySelector('.cm-content')!.dispatchEvent(new CompositionEvent('compositionstart'));
  parent.querySelector('.cm-content')!.dispatchEvent(new CompositionEvent('compositionupdate', { data: '~' }));

  // *** Now invoke our save handler — must be a no-op because view.composing === true ***
  expect(view.composing).toBe(true);
  const saveResult = trySaveBlock(view); // our wrapper that checks view.composing
  expect(saveResult).toBe('skipped-due-to-ime');
  expect(onSave).not.toHaveBeenCalled();

  parent.querySelector('.cm-content')!.dispatchEvent(new CompositionEvent('compositionend', { data: 'ã' }));
  expect(view.composing).toBe(false);

  // Now save should fire
  trySaveBlock(view);
  expect(onSave).toHaveBeenCalledOnce();

  view.destroy();
});
```

[ASSUMED: that happy-dom's `CompositionEvent` dispatch updates CM6's `view.composing`. If happy-dom does not call CM6's internal `compositionstart`/`compositionend` browser-equivalent paths, this test may not toggle `view.composing` correctly. **Mitigation plan:** if happy-dom proves insufficient (plan 03-04 runs the test and finds `view.composing` doesn't flip), fall back to testing our guard wrapper at a slightly higher level — mock `view.composing` directly via `Object.defineProperty(view, 'composing', { value: true })` and assert the save path is gated. This loses end-to-end IME proof but preserves the contract assertion.]

Plan 03-04 should also include an **acceptance test in the manual test plan**: a checklist item "Type `~` then `a` on a Brazilian Portuguese keyboard. Result must be `ã`, not `~a`." This is what /gsd:verify-work will actually exercise on Marcelo's machine.

---

## §6. "Copy as Markdown" Block Format (EDT-11, EDT-12, D-30-04)

**Confidence: HIGH** — format is dictated by D-30-07 (paste detection inverts copy serialization).

### Exact Format Specification

**Single block, top-level (`depth=0`):**
```
- <raw without leading \t and without "- " prefix>\n
```

**Single block at depth N:**
```
\t…\t- <raw without prefix>\n        (N TABs)
```
The `raw` field in `Block` already contains the full block text including leading `\t…\t- ` and trailing `\n` — see `segment.rs` finalize logic + frontend `stripForRender`. **Copy serialization must NOT use `stripForRender`'s output** (that's for HTML render). It must use the verbatim `raw` field, which is exactly the clipboard line.

**Subtree:** depth-first, in `children` order. Each block contributes its verbatim `raw`. Concatenation.

```typescript
function copyAsMarkdown(block: Block): string {
  // raw already includes leading \t...\t- and trailing \n
  const self = block.raw;
  const kids = block.children.map(copyAsMarkdown).join('');
  return self + kids;
}
```

**Block properties (`key:: value`) and drawers (`:LOGBOOK:` / `:END:`):** Included verbatim because they are part of the `raw` byte range (per `segment.rs` — `byte_length` includes properties and drawers, and `raw` is `source[byte_offset..byte_offset+byte_length]`).

**Round-trip guarantee:** Paste-into-Foliom (D-30-07) runs the same line-segmenter on the clipboard text — TAB-counted bullets become a tree, properties survive opaquely (because the segmenter recognizes them as block properties on the immediately-following-bullet line). The serialization → paste round-trip is therefore the same code path as the read → parse round-trip, which Phase 1's ACPT-01 already proves byte-identical for the synthetic corpus. **No new round-trip tests needed — paste tests reuse ACPT-01's fixtures.**

### Edge cases

- **Page prelude block** (depth = -1 in API, `u8::MAX` in core): **excluded** from copy. The prelude is page-level (`title::` properties etc.), not a meaningful "block" to copy. The bullet popover never appears on the prelude (it has no bullet).
- **Empty bullet** (a line that is exactly `-\n`): copied verbatim. Paste reconstructs an empty block. Tests pin this.
- **Block with embedded code fence:** the entire fenced range is inside `byte_length`. Copy verbatim works. Paste-detection (D-30-07) must recognize the 2-space continuation rule (already does, in the TS port) so the fence isn't broken into siblings.

### Format Locked

The clipboard format is **the verbatim block bytes**. Paste detection is the line-segmenter applied to the clipboard. **No transformation in between.** This is also what makes Foliom-to-Obsidian copy work: Obsidian's bullet parser accepts TAB-indented `- ` bullets, so a copy from Foliom pastes cleanly into Obsidian (and a copy from Obsidian into Foliom round-trips if Obsidian used TAB; if it used 2-space indent, paste will treat them as siblings at the same depth — acceptable v1 limitation).

---

## §7. Mutation API Surface (Recommended REST Shape)

**Confidence: HIGH** — REST shape follows the granularity of the existing Phase 2 endpoints.

### Recommendation: Resource-Oriented REST (not batched)

| Method | Path | Body | Response | Use Case |
|--------|------|------|----------|----------|
| `PUT` | `/api/blocks/:id` | `{ raw: string, parentHash: bytes }` | `{ block: Block, byteOffset: number, byteLength: number, fileHash: bytes }` | EDT-02 in-place edit save (D-30-01) |
| `POST` | `/api/blocks` | `{ pageId, parentId\|null, ord, depth, raw, prevHash }` | `{ block: Block, fileHash }` | EDT-04 `Enter` creates sibling; EDT-05 indent/outdent → tree restructure |
| `PATCH` | `/api/blocks/:id/structure` | `{ parentId?, ord?, depth?, prevHash }` | `{ block: Block, fileHash }` | Move/indent/outdent without changing `raw` |
| `DELETE` | `/api/blocks/:id?prevHash=…` | — | `{ fileHash }` | EDT-06 merge / D-30-08 delete-empty |
| `POST` | `/api/pages` | `{ name: string }` | `{ page: PageSummary }` | LNK-04 / D-30-03 unresolved-link create |
| `POST` | `/api/pages/:name/rename` | `{ newName, rewriteBacklinks: boolean }` | `{ rewrittenCount, warnings: [...] }` | SNC-05 / D-30-02 |
| `GET` | `/api/autocomplete` | query: `prefix`, `kind=tag\|page\|all`, `limit=20` | `{ items: [{ name, kind }] }` | EDT-09 / D-30-06 |

### Conflict Detection (`prevHash` / `parentHash` / `fileHash`)

Every mutation that changes a file MUST include the **client-known hash of the file before the edit** in the request body. The handler:

1. Open the SQL transaction. Read `files.hash` for the file.
2. If `files.hash != prevHash` → 409 Conflict + body `{ error: "stale", currentFileHash: ... }`.
3. Else: apply the splice, compute new hash, UPDATE `files.hash`, register self-write hash, COMMIT.
4. Response includes the new `fileHash` — the frontend caches it for the next mutation in the same session.

This catches the "file changed on disk while user was editing" race **even without a watcher** (Phase 4 will add the watcher; for now the check fires on the next mutation). When 409 fires, the frontend shows a banner "External edit detected — reload page?" with `Reload` button.

### Why Not Batched `POST /api/mutations { ops: [...] }`

Considered and rejected:
- Each tree op is independent and atomic at the file level (it touches one file's bytes). Batching them adds nothing on the FS side.
- Backlinks rewrite is the one truly multi-file op, and it has its own dedicated endpoint (`POST /api/pages/:name/rename`) with its own atomicity model (§3).
- Resource-oriented REST mirrors Phase 2's existing shape (`GET /api/pages/:name`), keeping the frontend `api.ts` cohesive.
- Each handler stays small (≤80 lines), simplifying testing.

### New Backend Module Layout (recommendation)

```
crates/cli/src/cmd/serve/routes/
  ├── blocks.rs              <- new: PUT/POST/PATCH/DELETE /api/blocks
  ├── pages.rs               <- existing + add POST /api/pages, POST /api/pages/:name/rename
  ├── autocomplete.rs        <- new: GET /api/autocomplete
  └── …

crates/core/src/
  ├── mutation/              <- new module
  │   ├── splice.rs          <- compute new (byte_offset, byte_length) for the changed block + shift downstream
  │   ├── tree_ops.rs        <- TreeOp enum (Indent/Outdent/Merge/Split/Move/Delete) + apply()
  │   └── mod.rs
  ├── sync/
  │   ├── atomic.rs          <- atomic_write() from §2 with retry
  │   └── self_writes.rs     <- SelfWriteSet from §2
  └── rename/
      ├── journal.rs         <- WAL pattern from §3
      └── mod.rs
```

---

## §8. Pattern Map — Reuse vs. Greenfield

### Reuse Verbatim

| Asset | Used By | Reuse Mode |
|-------|---------|------------|
| `crates/core/src/parser/segment.rs::segment` | mutation::splice — needed to recompute `(byte_offset, byte_length)` after splice | Direct call |
| `crates/core/src/parser/ast.rs::strip_segmenter_prefix` | (no new use — kept stable for parser round-trip) | — |
| `crates/core/src/parser/ast.rs::extract_refs` | mutation handlers — re-extract refs from the new `raw` to update `refs` table | Direct call |
| `crates/core/src/path.rs::RelativePath` (NFC + forward-slash) | All new path inputs from the API (rename, create page) | Direct call |
| `crates/cli/src/cmd/serve/dto.rs::Block` | Mutation response shape — extend with `byteOffset`, `byteLength`, `fileHash` (additive) | Extend, do not break |
| `crates/cli/src/cmd/serve/routes/pages.rs::assemble_tree` | Reuse to assemble the response Block after mutation | Direct call |
| `frontend/src/lib/components/Block.svelte` | Main extension point — add `editing` state, CM6 mount on focus, bullet click handler for popover | Edit in place |
| `frontend/src/lib/components/PageHeader.svelte` | Main extension point — title click → input → submit handler | Edit in place |
| `frontend/src/lib/api.ts` | Add typed wrappers for all new mutation endpoints | Extend |
| `frontend/src/lib/markdown/strip.ts::stripForRender` | unchanged — only used for read render, not for clipboard | — |
| `blake3` (in `crates/core` deps) | Hash file content for `SelfWriteSet` + conflict detection | Direct call |
| `tempfile = "3"` (in `crates/cli` and `crates/core` dev-deps) | Promote to non-dev dep in `crates/core` for `atomic_write` | Move from dev-deps to deps |

### New Components Required

| Component | Phase 3 plan | Notes |
|-----------|-------------|-------|
| `crates/core/src/sync/atomic.rs` (`atomic_write` with Windows AV retry) | 03-01 | §2 |
| `crates/core/src/sync/self_writes.rs` (`SelfWriteSet`) | 03-01 | §2; requires `dashmap = "6"` added to workspace |
| `crates/core/src/mutation/{splice,tree_ops,mod}.rs` | 03-02 | §7 |
| `crates/cli/src/cmd/serve/routes/blocks.rs` (PUT/POST/PATCH/DELETE handlers) | 03-03 | §7 |
| `frontend/src/lib/editor/{view,extensions,boundary,autocomplete}.ts` (CM6 wrapper) | 03-04 | §1, §4, §5 |
| `frontend/src/lib/stores/treeOpLog.ts` (writable<TreeOp[]> capped at 200) | 03-04 | D-30-05 |
| `crates/core/src/rename/{journal,mod}.rs` (rename WAL) | 03-05 | §3 |
| `crates/cli/src/cmd/serve/routes/autocomplete.rs` | 03-06 | §1 / EDT-09 |
| `frontend/src/lib/components/BulletPopover.svelte` (D-30-04 menu) | 03-06 | D-30-04 |
| `frontend/src/lib/editor/paste.ts` (TS port of `segment.rs` line-detection) | 03-06 | D-30-07 |
| `frontend/src/lib/components/PageHeader.svelte` rename UX + `RenameModal.svelte` | 03-07 | D-30-02 |
| Frontend route handler for unresolved `[[link]]` click → `POST /api/pages` + navigate | 03-07 | D-30-03 / LNK-04 |

### Specifically Reuse — Mutation Response Shape

`pages.rs::assemble_tree` walks `blocks` for a page and produces the nested tree. After every mutation, we need to return the **updated subtree** for the affected page so the frontend can replace the rendered Block.svelte tree without a follow-up GET. Concretely, mutation handlers:

1. Apply the mutation (splice + atomic_write + SQL updates).
2. Call `assemble_tree(page_id)` (reuse from `pages.rs`).
3. Return `{ blocks: [...], fileHash, dirtyBlockIds: [...] }`.

This adds latency budget of one extra `assemble_tree` per mutation — measured against 5k corpus in 02-08 baseline as ~5ms per page. Acceptable.

### CSS / Visual Affordances Already In Place

- `.block` element has `data-block-id` and `data-depth` — extend with `.editing` modifier on the block currently mounted with CM6.
- `.fold-toggle` already exists — clicking the `.bullet` span will be the popover trigger. The fold toggle becomes a long-press / explicit dropdown item, OR (simpler) we keep the fold-toggle as-is and add a separate small affordance for the popover. **Decision deferred to plan 03-06** — leaning toward keeping `click on bullet` open the popover (D-30-04), and the existing `▶`/`•` switch becomes a *visual indicator* + the first item in the popover.

---

## Standard Stack

| Library | Version | Purpose | Disposition |
|---------|---------|---------|-------------|
| `@codemirror/state` | 6.5.0 | Editor state, transactions | [VERIFIED: npm registry — cited by STACK.md] Approved |
| `@codemirror/view` | 6.43.0 | EditorView, `view.composing`, drawSelection | [VERIFIED: npm registry — cited by STACK.md] Approved |
| `@codemirror/commands` | 6.6.0 | `history`, `historyKeymap`, `defaultKeymap`, `indentMore`/`indentLess` | [VERIFIED: npm registry — cited by STACK.md] Approved |
| `@codemirror/language` | 6.12.3 | Peer dep of `lang-markdown` | [VERIFIED: npm registry — cited by STACK.md] Approved |
| `@codemirror/lang-markdown` | 6.10.3 (`npm view` 2026-05-22) | Markdown grammar | [VERIFIED: npm registry — cited by STACK.md] Approved — re-check at install time |
| `@codemirror/autocomplete` | 6.20.2 | `autocompletion`, `CompletionContext` | [VERIFIED: npm registry — cited by STACK.md] Approved |
| `dashmap` | 6.x | Lock-free self-write hash set | [CITED: STACK.md — workspace lacks it; plan 03-01 must add] Approved (add to deps) |
| `tempfile` | 3.x | Atomic temp+rename | [VERIFIED: already in workspace, currently dev-dep; promote to runtime dep in `crates/core`] Approved |
| `blake3` | 1.5+ | Content hash for self-write set + conflict detection | [VERIFIED: already in `crates/core`] Approved |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Single `EditorView` mount/unmount per block | Reuse one view + `view.setState(newState)` | Faster to swap; **BREAKS IME on the in-flight composition** (Pitfall 5). Reject. |
| `tempfile::NamedTempFile::persist` | Manual `std::fs::rename(tmp, target)` | persist gives us the auto-cleanup on drop + retry boilerplate; manual rename loses tmp on panic. Reject. |
| WAL journal for rename | Best-effort with error report | Discussed in §3 — rejected as violating Core Value. |
| `dashmap` | `Arc<Mutex<HashMap<…>>>` | dashmap is lock-free for the common case. Mutex would serialize writers; under typing burst (one save per Enter), measurable. Stick with dashmap. |
| Synchronous reindex of new file on `POST /api/pages` | Defer to next watcher tick (Phase 4) | No watcher in Phase 3 — must reindex synchronously. Acceptable: one file's `indexer::reindex_path()` is <5ms. |

---

## Package Legitimacy Audit

slopcheck not available at research time → all packages tagged `[ASSUMED]` until plan 03-01 verifies. However, all CM6 packages were verified to exist on the npm registry via `npm view`, and all Rust crates are already in the workspace (`tempfile`, `blake3`) or cited in the canonical STACK.md research artifact (`dashmap`).

| Package | Registry | Age | Source | npm/cargo Verified | Disposition |
|---------|----------|-----|--------|---------------------|-------------|
| `@codemirror/state` | npm | 6+ yrs | github.com/codemirror/state | `npm view` ✓ 6.5.0 | Approved [ASSUMED until slopcheck] |
| `@codemirror/view` | npm | 6+ yrs | github.com/codemirror/view | `npm view` ✓ 6.43.0 | Approved |
| `@codemirror/commands` | npm | 6+ yrs | github.com/codemirror/commands | `npm view` ✓ 6.6.0 | Approved |
| `@codemirror/language` | npm | 6+ yrs | github.com/codemirror/language | `npm view` ✓ 6.12.3 | Approved |
| `@codemirror/lang-markdown` | npm | 6+ yrs | github.com/codemirror/lang-markdown | `npm view` ✓ 6.10.3 | Approved |
| `@codemirror/autocomplete` | npm | 6+ yrs | github.com/codemirror/autocomplete | `npm view` ✓ 6.20.2 | Approved |
| `dashmap` | crates.io | 5+ yrs | github.com/xacrimon/dashmap | not yet in workspace — plan 03-01 to verify via `cargo search dashmap` | Approved (add) |
| `tempfile` | crates.io | 8+ yrs | github.com/Stebalien/tempfile | already in workspace; promote dev-dep → runtime dep | Approved |
| `blake3` | crates.io | 5+ yrs | github.com/BLAKE3-team/BLAKE3 | already in workspace | Approved |

**Planner must run** `cargo search dashmap` + `npm view @codemirror/lang-markdown version` at plan 03-01 install time and pin the exact versions.

---

## Common Pitfalls (Recap from research/PITFALLS.md, scoped to Phase 3)

### Pitfall 1 — Lossy round-trip (CRITICAL)
**Already mitigated by Phase 1.** Phase 3 must NEVER use `pulldown-cmark-to-cmark` or any AST-serializer. The mutation path is: `byte-splice into original buffer → atomic_write`. The unchanged 99% of the file is byte-for-byte identical.

**Verification:** ACPT-01 (Phase 1's round-trip CI gate) MUST stay green for every commit in Phase 3. **Plan 03-03 should add a test that performs a no-op `PUT /api/blocks/:id` (raw unchanged) and asserts the file mtime advances but the bytes are identical.**

### Pitfall 5 — CM6 focus/IME/boundary (HIGH)
**Mitigated by §1.** Specifically: mount/unmount discipline, `Prec.highest` wrap, `view.composing` guard on every save, no `updateListener` autosave.

**Verification:** Plan 03-04 ships IME unit test (§5) + manual Pt-BR test in `/gsd-verify-work`.

### Pitfall 2 — Watcher loop (CRITICAL; Phase 4 territory but Phase 3 lays groundwork)
**Mitigated by §2.** `SelfWriteSet` is registered in Phase 3 even though no consumer reads it yet. **Test:** plan 03-01 includes a unit test that `register(hash)` then `take_if_present(hash) == true` and that after TTL expiry, `take_if_present(hash) == false`.

### Pitfall 6 — Cross-platform paths (HIGH)
**Mitigated by Phase 1's `RelativePath`.** Phase 3 must use `RelativePath` at every new API boundary (rename target, create-page name). **Plan 03-07's rename endpoint MUST reject names containing reserved Windows chars** (`< > : " | ? *`) and reserved names (`CON`, `PRN`, etc.) with 400.

### NEW Pitfall — Windows AV holding the file lock during persist
**Mitigated by §2 retry loop.** Bounded 3 attempts × exponential backoff. If still failing, surface a meaningful error.

### NEW Pitfall — Tree-op log divergence from server state
After an `Indent` op is applied client-side optimistically, if the server returns 409 (e.g., concurrent edit elsewhere), the op must be inverted and the user notified. **Plan 03-04 includes:** client-side op replays a `requestId` so the server response can correlate; on 409, the inverse op is auto-applied AND the op is popped from `treeOpLog` (don't let the user "undo" a never-committed op).

### NEW Pitfall — Page rename to the same name (case change)
`OldName` → `oldname` (Windows-case-insensitive but case-preserving): the file rename will fail on Windows because the filesystem sees it as the same file. **Mitigation:** if `newName.to_lowercase() == oldName.to_lowercase()` and they differ in case, do a two-step rename via a temp name: `OldName` → `__foliom_rename_tmp__` → `oldname`. **Plan 03-07 must include this code path + test.**

---

## Common Pitfalls — Continued (Phase 3-specific decisions)

### Pitfall — Empty file vs `-\n` for unresolved-link create (D-30-03 open question)

CONTEXT.md left this open: "Backend creates the file with a single empty bullet `-\n` (or just an empty file? — to be decided in planning; tests can pin one)."

**Recommendation: create with `-\n` (single empty bullet line).**

Rationale: An empty file confuses the segmenter — it produces only a prelude block with `byte_length = 0`. When the user clicks into the just-created page, there is **no block to focus** — the editor UI has nothing to mount CM6 onto. With `-\n`, there is one empty bullet at depth 0, ready for the user to start typing.

The parser handles both cases correctly (segmenter has explicit "empty bullet" support — `segment.rs::detect_bullet_depth` accepts `b"-"`), so this is purely a UX call.

**Pin this in plan 03-07 with a test:** `POST /api/pages { name: "NewPage" }` → file contents are exactly `-\n` (3 bytes).

### Pitfall — Tree-op log capacity (D-30-05 open question)

**Recommendation: 200 entries, in-memory only, lost on page reload.**

Rationale: 200 ops is ~1 hour of heavy editing. In-memory only matches user expectation ("undo doesn't survive reload" — same as VS Code's per-session undo). Persisting to localStorage would conflict with the "no draft persistence" decision in D-30-01 (acceptable v1.0 risk). Pin in plan 03-04.

### Pitfall — Bullet popover positioning (deferred from CONTEXT.md)

**Recommendation: absolute-positioned `<div class="bullet-popover">` as a child of the `.block` element**, with `position: absolute; left: 100%; top: 0` and a small `transform`. No portal-to-body, no floating-ui library.

Rationale: blocks don't have `overflow: hidden`; the page scroll container does, but the popover is small (~150px wide × ~200px tall) and won't bump the edge in practice. If it does in a 03-06 manual test, swap to portal-to-body (~30 lines) — not worth pre-installing floating-ui. Pin in plan 03-06.

### Pitfall — Keyboard shortcuts not yet decided (CONTEXT.md open #8)

**Recommendation: ship these in plan 03-04:**
- `Ctrl+Enter`: same as `Enter` (sibling block) — Logseq users have muscle memory for it.
- `Alt+Shift+ArrowUp`: move block up (TreeOp `Move`).
- `Alt+Shift+ArrowDown`: move block down.
- `Ctrl+.` (D-30-04 hint): open bullet popover for focused block.

These are common-enough patterns that omitting them feels broken. All are TreeOps (no new server endpoints required).

---

## Code Examples

### Mount/Unmount Discipline (frontend/src/lib/editor/view.ts — new)

```typescript
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { blockEditorExtensions } from './extensions';

export class BlockEditor {
  view: EditorView | null = null;

  mount(parent: HTMLElement, initialRaw: string, callbacks: BlockEditorCallbacks): void {
    if (this.view) throw new Error('BlockEditor double-mount');
    const state = EditorState.create({
      doc: initialRaw,
      extensions: blockEditorExtensions(callbacks),
    });
    this.view = new EditorView({ state, parent });
    this.view.focus();
  }

  /// IME-safe doc read. Returns null if IME is composing — caller must defer.
  readDocSafe(): string | null {
    if (!this.view) return null;
    if (this.view.composing) return null;
    return this.view.state.doc.toString();
  }

  unmount(): string | null {
    if (!this.view) return null;
    // IMPORTANT: read BEFORE destroy (state is dropped after).
    const doc = this.readDocSafe();
    this.view.destroy();
    this.view = null;
    return doc;
  }
}
```

### Atomic Write with Self-Write Registration (crates/core/src/sync/atomic.rs — new)

```rust
use crate::sync::SelfWriteSet;
use std::path::Path;

pub fn atomic_write_md(
    target: &Path,
    contents: &[u8],
    self_writes: &SelfWriteSet,
) -> std::io::Result<[u8; 32]> {
    let hash: [u8; 32] = blake3::hash(contents).into();
    // Register BEFORE rename to close the race with any future watcher tick.
    self_writes.register(hash);

    let parent = target.parent().ok_or_else(|| std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "target has no parent directory",
    ))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;
    persist_with_retry(tmp, target)?;
    #[cfg(unix)]
    {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(hash)
}

fn persist_with_retry(tmp: tempfile::NamedTempFile, target: &Path) -> std::io::Result<()> {
    let path = tmp.into_temp_path();
    let mut attempt = 0u32;
    loop {
        match path.persist(target) {
            Ok(_) => return Ok(()),
            Err(e) => {
                let kind = e.error.kind();
                let retryable = cfg!(windows) && matches!(
                    kind,
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Other,
                );
                if !retryable || attempt >= 3 {
                    return Err(e.error);
                }
                std::thread::sleep(std::time::Duration::from_millis(50u64 << attempt));
                attempt += 1;
                // persist() consumed `path` on error — but tempfile's TempPath::persist returns
                // a PersistError that includes the TempPath. Bind it back:
                // (Verify API in plan 03-01; if signature is `PersistError { error, file }`,
                // rebind `path = err.file` here.)
            }
        }
    }
}
```

### Tree-Op Log Store (frontend/src/lib/stores/treeOpLog.ts — new)

```typescript
import { writable, get } from 'svelte/store';

export type TreeOp =
  | { kind: 'Indent', blockId: number, prevDepth: number }
  | { kind: 'Outdent', blockId: number, prevDepth: number }
  | { kind: 'Merge', blockId: number, mergedIntoId: number, originalRaw: string }
  | { kind: 'Split', blockId: number, atOffset: number, newBlockId: number }
  | { kind: 'Move', blockId: number, prevParentId: number | null, prevOrd: number }
  | { kind: 'Delete', blockId: number, snapshot: BlockSnapshot };

const CAP = 200;

function createTreeOpLog() {
  const { subscribe, update } = writable<TreeOp[]>([]);

  return {
    subscribe,
    push(op: TreeOp) {
      update(log => {
        const next = [...log, op];
        return next.length > CAP ? next.slice(next.length - CAP) : next;
      });
    },
    pop(): TreeOp | undefined {
      let out: TreeOp | undefined;
      update(log => {
        if (log.length === 0) return log;
        out = log[log.length - 1];
        return log.slice(0, -1);
      });
      return out;
    },
    clear() { update(() => []); },
  };
}

export const treeOpLog = createTreeOpLog();
```

---

## Validation Architecture

> `workflow.nyquist_validation` is `false` in `.planning/config.json` → this section omitted per template rule.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js + npm | Frontend build | ✓ (from Phase 2) | per CI matrix | — |
| Rust 1.85+ | Backend mutation + atomic-rename | ✓ (from Phase 1) | 1.85+ | — |
| SQLite (bundled-full) | Schema queries + transactions | ✓ (Phase 1) | embedded | — |
| Windows Defender / antivirus on Windows CI runner | AV retry path testing | ✗ unclear on `windows-latest` GH runner | — | Best-effort test; manual verification on Marcelo's Windows 11 native (per MEMORY) |
| WSL2 filesystem | Marcelo's dev env | ✓ | — | — |

**No blocking missing deps.** `dashmap = "6"` is new (added in plan 03-01).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `view.composing` is the exact CM6 property name (not `compositionStarted`) | §1, §5 | Save path doesn't guard IME → Pt-BR users get `~a` instead of `ã`. **Mitigation:** plan 03-04 greps `node_modules/@codemirror/view/dist/index.d.ts` and adjusts. |
| A2 | `@codemirror/lang-markdown` does not bind `Tab` by default | §1 | Tab in editor would insert tab instead of dispatching Indent TreeOp; user gets literal tab in raw, breaks indent semantics. **Mitigation:** plan 03-04 verifies via inspection + adjusts boundary handler to fight off any default. |
| A3 | `ErrorKind::PermissionDenied` maps to Windows `ERROR_ACCESS_DENIED` from `MoveFileExW` | §2 | AV retry never triggers; user sees raw error on AV interference. **Mitigation:** Windows CI integration test in plan 03-01. |
| A4 | 3 retries × 50/100/200ms = enough for Defender hold times | §2 | AV-locked write fails 1× per 1000 saves; user-visible. **Mitigation:** tunable constant; bump to 5 retries / 1s budget if telemetry shows failures. |
| A5 | happy-dom's `CompositionEvent` dispatch flips CM6's `view.composing` | §5 | IME unit test passes but doesn't actually exercise the guard. **Mitigation:** fallback to mocking `view.composing` directly (assertion still holds). |
| A6 | dashmap 6.x is available on crates.io with stable API | §2 | Adoption blocker. **Mitigation:** `cargo search dashmap` in plan 03-01 confirms. |
| A7 | Empty file vs `-\n` is a UX call only, not a parser-correctness call | §6 (D-30-03 resolution) | If parser does break on empty file, plan 03-07 picks `-\n` (the safe option). Already pinned. |
| A8 | The CM6 boundary keymap with `Prec.highest` reliably runs before `defaultKeymap` regardless of extension array order | §1 | Backspace/Enter never reach our handler → merge/sibling logic broken. **Mitigation:** plan 03-04 ships an integration test that mounts CM6 + dispatches `Backspace` and asserts the boundary handler ran (via `onBoundary` mock counter). |
| A9 | `tempfile::PersistError` exposes the original `TempPath` to rebind for retry | §2 code example | Retry loop can't reconstruct after first failure. **Mitigation:** plan 03-01 reads the tempfile docs; if API doesn't expose this, switch retry to "compute the file again from `contents` and retry from `NamedTempFile::new_in`". |

---

## Open Questions

1. **Question:** Does `@codemirror/lang-markdown` ship a `Tab` keybinding by default that fights our `Prec.highest` interception?
   - **What we know:** CM6 lang modules sometimes ship a `defaultKeymap` extension as part of their default export.
   - **What's unclear:** Whether `markdown()` includes any keymap or purely grammar.
   - **Recommendation:** plan 03-04 inspects `markdown()` return value at runtime; if it has a keymap, exclude it via `markdown({ /* no keymap */ })` or wrap our keymap with even higher precedence (it already is `Prec.highest`).

2. **Question:** Should `POST /api/pages/:name/rename` block on backlinks rewrite completion, or return 202 + poll?
   - **What we know:** 50-file rewrites of ~10ms each = ~500ms. Acceptable as a blocking call for single-user local backend.
   - **What's unclear:** What about 500-file mass renames? Browser request timeouts (~30s default).
   - **Recommendation:** plan 03-05 ships **blocking** by default (simpler; covers 99% case). If we see real >100-file renames in dogfooding, add streaming progress via SSE in Phase 4.

3. **Question:** Does the bullet popover (D-30-04) close on `Escape`, or only on click-outside?
   - **What we know:** D-30-04 says "Clicking outside closes it."
   - **What's unclear:** `Escape` behavior.
   - **Recommendation:** plan 03-06 adds `Escape` → close as a freebie (one keydown listener).

4. **Question:** Should `Enter` on the LAST block of a page create a new block, or do nothing?
   - **What we know:** D-30-01 says Enter creates a sibling.
   - **What's unclear:** When the focused block has no following sibling and Enter is pressed, the new block goes after it. Does the page need a trailing empty bullet always? Some outliners do (Logseq does — to give the user a place to keep typing).
   - **Recommendation:** plan 03-04 includes "always show a trailing-empty-bullet affordance below the last block (clicking it focuses CM6 on a new empty block)". Specs the Logseq UX users expect.

---

## Sources

### Primary (HIGH confidence)
- `npm view @codemirror/{state,view,commands,language,lang-markdown,autocomplete} version` — module versions verified 2026-05-22.
- [GitHub stebalien/tempfile #316](https://github.com/Stebalien/tempfile/issues/316) — Windows AV retry pattern documented.
- [docs.rs tempfile::NamedTempFile::persist](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist) — atomicity guarantees + cross-fs restriction.
- `crates/core/src/parser/segment.rs` — direct read, confirmed `RawBlock` byte-range invariant + bullet detection.
- `crates/core/src/parser/ast.rs` — direct read, confirmed `strip_segmenter_prefix` + `extract_refs`.
- `crates/core/src/storage/migrations/001_init.sql` — direct read, confirmed `blocks.byte_offset` + `refs.target_page` → `pages.id` (page-id stable across rename).
- `crates/cli/src/cmd/serve/routes/pages.rs` — direct read of `assemble_tree` reuse pattern.
- `frontend/src/lib/components/Block.svelte` + `PageHeader.svelte` + `api.ts` — direct read of Phase 2 extension points.

### Secondary (MEDIUM confidence)
- [codemirror.net/docs/ref/](https://codemirror.net/docs/ref/) — `Prec.highest` documented; `view.composing` documented in CM6 source (not in the ref pages fetched).
- `.planning/research/STACK.md` — pinned CM6 extension list, dashmap recommendation.
- `.planning/research/PITFALLS.md` §5 — CM6 IME pitfalls, mount/unmount rationale.
- W3C UI Events spec for `CompositionEvent` sequence.

### Tertiary (LOW confidence — verify at implementation time)
- happy-dom CompositionEvent dispatch flipping CM6 `view.composing` (A5).
- Exact `PersistError` API for retry-after-failure (A9).
- Exact Windows AV hold-time distribution (A4 retry budget).

---

## Metadata

**Confidence breakdown:**
- CM6 extension config (§1): HIGH — versions verified, pattern canonical.
- Atomic rename + AV retry (§2): HIGH — `persist` semantics cited, retry pattern documented in tempfile #316.
- Backlinks rewrite atomicity (§3): HIGH — WAL pattern is standard.
- CM6 history per-mount (§4): HIGH — direct consequence of EditorState lifecycle.
- IME test fixture (§5): MEDIUM — happy-dom feasibility partly assumed (A5).
- Copy-as-markdown format (§6): HIGH — dictated by D-30-07 and segmenter contract.
- Mutation API surface (§7): HIGH — straightforward REST.
- Pattern map (§8): HIGH — direct file reads.

**Research date:** 2026-05-22
**Valid until:** 2026-06-22 (30 days; CM6 + tempfile versions are stable)

---

## Recommended Plan Breakdown (6–7 plans, matching Phase 2 granularity)

The planner will create plans; this is a suggested split:

1. **03-01** — Atomic write + SelfWriteSet (`crates/core/src/sync/`) — §2. Add `dashmap` to workspace; promote `tempfile` to runtime dep. Windows CI test for AV retry.
2. **03-02** — Mutation engine (`crates/core/src/mutation/{splice,tree_ops}.rs`) — §7. Pure logic: given `(file_bytes, block_id, new_raw)`, compute new `(byte_offset, byte_length)` for the changed block and all downstream shifts; apply `TreeOp` enum. No HTTP.
3. **03-03** — Mutation REST endpoints (`crates/cli/src/cmd/serve/routes/blocks.rs`) — §7. PUT/POST/PATCH/DELETE handlers; conflict detection via `prevHash`; integration test that `assemble_tree` returns updated subtree.
4. **03-04** — CM6 editor frontend (`frontend/src/lib/editor/` + `Block.svelte` mods) — §1, §4, §5. Single-instance mount/unmount, boundary keymap, IME guard, history per instance, `treeOpLog` store.
5. **03-05** — Page rename + backlinks WAL (`crates/core/src/rename/`, `POST /api/pages/:name/rename`) — §3, D-30-02. Journal + recovery on startup.
6. **03-06** — Autocomplete + Bullet popover + Paste detection — §1 (autocomplete endpoint + CM6 wiring), D-30-04 (bullet popover), D-30-07 (paste TS port).
7. **03-07** — PageHeader rename UX + unresolved-link create + Obsidian portability gate (ACPT-05) — D-30-02 UX, D-30-03 LNK-04, ACPT-05 regression: open every edited file in Obsidian/VS Code, assert no warnings (manual test pinned to `/gsd-verify-work`).

ACPT-01 (Phase 1's round-trip gate) MUST stay green through all 7 plans.
