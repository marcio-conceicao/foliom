# Pitfalls Research

**Domain:** Local-first markdown outliner with file-system sync (Logseq-base-compatible)
**Researched:** 2026-05-21
**Confidence:** MEDIUM-HIGH (based on training knowledge of comrak/goldmark/notify/CodeMirror 6/Tauri internals and direct inspection of `data-folder-sample/Logseq/`; WebSearch was unavailable in this run, so claims tied to specific GitHub issues are stated as "known pattern" rather than "verified at URL X". Cross-check critical items in M0 prototype before locking design.)

> **Why these pitfalls and not others:** This document is scoped to the four PRD load-bearing principles — `.md` is canonical, no metadata injected, must open the existing 600-file Logseq base without corruption on first edit, and watcher must not loop. Generic web-app pitfalls are deliberately excluded.

---

## Critical Pitfalls

### Pitfall 1: Lossy round-trip in re-serialization (AST → markdown)

**What goes wrong:**
App parses a `.md` file to an AST, the user edits one block, app re-serializes the whole document → the bytes that come back differ from the bytes that went in, even in regions the user never touched. After one edit, `git diff` shows thousands of irrelevant changes; on the second edit, more drift accumulates. On the user's 600-file base this destroys git history, breaks Syncthing conflict resolution, and silently mutates `key:: value` block properties.

Common concrete mutations:
- TAB indent normalized to 2 or 4 spaces (kills RF-50 immediately).
- `*` bullets rewritten as `-`, or vice-versa.
- Hard line wraps inserted/removed.
- Trailing whitespace stripped (breaks Markdown's 2-space hard-break syntax).
- `# Heading` rewritten as `Heading\n=======` (setext) or vice-versa.
- Code-fence info string `\`\`\`ts` rewritten as `\`\`\`typescript`.
- `[text](url "title")` quote style changed (`"` ↔ `'`).
- `:LOGBOOK: ... :END:` drawer (seen in `data-folder-sample/Logseq/pages/`) flattened or its indentation lost.
- 2-space continuation lines under a bullet re-indented or merged into the bullet text.
- Final newline added/removed (every CI diff tool flags this).
- HTML blocks normalized (attribute order, self-closing slashes).

**Why it happens:**
All mainstream CommonMark parsers (comrak, pulldown-cmark, goldmark, markdown-it, remark) are **lexers, not formatters**. Their AST is *semantic* — it intentionally throws away "irrelevant" source information because CommonMark says two different byte sequences can produce the same render. Re-serializing from that AST is a *new* formatting decision, not a reconstruction. Comrak's `format_commonmark` is explicitly documented as "best-effort, not byte-preserving"; pulldown-cmark has no canonical re-serializer at all (third-party `pulldown-cmark-to-cmark` exists and has known drift bugs); goldmark's renderer subpackage targets HTML, not markdown.

**How to avoid (load-bearing for this project):**
1. **Never re-serialize the whole document.** Adopt the discipline: *the file is a sequence of byte ranges; one block = one range; on edit, splice the new block bytes into the original buffer; write back.* The unchanged 99% of the file is byte-identical to what was read.
2. **Parse to (block, byte_offset, byte_length) tuples**, not to a tree of strings. Keep the original `Vec<u8>` / `[]byte` alive for the lifetime of the editor session.
3. **Serialize only newly-created blocks** from the editor's raw text (which is already markdown — see RF-13). Existing blocks are spliced verbatim.
4. **Round-trip property test in CI:** for every file in `data-folder-sample/Logseq/`, run `parse → serialize → assert byte-equal`. Fail the build on any drift. This is the single most valuable test in the project.
5. **Treat `key:: value`, `:LOGBOOK:`/`:END:` drawers, `SCHEDULED:`/`DEADLINE:` lines, and any unrecognized "weird" line as opaque text** that lives inside a block's raw byte range. Never let them touch the AST normalizer.
6. **Preserve TAB exactly.** Do not normalize indentation. Configure the editor (CodeMirror 6) so TAB inserts `\t`, not spaces.
7. **Preserve trailing newline policy** of the source file (detect on read; reapply on write).

**Warning signs:**
- A `parse → serialize → diff` test on real files shows *any* non-empty diff.
- After a no-op "save" of an unedited file, mtime changes and hash differs.
- Git `--word-diff` on a small user edit shows unrelated blocks changed.
- Code-fence content with leading whitespace gets re-indented.

**Phase to address:** **M0 (critical).** The "byte-range, not tree-of-strings" decision dictates the entire parser API and the SQLite schema for `blocks`. Decided after M0 = full rewrite. Make the round-trip property test the first test written.

**Severity:** CRITICAL.

---

### Pitfall 2: Watcher loop — app writes a file, watcher fires, app reparses, UI clobbers in-flight edit

**What goes wrong:**
On save, the OS emits a filesystem event for the file the app just wrote. The watcher catches it, debounces, calls "reindex this file", reparses, and — depending on UI wiring — re-hydrates the open page, blowing away cursor position or unsaved edits in another block. Worse, on macOS FSEvents and Windows ReadDirectoryChangesW, a single `write` can produce 2-5 events (CREATE + MODIFY + MODIFY + CHMOD), so naive ignore-by-time misses some.

A subtler failure: editor in another tool (VS Code) writes via *atomic save* (write tmp + rename), which fires `CREATE` + `RENAME` rather than `MODIFY`. App's "ignore my own writes" logic doesn't recognize the rename pattern, so external edits get classified as own-writes and silently dropped. Or the inverse: app's own writes-via-rename get classified as external and trigger a reload.

**Why it happens:**
- Naive approaches use timestamps ("ignore events within 500ms of my own write") — flaky under load, racy under git pull, and breaks when the user's clock is wrong.
- Path-based ignore lists leak (file gets written, added to ignore list, ignore list never cleared because the expected event never arrived → external edits to that path silently dropped forever).
- `notify` (Rust) and `fsnotify` (Go) coalesce events differently per OS. On Linux, inotify gives fine-grained masks; on macOS, FSEvents only tells you "something in this directory changed" at coarse granularity; on Windows, ReadDirectoryChangesW has a buffer that overflows under bulk operations (git checkout of 500 files) and you get a single "rescan" event with no path detail.

**How to avoid:**
1. **Hash-based dedup, not time-based.** On every own-write, compute `sha256(content)` and store `(path → hash)` in a write-fence map. On every incoming watcher event, read the file, hash it, and if it matches the fence map entry for that path → drop the event and remove the fence. This is robust against atomic-save rename patterns and clock skew.
2. **Stat-then-hash, not hash-eagerly.** For events on files the user has not opened, only read+hash if `mtime` changed vs. the indexed `mtime`. This keeps watcher cost bounded under bulk operations.
3. **Handle the "rescan" event explicitly.** On Windows ReadDirectoryChangesW buffer overflow and on macOS FSEvents kFSEventStreamEventFlagMustScanSubDirs, fall back to a full rescan of the affected subtree — do not drop the event.
4. **Debounce per-path, not globally.** A 300-500ms per-path debounce coalesces multi-event bursts from a single save. A global debounce drops events when many files change at once (git pull).
5. **Never reset editor state from a watcher event for the currently-open block.** If the watcher detects external change to a file the user has open with unsaved edits, surface a conflict UI ("file changed on disk — keep yours / load disk / diff"). Never silently overwrite either side.
6. **Use the parent-directory recursive watch, not per-file watches.** Per-file watches break on rename and exhaust inotify limits (`fs.inotify.max_user_watches`, default 8192 on many distros — easy to hit with 600+ files plus assets).
7. **Watch `.md` only.** Filter by extension in the watcher callback. The Logseq base has `assets/`, `draws/`, `whiteboards/` with high-churn binary files that should not even be hashed.

**Warning signs:**
- Saving a block causes a brief UI flicker (re-render after own-write event).
- Opening DevTools shows duplicate "reindex" log lines per save.
- Cursor jumps to start of block on save.
- On Linux, app silently stops detecting external edits after ~8000 files (inotify watch exhaustion).
- `git pull` of many files causes app to freeze (per-event reparse pile-up).

**Phase to address:** **M3 (sync with disk).** But the write-fence (hash map) must be designed into the storage layer in **M0** — retrofitting is painful because every write path needs to update the fence.

**Severity:** CRITICAL.

---

### Pitfall 3: FTS5 index drift after external `rm`, app crash mid-write, or watcher missed event

**What goes wrong:**
SQLite FTS5 returns hits pointing to files that no longer exist on disk (user `rm`'d outside the app) → click result → 404. Or the file exists but the indexed content is stale by hours/days because the watcher missed the event (Linux inotify overflow under bulk operation, app was not running when change happened). Or the app crashed mid-`UPDATE files SET hash=?` — `hash` is updated but the FTS row was never rebuilt → searches return stale content forever. Worst case: SQLite WAL was not checkpointed, the `.db-wal` file is corrupt after an abrupt shutdown, and the index unbootable.

**Why it happens:**
- FTS5 contentless / external-content tables require manual `INSERT INTO fts(fts) VALUES('rebuild')` after content-table updates; easy to forget for one code path.
- `mtime`+`hash` is a one-way trust: the app trusts the watcher to invalidate cache; nothing periodically verifies the index against disk truth.
- WAL mode (`PRAGMA journal_mode=WAL`) is correct for read-heavy workloads but requires `PRAGMA synchronous=NORMAL` (default `FULL` is overkill, `OFF` is dangerous) and a checkpoint policy.
- "Cache derivable, can be deleted" (PRD §5.1) is true in theory but psychologically tempting to skip cold-start scans because they were "expensive" — leading to no rescan ever happening in practice.

**How to avoid:**
1. **Treat the index as eventually-consistent and design the resync.** On startup, always do `stat` on every `.md` (cheap, per PRD §5.2). For each file, compare `(disk_mtime, disk_size)` vs. `(indexed_mtime, indexed_size)`. Mismatch → reparse. Indexed file no longer on disk → delete from index. Disk file not in index → parse.
2. **Atomic writes with `fsync` + rename.** Write to `path.tmp`, `fsync(tmp)`, `rename(tmp, path)`. This guarantees the file is either fully old or fully new — never half-written. Same pattern for SQLite (already handled by WAL if synchronous ≥ NORMAL).
3. **Wrap reindex of one file in a single transaction.** `BEGIN; DELETE FROM refs WHERE source=?; INSERT INTO blocks ...; UPDATE files SET hash=?; INSERT INTO fts(fts) VALUES('rebuild') — no, scope smaller — INSERT INTO fts(rowid, content) ...; COMMIT;`. Crash mid-transaction → SQLite rolls back, file marked dirty by hash mismatch on next start, retried.
4. **SQLite settings for this workload:**
   - `PRAGMA journal_mode=WAL;`
   - `PRAGMA synchronous=NORMAL;` (durability tradeoff: lose ≤ last few txns on power loss, but index is reconstructable from disk so this is fine)
   - `PRAGMA wal_autocheckpoint=1000;` (default; OK)
   - `PRAGMA temp_store=MEMORY;`
   - `PRAGMA mmap_size=268435456;` (256 MB; lazy, doesn't actually allocate)
   - Periodic `PRAGMA wal_checkpoint(TRUNCATE);` on idle (every 5 min, or on quit) to prevent unbounded `.db-wal` growth.
   - `PRAGMA optimize;` on quit (runs ANALYZE on dirty tables).
5. **Avoid `VACUUM` on hot path.** It rewrites the entire DB and locks for the duration. Schedule only on explicit user action ("rebuild index") or never (WAL + autocheckpoint is enough for years of operation).
6. **FTS5 contentless-delete tables (FTS5 `contentless_delete=1`, SQLite ≥ 3.43).** Lets you `DELETE FROM fts WHERE rowid=?` without storing content twice. Otherwise use external-content with explicit triggers.
7. **Version the index schema.** Store `PRAGMA user_version=N`. On app boot, if schema version differs → delete `.db` and rescan. Avoids brittle migrations on a regenerable cache.
8. **Integrity probe on boot.** `PRAGMA integrity_check;` on first start of a day. Failure → nuke and rescan.

**Warning signs:**
- Search results that 404 when clicked.
- `du -h notes.db-wal` grows above 50 MB (checkpoint not running).
- Search latency suddenly increases (FTS5 internal segments need merge — fixable with `INSERT INTO fts(fts) VALUES('merge=200,8')`).
- Index size grows monotonically even after deleting files (forgot to `DELETE FROM fts` on file removal).
- After `kill -9`, app refuses to start (WAL corruption — usually recoverable but reveals missing recovery path).

**Phase to address:** **M0 for transactional reindex + atomic writes + schema versioning. M1 for FTS5 query path. M3 for delete-detection on watcher events. Recovery probe in M3.**

**Severity:** CRITICAL.

---

### Pitfall 4: CommonMark/GFM parser quirks colliding with Logseq's TAB outliner convention

**What goes wrong:**
CommonMark says **4 spaces of indentation = indented code block**. Logseq nests bullets with TAB, which most parsers internally normalize to either 4 spaces or 8 spaces. Result: a deeply-nested bullet gets misparsed as a code block. The text is then "wrapped" in `<pre>` on render, and any inline `[[link]]` or `#tag` inside it is invisible to the linker because it's no longer a Text node.

Additional quirks on the real `data-folder-sample/Logseq/` base:
- `---` on its own line is a CommonMark **thematic break**, but inside an outliner it often appears under a bullet as a separator. Parser sees `- foo\n- ---\n- bar` and may emit `<hr>` mid-list.
- `:LOGBOOK:` ... `:END:` (seen in `pages/`) is an Org-mode "drawer" — CommonMark has zero awareness of it, treats the lines as paragraph text, and may join them with the bullet above.
- Tag-like text inside ATX headings (`### #Bruno`) must not be extracted as `#Bruno` tag (RF-21 spec, but easy to forget when extraction is a post-AST walk).
- HTML blocks (`<div>...</div>`) follow CommonMark "HTML block" rules that have **7 different start conditions**; getting this wrong eats following content or escapes it.
- Hard-wrapped paragraphs: `A line\nanother line` becomes one paragraph in CommonMark; if the parser is configured with `hardbreaks=true`, behavior diverges from Logseq.
- `==highlight==` is not CommonMark; Obsidian/Logseq render it; if the parser doesn't, users will see literal `==`.
- `> quote` followed by 2-space-continued bullet content can confuse the lazy-continuation rules.

**Why it happens:**
CommonMark spec assumes a top-level document. Block-per-block parsing (RF-16) is *not* what CommonMark was designed for. Spec-compliant parsers (comrak, goldmark) do exactly what the spec says, including the TAB=4-spaces=code-block rule.

**How to avoid:**
1. **Pre-tokenize the outliner structure before invoking the markdown parser.** A two-stage pipeline:
   - Stage 1 ("outliner lex"): split the file into blocks based on `^\t*- ` pattern + 2-space continuation rule + block-property/drawer recognition. Output: `Vec<RawBlock { indent: usize, byte_range: Range<usize>, content_bytes: &[u8] }>`.
   - Stage 2 ("block parse"): for each block, feed *only the content bytes* (with indentation stripped) into the CommonMark parser. The parser never sees TABs as indentation because we stripped them.
2. **Test fixture for every quirk.** Create `tests/fixtures/quirks/` with: tab-indented-code.md, thematic-break-in-list.md, logbook-drawer.md, hashtag-in-heading.md, html-block.md. Each fixture has an `expected.json` of extracted (blocks, tags, links). Lock behavior with golden tests.
3. **Configure the parser conservatively.** For comrak: `extension: { strikethrough, table, autolink, tasklist }`. Avoid `superscript`, `header_ids`, `front_matter_delimiter` — they introduce surprises. Disable `hardbreaks`. For goldmark: parser.WithBlockParsers default minus indented code block (since outliner stage already swallows leading TABs).
4. **Tag/link extraction walks only `Text` nodes.** Explicitly skip `CodeBlock`, `Code` (inline), `Html`, `Image` (alt text is debatable — pick "no"), and any ancestor heading. Property test: synthesize a code block containing `#tag` and `[[link]]`; assert zero refs extracted.
5. **Treat block properties (`key:: value`) and Org drawers (`:NAME: ... :END:`) as opaque lines inside the block.** They live in the raw byte range but are *not* fed to CommonMark — they sit in a separate `properties: Vec<(String, String)>` slot on the block, preserved verbatim, written back at the same position on re-serialize.
6. **Document the supported subset** in a `MARKDOWN.md` reference doc that users can read. Set expectations.

**Warning signs:**
- Deeply-nested bullets render in monospace (= got classified as code block).
- A `#tag` inside `\`code\`` shows up in the backlinks panel.
- A `---` separator under a bullet renders as `<hr>`.
- `:LOGBOOK:` text appears as part of a paragraph, mangled.
- Round-trip test (Pitfall 1) fails on files containing any of the above.

**Phase to address:** **M0 for the two-stage outliner-then-markdown pipeline, opaque preservation of properties/drawers, and tag/link extraction rules. M1 for renderer quirks (highlight syntax, etc.).**

**Severity:** CRITICAL.

---

### Pitfall 5: CodeMirror 6 + per-block "one editor at a time" — focus, IME, and multi-block navigation bugs

**What goes wrong:**
The PRD's editor model (RF-11/12/15) is "one block in edit, all others read-only". Naive implementations:
- Instantiate a fresh `EditorView` per focus → 50-200ms hiccup on every block transition (CM6 view construction is not free), cursor "jumps" visible to user.
- Reuse a single `EditorView` and reparent it on focus → DOM moves break focus, lose IME composition state mid-character (catastrophic for Japanese/Chinese/Korean users; less visible but still wrong for European dead-key composition like `~` + `a` = `ã`, which Brazilian Portuguese users hit constantly — and the PRD author writes Portuguese).
- Up/Down at the edge of a block tries to navigate to the previous/next block but CM6 `keymap` is per-instance — coordinating "leave editor on arrow at boundary" requires checking `view.state.selection.main.head` against `view.state.doc.length` *before* CM6 consumes the key, then dispatching a custom event the outer outliner handles.
- `Tab` in CM6 by default inserts a tab character — fine — but `Shift+Tab` does nothing by default; need explicit `indentLess` / outliner-outdent binding.
- `Backspace` at position 0 needs to merge with previous block; CM6's default deletes nothing, swallows the key, so the outliner-level merge handler never fires unless you intercept with `Prec.highest`.
- Selection across blocks (drag-select from block A into block B) is conceptually broken because each block is a separate contenteditable; browser default selection works visually but `document.execCommand('copy')` returns garbage.
- IME composition events fire on the underlying contenteditable; if your "save on blur" handler doesn't check `view.composing`, you save mid-composition, breaking the IME.
- Auto-save on every keystroke (`updateListener`) plus the watcher write-fence (Pitfall 2) creates write amplification — every keystroke writes the file. Need debounced save (500ms after last keystroke) + immediate save on blur/Enter.

**Why it happens:**
CodeMirror 6 is a *single-document* editor, designed for one large textarea, not a swarm of small ones. Every "outliner with CM6" project hits these issues. Notion, Logseq, Roam, Tana all use custom block editors built on contenteditable + ProseMirror or custom code, not CM6 per-block, partly for this reason.

**How to avoid:**
1. **Single CM6 instance, moved by mount/unmount, not by DOM reparenting.** On focus block B: tear down CM6 in block A (committing its raw text back into the block model), instantiate fresh CM6 in block B with B's raw text as initial doc. Construction cost is ~5-20ms for a small doc — acceptable if you preload the EditorState before mount.
2. **Pre-build `EditorState` synchronously on click, mount asynchronously in `requestAnimationFrame`.** Hides the hiccup behind the browser's next paint.
3. **Boundary-key extension.** A CM6 extension that, on ArrowUp/ArrowDown/Backspace/Enter, checks selection position and either lets CM6 handle (Prec.default) or dispatches a custom outliner event (Prec.highest). Test matrix: cursor at start, end, middle, in soft-wrapped line; with selection, without; with IME active (must always defer to CM6).
4. **`view.composing` guard on every save path.** `if (view.composing) return;` — never save during IME composition.
5. **Disable cross-block native selection.** `user-select: none` on the outer outliner; allow selection only within the active block. Implement copy-multiple-blocks via outliner-level command, not browser selection.
6. **Debounced autosave: 500ms idle OR blur OR Enter, whichever first.** Coalesces bursts of typing into one disk write.
7. **Persist cursor position across edit-mode transitions for the same block** (so re-entering doesn't always land at the end — though PRD §12.2 says "end of block is acceptable for v1"; document the decision).
8. **Don't ship CM6 language packs you don't use.** Each `@codemirror/lang-*` is ~30 KB. For markdown blocks, only `@codemirror/lang-markdown` is needed.

**Warning signs:**
- Typing `~a` in Brazilian Portuguese produces `~a` instead of `ã` (IME/dead-key broken).
- Cursor flickers when moving between blocks.
- Backspace at start of block deletes a character from the *previous* block (the merge handler ran after CM6 already deleted).
- Selection visually spans blocks but Cmd-C copies wrong text.
- DevTools shows multiple `EditorView` instances accumulating (memory leak).
- Profiler shows GC pauses correlated with block-focus transitions.

**Phase to address:** **M2 (editor outliner).** Build a 3-block toy first (no SQLite, no watcher) to nail focus/IME/boundary keys before integrating.

**Severity:** HIGH.

---

### Pitfall 6: Cross-platform file paths — Windows encoding, case-insensitivity, macOS NFC/NFD

**What goes wrong:**
- macOS HFS+/APFS stores filenames as **NFD** (decomposed Unicode); other tools (git, Linux, Windows) use **NFC** (composed). A page named `Avaliação` written by the user on Linux becomes `Avaliãç̃o` (NFD) when read back on macOS. The app's hash map key `"Avaliação"` (NFC) misses the file → app shows it as a new page, user has duplicates.
- Windows is **case-insensitive but case-preserving** by default. Two pages `Bruno.md` and `bruno.md` cannot coexist; the second write silently overwrites the first. On Linux they're distinct. Syncthing'ing between OSes triggers conflict files.
- Windows reserves filenames: `CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9` — and they're case-insensitive. User creates a page `Con` → Windows write fails or behaves bizarrely.
- Windows path separator `\` vs. `/`. Storing paths with `\` in SQLite breaks portability of the index across OSes (relevant if user syncs `notes.db` — they shouldn't, but they will).
- Windows MAX_PATH (260 chars) without the `\\?\` prefix or long-path manifest opt-in: deeply-nested journals + long titles can fail to open.
- macOS `.DS_Store`, Windows `Thumbs.db`, Linux backup files (`~`, `.swp`) appear in the directory — must be ignored.
- Trailing dots/spaces in filenames are silently stripped by Windows. Page `Foo ` and `Foo` collide.
- macOS Spotlight indexes the notes folder and can hold open file handles → write fails with EBUSY.
- WSL paths (`/mnt/c/...`) work but performance is 10-100x slower than native NTFS for many small files.

**Why it happens:**
File systems are not abstractions over "string → bytes" — they're full of OS-specific edge cases that don't surface in dev (everyone develops on macOS or Linux, users hit them on Windows).

**How to avoid:**
1. **Normalize all paths and filenames to NFC at the storage layer.** On read from disk, normalize to NFC before keying any hash map or SQLite row. On write, normalize the user's input to NFC. Use `unicode-normalization` crate (Rust) or `golang.org/x/text/unicode/norm` (Go).
2. **Store paths as forward-slash-separated, relative to the notes root.** Never store absolute paths or backslashes in SQLite. Convert at the OS boundary.
3. **Case-folded duplicate detection on import.** When indexing, build a `HashMap<CaseFoldedName, Vec<ActualName>>`. Any key with >1 entry → warn user on first run ("These files collide on Windows: ...").
4. **Reject reserved filenames on create.** Maintain the Windows-reserved list; on "new page" with such a name, refuse and explain. Same for trailing dots/spaces and characters `< > : " | ? *`.
5. **Use long-path-aware APIs on Windows.** In Rust, `std::fs` handles this since 1.58 if `Cargo.toml` opts into the manifest. In Go, use the `\\?\` prefix or ensure the binary is built with long-path manifest.
6. **Hard-coded ignore list:** `.DS_Store`, `Thumbs.db`, `desktop.ini`, `*.swp`, `*~`, `.git/`, `.svn/`, `.stfolder` (Syncthing), `.stversions/`, plus Logseq's `logseq/ assets/ draws/ whiteboards/ bak/ .recycle/ version-files/` (RF-53).
7. **Document WSL performance caveat.** Recommend users keep the notes folder on the WSL filesystem, not `/mnt/c/`. Add a startup warning if the root is detected on `/mnt/`.
8. **CI matrix runs tests on Linux, macOS, Windows.** Catches NFC/NFD on macOS, case-insensitive on Windows. GitHub Actions has all three free.

**Warning signs:**
- Mac user reports "I see two copies of the same page" (NFC/NFD mismatch).
- Windows user reports a page disappeared after sync (case collision).
- Index has duplicate rows for what users perceive as one file.
- App crashes opening a path > 260 chars on Windows.

**Phase to address:** **M0 for path normalization, separator handling, and reserved-name validation. M3 for the watcher's interaction with .DS_Store / Thumbs.db. M4 for Windows long-path manifest in Tauri config.**

**Severity:** HIGH.

---

### Pitfall 7: Logseq base compatibility — block properties, drawers, aliases, journal titles

**What goes wrong:**
PRD §6.6 lists the explicit Logseq compat requirements (RF-50 through RF-56), but the *interactions* between them are where bugs hide:

- **`alias:: Page Name` resolution:** If you don't interpret `alias::`, then `[[Page Name]]` in another file fails to backlink to the file declaring the alias → graph is silently broken for any user who relies on aliases (which the real base might or might not — needs grep). If you *do* interpret it, you must store the mapping but **not write it** — `alias::` lives only in the source file as a block property.
- **`id:: <uuid>` lines:** The PRD notes 9 such blocks exist in the sample base. If you treat them as block content, they render literally as `id:: abc-123` → ugly. If you strip them on render, you must preserve them on re-serialize (Pitfall 1). The right model: parse as a `properties: { id: "abc-123" }` map on the block, never render, always write back in canonical position (immediately after the bullet text, indented 2 spaces).
- **`#[[tag com espaços]]` extraction:** Easy to write a regex `#\w+` that misses these. Must match `#[[...]]` with a separate path, and the `...` interior is the tag name (which may equal a page name — PRD §12.1 unresolved decision).
- **2-space continuation under a bullet:** This is the rule that makes `:LOGBOOK:`/`:END:` drawers work, that makes code fences inside bullets work (RF-51 explicitly), and that makes `id:: ...` lines work. **Any block parser that doesn't honor it will corrupt the file on first edit.** Concrete failure: parser sees `- text\n  :LOGBOOK:\n  CLOCK: ...\n  :END:\n- next` as one bullet ("text") followed by 3 orphan paragraph lines followed by another bullet. Serialize → those 3 lines now belong to nothing → either dropped or hoisted to top level.
- **Journal title formatting (`May 21st, 2026`):** Requires `st/nd/rd/th` ordinal logic that's English-specific. Logseq stores the format in `config.edn` as `:journal/page-title-format "MMM do, yyyy"` — a *Tick* (Clojure date library) format string. Re-implementing the Tick subset Logseq uses is non-trivial. **Simplification:** detect the `YYYY_MM_DD.md` pattern in `journals/`, parse the date, format with a configurable strftime-like template (default `%B %-d{ord}, %Y`), expose to user. Don't try to interpret `config.edn`'s Tick format.
- **`config.edn`:** Clojure EDN, not JSON or TOML. Rust `edn-rs` and Go EDN libs exist but are immature. Either ship a vendored EDN reader (small subset is enough for `:journal/page-title-format`, `:hidden`, `:pages-directory`) or do regex extraction with documented limitations.
- **`pages-directory` and `journals-directory`:** Logseq lets users rename these. Don't hard-code `pages/` and `journals/` — read from `config.edn` if present, fall back to defaults.
- **`bak/` and `version-files/`:** Logseq stores backups of every edit here. Even after RF-53 says "ignore", users may have *manually* moved real notes into `bak/` (the directory name is not reserved by them) — surface a startup hint, don't silently drop content.
- **`namespace/sub-namespace` pages:** Logseq encodes namespaces as `Parent%2FChild.md` (URL-encoded slash). Filenames with `%2F` must be decoded back to `Parent/Child` for page-name display, and `[[Parent/Child]]` links must encode back to `Parent%2FChild.md` to resolve. The sample base doesn't show this in the first listing — but it's a real Logseq feature and the user's real 600 files might contain them.

**Why it happens:**
Logseq's on-disk format is *underspecified* — it's "whatever the Clojure code writes" — and evolves between versions. The only reliable spec is "open the user's actual files, see what conventions they use".

**How to avoid:**
1. **Inventory script.** Write a one-shot script that scans `data-folder-sample/Logseq/` and reports: count of files with `alias::`, with `id::`, with `:LOGBOOK:`, with `#[[...]]`, with `%2F` in filename, with `template::`, with code fences inside bullets, with `SCHEDULED:/DEADLINE:`. This converts unknowns into knowns. Run this *before* finalizing M0 parser.
2. **Block-property parser is a first-class concern, not an afterthought.** Recognize lines matching `^[\t ]*([a-zA-Z][a-zA-Z0-9._-]*):: (.*)$` *immediately following* a bullet line (at correct indent) as properties of that block. Store as `(key, value, original_byte_range)`. Render: hide. Serialize: emit back at canonical position.
3. **Drawer parser:** lines matching `^[\t ]*:[A-Z]+:$` start a drawer; ends at `:END:`. Everything between is opaque content of the drawer, attached to the parent block.
4. **Roundtrip test the entire real base.** Once Pitfall 1's round-trip test exists, point it at all 600+ real files. Any drift = parser bug to fix before M0 closes.
5. **Defer `alias::` and `id::` interpretation.** v1: preserve opaque (RF-54). v1.1: opt-in alias resolution behind a setting, after observing how the real base uses them.
6. **Workflow markers (`TODO`/`DONE`/`SCHEDULED:`):** PRD §12.9 unresolved. Recommendation: treat as plain text in v1 (render as-is, no special UI), preserve verbatim. Decide post-M2 based on usage.
7. **Namespace encoding:** decode `%2F` on read for display, encode back on write. Test fixture mandatory.

**Warning signs:**
- After first edit of a page with `:LOGBOOK:`, the LOGBOOK content moves or disappears.
- `[[Some Alias]]` doesn't find its target (alias resolution missing).
- Journal title shows as `2026_05_21` instead of `May 21st, 2026` (formatter missing).
- A page with `%2F` in its filename shows up as a literal `%2F` in the UI.
- `id:: <uuid>` line renders as visible text in a block.
- Inventory script reveals a Logseq feature in real files not covered by any test fixture.

**Phase to address:** **M0 for inventory script, block-property parser, drawer parser, namespace decoding. M1 for journal title formatter, link resolution (including alias decision). M2 for workflow marker decision.**

**Severity:** CRITICAL.

---

## High-Severity Pitfalls

### Pitfall 8: Tauri/Wails packaging — code signing, antivirus, autoupdater

**What goes wrong:**
- **macOS:** Unsigned Tauri binary → Gatekeeper shows "cannot be opened, developer cannot be verified" — user must right-click → Open, *every download*. With Apple Notarization missing, even right-click stops working on macOS 15+. Apple Developer Program: $99/yr + Notarytool setup.
- **Windows:** Unsigned binary → SmartScreen "Windows protected your PC" warning. EV code-signing cert: $300-600/yr; OV cert is cheaper but has a "reputation warm-up" period of weeks-to-months where SmartScreen still warns.
- **Antivirus false positives:** Tauri's WebView2 bootstrapper and Rust-compiled binaries are heuristically flagged by some AVs (BitDefender, Kaspersky historically). Wails has similar issues. Real risk: corporate users can't run the app.
- **Linux:** AppImage works without signing but isn't installable system-wide cleanly. `.deb`/`.rpm` need maintainer signing for repository inclusion.
- **Autoupdater:** Tauri's updater requires signed update manifests; misconfiguration → users on v0.1 forever, or worse, MitM update-injection vector.
- **WebView2 on Windows:** Tauri assumes Evergreen WebView2 is installed (it is on Win11, may not be on stale Win10). Fixed-version WebView2 bundling adds ~150MB to installer — kills RNF-05 (small footprint).
- **WebKit on Linux:** Tauri uses webkit2gtk; version skew across distros causes rendering differences. Wails has similar issues.
- **macOS notarization timing:** Notary service takes 5-30 min per build. Breaks fast CI.
- **Universal binary on macOS:** Need both `x86_64-apple-darwin` and `aarch64-apple-darwin` builds + `lipo`. Cross-compilation has toolchain gotchas.

**How to avoid:**
1. **Budget for signing certs from day 1 of M4.** $99 Apple + ~$300 Windows OV = $400/yr. Without these, the desktop app has a hostile install experience.
2. **Use Tauri's official GitHub Actions templates** — they handle signing, notarization, universal binary, and updater manifests correctly. Don't roll your own.
3. **Submit a sample binary to BitDefender/Kaspersky/Microsoft Defender false-positive forms** after first signed release. Pre-emptively prevents user reports.
4. **Document the Win10 WebView2 dependency** in install docs; provide a bootstrapper-included installer variant for offline-install scenarios.
5. **For v1, skip the autoupdater.** Use GitHub Releases + manual download. Add updater in v1.1 after signing is stable. Reduces attack surface and shipping complexity.
6. **Universal binary on macOS via `cargo tauri build --target universal-apple-darwin`** (Tauri 2.x). Test on both Intel and Apple Silicon.
7. **Footprint test in CI.** Fail the build if Tauri bundle size exceeds a threshold (e.g., 50 MB on macOS, 30 MB on Linux). Drift watch keeps RNF-05 honest.

**Warning signs:**
- macOS users report "cannot be opened" → notarization missing or expired.
- Windows users report SmartScreen → cert reputation not warmed up; needs more downloads to build reputation, or upgrade to EV cert.
- Bundle size grew 20 MB between releases → audit `Cargo.toml` features, drop unused Tauri plugins.
- Corporate user reports AV quarantine → submit false-positive ticket.

**Phase to address:** **M4 (packaging).** But the signing cert procurement has weeks of lead time (Apple verification, OV vetting) — start this admin work at the *start* of M3.

**Severity:** HIGH (project-killing for desktop adoption if ignored; not blocking for headless/web phases).

---

### Pitfall 9: "Lazy loading" misimplemented — accidentally loading the world

**What goes wrong:**
PRD §5.3 mandates lazy loading: "metadata in index, content loaded on demand". Easy to break:
- Backlinks panel queries `SELECT * FROM blocks WHERE refs_target = X` and renders block text → if `blocks.raw` is in the index (not just on disk), this loads thousands of blocks into memory.
- Search results show snippets → FTS5 snippet generation requires loading content. Bounded if you `LIMIT 50`, unbounded if pagination loads more.
- "Recent pages" sidebar opens 20 files on startup to show titles → defeats cold-start optimization.
- Graph view (if added later) tries to render all nodes with content tooltips.
- `mtime` rescan on every focus event (file picker dialog) re-stats N files.

**How to avoid:**
1. **Decide: store `raw` in SQLite or not?** PRD §12.3 unresolved. Recommendation: store **only block metadata** (id, page, parent, order, properties, extracted refs) in `blocks`. Block raw text lives in the source file at `(byte_offset, byte_length)`. Loading a page = read file, slice byte ranges.
2. **Bounded queries by default.** All "list" queries have `LIMIT 100` enforced at the data-access layer. UI handles "load more".
3. **Snippet generation uses FTS5's `snippet()` function**, which works against indexed content (small) without loading the full block.
4. **No content in startup path.** Boot sequence: open SQLite, stat all files, schedule dirty-file reparse on background thread, return control to UI. Do not read any `.md` content on the critical path.
5. **Profile memory at boot, at 100 pages open, at 10k-file graph.** Set RNF-02 targets as CI gates.

**Warning signs:**
- Cold start time grows linearly with graph size (should be ~constant + dirty-file count).
- RAM at idle grows over time with no pages open (block content cached but never released).
- Backlinks panel takes seconds to render.

**Phase to address:** **M0 (decide blocks-storage model). M1 (bounded queries). Memory profiling continuous from M1.**

**Severity:** HIGH (this is the project's core value prop — if missed, no reason for Foliom to exist).

---

### Pitfall 10: Conflict resolution when external edit collides with in-app edit

**What goes wrong:**
User edits block in Foliom (unsaved); meanwhile Syncthing pulls a remote edit to the same file. App's watcher fires. What now?
- Naive: reload from disk → user loses their typing.
- Naive: ignore → user saves, overwrites the remote edit silently.
- Realistic: detect, surface conflict UI. But "block-level conflict" doesn't fit "file-level diff" — and the user thinks in blocks.

**How to avoid:**
1. **On watcher event for a file with an open editor:** compare `(disk_hash, last_seen_disk_hash, editor_dirty)`. If editor not dirty → silently reload, refresh UI. If dirty → block save, show banner: "File changed on disk. [View diff] [Keep mine] [Reload disk]".
2. **Write a `.foliom-conflict-<timestamp>.md` copy** when user picks "Keep mine" but wants to preserve the disk version. Same pattern Syncthing uses.
3. **Never auto-merge.** Markdown text merge is what `git merge` is for; defer to it. Document: "for sync conflicts, use Syncthing's conflict files or git".

**Warning signs:**
- Users report "I lost my edit after Syncthing synced".
- Two devices show different content for the same file with no warning.

**Phase to address:** **M3.**

**Severity:** HIGH.

---

## Medium-Severity Pitfalls

### Pitfall 11: Symlinks and bind mounts in the notes root

User has `~/notes/` with a symlink `~/notes/work -> /mnt/shared/team-notes/`. App walks, descends symlink, indexes 50k unrelated files, OR fails on broken symlink mid-scan. Fix: configurable "follow symlinks" (default false); on broken symlink, log warning and skip. **Phase:** M0.

### Pitfall 12: Empty pages and zero-byte files

`touch foo.md` creates a 0-byte file. Parser must accept; renderer must show "(empty page)". Deleting all content in the editor must not delete the file (user expectation). **Phase:** M1/M2.

### Pitfall 13: Very large files

A user might have a 10 MB journal aggregate. Reading + parsing on focus blocks the UI. Mitigate: parse on background thread, show "loading" indicator above some threshold (1 MB). **Phase:** M1.

### Pitfall 14: BOM and encoding

UTF-8 BOM (`EF BB BF`) at start of file — most editors handle, but it shows as a visible character in some renderers. Strip on read, re-add on write only if originally present. Reject non-UTF-8 files with a clear error rather than mis-decoding. **Phase:** M0.

### Pitfall 15: Backlinks performance at scale

`SELECT source FROM refs WHERE target = ?` without an index → seq scan over millions of rows. Mandatory: `CREATE INDEX idx_refs_target ON refs(target);`. **Phase:** M1.

### Pitfall 16: SQLite database in a cloud-synced folder

User puts the notes folder (containing `.foliom/notes.db`) in Dropbox/iCloud → corruption from concurrent multi-device access to SQLite. Default location for the DB must be **outside** the notes folder (e.g., `$XDG_DATA_HOME/foliom/<root-hash>.db`). **Phase:** M0.

### Pitfall 17: Time zones in journal page titles

`YYYY_MM_DD.md` is timezone-naive. User in UTC+0 creates `2026_05_21.md`, opens on a device in UTC-5 at 11pm previous day — "today" mismatch causes new journal page creation. Pick the user's local timezone, document it. **Phase:** M1.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Re-serialize whole file on save instead of byte-splice | Simple `serialize(ast)` call | Fails Pitfall 1; corrupts user's 600 files on first edit | **Never** in this project |
| Store `raw` block text in SQLite | Faster page open (no file read) | Violates "index is derivable cache"; doubles disk footprint; cache-invalidation hell | Acceptable as a small in-memory LRU; never as authoritative storage |
| Time-based watcher dedup instead of hash-based | 10 lines of code vs. 50 | Pitfall 2 — race conditions, clock skew | Acceptable only as second-line defense backing up hash-based |
| Skip macOS notarization for early releases | Saves $99 + 30 min/release | Users can't install on macOS 15+ | Acceptable in pre-M4 internal builds; never in public release |
| Hard-code `pages/` and `journals/` | Don't need EDN parser | Breaks for Logseq users who renamed dirs | Acceptable in M0 with TODO; must fix by M1 |
| Parser supports CommonMark only, no GFM tables | Smaller parser config | Real Logseq base might have tables | Acceptable in M0; revisit per §12.5 |
| Single global watcher debounce | Simple | Drops events under bulk operations | Acceptable for v0.1; switch to per-path in M3 |
| No cross-platform CI | Faster CI | Bugs found by users on Windows | **Never** — GitHub Actions runners are free |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| SQLite + WAL | Leaving `synchronous=FULL` | `synchronous=NORMAL` for derivable cache; document tradeoff |
| FTS5 | Forgetting `INSERT INTO fts VALUES('optimize')` on quit | Wire into shutdown hook |
| `notify` (Rust) / `fsnotify` (Go) | Per-file watches | Recursive parent-directory watch |
| Syncthing | Assuming files appear atomically | Handle `.tmp.<random>` partial files; ignore until renamed |
| Git | Storing `.db` in repo | Always `.gitignore` the index dir |
| Tauri | Calling Rust from JS without IPC contracts | Define typed `tauri::command` handlers with schema |
| WebView2 | Assuming evergreen install on Win10 | Detect, prompt to install bootstrapper |
| CodeMirror 6 | Importing the whole package | Cherry-pick `@codemirror/state`, `@codemirror/view`, `@codemirror/lang-markdown` |
| Logseq `config.edn` | Parsing as JSON | Vendored minimal EDN reader or regex extraction with documented scope |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Reparse whole file on watcher event for a single-block change | UI hiccup on every external save | Only reparse changed byte range if possible; for v1 reparse whole file but on background thread | At 5k+ files with active Syncthing |
| Eagerly hydrate backlinks panel | Slow page open | Query metadata only; load block snippets on hover/scroll | At 100+ backlinks |
| FTS5 without `LIMIT` | Search returns 10k rows, UI freezes | `LIMIT 100` + pagination | At 1k+ matching results |
| Synchronous stat-all-files on UI thread | Cold start blocks for 1-3s on 5k files | Background thread + progress indicator | At PRD target scale (5k files) |
| Per-file inotify watch on Linux | Silent failure beyond 8192 files | Recursive dir watch + raise limit warning | At 8k+ tracked files |
| String-keyed HashMap<String, Block> for all blocks | RAM grows linearly with graph | Lazy load, LRU eviction, ~100 page cache cap | At 10k+ blocks open in session |
| Re-render all open blocks on any state change | UI lag during editing | Block-level memoization (Svelte `{#each (key)}`, Solid `<For>`, React `memo`) | At 200+ blocks on screen |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Local backend listens on `0.0.0.0` | LAN attacker reads notes | Bind to `127.0.0.1` only; loopback is non-negotiable |
| No origin check on local API | Browser CSRF from malicious site | Check `Origin` header; reject non-localhost; or use random per-launch port + token in URL fragment |
| Symlink traversal | App reads `~/.ssh/id_rsa` because user's notes root has a symlink | Resolve symlinks, reject paths outside configured root |
| HTML in markdown rendered raw | XSS if user pastes `<script>` (low risk for single-user, but renderer might be shared with backlinks panel where another file's HTML is rendered in this file's context) | Sanitize HTML output (DOMPurify or comrak's `unsafe_=false`) |
| Logging file contents | Notes leak to log files / crash reports | Log paths and counts; never content |
| Tauri allowlist too broad | Renderer can call dangerous Rust APIs | Tauri 2.x capabilities — minimal, per-window |
| WebView2 navigates to arbitrary URLs | Phishing via `[[https://evil]]` rendering as link, opened in app webview with privileged IPC | Open external links in system browser, never in-app webview |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Silent data normalization on first save | User's files change without consent; loses trust | Show a one-time "preview changes" diff on first save of any file, especially after parser upgrades |
| No "show me what's indexed" view | User can't debug missing backlinks | Per-page debug panel: extracted tags, links, properties |
| Modal "file changed on disk" without diff | User picks blindly | Side-by-side diff in conflict UI |
| Auto-creating page on `[[New Name]]` typo | Junk pages everywhere | Surface "Create page?" inline confirmation, or auto-create only on follow |
| Indenting with spaces in the editor when file uses TAB | First save mixes indentation styles | Editor reads existing file's convention, mirrors it |
| "Save" button (mental model of "unsaved") | Confusing in autosave editor | No save button; show "saved Xs ago" indicator |
| Cold-start "loading…" without progress | Feels frozen on large graphs | Streaming indicator: "indexed 200/600 files" |
| No way to exclude a subfolder | User has unrelated `.md` in notes root | Glob-based ignore list in settings |

---

## "Looks Done But Isn't" Checklist

- [ ] **Markdown round-trip:** Verify on real base — `parse → serialize → byte-equal` for all 600 files in `data-folder-sample/Logseq/`.
- [ ] **Watcher loop:** Save in app, verify exactly one disk-write event consumed, no reparse triggered. Then save externally, verify reparse fires.
- [ ] **FTS5 delete:** `rm` a file outside the app, run search, verify deleted file is gone from results within one watcher cycle.
- [ ] **Crash recovery:** `kill -9` mid-reindex; restart; verify index converges to disk truth.
- [ ] **NFC/NFD:** Create file with accented filename on Linux, sync to macOS, verify same page appears (not duplicated).
- [ ] **Windows path:** Open notes root at deep path on Windows (>250 chars), verify no crash.
- [ ] **Logseq compat:** Open every file in `data-folder-sample/Logseq/`, edit one block in each, save, `git diff` shows only intended changes.
- [ ] **`:LOGBOOK:` preservation:** Edit a block that has a LOGBOOK drawer; verify drawer survives unchanged.
- [ ] **`id::` preservation:** Edit a block with `id:: <uuid>`; verify UUID line is still there, in same position, after save.
- [ ] **IME composition:** Type `~` then `a` with Brazilian Portuguese keyboard → see `ã`, not `~a`. Test on macOS, Windows, Linux.
- [ ] **Boundary keys:** Up at start of block goes to previous block at end position. Down at end goes to next block at start.
- [ ] **Lazy loading:** Open app pointed at 5k-file graph; verify RAM < 200 MB at idle with no pages open.
- [ ] **Cold start:** Same graph, measure time to interactive. Target < 3s on warm cache, < 5s on cold cache.
- [ ] **Tauri signed installer:** Download fresh on clean Win11 / macOS / Ubuntu, install without security warning, launch successfully.
- [ ] **External backlinks:** Page A references `[[B]]`; create B externally with VS Code; backlink appears in B without app restart.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Re-serialization corruption discovered late | HIGH | Restore from user's git/Syncthing history. Add round-trip CI gate. Communicate. |
| Watcher loop ships to users | LOW | Hotfix with hash-based dedup; users see brief flicker until update. |
| FTS5 corrupted | LOW | Delete `.db`, app rescans on next launch (per PRD §5.1). |
| Wrong indentation style on save (spaces vs TAB) | MEDIUM | Restore from backup; ship fix that reads original file's convention. |
| `:LOGBOOK:` lost | HIGH | Per-user manual restore from git/backup; no programmatic recovery if not in version control. |
| macOS notarization expired mid-release | LOW | Re-notarize; users redownload. |
| Index DB in cloud-synced folder corrupted | LOW (data) / HIGH (trust) | Auto-detect, move to platform data dir, rescan. Communicate. |
| Wrong NFC/NFD on Mac → duplicate pages | MEDIUM | Migration tool: detect duplicates by normalized name, prompt user to merge. |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Re-serialization corruption | M0 | Round-trip property test on all real files in CI |
| 2. Watcher loop | M0 (design) + M3 (impl) | Save-in-app produces zero reparse events |
| 3. FTS5/index drift | M0 (txn design) + M3 (delete detection) | Crash recovery test in CI; integrity check on boot |
| 4. CommonMark quirks vs TAB outliner | M0 | Quirks fixture suite with golden expected outputs |
| 5. CodeMirror 6 focus/IME | M2 | Manual IME test on Pt-BR, JA; automated boundary-key tests |
| 6. Cross-platform paths | M0 (norm) + M4 (Win manifest) | CI matrix Linux/macOS/Windows |
| 7. Logseq compat (properties, drawers, alias, journal title) | M0 (parser) + M1 (resolution/rendering) | Inventory script + edit-every-real-file test |
| 8. Tauri/Wails packaging | M4 (start admin in late M3) | Installer test on three clean OSes |
| 9. Lazy-loading regression | M0 (model) + continuous profiling from M1 | Memory ceiling gates in CI |
| 10. Sync conflict resolution | M3 | Manual two-process edit test |
| 11. Symlinks | M0 | Setting-gated, broken-symlink test |
| 12. Empty/zero-byte files | M1/M2 | Fixture: empty.md round-trips |
| 13. Large files | M1 | Background-thread parse for >1MB files |
| 14. BOM/encoding | M0 | BOM fixture round-trips |
| 15. Backlinks index | M1 | EXPLAIN QUERY PLAN check |
| 16. DB in cloud-synced folder | M0 | DB lives outside notes root by default |
| 17. Journal timezone | M1 | Document chosen tz behavior |

---

## Sources

- **Direct inspection** of `data-folder-sample/Logseq/` (verified `:LOGBOOK:` drawers, TAB indent, `#[[tag]]` patterns in `pages/`).
- **PRD `PRD-outliner-markdown.md`** — RF-50 to RF-56 compat requirements; decisions §5; open questions §12.
- **PROJECT.md** — confirms scope and stack candidates.
- **Training knowledge** (MEDIUM confidence — verify on prototype):
  - CommonMark spec §4.4 (indented code blocks = 4 spaces).
  - Comrak `format_commonmark` documented as best-effort.
  - pulldown-cmark-to-cmark known drift on quote styles, list markers.
  - Tauri 2.x signing/notarization docs.
  - CodeMirror 6 docs on `view.composing`, `Prec`, transaction model.
  - `notify` crate / `fsnotify` Go docs on platform event coalescing.
  - SQLite docs on WAL, FTS5 (contentless-delete since 3.43), `PRAGMA optimize`.
  - Logseq docs (and reverse-engineered behavior) for `config.edn`, alias, namespace `%2F` encoding.
- **Personal experience patterns:** every "markdown-on-disk app" project (Obsidian, Logseq, Foam, Dendron, Athens, Roam-research clones) has hit pitfalls 1–4. They are the *de facto* failure mode of this product category.

---
*Pitfalls research for: local-first markdown outliner with Logseq-base compat*
*Researched: 2026-05-21*
