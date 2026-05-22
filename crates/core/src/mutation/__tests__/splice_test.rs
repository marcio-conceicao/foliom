//! Tests for `mutation::splice` — pure byte-splice + offset-shift math.
//!
//! See plan `.planning/phases/03-outliner-editor/03-02-PLAN.md` Task 1.

use crate::mutation::splice::{compute_shifted_offsets, splice_block, BlockOffset};
use crate::parser::segment::segment;

// -- Literal byte-substitution tests -----------------------------------------

#[test]
fn splice_block_equal_length_substitution() {
    let original = b"abc\n- old\n- z\n";
    // Replace bytes [4, 10) = "- old\n" (6 bytes) with "- NEW\n" (6 bytes).
    let out = splice_block(original, 4, 6, b"- NEW\n");
    assert_eq!(out, b"abc\n- NEW\n- z\n");
}

#[test]
fn splice_block_shrinking_replacement_shifts_left() {
    // "abc\n- old block\n- z\n"  — block "- old block\n" = 12 bytes at offset 4.
    let original = b"abc\n- old block\n- z\n";
    let out = splice_block(original, 4, 12, b"- new\n");
    assert_eq!(out, b"abc\n- new\n- z\n");
}

#[test]
fn splice_block_growing_replacement_shifts_right() {
    let original = b"abc\n- x\n- z\n";
    // Replace "- x\n" (4 bytes) at offset 4 with "- expanded text\n" (16 bytes).
    let out = splice_block(original, 4, 4, b"- expanded text\n");
    assert_eq!(out, b"abc\n- expanded text\n- z\n");
}

#[test]
fn splice_block_empty_replacement_deletes_range() {
    let original = b"abc\n- gone\n- z\n";
    let out = splice_block(original, 4, 7, b"");
    assert_eq!(out, b"abc\n- z\n");
}

#[test]
fn splice_block_at_start() {
    let original = b"abc\nrest\n";
    let out = splice_block(original, 0, 4, b"XYZ\n");
    assert_eq!(out, b"XYZ\nrest\n");
}

#[test]
fn splice_block_at_end() {
    let original = b"abc\ndef\n";
    let out = splice_block(original, 4, 4, b"X\n");
    assert_eq!(out, b"abc\nX\n");
}

// -- Round-trip noop over every synthetic fixture ----------------------------

fn fixtures_root() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/logseq-synthetic");
    p
}

fn synthetic_md_files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(fixtures_root())
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file()
            && entry.path().extension().is_some_and(|e| e == "md")
            // README.md is documentation, not a fixture.
            && entry.path().file_name().is_some_and(|n| n != "README.md")
        {
            out.push(entry.into_path());
        }
    }
    out.sort();
    out
}

#[test]
fn round_trip_noop_splice_byte_identical_for_every_fixture() {
    let files = synthetic_md_files();
    assert!(
        !files.is_empty(),
        "no synthetic fixtures found under {:?}",
        fixtures_root()
    );

    let mut covered = 0usize;
    for path in &files {
        let bytes = std::fs::read(path).expect("read fixture");
        let blocks = segment(&bytes);

        // Splice EVERY non-prelude block in place with its own raw bytes —
        // result must be byte-identical to input.
        for block in blocks.iter().filter(|b| b.depth != u8::MAX) {
            let out = splice_block(
                &bytes,
                block.byte_offset,
                block.byte_length,
                block.raw.as_bytes(),
            );
            assert_eq!(
                out, bytes,
                "no-op splice diverged for {:?} block @ {}+{}",
                path, block.byte_offset, block.byte_length
            );
            covered += 1;
        }
    }
    assert!(covered > 0, "no non-prelude blocks exercised");
}

// -- compute_shifted_offsets math --------------------------------------------

// Tuple impl is provided by the production module so tests can stay compact.

fn fixed_5_blocks() -> Vec<(i64, usize, usize)> {
    vec![
        (1, 0, 10),
        (2, 10, 10),
        (3, 20, 10),
        (4, 30, 10),
        (5, 40, 10),
    ]
}

#[test]
fn shifted_offsets_grow_middle_block_pushes_downstream_right() {
    let blocks = fixed_5_blocks();
    let out = compute_shifted_offsets(&blocks, 3, 10, 15);
    assert_eq!(
        out,
        vec![
            (1, 0, 10),
            (2, 10, 10),
            (3, 20, 15),
            (4, 35, 10),
            (5, 45, 10),
        ]
    );
}

#[test]
fn shifted_offsets_shrink_middle_block_pulls_downstream_left() {
    let blocks = fixed_5_blocks();
    let out = compute_shifted_offsets(&blocks, 3, 10, 5);
    assert_eq!(
        out,
        vec![
            (1, 0, 10),
            (2, 10, 10),
            (3, 20, 5),
            (4, 25, 10),
            (5, 35, 10),
        ]
    );
}

#[test]
fn shifted_offsets_changed_block_is_last_no_downstream_shift() {
    let blocks = fixed_5_blocks();
    let out = compute_shifted_offsets(&blocks, 5, 10, 20);
    assert_eq!(
        out,
        vec![
            (1, 0, 10),
            (2, 10, 10),
            (3, 20, 10),
            (4, 30, 10),
            (5, 40, 20),
        ]
    );
}

#[test]
fn shifted_offsets_no_change_when_lengths_equal() {
    let blocks = fixed_5_blocks();
    let out = compute_shifted_offsets(&blocks, 3, 10, 10);
    assert_eq!(out, blocks);
}

#[test]
fn shifted_offsets_changed_block_is_prelude_shifts_all_real_blocks() {
    // Prelude (id=0, offset=0, len=5) followed by four real blocks.
    let blocks: Vec<(i64, usize, usize)> = vec![
        (0, 0, 5),
        (1, 5, 10),
        (2, 15, 10),
        (3, 25, 10),
        (4, 35, 10),
    ];
    let out = compute_shifted_offsets(&blocks, 0, 5, 12);
    assert_eq!(
        out,
        vec![
            (0, 0, 12),
            (1, 12, 10),
            (2, 22, 10),
            (3, 32, 10),
            (4, 42, 10),
        ]
    );
}

#[test]
fn raw_block_impl_block_offset_via_blanket_trait() {
    // RawBlock should satisfy BlockOffset. We need an `id()` source — for now
    // tests of RawBlock-as-BlockOffset just exercise byte_offset/byte_length;
    // id() is supplied by storage rows (i64 PK) in plan 03-03.
    let bytes = b"- a\n- b\n";
    let blocks = crate::parser::segment::segment(bytes);
    let first_real = blocks.iter().find(|b| b.depth != u8::MAX).unwrap();
    // Use the trait via a generic function to prove the impl exists.
    fn check<B: BlockOffset>(b: &B) -> (usize, usize) {
        (b.byte_offset(), b.byte_length())
    }
    let (off, len) = check(first_real);
    assert_eq!(off, 0);
    assert_eq!(len, 4); // "- a\n"
}
