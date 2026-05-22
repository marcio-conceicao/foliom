//! End-to-end CLI integration tests (Plan 01-07, Task 4).
//!
//! Drives `target/{profile}/foliom` via `assert_cmd` against the
//! synthetic fixture (`crates/core/tests/fixtures/logseq-synthetic/`).
//! Per the 2026-05-21 plan revision, all pinned counts are tied to the
//! synthetic corpus (the real PII corpus is gitignored and not present in
//! CI). The real-corpus leg is intentionally not exercised here.
//!
//! ## Pinned inventory counts
//!
//! Captured 2026-05-21 from a clean run of
//! `foliom inventory crates/core/tests/fixtures/logseq-synthetic --json`.
//! If you intentionally change parser semantics or add fixtures, set
//! `FOLIOM_REGEN_INVENTORY=1`, run the test, copy the printed Rust array
//! into [`EXPECTED_PATTERNS`], and commit. Otherwise this test is a
//! regression gate — any silent drift in pattern counts trips CI.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use tempfile::TempDir;

/// Synthetic fixture path (CI target — committed, no PII).
fn synthetic_corpus() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/cli/. Step up to repo root then into the
    // core crate's tests fixtures.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

/// Build a `Command` for the `foliom` binary with an isolated
/// `XDG_DATA_HOME` so concurrent tests do not collide on the DB cache.
fn foliom_cmd(data_home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("foliom").expect("foliom binary built");
    // Isolate the DB cache location (Db::open puts the SQLite file under
    // $XDG_DATA_HOME/foliom/<root-hash>.db on Linux).
    cmd.env("XDG_DATA_HOME", data_home.path());
    // macOS / Windows fall back to platform-native dirs. Override the
    // generic HOME and LOCALAPPDATA too so tests stay isolated everywhere.
    cmd.env("HOME", data_home.path());
    cmd.env("LOCALAPPDATA", data_home.path());
    cmd
}

/// PINNED inventory counts (capture protocol described in the module doc).
/// `(pattern_name, files_with, occurrences)`. Order matches PATTERN_KEYS.
const EXPECTED_PATTERNS: &[(&str, u32, u32)] = &[
    ("alias::", 2, 2),
    ("id::", 2, 2),
    ("template::", 2, 2),
    ("LOGBOOK", 3, 4),
    ("#[[...]]", 4, 6),
    ("SCHEDULED:", 3, 3),
    ("DEADLINE:", 0, 0),
    ("code-fence-in-bullet", 3, 3),
    ("%2F-in-filename", 1, 1),
];
const EXPECTED_SCANNED: u64 = 11;
const EXPECTED_JOURNALS: u64 = 1;
const EXPECTED_PAGES: u64 = 10;
const EXPECTED_BLOCK_PROPERTY_FILES: u64 = 2;
const EXPECTED_DRAWER_FILES: u64 = 3;

#[test]
fn test_index_then_inventory_reports_pinned_counts() {
    let data_home = TempDir::new().unwrap();
    let corpus = synthetic_corpus();
    assert!(corpus.is_dir(), "synthetic fixture must exist at {corpus:?}");

    // First: index the corpus so the DB exists (so any future cross-cmd
    // dependency works). Inventory itself does not need the DB.
    let out = foliom_cmd(&data_home)
        .arg("index")
        .arg(&corpus)
        .output()
        .expect("foliom index ran");
    assert!(
        out.status.success(),
        "index failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Run inventory --json and parse.
    let out = foliom_cmd(&data_home)
        .arg("inventory")
        .arg(&corpus)
        .arg("--json")
        .output()
        .expect("foliom inventory ran");
    assert!(
        out.status.success(),
        "inventory failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).expect("inventory JSON is UTF-8");
    let report: Value =
        serde_json::from_str(&stdout).expect("inventory must emit valid JSON");

    // Regen mode: dump the current values in copy-paste-ready Rust syntax
    // and fail with a clear message so the developer can re-pin.
    if std::env::var("FOLIOM_REGEN_INVENTORY").is_ok() {
        eprintln!("--- FOLIOM_REGEN_INVENTORY ---");
        eprintln!("EXPECTED_SCANNED = {};", report["scannedFiles"]);
        eprintln!("EXPECTED_JOURNALS = {};", report["journalFiles"]);
        eprintln!("EXPECTED_PAGES = {};", report["pageFiles"]);
        eprintln!(
            "EXPECTED_BLOCK_PROPERTY_FILES = {};",
            report["blockPropertyFiles"]
        );
        eprintln!("EXPECTED_DRAWER_FILES = {};", report["drawerFiles"]);
        eprintln!("const EXPECTED_PATTERNS: &[(&str, u32, u32)] = &[");
        for p in report["patterns"].as_array().unwrap() {
            eprintln!(
                "    ({:?}, {}, {}),",
                p["name"].as_str().unwrap(),
                p["filesWith"],
                p["occurrences"]
            );
        }
        eprintln!("];");
        panic!("FOLIOM_REGEN_INVENTORY set — copy the above into EXPECTED_*");
    }

    // Top-level pins.
    assert_eq!(report["scannedFiles"].as_u64(), Some(EXPECTED_SCANNED));
    assert_eq!(report["journalFiles"].as_u64(), Some(EXPECTED_JOURNALS));
    assert_eq!(report["pageFiles"].as_u64(), Some(EXPECTED_PAGES));
    assert_eq!(
        report["blockPropertyFiles"].as_u64(),
        Some(EXPECTED_BLOCK_PROPERTY_FILES)
    );
    assert_eq!(
        report["drawerFiles"].as_u64(),
        Some(EXPECTED_DRAWER_FILES)
    );

    // Per-pattern pins.
    let patterns = report["patterns"].as_array().expect("patterns is array");
    assert_eq!(
        patterns.len(),
        EXPECTED_PATTERNS.len(),
        "pattern count drifted — update EXPECTED_PATTERNS"
    );
    for (i, (name, files_with, occurrences)) in EXPECTED_PATTERNS.iter().enumerate() {
        let p = &patterns[i];
        assert_eq!(p["name"].as_str(), Some(*name), "pattern[{i}].name");
        assert_eq!(
            p["filesWith"].as_u64(),
            Some(*files_with as u64),
            "pattern[{i}]={name} filesWith"
        );
        assert_eq!(
            p["occurrences"].as_u64(),
            Some(*occurrences as u64),
            "pattern[{i}]={name} occurrences"
        );
    }
}

#[test]
fn test_search_finds_known_pattern() {
    let data_home = TempDir::new().unwrap();
    let corpus = synthetic_corpus();

    let out = foliom_cmd(&data_home)
        .arg("index")
        .arg(&corpus)
        .output()
        .expect("index ran");
    assert!(out.status.success());

    // "alias" appears in the block-properties fixture; safe FTS5 query.
    let out = foliom_cmd(&data_home)
        .arg("search")
        .arg(&corpus)
        .arg("alias")
        .arg("--json")
        .output()
        .expect("search ran");
    assert!(
        out.status.success(),
        "search failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let hits: Value = serde_json::from_slice(&out.stdout).expect("search emits JSON");
    let arr = hits.as_array().expect("search returns array");
    assert!(!arr.is_empty(), "expected ≥1 hit for 'alias'");
    // Every hit must carry pagePath / blockId / snippet keys (JSON contract).
    for hit in arr {
        assert!(hit["pagePath"].is_string(), "pagePath must be string");
        assert!(hit["blockId"].is_i64(), "blockId must be int");
        assert!(hit["snippet"].is_string(), "snippet must be string");
    }
}

#[test]
fn test_dump_tree_synthetic_page_returns_blocks() {
    let data_home = TempDir::new().unwrap();
    let corpus = synthetic_corpus();

    let out = foliom_cmd(&data_home)
        .arg("index")
        .arg(&corpus)
        .output()
        .expect("index ran");
    assert!(out.status.success());

    // page name is the file stem; pages/05-links-and-tags.md → "05-links-and-tags".
    let out = foliom_cmd(&data_home)
        .arg("dump-tree")
        .arg(&corpus)
        .arg("05-links-and-tags")
        .arg("--json")
        .output()
        .expect("dump-tree ran");
    assert!(
        out.status.success(),
        "dump-tree failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tree: Value = serde_json::from_slice(&out.stdout).expect("dump-tree JSON");
    let nodes = tree.as_array().expect("tree is array");
    assert!(
        !nodes.is_empty(),
        "expected ≥1 top-level block in 05-links-and-tags"
    );
    // Every node has depth / raw / children.
    for node in nodes {
        assert!(node["depth"].is_i64(), "depth is int");
        assert!(node["raw"].is_string(), "raw is string");
        assert!(node["children"].is_array(), "children is array");
    }
}

#[test]
fn test_reindex_idempotent_unchanged_count() {
    let data_home = TempDir::new().unwrap();
    let corpus = synthetic_corpus();

    // First pass: everything added.
    let out = foliom_cmd(&data_home)
        .arg("index")
        .arg(&corpus)
        .arg("--json")
        .output()
        .expect("first index ran");
    assert!(out.status.success());
    let stats: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stats["added"].as_u64(), Some(EXPECTED_SCANNED));
    assert_eq!(stats["modified"].as_u64(), Some(0));

    // Second pass: everything unchanged (cache hit on (mtime, size)).
    let out = foliom_cmd(&data_home)
        .arg("reindex")
        .arg(&corpus)
        .arg("--json")
        .output()
        .expect("reindex ran");
    assert!(out.status.success());
    let stats: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stats["added"].as_u64(), Some(0));
    assert_eq!(stats["modified"].as_u64(), Some(0));
    assert_eq!(
        stats["unchanged"].as_u64(),
        Some(EXPECTED_SCANNED),
        "incremental reindex must report all files unchanged"
    );

    // Full pass: re-reads every file; hash matches → mtime_touched.
    let out = foliom_cmd(&data_home)
        .arg("reindex")
        .arg(&corpus)
        .arg("--full")
        .arg("--json")
        .output()
        .expect("reindex --full ran");
    assert!(out.status.success());
    let stats: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stats["modified"].as_u64(), Some(0));
    assert_eq!(
        stats["mtimeTouched"].as_u64(),
        Some(EXPECTED_SCANNED),
        "Full mode on unchanged corpus must report all files mtime_touched"
    );
}
