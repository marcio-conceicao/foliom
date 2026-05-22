//! Invertible tree operations over a `MutableTree`. Pure logic — no IO.
//!
//! Each variant of `TreeOp` corresponds to one of the keyboard-driven editor
//! ops from D-30-07/08 (Tab/Shift-Tab indent/outdent, Backspace-at-start merge,
//! Enter-in-middle split, drag/drop move, deletion). `apply()` returns the
//! inverse op so plan 03-04's `treeOpLog` undo stack works without server
//! state.
//!
//! See `.planning/phases/03-outliner-editor/03-RESEARCH.md` §7 for the
//! mutation API surface, and `03-02-PLAN.md` for behaviour specs.
//!
//! # Deviation from plan <interfaces> stanza
//!
//! `BlockSnapshot` carries a `reparented_children: Vec<i64>` field beyond
//! what the plan's <interfaces> block enumerates. This is required for
//! invertibility of `Delete` on a block with children — without recording
//! which blocks were re-parented and how, the inverse `apply` cannot
//! reconstruct the original tree shape. The field is `Default::default()` for
//! callers who don't need it (leaf delete) and the serde tag is auto-derived,
//! so the wire shape is a strict superset of what the plan documented.
//! Logged as Rule 2 (auto-add missing critical functionality) in the SUMMARY.

use serde::{Deserialize, Serialize};

/// Sentinel matching `crate::parser::segment::PRELUDE_DEPTH` — the page
/// prelude block is depth `u8::MAX` and may have zero byte length.
const PRELUDE_DEPTH: u8 = u8::MAX;

/// Everything needed to rebuild a deleted block. `TreeOp::Delete` carries a
/// snapshot in its payload, and the inverse `Delete` reuses the same struct
/// with the snapshot populated so `apply` can re-insert.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub raw: String,
    pub depth: u8,
    pub parent_id: Option<i64>,
    pub ord: i32,
    /// Child ids that were re-parented to `parent_id` on delete, in the order
    /// they were re-numbered. Empty for leaf deletes. Required for inverse.
    #[serde(default)]
    pub reparented_children: Vec<i64>,
}

/// All six block-tree operations. `apply()` returns the inverse op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeOp {
    /// Move `block_id` one depth deeper, making it a child of its preceding
    /// sibling at the same depth. Errors with `FirstChildCannotIndent` if no
    /// such predecessor exists.
    Indent { block_id: i64, prev_depth: u8 },
    /// Move `block_id` one depth shallower, making it a sibling of its parent
    /// (inserted just after parent in the grandparent's child list). Errors
    /// with `AlreadyAtRoot` if `block.depth == 0`.
    Outdent { block_id: i64, prev_depth: u8 },
    /// Concatenate `block_id`'s raw onto `merged_into_id`'s raw and remove
    /// `block_id`. `original_raw` is set by `apply` for inverse reconstruction.
    Merge {
        block_id: i64,
        merged_into_id: i64,
        original_raw: String,
    },
    /// Split `block_id` at byte offset `at_offset` in its raw — left half stays
    /// in `block_id`, right half goes into a new block with id `new_block_id`,
    /// inserted as the next sibling of `block_id`.
    Split {
        block_id: i64,
        at_offset: usize,
        new_block_id: i64,
    },
    /// Reposition `block_id` under a different parent/ord. Inverse swaps
    /// prev/new fields.
    Move {
        block_id: i64,
        prev_parent_id: Option<i64>,
        prev_ord: i32,
        new_parent_id: Option<i64>,
        new_ord: i32,
    },
    /// Delete `block_id`. Children are re-parented to the deleted block's
    /// parent at the deleted block's ord. `snapshot` is filled by `apply` so
    /// the inverse can re-insert.
    ///
    /// `apply` distinguishes "delete" from "re-insert" by checking whether
    /// the block exists in the tree: if it exists we delete it; otherwise we
    /// re-insert from the snapshot.
    Delete {
        block_id: i64,
        snapshot: BlockSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOpError {
    BlockNotFound(i64),
    FirstChildCannotIndent,
    AlreadyAtRoot,
    CannotMergeIntoPrelude,
    InvalidSplitOffset,
}

impl std::fmt::Display for TreeOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockNotFound(id) => write!(f, "block {} not found", id),
            Self::FirstChildCannotIndent => write!(f, "first child cannot be indented further"),
            Self::AlreadyAtRoot => write!(f, "block is already at root depth"),
            Self::CannotMergeIntoPrelude => write!(f, "cannot merge into page prelude"),
            Self::InvalidSplitOffset => write!(f, "split offset out of bounds"),
        }
    }
}

impl std::error::Error for TreeOpError {}

/// One block in the in-memory tree. Mirrors the SQL `blocks` row shape
/// (subset relevant to mutation: id, parent, ord, depth, raw).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableBlock {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub ord: i32,
    pub depth: u8,
    pub raw: String,
}

/// Flat container with parent/ord-based traversal. Plan 03-03 will adapt SQL
/// rows into/out of this representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableTree {
    pub blocks: Vec<MutableBlock>,
}

impl MutableTree {
    pub fn new(blocks: Vec<MutableBlock>) -> Self {
        Self { blocks }
    }

    pub fn get(&self, id: i64) -> Option<&MutableBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn get_mut(&mut self, id: i64) -> Option<&mut MutableBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// Children of `parent_id`, sorted by `ord`. Prelude (depth=u8::MAX) is
    /// excluded from sibling lists.
    pub fn children_of(&self, parent_id: Option<i64>) -> Vec<&MutableBlock> {
        let mut v: Vec<&MutableBlock> = self
            .blocks
            .iter()
            .filter(|b| b.parent_id == parent_id && b.depth != PRELUDE_DEPTH)
            .collect();
        v.sort_by_key(|b| b.ord);
        v
    }

    /// The block immediately preceding `id` among its siblings (same parent,
    /// strictly lower ord, max such). Used by `Indent` to find the new parent.
    pub fn predecessor_at_same_depth(&self, id: i64) -> Option<i64> {
        let me = self.get(id)?;
        self.blocks
            .iter()
            .filter(|b| {
                b.parent_id == me.parent_id && b.ord < me.ord && b.depth != PRELUDE_DEPTH
            })
            .max_by_key(|b| b.ord)
            .map(|b| b.id)
    }
}

impl TreeOp {
    /// Apply to the tree, returning the inverse op. See variant docs for
    /// per-op semantics. Errors are documented at the variant level.
    pub fn apply(self, tree: &mut MutableTree) -> Result<TreeOp, TreeOpError> {
        match self {
            TreeOp::Indent { block_id, .. } => apply_indent(tree, block_id),
            TreeOp::Outdent { block_id, .. } => apply_outdent(tree, block_id),
            TreeOp::Merge {
                block_id,
                merged_into_id,
                ..
            } => apply_merge(tree, block_id, merged_into_id),
            TreeOp::Split {
                block_id,
                at_offset,
                new_block_id,
            } => apply_split(tree, block_id, at_offset, new_block_id),
            TreeOp::Move {
                block_id,
                prev_parent_id,
                prev_ord,
                new_parent_id,
                new_ord,
            } => apply_move(
                tree,
                block_id,
                prev_parent_id,
                prev_ord,
                new_parent_id,
                new_ord,
            ),
            TreeOp::Delete { block_id, snapshot } => {
                apply_delete_or_restore(tree, block_id, snapshot)
            }
        }
    }
}

// -- Per-variant implementations --------------------------------------------

fn apply_indent(tree: &mut MutableTree, block_id: i64) -> Result<TreeOp, TreeOpError> {
    let predecessor_id = tree
        .predecessor_at_same_depth(block_id)
        .ok_or(TreeOpError::FirstChildCannotIndent)?;

    let prev_depth = tree
        .get(block_id)
        .ok_or(TreeOpError::BlockNotFound(block_id))?
        .depth;

    let new_ord = tree
        .children_of(Some(predecessor_id))
        .iter()
        .map(|b| b.ord)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    let (old_parent, old_ord) = {
        let b = tree.get(block_id).unwrap();
        (b.parent_id, b.ord)
    };

    {
        let b = tree.get_mut(block_id).unwrap();
        b.parent_id = Some(predecessor_id);
        b.depth = b.depth.saturating_add(1);
        b.ord = new_ord;
    }

    // Close gap under the old parent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == old_parent && sib.ord > old_ord {
            sib.ord -= 1;
        }
    }

    Ok(TreeOp::Outdent {
        block_id,
        prev_depth,
    })
}

fn apply_outdent(tree: &mut MutableTree, block_id: i64) -> Result<TreeOp, TreeOpError> {
    let (old_parent_id, prev_depth, old_ord) = {
        let b = tree
            .get(block_id)
            .ok_or(TreeOpError::BlockNotFound(block_id))?;
        if b.depth == 0 || b.parent_id.is_none() {
            return Err(TreeOpError::AlreadyAtRoot);
        }
        (b.parent_id.unwrap(), b.depth, b.ord)
    };

    let (grandparent_id, parent_ord) = {
        let parent = tree
            .get(old_parent_id)
            .ok_or(TreeOpError::BlockNotFound(old_parent_id))?;
        (parent.parent_id, parent.ord)
    };

    let new_ord = parent_ord + 1;

    // Make room under the grandparent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == grandparent_id && sib.ord >= new_ord {
            sib.ord += 1;
        }
    }
    // Close gap under the old parent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == Some(old_parent_id) && sib.ord > old_ord {
            sib.ord -= 1;
        }
    }
    // Relocate.
    let b = tree.get_mut(block_id).unwrap();
    b.parent_id = grandparent_id;
    b.depth -= 1;
    b.ord = new_ord;

    Ok(TreeOp::Indent {
        block_id,
        prev_depth,
    })
}

fn apply_merge(
    tree: &mut MutableTree,
    block_id: i64,
    merged_into_id: i64,
) -> Result<TreeOp, TreeOpError> {
    let target_depth = tree
        .get(merged_into_id)
        .ok_or(TreeOpError::BlockNotFound(merged_into_id))?
        .depth;
    if target_depth == PRELUDE_DEPTH {
        return Err(TreeOpError::CannotMergeIntoPrelude);
    }

    let (original_raw, merging_parent, merging_ord) = {
        let b = tree
            .get(block_id)
            .ok_or(TreeOpError::BlockNotFound(block_id))?;
        (b.raw.clone(), b.parent_id, b.ord)
    };

    let merged_into_raw_len_before = tree.get(merged_into_id).unwrap().raw.len();

    {
        let target = tree.get_mut(merged_into_id).unwrap();
        target.raw.push_str(&original_raw);
    }

    // Inverse Split offset == the length of the merge-target before append.
    let inv_split_offset = merged_into_raw_len_before;

    tree.blocks.retain(|b| b.id != block_id);

    // Close the gap left by the removed block among its former siblings.
    for sib in tree.blocks.iter_mut() {
        if sib.parent_id == merging_parent && sib.ord > merging_ord {
            sib.ord -= 1;
        }
    }

    Ok(TreeOp::Split {
        block_id: merged_into_id,
        at_offset: inv_split_offset,
        new_block_id: block_id,
    })
}

fn apply_split(
    tree: &mut MutableTree,
    block_id: i64,
    at_offset: usize,
    new_block_id: i64,
) -> Result<TreeOp, TreeOpError> {
    let (parent_id, ord, depth, left_raw, right_raw) = {
        let b = tree
            .get(block_id)
            .ok_or(TreeOpError::BlockNotFound(block_id))?;
        if at_offset > b.raw.len() {
            return Err(TreeOpError::InvalidSplitOffset);
        }
        if !b.raw.is_char_boundary(at_offset) {
            return Err(TreeOpError::InvalidSplitOffset);
        }
        let (l, r) = b.raw.split_at(at_offset);
        (b.parent_id, b.ord, b.depth, l.to_string(), r.to_string())
    };

    let new_ord = ord + 1;
    for sib in tree.blocks.iter_mut() {
        if sib.parent_id == parent_id && sib.ord >= new_ord && sib.id != block_id {
            sib.ord += 1;
        }
    }

    {
        let b = tree.get_mut(block_id).unwrap();
        b.raw = left_raw;
    }

    tree.blocks.push(MutableBlock {
        id: new_block_id,
        parent_id,
        ord: new_ord,
        depth,
        raw: right_raw.clone(),
    });

    Ok(TreeOp::Merge {
        block_id: new_block_id,
        merged_into_id: block_id,
        original_raw: right_raw,
    })
}

fn apply_move(
    tree: &mut MutableTree,
    block_id: i64,
    _prev_parent_id: Option<i64>,
    _prev_ord: i32,
    new_parent_id: Option<i64>,
    new_ord: i32,
) -> Result<TreeOp, TreeOpError> {
    let (old_parent_id, old_ord) = {
        let b = tree
            .get(block_id)
            .ok_or(TreeOpError::BlockNotFound(block_id))?;
        (b.parent_id, b.ord)
    };

    let new_depth = match new_parent_id {
        Some(pid) => {
            tree.get(pid).ok_or(TreeOpError::BlockNotFound(pid))?.depth + 1
        }
        None => 0,
    };

    // Close gap under old parent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == old_parent_id && sib.ord > old_ord {
            sib.ord -= 1;
        }
    }
    // Open gap under new parent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == new_parent_id && sib.ord >= new_ord {
            sib.ord += 1;
        }
    }
    let b = tree.get_mut(block_id).unwrap();
    b.parent_id = new_parent_id;
    b.ord = new_ord;
    b.depth = new_depth;

    Ok(TreeOp::Move {
        block_id,
        prev_parent_id: new_parent_id,
        prev_ord: new_ord,
        new_parent_id: old_parent_id,
        new_ord: old_ord,
    })
}

fn apply_delete_or_restore(
    tree: &mut MutableTree,
    block_id: i64,
    snapshot: BlockSnapshot,
) -> Result<TreeOp, TreeOpError> {
    if tree.get(block_id).is_some() {
        apply_delete(tree, block_id)
    } else {
        apply_restore(tree, block_id, snapshot)
    }
}

fn apply_delete(tree: &mut MutableTree, block_id: i64) -> Result<TreeOp, TreeOpError> {
    let (raw, depth, parent_id, ord) = {
        let b = tree
            .get(block_id)
            .ok_or(TreeOpError::BlockNotFound(block_id))?;
        (b.raw.clone(), b.depth, b.parent_id, b.ord)
    };

    // Collect child ids in their ord order.
    let mut children_sorted: Vec<&MutableBlock> = tree
        .blocks
        .iter()
        .filter(|b| b.parent_id == Some(block_id))
        .collect();
    children_sorted.sort_by_key(|b| b.ord);
    let child_ids: Vec<i64> = children_sorted.iter().map(|b| b.id).collect();

    let n_children = child_ids.len() as i32;
    let shift = n_children - 1; // negative if 0 children — closes gap

    // Adjust later siblings under the deleted block's parent.
    for sib in tree.blocks.iter_mut() {
        if sib.id != block_id && sib.parent_id == parent_id && sib.ord > ord {
            sib.ord += shift;
        }
    }

    // Reparent children: into deleted's parent, at ord = deleted.ord + i, depth -= 1.
    for (i, &cid) in child_ids.iter().enumerate() {
        let c = tree.get_mut(cid).unwrap();
        c.parent_id = parent_id;
        c.ord = ord + i as i32;
        c.depth = c.depth.saturating_sub(1);
    }

    tree.blocks.retain(|b| b.id != block_id);

    Ok(TreeOp::Delete {
        block_id,
        snapshot: BlockSnapshot {
            raw,
            depth,
            parent_id,
            ord,
            reparented_children: child_ids,
        },
    })
}

fn apply_restore(
    tree: &mut MutableTree,
    block_id: i64,
    snapshot: BlockSnapshot,
) -> Result<TreeOp, TreeOpError> {
    let n = snapshot.reparented_children.len() as i32;

    // Step 1: pull formerly-reparented children back under block_id, depth+1.
    // Their current ord is snapshot.ord + i; their new ord under block_id is i.
    if n > 0 {
        for (i, &cid) in snapshot.reparented_children.iter().enumerate() {
            let c = tree
                .get_mut(cid)
                .ok_or(TreeOpError::BlockNotFound(cid))?;
            c.parent_id = Some(block_id);
            c.depth = snapshot.depth + 1;
            c.ord = i as i32;
        }
    }

    // Step 2: shift remaining siblings under snapshot.parent_id at ord >=
    // snapshot.ord + n right by (1 - n) so the snapshot.ord slot opens.
    let shift = 1 - n;
    for sib in tree.blocks.iter_mut() {
        if sib.parent_id == snapshot.parent_id && sib.ord >= snapshot.ord + n {
            sib.ord += shift;
        }
    }

    // Step 3: insert the restored block.
    tree.blocks.push(MutableBlock {
        id: block_id,
        parent_id: snapshot.parent_id,
        ord: snapshot.ord,
        depth: snapshot.depth,
        raw: snapshot.raw.clone(),
    });

    Ok(TreeOp::Delete {
        block_id,
        snapshot: BlockSnapshot::default(),
    })
}

#[cfg(test)]
#[path = "__tests__/tree_ops_test.rs"]
mod tree_ops_test;
