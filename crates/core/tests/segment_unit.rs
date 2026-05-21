// Hand-crafted unit tests for the Stage 1 segmenter state machine
// (plan 01-02, Task 2). Each test asserts the splice-noop invariant
// (concat of byte ranges == source) plus the structural shape (block
// count, depths, drawer/property counts). Together with the corpus-level
// round-trip test in `tests/roundtrip.rs`, this is the unit-test safety
// net beneath the segmenter.

use foliom_core::parser::segment::{RawBlock, segment};

/// Shared invariant assertion: byte ranges of `blocks` are contiguous,
/// non-overlapping, and reconstruct `source` exactly.
fn assert_splice_noop(source: &[u8], blocks: &[RawBlock]) {
    let rebuilt: Vec<u8> = blocks
        .iter()
        .flat_map(|b| source[b.byte_offset..b.byte_offset + b.byte_length].iter().copied())
        .collect();
    assert_eq!(
        rebuilt, source,
        "splice-noop violated: rebuilt buffer differs from source"
    );
    // Also assert contiguity explicitly.
    let mut cursor = 0usize;
    for b in blocks {
        assert_eq!(b.byte_offset, cursor, "block byte_offset not contiguous");
        cursor = b.byte_offset + b.byte_length;
    }
    assert_eq!(cursor, source.len(), "blocks do not cover full source");
}

const PRELUDE_DEPTH: u8 = u8::MAX;

mod prelude {
    use super::*;

    #[test]
    fn empty_input_produces_concat_equal_source() {
        let src = b"";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        // We always emit at least the prelude (length 0).
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].depth, PRELUDE_DEPTH);
        assert_eq!(blocks[0].byte_length, 0);
    }

    #[test]
    fn prelude_only_no_bullets() {
        let src = b"just text\nno bullets\n";
        assert_eq!(src.len(), 21);
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].depth, PRELUDE_DEPTH);
        assert_eq!(blocks[0].byte_length, 21);
    }
}

mod bullets {
    use super::*;

    #[test]
    fn single_top_level_bullet() {
        let src = b"- foo\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].depth, PRELUDE_DEPTH);
        assert_eq!(blocks[0].byte_length, 0);
        assert_eq!(blocks[1].depth, 0);
        assert_eq!(blocks[1].byte_length, 6);
        assert_eq!(blocks[1].raw, "- foo\n");
    }

    #[test]
    fn nested_bullets_three_levels() {
        let src = b"- a\n\t- b\n\t\t- c\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        // prelude + 3 bullets
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[1].depth, 0);
        assert_eq!(blocks[2].depth, 1);
        assert_eq!(blocks[3].depth, 2);
    }

    #[test]
    fn empty_bullet_dash_only() {
        // Logseq emits `-\n` for an empty bullet (fixture 08).
        let src = b"-\n- next\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].depth, 0);
        assert_eq!(blocks[1].byte_length, 2);
        assert_eq!(blocks[2].depth, 0);
    }
}

mod continuation {
    use super::*;

    #[test]
    fn two_space_continuation_joined() {
        let src = b"- header\n  continued line\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].depth, 0);
        // entire input belongs to the one bullet
        assert_eq!(blocks[1].byte_length, src.len());
    }

    #[test]
    fn blank_line_inside_block() {
        // Bullet, continuation, blank, continuation — all one block.
        let src = b"- header\n  line a\n\n  line b\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].byte_length, src.len());
    }

    #[test]
    fn two_bullets_continuation_only_on_first() {
        let src = b"- foo\n  cont\n- bar\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 3);
        // First bullet covers "- foo\n  cont\n" (13 bytes).
        assert_eq!(blocks[1].byte_length, 13);
        // Second bullet covers "- bar\n" (6 bytes).
        assert_eq!(blocks[2].byte_length, 6);
    }
}

mod fences {
    use super::*;

    #[test]
    fn code_fence_inside_bullet_swallows_dashes() {
        // Bullet opens with ``` then has `- not a bullet` inside; the
        // segmenter must NOT split there.
        let src = b"- ```rust\n  - not a bullet\n  ```\n- real bullet\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        // prelude + first-bullet-with-fence + second real bullet
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].depth, 0);
        // The fence-hosting bullet covers everything through `  ```\n`.
        assert_eq!(blocks[1].byte_length, b"- ```rust\n  - not a bullet\n  ```\n".len());
        assert_eq!(blocks[2].depth, 0);
        assert_eq!(blocks[2].raw, "- real bullet\n");
    }

    #[test]
    fn nested_bullet_opens_fence_then_sibling() {
        // From fixture 02: nested bullet hosts the fence, then a sibling.
        let src = b"- a\n\t- ```rust\n\t  body\n\t  ```\n\t- sibling\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        // prelude + bullet a (depth 0) + fence-host (depth 1) + sibling (depth 1)
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[2].depth, 1);
        assert_eq!(blocks[3].depth, 1);
        assert_eq!(blocks[3].raw, "\t- sibling\n");
    }
}

mod drawers {
    use super::*;

    #[test]
    fn logbook_drawer_attached_to_parent() {
        let src = b"- task\n  :LOGBOOK:\n  CLOCK: stuff\n  :END:\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        let bullet = &blocks[1];
        assert_eq!(bullet.drawers.len(), 1);
        let d = &bullet.drawers[0];
        assert_eq!(d.name, "LOGBOOK");
        // Drawer bytes must be fully inside the bullet's range.
        assert!(d.byte_offset >= bullet.byte_offset);
        assert!(d.byte_offset + d.byte_length <= bullet.byte_offset + bullet.byte_length);
        // Drawer covers `:LOGBOOK:\n  CLOCK: stuff\n  :END:\n` — note the
        // drawer byte_offset is the start of the `:LOGBOOK:` line, which
        // starts with the 2-space prefix in the source.
        let drawer_slice = &src[d.byte_offset..d.byte_offset + d.byte_length];
        assert!(drawer_slice.ends_with(b":END:\n"));
    }

    #[test]
    fn two_drawers_same_block() {
        let src = b"- task\n  :LOGBOOK:\n  CLOCK: a\n  :END:\n  :LOGBOOK:\n  CLOCK: b\n  :END:\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].drawers.len(), 2);
    }
}

mod properties {
    use super::*;

    #[test]
    fn block_property_in_continuation() {
        let src = b"- header\n  alias:: Old Name\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks.len(), 2);
        let b = &blocks[1];
        assert_eq!(b.properties, vec![("alias".to_string(), "Old Name".to_string())]);
        // Verbatim in raw.
        assert!(b.raw.contains("alias:: Old Name"));
    }

    #[test]
    fn multiple_properties_collected_in_order() {
        let src = b"- header\n  collapsed:: true\n  id:: abc-123\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(blocks[1].properties.len(), 2);
        assert_eq!(blocks[1].properties[0].0, "collapsed");
        assert_eq!(blocks[1].properties[0].1, "true");
        assert_eq!(blocks[1].properties[1].0, "id");
        assert_eq!(blocks[1].properties[1].1, "abc-123");
    }

    #[test]
    fn property_with_dotted_key() {
        // logseq.order-list-type is a real Logseq property name.
        let src = b"- header\n  logseq.order-list-type:: number\n";
        let blocks = segment(src);
        assert_splice_noop(src, &blocks);
        assert_eq!(
            blocks[1].properties,
            vec![("logseq.order-list-type".to_string(), "number".to_string())]
        );
    }
}
