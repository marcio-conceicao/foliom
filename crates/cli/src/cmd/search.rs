//! `foliom search <root> <query> [--limit N]` — FTS5 search across the
//! `blocks_fts` virtual table.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::query::search_blocks;
use foliom_core::storage::Db;

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Notes root (must have been indexed at least once).
    pub root: PathBuf,
    /// FTS5 query string (passes through to the MATCH clause as-is).
    pub query: String,
    /// Max hits to return (default 50).
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    /// Emit hits as JSON (`Vec<SearchHit>`).
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: SearchArgs) -> Result<()> {
    let db = Db::open(&args.root)
        .with_context(|| format!("opening DB for root {:?}", args.root))?;
    let hits = search_blocks(&db, &args.query, args.limit)
        .with_context(|| format!("searching for {:?}", args.query))?;

    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), &hits)?;
        println!();
    } else if hits.is_empty() {
        println!("No hits.");
    } else {
        for hit in &hits {
            println!("{}  [block {}]  {}", hit.page_path, hit.block_id, hit.snippet);
        }
        println!("\n{} hit(s).", hits.len());
    }
    Ok(())
}
