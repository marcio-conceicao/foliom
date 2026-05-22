//! `foliom inventory <root>` — Logseq pattern aggregator. Gatekeeps
//! parser sign-off (IDX-08); pinned in CI by the integration test.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::inventory::{InventoryReport, inventory_report};

#[derive(Args, Debug)]
pub struct InventoryArgs {
    /// Notes root to walk (must exist and be a directory).
    pub root: PathBuf,
    /// Emit the full `InventoryReport` as JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: InventoryArgs) -> Result<()> {
    let report = inventory_report(&args.root)
        .with_context(|| format!("scanning inventory for {:?}", args.root))?;

    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), &report)?;
        println!();
    } else {
        print_human(&report);
    }
    Ok(())
}

fn print_human(r: &InventoryReport) {
    println!("Foliom inventory — {}", r.root);
    println!();
    println!(
        "  scanned        {}  (journals: {}, pages: {})",
        r.scanned_files, r.journal_files, r.page_files
    );
    println!("  total bytes    {}", r.total_size_bytes);
    println!("  block-property files: {}", r.block_property_files);
    println!("  drawer files:         {}", r.drawer_files);
    println!();
    println!("  {:<24} {:>10} {:>14}", "pattern", "filesWith", "occurrences");
    println!("  {:-<24} {:->10} {:->14}", "", "", "");
    for p in &r.patterns {
        println!(
            "  {:<24} {:>10} {:>14}",
            p.name, p.files_with, p.occurrences
        );
    }
}
