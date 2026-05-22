//! Tests for `mutation::tree_ops` — invertible tree operations over an
//! in-memory `MutableTree`. Pure-logic only (no IO).
//!
//! See plan `.planning/phases/03-outliner-editor/03-02-PLAN.md` Task 2.

use crate::mutation::tree_ops::{
    BlockSnapshot, MutableBlock, MutableTree, TreeOp, TreeOpError,
};

// -- Helpers ----------------------------------------------------------------

/// Compare two trees ignoring Vec-storage order (since `apply` may `push` new
/// blocks at the end). We sort both block lists by id and compare.
fn assert_trees_equal(actual: &MutableTree, expected: &MutableTree) {
    let mut a = actual.blocks.clone();
    let mut e = expected.blocks.clone();
    a.sort_by_key(|b| b.id);
    e.sort_by_key(|b| b.id);
    assert_eq!(a, e, "trees diverge (sorted-by-id)");
}

// -- Tree builders -----------------------------------------------------------

/// Build a 5-block tree:
///
/// ```text
/// (prelude id=0, depth=255)
/// (A id=1, depth=0, parent=None, ord=0)
///   (B id=2, depth=1, parent=Some(1), ord=0)
/// (C id=3, depth=0, parent=None, ord=1)
/// (D id=4, depth=0, parent=None, ord=2)
/// ```
fn small_tree() -> MutableTree {
    MutableTree::new(vec![
        MutableBlock {
            id: 0,
            parent_id: None,
            ord: -1,
            depth: u8::MAX,
            raw: String::new(),
        },
        MutableBlock {
            id: 1,
            parent_id: None,
            ord: 0,
            depth: 0,
            raw: "- A\n".into(),
        },
        MutableBlock {
            id: 2,
            parent_id: Some(1),
            ord: 0,
            depth: 1,
            raw: "\t- B\n".into(),
        },
        MutableBlock {
            id: 3,
            parent_id: None,
            ord: 1,
            depth: 0,
            raw: "- C\n".into(),
        },
        MutableBlock {
            id: 4,
            parent_id: None,
            ord: 2,
            depth: 0,
            raw: "- D\n".into(),
        },
    ])
}

// -- Indent / Outdent --------------------------------------------------------

#[test]
fn indent_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Indent C (id=3) — becomes child of A (id=1, its predecessor at depth 0).
    let op = TreeOp::Indent {
        block_id: 3,
        prev_depth: 0,
    };
    let inv = op.apply(&mut t).expect("indent");
    // After indent: C's parent should be A, depth=1, ord = max child ord of A + 1 = 1
    let c = t.get(3).unwrap();
    assert_eq!(c.parent_id, Some(1));
    assert_eq!(c.depth, 1);
    assert_eq!(c.ord, 1);

    // Invert.
    inv.apply(&mut t).expect("outdent (inverse)");
    assert_trees_equal(&t, &orig);
}

#[test]
fn indent_first_child_returns_error() {
    let mut t = small_tree();
    // B (id=2) is the first child under A — has no preceding depth-1 sibling.
    // Trying to indent further must fail with FirstChildCannotIndent.
    let err = TreeOp::Indent {
        block_id: 2,
        prev_depth: 1,
    }
    .apply(&mut t)
    .unwrap_err();
    assert_eq!(err, TreeOpError::FirstChildCannotIndent);
}

#[test]
fn indent_first_top_level_block_returns_error() {
    let mut t = small_tree();
    // A (id=1) is the first top-level bullet — no preceding sibling.
    let err = TreeOp::Indent {
        block_id: 1,
        prev_depth: 0,
    }
    .apply(&mut t)
    .unwrap_err();
    assert_eq!(err, TreeOpError::FirstChildCannotIndent);
}

#[test]
fn outdent_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Outdent B (id=2) — becomes sibling of A at the root, just after A.
    let op = TreeOp::Outdent {
        block_id: 2,
        prev_depth: 1,
    };
    let inv = op.apply(&mut t).expect("outdent");
    let b = t.get(2).unwrap();
    assert_eq!(b.parent_id, None);
    assert_eq!(b.depth, 0);
    // B should be inserted just after A (ord=1), C/D bumped to 2/3.
    assert_eq!(b.ord, 1);
    assert_eq!(t.get(3).unwrap().ord, 2);
    assert_eq!(t.get(4).unwrap().ord, 3);

    inv.apply(&mut t).expect("indent (inverse)");
    assert_trees_equal(&t, &orig);
}

#[test]
fn outdent_at_root_returns_error() {
    let mut t = small_tree();
    let err = TreeOp::Outdent {
        block_id: 1,
        prev_depth: 0,
    }
    .apply(&mut t)
    .unwrap_err();
    assert_eq!(err, TreeOpError::AlreadyAtRoot);
}

// -- Merge / Split -----------------------------------------------------------

#[test]
fn merge_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Merge C (id=3) into A (id=1). C's raw appended to A's raw with '\n' separator.
    let op = TreeOp::Merge {
        block_id: 3,
        merged_into_id: 1,
        original_raw: String::new(), // filled by apply
    };
    let inv = op.apply(&mut t).expect("merge");

    // C must be gone.
    assert!(t.get(3).is_none());
    // A's raw must contain both raws.
    let a = t.get(1).unwrap();
    assert!(a.raw.contains("- A"));
    assert!(a.raw.contains("- C"));

    inv.apply(&mut t).expect("split (inverse)");
    assert_trees_equal(&t, &orig);
}

#[test]
fn merge_into_prelude_returns_error() {
    let mut t = small_tree();
    // Try to merge A into the prelude (id=0, depth=255).
    let err = TreeOp::Merge {
        block_id: 1,
        merged_into_id: 0,
        original_raw: String::new(),
    }
    .apply(&mut t)
    .unwrap_err();
    assert_eq!(err, TreeOpError::CannotMergeIntoPrelude);
}

#[test]
fn split_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Split A's raw "- A\n" at offset 2 (between "- " and "A\n").
    let op = TreeOp::Split {
        block_id: 1,
        at_offset: 2,
        new_block_id: 99,
    };
    let inv = op.apply(&mut t).expect("split");

    let a = t.get(1).unwrap();
    let new_block = t.get(99).unwrap();
    assert_eq!(a.raw, "- ");
    assert_eq!(new_block.raw, "A\n");
    // New block at same depth/parent as A, inserted after A.
    assert_eq!(new_block.parent_id, None);
    assert_eq!(new_block.depth, 0);

    inv.apply(&mut t).expect("merge (inverse)");
    assert_trees_equal(&t, &orig);
}

#[test]
fn split_offset_out_of_bounds_returns_error() {
    let mut t = small_tree();
    let err = TreeOp::Split {
        block_id: 1,
        at_offset: 9999,
        new_block_id: 100,
    }
    .apply(&mut t)
    .unwrap_err();
    assert_eq!(err, TreeOpError::InvalidSplitOffset);
}

// -- Move --------------------------------------------------------------------

#[test]
fn move_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Move D (id=4) to become a child of A (id=1) at ord=1 (after B).
    let op = TreeOp::Move {
        block_id: 4,
        prev_parent_id: None,
        prev_ord: 2,
        new_parent_id: Some(1),
        new_ord: 1,
    };
    let inv = op.apply(&mut t).expect("move");

    let d = t.get(4).unwrap();
    assert_eq!(d.parent_id, Some(1));
    assert_eq!(d.ord, 1);

    inv.apply(&mut t).expect("move (inverse)");
    assert_trees_equal(&t, &orig);
}

// -- Delete ------------------------------------------------------------------

#[test]
fn delete_then_inverse_restores_tree() {
    let orig = small_tree();
    let mut t = orig.clone();
    let op = TreeOp::Delete {
        block_id: 3,
        snapshot: BlockSnapshot {
            raw: String::new(),
            depth: 0,
            parent_id: None,
            ord: 0,
            reparented_children: Vec::new(),
        },
    };
    let inv = op.apply(&mut t).expect("delete");

    assert!(t.get(3).is_none());
    // D's ord should have shifted down to 1 (was 2).
    assert_eq!(t.get(4).unwrap().ord, 1);

    inv.apply(&mut t).expect("undelete (inverse)");
    assert_trees_equal(&t, &orig);
}

#[test]
fn delete_block_with_children_reparents_to_grandparent() {
    let orig = small_tree();
    let mut t = orig.clone();
    // Delete A (id=1). B (its child) must be re-parented to None (root),
    // taking A's ord position (ord=0). C/D bump up by 1 + (children-1) effect:
    // here only B is re-parented (1 child), so C/D land at ord 1/2 unchanged.
    let op = TreeOp::Delete {
        block_id: 1,
        snapshot: BlockSnapshot {
            raw: String::new(),
            depth: 0,
            parent_id: None,
            ord: 0,
            reparented_children: Vec::new(),
        },
    };
    let inv = op.apply(&mut t).expect("delete A");

    assert!(t.get(1).is_none());
    let b = t.get(2).unwrap();
    assert_eq!(b.parent_id, None);
    // B inherits A's ord (0); existing C/D at 1/2 are preserved.
    assert_eq!(b.ord, 0);
    // depth lowered by 1 (the difference between old parent A's depth 0 and B's depth 1).
    assert_eq!(b.depth, 0);

    inv.apply(&mut t).expect("undelete A (inverse)");
    assert_trees_equal(&t, &orig);
}

// -- Serde round-trip --------------------------------------------------------

#[test]
fn serde_round_trip_every_variant() {
    let ops = vec![
        TreeOp::Indent {
            block_id: 7,
            prev_depth: 1,
        },
        TreeOp::Outdent {
            block_id: 7,
            prev_depth: 2,
        },
        TreeOp::Merge {
            block_id: 8,
            merged_into_id: 9,
            original_raw: "- old\n".into(),
        },
        TreeOp::Split {
            block_id: 10,
            at_offset: 4,
            new_block_id: 11,
        },
        TreeOp::Move {
            block_id: 12,
            prev_parent_id: Some(1),
            prev_ord: 0,
            new_parent_id: None,
            new_ord: 3,
        },
        TreeOp::Delete {
            block_id: 13,
            snapshot: BlockSnapshot {
                raw: "- x\n".into(),
                reparented_children: Vec::new(),
                depth: 0,
                parent_id: None,
                ord: 5,
            },
        },
    ];
    for op in ops {
        let s = serde_json::to_string(&op).unwrap();
        let parsed: TreeOp = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, op);
    }
}

// -- MutableTree helpers -----------------------------------------------------

#[test]
fn children_of_sorts_by_ord() {
    let t = MutableTree::new(vec![
        MutableBlock {
            id: 1,
            parent_id: None,
            ord: 2,
            depth: 0,
            raw: "- c\n".into(),
        },
        MutableBlock {
            id: 2,
            parent_id: None,
            ord: 0,
            depth: 0,
            raw: "- a\n".into(),
        },
        MutableBlock {
            id: 3,
            parent_id: None,
            ord: 1,
            depth: 0,
            raw: "- b\n".into(),
        },
    ]);
    let kids = t.children_of(None);
    let ids: Vec<i64> = kids.iter().map(|b| b.id).collect();
    assert_eq!(ids, vec![2, 3, 1]);
}

#[test]
fn predecessor_at_same_depth_returns_none_for_first_child() {
    let t = small_tree();
    assert_eq!(t.predecessor_at_same_depth(2), None); // B is the first (and only) child of A
    assert_eq!(t.predecessor_at_same_depth(1), None); // A is the first top-level block
    assert_eq!(t.predecessor_at_same_depth(3), Some(1)); // C's predecessor is A
    assert_eq!(t.predecessor_at_same_depth(4), Some(3)); // D's predecessor is C
}
