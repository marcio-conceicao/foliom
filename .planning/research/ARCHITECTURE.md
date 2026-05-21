# Architecture Research — Foliom

**Domain:** Local-first markdown outliner (Logseq-style) with native backend + web UI
**Researched:** 2026-05-21
**Confidence:** MEDIUM-HIGH (architecture follows well-trodden local-app patterns; specific parser strategy informed by direct inspection of `data-folder-sample/Logseq/journals/2023_11_09.md`)

> Scope note: this document answers the Architecture dimension. Module boundaries, data flow, build order, and the open architectural questions from PRD §12.3, §5.4–5.5, and §6.2. Stack selection (Rust vs Go, Tauri vs Wails) is covered in STACK.md — here we only describe the shape, not the language.

---

## 1. System Overview

```
┌───────────────────────────────────────────────────────────────────┐
│  Frontend (browser tab OR Tauri webview)                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────────────────────┐   │
│  │ Outliner   │  │ Renderer   │  │ Search / Backlinks panes  │   │
│  │ (CM6)      │  │ (md→HTML)  │  │                            │   │
│  └─────┬──────┘  └─────┬──────┘  └─────────────┬──────────────┘   │
│        │   HTTP (REST + SSE) over 127.0.0.1    │                  │
└────────┼─────────────────┼──────────────────────┼──────────────────┘
         ▼                 ▼                      ▼
┌───────────────────────────────────────────────────────────────────┐
│  Native backend process (single binary, owns the workspace)       │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ HTTP/SSE server  (routing + JSON serialization)            │  │
│  └───────┬───────────────────┬───────────────────────┬────────┘  │
│          │                   │                       │           │
│  ┌───────▼──────┐   ┌────────▼────────┐   ┌──────────▼────────┐  │
│  │ Query layer  │   │ Mutation layer  │   │ Event broker (SSE)│  │
│  │ (pages,      │   │ (save block,    │   │ — broadcasts      │  │
│  │  backlinks,  │   │  rename page)   │   │  reindex events   │  │
│  │  FTS)        │   │                 │   │                   │  │
│  └───────┬──────┘   └────────┬────────┘   └──────────▲────────┘  │
│          │                   │                       │           │
│  ┌───────▼───────────────────▼───────────────────────┴────────┐  │
│  │ Indexer  (orchestrates: scan → parse → diff → write SQL)   │  │
│  └───────┬──────────────────┬───────────────────────┬─────────┘  │
│          │                  │                       │            │
│  ┌───────▼──────┐   ┌───────▼────────┐   ┌──────────▼─────────┐  │
│  │ Scanner      │   │ Block parser   │   │ Watcher (notify/   │  │
│  │ (walk +      │   │ (segment +     │   │  fsnotify)         │  │
│  │  stat +      │   │  CommonMark    │   │  + debouncer       │  │
│  │  ignore)     │   │  per block)    │   │  + self-write      │  │
│  │              │   │                │   │    suppressor      │  │
│  └──────────────┘   └────────────────┘   └─────────┬──────────┘  │
│                                                    │             │
│  ┌─────────────────────────────────────────────────▼──────────┐  │
│  │ Storage:  SQLite (rusqlite/go-sqlite3)  +  FTS5 vtable    │  │
│  │           Workspace `.foliom/index.db`                     │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                   │
│  Filesystem (canonical):  <workspace>/**/*.md                     │
└───────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Owns | Talks to |
|-----------|------|----------|
| **Scanner** | Recursive walk, ignore lists (`logseq/`, `assets/`, …), `stat` for `mtime`+size, returns list of file refs | Indexer |
| **Block parser** | Reads a single `.md`, segments into blocks by TAB indentation + 2-space continuation, runs CommonMark/GFM on each block's raw text, extracts `[[page]]`/`#tag`/`#[[composite]]` from text nodes only | Indexer |
| **Indexer** | Diff (`mtime`/`hash` per file), drives parser, writes SQLite in a single transaction per file, emits change events | Scanner, Parser, Storage, Event broker |
| **Storage** | SQLite schema, FTS5 vtable, migrations, transactional writes | Indexer, Query layer, Mutation layer |
| **Query layer** | Read-only: page tree, backlinks, FTS search, tag pages. Streams content from disk for the *open* page only | Storage, Filesystem |
| **Mutation layer** | Apply edits → serialize block tree back to `.md` → write file → record `(path, hash)` in self-write set → trigger same-file reindex from cache (in-process, skipping watcher) | Storage, Filesystem, Watcher (suppress set) |
| **Watcher** | OS file events, debounce window, drops events whose `(path, hash)` matches recent self-writes, dispatches dirty paths to Indexer | Indexer, Mutation layer |
| **HTTP server** | REST for queries/mutations, **SSE** for server→client live updates (external file changes, reindex progress) | All of the above; Frontend |
| **Frontend** | Outliner state machine, CM6 for the one editing block, markdown renderer for inactive blocks, subscribes to SSE stream | HTTP server |

**Boundary discipline:** Scanner, Parser, and Storage have **no** knowledge of HTTP. The HTTP layer is a thin adapter over Query/Mutation. This makes M0 (headless CLI) genuinely testable and keeps Tauri integration trivial.

---

## 2. Recommended Project Structure

Language-agnostic skeleton (Rust shown; Go would mirror with `internal/`):

```
foliom/
├── crates/  (or  cmd/ + internal/  in Go)
│   ├── core/                  # No I/O orthogonal to FS+SQLite. No HTTP.
│   │   ├── scanner/           # walk, ignore lists, stat
│   │   ├── parser/            # block segmentation + CommonMark per block
│   │   │   ├── segment.rs     # raw .md → Vec<RawBlock>
│   │   │   ├── ast.rs         # RawBlock → ParsedBlock (tags, links, fts_text)
│   │   │   └── serialize.rs   # BlockTree → .md (round-trip preserving opaque props)
│   │   ├── storage/           # schema, migrations, prepared queries
│   │   │   ├── schema.sql
│   │   │   ├── files.rs
│   │   │   ├── pages.rs
│   │   │   ├── refs.rs
│   │   │   └── fts.rs
│   │   ├── indexer/           # orchestrates scan→parse→write, incremental diff
│   │   ├── watcher/           # notify wrapper + debouncer + self-write suppressor
│   │   └── query/             # page tree, backlinks, search APIs (pure Rust types)
│   ├── server/                # HTTP + SSE adapter over `core::query` / `core::mutation`
│   │   ├── routes/
│   │   └── events.rs          # SSE broker
│   ├── cli/                   # M0 surface: `foliom index`, `foliom search`, `foliom dump-tree`
│   └── desktop/               # M4: Tauri shell that embeds `server` in-process
└── web/
    ├── src/
    │   ├── outliner/          # state machine, key handlers
    │   ├── renderer/          # markdown→HTML for read-only blocks
    │   ├── api/               # fetch wrapper + SSE subscription
    │   └── pages/
    └── ...
```

**Why this shape:**

- `core/` is the only place that touches files or SQLite. Both CLI (M0) and HTTP server (M1+) and Tauri shell (M4) depend on it. Same logic, three entry points.
- `server/` and `cli/` are the *only* places that do JSON/CLI serialization. Keeping that out of `core/` is what allows the M0 milestone to be testable without any UI.
- `desktop/` is intentionally separate from `server/` so the dependency is `desktop → server → core`, never the other way.

---

## 3. Resolving the Open Architectural Questions

### Q1 — Module boundaries

Answered in §1 + §2 above. The hard rule: **dependency arrows only point inward toward `core/`**. The watcher is part of `core/` (it's a file-IO concern), not the server.

### Q2 — SQLite schema: materialize `blocks` or derive at runtime?

**Recommendation: materialize blocks. Per-block FTS5 row, not per-file.** (Confidence: MEDIUM-HIGH)

Rationale:

- **FTS granularity.** RF-31 demands "trecho/contexto e navegação direta ao bloco." If FTS is per-file, hit→block resolution requires re-parsing the file at query time. Per-block FTS rows give you the block id directly. The PRD's own success criterion (navigate to the block) effectively decides this.
- **Backlinks (RF-22).** `refs(source_id, target, type)` is naturally keyed on `source_id = block_id`. Without materialized blocks you have no stable `source_id` to point at — you'd have to reference (file, byte offset) and re-resolve.
- **Cost is bounded.** Materializing blocks only stores: `id, page_id, parent_id, order, raw, hash`. For 5k pages × ~50 blocks/page = 250k rows. SQLite handles this trivially (~tens of MB).
- **Cache invariant is preserved.** The PRD's §5.1 cache-can-be-deleted contract still holds: blocks are derivable from the `.md`. We just choose to keep the derivation persisted instead of recomputed on every query.
- **Per-page raw is also kept on disk** (canonical `.md`), so we don't violate "lazy content loading" (RF-RNF-02): the *parsed* per-block metadata is small; the `raw` column for the block is small (one bullet's text); we only load the full document into UI memory for the open page.

**Schema (concrete):**

```sql
CREATE TABLE files (
  id        INTEGER PRIMARY KEY,
  path      TEXT NOT NULL UNIQUE,    -- workspace-relative, forward slashes
  mtime_ns  INTEGER NOT NULL,
  size      INTEGER NOT NULL,
  hash      BLOB NOT NULL            -- xxh3 or blake3 of file bytes
);

CREATE TABLE pages (
  id        INTEGER PRIMARY KEY,
  file_id   INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  name      TEXT NOT NULL,           -- "May 21st, 2026" for journals; filename-derived for pages
  kind      TEXT NOT NULL,           -- 'journal' | 'page'
  journal_date  TEXT                 -- ISO date for journals, NULL otherwise
);
CREATE UNIQUE INDEX pages_name_idx ON pages(name COLLATE NOCASE);

CREATE TABLE blocks (
  id        INTEGER PRIMARY KEY,
  page_id   INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  parent_id INTEGER REFERENCES blocks(id) ON DELETE CASCADE,
  ord       INTEGER NOT NULL,        -- sibling order
  depth     INTEGER NOT NULL,        -- TAB count, denormalized for fast tree queries
  raw       TEXT NOT NULL,           -- the bullet's raw markdown (incl. multi-line continuation)
  hash      BLOB NOT NULL
);
CREATE INDEX blocks_page_ord_idx ON blocks(page_id, ord);
CREATE INDEX blocks_parent_idx   ON blocks(parent_id);

CREATE TABLE tags (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE refs (
  source_block INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
  target_kind  TEXT NOT NULL,        -- 'page' | 'tag'
  target_id    INTEGER NOT NULL,     -- pages.id or tags.id
  PRIMARY KEY (source_block, target_kind, target_id)
);
CREATE INDEX refs_target_idx ON refs(target_kind, target_id);

-- FTS: external-content variant so the canonical text lives in `blocks.raw`
-- and FTS5 only stores its inverted index.
CREATE VIRTUAL TABLE blocks_fts USING fts5(
  raw,
  content='blocks', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);
-- Maintain via triggers (INSERT/UPDATE/DELETE on blocks → INSERT/DELETE on blocks_fts).
```

`external content` keeps FTS5 disk overhead minimal — it only holds the inverted index, not a duplicate of `raw`.

### Q3 — Watcher → reindex pipeline

**Recommendation:**

1. **Self-write detection by hash-of-just-written, not timestamp.** Timestamp-based suppression is unreliable across filesystems (timer skew, ms-precision on some FSes, Syncthing rewriting the same content with new mtime). Hash is deterministic.

   ```
   On save: compute new_hash; insert (path, new_hash) into SELF_WRITE_SET with TTL ~2s.
   On watcher event for path:
       read file → compute hash
       if (path, hash) in SELF_WRITE_SET: drop event, remove entry
       else: enqueue for reindex
   ```

   The TTL exists only to bound the set's size if the file is later deleted before the event arrives.

2. **Debouncing:** coalesce per-path events with a ~150ms quiet window. Many editors (VS Code, Vim with backup files) generate `CREATE → RENAME → MODIFY` bursts. Use a path-keyed map of pending events; reset the timer on each new event; flush when quiet.

3. **Coalescing rapid edits to the same file:** the debouncer naturally handles this. For directory-level events (move/rename), use `MOVE_FROM` + `MOVE_TO` pairing within the debounce window to detect renames vs delete+create.

4. **Event flow:**
   ```
   notify → raw_event_chan → debouncer (per-path quiet window)
                          → self_write filter (hash check)
                          → indexer.dirty_queue
                          → indexer worker:  re-stat → re-hash → reparse if hash changed
                                          → diff blocks vs existing rows
                                          → transactional update + FTS triggers fire
                                          → emit ChangeEvent on event broker (SSE → clients)
   ```

5. **Crash safety:** the dirty queue is in-memory only. On startup, the full incremental scan (PRD §5.2) catches anything missed. No separate WAL needed beyond SQLite's own.

(Confidence: MEDIUM — the hash-suppression pattern is standard in IDEs and sync tools; specific debounce window is a tuning guess.)

### Q4 — Frontend ↔ backend protocol

**Recommendation: REST + SSE. Not WebSocket, not JSON-RPC.** (Confidence: HIGH for "not WebSocket"; MEDIUM for REST vs JSON-RPC.)

| Concern | Choice | Why |
|---------|--------|-----|
| Queries (page, search, backlinks) | **REST** (`GET /pages/:name`, `GET /search?q=…`) | Cacheable, debuggable in browser devtools, no schema layer needed |
| Mutations (save block, rename page) | **REST** (`PUT /pages/:name/blocks/:id`) | Same |
| Live updates (external file change, reindex progress) | **SSE** (`GET /events` with `text/event-stream`) | One-way server→client is exactly the use case; auto-reconnects natively; trivially proxied; no framing complexity |
| Bidirectional realtime | — | Not needed. Client never pushes streams. |

WebSocket is overkill for single-user localhost and forces you to invent message envelopes and reconnection logic. SSE gives you both for free with a 5-line `EventSource` on the frontend.

JSON-RPC would be reasonable but adds an envelope layer for no payoff at this scale; REST URLs map naturally to pages/blocks.

**Loopback safety:** bind to `127.0.0.1` only, random high port, embed a per-session token in the URL or in an `Authorization` header to defeat DNS-rebinding from malicious websites. (See PITFALLS.md.)

### Q5 — Block parser strategy

**Recommendation: two-stage parse. Stage 1 = bespoke segmenter (NOT CommonMark) operating line-by-line. Stage 2 = CommonMark on each block's raw text.** (Confidence: HIGH — directly validated against the sample file.)

**Why not "parse the whole `.md` as one CommonMark tree":**

The sample file `data-folder-sample/Logseq/journals/2023_11_09.md` is decisive. Block 5 is:

```
	- {{cloze }} ```SQL
	  SELECT
	  	c0_.id AS id_0,
	  	...
	  ``` {{cloze }}
```

This is a single bullet at depth 2 containing a fenced code block. The fence opens on the bullet line and closes ~150 lines later. The continuation lines are indented with **TAB + 2 spaces** (one TAB for the bullet's depth, then 2 spaces of "hanging indent" under the `- `). CommonMark does not natively understand "list continuation by 2-space hanging indent under a TAB-indented bullet" the way Logseq writes it — the spec allows it in principle, but a single-pass CommonMark parser will produce wildly different trees depending on whether you used 2 or 4 spaces of continuation, and will collapse multiple bullets into "tight" lists in ways that destroy the 1:1 block↔bullet mapping the outliner needs.

**Two-stage approach:**

```
Stage 1 — Segmenter (deterministic, line-based):
  Input:  raw .md text
  Output: Vec<RawBlock { depth: u8, raw: String, line_start: u32, line_end: u32 }>

  Algorithm:
    for each line:
      if line matches /^(\t*)- (.*)$/:
        emit previous block (if any)
        start new block at depth = count of leading TABs
        raw = capture group 2  (the text after "- ")
      elif inside a block AND line starts with (depth+1 worth of indent) i.e. tabs + "  ":
        append line to current block's raw (preserving relative indentation)
      elif blank line:
        append to current block (blank lines inside code fences must survive)
      else:
        // either page-level prelude or malformed; collect into a synthetic root block

    Track fenced-code state (```` ``` ````) to suppress bullet-pattern matching
    while inside a code fence — otherwise "- " inside SQL comments would split blocks.

Stage 2 — Block parser:
  For each RawBlock, run CommonMark/GFM on `raw` to produce a per-block AST.
  Extract `[[page]]`, `#tag`, `#[[composite tag]]` from text nodes only
    (RF-21: skip headings, code blocks, hex colors like #fff, URLs).
  Compute fts_text = plain-text projection of the AST.
```

Each bullet is therefore a **mini-document** for CommonMark purposes (RF-16). The tree structure comes from the segmenter, not from CommonMark.

**Block properties (`key:: value`)** are kept inside `raw` opaquely (RF-54). Round-trip serialization is byte-preserving for them.

### Q6 — Tauri/Wails integration

**Recommendation: same backend, in-process under Tauri, not sidecar.** (Confidence: MEDIUM-HIGH if Rust+Tauri; same idea for Go+Wails.)

Two viable patterns:

| Pattern | Description | Verdict |
|---------|-------------|---------|
| **A. In-process HTTP server** | Tauri spawns the Axum/Actix server on a random localhost port on app boot. The webview loads `http://127.0.0.1:<port>/`. | **Pick this.** Web build and desktop build share *one* code path. The frontend doesn't know it's inside Tauri. |
| B. Tauri commands (IPC) | Frontend calls `invoke('get_page', …)`; backend functions are exposed via Tauri's command system. No HTTP. | Forces two API surfaces (REST for web, IPC for desktop) and two frontend transports. Don't do this. |
| C. Sidecar binary | Backend ships as a separate binary that Tauri launches as a child process. | Worth considering only if you want the *exact* same binary to power both `foliom serve` (web mode) and the desktop app. Adds packaging complexity. Defer to M4 decision. |

Pattern A means M4 ("Empacotamento desktop") is genuinely thin: a Tauri shell, a small bit of glue to start the server on a free port before loading the webview, and the existing frontend reused unchanged. Tauri's window chrome and native dialogs (folder picker for selecting workspace) are the only Tauri-specific code.

---

## 4. Data Flow — Three Scenarios

### Scenario A: Cold start

```
1. User launches binary, passes --workspace=/path/to/notes
2. Open SQLite at <workspace>/.foliom/index.db (create if missing, run migrations)
3. Scanner walks tree, applying ignore list. Yields (path, mtime, size) for each .md.
4. Indexer joins against `files` table:
     - new path                            → mark dirty (insert)
     - existing path, mtime/size changed   → mark dirty (update)
     - existing path, unchanged            → skip
     - row exists in DB but not on disk    → mark deleted
5. For each dirty file (parallel, bounded pool):
     read bytes → hash → if hash matches DB row, just update mtime; else parse.
6. Parser produces blocks → Indexer writes in a single tx per file
     (DELETE old blocks for file_id; INSERT new blocks; FTS triggers fire; rebuild refs).
7. Start watcher. Start HTTP server. Open browser / show webview.

Cost: O(N) stat calls + O(K) parses where K = files changed since last open.
```

### Scenario B: User edits a block in the UI

```
1. Click on block → frontend transitions render→edit, mounts CodeMirror with raw.
2. User types. On blur or Enter:
     PUT /pages/Speech%20Analytics/blocks/4271
       body: { raw: "new bullet text\n  continuation line" }
3. Mutation layer:
     a. Load full block tree for the page from `blocks` (small).
     b. Apply edit in-memory.
     c. Serialize tree → markdown (TAB indents + 2-space continuation, RF-50/51).
     d. Compute new_hash of file content.
     e. Insert (path, new_hash) into SELF_WRITE_SET.
     f. Atomic write: write to `path.tmp`, fsync, rename over `path`.
     g. Synchronously reparse the file in-process (skip watcher path) →
        update files/blocks/refs/FTS in one tx.
     h. Publish ChangeEvent{page_id, changed_block_ids} to SSE broker.
4. Watcher receives its event ~50ms later, sees (path, new_hash) in SELF_WRITE_SET,
   drops it.
5. Other open frontend tabs (if any) receive SSE event, refresh their view.
6. Editing tab receives the response, transitions edit→render with the
   newly-parsed HTML for that block.
```

### Scenario C: External edit (git pull, VS Code save)

```
1. notify fires MODIFY on /path/to/notes/pages/Foo.md.
2. Debouncer holds for 150ms. Another event arrives (atomic rename from VS Code) →
   timer resets. Quiet window elapses → emit single event.
3. Self-write filter reads file, hashes, checks SELF_WRITE_SET. Not present → pass through.
4. Indexer: re-stat → hash unchanged? (no, since VS Code wrote it) → reparse.
5. Diff blocks: identify added/removed/modified block ids.
6. Update SQLite in one tx; FTS triggers fire.
7. SSE broker emits ChangeEvent{page_id, changed_block_ids}.
8. Frontend, if viewing that page, fetches affected blocks (or refetches the page)
   and re-renders. If the user is currently editing a block on that page, the UI
   shows a non-destructive "page changed on disk, reload?" prompt — never silently
   overwrite an in-flight edit.
```

---

## 5. Build Order — Mapped to PRD §10 Milestones

The architecture suggests a strict build order. Earlier layers are dependencies of later ones; nothing should be built out of order.

| Order | Component | Milestone | Depends on |
|-------|-----------|-----------|------------|
| 1 | `core/storage` (schema, migrations) | M0 | — |
| 2 | `core/scanner` (walk + ignore + stat) | M0 | — |
| 3 | `core/parser/segment` (line-based block segmenter) | M0 | — (validate against `2023_11_09.md` first) |
| 4 | `core/parser/ast` (CommonMark per block + tag/link extraction) | M0 | (3) |
| 5 | `core/parser/serialize` (block tree → `.md`, round-trip test) | M0 | (3, 4) |
| 6 | `core/indexer` (orchestrator + incremental diff) | M0 | (1, 2, 4) |
| 7 | `cli` (M0 surface: `index`, `reindex`, `search`, `dump-tree`) | M0 | (6) |
| **M0 milestone gate:** CLI can `index` the sample workspace, `search "bom dia"` returns the right block, round-trip `dump-tree → serialize` is byte-identical for unedited files. |
| 8 | `core/query` (page tree, backlinks, FTS as typed API) | M1 | (6) |
| 9 | `server` (HTTP routes + JSON) — read-only endpoints first | M1 | (8) |
| 10 | `web/renderer` (markdown→HTML for blocks; navigation) | M1 | (9) |
| 11 | `web/search` UI + backlinks pane | M1 | (9, 10) |
| **M1 gate:** Browser shows pages read-only, navigation by `[[link]]`/`#tag` works, search returns clickable hits. |
| 12 | `core/mutation` (in-memory tree edit + serialize + atomic write) | M2 | (5, 6) |
| 13 | `server` mutation endpoints | M2 | (12) |
| 14 | `web/outliner` state machine + CM6 single-block editor + key handlers | M2 | (13) |
| **M2 gate:** Can edit, indent, outdent, merge, split bullets. Save round-trips through `.md`. |
| 15 | `core/watcher` (notify + debouncer + self-write suppressor) | M3 | (6, 12) |
| 16 | `server/events` (SSE broker) | M3 | (15) |
| 17 | `web/api` SSE subscription + UI refresh on external change | M3 | (16) |
| **M3 gate:** `git pull` or external VS Code save shows up in the UI within ~200ms; saves from the UI do not loop. |
| 18 | `desktop` (Tauri shell, folder picker, in-process server, window chrome) | M4 | (M1+M2+M3 all green) |
| **M4 gate:** Single binary on macOS/Linux/Windows; RAM and startup beat Logseq on the sample workspace. |

The critical insight: **the parser segmenter (step 3) is the highest-risk component and must be validated against the real sample file before any of the dependent layers are built.** A flaw there cascades into every later milestone.

---

## 6. Anti-Patterns to Avoid

### AP-1: Letting CommonMark drive block segmentation

**What people do:** Parse the whole file with comrak/goldmark, walk the resulting list nodes, treat each `ListItem` as a block.
**Why wrong:** CommonMark's list-tightness rules, lazy-continuation interpretation, and treatment of nested code fences don't match Logseq's "TAB-indented bullet + 2-space continuation" convention. The sample file's SQL-fenced bullet will be misparsed.
**Do instead:** Stage 1 segmenter is line-based and aware of fenced-code state. Stage 2 runs CommonMark on the already-extracted block raw.

### AP-2: Using timestamps to detect self-writes

**What people do:** Remember "I just wrote at time T" and ignore any event with `mtime` close to T.
**Why wrong:** Some filesystems have 1s mtime resolution. Syncthing/Dropbox rewrite files with the same content and a *new* mtime, which you must NOT suppress. Battery-saving filesystems sometimes defer mtime updates.
**Do instead:** Hash-of-just-written set with TTL. Idempotent: external rewrite of identical content is correctly treated as a no-op (hash matches what's already in `files` table).

### AP-3: Loading all pages into memory at startup

**What people do:** "Pre-warm the cache" by reading every `.md` into a `HashMap<PageId, String>`.
**Why wrong:** Defeats RNF-02 ("memória proporcional ao que está aberto") — the entire reason for the project.
**Do instead:** Index-only at startup. Page content is read from disk on demand when the user opens the page. The block index + FTS lets you do search and backlinks without ever loading the full content.

### AP-4: WebSocket for one-way updates

**What people do:** Reach for WebSocket because "realtime."
**Why wrong:** Adds framing, reconnection, and message-envelope concerns for no benefit. The client never streams to the server.
**Do instead:** SSE. `EventSource` is 5 lines on the frontend; reconnection is automatic; events are plain `data: {...}\n\n`.

### AP-5: Two separate IPC surfaces for web and desktop

**What people do:** Use REST for the web build and Tauri `invoke()` commands for the desktop build.
**Why wrong:** Two protocols, two frontend wrappers, two surfaces to keep in sync. Desktop and web diverge over time.
**Do instead:** In-process HTTP server inside Tauri (Pattern A in §3.Q6). Frontend is identical.

### AP-6: Storing block content twice (in `blocks.raw` AND inside FTS)

**What people do:** Default FTS5 setup duplicates the indexed text.
**Why wrong:** Doubles disk usage on what is already the largest table.
**Do instead:** `content='blocks', content_rowid='id'` (external content). FTS5 stores only the inverted index; canonical text lives in `blocks.raw`.

### AP-7: Synchronous reindex blocking the HTTP server

**What people do:** Watcher event → reindex on the request thread → server unresponsive during big imports.
**Why wrong:** Cold-start and bulk renames stall the UI.
**Do instead:** Indexer runs on a dedicated worker (or pool). HTTP handlers enqueue and return; progress is broadcast via SSE.

---

## 7. Integration Points

### External

| Service | Pattern | Notes |
|---------|---------|-------|
| Filesystem | Direct, owned by `core` | Only the backend touches files; frontend never |
| OS file-watch API | `notify` (Rust) / `fsnotify` (Go) | Behaves differently on macOS (FSEvents coalesces) vs Linux (inotify, watch limits) vs Windows (ReadDirectoryChangesW) — abstract behind a single trait |
| Syncthing / git / Dropbox | None (transparent) | They write files; watcher picks them up; self-write filter doesn't suppress (different hash from what *we* wrote) — works for free |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Server ↔ Core | Direct function calls (same process) | Core returns typed values; server serializes |
| Indexer ↔ Watcher | Channel (mpsc) | Watcher pushes dirty paths; indexer pulls |
| Indexer ↔ Event broker | Channel (broadcast) | Indexer publishes ChangeEvents; SSE handler subscribes |
| Mutation ↔ Watcher | Shared `SELF_WRITE_SET` (Arc<Mutex<HashMap<…>>>) | Only place where two components share mutable state — keep the surface small |
| Tauri ↔ Server | In-process (spawn server task in app `setup`) | No IPC envelope, just function call to start the listener |

---

## 8. Scaling Considerations

This is explicitly a single-user app. "Scale" means workspace size, not concurrent users.

| Workspace size | What works as-is | What needs attention |
|----------------|------------------|----------------------|
| < 1k files | Everything | — |
| 1k–10k files (target: Logseq sample = ~600; PRD target = 5k) | Everything | Parallel parsing pool sized to CPU; FTS5 `optimize` after bulk index |
| 10k–100k files | Cold scan is still fast (stat is ~µs per file); incremental reindex works | Memory of the SQLite page cache; consider WAL mode (`PRAGMA journal_mode=WAL`) for concurrent reader + writer; watch out for inotify watch limits on Linux (`/proc/sys/fs/inotify/max_user_watches`) |
| 100k+ | Not a realistic personal-notes scale | If ever needed: shard FTS, batch SSE events, paginate page tree fetches |

### First bottlenecks (in expected order)

1. **inotify watch limit on Linux** at ~10k files. Mitigation: bump the limit at first run (with user consent) or document it; alternatively use a recursive-watch mode where supported.
2. **Parser throughput on cold first-time index.** Mitigation: rayon/goroutine pool; the work is embarrassingly parallel since each file is independent.
3. **FTS query latency** if many tokens. Mitigation: `fts5(..., prefix='2 3 4')` for prefix queries; `unicode61 remove_diacritics 2` is already fast.

---

## Sources

- Direct inspection: `/home/mconceicao/work-others/foliom/PRD-outliner-markdown.md` (§5, §6, §8, §10, §12)
- Direct inspection: `/home/mconceicao/work-others/foliom/data-folder-sample/Logseq/journals/2023_11_09.md` (multi-line SQL code-fence bullet — validates the segmentation strategy)
- Architecture patterns from training data: local-first app patterns (Obsidian, Logseq DB version post-mortems), SSE vs WebSocket for localhost (HTML5 spec for `EventSource`), SQLite FTS5 external-content (sqlite.org documentation), `notify`/`fsnotify` debouncing patterns (commonly used in dev-tool watch loops).
- **Note:** Live web verification was not available during this research (WebSearch denied). Recommendations that depend on current library behavior — specifically Tauri sidecar tooling and the latest `notify` debouncer API — are MEDIUM confidence and should be re-verified in the M0 spike.

---
*Architecture research for: Foliom (local-first markdown outliner)*
*Researched: 2026-05-21*
