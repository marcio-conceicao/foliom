//! Byte-splice and offset-shift primitives — SNC-01 foundation.
//!
//! These are *pure* functions: no IO, no allocation beyond the result buffer,
//! no globals. The HTTP layer (plan 03-03) composes them with `atomic_write`
//! (plan 03-01).

use crate::parser::segment::RawBlock;

/// Splice `new_raw` into `original` at `[byte_offset, byte_offset + byte_length)`.
///
/// Returns a fresh buffer with the substitution applied. Bytes outside the
/// changed range are byte-identical to `original`. Panics if `byte_offset +
/// byte_length` exceeds `original.len()`.
pub fn splice_block(
    original: &[u8],
    byte_offset: usize,
    byte_length: usize,
    new_raw: &[u8],
) -> Vec<u8> {
    assert!(
        byte_offset + byte_length <= original.len(),
        "splice_block: range [{}, {}) out of bounds for buffer of {} bytes",
        byte_offset,
        byte_offset + byte_length,
        original.len()
    );
    let mut out = Vec::with_capacity(original.len() - byte_length + new_raw.len());
    out.extend_from_slice(&original[..byte_offset]);
    out.extend_from_slice(new_raw);
    out.extend_from_slice(&original[byte_offset + byte_length..]);
    out
}

/// Minimal access pattern needed by `compute_shifted_offsets`. Implemented
/// for `RawBlock`, for the tuple `(i64, usize, usize)` used in tests, and for
/// any future storage row type that exposes the same three fields.
pub trait BlockOffset {
    fn id(&self) -> i64;
    fn byte_offset(&self) -> usize;
    fn byte_length(&self) -> usize;
}

impl BlockOffset for (i64, usize, usize) {
    fn id(&self) -> i64 {
        self.0
    }
    fn byte_offset(&self) -> usize {
        self.1
    }
    fn byte_length(&self) -> usize {
        self.2
    }
}

impl BlockOffset for RawBlock {
    /// `RawBlock` has no SQL id yet — storage rows wrap it in plan 03-03.
    /// We return `0` so the type satisfies the trait for offset-only callers
    /// (the splice tests). Production callers always use the storage-row
    /// wrapper which carries a real id.
    fn id(&self) -> i64 {
        0
    }
    fn byte_offset(&self) -> usize {
        self.byte_offset
    }
    fn byte_length(&self) -> usize {
        self.byte_length
    }
}

/// Recompute `(byte_offset, byte_length)` for every block in `blocks` after the
/// block identified by `changed_block_id` was resized from `old_len` to `new_len`.
///
/// - Blocks BEFORE the changed block are unchanged.
/// - The changed block keeps its `byte_offset` and adopts `new_len` as its
///   `byte_length`.
/// - Blocks AFTER the changed block shift by `(new_len - old_len) as i64`.
///
/// `blocks` does not have to be sorted; we locate the changed block by id and
/// use its `byte_offset` to partition.
///
/// Panics if `changed_block_id` is not found in `blocks`, or if applying a
/// negative shift would underflow a downstream offset (would only happen for
/// pathological inputs — the splice contract guarantees this is impossible).
pub fn compute_shifted_offsets<B: BlockOffset>(
    blocks: &[B],
    changed_block_id: i64,
    old_len: usize,
    new_len: usize,
) -> Vec<(i64, usize, usize)> {
    let changed = blocks
        .iter()
        .find(|b| b.id() == changed_block_id)
        .expect("compute_shifted_offsets: changed_block_id not in blocks");
    let pivot_offset = changed.byte_offset();
    // Sanity: old_len matches what we stored. Loud failure protects the caller
    // from feeding us stale state.
    debug_assert_eq!(
        changed.byte_length(),
        old_len,
        "compute_shifted_offsets: old_len does not match block's recorded byte_length",
    );

    let delta: i64 = new_len as i64 - old_len as i64;
    let mut out = Vec::with_capacity(blocks.len());
    for b in blocks {
        if b.id() == changed_block_id {
            out.push((b.id(), b.byte_offset(), new_len));
        } else if b.byte_offset() > pivot_offset {
            let shifted = (b.byte_offset() as i64) + delta;
            assert!(
                shifted >= 0,
                "compute_shifted_offsets: downstream offset underflow (id={}, off={}, delta={})",
                b.id(),
                b.byte_offset(),
                delta
            );
            out.push((b.id(), shifted as usize, b.byte_length()));
        } else {
            out.push((b.id(), b.byte_offset(), b.byte_length()));
        }
    }
    out
}

#[cfg(test)]
#[path = "__tests__/splice_test.rs"]
mod splice_test;
