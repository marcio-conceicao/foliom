//! End-to-end integration test for `foliom_core::indexer::reindex`.
//!
//! Exercises:
//!   - First-pass insert against the synthetic fixture corpus.
//!   - Idempotence: second `reindex(Incremental)` reports `unchanged`.
//!   - `mtime_touched` when only mtime changes (content hash matches).
//!   - `modified` when content changes.
//!   - `deleted` when a known file is removed from disk.
//!   - Full mode re-reads but produces identical row counts.
//!   - Deleting the DB and re-running reproduces the baseline.
//!   - blocks_fts row count tracks blocks row count.
//!   - Known refs (page links + tags) extracted from page 05.
//!   - Hex-color and URL-fragment false-positives are NOT in refs (page 06).
//!
//! Per REVISION 2026-05-21 the primary corpus is the synthetic fixture
//! (10 files). The real corpus (`data-folder-sample/Logseq/`) is exercised
//! as an opt-in second leg when present locally.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use filetime::FileTime;
use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::storage::Db;

// 10 fixture files + the top-level README.md that lives alongside the
// `pages/` and `journals/` dirs in the fixture root. The scanner walks
// every `.md` under the root, so the README counts.
const SYNTHETIC_FILE_COUNT: usize = 11;

/// Recursively copy `src` into `dst`, creating `dst` if missing.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create dst");
    for entry in fs::read_dir(src).expect("read src") {
        let entry = entry.expect("entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type().expect("file_type");
        if ft.is_dir() {
            copy_dir_recursive(&from, &to);
        } else if ft.is_file() {
            fs::copy(&from, &to).expect("copy");
        }
    }
}

fn fixture_root() -> PathBuf {
    // tests/ relative to the crate root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/logseq-synthetic")
}

fn count_rows(db: &Db, table: &str) -> i64 {
    db.conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .expect("count")
}

/// Create a fresh notes-root + on-disk DB for one test scenario.
fn setup_scratch_corpus() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let notes_root = tmp.path().join("notes");
    copy_dir_recursive(&fixture_root(), &notes_root);
    let db_path = tmp.path().join("foliom.db");
    (tmp, notes_root, db_path)
}

#[test]
fn synthetic_corpus_first_pass_inserts_every_file() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental)
        .expect("reindex");

    assert_eq!(
        stats.added, SYNTHETIC_FILE_COUNT,
        "expected {SYNTHETIC_FILE_COUNT} added, got {stats:?}"
    );
    assert_eq!(stats.modified, 0, "{stats:?}");
    assert_eq!(stats.unchanged, 0, "{stats:?}");
    assert_eq!(stats.deleted, 0, "{stats:?}");
    assert_eq!(stats.mtime_touched, 0, "{stats:?}");

    // Sanity: every file shows up.
    assert_eq!(
        count_rows(&db, "files") as usize,
        SYNTHETIC_FILE_COUNT,
        "files row count"
    );
    // Every file backs at least one page row (self-page).
    let self_pages: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM pages WHERE file_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(self_pages as usize, SYNTHETIC_FILE_COUNT);

    // FTS5 stays in lockstep with blocks.
    let blocks_n = count_rows(&db, "blocks");
    let fts_n = count_rows(&db, "blocks_fts");
    assert_eq!(blocks_n, fts_n, "FTS5 row count must equal blocks row count");
}

#[test]
fn synthetic_corpus_idempotent_on_second_pass() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    assert_eq!(stats.added, 0, "{stats:?}");
    assert_eq!(stats.modified, 0, "{stats:?}");
    assert_eq!(stats.unchanged, SYNTHETIC_FILE_COUNT, "{stats:?}");
    assert_eq!(stats.deleted, 0, "{stats:?}");
    assert_eq!(stats.mtime_touched, 0, "{stats:?}");
}

#[test]
fn mtime_touch_without_content_change_marks_mtime_touched() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    // Bump mtime on one file without rewriting its content.
    let victim = notes_root.join("pages/01-simple-bullets.md");
    let one_hour_ahead = SystemTime::now() + Duration::from_secs(3600);
    filetime::set_file_mtime(&victim, FileTime::from_system_time(one_hour_ahead))
        .expect("set_file_mtime");

    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    assert_eq!(stats.mtime_touched, 1, "{stats:?}");
    assert_eq!(stats.modified, 0, "{stats:?}");
    assert_eq!(stats.unchanged, SYNTHETIC_FILE_COUNT - 1, "{stats:?}");
}

#[test]
fn content_change_triggers_modified_and_replaces_blocks() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    let baseline_blocks = count_rows(&db, "blocks");

    let victim = notes_root.join("pages/01-simple-bullets.md");
    fs::write(
        &victim,
        b"- A completely different first bullet\n- Second bullet\n",
    )
    .unwrap();

    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    assert_eq!(stats.modified, 1, "{stats:?}");
    assert_eq!(stats.added, 0, "{stats:?}");
    assert_eq!(stats.unchanged, SYNTHETIC_FILE_COUNT - 1, "{stats:?}");

    // The new file has fewer blocks; baseline should now differ.
    let after_blocks = count_rows(&db, "blocks");
    assert!(
        after_blocks != baseline_blocks,
        "block count should have changed; before={baseline_blocks} after={after_blocks}"
    );
    // FTS5 still in sync.
    assert_eq!(count_rows(&db, "blocks_fts"), after_blocks);
}

#[test]
fn deleting_a_file_cascades_blocks_and_refs() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    let before_blocks = count_rows(&db, "blocks");
    let before_files = count_rows(&db, "files");

    let victim = notes_root.join("pages/01-simple-bullets.md");
    fs::remove_file(&victim).expect("remove victim");

    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    assert_eq!(stats.deleted, 1, "{stats:?}");
    assert_eq!(stats.unchanged, SYNTHETIC_FILE_COUNT - 1, "{stats:?}");

    assert_eq!(count_rows(&db, "files"), before_files - 1);
    // Some blocks should have disappeared (the deleted file's blocks).
    let after_blocks = count_rows(&db, "blocks");
    assert!(
        after_blocks < before_blocks,
        "blocks should drop after delete: before={before_blocks} after={after_blocks}"
    );
    // FTS5 cascade.
    assert_eq!(count_rows(&db, "blocks_fts"), after_blocks);
}

#[test]
fn full_mode_on_unchanged_corpus_preserves_row_counts() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    let baseline_blocks = count_rows(&db, "blocks");
    let baseline_files = count_rows(&db, "files");
    let baseline_pages = count_rows(&db, "pages");

    let stats = reindex(&mut db, &notes_root, ReindexMode::Full).unwrap();
    // Full mode re-hashes everything; since content is unchanged it should
    // mark all as mtime_touched (since cached_mtime matches but we still
    // read+hash) — actually our impl: Full skips the (mtime,size) fast path
    // and goes straight to read+hash; hash matches cached → mtime_touched.
    assert_eq!(
        stats.mtime_touched, SYNTHETIC_FILE_COUNT,
        "Full mode on unchanged corpus should mtime_touch every file: {stats:?}"
    );
    assert_eq!(stats.added, 0);
    assert_eq!(stats.modified, 0);
    assert_eq!(stats.deleted, 0);

    assert_eq!(count_rows(&db, "blocks"), baseline_blocks);
    assert_eq!(count_rows(&db, "files"), baseline_files);
    assert_eq!(count_rows(&db, "pages"), baseline_pages);
}

#[test]
fn delete_db_and_rebuild_reproduces_row_counts() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();

    let baseline_blocks;
    let baseline_pages;
    let baseline_files;
    {
        let mut db = Db::open_at(&db_path).expect("open db");
        let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
        baseline_blocks = count_rows(&db, "blocks");
        baseline_pages = count_rows(&db, "pages");
        baseline_files = count_rows(&db, "files");
    }

    // Wipe the DB; also wipe sidecars (-wal, -shm) so re-open is fully fresh.
    fs::remove_file(&db_path).expect("remove db");
    let _ = fs::remove_file(format!("{}-wal", db_path.display()));
    let _ = fs::remove_file(format!("{}-shm", db_path.display()));

    let mut db = Db::open_at(&db_path).expect("reopen db");
    let stats = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();
    assert_eq!(stats.added, SYNTHETIC_FILE_COUNT, "{stats:?}");

    assert_eq!(count_rows(&db, "blocks"), baseline_blocks);
    assert_eq!(count_rows(&db, "pages"), baseline_pages);
    assert_eq!(count_rows(&db, "files"), baseline_files);
}

#[test]
fn page_05_emits_expected_page_links_and_tags() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    // Fetch refs for any block under page 05 — page name should be
    // "05-links-and-tags".
    let page_id: i64 = db
        .conn()
        .query_row(
            "SELECT id FROM pages WHERE name = ?",
            ["05-links-and-tags"],
            |row| row.get(0),
        )
        .expect("page 05 row");

    // Collect distinct ref-target names for that page (across its blocks).
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT p.name, r.type
             FROM refs r
             JOIN blocks b ON b.id = r.source_block
             JOIN pages  p ON p.id = r.target_page
             WHERE b.page_id = ?
             ORDER BY p.name",
        )
        .unwrap();
    let found: Vec<(String, String)> = stmt
        .query_map([page_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    // Expected targets (from fixture 05):
    //   page-link: Glauber, Speech Analytics, Marcelo
    //   tag:       urgente, Monitoria de Qualidade, conector-com-parceiro,
    //              roadmap, fim
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    for expected in [
        "Glauber",
        "Speech Analytics",
        "Marcelo",
        "urgente",
        "Monitoria de Qualidade",
        "conector-com-parceiro",
        "roadmap",
        "fim",
    ] {
        assert!(
            names.contains(&expected),
            "expected ref '{expected}' missing; got {names:?}"
        );
    }
}

#[test]
fn page_06_hex_and_url_false_positives_do_not_become_refs() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    let page_id: i64 = db
        .conn()
        .query_row(
            "SELECT id FROM pages WHERE name = ?",
            ["06-hex-url-heading-not-tag"],
            |row| row.get(0),
        )
        .expect("page 06 row");

    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT p.name FROM refs r
             JOIN blocks b ON b.id = r.source_block
             JOIN pages  p ON p.id = r.target_page
             WHERE b.page_id = ?",
        )
        .unwrap();
    let found: Vec<String> = stmt
        .query_map([page_id], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    // None of these are real tags: hex colors, URL fragment, in-code, in-fence.
    for forbidden in [
        "fff",
        "1a2b3c",
        "section-anchor",
        "not-a-tag",
        "also-not-a-tag",
    ] {
        assert!(
            !found.iter().any(|n| n == forbidden),
            "false-positive '{forbidden}' leaked into refs: {found:?}"
        );
    }
    // The legit one DOES show up.
    assert!(found.iter().any(|n| n == "legit"), "expected #legit in {found:?}");
}

#[test]
fn journal_page_gets_journal_kind_and_iso_date() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    let (kind, journal_date): (String, Option<String>) = db
        .conn()
        .query_row(
            "SELECT kind, journal_date FROM pages WHERE name = ?",
            ["2024_03_15"],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("journal page row");
    assert_eq!(kind, "journal");
    assert_eq!(journal_date.as_deref(), Some("2024-03-15"));
}

#[test]
fn percent_2f_filename_becomes_slash_in_page_name() {
    let (_tmp, notes_root, db_path) = setup_scratch_corpus();
    let mut db = Db::open_at(&db_path).expect("open db");

    let _ = reindex(&mut db, &notes_root, ReindexMode::Incremental).unwrap();

    // Filename Parent%2FChild.md → page name "Parent/Child".
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM pages WHERE name = ?",
            ["Parent/Child"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "expected Parent/Child page row");
}

/// Opt-in real-corpus smoke test. Skipped silently when the gitignored
/// `data-folder-sample/Logseq/` is not present (CI environment).
#[test]
fn real_corpus_smoke_if_present() {
    // Walk up from CARGO_MANIFEST_DIR (crates/core) to repo root.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let real = repo_root.join("data-folder-sample/Logseq");
    if !real.is_dir() {
        eprintln!("skipping — data-folder-sample/Logseq not present");
        return;
    }
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("foliom.db");
    let mut db = Db::open_at(&db_path).expect("open db");

    let stats = reindex(&mut db, &real, ReindexMode::Incremental)
        .expect("real-corpus reindex should not panic");

    eprintln!("real-corpus reindex stats: {stats:?}");
    assert!(stats.added > 0, "expected at least one file: {stats:?}");
    // Second pass must be idempotent.
    let stats2 = reindex(&mut db, &real, ReindexMode::Incremental).unwrap();
    assert_eq!(stats2.added, 0, "{stats2:?}");
    assert_eq!(stats2.modified, 0, "{stats2:?}");
}
