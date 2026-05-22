//! `foliom reindex <root> [--full]` — re-index a notes root. `--full`
//! skips the (mtime, size) fast path and re-hashes every file.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::storage::Db;

#[derive(Args, Debug)]
pub struct ReindexArgs {
    /// Notes root to reindex.
    pub root: PathBuf,
    /// Skip the (mtime, size) fast path; re-read and re-hash every file.
    #[arg(long)]
    pub full: bool,
    /// Emit the ReindexStats struct as JSON on stdout (D-02).
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: ReindexArgs) -> Result<()> {
    let mode = if args.full {
        ReindexMode::Full
    } else {
        ReindexMode::Incremental
    };
    let mut db = Db::open(&args.root)
        .with_context(|| format!("opening DB for root {:?}", args.root))?;
    let stats = reindex(&mut db, &args.root, mode)
        .with_context(|| format!("reindexing {:?}", args.root))?;

    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), &stats)?;
        println!();
    } else {
        println!("Reindex ({:?}) complete:", mode);
        println!("  scanned        {}", stats.scanned);
        println!("  added          {}", stats.added);
        println!("  modified       {}", stats.modified);
        println!("  unchanged      {}", stats.unchanged);
        println!("  mtime-touched  {}", stats.mtime_touched);
        println!("  deleted        {}", stats.deleted);
    }
    Ok(())
}
