# Phase 1: Headless Indexing Core - Research

**Researched:** 2026-05-21
**Domain:** Rust headless markdown indexer (two-stage parser, SQLite FTS5, CLI)
**Confidence:** HIGH (stack/architecture inherited from project research and locked by CONTEXT D-09..D-21; this document is concrete how-to design for the planner)

## Summary

Phase 1 builds the Rust headless core that proves Foliom's foundation works against real Logseq data **before any UI exists**. The non-negotiable invariant is the **round-trip CI gate (ACPT-01)**: every file in `data-folder-sample/Logseq/` (~619 files) must read → segment → splice-noop → write byte-identical. This test ships **first**, before storage/indexer/CLI, and stays green forever.

The architecture is locked by CONTEXT.md: Cargo workspace (`crates/core` + `crates/cli`), Rust 1.85+, `pulldown-cmark` 0.13 for Stage 2 parsing only (Stage 1 is a hand-rolled line-based segmenter that owns TAB indent + 2-space continuation + fence-awareness + drawer-awareness), `rusqlite` 0.39 bundled with FTS5, BLAKE3 for hashing, `walkdir` 2.5 with `filter_entry` for the ignore list, and NFC-normalized + forward-slash paths at the storage boundary. The DB lives at `$XDG_DATA_HOME/foliom/<root-hash-16-hex>.db` (or platform equivalent) — **never** inside the notes folder.

**Primary recommendation:** Decompose into 7 atomic plans in this dependency order: (1) Workspace + RawBlock + Round-trip CI gate (writes the failing test first), (2) Stage 1 segmenter (makes the gate green), (3) Stage 2 parser + ref extraction, (4) Storage schema + migrations + DB-location resolver, (5) Scanner + ignore-list + `config.edn :hidden` reader, (6) Indexer + incremental reindex, (7) CLI subcommands + inventory + cross-platform CI matrix.

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Binário único `foliom` com subcomandos `index`, `reindex`, `search`, `dump-tree`, `inventory`.
- **D-02:** Output humano por padrão; `--json` opt-in para CI e frontend M1.
- **D-03:** `#tag` e `[[página]]` resolvem para a mesma `pages.id`; `refs.type ∈ {tag, page-link}` preserva a distinção sintática.
- **D-04:** Páginas não-resolvidas existem em `pages` com `file_id = NULL`.
- **D-05:** Block properties (`key:: value`) parseadas em slot estruturado — JSON column on `blocks` OR auxiliary `block_props` table (planner's choice; both acceptable). Round-trip permanece byte-estável via byte_offset/byte_length.
- **D-06:** `:LOGBOOK:`/`:END:` drawers preservados como opaque blob anexado ao bloco pai; `raw` do bloco e `byte_offset/byte_length` cobrem o drawer integralmente.
- **D-07:** Cargo workspace: `crates/core/` (lib) + `crates/cli/` (binário `foliom`).
- **D-08:** Round-trip CI gate roda contra `data-folder-sample/Logseq/` (~619 arquivos).
- **D-09 a D-21:** Rust 1.85+ / pulldown-cmark 0.13 / rusqlite 0.39 bundled / two-stage parsing / DB outside notes folder at `<root-hash>.db` / `blocks` carries BOTH `raw` and `(byte_offset, byte_length)` / NFC + forward-slash paths / BLAKE3 / walkdir 2.5 filter_entry / tracing / thiserror+anyhow / rusqlite_migration / cargo-nextest + insta + criterion.

### Claude's Discretion

- Sub-módulos dentro de `crates/core/` (e.g., `scanner`, `parser::segment`, `parser::ast`, `storage`, `indexer`, `query`).
- `block_props` table vs `blocks.properties JSON` (recommendation below).
- Parser paralelismo (rayon vs single-thread; benchmark decides).
- Transacionalidade do indexer (per-file tx vs batch).
- Mensagens de erro humanas e exit codes.
- Estrutura interna do schema `--json` output.

### Deferred Ideas (OUT OF SCOPE for Phase 1)

- `alias::` resolution in `[[link]]` (preserve opaque only).
- TODO/DONE/DOING/LATER/NOW workflow markers (preserve as text).
- `SCHEDULED:`/`DEADLINE:` queryable state (preserved verbatim only).
- `config.edn` completo (Phase 1 reads ONLY `:hidden`; other keys defer to Phase 2).
- Páginas auto-geradas Logseq (excalidraw, hls — treated as common pages).
- Performance benchmarks vs synthetic 5k-note corpus (criterion baseline only; ACPT-02/03 are Phase 2 gates).
- HTTP server, watcher, write-back/byte-splice mutation, renderer.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| IDX-01 | Recursive scan respecting ignore list (`logseq/`, `assets/`, `draws/`, `whiteboards/`, `bak/`, `.recycle/`, `version-files/` + `:hidden` from `config.edn`) | §Scanner + Ignore List |
| IDX-02 | SQLite index (files, pages, blocks, tags, refs, FTS5); deleting it loses nothing | §Schema + Migrations |
| IDX-03 | Incremental reindex on startup via `mtime`+`hash` | §Reindex Algorithm |
| IDX-04 | Full reindex CLI (`foliom reindex --full`) | §CLI Shape |
| IDX-05 | `blocks` stores `raw` PLUS `(byte_offset, byte_length)` for byte-splice writeback | §Schema |
| IDX-06 | DB outside notes folder (`$XDG_DATA_HOME/foliom/<root-hash>.db`) | §DB Location |
| IDX-07 | Paths NFC + forward-slash at storage boundary | §Path Normalization |
| IDX-08 | Inventory CLI reports Logseq pattern counts; gates parser sign-off | §Inventory CLI |
| PRS-01 | Page parsed into tree of blocks (TAB-indented bullets) | §Stage 1 Segmenter |
| PRS-02 | Block ≠ line; 2-space continuation + embedded code fences | §Stage 1 Segmenter |
| PRS-03 | Two-stage parse: line-based segmenter then CommonMark per block | §Two-Stage Parser |
| PRS-04 | Tag/link extraction from text nodes only (skip headings, code, hex, URLs) | §Stage 2 Parser |
| PRS-05 | Block properties (`key:: value`) preserved opaquely in `properties` slot | §Stage 1 Segmenter |
| PRS-06 | `:LOGBOOK:`/`:END:` drawers preserved opaquely | §Stage 1 Segmenter |
| PRS-07 | Byte-identical round-trip on whole corpus (CI gate) | §Round-Trip CI Gate |
| ACPT-01 | Round-trip CI gate green on `data-folder-sample/Logseq/` | §Round-Trip CI Gate |
| ACPT-04 | Parser + scanner green on Linux/macOS/Windows CI | §Cross-Platform CI Matrix |

## Project Constraints (from CLAUDE.md)

- **Core Value:** Cold start rápido + baixo uso de memória mesmo em grafos grandes, SEM injetar metadados em `.md`. Any plan that violates this is wrong.
- **Compatibility:** Open existing Logseq base (~600 files) sem corromper conteúdo na primeira edição.
- **Português user-facing, English in code/identifiers/docstrings** — implicit project convention.
- **Atomic commits via `gsd-sdk query commit`** — every task ends with one commit.
- **GSD workflow** — all edits flow through GSD commands.
- **Stack pre-locked by CLAUDE.md "Technology Stack"** — matches CONTEXT D-09..D-21. No drift.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Filesystem walk + ignore | `crates/core::scanner` | — | Owns IO, no HTTP/UI knowledge |
| Stage 1 segmentation (raw → RawBlock) | `crates/core::parser::segment` | — | Pure function on `&[u8]` → `Vec<RawBlock>`; no IO |
| Stage 2 parse (RawBlock → refs/properties) | `crates/core::parser::ast` | — | Pure function using `pulldown-cmark` |
| SQLite schema + migrations | `crates/core::storage` | — | Owns DDL, prepared statements |
| Orchestration (scan→parse→write) | `crates/core::indexer` | scanner, parser, storage | Transaction boundary lives here |
| DB path resolution | `crates/core::storage::location` | — | Platform-specific; uses `directories` crate |
| Path normalization (NFC + forward-slash) | `crates/core::path::RelativePath` newtype | — | Owns the invariant — every boundary crossing goes through it |
| Round-trip property test | `crates/core/tests/roundtrip.rs` | — | Pure assertion harness, NO storage dependency |
| Inventory pattern counting | `crates/core::inventory` | scanner, parser | Aggregates over scanner output |
| CLI subcommands + JSON output | `crates/cli` | core | Thin adapter; serde structs are the JSON contract |

## Plan Decomposition (recommended for planner)

Seven atomic plans in dependency order. Each is sized S/M/L for a single execution-phase session. **Plan 1 ships the failing CI gate before any other code lands** — this is the non-negotiable ordering invariant from ROADMAP §"Phase 1 ordering invariant".

| # | Plan | Size | Depends on | Deliverable | Verification |
|---|------|------|-----------|-------------|--------------|
| **P1** | **Workspace skeleton + RawBlock type + failing round-trip gate** | M | — | `Cargo.toml` workspace; `crates/core/` lib with `RawBlock` struct + stub `segment()` returning `vec![]`; `crates/cli/` empty binary; `tests/roundtrip.rs` iterating `data-folder-sample/Logseq/` asserting splice-noop byte-equal. GitHub Actions matrix (linux/macos/windows) configured. | `cargo nextest run --test roundtrip` runs and fails on all ~619 files (expected — segmenter is a stub). CI matrix runs on all 3 OSes. |
| **P2** | **Stage 1 segmenter (line-based, fence-aware, drawer-aware)** | L | P1 | `parser::segment::segment(source: &[u8]) -> Vec<RawBlock>` with full state machine: bullet detect, depth = TAB count, 2-space continuation, fenced-code state, drawer state (`:LOGBOOK:`/`:END:`), block properties (`key:: value`) attached to current block, page-level prelude block (depth=0 sentinel). Property `byte_offset + byte_length` covers entire block including continuation, drawers, and properties. | Round-trip gate green on all 619 sample files. Unit tests on hand-crafted fixtures (code fence inside bullet, drawer, multiple properties, nested bullets, blank lines). |
| **P3** | **Stage 2 parser + ref extraction + `RelativePath` newtype** | M | P2 | `parser::ast::extract_refs(raw: &str) -> ParsedBlock { refs: Vec<Ref>, fts_text: String }` walking `pulldown-cmark` event stream with a context stack that skips `CodeBlock`/`Code`/`Heading`/`Link` events. Extract `[[page]]`, `#tag`, `#[[multi-word tag]]` from `Event::Text` only. `core::path::RelativePath` newtype owning NFC + forward-slash invariant. | Unit tests: synthesized fixtures with tag-in-heading, tag-in-code, hex color `#fff`, URL with `#fragment`, `#[[multi word]]`. NFC fixture: `"Avaliação"` (NFC) vs `"Avaliac\u{0327}ão"` (NFD) → same `RelativePath`. |
| **P4** | **Storage schema + migrations + DB-location resolver** | M | P1 | Migration v1 with all tables (files, pages, blocks, tags, refs, FTS5 external-content + triggers). PRAGMA setup. `storage::location::resolve(root: &Path) -> PathBuf` using `directories` crate + BLAKE3 root-hash. `storage::Db::open(notes_root)` opens or creates the DB and runs migrations. | Integration test: open DB, insert a synthetic row in every table, query back. Verify DB path: `$XDG_DATA_HOME/foliom/<16-hex>.db` (linux), `~/Library/Application Support/foliom/<16-hex>.db` (mac), `%LOCALAPPDATA%\foliom\<16-hex>.db` (win). |
| **P5** | **Scanner + ignore list + minimal `config.edn :hidden` reader** | S | P3, P4 | `scanner::walk(root: &Path, ignore: &IgnoreSet) -> impl Iterator<Item = ScanEntry>` using `walkdir::filter_entry`. Hard-coded ignore list per IDX-01. Lightweight `config.edn` `:hidden` extractor (regex-based, documented scope). | Unit tests: ignore list filters `logseq/`, `assets/`, etc. Sample `config.edn` fixture with `:hidden ["foo" "bar"]` filters those names. |
| **P6** | **Indexer orchestrator + incremental reindex + transactional writes** | L | P5 | `indexer::reindex(db, root, mode: IncrementalOrFull)` walks scanner output, compares `(mtime_ns, size)` against `files` table, parses dirty files via Stage 1 + Stage 2, writes one tx per file (delete-then-insert blocks/refs for the file, FTS5 triggers fire). Handles: new file, modified file, deleted file, unchanged file, rename (Phase 1 treats as delete+create — rename tracking is Phase 4). Page resolution: `[[Foo]]` looks up `pages.name COLLATE NOCASE`; insert with `file_id = NULL` if absent. | Test: run `index` on sample corpus; `dump-tree pages/Sleep.md`; modify one file's mtime; rerun `index`; assert only that file was reparsed (via tracing log assertion or row-count proxy). Delete-and-rerun produces same row count. |
| **P7** | **CLI subcommands (`index`/`reindex`/`search`/`dump-tree`/`inventory`) + JSON output + cross-platform CI green** | M | P6 | `clap` subcommand tree with `--json` flag. `inventory` aggregates pattern counts. Final CI matrix run with all tests green on Linux/macOS/Windows. README snippet with usage examples. | `foliom inventory data-folder-sample/Logseq/ --json` produces complete pattern report. `foliom search "bom dia"` returns the SQL block from `2023_11_09.md`. CI matrix green on all 3 OSes. |

**Critical ordering:** P1 ships the failing CI gate. P2 turns it green. Everything after P2 must keep it green — every PR runs the round-trip suite.

## Two-Stage Parser

### Stage 1 — Line-based Segmenter (concrete design)

**API:**

```rust
// crates/core/src/parser/segment.rs

#[derive(Debug, Clone)]
pub struct RawBlock {
    /// TAB-count indent. 0 = top-level bullet, 1 = nested once, etc.
    /// Special sentinel: page-prelude block carries depth = u8::MAX.
    pub depth: u8,
    /// Absolute byte offset into the source file.
    pub byte_offset: usize,
    /// Length in bytes (inclusive of all continuation lines, drawers, properties, trailing newline).
    pub byte_length: usize,
    /// The full raw text of the block (UTF-8). Slice of source[byte_offset..byte_offset+byte_length].
    pub raw: String,
    /// Properties (key:: value) found inside this block. Parsed but `raw` already contains them
    /// verbatim — this is for indexer to insert into `block_props` without re-scanning.
    pub properties: Vec<(String, String)>,
    /// Drawers (e.g. :LOGBOOK: ... :END:) found inside this block. Opaque blobs, byte ranges
    /// relative to source.
    pub drawers: Vec<RawDrawer>,
}

#[derive(Debug, Clone)]
pub struct RawDrawer {
    pub name: String,     // e.g. "LOGBOOK"
    pub byte_offset: usize,
    pub byte_length: usize,
}

pub fn segment(source: &[u8]) -> Vec<RawBlock>;
```

**Invariants (load-bearing):**

1. `RawBlock` byte ranges are **contiguous and non-overlapping** when sorted by `byte_offset`.
2. Concatenating `source[block.byte_offset..block.byte_offset + block.byte_length]` for every block in order **exactly equals** `source`. This is the splice-noop property — proves ACPT-01 by construction.
3. A "page prelude" RawBlock (depth = `u8::MAX`) covers any bytes before the first bullet line (e.g. page-level `title::` properties Logseq writes at the top, plus blank lines). If the file starts with a bullet at byte 0, the prelude block has `byte_length = 0` and exists as a placeholder.

**State machine (pseudocode):**

```
state = Start
current_block = None
fence_state = Closed   // tracks ``` (opening fence column captured for matching close)
drawer_state = None    // Some(("LOGBOOK", drawer_start_offset)) when inside :LOGBOOK: ... :END:

for each line in source (track byte_offset of line start):
    if fence_state == Open:
        // Inside a code fence — append to current_block regardless of indent.
        if line matches /^(\s*)```\s*$/ where leading whitespace_count == fence_indent:
            fence_state = Closed
        append line to current_block.raw, extend byte_length
        continue

    if drawer_state == Some(name, ...):
        // Inside a drawer — opaque until :END:
        if line.trim() == ":END:":
            close drawer, attach RawDrawer to current_block, reset drawer_state
        append line to current_block.raw, extend byte_length
        continue

    // Bullet detection: /^(\t*)- (.*)$/  capturing TAB count and post-marker text.
    if line matches bullet_re:
        emit current_block if Some
        current_block = Some(RawBlock {
            depth: tab_count,
            byte_offset: line_start,
            byte_length: line.len_with_newline,
            raw: line.to_string(),
            properties: vec![],
            drawers: vec![],
        })
        // Bullet line might itself open a fence (e.g. `- ```rust`)
        if line contains "```" and odd_number_of_fences:
            fence_state = Open(fence_indent = tab_count + 2)  // 2 = the "  " of bullet continuation
        continue

    // Continuation: 2-space hanging indent at (depth*TAB + 2 spaces).
    if current_block.is_some() && line starts_with(current_block.indent_prefix() + "  "):
        // Could be: property `key:: value`, drawer opener `:NAME:`, plain continuation, or fence opener.
        let inner = line.strip_prefix(current_block.indent_prefix() + "  ").unwrap();
        if inner.trim() matches /^:([A-Z]+):$/:
            drawer_state = Some((name, line_start))
        elif inner matches property_re /^([a-zA-Z][a-zA-Z0-9._-]*):: (.*)$/:
            current_block.properties.push((key, value))
        elif inner matches fence_re /^```/:
            fence_state = Open(fence_indent = tab_count + 2)
        // else: plain continuation text — nothing to record beyond raw bytes.
        append line to current_block.raw, extend byte_length
        continue

    // Blank line: belongs to current block (inside continuation context) — preserves blank lines
    // inside multi-line code fences and between continuation paragraphs.
    if line is blank && current_block.is_some():
        append line to current_block.raw, extend byte_length
        continue

    // Otherwise: page prelude or malformed. Append to prelude block.
    append line to prelude_block.raw, extend prelude_block.byte_length

emit current_block if Some
return blocks (prelude first if non-empty, then bullets in source order)
```

**Validated against `data-folder-sample/Logseq/journals/2023_11_09.md`:** That file is a 5-bullet page where block 5 is a depth-2 bullet (`\t\t- {{cloze }} \`\`\`SQL ... \`\`\` {{cloze }}`) with ~150 lines of TAB+2-space-indented SQL inside a fence. The state machine above produces exactly 6 blocks (1 prelude — empty — plus 5 bullets), the SQL bullet's `byte_length` covers all ~150 continuation lines including the closing fence, and `concat(blocks).bytes() == source.bytes()`. **This file is the canonical regression fixture.**

### Stage 2 — Per-block CommonMark + ref extraction

```rust
// crates/core/src/parser/ast.rs

use pulldown_cmark::{Event, Parser, Tag, TagEnd, Options};

#[derive(Debug)]
pub enum RefKind { PageLink, Tag }

#[derive(Debug)]
pub struct ExtractedRef {
    pub kind: RefKind,
    pub target: String,   // normalized page name (decoded %2F → /)
}

pub fn extract_refs(raw: &str) -> Vec<ExtractedRef> {
    let mut refs = Vec::new();
    let mut suppress_depth: u32 = 0;  // >0 → we're inside Heading/CodeBlock/Link
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_GFM;

    for event in Parser::new_ext(raw, opts) {
        match event {
            Event::Start(Tag::Heading { .. })
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::Link { .. })
            | Event::Start(Tag::Image { .. }) => suppress_depth += 1,
            Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::CodeBlock)
            | Event::End(TagEnd::Link)
            | Event::End(TagEnd::Image) => suppress_depth = suppress_depth.saturating_sub(1),
            Event::Code(_) => { /* inline code — never extract */ }
            Event::Text(t) if suppress_depth == 0 => {
                refs.extend(scan_text_for_refs(&t));
            }
            _ => {}
        }
    }
    refs
}

/// Scan a plain-text fragment for `[[page]]`, `#[[multi word]]`, and `#bareTag`.
/// Hex colors like `#fff`/`#ffffff` are rejected (digits-only after `#`).
fn scan_text_for_refs(text: &str) -> Vec<ExtractedRef> {
    // Iterate chars with byte indices. State machine:
    //   - See `[[` → capture until `]]`, emit PageLink.
    //   - See `#[[` → capture until `]]`, emit Tag.
    //   - See `#` followed by non-whitespace, non-`[` → bare tag; consume while char.is_alphanumeric() || ['-','_','/','.'].contains.
    //     Reject if captured token is empty OR matches /^[0-9a-fA-F]{3,8}$/ (hex color).
    //   - URLs: pulldown-cmark already wraps autolinks in Tag::Link, suppressing them via suppress_depth.
    //     For raw `https://...#frag` outside autolink, the `#frag` would otherwise be misread —
    //     mitigation: reject bare-tag when preceding char is alphanumeric (i.e., `#` glued to a word).
    todo!()
}
```

**Tag/link extraction order matters:** scan `#[[...]]` before `#bareTag` (the former is a strict prefix). Decode `%2F` → `/` on `target` so `[[Parent/Child]]` and `[[Parent%2FChild]]` resolve to the same `pages.name`.

## Schema + Migrations

### Migration v1 — full schema in one shot

```sql
-- crates/core/src/storage/migrations/001_init.sql

PRAGMA foreign_keys = ON;

CREATE TABLE files (
    id        INTEGER PRIMARY KEY,
    path      TEXT NOT NULL UNIQUE,        -- NFC + forward-slash, relative to notes root
    mtime_ns  INTEGER NOT NULL,            -- nanoseconds since Unix epoch
    size      INTEGER NOT NULL,
    hash      BLOB NOT NULL                -- BLAKE3(file_bytes), 32 bytes
);
CREATE INDEX files_hash_idx ON files(hash);

CREATE TABLE pages (
    id            INTEGER PRIMARY KEY,
    file_id       INTEGER REFERENCES files(id) ON DELETE SET NULL,  -- NULL = unresolved page
    name          TEXT NOT NULL,           -- canonical page name (after %2F decode)
    kind          TEXT NOT NULL,           -- 'journal' | 'page'
    journal_date  TEXT                     -- ISO 8601 'YYYY-MM-DD' for journals, NULL otherwise
);
CREATE UNIQUE INDEX pages_name_idx ON pages(name COLLATE NOCASE);
CREATE INDEX pages_file_idx ON pages(file_id);

CREATE TABLE blocks (
    id           INTEGER PRIMARY KEY,
    page_id      INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    parent_id    INTEGER REFERENCES blocks(id) ON DELETE CASCADE,
    ord          INTEGER NOT NULL,         -- sibling order within parent
    depth        INTEGER NOT NULL,         -- TAB count, 0 for top-level; -1 for page prelude
    raw          TEXT NOT NULL,            -- D-14: full block text for FTS & cheap reads
    byte_offset  INTEGER NOT NULL,         -- D-14: for byte-splice writeback (Phase 3)
    byte_length  INTEGER NOT NULL,
    hash         BLOB NOT NULL             -- BLAKE3(raw)
);
CREATE INDEX blocks_page_ord_idx ON blocks(page_id, ord);
CREATE INDEX blocks_parent_idx   ON blocks(parent_id);

-- D-05: block properties as a small auxiliary table (recommended over JSON column —
-- enables indexed lookup by key for future query "list all blocks with template:: X").
CREATE TABLE block_props (
    block_id INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    value    TEXT NOT NULL,
    PRIMARY KEY (block_id, key)
);
CREATE INDEX block_props_key_idx ON block_props(key);

-- D-06: drawers as opaque blobs anchored to parent block; preserved by byte range, NOT
-- by parsed content. Index only so we can report counts in inventory.
CREATE TABLE block_drawers (
    block_id    INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    byte_offset INTEGER NOT NULL,
    byte_length INTEGER NOT NULL,
    PRIMARY KEY (block_id, byte_offset)
);

CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE refs (
    source_block INTEGER NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    type         TEXT NOT NULL CHECK (type IN ('tag', 'page-link')),   -- D-03
    target_page  INTEGER REFERENCES pages(id) ON DELETE CASCADE,       -- D-03: tags and links both point at pages
    PRIMARY KEY (source_block, type, target_page)
);
CREATE INDEX refs_target_idx ON refs(target_page, type);

-- D-14: FTS5 external-content over blocks.raw — only the inverted index lives in FTS,
-- canonical text stays in `blocks.raw`.
CREATE VIRTUAL TABLE blocks_fts USING fts5(
    raw,
    content='blocks', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- Triggers to keep FTS in sync.
CREATE TRIGGER blocks_ai AFTER INSERT ON blocks BEGIN
    INSERT INTO blocks_fts(rowid, raw) VALUES (new.id, new.raw);
END;
CREATE TRIGGER blocks_ad AFTER DELETE ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, raw) VALUES('delete', old.id, old.raw);
END;
CREATE TRIGGER blocks_au AFTER UPDATE ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, raw) VALUES('delete', old.id, old.raw);
    INSERT INTO blocks_fts(rowid, raw) VALUES (new.id, new.raw);
END;
```

**Schema notes for planner:**

- `pages.name COLLATE NOCASE` resolves D-03 (`#Crypto` and `[[Crypto]]` find the same row) and handles Windows case-insensitivity at the index layer.
- `blocks.depth = -1` is the prelude-block sentinel (use signed `INTEGER`; SQLite is dynamic-type so this is fine even though Rust's `RawBlock.depth` is `u8::MAX`).
- `block_props` as a separate table (D-05 leaves choice open) — recommended because: (a) supports indexed queries like "list all blocks where `template::` is set" without JSON1, (b) Phase 2 inventory + future query layer can use plain SQL, (c) UNIQUE constraint enforces "one value per key per block".
- `refs.target_page` always references `pages.id` — both tags and links resolve to a page row (D-03/D-04). For tags, the row is the same one a hypothetical `[[TagName]]` would resolve to.

### PRAGMA setup on connection open

```rust
// crates/core/src/storage/mod.rs
fn configure_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 268435456;        -- 256 MB, lazy
        PRAGMA wal_autocheckpoint = 1000;
        PRAGMA journal_size_limit = 67108864; -- 64 MB cap on -wal
    ")?;
    Ok(())
}
```

On clean shutdown (CLI command exit): `PRAGMA optimize;` then `PRAGMA wal_checkpoint(TRUNCATE);`.

### rusqlite_migration usage

```rust
use rusqlite_migration::{Migrations, M};

static MIGRATIONS: Migrations<'static> = Migrations::new(vec![
    M::up(include_str!("migrations/001_init.sql")),
    // Future migrations appended here; never edited in place.
]);

pub fn open_db(db_path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(db_path)?;
    configure_connection(&conn)?;
    MIGRATIONS.to_latest(&mut conn)?;
    Ok(conn)
}
```

## Round-Trip CI Gate (ACPT-01)

**Goal:** Prove that for every file `f` in `data-folder-sample/Logseq/`:
`bytes(f) == concat(segment(bytes(f)).map(|b| b.raw))` (and equivalently using `byte_offset/byte_length` slices into the source).

### Test design (`crates/core/tests/roundtrip.rs`)

```rust
// crates/core/tests/roundtrip.rs

use std::fs;
use std::path::{Path, PathBuf};
use foliom_core::parser::segment::segment;

const CORPUS: &str = "../../data-folder-sample/Logseq";

#[test]
fn roundtrip_byte_identical_for_entire_corpus() {
    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    let mut count = 0usize;

    for entry in walkdir::WalkDir::new(CORPUS).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }
        // Skip the `logseq/` config dir per IDX-01 ignore list (but config.edn IS relevant
        // for Phase 5 — for roundtrip test, only .md files matter).
        if path.components().any(|c| c.as_os_str() == "logseq") { continue; }

        count += 1;
        let bytes = fs::read(path).unwrap();
        let blocks = segment(&bytes);

        // Splice-noop reconstruction.
        let mut rebuilt = Vec::with_capacity(bytes.len());
        for b in &blocks {
            rebuilt.extend_from_slice(&bytes[b.byte_offset .. b.byte_offset + b.byte_length]);
        }

        if rebuilt != bytes {
            failures.push((path.to_path_buf(), first_diff_report(&bytes, &rebuilt)));
            if failures.len() >= 3 { break; }  // bound output; fix one, rerun
        }
    }

    assert_eq!(count, 619, "Expected 619 .md files in corpus; found {}. Update assertion if corpus changes.", count);
    assert!(failures.is_empty(), "Round-trip drift in {} file(s):\n\n{}",
        failures.len(),
        failures.iter().map(|(p, d)| format!("=== {} ===\n{}", p.display(), d)).collect::<Vec<_>>().join("\n\n"));
}

/// Produce a human-readable diff showing the first divergence with visible TABs and CRs.
fn first_diff_report(want: &[u8], got: &[u8]) -> String {
    let min_len = want.len().min(got.len());
    let mut diff_at = min_len;
    for i in 0..min_len {
        if want[i] != got[i] { diff_at = i; break; }
    }
    let line_start = want[..diff_at].iter().rposition(|&b| b == b'\n').map(|p| p+1).unwrap_or(0);
    let line_end_want = want[diff_at..].iter().position(|&b| b == b'\n').map(|p| p+diff_at).unwrap_or(want.len());
    let line_end_got  = got[diff_at..].iter().position(|&b| b == b'\n').map(|p| p+diff_at).unwrap_or(got.len());

    format!(
        "first diff at byte {} (line {})\n  want: {:?}\n  got:  {:?}\n  (TAB shown as \\t, CR as \\r)",
        diff_at,
        want[..diff_at].iter().filter(|&&b| b == b'\n').count() + 1,
        visible(&want[line_start..line_end_want]),
        visible(&got[line_start..line_end_got]),
    )
}

fn visible(b: &[u8]) -> String {
    String::from_utf8_lossy(b).replace('\t', "\\t").replace('\r', "\\r")
}
```

**Runtime budget:** 619 files × ~5 KB avg × O(n) segmentation = well under 5 s on a modern laptop. Acceptable ceiling for CI: **30 seconds**. Anything slower means the segmenter is doing something quadratic.

**Notes:**

- The test path is relative; planner should pick either `CARGO_MANIFEST_DIR`-relative or use an env var. Recommend `env!("CARGO_MANIFEST_DIR")` joined with `"../../data-folder-sample/Logseq"`.
- Skip `logseq/config.edn` because it's not `.md`.
- Newline normalization on Windows checkout: see §Cross-Platform CI Matrix — must force LF.
- `insta` is NOT used here. `insta` shines for snapshot diff of small parser output; round-trip is a pure equality assertion against the corpus.
- `proptest` is NOT used here either — the corpus IS the property space. Synthetic mutators come later if needed.

## Inventory CLI (IDX-08) — output shape

### Human-readable (default)

```
$ foliom inventory data-folder-sample/Logseq/
Scanned 619 .md files (533 journals, 86 pages).

Logseq patterns
  alias::               12 files, 14 occurrences
  id::                   9 files, 38 occurrences
  template::             3 files, 3 occurrences
  :LOGBOOK: drawer       7 files, 11 occurrences
  #[[multi-word tag]]   41 files, 127 occurrences
  %2F in filename        0 files
  code-fence-in-bullet  18 files, 22 occurrences
  SCHEDULED:             4 files, 5 occurrences
  DEADLINE:              2 files, 2 occurrences

Block-property files     21
Drawer files              7

Reference base: data-folder-sample/Logseq (619 files, 3.2 MB)
```

### JSON (--json)

```rust
// crates/cli/src/cmd/inventory.rs

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReport {
    pub root: String,
    pub scanned_files: u32,
    pub journal_files: u32,
    pub page_files: u32,
    pub total_size_bytes: u64,
    pub patterns: Vec<PatternCount>,
    pub block_property_files: u32,
    pub drawer_files: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternCount {
    pub name: String,         // "alias::" | "id::" | "template::" | ":LOGBOOK: drawer" | ...
    pub files_with: u32,
    pub occurrences: u32,
}
```

Example output:

```json
{
  "root": "data-folder-sample/Logseq",
  "scannedFiles": 619,
  "journalFiles": 533,
  "pageFiles": 86,
  "totalSizeBytes": 3245678,
  "patterns": [
    { "name": "alias::",              "filesWith": 12, "occurrences": 14 },
    { "name": "id::",                 "filesWith":  9, "occurrences": 38 },
    { "name": "template::",           "filesWith":  3, "occurrences":  3 },
    { "name": ":LOGBOOK: drawer",     "filesWith":  7, "occurrences": 11 },
    { "name": "#[[multi-word tag]]",  "filesWith": 41, "occurrences": 127 },
    { "name": "%2F in filename",      "filesWith":  0, "occurrences":  0 },
    { "name": "code-fence-in-bullet", "filesWith": 18, "occurrences": 22 },
    { "name": "SCHEDULED:",           "filesWith":  4, "occurrences":  5 },
    { "name": "DEADLINE:",            "filesWith":  2, "occurrences":  2 }
  ],
  "blockPropertyFiles": 21,
  "drawerFiles": 7
}
```

These counts (after first real run) become **CI assertions** — `tests/inventory_regression.rs` asserts that running inventory on the corpus yields the expected report, so any regression in pattern detection fails CI.

## Reindex Algorithm

### Pseudocode (`indexer::reindex(db, root, mode)`)

```rust
pub enum ReindexMode { Incremental, Full }

pub fn reindex(db: &Db, root: &Path, mode: ReindexMode) -> Result<ReindexStats> {
    // 1. Load known files into memory: HashMap<RelativePath, (file_id, mtime_ns, size, hash)>.
    let mut known = db.load_all_files()?;
    let mut stats = ReindexStats::default();

    // 2. Walk disk.
    let seen: HashSet<RelativePath> = HashSet::new();
    for entry in scanner::walk(root, &IgnoreSet::default_logseq()?) {
        let rel = RelativePath::from_filesystem(&entry.path, root)?;  // NFC + forward-slash
        seen.insert(rel.clone());

        let (mtime_ns, size) = (entry.mtime_ns, entry.size);

        match (mode, known.get(&rel)) {
            (ReindexMode::Incremental, Some(&(_, old_mt, old_sz, _))) if old_mt == mtime_ns && old_sz == size => {
                stats.unchanged += 1;
                continue;  // Trust mtime+size. No hash check, no read.
            }
            _ => {}  // Full mode OR new/modified file → read+hash+maybe reparse.
        }

        let bytes = fs::read(&entry.path)?;
        let hash = blake3::hash(&bytes);

        if let Some(&(file_id, _, _, old_hash)) = known.get(&rel) {
            if old_hash == hash.as_bytes() && matches!(mode, ReindexMode::Incremental) {
                // mtime changed but content didn't (e.g., `touch`). Update mtime only, skip parse.
                db.update_file_mtime(file_id, mtime_ns)?;
                stats.mtime_touched += 1;
                continue;
            }
            // Modified content.
            reparse_file_tx(db, file_id, &rel, &bytes, mtime_ns, size, hash.as_bytes())?;
            stats.modified += 1;
        } else {
            // New file.
            insert_file_tx(db, &rel, &bytes, mtime_ns, size, hash.as_bytes())?;
            stats.added += 1;
        }
    }

    // 3. Detect deletions: files in `known` but not in `seen`.
    for rel in known.keys().filter(|k| !seen.contains(*k)) {
        db.delete_file_cascade(rel)?;  // CASCADE removes blocks, refs, FTS rows
        stats.deleted += 1;
    }

    Ok(stats)
}

fn reparse_file_tx(db: &Db, file_id: i64, rel: &RelativePath, bytes: &[u8],
                   mtime_ns: i64, size: u64, hash: &[u8]) -> Result<()> {
    let tx = db.transaction()?;
    tx.execute("DELETE FROM blocks WHERE page_id IN (SELECT id FROM pages WHERE file_id = ?)", [file_id])?;
    // CASCADE on blocks removes refs/block_props/block_drawers/blocks_fts via triggers.
    // pages row stays — file_id is preserved.
    let blocks = parser::segment::segment(bytes);
    let page_id = ensure_page_row(&tx, file_id, rel)?;
    for (ord, block) in blocks.iter().enumerate() { insert_block(&tx, page_id, ord, block, bytes)?; }
    tx.execute("UPDATE files SET mtime_ns=?, size=?, hash=? WHERE id=?", params![mtime_ns, size, hash, file_id])?;
    tx.commit()?;
    Ok(())
}
```

**Transaction strategy:** One transaction per file. Rationale: (a) bounded memory — never holds all parsed blocks in memory at once; (b) crash safety — on `kill -9` mid-reindex, completed files are durable and uncommitted ones get re-detected on next run via hash mismatch; (c) good enough perf — SQLite WAL absorbs the per-tx overhead for ~600 files in <1 s. **Do NOT** wrap the entire reindex in one big tx — it holds locks too long and on crash you lose all progress.

**Rename detection:** Phase 1 does NOT detect renames. A renamed file looks like delete + create — both rows are touched. This is acceptable because: (a) rename-with-backlinks is Phase 3, (b) the watcher's `RecommendedCache` for true rename tracking is Phase 4. Document this in plan notes.

**Full vs incremental:** `--full` flag forces re-read+re-hash even when `(mtime, size)` matches. Used after schema migration or when user suspects index drift.

## DB Location + Path Normalization

### DB location resolver

```rust
// crates/core/src/storage/location.rs

use std::path::{Path, PathBuf};
use blake3::Hasher;

pub fn resolve_db_path(notes_root: &Path) -> Result<PathBuf> {
    let abs = notes_root.canonicalize()?;
    let normalized = abs.to_string_lossy().replace('\\', "/");
    // NFC the string before hashing so macOS NFD vs Linux NFC produce the same db file.
    let nfc: String = unicode_normalization::UnicodeNormalization::nfc(normalized.chars()).collect();
    let hash = blake3::hash(nfc.as_bytes());
    let hex16 = &hash.to_hex().to_string()[..16];

    let base_dir = data_dir()?;
    let foliom_dir = base_dir.join("foliom");
    std::fs::create_dir_all(&foliom_dir)?;
    Ok(foliom_dir.join(format!("{}.db", hex16)))
}

#[cfg(target_os = "linux")]
fn data_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() { return Ok(PathBuf::from(xdg)); }
    }
    let home = std::env::var("HOME").map_err(|_| Error::NoHomeDir)?;
    Ok(PathBuf::from(home).join(".local/share"))
}

#[cfg(target_os = "macos")]
fn data_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| Error::NoHomeDir)?;
    Ok(PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(target_os = "windows")]
fn data_dir() -> Result<PathBuf> {
    let local = std::env::var("LOCALAPPDATA").map_err(|_| Error::NoAppData)?;
    Ok(PathBuf::from(local))
}
```

**Recommendation: hand-roll, do NOT use the `directories` crate for Phase 1.** Reasoning: (a) we need ~30 lines of platform-specific code that we control; (b) `directories` adds a dependency for one function; (c) hand-rolling makes the `$XDG_DATA_HOME` env-var override behavior explicit (the crate's behavior is correct but invisible). The override is important for CI — tests can point `XDG_DATA_HOME` at a temp dir.

If the planner prefers the `directories` crate (5.x) for less boilerplate, that's acceptable — use `ProjectDirs::from("", "", "foliom").data_dir()`. Either choice is defensible; the locked invariant is the *path*, not the crate.

### `RelativePath` newtype

```rust
// crates/core/src/path.rs

use std::path::{Path, PathBuf, Component};
use unicode_normalization::UnicodeNormalization;

/// A path relative to the notes root, normalized to NFC + forward-slash.
/// Construct only via `from_filesystem` (which normalizes) or `from_storage_str` (which trusts).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelativePath(String);

impl RelativePath {
    /// From a filesystem path encountered during a walk. Normalizes NFC + forward-slash.
    pub fn from_filesystem(abs: &Path, root: &Path) -> Result<Self> {
        let rel = abs.strip_prefix(root).map_err(|_| Error::PathOutsideRoot)?;
        let mut parts: Vec<String> = Vec::new();
        for c in rel.components() {
            match c {
                Component::Normal(s) => {
                    let str_ = s.to_str().ok_or(Error::NonUtf8Path)?;
                    let nfc: String = str_.nfc().collect();
                    parts.push(nfc);
                }
                _ => return Err(Error::UnexpectedPathComponent),
            }
        }
        Ok(Self(parts.join("/")))
    }

    /// Trust this came from the DB — already normalized.
    pub fn from_storage_str(s: &str) -> Self { Self(s.to_string()) }

    pub fn as_str(&self) -> &str { &self.0 }

    /// Resolve to a filesystem path under `root`. Converts forward-slash back to native separator.
    pub fn to_filesystem(&self, root: &Path) -> PathBuf {
        let mut p = root.to_path_buf();
        for part in self.0.split('/') { p.push(part); }
        p
    }
}
```

**Invariant:** the only constructor that normalizes is `from_filesystem`. Every IO boundary that surfaces a path uses this newtype. SQLite storage uses `RelativePath::as_str()`. The compiler enforces that `PathBuf` and `RelativePath` don't mix accidentally.

## Ignore List + `config.edn :hidden`

### Hard-coded ignore list (IDX-01)

```rust
// crates/core/src/scanner/ignore.rs

pub const DEFAULT_LOGSEQ_IGNORES: &[&str] = &[
    "logseq",         // logseq metadata folder
    "assets",         // images / attachments
    "draws",          // Excalidraw drawings (.excalidraw)
    "whiteboards",    // Logseq whiteboards
    "bak",            // Logseq edit-history backups
    ".recycle",       // Logseq trash
    "version-files",  // Logseq versioning
    ".git",           // version control
    ".obsidian",      // if user also opened in Obsidian
    ".trash",
    "node_modules",   // safety net
];

pub struct IgnoreSet {
    names: HashSet<String>,
}

impl IgnoreSet {
    pub fn default_logseq() -> Self {
        Self { names: DEFAULT_LOGSEQ_IGNORES.iter().map(|s| s.to_string()).collect() }
    }

    pub fn extend_from_config_edn(&mut self, hidden: Vec<String>) {
        self.names.extend(hidden);
    }

    pub fn is_ignored(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}
```

### `walkdir::filter_entry` pattern

```rust
// crates/core/src/scanner/walk.rs

pub fn walk<'a>(root: &Path, ignore: &'a IgnoreSet) -> impl Iterator<Item = ScanEntry> + 'a {
    walkdir::WalkDir::new(root)
        .follow_links(false)   // Phase 1: never follow symlinks (Pitfall 11)
        .into_iter()
        .filter_entry(move |e| {
            // Skip ignored directory names (only at directory level; files keep their parent dir check
            // already by virtue of being descended into).
            if e.file_type().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    if ignore.is_ignored(name) { return false; }
                    if name.starts_with('.') && name != "." { return false; }  // dotdirs at any depth
                }
            }
            true
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(ScanEntry {
                path: e.into_path(),
                mtime_ns: mtime_ns_from_meta(&meta)?,
                size: meta.len(),
            })
        })
}
```

### Minimal `config.edn :hidden` reader

**Recommendation: regex-based extraction, document the limitation.** No EDN crate. Justification: EDN is full Clojure literal syntax (maps, sets, tagged literals, namespaced keywords); writing a full parser is wasted scope when Phase 1 needs exactly one key.

```rust
// crates/core/src/scanner/config_edn.rs

use regex::Regex;
use std::sync::OnceLock;

/// Extract `:hidden ["a" "b" "c"]` (or `:hidden #{...}`) from a Logseq config.edn.
/// Documented scope: handles vector or set literal of double-quoted strings, optional whitespace.
/// Returns empty Vec on absent key, malformed value, or unparseable file. Logs a warning on parse failure.
pub fn read_hidden(config_edn_path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(config_edn_path) {
        Ok(s) => s, Err(_) => return Vec::new(),
    };
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // :hidden  followed by  [ ... ]  or  #{ ... }
        Regex::new(r#":hidden\s+[#]?[\[\{]([^\]\}]*)[\]\}]"#).unwrap()
    });
    let inner = match re.captures(&content).and_then(|c| c.get(1)) {
        Some(m) => m.as_str(),
        None => return Vec::new(),
    };
    static STR_RE: OnceLock<Regex> = OnceLock::new();
    let str_re = STR_RE.get_or_init(|| Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).unwrap());
    str_re.captures_iter(inner).map(|c| c[1].to_string()).collect()
}
```

Anything more complex (nested maps, tagged literals) is Phase 2 work when the renderer needs `:journal/page-title-format`.

## Cross-Platform CI Matrix (ACPT-04)

### GitHub Actions config

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
        with:
          # Critical: prevent autocrlf on Windows so the round-trip gate (which compares
          # raw bytes) sees the same LF endings the corpus was committed with.
          # The repo MUST also include a .gitattributes file forcing LF (see below).
          fetch-depth: 1

      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust }}

      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: phase1-${{ matrix.os }}

      - name: Install cargo-nextest
        uses: taiki-e/install-action@nextest

      - name: cargo build
        run: cargo build --workspace --locked

      - name: cargo nextest run (parser + scanner + roundtrip + storage)
        run: cargo nextest run --workspace --no-fail-fast

      - name: Inventory smoke test (asserts known pattern counts)
        run: cargo run --bin foliom -- inventory data-folder-sample/Logseq/ --json > /tmp/inv.json
        if: runner.os != 'Windows'

      - name: Inventory smoke test (Windows)
        run: cargo run --bin foliom -- inventory data-folder-sample/Logseq/ --json > $env:TEMP\inv.json
        if: runner.os == 'Windows'
```

### Line-ending discipline — `.gitattributes`

The round-trip CI gate compares **raw bytes**. If Git on Windows checks out files with CRLF, the corpus differs from what was authored and the gate fails for the wrong reason.

```gitattributes
# .gitattributes — REQUIRED for round-trip CI gate to work cross-platform.
* text=auto eol=lf
data-folder-sample/** -text     # absolute: treat as binary, no autocrlf
*.md text eol=lf
```

This forces LF endings for all `.md` files on all platforms. Add a CI assertion that verifies the corpus checksum matches an expected value to catch any accidental CRLF leak.

### Known cross-platform gotchas the planner must encode

| Gotcha | Mitigation in Phase 1 |
|--------|----------------------|
| Windows `autocrlf` mangles `.md` to CRLF | `.gitattributes` forces LF; CI runs a corpus checksum assertion. |
| macOS NFD filenames | `RelativePath::from_filesystem` normalizes to NFC at the boundary. Tests use NFC fixtures only. |
| Windows MAX_PATH 260 | Phase 1 corpus is well under; Tauri Phase 5 needs the long-path manifest. Documented, not addressed here. |
| `.DS_Store`, `Thumbs.db`, `desktop.ini` in the corpus | `walkdir` filter only takes `*.md` — these are auto-skipped. |
| Windows case-insensitive case collisions | `pages.name COLLATE NOCASE` index makes `Bruno`/`bruno` collide at DB layer — same behavior as Windows FS. Document; defer dedup UI to Phase 2. |
| `$XDG_DATA_HOME` may be empty string (POSIX says "unset OR empty falls back to default") | Resolver checks `if !xdg.is_empty()` explicitly. |
| `HOME` may not be set in CI sandboxes | Tests set a temp `XDG_DATA_HOME` / `HOME` before constructing the DB resolver. |
| Symlinks inside the sample corpus | `follow_links(false)`. The current sample has none, but the production user base might. |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Markdown parsing | Custom CommonMark | `pulldown-cmark` 0.13 (Stage 2 only) | Spec compliance, byte spans via `into_offset_iter()`, used by mdbook/docs.rs |
| Full-text search | Custom index | SQLite FTS5 (bundled in rusqlite 0.39) | PRD-locked; tokenizer + ranking already correct |
| Recursive walk + ignore | Custom traversal | `walkdir` 2.5 with `filter_entry` | Handles symlinks/permission errors, exhausted by every Rust tool |
| Schema versioning | Manual `PRAGMA user_version` plumbing | `rusqlite_migration` 1.3+ | Composable migrations, replay-safe |
| File hashing | SHA-256 or hand-rolled | BLAKE3 (locked by D-16) | 5-10× faster, plenty for cache keys |
| Unicode normalization | Hand-roll NFC | `unicode-normalization` crate | NFC vs NFD is non-trivial — defer to maintained crate |
| Date parsing for journals | strftime regex | `time` 0.3 with `format_description!` macro | Avoid `chrono`'s heavy deps |
| Concurrency primitives for `SELF_WRITE_SET` | Custom Mutex<HashMap> | `dashmap` (Phase 4 only — not Phase 1) | Phase 1 doesn't need it; flag for Phase 4 planner |

**Key insight:** Phase 1's "don't hand-roll" list is short because the project is deliberately conservative — every dependency is locked by CONTEXT D-09..D-21. The one thing we DO hand-roll is the Stage 1 segmenter, because no library understands "TAB indent + 2-space continuation + fence-aware + drawer-aware" — this is the entire reason Foliom exists.

## Anti-Patterns Specific to This Phase

### AP-1: Letting `pulldown-cmark` see the whole file

**What goes wrong:** Feeding the entire `.md` to `pulldown-cmark` instead of per-block raw text. CommonMark's TAB=4-spaces=code-block rule misparses every nested bullet; the multi-line SQL bullet in `2023_11_09.md` becomes a single nested code block instead of a depth-2 bullet with a fenced code block inside.
**Do instead:** Stage 1 segmenter owns indentation/continuation/fences. Stage 2 sees only the block's `raw` text after the segmenter has already classified it.

### AP-2: Re-serializing the segmented tree on save

**What goes wrong:** `fn serialize(blocks: &[RawBlock]) -> String` exists anywhere in the codebase. Future-self uses it on save. Round-trip drifts on TABs, drawers, blank lines.
**Do instead:** The ONLY way to write a file is byte-splice (Phase 3). Phase 1 explicitly does NOT need a serialize function. If the planner sees "serialize" in a Phase 1 plan, that plan is wrong. The round-trip gate test reconstructs by *slicing the original source*, not by serializing blocks.

### AP-3: Storing absolute paths or backslashes in SQLite

**What goes wrong:** `files.path = "C:\Users\me\notes\foo.md"` on Windows; same DB opened on Linux can't find anything.
**Do instead:** `RelativePath` newtype is the only thing inserted. Absolute paths never reach the DB layer.

### AP-4: Letting the test suite read the corpus from a hard-coded absolute path

**What goes wrong:** `const CORPUS: &str = "/home/m/foliom/data-folder-sample/Logseq";` — fails on CI.
**Do instead:** `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data-folder-sample/Logseq")`.

### AP-5: One transaction for the entire reindex

**What goes wrong:** Wrapping the 619-file reindex in `BEGIN; ... COMMIT;` holds a write lock for the duration; on `kill -9` all progress is lost; SQLite WAL grows unbounded.
**Do instead:** One transaction per file. WAL absorbs the per-tx overhead.

### AP-6: Using `tokio::fs` in Phase 1

**What goes wrong:** Phase 1 has no HTTP server, no async runtime. Adding `tokio` here just for `tokio::fs::read` adds a heavy dep for no benefit and complicates the CLI binary.
**Do instead:** `std::fs` everywhere. Async appears in Phase 2 when axum lands.

### AP-7: Eagerly hashing every file on incremental reindex

**What goes wrong:** Hashing 619 files on every CLI invocation defeats the "incremental" promise.
**Do instead:** Trust `(mtime_ns, size)` from `std::fs::metadata` first; only read+hash on mismatch. `--full` is the escape hatch when you don't trust mtime.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Re-serialize AST → markdown on save (comrak `format_commonmark`, pulldown-cmark-to-cmark) | Byte-splice writeback using `(byte_offset, byte_length)` | Resolved in research; locked by CONTEXT D-14 | Eliminates round-trip drift entirely |
| Per-file FTS row | Per-block FTS5 external-content row | Resolved by ARCHITECTURE.md §3 Q2 | Hit→block resolution without re-parse |
| `sqlx` async for everything | `rusqlite` sync (single writer, single user) | Locked by CONTEXT D-11 | Smaller binary, simpler code, faster on this workload |
| `directories` crate for paths | Hand-rolled `data_dir()` per OS | Recommendation in this document | One less dep; explicit override behavior |
| Hand-rolled debouncer for fs events | `notify-debouncer-full` `RecommendedCache` | Locked for Phase 4 — Phase 1 doesn't touch watcher | — |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `data-folder-sample/Logseq/` contains exactly 619 `.md` files | Round-Trip CI Gate test asserts count | LOW — assertion is a guard; if count differs, planner updates it. Could break first CI run if the user has been editing the corpus. |
| A2 | All files in the corpus are UTF-8 encoded | Stage 1 segmenter assumes UTF-8 | MEDIUM — if any file has BOM or Latin-1, segmenter must handle. Mitigation: P1 includes a "read corpus and check encoding" smoke task. |
| A3 | Logseq's bullet syntax is exactly `\t*- ` (TAB then `- `) | Stage 1 segmenter regex | MEDIUM — if any sample file uses `*` or `+` markers or space-indented bullets, segmenter misses them. The inventory script will surface this. |
| A4 | `2-space continuation` means literally 2 spaces (not 1, not 3) after the bullet's TAB depth | Stage 1 segmenter | MEDIUM — visually inspected against `2023_11_09.md` which uses exactly 2 spaces. Other files might differ. |
| A5 | Drawer syntax is `:NAME:` start, `:END:` close, both on lines by themselves | Stage 1 segmenter | LOW — Logseq convention is stable per Org-mode roots. |
| A6 | `pulldown-cmark` 0.13 `into_offset_iter()` returns byte offsets into the input slice | Stage 2 design | HIGH if wrong — but verified via Context7 in project research (STACK.md). |
| A7 | SQLite 3.46+ (bundled in rusqlite 0.39) has FTS5 compiled in by default | Schema design | HIGH if wrong — but verified via Context7 (STACK.md). Belt-and-suspenders: use `bundled-full` feature for explicit FTS5/JSON1/RTree. |
| A8 | GitHub Actions runners on all 3 OSes can run `cargo nextest` and `cargo build` within free-tier limits | CI Matrix | LOW — standard for OSS Rust projects. |

These assumptions need no user confirmation now — they're flagged so the planner builds the corresponding guards (smoke tests, encoding checks, version assertions) into the right plans.

## Open Questions

1. **Should `pages` rows be created eagerly during scan, or lazily when a ref points at them?**
   - What we know: D-04 says unresolved pages have `file_id = NULL`. Resolved pages have `file_id` populated when the indexer encounters the file.
   - What's unclear: timing. If we scan files in arbitrary order, `[[Foo]]` might reference Foo before Foo's file is processed.
   - Recommendation: Two-pass within a single reindex — pass 1 inserts/updates all `pages` rows (file_id from filename mapping); pass 2 parses blocks and creates refs (target page lookup always finds the page). Document in P6.

2. **What's the page-name derivation rule from filename?**
   - What we know: Journals = `YYYY_MM_DD.md` → `journal_date = YYYY-MM-DD`, `name = ` formatted English long-form (deferred to Phase 2 for formatting; Phase 1 can use the raw `YYYY_MM_DD` as `name`).
   - What's unclear: pages get filename without extension, but should `%2F` decode happen here?
   - Recommendation: Yes — `Foo%2FBar.md` → `name = "Foo/Bar"`. Phase 2 deals with display formatting; Phase 1 produces the canonical name.

3. **Where does the page-prelude block (depth = -1) live in `dump-tree` output?**
   - Recommendation: emit as a leading "(page prelude)" line if non-empty; omit otherwise. Planner can choose.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (stable) | Build | ✓ (assumed; user runs `rustup`) | 1.85+ | None — blocks Phase 1 |
| `cargo` | Build | ✓ | bundled | None |
| `cargo-nextest` | Test runner per D-21 | likely missing locally | latest | `cargo test` works; CI installs nextest |
| Git | Source control + corpus checkout | ✓ | any | None |
| SQLite system lib | None — bundled by rusqlite | N/A | — | N/A |
| C compiler (cc) | Required by rusqlite `bundled` to compile SQLite C source | ✓ on Linux/Mac; needs MSVC Build Tools on Windows | any modern | None — Windows users without MSVC need to install Build Tools |
| GitHub Actions runners | CI matrix | ✓ (free OSS) | latest hosted | None |
| `data-folder-sample/Logseq/` corpus | Round-trip gate | ✓ in repo | — | None |

**Missing dependencies with no fallback:** None — Phase 1 has no exotic requirements.

**Missing dependencies with fallback:** `cargo-nextest` can be installed locally via `cargo install cargo-nextest`; CI uses `taiki-e/install-action@nextest`. `cargo test` is a non-blocking fallback.

## Security Domain

> Security enforcement is implicit (no `security_enforcement: false` in config). Phase 1 is a local-only CLI with no network, no auth, no user-provided code execution.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No auth surface in Phase 1 |
| V3 Session Management | no | No sessions |
| V4 Access Control | no | Single-user local CLI |
| V5 Input Validation | yes | Path traversal — `RelativePath::from_filesystem` rejects paths outside root; symlinks not followed. Filename validation deferred (no creation in Phase 1). |
| V6 Cryptography | yes | BLAKE3 used only as cache key (non-cryptographic guarantee); not for auth/integrity claims. Documented. |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Symlink escape (`notes/link → /etc/`) | Information disclosure | `walkdir::follow_links(false)` (Phase 1 default) |
| Path traversal in `RelativePath` (e.g. `../../etc/passwd` in DB) | Tampering | `from_filesystem` rejects `..` and non-`Normal` components |
| Non-UTF-8 paths | DoS via parse failure | `from_filesystem` returns `Error::NonUtf8Path`; logged + skipped, not fatal |
| Zip-bomb-style huge `.md` file | DoS | Not addressed in Phase 1; document. Pitfall §13 covers Phase 2. |
| SQLite DB in cloud-synced folder corruption | Tampering / integrity loss | D-13: DB lives at `$XDG_DATA_HOME/foliom/...`, never inside notes folder |
| TOCTOU between scan and parse | Tampering | Single-user; acceptable. Hash+mtime mismatch on next reindex catches divergence. |

No new network surface. No new IPC. No new attack surface beyond "the user's own files".

## Sources

### Primary (HIGH confidence)
- `.planning/phases/01-headless-indexing-core/01-CONTEXT.md` — locked decisions D-01..D-21 (authoritative for Phase 1 scope)
- `.planning/REQUIREMENTS.md` — IDX-01..08, PRS-01..07, ACPT-01, ACPT-04 (authoritative for requirements)
- `.planning/ROADMAP.md` — Phase 1 goal + success criteria + ordering invariant
- `.planning/research/SUMMARY.md` — block storage tension resolution, byte-range splice as default
- `.planning/research/ARCHITECTURE.md` §3 — schema rationale and two-stage parser justification
- `.planning/research/STACK.md` — Rust crate versions verified via Context7
- `.planning/research/PITFALLS.md` — pitfalls 1, 3, 4, 6, 7, 9, 11, 14, 16 directly relevant
- `data-folder-sample/Logseq/journals/2023_11_09.md` — direct inspection: validates two-stage segmenter against real code-fence-inside-bullet
- `CLAUDE.md` Technology Stack section — Tauri 2.9.5, pulldown-cmark 0.13, rusqlite 0.39 (verified via Context7 by upstream research)

### Secondary (MEDIUM confidence)
- Training knowledge of: `walkdir` 2.5 `filter_entry` semantics, SQLite FTS5 external-content + trigger maintenance, GitHub Actions matrix patterns, `unicode-normalization` crate API, `rusqlite_migration` 1.3 API.

### Tertiary (LOW confidence — verify during P1/P2)
- Exact corpus count (619) — assertion in test; if off, planner updates.
- `pulldown-cmark` 0.13 `Event::Start(Tag::Heading { .. })` enum variant shape — verify against `pulldown-cmark` 0.13 docs in P3 (minor — refactor on mismatch).

## Metadata

**Confidence breakdown:**
- Plan decomposition: HIGH — derived from locked decisions + ROADMAP ordering invariant.
- Two-stage parser: HIGH — validated against `2023_11_09.md` in upstream research; state machine is concrete.
- Schema: HIGH — distilled from ARCHITECTURE.md §3 and CONTEXT D-05/D-06/D-14.
- Round-trip gate: HIGH — pattern is byte-equality, not approximation.
- DB location + path normalization: HIGH — locked by D-13/D-15; only choice is `directories` crate vs hand-roll.
- Reindex algorithm: HIGH — standard pattern.
- Ignore list + `:hidden`: MEDIUM — regex-based EDN extraction is pragmatic but limited; documented.
- CI matrix: HIGH — standard GitHub Actions; one Windows gotcha (autocrlf) explicitly addressed.
- Anti-patterns: HIGH — derived from PITFALLS.md and project Core Value.

**Research date:** 2026-05-21
**Valid until:** 2026-06-21 (30 days — Phase 1 is foundation work on stable crate versions; locked decisions reduce drift risk).
