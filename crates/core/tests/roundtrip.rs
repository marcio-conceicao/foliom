// ACPT-01 — round-trip byte-identical gate. Ships RED in plan 01-01;
// flipped GREEN in plan 01-02. MUST stay green for the life of the project.
//
// Strategy: for every `.md` file in `data-folder-sample/Logseq/` (excluding
// the `logseq/` config dir per IDX-01), read raw bytes, run them through
// `segment()`, then reconstruct by concatenating `bytes[b.byte_offset ..
// b.byte_offset + b.byte_length]` for each block in order. The result must
// be byte-identical to the source (PRS-07).
//
// In plan 01-01 `segment()` is a stub returning `vec![]`, so the rebuilt
// buffer is empty and this test fails on every non-empty file. Plan 01-02
// implements the state machine and flips the test green.

use std::fs;
use std::path::{Path, PathBuf};

use foliom_core::parser::segment::segment;

/// Expected corpus size — assumption A1 from RESEARCH §Assumptions. If the
/// corpus drifts (files added/removed), update this constant.
const EXPECTED_CORPUS_COUNT: usize = 620;

#[test]
fn roundtrip_byte_identical_for_entire_corpus() {
    // Resolve the corpus path relative to CARGO_MANIFEST_DIR per AP-4 — the
    // test must work regardless of where `cargo test` is invoked from.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_root = Path::new(manifest_dir).join("../../data-folder-sample/Logseq");

    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    let mut count: usize = 0;

    for entry in walkdir::WalkDir::new(&corpus_root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        // Skip the `logseq/` config dir per IDX-01 ignore list (config.edn is
        // relevant for Phase 5, but not for the round-trip gate).
        if path.components().any(|c| c.as_os_str() == "logseq") {
            continue;
        }

        count += 1;
        let bytes = fs::read(path).expect("read corpus file");
        let blocks = segment(&bytes);

        // Splice-noop reconstruction.
        let mut rebuilt = Vec::with_capacity(bytes.len());
        for b in &blocks {
            rebuilt.extend_from_slice(&bytes[b.byte_offset..b.byte_offset + b.byte_length]);
        }

        if rebuilt != bytes {
            failures.push((path.to_path_buf(), first_diff_report(&bytes, &rebuilt)));
            if failures.len() >= 3 {
                // Bound output; fix one, rerun.
                break;
            }
        }
    }

    // Surface content drift first — that produces a human-readable "want N
    // bytes / got 0 bytes" diff (the desired RED message in plan 01-01 and
    // the actionable failure thereafter). The corpus-size assert is a
    // separate guard against silent corpus shrinkage.
    assert!(
        failures.is_empty(),
        "Round-trip drift in {} file(s):\n\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(p, d)| format!("=== {} ===\n{}", p.display(), d))
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    assert_eq!(
        count, EXPECTED_CORPUS_COUNT,
        "Expected {} .md files in corpus; found {}. \
         Update EXPECTED_CORPUS_COUNT if corpus changed intentionally.",
        EXPECTED_CORPUS_COUNT, count
    );
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
