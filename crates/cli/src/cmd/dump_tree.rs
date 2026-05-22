//! `foliom dump-tree <root> <page>` — print the block tree of a single
//! page with depth-indented bullets.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::query::{BlockNode, dump_page_tree};
use foliom_core::storage::Db;

#[derive(Args, Debug)]
pub struct DumpTreeArgs {
    /// Notes root (must have been indexed at least once).
    pub root: PathBuf,
    /// Page name (case-insensitive, NOCASE lookup per D-03).
    pub page: String,
    /// Emit the tree as JSON (`Vec<BlockNode>`).
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: DumpTreeArgs) -> Result<()> {
    let db = Db::open(&args.root)
        .with_context(|| format!("opening DB for root {:?}", args.root))?;
    let tree = dump_page_tree(&db, &args.page)
        .with_context(|| format!("dumping page {:?}", args.page))?;

    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), &tree)?;
        println!();
    } else if tree.is_empty() {
        println!("Page {:?} not found or empty.", args.page);
    } else {
        for node in &tree {
            print_node(node, 0);
        }
    }
    Ok(())
}

fn print_node(node: &BlockNode, indent: usize) {
    // First non-empty line of the raw block, truncated to 80 chars.
    let first_line = node.raw.lines().next().unwrap_or("");
    let truncated = if first_line.chars().count() > 80 {
        let cut: String = first_line.chars().take(79).collect();
        format!("{cut}…")
    } else {
        first_line.to_string()
    };
    let prefix = "  ".repeat(indent);
    println!("{prefix}- {truncated}");
    for child in &node.children {
        print_node(child, indent + 1);
    }
}
