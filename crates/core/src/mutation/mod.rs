//! Pure mutation primitives — SNC-01.
//!
//! This module is pure (no IO, no HTTP, no SQL). It exposes two layers:
//!
//! - `splice`: byte-splice into a file buffer and downstream offset shifting.
//! - `tree_ops`: invertible tree operations (Indent/Outdent/Merge/Split/Move/
//!   Delete) over an in-memory `MutableTree`.
//!
//! The HTTP handlers in plan 03-03 compose these with `atomic_write` from
//! plan 03-01 (`crates/core/src/sync/`).

pub mod splice;
pub mod tree_ops;

pub use splice::{compute_shifted_offsets, splice_block, BlockOffset};
pub use tree_ops::{BlockSnapshot, MutableBlock, MutableTree, TreeOp, TreeOpError};
