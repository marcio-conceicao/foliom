# Foliom

Local-first markdown outliner inspired by Logseq/Roam. Foliom treats `.md`
files on disk as the canonical source of truth and builds a knowledge
network from `#tags` and `[[links]]` — **without injecting any
proprietary metadata into the files themselves**. The bet is fast cold
start and small RAM footprint even on large graphs, which is the pain
point that drove the project away from Electron-based outliners.

> See `PRD-outliner-markdown.md` for the full product spec and
> `.planning/` for the active phase decisions, requirements, and roadmap.

## Status

**Phase 1 (Headless Indexing Core) — complete.**

The Rust workspace ships a single binary `foliom` that scans a notes
root, parses every `.md` file with a Logseq-aware two-stage parser,
materialises an SQLite index with FTS5 over blocks, and exposes five
read-only subcommands. The web UI, HTTP server, filesystem watcher,
write-back, and Tauri desktop shell land in later phases — Phase 1 is
the engine those layers consume.

## Quick start

```bash
# Build (debug or release; release is faster for big corpora).
cargo build --release --bin foliom

# Index a notes root (first run builds the cache; subsequent runs use the
# (mtime, size) fast path so unchanged files are skipped).
./target/release/foliom index path/to/notes/

# Inventory: aggregate Logseq pattern counts across the corpus.
./target/release/foliom inventory path/to/notes/

# Full-text search across all indexed blocks (FTS5 over the `raw` column).
./target/release/foliom search path/to/notes/ "bom dia"

# Print the block tree of a single page (case-insensitive lookup).
./target/release/foliom dump-tree path/to/notes/ "2023_11_09"

# Re-index. `--full` skips the (mtime, size) cache and re-reads every file.
./target/release/foliom reindex path/to/notes/ [--full]
```

Every subcommand accepts `--json` to switch from the human-readable
default to a stable, `serde`-driven JSON contract. The structs are
defined in [`crates/core/src/inventory.rs`](crates/core/src/inventory.rs)
and [`crates/core/src/query.rs`](crates/core/src/query.rs); new fields
will only ever be added, never removed or renamed.

## Where the DB lives

The SQLite cache is **never** placed inside the notes folder (this
keeps sync tools like Syncthing / iCloud Drive from fighting the WAL
files). Location by platform:

| Platform | Path |
|----------|------|
| Linux    | `$XDG_DATA_HOME/foliom/<root-hash>.db` (defaults to `~/.local/share/foliom/`) |
| macOS    | `~/Library/Application Support/foliom/<root-hash>.db` |
| Windows  | `%LOCALAPPDATA%\foliom\<root-hash>.db` |

`<root-hash>` is the first 16 chars of the BLAKE3 of the absolute path
to the notes folder, so two roots (or the same root accessed from WSL
vs Windows native) get independent caches by design.

## Round-trip guarantee

Foliom never re-serialises your markdown. The parser exposes
`byte_offset` + `byte_length` for every block; Phase 3's write-back
will splice changes back into the file without touching unrelated
bytes. A CI gate
([`crates/core/tests/roundtrip.rs`](crates/core/tests/roundtrip.rs))
enforces byte-identical reads of the curated synthetic corpus on every
platform, so any future change that would corrupt round-trip fails the
build before merge.

## Architecture

Cargo workspace with two crates today; HTTP server (Phase 2) and Tauri
shell (Phase 5) will land alongside without refactoring the core:

```
crates/
  core/   # parser, scanner, indexer, storage — pure logic, no IO bindings
  cli/    # the `foliom` binary; clap subcommand tree + JSON output
```

Key tech: Rust 1.85+ (edition 2024), `pulldown-cmark` 0.13 for
CommonMark + GFM with byte spans, `rusqlite` 0.39 with `bundled-full`
(SQLite + FTS5 compiled in), `blake3` for file/block hashing,
`walkdir` for the scan walk, `notify` (Phase 2) for the watcher,
Svelte 5 + CodeMirror 6 for the future UI. See
[`.planning/research/STACK.md`](.planning/research/STACK.md) for the
full inventory and the alternatives considered.

## Testing

Phase 1's test suite runs in <5 s on a modern laptop and covers
parser, scanner, storage, indexer, and CLI end-to-end:

```bash
# Full workspace (121 tests as of Phase 1 close-out).
cargo test --workspace --locked

# Or the faster runner used in CI.
cargo nextest run --workspace
```

### Cross-OS smoke test (manual procedure)

The primary developer works in WSL2 (Ubuntu) on Windows 11 and
verifies on Windows 11 native before each phase close-out. The CI
matrix runs `ubuntu-latest`, `macos-latest`, and `windows-latest`
against the committed synthetic fixture; the real PII corpus in
`data-folder-sample/Logseq/` is gitignored and only exercised
locally.

To smoke-test on Windows 11 native (PowerShell):

```powershell
git clone https://github.com/<you>/foliom.git
cd foliom
cargo test --workspace --locked
cargo build --release --bin foliom
.\target\release\foliom.exe inventory .\crates\core\tests\fixtures\logseq-synthetic --json
```

When the Phase 2 filesystem watcher lands, note that running Foliom
from inside WSL against a `/mnt/c/...` path is **not** supported for
the watcher (the inotify→Windows-FS bridge does not propagate events
reliably). On the Windows side of WSL2, run the Windows-native build
against `C:\...` paths.

## License

Apache-2.0.
