//! Phase 1 / Plan 01-07 — Logseq pattern inventory aggregator.
//!
//! `inventory_report(root)` walks the corpus via [`scanner::walk`] and
//! counts occurrences (and number of distinct files containing them) of
//! every Logseq-specific pattern the indexer cares about. The result is
//! serialized to JSON via [`InventoryReport`] / [`PatternCount`] and is
//! the long-lived contract Phase 2 consumes (D-02).
//!
//! Implementation reuses [`parser::segment::segment`] so the counts cannot
//! drift from what the indexer would actually store. The aggregator never
//! touches the SQLite cache — it's pure, side-effect-free, and safe to run
//! anywhere.
//!
//! ## Pattern definitions (load-bearing — pinned by the CLI integration
//! test in `crates/cli/tests/cli_integration.rs`):
//!
//! * `alias::` / `id::` / `template::` — block-property substring matches
//!   inside any block's `raw`. Counted per occurrence (substring count)
//!   and per file (one file may have many).
//! * `LOGBOOK` — counted from `block.drawers` whose `name == "LOGBOOK"`.
//! * `#[[...]]` — composite tag, counted via `raw.match_indices("#[[")`.
//! * `SCHEDULED:` / `DEADLINE:` — substring matches inside `raw`.
//! * `code-fence-in-bullet` — non-prelude blocks whose `raw` contains
//!   `` ``` ``. Counted once per such block (each opening fence in a
//!   block counts once).
//! * `%2F-in-filename` — files whose relative path contains `%2F`
//!   (case-insensitive).
//! * `block_property_files` / `drawer_files` — distinct files with at
//!   least one parsed property / drawer.

use std::collections::HashSet;
use std::path::Path;

use crate::indexer::IndexerError;
use crate::parser::segment::segment;
use crate::path::RelativePath;
use crate::scanner::config_edn::read_hidden;
use crate::scanner::ignore::IgnoreSet;
use crate::scanner::walk::walk;

/// Aggregated report serialized as the long-lived JSON contract (D-02).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReport {
    /// Notes root the report was generated from (forward-slash form for
    /// JSON stability across platforms).
    pub root: String,
    /// Total `.md` files visited by the scanner.
    pub scanned_files: u32,
    /// Files classified as journals (path prefix `journals/`).
    pub journal_files: u32,
    /// All other `.md` files (`scanned_files - journal_files`).
    pub page_files: u32,
    /// Sum of file sizes in bytes.
    pub total_size_bytes: u64,
    /// Per-pattern counts. Order is deterministic — keyed by `name`.
    pub patterns: Vec<PatternCount>,
    /// Distinct files containing at least one block property.
    pub block_property_files: u32,
    /// Distinct files containing at least one drawer (any name).
    pub drawer_files: u32,
}

/// One row of [`InventoryReport::patterns`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternCount {
    /// Pattern identifier (stable across releases — JSON contract).
    pub name: String,
    /// Number of distinct files containing at least one match.
    pub files_with: u32,
    /// Total occurrences across the entire corpus.
    pub occurrences: u32,
}

/// Pattern keys in the order they appear in the output. Centralised so
/// callers and tests can iterate without magic strings.
pub const PATTERN_KEYS: &[&str] = &[
    "alias::",
    "id::",
    "template::",
    "LOGBOOK",
    "#[[...]]",
    "SCHEDULED:",
    "DEADLINE:",
    "code-fence-in-bullet",
    "%2F-in-filename",
];

/// Walk `root` and aggregate Logseq pattern statistics over every `.md`
/// file the scanner emits.
pub fn inventory_report(root: &Path) -> Result<InventoryReport, IndexerError> {
    // Build the ignore set the same way the indexer does so the report
    // never diverges from what would actually be stored.
    let mut ignore = IgnoreSet::default_logseq();
    let config_edn = root.join("logseq").join("config.edn");
    if config_edn.is_file() {
        let extra = read_hidden(&config_edn);
        if !extra.is_empty() {
            ignore.extend_from_config_edn(extra);
        }
    }

    // One occurrence-counter + one files-with set per pattern key.
    // `files_with` is a HashSet<RelativePath as String> so case-sensitive
    // FS quirks don't double-count the same file.
    let mut occurrences: Vec<u32> = vec![0; PATTERN_KEYS.len()];
    let mut files_with: Vec<HashSet<String>> = vec![HashSet::new(); PATTERN_KEYS.len()];

    let mut scanned_files: u32 = 0;
    let mut journal_files: u32 = 0;
    let mut total_size_bytes: u64 = 0;

    let mut block_property_files: HashSet<String> = HashSet::new();
    let mut drawer_files: HashSet<String> = HashSet::new();

    for entry in walk(root, &ignore) {
        scanned_files += 1;
        total_size_bytes = total_size_bytes.saturating_add(entry.size);

        // Relative path in storage form (NFC + forward-slash). We need it
        // both to classify journal vs page and to detect %2F filenames.
        let rel = match RelativePath::from_filesystem(&entry.path, root) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(path = %entry.path.display(), error = %err, "inventory: path normalize failed — skipping");
                continue;
            }
        };
        let rel_str = rel.as_str().to_string();

        let is_journal = rel_str.starts_with("journals/");
        if is_journal {
            journal_files += 1;
        }

        // %2F-in-filename — check the relative path, case-insensitive.
        if rel_str.to_ascii_lowercase().contains("%2f") {
            let idx = pattern_index("%2F-in-filename");
            occurrences[idx] += 1;
            files_with[idx].insert(rel_str.clone());
        }

        // Read & segment the file. IO errors are logged and the file is
        // skipped — inventory is best-effort, not a write path.
        let bytes = match std::fs::read(&entry.path) {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(path = %entry.path.display(), error = %err, "inventory: read failed — skipping");
                continue;
            }
        };
        let blocks = segment(&bytes);

        // Per-file flags for files_with bookkeeping.
        let mut had_alias = false;
        let mut had_id = false;
        let mut had_template = false;
        let mut had_logbook = false;
        let mut had_composite_tag = false;
        let mut had_scheduled = false;
        let mut had_deadline = false;
        let mut had_fence_in_bullet = false;
        let mut had_block_property = false;
        let mut had_drawer = false;

        for block in &blocks {
            // Drawer / property aggregation (authoritative — same source
            // the indexer uses).
            if !block.properties.is_empty() {
                had_block_property = true;
            }
            if !block.drawers.is_empty() {
                had_drawer = true;
            }
            for d in &block.drawers {
                if d.name == "LOGBOOK" {
                    occurrences[pattern_index("LOGBOOK")] += 1;
                    had_logbook = true;
                }
            }

            // Pure substring scans against `raw`. Cheap and faithful to
            // what users see in the file.
            let raw = &block.raw;

            let alias_n = count_occurrences(raw, "alias::");
            if alias_n > 0 {
                occurrences[pattern_index("alias::")] += alias_n;
                had_alias = true;
            }
            let id_n = count_occurrences(raw, "id::");
            if id_n > 0 {
                occurrences[pattern_index("id::")] += id_n;
                had_id = true;
            }
            let template_n = count_occurrences(raw, "template::");
            if template_n > 0 {
                occurrences[pattern_index("template::")] += template_n;
                had_template = true;
            }

            let composite_n = count_occurrences(raw, "#[[");
            if composite_n > 0 {
                occurrences[pattern_index("#[[...]]")] += composite_n;
                had_composite_tag = true;
            }

            let sched_n = count_occurrences(raw, "SCHEDULED:");
            if sched_n > 0 {
                occurrences[pattern_index("SCHEDULED:")] += sched_n;
                had_scheduled = true;
            }
            let dead_n = count_occurrences(raw, "DEADLINE:");
            if dead_n > 0 {
                occurrences[pattern_index("DEADLINE:")] += dead_n;
                had_deadline = true;
            }

            // code-fence-in-bullet: any non-prelude block whose raw
            // contains a ``` fence opener. The segmenter has already
            // ensured fences here are fully balanced (otherwise the
            // block would extend to EOF), so `contains("```")` is
            // sufficient signal that this block opens a fenced block.
            if block.depth != u8::MAX && raw.contains("```") {
                occurrences[pattern_index("code-fence-in-bullet")] += 1;
                had_fence_in_bullet = true;
            }
        }

        // Promote per-file flags into the global files_with sets.
        if had_alias {
            files_with[pattern_index("alias::")].insert(rel_str.clone());
        }
        if had_id {
            files_with[pattern_index("id::")].insert(rel_str.clone());
        }
        if had_template {
            files_with[pattern_index("template::")].insert(rel_str.clone());
        }
        if had_logbook {
            files_with[pattern_index("LOGBOOK")].insert(rel_str.clone());
        }
        if had_composite_tag {
            files_with[pattern_index("#[[...]]")].insert(rel_str.clone());
        }
        if had_scheduled {
            files_with[pattern_index("SCHEDULED:")].insert(rel_str.clone());
        }
        if had_deadline {
            files_with[pattern_index("DEADLINE:")].insert(rel_str.clone());
        }
        if had_fence_in_bullet {
            files_with[pattern_index("code-fence-in-bullet")].insert(rel_str.clone());
        }
        if had_block_property {
            block_property_files.insert(rel_str.clone());
        }
        if had_drawer {
            drawer_files.insert(rel_str.clone());
        }
    }

    let patterns: Vec<PatternCount> = PATTERN_KEYS
        .iter()
        .enumerate()
        .map(|(i, key)| PatternCount {
            name: (*key).to_string(),
            files_with: files_with[i].len() as u32,
            occurrences: occurrences[i],
        })
        .collect();

    Ok(InventoryReport {
        root: root.to_string_lossy().replace('\\', "/"),
        scanned_files,
        journal_files,
        page_files: scanned_files - journal_files,
        total_size_bytes,
        patterns,
        block_property_files: block_property_files.len() as u32,
        drawer_files: drawer_files.len() as u32,
    })
}

fn pattern_index(key: &str) -> usize {
    PATTERN_KEYS
        .iter()
        .position(|k| *k == key)
        .expect("pattern key must be in PATTERN_KEYS")
}

fn count_occurrences(haystack: &str, needle: &str) -> u32 {
    if needle.is_empty() {
        return 0;
    }
    let mut n: u32 = 0;
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        n += 1;
        start += pos + needle.len();
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn synthetic_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("logseq-synthetic")
    }

    #[test]
    fn synthetic_fixture_inventory_smoke() {
        let root = synthetic_fixture();
        let report = inventory_report(&root).expect("inventory must succeed");
        assert!(report.scanned_files > 0, "scanned_files must be > 0");
        assert!(
            report.patterns.len() == PATTERN_KEYS.len(),
            "patterns vec must have one row per PATTERN_KEYS entry"
        );
        let json = serde_json::to_string_pretty(&report).expect("must serialize to JSON");
        assert!(json.contains("\"scannedFiles\""), "camelCase keys expected");
        assert!(json.contains("\"filesWith\""), "camelCase keys expected");
    }

    #[test]
    fn synthetic_fixture_detects_known_patterns() {
        let report = inventory_report(&synthetic_fixture()).expect("inventory must succeed");
        // Sanity: the synthetic corpus is curated to hit every pattern.
        let by_name = |name: &str| {
            report
                .patterns
                .iter()
                .find(|p| p.name == name)
                .unwrap_or_else(|| panic!("pattern {name} missing"))
        };
        assert!(
            by_name("LOGBOOK").occurrences >= 1,
            "expected ≥1 LOGBOOK drawer in synthetic corpus"
        );
        assert!(
            by_name("#[[...]]").occurrences >= 1,
            "expected ≥1 composite tag"
        );
        assert!(
            by_name("%2F-in-filename").files_with >= 1,
            "expected Parent%2FChild.md to be picked up"
        );
        assert!(
            report.block_property_files >= 1,
            "expected ≥1 file with block properties"
        );
    }

    #[test]
    fn count_occurrences_handles_overlap_correctly() {
        assert_eq!(count_occurrences("aaaa", "aa"), 2);
        assert_eq!(count_occurrences("", "x"), 0);
        assert_eq!(count_occurrences("abc", ""), 0);
        assert_eq!(count_occurrences("alias::alias::", "alias::"), 2);
    }
}
