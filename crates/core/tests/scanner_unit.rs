//! Integration tests for `foliom_core::scanner::walk`.
//!
//! Fixtures are built on the fly via `tempfile::TempDir` so the tests are
//! deterministic and don't depend on any committed corpus. A separate
//! smoke leg exercises the real (gitignored) `data-folder-sample/Logseq/`
//! when present, but never fails CI if it isn't.

use std::fs;
use std::path::{Path, PathBuf};

use foliom_core::scanner::{IgnoreSet, ScanEntry, walk};

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn collect_sorted(root: &Path, ignore: &IgnoreSet) -> Vec<ScanEntry> {
    let mut v: Vec<ScanEntry> = walk(root, ignore).collect();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

fn rel(root: &Path, entry: &ScanEntry) -> PathBuf {
    entry.path.strip_prefix(root).unwrap().to_path_buf()
}

#[test]
fn walk_returns_only_md_files_under_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("a.md"), "alpha");
    write(&root.join("b.md"), "bravo");
    write(&root.join("notes.txt"), "text");
    write(&root.join("image.png"), "binary");

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());

    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(names, vec![PathBuf::from("a.md"), PathBuf::from("b.md")]);
}

#[test]
fn walk_recurses_into_user_subdirs() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("top.md"), "top");
    write(&root.join("journals/2023_11_09.md"), "j");
    write(&root.join("pages/Crypto.md"), "c");

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());

    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(
        names,
        vec![
            PathBuf::from("journals/2023_11_09.md"),
            PathBuf::from("pages/Crypto.md"),
            PathBuf::from("top.md"),
        ]
    );
}

#[test]
fn walk_skips_ignored_directories() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("a.md"), "a");
    write(&root.join("assets/c.md"), "asset");
    write(&root.join("logseq/config.edn"), ":hidden []");
    write(&root.join("logseq/inside.md"), "should-not-appear");
    write(&root.join("bak/old.md"), "backup");
    write(&root.join(".git/HEAD.md"), "git");

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(names, vec![PathBuf::from("a.md")]);
}

#[test]
fn walk_skips_dotdirs_at_any_depth() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("a.md"), "a");
    write(&root.join(".obsidian/cache.md"), "x");
    write(&root.join("pages/.hidden/secret.md"), "y");

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(names, vec![PathBuf::from("a.md")]);
}

#[test]
fn walk_entries_carry_size_and_mtime_ns() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let body = "the quick brown fox\n";
    write(&root.join("note.md"), body);

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.size, body.len() as u64);
    assert!(e.mtime_ns > 0, "mtime_ns should be a positive epoch ns");
}

#[test]
fn walk_respects_extended_ignore_from_config_edn() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("a.md"), "a");
    write(&root.join("archived/old.md"), "old");
    write(&root.join("drafts/new.md"), "draft");

    let mut ig = IgnoreSet::default_logseq();
    ig.extend_from_config_edn(vec!["archived".to_string(), "drafts".to_string()]);

    let entries = collect_sorted(root, &ig);
    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(names, vec![PathBuf::from("a.md")]);
}

#[cfg(unix)]
#[test]
fn walk_does_not_follow_symlinks_to_files() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::TempDir::new().unwrap();
    let outside = tempfile::TempDir::new().unwrap();
    let target = outside.path().join("outside.md");
    write(&target, "secret");

    let root = tmp.path();
    write(&root.join("a.md"), "a");
    symlink(&target, root.join("link.md")).unwrap();

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    // `link.md` is a symlink to a file; with follow_links(false) walkdir
    // yields the symlink entry itself but file_type().is_file() is false
    // for a symlink — so it's filtered out by the .filter(is_file) stage.
    assert_eq!(names, vec![PathBuf::from("a.md")]);
}

#[cfg(unix)]
#[test]
fn walk_does_not_follow_symlinks_to_directories() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::TempDir::new().unwrap();
    let outside = tempfile::TempDir::new().unwrap();
    write(&outside.path().join("outside.md"), "outside");

    let root = tmp.path();
    write(&root.join("a.md"), "a");
    symlink(outside.path(), root.join("linkdir")).unwrap();

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    let names: Vec<_> = entries.iter().map(|e| rel(root, e)).collect();
    assert_eq!(names, vec![PathBuf::from("a.md")]);
}

#[test]
fn walk_paths_use_native_separator_until_relativepath() {
    // The scanner returns OS-native absolute paths; conversion to the
    // forward-slash storage form happens at the RelativePath boundary
    // (Plan 01-03). This test just pins that the returned path is
    // absolute and rooted under `root`.
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    write(&root.join("nested/deep/file.md"), "ok");

    let entries = collect_sorted(root, &IgnoreSet::default_logseq());
    assert_eq!(entries.len(), 1);
    assert!(entries[0].path.starts_with(root));
    assert!(entries[0].path.is_absolute());
}

#[test]
fn walk_against_real_config_edn_corpus_if_present() {
    // Smoke test against the gitignored real corpus when available.
    // In CI the directory doesn't exist; the test prints "skipping" and
    // passes. Per REVISION 2026-05-21 banner.
    let path = Path::new("../../data-folder-sample/Logseq");
    if !path.is_dir() {
        eprintln!("skipping — data-folder-sample/Logseq/ not present");
        return;
    }
    let entries: Vec<_> = walk(path, &IgnoreSet::default_logseq()).collect();
    // We don't pin a count — the real corpus changes. Just assert we got
    // *something* and every entry has an .md extension.
    assert!(!entries.is_empty(), "real corpus should yield at least one .md");
    for e in &entries {
        assert_eq!(e.path.extension().and_then(|s| s.to_str()), Some("md"));
        assert!(e.size > 0);
        assert!(e.mtime_ns > 0);
    }
}
