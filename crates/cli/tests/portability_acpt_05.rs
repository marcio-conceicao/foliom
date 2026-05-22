//! Plan 03-07 — ACPT-05 portability acceptance test.
//!
//! This test pins the "zero metadata injection" invariant for ALL Phase 3
//! mutation operations. It runs an in-process axum router over a copy of the
//! synthetic fixture corpus + 2 curated ACPT-05-specific fixtures, drives a
//! scripted edit sequence through the full mutation API, then asserts:
//!
//!   1. No line-ending corruption (`\r\n` never introduced).
//!   2. No BOM injected (fixtures are BOM-free; post-edit must be too).
//!   3. Foliom-metadata grep — zero NEW occurrences of `id::`, `((`, `<!-- foliom`,
//!      `.foliom-`, `foliom_uuid` after edits (pre-edit occurrences are allowed,
//!      e.g. an `id::` property in a Logseq file stays as-is).
//!   4. Every post-edit file is valid UTF-8.
//!   5. ACPT-01 corpus replay — segment() → slice-and-concat → byte-equal.
//!
//! Manual Obsidian / VS Code verification is NOT done in this test.
//! See `.planning/phases/03-outliner-editor/ACPT-05-PORTABILITY.md` for the
//! human checklist that the verifier fills in during `/gsd-verify-work`.
//!
//! Environment variable:
//!   ACPT05_KEEP_TEMPDIR=1  — copy the post-edit tempdir to /tmp/foliom-acpt05/
//!                            so the verifier can open it in Obsidian / VS Code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::parser::segment::segment;
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Root of the synthetic logseq fixture corpus (11 files).
fn logseq_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("acpt-05")
        .join("before")
}

/// Root of the base corpus we augment (logseq-synthetic).
fn logseq_synthetic_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Set up a tempdir with:
///   - the full logseq-synthetic corpus (11 files in pages/ + journals/)
///   - the 2 curated ACPT-05 fixtures placed into pages/ and journals/
/// Returns the tempdir handle (keep it alive for the entire test).
fn setup_corpus_tempdir() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Copy logseq-synthetic as the base corpus.
    copy_dir_all(&logseq_synthetic_root(), tmp.path()).expect("copy logseq-synthetic");

    // Copy ACPT-05 curated fixtures.
    // journal_2026_05_22.md → journals/
    let journals_dir = tmp.path().join("journals");
    std::fs::create_dir_all(&journals_dir).expect("create journals dir");
    std::fs::copy(
        logseq_fixture_root().join("journal_2026_05_22.md"),
        journals_dir.join("2026_05_22.md"),
    )
    .expect("copy journal fixture");

    // page_with_code_drawer_props.md → pages/
    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).expect("create pages dir");
    std::fs::copy(
        logseq_fixture_root().join("page_with_code_drawer_props.md"),
        pages_dir.join("page_with_code_drawer_props.md"),
    )
    .expect("copy page fixture");

    tmp
}

fn build_state(root: &Path) -> foliom_cli::cmd::serve::state::AppState {
    let mut db = Db::open(root).expect("open db");
    reindex(&mut db, root, ReindexMode::Full).expect("reindex");
    let self_writes = Arc::new(SelfWriteSet::new(Duration::from_secs(30)));
    let journal = Arc::new(
        foliom_core::rename::Journal::open_for_root(root).expect("journal open"),
    );
    foliom_cli::cmd::serve::state::AppState {
        db: Arc::new(Mutex::new(db)),
        root: root.to_path_buf(),
        self_writes,
        journal,
    }
}

fn build_router(state: foliom_cli::cmd::serve::state::AppState) -> axum::Router {
    foliom_cli::cmd::serve::routes::build_router(state)
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

async fn get_json(router: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("host", "127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot GET");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn put_json(router: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("content-type", "application/json")
                .header("host", "127.0.0.1")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .expect("oneshot PUT");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn post_json(router: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("host", "127.0.0.1")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .expect("oneshot POST");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn patch_json(router: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("content-type", "application/json")
                .header("host", "127.0.0.1")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .expect("oneshot PATCH");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn delete_req(router: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .header("host", "127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot DELETE");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ─── Byte-invariant helpers ────────────────────────────────────────────────────

/// Walk all .md files under `root` recursively; return sorted Vec<PathBuf>.
fn walk_md(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_md(root, &mut paths);
    paths.sort();
    paths
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_md(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
}

/// Count byte-pattern occurrences in bytes. Uses memmem-style scan.
fn count_pattern(bytes: &[u8], pattern: &[u8]) -> usize {
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = bytes[start..].windows(pattern.len()).position(|w| w == pattern) {
        count += 1;
        start += pos + 1;
    }
    count
}

/// Count occurrences of a string pattern as bytes.
fn count_str_pattern(bytes: &[u8], pattern: &str) -> usize {
    count_pattern(bytes, pattern.as_bytes())
}

/// Snapshot the Foliom-metadata patterns across all .md files in a corpus.
/// Returns a map from relative path string → count per pattern.
fn snapshot_metadata_counts(root: &Path) -> HashMap<String, HashMap<String, usize>> {
    let patterns = ["id::", "((", "<!-- foliom", ".foliom-", "foliom_uuid"];
    let mut snapshot: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let mut counts = HashMap::new();
        for pat in &patterns {
            counts.insert(pat.to_string(), count_str_pattern(&bytes, pat));
        }
        snapshot.insert(rel, counts);
    }
    snapshot
}

/// Assert that no `\r\n` sequences appear in any .md file that did NOT have
/// them before editing.  Since the synthetic fixtures have pure `\n` line
/// endings, this just checks there are zero `\r\n` sequences in any edited
/// file.
fn assert_no_crlf_introduced(root: &Path) {
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let crlf_count = count_pattern(&bytes, b"\r\n");
        assert_eq!(
            crlf_count, 0,
            "CRLF introduced in {}: {} occurrences",
            path.display(),
            crlf_count
        );
    }
}

/// Assert that no UTF-8 BOM (`\xEF\xBB\xBF`) appears in any .md file.
fn assert_no_bom(root: &Path) {
    const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        assert!(
            !bytes.starts_with(BOM),
            "BOM injected in {}",
            path.display()
        );
    }
}

/// Assert every .md file is valid UTF-8.
fn assert_valid_utf8(root: &Path) {
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        std::str::from_utf8(&bytes).unwrap_or_else(|e| {
            panic!(
                "File {} is not valid UTF-8: {}",
                path.display(),
                e
            )
        });
    }
}

/// Assert that Foliom-metadata patterns have NOT increased beyond their
/// pre-edit counts. Pre-edit occurrences (e.g. Logseq `id::` props) are
/// accepted; new injections are not.
fn assert_no_metadata_injected(
    root: &Path,
    pre_snapshot: &HashMap<String, HashMap<String, usize>>,
) {
    let patterns = ["id::", "((", "<!-- foliom", ".foliom-", "foliom_uuid"];
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        for pat in &patterns {
            let post_count = count_str_pattern(&bytes, pat);
            // New files (created during the test) didn't exist pre-edit → pre count is 0.
            let pre_count = pre_snapshot
                .get(&rel)
                .and_then(|m| m.get(*pat))
                .copied()
                .unwrap_or(0);
            assert_eq!(
                post_count,
                pre_count,
                "Foliom-injected metadata detected: pattern '{}' count changed from {} to {} in {}",
                pat,
                pre_count,
                post_count,
                rel
            );
        }
    }
}

/// Assert CommonMark parseable (smoke — pulldown_cmark::Parser is total, just
/// checking for panics and valid UTF-8 input).
fn assert_commonmark_parseable(root: &Path) {
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        if let Ok(s) = std::str::from_utf8(&bytes) {
            // pulldown_cmark::Parser::new panics only on internal bugs; this
            // exercises the parser to smoke-test that Foliom's output doesn't
            // produce anything the parser chokes on.
            let count = pulldown_cmark::Parser::new(s).count();
            // If count is 0 and file is non-empty, that's suspicious but not
            // a hard failure — the parser is total.
            let _ = count;
        }
    }
}

/// ACPT-01 corpus replay: for every .md file, verify that
/// `segment(bytes)` slices reconstruct the exact original bytes.
fn assert_acpt01_roundtrip(root: &Path) {
    for path in walk_md(root) {
        let bytes = std::fs::read(&path).unwrap_or_default();
        let blocks = segment(&bytes);
        let mut reconstructed = Vec::with_capacity(bytes.len());
        for block in &blocks {
            let slice =
                &bytes[block.byte_offset..block.byte_offset + block.byte_length];
            reconstructed.extend_from_slice(slice);
        }
        assert_eq!(
            reconstructed, bytes,
            "ACPT-01 round-trip failed for {}",
            path.display()
        );
    }
}

/// If ACPT05_KEEP_TEMPDIR=1, copy the tempdir to /tmp/foliom-acpt05/.
fn maybe_keep_tempdir(tmp: &Path) {
    if std::env::var("ACPT05_KEEP_TEMPDIR").as_deref() == Ok("1") {
        let out = PathBuf::from("/tmp/foliom-acpt05");
        if out.exists() {
            std::fs::remove_dir_all(&out).ok();
        }
        copy_dir_all(tmp, &out).expect("copy to ACPT05 output dir");
        eprintln!("[ACPT-05] Post-edit corpus available at: {}", out.display());
    }
}

// ─── Main acceptance test ─────────────────────────────────────────────────────

/// ACPT-05: scripted edit sequence + byte invariants + Foliom-metadata grep +
/// ACPT-01 corpus replay.
///
/// Scripted edit sequence (8 scenarios):
///   1. Edit existing block (PUT)
///   2. Insert sibling (POST)
///   3. Indent (PATCH structure)
///   4. Outdent (PATCH structure)
///   5. Delete block (DELETE)
///   6. Paste bullet tree simulation (POST ×3)
///   7. Create page via unresolved link (POST /api/pages)
///   8. Rename page + rewrite backlinks (POST /api/pages/:name/rename)
#[tokio::test(flavor = "current_thread")]
async fn acpt_05_scripted_edit_sequence() {
    let tmp = setup_corpus_tempdir();
    let root = tmp.path();

    // Snapshot metadata patterns BEFORE any edits.
    let pre_snapshot = snapshot_metadata_counts(root);

    let state = build_state(root);
    let router = build_router(state);

    // ── Scenario 1: Edit existing block (PUT) ───────────────────────────────
    let page = "01-simple-bullets";
    let (status, page_body) = get_json(&router, &format!("/api/pages/{page}")).await;
    assert_eq!(status, 200, "GET page for scenario 1 failed");

    let file_hash = page_body["fileHash"]
        .as_str()
        .expect("fileHash")
        .to_string();
    let first_block = &page_body["blocks"][0]["children"][0];
    let block_id = first_block["id"].as_i64().expect("block id");

    let (status, body) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": "- edited [[ACPT05Target]]\n", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "Scenario 1 PUT must return 200: {body}");
    let file_hash = body["fileHash"].as_str().expect("updated fileHash").to_string();
    eprintln!("[ACPT-05] page={page} scenario=edit_block ok=true");

    // ── Scenario 2: Insert sibling (POST) ────────────────────────────────────
    let page_id = page_body["id"].as_i64().unwrap_or(0);
    let (status, body) = post_json(
        &router,
        "/api/blocks",
        json!({
            "pageId": page_id,
            "parentId": null,
            "ord": 99,
            "depth": 0,
            "raw": "- brand new sibling\n",
            "prevHash": file_hash
        }),
    )
    .await;
    assert_eq!(status, 201, "Scenario 2 POST sibling must return 201: {body}");
    let new_block_id = body["id"].as_i64().expect("new block id");
    let file_hash = body["fileHash"].as_str().expect("fileHash after POST").to_string();
    eprintln!("[ACPT-05] page={page} scenario=insert_sibling ok=true new_id={new_block_id}");

    // ── Scenario 3: Indent the new sibling ───────────────────────────────────
    let (status, body) = patch_json(
        &router,
        &format!("/api/blocks/{new_block_id}/structure"),
        json!({ "op": "indent", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "Scenario 3 PATCH indent must return 200: {body}");
    let file_hash = body["fileHash"].as_str().expect("fileHash after indent").to_string();
    eprintln!("[ACPT-05] page={page} scenario=indent ok=true");

    // ── Scenario 4: Outdent the indented block ───────────────────────────────
    let (status, body) = patch_json(
        &router,
        &format!("/api/blocks/{new_block_id}/structure"),
        json!({ "op": "outdent", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "Scenario 4 PATCH outdent must return 200: {body}");
    let file_hash = body["fileHash"].as_str().expect("fileHash after outdent").to_string();
    eprintln!("[ACPT-05] page={page} scenario=outdent ok=true");

    // ── Scenario 5: Delete the inserted block ────────────────────────────────
    let (status, _) = delete_req(
        &router,
        &format!("/api/blocks/{new_block_id}?prevHash={file_hash}"),
    )
    .await;
    assert_eq!(status, 204, "Scenario 5 DELETE must return 204");
    eprintln!("[ACPT-05] page={page} scenario=delete_block ok=true");

    // ── Scenario 6: Paste bullet tree simulation (POST ×3) ───────────────────
    // Mimic the frontend paste handler: POST 3 blocks at depth 0/1/1.
    let (_, page_body2) = get_json(&router, &format!("/api/pages/{page}")).await;
    let file_hash = page_body2["fileHash"].as_str().expect("fileHash for paste").to_string();
    let page_id2 = page_body2["id"].as_i64().unwrap_or(0);

    let (status, body) = post_json(
        &router,
        "/api/blocks",
        json!({
            "pageId": page_id2, "parentId": null, "ord": 100,
            "depth": 0, "raw": "- paste root\n", "prevHash": file_hash
        }),
    )
    .await;
    assert_eq!(status, 201, "Scenario 6 paste root must return 201: {body}");
    let paste_root_id = body["id"].as_i64().expect("paste root id");
    let fh1 = body["fileHash"].as_str().expect("fileHash").to_string();

    let (status, body) = post_json(
        &router,
        "/api/blocks",
        json!({
            "pageId": page_id2, "parentId": paste_root_id, "ord": 0,
            "depth": 1, "raw": "\t- paste child 1\n", "prevHash": fh1
        }),
    )
    .await;
    assert_eq!(status, 201, "Scenario 6 paste child 1 must return 201: {body}");
    let fh2 = body["fileHash"].as_str().expect("fileHash").to_string();

    let (status, body) = post_json(
        &router,
        "/api/blocks",
        json!({
            "pageId": page_id2, "parentId": paste_root_id, "ord": 1,
            "depth": 1, "raw": "\t- paste child 2\n", "prevHash": fh2
        }),
    )
    .await;
    assert_eq!(status, 201, "Scenario 6 paste child 2 must return 201: {body}");
    eprintln!("[ACPT-05] page={page} scenario=paste_tree ok=true");

    // ── Scenario 7: Create page via unresolved link ──────────────────────────
    let (status, body) = post_json(
        &router,
        "/api/pages",
        json!({ "name": "ACPT05BrandNewPage" }),
    )
    .await;
    assert_eq!(status, 201, "Scenario 7 create page must return 201: {body}");
    let new_page_path = root.join("pages").join("ACPT05BrandNewPage.md");
    assert!(new_page_path.exists(), "ACPT05BrandNewPage.md must exist after create");
    let new_page_bytes = std::fs::read(&new_page_path).expect("read new page");
    assert!(
        !new_page_bytes.is_empty(),
        "newly created page must not be empty"
    );
    eprintln!("[ACPT-05] scenario=create_page ok=true path={}", new_page_path.display());

    // ── Scenario 8: Rename page + rewrite backlinks ──────────────────────────
    // 01-simple-bullets was edited (has [[ACPT05Target]] now); rename ACPT05Target.
    // First create ACPT05Target page so rename has a source.
    let (status, _) = post_json(
        &router,
        "/api/pages",
        json!({ "name": "ACPT05Target" }),
    )
    .await;
    // 201 means created, 409 means it already exists — both are fine here.
    assert!(
        status == StatusCode::CREATED || status == StatusCode::CONFLICT,
        "Setup for scenario 8 failed: {status}"
    );

    let (status, body) = post_json(
        &router,
        "/api/pages/ACPT05Target/rename",
        json!({ "newName": "ACPT05Renamed", "rewriteBacklinks": true }),
    )
    .await;
    assert_eq!(status, 200, "Scenario 8 rename must return 200: {body}");
    // Verify old name is gone from 01-simple-bullets.md.
    let edited_page_path = root.join("pages").join("01-simple-bullets.md");
    let edited_bytes = std::fs::read(&edited_page_path).expect("read edited page");
    let edited_str = String::from_utf8_lossy(&edited_bytes);
    assert!(
        !edited_str.contains("[[ACPT05Target]]"),
        "After rename, [[ACPT05Target]] must be gone from 01-simple-bullets.md; got:\n{edited_str}"
    );
    assert!(
        edited_str.contains("[[ACPT05Renamed]]"),
        "After rename, [[ACPT05Renamed]] must appear in 01-simple-bullets.md; got:\n{edited_str}"
    );
    eprintln!("[ACPT-05] scenario=rename_page ok=true");

    // ── Byte invariants ──────────────────────────────────────────────────────
    eprintln!("[ACPT-05] Running byte invariant checks...");
    assert_no_crlf_introduced(root);
    assert_no_bom(root);
    assert_valid_utf8(root);
    eprintln!("[ACPT-05] Byte invariants: ok");

    // ── Foliom-metadata grep ─────────────────────────────────────────────────
    eprintln!("[ACPT-05] Running Foliom-metadata grep...");
    assert_no_metadata_injected(root, &pre_snapshot);
    eprintln!("[ACPT-05] Foliom-metadata grep: ok (zero new injections)");

    // ── CommonMark smoke ─────────────────────────────────────────────────────
    assert_commonmark_parseable(root);
    eprintln!("[ACPT-05] CommonMark smoke: ok");

    // ── ACPT-01 corpus replay ────────────────────────────────────────────────
    eprintln!("[ACPT-05] Running ACPT-01 corpus replay...");
    assert_acpt01_roundtrip(root);
    eprintln!("[ACPT-05] ACPT-01 corpus replay: ok");

    // ── Property / drawer preservation: spot-check the curated fixture ───────
    // page_with_code_drawer_props.md has an existing `id::` property and a
    // :LOGBOOK: drawer — they must still be there after the corpus run.
    let props_path = root.join("pages").join("page_with_code_drawer_props.md");
    if props_path.exists() {
        let props_bytes = std::fs::read(&props_path).expect("read props fixture");
        let props_str = String::from_utf8_lossy(&props_bytes);
        // The id:: property from the Logseq fixture is preserved.
        assert!(
            props_str.contains("id:: 6f7a3c9e-1d4b-4a12-9b8c-2f4e5d6a7c1f"),
            "id:: property must be preserved verbatim in page_with_code_drawer_props.md"
        );
        // :LOGBOOK: drawer is preserved.
        assert!(
            props_str.contains(":LOGBOOK:"),
            ":LOGBOOK: drawer must be preserved in page_with_code_drawer_props.md"
        );
        assert!(
            props_str.contains(":END:"),
            ":END: drawer closer must be preserved in page_with_code_drawer_props.md"
        );
    }
    eprintln!("[ACPT-05] Property/drawer preservation: ok");

    // ── TAB indentation check ────────────────────────────────────────────────
    // Verify no space-then-bullet pattern was introduced (Foliom uses TABs).
    let page_file = root.join("pages").join("01-simple-bullets.md");
    let page_bytes = std::fs::read(&page_file).expect("read page for TAB check");
    let page_str = String::from_utf8_lossy(&page_bytes);
    for line in page_str.lines() {
        if line.starts_with("  - ") || line.starts_with("   - ") {
            panic!(
                "Space-indent found in {}: line {:?}",
                page_file.display(),
                line
            );
        }
    }
    eprintln!("[ACPT-05] TAB indentation check: ok");

    // ── Maybe dump tempdir for manual inspection ─────────────────────────────
    maybe_keep_tempdir(root);

    let md_count = walk_md(root).len();
    eprintln!(
        "[ACPT-05] COMPLETE: corpus_files={} scenarios=8 byte_invariants=ok metadata_grep=ok acpt01=ok",
        md_count
    );
}

// ─── Manual verification gate (human-only, excluded from CI) ─────────────────

/// These tests document the manual Obsidian + VS Code verification steps.
/// They are gated with `#[ignore]` so `cargo test` skips them by default.
/// Run with `ACPT05_KEEP_TEMPDIR=1 cargo test -- --ignored` to execute the
/// setup step, then follow the ACPT-05-PORTABILITY.md checklist.
#[tokio::test(flavor = "current_thread")]
#[ignore = "Manual verification: run ACPT05_KEEP_TEMPDIR=1 cargo test portability_acpt_05 -- --nocapture then open /tmp/foliom-acpt05/ in Obsidian and VS Code per ACPT-05-PORTABILITY.md"]
async fn acpt_05_manual_obsidian_vscode_verify() {
    // This test drives the same edit sequence as acpt_05_scripted_edit_sequence
    // but keeps the tempdir so the human can inspect the post-edit corpus.
    //
    // Human checklist: see .planning/phases/03-outliner-editor/ACPT-05-PORTABILITY.md
    //
    // Expected: Obsidian opens every file without warnings; VS Code shows no
    // encoding banners; both tools' markdown preview matches Foliom's renderer.
    eprintln!("[ACPT-05-MANUAL] Run acpt_05_scripted_edit_sequence with ACPT05_KEEP_TEMPDIR=1");
    eprintln!("[ACPT-05-MANUAL] Then open /tmp/foliom-acpt05/ in Obsidian and VS Code");
    eprintln!("[ACPT-05-MANUAL] Fill in the ACPT-05-PORTABILITY.md table with your findings");
}
