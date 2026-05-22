//! Placeholder — populated by Task 2 of plan 03-02.
//! Kept here so `mutation::mod` compiles during Task 1's RED/GREEN cycles.

use serde::{Deserialize, Serialize};

/// Block snapshot used by `TreeOp::Delete` to record everything needed to
/// rebuild the deleted block (see Task 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSnapshot {
    pub raw: String,
    pub depth: u8,
    pub parent_id: Option<i64>,
    pub ord: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeOp {
    Indent {
        block_id: i64,
        prev_depth: u8,
    },
    Outdent {
        block_id: i64,
        prev_depth: u8,
    },
    Merge {
        block_id: i64,
        merged_into_id: i64,
        original_raw: String,
    },
    Split {
        block_id: i64,
        at_offset: usize,
        new_block_id: i64,
    },
    Move {
        block_id: i64,
        prev_parent_id: Option<i64>,
        prev_ord: i32,
        new_parent_id: Option<i64>,
        new_ord: i32,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableBlock {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub ord: i32,
    pub depth: u8,
    pub raw: String,
}

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
    pub fn children_of(&self, parent_id: Option<i64>) -> Vec<&MutableBlock> {
        let mut v: Vec<&MutableBlock> = self
            .blocks
            .iter()
            .filter(|b| b.parent_id == parent_id)
            .collect();
        v.sort_by_key(|b| b.ord);
        v
    }
    /// Return the id of the block that immediately precedes `id` among its
    /// siblings (same parent_id, lower ord, max such ord). `None` if `id` is
    /// the first child.
    pub fn predecessor_at_same_depth(&self, id: i64) -> Option<i64> {
        let me = self.get(id)?;
        self.blocks
            .iter()
            .filter(|b| b.parent_id == me.parent_id && b.ord < me.ord)
            .max_by_key(|b| b.ord)
            .map(|b| b.id)
    }
}

impl TreeOp {
    /// Stub: Task 2 implements the real semantics.
    pub fn apply(self, _tree: &mut MutableTree) -> Result<TreeOp, TreeOpError> {
        unimplemented!("TreeOp::apply — implemented in Task 2 of plan 03-02")
    }
}
