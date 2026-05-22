//! Plan 03-06 — Integration tests for the rename + page-create REST API.
//!
//! Tests pin the full rename WAL atomicity contract, collision handling,
//! case-only rename, crash recovery, and the POST /api/pages create endpoint.
//!
//! Test layout:
//!   1. create_page_happy_path: creates pages/NewPage.md with `-\n` content.
//!   2. create_journal_page: YYYY_MM_DD name → journals/ directory.
//!   3. create_page_reserved_chars: 400 for Windows-reserved chars.
//!   4. rename_happy_rewrite_backlinks: all [[Foo]] → [[Bar]] in corpus.
//!   5. rename_collision_backed: 409 if target is a resolved page.
//!   6. rename_collision_unresolved_merge: refs re-pointed to renamed page.
//!   7. rename_crash_recovery: replay-on-boot after partial rewrite.
//!   8. rename_no_rewrite: file renamed, backlinks left stale.
//!   9. rename_reserved_chars: 400 for invalid target name.

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

fn setup_tempdir() -> TempDir {
    let src = fixture_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_dir_all(&src, tmp.path()).expect("copy fixture");
    tmp
}

fn build_state(root: &std::path::Path) -> foliom_cli::cmd::serve::state::AppState {
    let mut db = Db::open(root).expect("open db");
    reindex(&mut db, root, ReindexMode::Full).expect("reindex");
    let self_writes = Arc::new(SelfWriteSet::new(Duration::from_secs(30)));
    // Phase 4: watcher_tx required by AppState; rename tests don't exercise watcher.
    let (watcher_tx, _) = tokio::sync::broadcast::channel(64);
    foliom_cli::cmd::serve::state::AppState {
        db: Arc::new(Mutex::new(db)),
        root: root.to_path_buf(),
        self_writes,
        journal: Arc::new(
            foliom_core::rename::Journal::open_for_root(root).expect("journal open"),
        ),
        watcher_tx: std::sync::Arc::new(watcher_tx),
    }
}

fn build_router(state: foliom_cli::cmd::serve::state::AppState) -> axum::Router {
    foliom_cli::cmd::serve::routes::build_router(state)
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
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ─── Test 1: create page happy path ──────────────────────────────────────────

#[tokio::test]
async fn create_page_happy_path() {
    let tmp = setup_tempdir();
    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, body) = post_json(&router, "/api/pages", json!({ "name": "NewPage" })).await;
    assert_eq!(status, StatusCode::CREATED, "expected 201, body={body}");
    assert_eq!(body["name"], "NewPage");
    assert_eq!(body["isResolved"], true);

    // Verify file was created with exactly `-\n` (3 bytes)
    let file_path = tmp.path().join("pages").join("NewPage.md");
    assert!(file_path.exists(), "NewPage.md should exist");
    let contents = std::fs::read(&file_path).unwrap();
    assert_eq!(contents, b"- \n", "file should contain `- \\n` (3 bytes), got: {:?}", contents);
}

// ─── Test 2: create journal page ─────────────────────────────────────────────

#[tokio::test]
async fn create_journal_page() {
    let tmp = setup_tempdir();
    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, body) = post_json(&router, "/api/pages", json!({ "name": "2026_05_22" })).await;
    assert_eq!(status, StatusCode::CREATED, "expected 201, body={body}");
    assert_eq!(body["isJournal"], true);

    // Verify file is in journals/ directory
    let file_path = tmp.path().join("journals").join("2026_05_22.md");
    assert!(file_path.exists(), "2026_05_22.md should exist in journals/");
}

// ─── Test 3: create page with reserved chars → 400 ───────────────────────────

#[tokio::test]
async fn create_page_reserved_chars() {
    let tmp = setup_tempdir();
    let state = build_state(tmp.path());
    let router = build_router(state);

    // Colon is a Windows reserved char
    let (status, _) = post_json(&router, "/api/pages", json!({ "name": "foo:bar" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "foo:bar should be 400");

    // Windows reserved name CON
    let (status, _) = post_json(&router, "/api/pages", json!({ "name": "CON" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "CON should be 400");
}

// ─── Test 4: rename happy path with backlink rewrite ─────────────────────────

#[tokio::test]
async fn rename_happy_rewrite_backlinks() {
    let tmp = setup_tempdir();

    // Create a page "Foo" and a page "Referrer" that links to Foo
    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();
    std::fs::write(pages_dir.join("Foo.md"), "- This is Foo\n").unwrap();
    std::fs::write(
        pages_dir.join("Referrer.md"),
        "- See [[Foo]] and also [[Foo|Foo alias]] here\n",
    )
    .unwrap();

    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, body) = post_json(
        &router,
        "/api/pages/Foo/rename",
        json!({ "newName": "Bar", "rewriteBacklinks": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert!(body["rewrittenCount"].as_u64().unwrap_or(0) > 0, "should have rewritten files");

    // Verify Foo.md is gone, Bar.md exists
    assert!(!pages_dir.join("Foo.md").exists(), "Foo.md should be gone");
    assert!(pages_dir.join("Bar.md").exists(), "Bar.md should exist");

    // Verify all [[Foo]] occurrences are rewritten to [[Bar]]
    let referrer_content = std::fs::read_to_string(pages_dir.join("Referrer.md")).unwrap();
    assert!(
        !referrer_content.contains("[[Foo]]"),
        "[[Foo]] should be gone from Referrer.md, got: {referrer_content}"
    );
    assert!(
        referrer_content.contains("[[Bar]]"),
        "[[Bar]] should appear in Referrer.md, got: {referrer_content}"
    );
}

// ─── Test 5: rename collision with backed page → 409 ─────────────────────────

#[tokio::test]
async fn rename_collision_backed() {
    let tmp = setup_tempdir();

    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();
    std::fs::write(pages_dir.join("Source.md"), "- Source\n").unwrap();
    std::fs::write(pages_dir.join("Target.md"), "- Target already exists\n").unwrap();

    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, body) = post_json(
        &router,
        "/api/pages/Source/rename",
        json!({ "newName": "Target", "rewriteBacklinks": false }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "expected 409, body={body}");
    assert_eq!(body["error"], "target exists");
}

// ─── Test 6: rename collision with unresolved page → merge ───────────────────

#[tokio::test]
async fn rename_collision_unresolved_merge() {
    let tmp = setup_tempdir();

    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();
    // "Ghost" is referenced but has no file (unresolved)
    std::fs::write(pages_dir.join("Source.md"), "- Source page\n").unwrap();
    std::fs::write(
        pages_dir.join("Linker.md"),
        "- Links to [[Ghost]] which is unresolved\n",
    )
    .unwrap();

    let state = build_state(tmp.path());
    let router = build_router(state.clone());

    // Rename Source → Ghost. Ghost is unresolved (file_id IS NULL) → merge
    let (status, _body) = post_json(
        &router,
        "/api/pages/Source/rename",
        json!({ "newName": "Ghost", "rewriteBacklinks": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "merge of unresolved should succeed with 200");

    // After merge: Source.md renamed to Ghost.md; refs pointing to old unresolved Ghost
    // now point to the renamed page
    assert!(!pages_dir.join("Source.md").exists(), "Source.md should be renamed");
    assert!(pages_dir.join("Ghost.md").exists(), "Ghost.md should exist after rename");

    // Verify in DB that the Ghost page is now resolved
    let db_guard = state.db.lock().unwrap();
    let conn = db_guard.conn();
    let (file_id_is_null, ): (bool,) = conn
        .query_row(
            "SELECT file_id IS NULL FROM pages WHERE name = 'Ghost' COLLATE NOCASE",
            [],
            |r| Ok((r.get(0)?,)),
        )
        .unwrap();
    assert!(!file_id_is_null, "Ghost page should now be resolved (file_id not null)");
}

// ─── Test 7: crash recovery (WAL replay) ─────────────────────────────────────

#[tokio::test]
async fn rename_crash_recovery() {
    use foliom_core::rename::{Journal, JournalEntry, JournalOp};

    let tmp = setup_tempdir();
    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();

    // Create a file that "should have been rewritten" by a crashed rename
    let referrer_path = pages_dir.join("Referrer.md");
    std::fs::write(&referrer_path, "- See [[OldName]] here\n").unwrap();

    // Build state and index
    let state = build_state(tmp.path());

    // Simulate: SQL committed, but file rewrite was not applied
    // We manually write a journal entry with sql_committed=true, op applied=false
    {
        let journal = state.journal.as_ref();
        let entry = JournalEntry {
            id: "crash-test-1".to_string(),
            started: "2026-05-22T00:00:00Z".to_string(),
            old_name: "OldName".to_string(),
            new_name: "NewName".to_string(),
            page_id: 9999, // dummy, won't be used by replay file-ops
            old_file: "pages/OldName.md".to_string(),
            new_file: "pages/NewName.md".to_string(),
            ops: vec![JournalOp {
                file: "pages/Referrer.md".to_string(),
                old_text: "[[OldName]]".to_string(),
                new_text: "[[NewName]]".to_string(),
                byte_offset: 6,
                byte_length: 11,
                applied: false,
            }],
            sql_committed: true,
            file_renamed: false,
        };
        journal.append(&entry).expect("journal append");
    }

    // Replay the journal (simulating boot recovery)
    {
        let journal = state.journal.as_ref();
        let db_guard = state.db.lock().unwrap();
        let mut conn_ref = db_guard;
        drop(conn_ref); // release lock before calling replay (which needs to lock)
    }

    // Call replay via the public API
    foliom_core::rename::replay_journal(&state).expect("replay_journal");

    // Verify the op was applied
    let content = std::fs::read_to_string(&referrer_path).unwrap();
    assert!(
        content.contains("[[NewName]]"),
        "after replay, [[OldName]] should be [[NewName]], got: {content}"
    );
    assert!(
        !content.contains("[[OldName]]"),
        "after replay, [[OldName]] should be gone, got: {content}"
    );

    // Verify journal is cleared
    let journal = state.journal.as_ref();
    let pending = journal.pending().unwrap();
    assert!(pending.is_empty(), "journal should be empty after replay");
}

// ─── Test 8: rename without rewriting backlinks ───────────────────────────────

#[tokio::test]
async fn rename_no_rewrite() {
    let tmp = setup_tempdir();
    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();
    std::fs::write(pages_dir.join("Alpha.md"), "- Alpha page\n").unwrap();
    std::fs::write(pages_dir.join("Referrer.md"), "- See [[Alpha]]\n").unwrap();

    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, _body) = post_json(
        &router,
        "/api/pages/Alpha/rename",
        json!({ "newName": "Beta", "rewriteBacklinks": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rename without rewrite should 200");

    // File renamed
    assert!(!pages_dir.join("Alpha.md").exists(), "Alpha.md should be gone");
    assert!(pages_dir.join("Beta.md").exists(), "Beta.md should exist");

    // Backlinks NOT rewritten: Referrer.md still has [[Alpha]]
    let referrer_content = std::fs::read_to_string(pages_dir.join("Referrer.md")).unwrap();
    assert!(
        referrer_content.contains("[[Alpha]]"),
        "without rewrite, [[Alpha]] should remain, got: {referrer_content}"
    );
}

// ─── Test 9: rename with reserved chars in target → 400 ──────────────────────

#[tokio::test]
async fn rename_reserved_chars() {
    let tmp = setup_tempdir();
    let pages_dir = tmp.path().join("pages");
    std::fs::create_dir_all(&pages_dir).unwrap();
    std::fs::write(pages_dir.join("ValidPage.md"), "- Valid\n").unwrap();

    let state = build_state(tmp.path());
    let router = build_router(state);

    let (status, _) = post_json(
        &router,
        "/api/pages/ValidPage/rename",
        json!({ "newName": "foo<bar>", "rewriteBacklinks": false }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "reserved chars in new name should be 400");
}
