//! Plan 03-03 — Integration tests for the mutation REST API.
//!
//! These tests spin up an in-process axum router (no socket binding) over a
//! tempdir corpus. This avoids port conflicts, makes tests fast and parallel,
//! and gives us direct access to the AppState for white-box assertions
//! (e.g., `self_writes.take_if_present`).
//!
//! Test layout:
//!  Task 1 (Task 1 tests 1..7): PUT /api/blocks/:id  — edit block raw with
//!    conflict detection, refs re-extraction, no-id-injection, self-write
//!    registration.
//!  Task 2 (Task 2 tests 1..5): POST/PATCH/DELETE + EDT-02 end-to-end +
//!    no-id-injection.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("tests")
        .join("fixtures")
        .join("logseq-synthetic")
}

/// Copy the synthetic fixture into a fresh tempdir so each test has an
/// isolated, writable notes root.
fn setup_tempdir() -> TempDir {
    let src = fixture_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir_all(&src, tmp.path()).expect("copy fixture");
    tmp
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
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

/// Build an AppState + indexed Db for the given notes root.
fn build_state(
    root: &std::path::Path,
) -> (
    foliom_cli::cmd::serve::state::AppState,
    Arc<SelfWriteSet>,
) {
    let mut db = Db::open(root).expect("open db");
    reindex(&mut db, root, ReindexMode::Full).expect("reindex");
    let self_writes = Arc::new(SelfWriteSet::new(Duration::from_secs(30)));
    let journal = Arc::new(
        foliom_core::rename::Journal::open_for_root(root).expect("journal open"),
    );
    let state = foliom_cli::cmd::serve::state::AppState {
        db: Arc::new(Mutex::new(db)),
        root: root.to_path_buf(),
        self_writes: self_writes.clone(),
        journal,
    };
    (state, self_writes)
}

/// Build the full axum router and return it.
fn build_router(
    state: foliom_cli::cmd::serve::state::AppState,
) -> axum::Router {
    foliom_cli::cmd::serve::routes::build_router(state)
}

/// POST JSON to the router, return (status, parsed body).
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
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// PUT JSON to the router.
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
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// PATCH JSON to the router.
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
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// DELETE to the router.
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
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// GET from the router.
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
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// Fetch a page detail and return (file_hash, first non-prelude block id, block raw).
async fn get_first_block(router: &axum::Router, page: &str) -> (String, i64, String) {
    let (status, body) = get_json(router, &format!("/api/pages/{page}")).await;
    assert_eq!(status, 200, "page detail failed: {body}");
    let file_hash = body["fileHash"]
        .as_str()
        .expect("fileHash in page detail")
        .to_string();
    // blocks[0] is the prelude (depth -1); blocks[0].children[0] is the first real block
    let first = &body["blocks"][0]["children"][0];
    let block_id = first["id"].as_i64().expect("block id");
    let raw = first["raw"].as_str().expect("raw").to_string();
    (file_hash, block_id, raw)
}

// ─── Task 1 tests ────────────────────────────────────────────────────────────

/// Test 1 (happy path): PUT a new raw, verify 200 + disk bytes updated in
/// the edited range while bytes outside the range are byte-identical.
#[tokio::test]
async fn t1_put_block_happy_path() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, _orig_raw) = get_first_block(&router, page).await;

    let new_raw = "- updated block text\n";
    let (status, body) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": new_raw, "prevHash": file_hash }),
    )
    .await;

    assert_eq!(status, 200, "PUT should return 200: {body}");
    assert!(body["fileHash"].is_string(), "response must have fileHash");
    assert!(body["blockSubtree"].is_array(), "response must have blockSubtree");

    // Verify the file on disk contains the new text.
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let disk_bytes = std::fs::read(&file_path).expect("read file");
    let disk_str = String::from_utf8_lossy(&disk_bytes);
    assert!(
        disk_str.contains("updated block text"),
        "file should contain new text; got: {disk_str}"
    );
}

/// Test 2 (stale conflict): PUT with wrong prev_hash returns 409; file unchanged.
#[tokio::test]
async fn t1_put_block_stale_hash_returns_409() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (_file_hash, block_id, _) = get_first_block(&router, page).await;
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let before = std::fs::read(&file_path).expect("read before");

    let (status, body) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": "- conflict\n", "prevHash": "0000000000000000000000000000000000000000000000000000000000000000" }),
    )
    .await;

    assert_eq!(status, 409, "stale hash must return 409: {body}");
    assert_eq!(body["error"], "stale", "error must be 'stale'");

    let after = std::fs::read(&file_path).expect("read after");
    assert_eq!(before, after, "file must be unchanged after 409");
}

/// Test 3 (no-op): PUT with unchanged raw leaves file byte-identical.
/// This is the ACPT-01 micro-rehearsal.
#[tokio::test]
async fn t1_put_block_noop_is_byte_identical() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, orig_raw) = get_first_block(&router, page).await;
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let before = std::fs::read(&file_path).expect("read before");

    let (status, _body) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": orig_raw, "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "no-op PUT must return 200");

    let after = std::fs::read(&file_path).expect("read after");
    assert_eq!(
        before, after,
        "no-op PUT must leave file byte-identical (ACPT-01 micro-rehearsal)"
    );
}

/// Test 4: PUT a block with [[NewLink]] — refs table gains a row for the target.
#[tokio::test]
async fn t1_put_block_new_link_creates_ref_row() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, _) = get_first_block(&router, page).await;

    let (status, _) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": "- hello [[NewLinkPage9999]]\n", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "PUT should succeed");

    // Verify the refs row via a backlinks query for the new page.
    let (bl_status, bl_body) =
        get_json(&router, "/api/pages/NewLinkPage9999/backlinks").await;
    assert_eq!(bl_status, 200, "backlinks endpoint for new page");
    let backlinks = bl_body.as_array().expect("array");
    assert!(
        !backlinks.is_empty(),
        "new [[NewLinkPage9999]] ref must appear in backlinks"
    );
}

/// Test 5: PUT a non-existent block id returns 404.
#[tokio::test]
async fn t1_put_block_nonexistent_returns_404() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    // Use a page to get a valid file_hash but a bogus block id.
    let (file_hash, _, _) = get_first_block(&router, "01-simple-bullets").await;

    let (status, _body) = put_json(
        &router,
        "/api/blocks/999999999",
        json!({ "raw": "- x\n", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 404, "non-existent block must return 404");
}

/// Test 6: malformed JSON body returns 400.
#[tokio::test]
async fn t1_put_block_malformed_json_returns_400() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/blocks/1")
                .header("content-type", "application/json")
                .header("host", "127.0.0.1")
                .body(Body::from(b"not json".to_vec()))
                .unwrap(),
        )
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "bad JSON must return 400");
}

/// Test 7: after a successful PUT, self_writes.take_if_present returns true.
#[tokio::test]
async fn t1_put_block_registers_self_write_hash() {
    let tmp = setup_tempdir();
    let (state, sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, _) = get_first_block(&router, page).await;

    let (status, body) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": "- self-write test\n", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "PUT must succeed: {body}");

    let new_hash_hex = body["fileHash"].as_str().expect("fileHash in response");
    let hash_bytes = hex::decode(new_hash_hex).expect("decode hex hash");
    let mut hash_array = [0u8; 32];
    hash_array.copy_from_slice(&hash_bytes);

    assert!(
        sw.take_if_present(&hash_array),
        "self_writes must have the new file hash after PUT"
    );
}

// ─── Task 2 tests ────────────────────────────────────────────────────────────

/// Task 2 Test 1: POST /api/blocks creates a new sibling block.
#[tokio::test]
async fn t2_post_block_creates_sibling() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, _block_id, _) = get_first_block(&router, page).await;

    // Fetch full page to get page_id context
    let (_, page_body) = get_json(&router, &format!("/api/pages/{page}")).await;
    let page_id = page_body["id"].as_i64().unwrap_or(0);

    let (status, body) = post_json(
        &router,
        "/api/blocks",
        json!({
            "pageId": page_id,
            "parentId": null,
            "ord": 99,
            "depth": 0,
            "raw": "- brand new block\n",
            "prevHash": file_hash
        }),
    )
    .await;

    assert_eq!(status, 201, "POST /api/blocks should return 201: {body}");
    let new_id = body["id"].as_i64().expect("new block id in response");
    assert!(new_id > 0, "new block must have positive id");

    // The new block should appear in the page tree.
    let (_, page_after) = get_json(&router, &format!("/api/pages/{page}")).await;
    let tree_json = serde_json::to_string(&page_after).unwrap();
    assert!(
        tree_json.contains("brand new block"),
        "new block must appear in page tree after POST"
    );
}

/// Task 2 Test 2: PATCH /api/blocks/:id/structure indents a block one level.
#[tokio::test]
async fn t2_patch_block_structure_indent() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, _block_id, _) = get_first_block(&router, page).await;

    // Get the full tree to find a block with a preceding sibling (for indent).
    let (_, page_body) = get_json(&router, &format!("/api/pages/{page}")).await;
    let prelude_children = &page_body["blocks"][0]["children"];
    // We need a block that has a preceding sibling so indent can work.
    // blocks[0] children: first child is at index 0, second at index 1.
    let second_block = &prelude_children[1];
    let second_id = second_block["id"].as_i64().expect("second block id");

    let (status, body) = patch_json(
        &router,
        &format!("/api/blocks/{second_id}/structure"),
        json!({ "op": "indent", "prevHash": file_hash }),
    )
    .await;

    assert_eq!(status, 200, "PATCH /structure indent must return 200: {body}");

    // After indent, check the file has an extra TAB prefix on that block.
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let disk_str = String::from_utf8_lossy(&std::fs::read(&file_path).unwrap()).to_string();
    // The second top-level block should now be TAB-indented.
    assert!(
        disk_str.contains("\t\t- Another top-level") || disk_str.contains("\t- Another top-level"),
        "indented block should gain TAB prefix in file; got:\n{disk_str}"
    );
}

/// Task 2 Test 3: DELETE /api/blocks/:id removes the block from the file.
#[tokio::test]
async fn t2_delete_block_removes_from_file() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, orig_raw) = get_first_block(&router, page).await;

    let (status, _body) = delete_req(
        &router,
        &format!("/api/blocks/{block_id}?prevHash={file_hash}"),
    )
    .await;
    assert_eq!(status, 204, "DELETE should return 204");

    // The deleted block's raw should not appear in the file.
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let disk_str = String::from_utf8_lossy(&std::fs::read(&file_path).unwrap()).to_string();
    // Strip segmenter prefix to get the content part.
    let content = orig_raw
        .trim_start_matches('\t')
        .trim_start_matches("- ");
    assert!(
        !disk_str.contains(content),
        "deleted block content should not appear in file after DELETE; got:\n{disk_str}"
    );
}

/// Task 2 Test 4 (no-id-injection): PUT a block with [[Foo]], then verify
/// no `id::` or `((uuid))` sequences appear in the file. (D-13 / PRD §5.6)
#[tokio::test]
async fn t2_no_id_injection_after_put() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";
    let (file_hash, block_id, _) = get_first_block(&router, page).await;
    let file_path = tmp
        .path()
        .join("pages")
        .join("01-simple-bullets.md");
    let before_bytes = std::fs::read(&file_path).unwrap();

    let (status, _) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": "- hello [[Foo]]\n", "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "PUT should succeed");

    let after_bytes = std::fs::read(&file_path).unwrap();
    let after_str = String::from_utf8_lossy(&after_bytes);

    // Count occurrences of id:: and (( in BEFORE file.
    let before_str = String::from_utf8_lossy(&before_bytes);
    let before_id_count = before_str.matches("id::").count();
    let before_paren_count = before_str.matches("((").count();

    // After must not gain new injections.
    let after_id_count = after_str.matches("id::").count();
    let after_paren_count = after_str.matches("((").count();

    assert_eq!(
        before_id_count, after_id_count,
        "PUT must not inject `id::` lines into the file (D-13)"
    );
    assert_eq!(
        before_paren_count, after_paren_count,
        "PUT must not inject `((uuid))` sequences into the file (D-13)"
    );
}

/// Task 2 Test 5 (EDT-02/EDT-03 end-to-end): GET → PUT → GET cycle.
/// Block N's raw must be updated; neighbors' raw must be unchanged.
#[tokio::test]
async fn t2_edt02_get_put_get_cycle() {
    let tmp = setup_tempdir();
    let (state, _sw) = build_state(tmp.path());
    let router = build_router(state);

    let page = "01-simple-bullets";

    // GET initial state.
    let (_, page_before) = get_json(&router, &format!("/api/pages/{page}")).await;
    let children = &page_before["blocks"][0]["children"];
    let block_id = children[0]["id"].as_i64().expect("block id");
    let neighbor_id = children[1]["id"].as_i64().expect("neighbor id");
    let neighbor_raw_before = children[1]["raw"].as_str().expect("neighbor raw").to_string();
    let file_hash = page_before["fileHash"].as_str().expect("fileHash").to_string();

    // PUT new raw to block 0.
    let new_raw = "- EDT-02 round-trip edit\n";
    let (status, _) = put_json(
        &router,
        &format!("/api/blocks/{block_id}"),
        json!({ "raw": new_raw, "prevHash": file_hash }),
    )
    .await;
    assert_eq!(status, 200, "PUT must succeed");

    // GET again and verify.
    let (_, page_after) = get_json(&router, &format!("/api/pages/{page}")).await;
    let children_after = &page_after["blocks"][0]["children"];

    // Find the edited block and the neighbor by id.
    let edited = children_after
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"].as_i64() == Some(block_id))
        .expect("edited block in response");
    let neighbor = children_after
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"].as_i64() == Some(neighbor_id))
        .expect("neighbor block in response");

    // EDT-02: edited block has new raw.
    assert_eq!(
        edited["raw"].as_str().unwrap(),
        new_raw,
        "edited block must have new raw after GET (EDT-02)"
    );
    // EDT-03: neighbor raw unchanged (no HTML round-trip contamination).
    assert_eq!(
        neighbor["raw"].as_str().unwrap(),
        neighbor_raw_before,
        "neighbor block raw must be unchanged (EDT-03)"
    );
    // fileHash must have changed.
    assert_ne!(
        page_after["fileHash"].as_str().unwrap(),
        file_hash,
        "fileHash must change after a mutation"
    );
}
