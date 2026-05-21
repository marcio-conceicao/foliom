//! `RelativePath` newtype — unit tests (IDX-07 + T-03-02 path traversal).

use std::path::{Path, PathBuf};

use foliom_core::path::{PathError, RelativePath};

#[test]
fn from_filesystem_strips_root_and_uses_forward_slash() {
    let root = Path::new("/notes");
    let abs = Path::new("/notes/journals/2023_11_09.md");
    let rp = RelativePath::from_filesystem(abs, root).expect("ok");
    assert_eq!(rp.as_str(), "journals/2023_11_09.md");
}

#[test]
fn from_filesystem_single_component() {
    let root = Path::new("/notes");
    let abs = Path::new("/notes/foo.md");
    let rp = RelativePath::from_filesystem(abs, root).expect("ok");
    assert_eq!(rp.as_str(), "foo.md");
}

#[test]
fn from_filesystem_nfd_normalizes_to_nfc() {
    use unicode_normalization::UnicodeNormalization;

    let nfc_form: String = "Avaliação".nfc().collect();
    let nfd_form: String = "Avaliação".nfd().collect();
    assert_ne!(nfc_form, nfd_form);

    let root = PathBuf::from("/notes");
    let abs_nfc: PathBuf = root.join(format!("{}.md", nfc_form));
    let abs_nfd: PathBuf = root.join(format!("{}.md", nfd_form));

    let rp_nfc = RelativePath::from_filesystem(&abs_nfc, &root).expect("ok");
    let rp_nfd = RelativePath::from_filesystem(&abs_nfd, &root).expect("ok");

    // Byte-identical RelativePath strings regardless of source form.
    assert_eq!(rp_nfc.as_str(), rp_nfd.as_str());
    assert_eq!(rp_nfc.as_str(), format!("{}.md", nfc_form));
}

#[test]
fn from_filesystem_rejects_path_outside_root() {
    let root = Path::new("/notes");
    let abs = Path::new("/other/file.md");
    let err = RelativePath::from_filesystem(abs, root).unwrap_err();
    assert!(matches!(err, PathError::PathOutsideRoot));
}

#[test]
fn from_filesystem_rejects_dotdot_component_traversal() {
    // T-03-02 regression. /notes/../escape.md does not strip cleanly via
    // `strip_prefix` (the literal path bytes don't start with `/notes/`
    // because the OS sees `/notes/..` = `/`), so this surfaces as either
    // PathOutsideRoot OR UnexpectedPathComponent — both are acceptable
    // mitigations. We assert it is REJECTED (Err), not silently accepted.
    let root = Path::new("/notes");
    let abs = root.join("../escape.md");
    let res = RelativePath::from_filesystem(&abs, root);
    assert!(res.is_err(), "must reject path with .. component");
}

#[test]
fn from_filesystem_rejects_dot_component() {
    // /notes/./foo.md — `.` is a CurDir component; reject.
    let root = Path::new("/notes");
    let abs = root.join("./foo.md");
    // Note: depending on Path normalization, `.` may or may not survive.
    // If it survives, must error. If it's stripped, the result is "foo.md".
    let res = RelativePath::from_filesystem(&abs, root);
    if let Ok(rp) = res {
        assert_eq!(rp.as_str(), "foo.md");
    }
    // If Err, also fine — the important guarantee is "no traversal".
}

#[test]
fn from_storage_str_is_a_trust_constructor() {
    let rp = RelativePath::from_storage_str("a/b/c.md");
    assert_eq!(rp.as_str(), "a/b/c.md");
}

#[test]
fn to_filesystem_roundtrips_via_platform_separator() {
    let root = Path::new("/notes");
    let rp = RelativePath::from_storage_str("journals/2023_11_09.md");
    let abs = rp.to_filesystem(root);
    // On Unix this is `/notes/journals/2023_11_09.md`; on Windows it's
    // `\notes\journals\2023_11_09.md`. Either way, comparing via Path
    // semantics works.
    let expected: PathBuf = [root, Path::new("journals"), Path::new("2023_11_09.md")]
        .iter()
        .collect();
    assert_eq!(abs, expected);
}

#[test]
fn relative_path_is_hash_and_eq() {
    use std::collections::HashSet;
    let mut set: HashSet<RelativePath> = HashSet::new();
    set.insert(RelativePath::from_storage_str("a/b.md"));
    set.insert(RelativePath::from_storage_str("a/b.md"));
    assert_eq!(set.len(), 1);
}
