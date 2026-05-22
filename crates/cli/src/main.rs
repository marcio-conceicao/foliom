//! Foliom CLI entry point (Plan 01-07).
//!
//! Single binary `foliom` with five subcommands per D-01:
//!
//! * `index <root> [--json]`
//! * `reindex <root> [--full] [--json]`
//! * `search <root> <query> [--limit N] [--json]`
//! * `dump-tree <root> <page> [--json]`
//! * `inventory <root> [--json]`
//!
//! Every subcommand defaults to a tidy human-readable terminal output
//! and switches to a serde-driven JSON contract when `--json` is set
//! (D-02). Logging is structured via `tracing` + `tracing-subscriber`
//! (`RUST_LOG=info` by default — D-18). Error context flows through
//! `anyhow` at the binary boundary, while the library surface uses
//! `thiserror` (D-19).

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod cmd;

#[derive(Parser, Debug)]
#[command(
    name = "foliom",
    version,
    about = "Local-first markdown outliner — headless indexing core (Phase 1)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Index a notes root (incremental — uses cached (mtime, size) fast path).
    Index(cmd::index::IndexArgs),
    /// Re-index a notes root. Pass --full to skip the (mtime, size) fast path.
    Reindex(cmd::reindex::ReindexArgs),
    /// Full-text search across all indexed blocks (FTS5).
    Search(cmd::search::SearchArgs),
    /// Print the block tree of a single page.
    #[command(name = "dump-tree")]
    DumpTree(cmd::dump_tree::DumpTreeArgs),
    /// Aggregate Logseq pattern counts across a notes root.
    Inventory(cmd::inventory::InventoryArgs),
    /// Sobe o servidor HTTP local read-only (Phase 2 — D-22..D-25).
    Serve(cmd::serve::ServeArgs),
}

fn main() -> Result<()> {
    // RUST_LOG-style filtering with a sensible default (D-18).
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Logs go to stderr so stdout stays a clean JSON stream when --json
    // is set (the JSON contract is the load-bearing interop surface).
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Index(args) => cmd::index::run(args),
        Cmd::Reindex(args) => cmd::reindex::run(args),
        Cmd::Search(args) => cmd::search::run(args),
        Cmd::DumpTree(args) => cmd::dump_tree::run(args),
        Cmd::Inventory(args) => cmd::inventory::run(args),
        Cmd::Serve(args) => cmd::serve::run(args),
    }
}
