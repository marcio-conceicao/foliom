//! Integration tests for plan 02-02 — the seven REST endpoints from D-24.
//!
//! Each test spawns `foliom serve <synthetic-fixture> --port 0` as a child
//! process (just like `serve_boot.rs`), reads the bound port from stdout,
//! issues HTTP requests with `ureq`, and asserts shape and contents.
//!
//! The integration tests share a single child process via a `OnceLock` to
//! amortize the ~1s startup-reindex cost across all assertions. SIGINT is
//! sent in a `dtor`-style cleanup at the end of the last test in the file
//! (cargo runs tests in this file on a single thread per the lack of
//! `#[test]` parallelism guarantees).
//!
//! Why per-test bodies share state: `cargo test` runs tests in parallel
//! threads, but they all share the process. A static handle lets the helper
//! lazy-spawn once and every test reuse the same port. Cleanup happens on
//! process exit (axum's child is dropped when the test runner exits, which
//! signals SIGPIPE → the server's graceful-shutdown future never sees ctrl_c
//! but the child is killed). Acceptable for a test binary — the
//! graceful-shutdown path is covered by `serve_boot.rs`.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

/// Single shared server: lazy-init on first use, kept alive for the rest of
/// the test binary's lifetime (cargo kills the process at end-of-run).
fn server_port() -> u16 {
    static PORT: OnceLock<u16> = OnceLock::new();
    static CHILD: OnceLock<Mutex<Child>> = OnceLock::new();
    *PORT.get_or_init(|| {
        let (child, port) = spawn_serve();
        // Stash the child so it is not dropped (and therefore not killed)
        // until the test binary exits.
        let _ = CHILD.set(Mutex::new(child));
        port
    })
}

fn base_url() -> String {
    format!("http://127.0.0.1:{}", server_port())
}

fn spawn_serve() -> (Child, u16) {
    let mut cmd = Command::cargo_bin("foliom").expect("locate foliom bin");
    cmd.arg("serve")
        .arg(fixture_root())
        .arg("--port")
        .arg("0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn foliom serve");

    let stdout = child.stdout.take().expect("stdout pipe");
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO);
        let line = rx
            .recv_timeout(remaining)
            .expect("timeout waiting for serve banner");
        if let Some(p) = parse_port(&line) {
            return (child, p);
        }
    }
}

fn parse_port(line: &str) -> Option<u16> {
    let after = line.split("http://127.0.0.1:").nth(1)?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn get_json(path: &str) -> Value {
    let url = format!("{}{}", base_url(), path);
    let resp = ureq::get(&url).call().expect("GET ok");
    assert_eq!(resp.status(), 200, "expected 200 from {url}");
    resp.into_json().expect("body is JSON")
}

// ---------------------------------------------------------------------------
// Task 1: page list / detail / titles / journals
// ---------------------------------------------------------------------------

#[test]
fn pages_list_returns_summary_array() {
    let body = get_json("/api/pages");
    let arr = body.as_array().expect("array");
    assert!(!arr.is_empty(), "synthetic fixture must yield pages");
    for entry in arr {
        assert!(entry.get("name").and_then(Value::as_str).is_some());
        assert!(entry.get("isJournal").and_then(Value::as_bool).is_some());
        assert!(entry.get("isResolved").and_then(Value::as_bool).is_some());
    }
    // Spot check: Parent/Child must appear with canonical (decoded) name.
    assert!(arr.iter().any(|e| e["name"] == "Parent/Child"));
    // And the canonical journal name.
    assert!(arr.iter().any(|e| e["name"] == "2024_03_15" && e["isJournal"] == true));
}

#[test]
fn pages_detail_percent_2f_round_trips_to_canonical_name() {
    // URL has the encoded form; the canonical name in the JSON body must
    // be the decoded form `Parent/Child` (LNK-02, D-37).
    let body = get_json("/api/pages/Parent%2FChild");
    assert_eq!(body["name"], "Parent/Child");
    assert_eq!(body["isJournal"], false);
    let blocks = body["blocks"].as_array().expect("blocks array");
    // Synthetic fixture has a depth=-1 prelude root then content.
    assert!(!blocks.is_empty(), "Parent/Child must have blocks");
}

#[test]
fn pages_detail_journal_has_formatted_title_and_prelude_root() {
    let body = get_json("/api/pages/2024_03_15");
    assert_eq!(body["isJournal"], true);
    assert_eq!(body["formattedTitle"], "March 15th, 2024");
    let blocks = body["blocks"].as_array().expect("blocks");
    // First root should be the depth=-1 prelude (segmenter contract).
    let first = &blocks[0];
    assert_eq!(first["depth"], -1, "first root must be prelude");
    let children = first["children"].as_array().expect("prelude children");
    assert!(!children.is_empty(), "prelude must own top-level blocks");
    // Verify the nested-tree shape: at least one child has further children.
    let any_with_children = children
        .iter()
        .any(|c| c["children"].as_array().map(|a| !a.is_empty()).unwrap_or(false));
    assert!(any_with_children, "deep nesting expected in 2024_03_15.md");
}

#[test]
fn pages_detail_returns_404_on_missing() {
    let url = format!("{}/api/pages/NonExistentPage", base_url());
    let result = ureq::get(&url).call();
    match result {
        Err(ureq::Error::Status(404, _)) => { /* expected */ }
        other => panic!("expected 404, got {other:?}"),
    }
}

#[test]
fn titles_list_returns_string_array() {
    let body = get_json("/api/page-titles");
    let arr = body.as_array().expect("string array");
    assert!(arr.iter().all(|v| v.is_string()));
    let names: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
    assert!(names.contains(&"Parent/Child"));
    assert!(names.contains(&"2024_03_15"));
}

#[test]
fn journals_today_returns_302_with_location() {
    let url = format!("{}/api/journals/today", base_url());
    // ureq follows redirects by default; turn that off so we can inspect 302.
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let resp = agent.get(&url).call();
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(302, r)) => r,
        Err(other) => panic!("expected 302, got {other:?}"),
    };
    assert_eq!(resp.status(), 302, "today should redirect");
    let location = resp.header("location").expect("Location header");
    assert!(
        location.starts_with("/#/journals/"),
        "Location must point at SPA hash route, got {location:?}"
    );
    // Trailing path must be YYYY_MM_DD (10 chars with underscores at 4/7).
    let stem = location.trim_start_matches("/#/journals/");
    assert_eq!(stem.len(), 10, "YYYY_MM_DD length");
    let b = stem.as_bytes();
    assert_eq!(b[4], b'_');
    assert_eq!(b[7], b'_');
}

#[test]
fn journals_range_returns_formatted_entries() {
    let body = get_json("/api/journals?from=2024-03-14&to=2024-03-16");
    let arr = body.as_array().expect("array");
    let entry = arr
        .iter()
        .find(|e| e["name"] == "2024_03_15")
        .expect("2024_03_15 in range");
    assert_eq!(entry["date"], "2024-03-15");
    assert_eq!(entry["formattedTitle"], "March 15th, 2024");
}

// ---------------------------------------------------------------------------
// Task 2: backlinks + search (FTS5 + tag-refs)
// ---------------------------------------------------------------------------

#[test]
fn backlinks_for_speech_analytics_returns_referencing_blocks() {
    // `Speech Analytics` is referenced from pages/05 and journals/2024_03_15.
    let body = get_json("/api/pages/Speech%20Analytics/backlinks");
    let arr = body.as_array().expect("array");
    assert!(
        !arr.is_empty(),
        "Speech Analytics must have at least one backlink"
    );
    for hit in arr {
        assert!(hit["page"].as_str().is_some());
        assert!(hit["blockId"].as_i64().is_some());
        assert!(hit["snippet"].as_str().is_some());
    }
}

#[test]
fn search_content_returns_mark_highlighted_snippet() {
    let body = get_json("/api/search?q=Glauber&limit=10");
    let arr = body.as_array().expect("array");
    assert!(!arr.is_empty(), "Glauber should match at least one block");
    let any_mark = arr
        .iter()
        .filter_map(|h| h["snippet"].as_str())
        .any(|s| s.contains("<mark>") && s.contains("</mark>"));
    assert!(
        any_mark,
        "at least one snippet must contain <mark>…</mark> markers"
    );
}

#[test]
fn search_empty_query_returns_empty_array() {
    let body = get_json("/api/search?q=&limit=10");
    let arr = body.as_array().expect("array");
    assert!(arr.is_empty(), "empty query short-circuits to []");
}

#[test]
fn search_with_colon_sanitized_returns_empty() {
    let body = get_json("/api/search?q=name%3Avalue");
    let arr = body.as_array().expect("array");
    assert!(
        arr.is_empty(),
        "unquoted `:` in query must be rejected by sanitizer"
    );
}

#[test]
fn search_kind_tag_routes_through_refs() {
    // `#urgente` exists as a tag in journals/2024_03_15.md and pages/05.
    let body = get_json("/api/search?q=urgente&kind=tag");
    let arr = body.as_array().expect("array");
    assert!(
        !arr.is_empty(),
        "tag `urgente` should be found via refs lookup"
    );
}

#[test]
fn search_kind_tag_strips_leading_hash() {
    let body = get_json("/api/search?q=%23urgente&kind=tag");
    let arr = body.as_array().expect("array");
    assert!(!arr.is_empty(), "leading # must be stripped for tag search");
}

#[test]
fn search_kind_page_returns_400() {
    let url = format!("{}/api/search?q=foo&kind=page", base_url());
    match ureq::get(&url).call() {
        Err(ureq::Error::Status(400, _)) => { /* expected */ }
        other => panic!("expected 400 for kind=page, got {other:?}"),
    }
}

#[test]
fn search_unicode_snippet_preserves_diacritics() {
    // `Avaliação` lives in pages/Avaliacao-fixture.md (created by this plan
    // alongside the other fixtures). FTS5's snippet() roundtrips UTF-8
    // bytes without corruption — Pitfall 6 verification.
    let body = get_json("/api/search?q=Avalia%C3%A7%C3%A3o");
    let arr = body.as_array().expect("array");
    assert!(
        !arr.is_empty(),
        "Avaliação fixture must exist and match (see plan 02-02 fixtures)"
    );
    let any_intact = arr
        .iter()
        .filter_map(|h| h["snippet"].as_str())
        .any(|s| s.contains("Avaliação"));
    assert!(
        any_intact,
        "snippet must preserve the exact Unicode characters (no � replacement)"
    );
}
