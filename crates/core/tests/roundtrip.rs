// ACPT-01 — round-trip byte-identical gate. Ships RED in plan 01-01;
// flipped GREEN in plan 01-02. MUST stay green for the life of the project.
//
// Strategy: for every `.md` file in the synthetic Logseq corpus, read raw
// bytes, run them through `segment()`, then reconstruct by concatenating
// `bytes[b.byte_offset .. b.byte_offset + b.byte_length]` for each block in
// order. The result must be byte-identical to the source (PRS-07).
//
// Two corpora:
//   1. Synthetic (always present, committed to repo) — under
//      `crates/core/tests/fixtures/logseq-synthetic/`. Covers every PRD §6.6
//      pattern in isolation. Target of the CI gate.
//   2. Real (opt-in, never committed — git-ignored) — under
//      `data-folder-sample/Logseq/` at the repo root. If the folder exists
//      locally, this test also walks it and surfaces any drift. In CI the
//      folder is absent and this leg is silently skipped.
//
// In plan 01-01 `segment()` is a stub returning `vec![]`, so the rebuilt
// buffer is empty and this test fails on every non-empty file. Plan 01-02
// implements the state machine and flips the test green.

use std::fs;
use std::path::{Path, PathBuf};

use foliom_core::parser::segment::segment;

/// Expected synthetic-corpus size — count of `.md` fixtures under
/// `tests/fixtures/logseq-synthetic/`, excluding `README.md`. If you add or
/// remove a fixture, update this constant.
// Phase 2 plan 02-02 added `pages/Avaliação.md` for Pitfall 6 verification.
const EXPECTED_SYNTHETIC_COUNT: usize = 11;

#[test]
fn roundtrip_byte_identical_for_synthetic_corpus() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let synthetic_root = Path::new(manifest_dir).join("tests/fixtures/logseq-synthetic");

    let (failures, count) = walk_and_check(&synthetic_root, &["README.md"]);

    assert!(
        failures.is_empty(),
        "Synthetic-corpus round-trip drift in {} file(s):\n\n{}",
        failures.len(),
        render_failures(&failures),
    );

    assert_eq!(
        count, EXPECTED_SYNTHETIC_COUNT,
        "Expected {} .md fixtures in synthetic corpus; found {}. \
         Add/remove a file and update EXPECTED_SYNTHETIC_COUNT if intentional.",
        EXPECTED_SYNTHETIC_COUNT, count
    );
}

/// Opt-in second leg: walk the real Logseq base if it exists locally. Skipped
/// silently in CI (where `data-folder-sample/` is git-ignored and absent).
/// Lets the maintainer dogfood against their own ~600 files without leaking
/// PII into the repo.
#[test]
fn roundtrip_byte_identical_for_real_corpus_if_present() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let real_root = Path::new(manifest_dir).join("../../data-folder-sample/Logseq");

    if !real_root.is_dir() {
        eprintln!(
            "skipping real-corpus round-trip — {} not present (this is normal in CI)",
            real_root.display()
        );
        return;
    }

    let (failures, count) = walk_and_check_with_skip(&real_root, |path| {
        // Skip the `logseq/` config dir per IDX-01 ignore list.
        path.components().any(|c| c.as_os_str() == "logseq")
    });

    assert!(
        failures.is_empty(),
        "Real-corpus round-trip drift in {} file(s):\n\n{}",
        failures.len(),
        render_failures(&failures),
    );

    // No EXPECTED_COUNT guard for the real corpus — its size is the user's,
    // not the project's. Just emit it for visibility.
    eprintln!("real-corpus round-trip OK — {} files checked", count);
}

fn walk_and_check(root: &Path, skip_names: &[&str]) -> (Vec<(PathBuf, String)>, usize) {
    walk_and_check_with_skip(root, |p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| skip_names.contains(&n))
            .unwrap_or(false)
    })
}

fn walk_and_check_with_skip(
    root: &Path,
    should_skip: impl Fn(&Path) -> bool,
) -> (Vec<(PathBuf, String)>, usize) {
    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    let mut count: usize = 0;

    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        if should_skip(path) {
            continue;
        }

        count += 1;
        let bytes = fs::read(path).expect("read corpus file");
        let blocks = segment(&bytes);

        let mut rebuilt = Vec::with_capacity(bytes.len());
        for b in &blocks {
            rebuilt.extend_from_slice(&bytes[b.byte_offset..b.byte_offset + b.byte_length]);
        }

        if rebuilt != bytes {
            failures.push((path.to_path_buf(), first_diff_report(&bytes, &rebuilt)));
            if failures.len() >= 3 {
                break;
            }
        }
    }

    (failures, count)
}

fn render_failures(failures: &[(PathBuf, String)]) -> String {
    failures
        .iter()
        .map(|(p, d)| format!("=== {} ===\n{}", p.display(), d))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Produce a human-readable diff showing the first byte where the two
/// buffers diverge, with TABs and CRs made visible so CRLF leaks (Windows
/// autocrlf) and indentation issues are obvious in CI logs.
fn first_diff_report(want: &[u8], got: &[u8]) -> String {
    let min_len = want.len().min(got.len());
    let mut diff_at = min_len;
    for i in 0..min_len {
        if want[i] != got[i] {
            diff_at = i;
            break;
        }
    }
    let line_start = want[..diff_at]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let line_end_want = want[diff_at..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p + diff_at)
        .unwrap_or(want.len());
    let line_end_got = got[diff_at..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p + diff_at)
        .unwrap_or(got.len());

    format!(
        "first diff at byte {} (line {})\n  want {} bytes, got {} bytes\n  want: {:?}\n  got:  {:?}\n  (TAB shown as \\t, CR as \\r)",
        diff_at,
        want[..diff_at].iter().filter(|&&b| b == b'\n').count() + 1,
        want.len(),
        got.len(),
        visible(&want[line_start..line_end_want]),
        visible(&got[line_start.min(got.len())..line_end_got]),
    )
}

fn visible(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}
