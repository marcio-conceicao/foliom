//! `foliom index <root>` — incremental reindex with cached (mtime, size)
//! fast-path. Equivalent to the first run that materialises the DB.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::indexer::{ReindexMode, ReindexStats, reindex};
use foliom_core::storage::Db;

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// Notes root to index (must exist and be a directory).
    pub root: PathBuf,
    /// Emit the ReindexStats struct as JSON on stdout (D-02).
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: IndexArgs) -> Result<()> {
    let mut db = Db::open(&args.root)
        .with_context(|| format!("opening DB for root {:?}", args.root))?;
    let stats = reindex(&mut db, &args.root, ReindexMode::Incremental)
        .with_context(|| format!("indexing {:?}", args.root))?;

    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), &stats)?;
        println!();
    } else {
        print_human(&stats);
    }
    Ok(())
}

fn print_human(stats: &ReindexStats) {
    println!("Index complete:");
    println!("  scanned        {}", stats.scanned);
    println!("  added          {}", stats.added);
    println!("  modified       {}", stats.modified);
    println!("  unchanged      {}", stats.unchanged);
    println!("  mtime-touched  {}", stats.mtime_touched);
    println!("  deleted        {}", stats.deleted);
}
