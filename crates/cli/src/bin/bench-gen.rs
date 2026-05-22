//! Synthetic Logseq-style corpus generator (plan 02-08, ACPT-02/03).
//!
//! `cargo run --release --bin foliom-bench-gen -- --out /tmp/synth-5k \
//!     --count 5000 --seed 42`
//!
//! Produces a deterministic corpus matching a realistic Logseq base:
//!   - 70 % journals (`journals/YYYY_MM_DD.md`), 30 % topical pages
//!     (`pages/Topic N.md`).
//!   - Bullets indented with `\t` (depths 0–5, skewed to 0–2 per
//!     PRD §5 outliner contract).
//!   - ~5 % bullets carry a fenced code block (` ```rust ` / ` ```python `
//!     / ` ```sql `).
//!   - ~5 % blocks carry `id:: <uuid>` or `collapsed:: true` properties.
//!   - ~10 % journal pages carry a `:LOGBOOK: … :END:` drawer.
//!   - ~5 link/tag refs per block on average; `[[Topic N]]`, `[[YYYY_MM_DD]]`,
//!     `#tag` and `#[[multi word tag]]` chips.
//!   - File sizes are log-normal — median ≈ 2 KB, occasional 30–50 KB.
//!
//! Determinism: every random choice is drawn from a single `ChaCha8Rng`
//! seeded by `--seed`. The same seed + count produces byte-identical
//! output (modulo filesystem mtime).
//!
//! Output is overwrite-safe: existing files under `--out` are replaced
//! but other files are left alone (no recursive delete).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// 70 / 30 journal-vs-page split per plan 02-08 §must_haves.
const JOURNAL_FRACTION: f64 = 0.70;

/// Day-zero for journal date stamping. 2010-01-01 keeps dates well within
/// the unambiguous Gregorian range and gives ~15 years of journals before
/// dates wrap into the future — plenty for a 3500-day corpus.
const EPOCH_YEAR: i32 = 2010;

/// Probability that a bullet emits a fenced code block on the next line.
const CODE_FENCE_PROB: f64 = 0.05;
/// Probability that a block carries an `id::` or `collapsed::` property.
const PROPERTY_PROB: f64 = 0.05;
/// Probability that a journal page carries a `:LOGBOOK:` drawer.
const LOGBOOK_PROB: f64 = 0.10;

/// Bullet-depth distribution weights for indices 0..=5 (skewed to 0–2).
const DEPTH_WEIGHTS: [u32; 6] = [40, 30, 15, 8, 4, 3];

#[derive(Parser, Debug)]
#[command(name = "foliom-bench-gen",
          about = "Synthetic Logseq-style 5k corpus for ACPT-02/03 benches")]
struct Args {
    /// Output directory (created if missing; `pages/` + `journals/`
    /// subdirs are created unconditionally).
    #[arg(long)]
    out: PathBuf,
    /// Total file count. Distributed 70 % journals / 30 % pages.
    #[arg(long, default_value_t = 5000)]
    count: usize,
    /// PRNG seed. Same seed + count + version produces byte-identical
    /// output (sans mtime).
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut rng = ChaCha8Rng::seed_from_u64(args.seed);

    fs::create_dir_all(args.out.join("pages"))?;
    fs::create_dir_all(args.out.join("journals"))?;

    let n_journals = (args.count as f64 * JOURNAL_FRACTION) as usize;
    let n_pages = args.count - n_journals;

    // Build the topical page-name pool first so journals can link to them.
    let topical_names: Vec<String> = (0..n_pages).map(topic_name).collect();
    // Journal dates — deterministic 1-day stride from EPOCH_YEAR-01-01.
    let journal_dates: Vec<String> = (0..n_journals).map(day_offset).collect();

    for date in &journal_dates {
        let path = args.out.join("journals").join(format!("{date}.md"));
        let content = render_journal(&mut rng, date, &topical_names,
                                     &journal_dates);
        write_file(&path, &content)?;
    }
    for name in &topical_names {
        let path = args.out.join("pages").join(format!("{name}.md"));
        let content = render_page(&mut rng, name, &topical_names,
                                  &journal_dates);
        write_file(&path, &content)?;
    }

    println!("Generated {} files ({} journals + {} pages) in {}",
             args.count, n_journals, n_pages, args.out.display());
    Ok(())
}

fn write_file(path: &std::path::Path, content: &str) -> anyhow::Result<()> {
    let mut f = fs::File::create(path)?;
    f.write_all(content.as_bytes())?;
    Ok(())
}

// ── Naming helpers ─────────────────────────────────────────────────────

fn topic_name(i: usize) -> String {
    format!("Topic {i}")
}

/// Render `YYYY_MM_DD` for the i-th day after `EPOCH_YEAR-01-01`.
///
/// Avoids the `time` crate's local-offset surface — pure arithmetic on
/// the proleptic Gregorian calendar, sufficient for synthetic corpus
/// stamping. Days are accumulated month-by-month so leap years land
/// correctly across decades.
fn day_offset(i: usize) -> String {
    let mut remaining = i as i64;
    let mut year = EPOCH_YEAR;
    loop {
        let yd = if is_leap(year) { 366 } else { 365 };
        if remaining < yd { break; }
        remaining -= yd;
        year += 1;
    }
    let mut month = 1u32;
    loop {
        let md = days_in_month(year, month) as i64;
        if remaining < md { break; }
        remaining -= md;
        month += 1;
    }
    let day = (remaining as u32) + 1;
    format!("{year:04}_{month:02}_{day:02}")
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(y) { 29 } else { 28 },
        _ => unreachable!("month out of range: {m}"),
    }
}

// ── Block-level renderers ──────────────────────────────────────────────

fn render_journal(rng: &mut ChaCha8Rng, date: &str,
                  topics: &[String], dates: &[String]) -> String {
    let mut out = String::new();
    // Journals have no first-line "title::" (matches Logseq convention).

    if rng.gen_bool(LOGBOOK_PROB) {
        emit_logbook(&mut out, date);
    }

    let n_blocks = log_normal_block_count(rng);
    for _ in 0..n_blocks {
        emit_block(rng, &mut out, topics, dates);
    }
    out
}

fn render_page(rng: &mut ChaCha8Rng, name: &str,
               topics: &[String], dates: &[String]) -> String {
    let mut out = String::new();
    // A handful of pages get a frontmatter-style title:: property block.
    if rng.gen_bool(0.30) {
        out.push_str(&format!("title:: {name}\n\n"));
    }
    let n_blocks = log_normal_block_count(rng);
    for _ in 0..n_blocks {
        emit_block(rng, &mut out, topics, dates);
    }
    out
}

/// Block-count distribution — discretized log-normal so most files are
/// small (3–15 bullets) but a tail of 100+ ones produces 30–50 KB files.
fn log_normal_block_count(rng: &mut ChaCha8Rng) -> usize {
    // Approximate exp(N(mu=2.3, sigma=0.9)) using two uniforms.
    let u1: f64 = rng.gen_range(1e-9..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    let v = (2.3_f64 + 0.9_f64 * z).exp();
    v.round().clamp(2.0, 200.0) as usize
}

fn emit_block(rng: &mut ChaCha8Rng, out: &mut String,
              topics: &[String], dates: &[String]) {
    let depth = weighted_depth(rng);
    let indent = "\t".repeat(depth);
    let body = bullet_text(rng, topics, dates);
    out.push_str(&format!("{indent}- {body}\n"));

    // Continuation lines for the block (2-space indented under `- `).
    let cont = "  ".repeat(depth + 1);
    if rng.gen_bool(PROPERTY_PROB) {
        let prop = if rng.gen_bool(0.5) {
            format!("id:: {}", pseudo_uuid(rng))
        } else {
            String::from("collapsed:: true")
        };
        out.push_str(&format!("{cont}{prop}\n"));
    }
    if rng.gen_bool(CODE_FENCE_PROB) {
        let lang = ["rust", "python", "sql"]
            .choose(rng).copied().unwrap_or("rust");
        out.push_str(&format!("{cont}```{lang}\n"));
        out.push_str(&format!("{cont}// {} sample\n", lang));
        out.push_str(&format!("{cont}fn x() {{ /* {} */ }}\n", lang));
        out.push_str(&format!("{cont}```\n"));
    }
}

fn weighted_depth(rng: &mut ChaCha8Rng) -> usize {
    let total: u32 = DEPTH_WEIGHTS.iter().sum();
    let mut pick = rng.gen_range(0..total);
    for (depth, w) in DEPTH_WEIGHTS.iter().enumerate() {
        if pick < *w { return depth; }
        pick -= *w;
    }
    0
}

fn bullet_text(rng: &mut ChaCha8Rng, topics: &[String],
               dates: &[String]) -> String {
    let n_words = rng.gen_range(4..18);
    let n_refs = rng.gen_range(0..=8); // mean ≈ 4 (close to 5 target).
    let mut parts: Vec<String> = (0..n_words)
        .map(|_| word(rng).to_string())
        .collect();
    for _ in 0..n_refs {
        // Choose ref kind: topic link, date link, simple tag, composite tag.
        let kind = rng.gen_range(0..4);
        let s = match kind {
            0 => format!("[[{}]]", topics.choose(rng).cloned()
                                          .unwrap_or_else(|| topic_name(0))),
            1 => format!("[[{}]]", dates.choose(rng).cloned()
                                          .unwrap_or_else(|| day_offset(0))),
            2 => format!("#{}", tag_word(rng)),
            _ => format!("#[[{} {}]]", tag_word(rng), tag_word(rng)),
        };
        let pos = if parts.is_empty() { 0 }
                  else { rng.gen_range(0..parts.len()) };
        parts.insert(pos, s);
    }
    parts.join(" ")
}

fn emit_logbook(out: &mut String, date: &str) {
    out.push_str(":LOGBOOK:\n");
    out.push_str(&format!("CLOCK: [{date} Mon 09:00:00]--[{date} Mon 09:30:00] => 00:30:00\n"));
    out.push_str(":END:\n\n");
}

// ── Tiny lexica (deterministic via &mut rng) ──────────────────────────

const WORDS: &[&str] = &[
    "outline", "block", "graph", "page", "journal", "link", "tag",
    "query", "scan", "index", "parser", "render", "search", "schema",
    "deploy", "build", "stack", "watch", "reload", "cache", "node",
    "edge", "tree", "leaf", "depth", "fence", "code", "rust", "axum",
    "svelte", "sqlite", "fts", "embed", "ship", "spec", "design",
    "review", "ticket", "draft", "sync", "diff", "commit", "merge",
];

const TAGS: &[&str] = &[
    "todo", "next", "later", "now", "idea", "bug", "fix", "ship",
    "perf", "ui", "api", "design", "research", "draft", "review",
    "qa", "infra", "ops", "doc", "spike", "demo",
];

fn word(rng: &mut ChaCha8Rng) -> &'static str {
    WORDS.choose(rng).copied().unwrap_or("outline")
}

fn tag_word(rng: &mut ChaCha8Rng) -> &'static str {
    TAGS.choose(rng).copied().unwrap_or("todo")
}

fn pseudo_uuid(rng: &mut ChaCha8Rng) -> String {
    // 32 hex chars in 8-4-4-4-12 layout; not a real RFC 4122 UUID but
    // close enough for synthetic property values (the indexer just stores
    // the string).
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11],
        bytes[12], bytes[13], bytes[14], bytes[15],
    )
}
