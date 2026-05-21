//! Stage 1 — line-based segmenter (PRS-01, PRS-02, PRS-03).
//!
//! # Invariants (load-bearing — see RESEARCH §Two-Stage Parser → Stage 1)
//!
//! 1. `RawBlock` byte ranges are **contiguous and non-overlapping** when
//!    sorted by `byte_offset`.
//! 2. Concatenating `source[block.byte_offset .. block.byte_offset +
//!    block.byte_length]` for every block in order **exactly equals**
//!    `source`. This is the splice-noop property — proves ACPT-01 / PRS-07
//!    by construction.
//! 3. A "page prelude" `RawBlock` (depth = `u8::MAX`) covers any bytes
//!    before the first bullet line (page-level `title::` properties, blank
//!    lines, etc.). If the file starts with a bullet at byte 0, the prelude
//!    block has `byte_length = 0` and exists as a placeholder.
//!
//! # Status in plan 01-01
//!
//! The body of [`segment`] is a deliberate stub returning `Vec::new()`. The
//! round-trip CI gate (`crates/core/tests/roundtrip.rs`) is wired against
//! this stub and is **expected to fail** at the end of plan 01-01. Plan
//! 01-02 implements the real state machine and flips the gate green.

/// A raw block as produced by the line-based segmenter.
///
/// `raw` is the exact slice of the source covering this block, inclusive
/// of continuation lines, drawers, properties, and the trailing newline.
/// `properties` and `drawers` are parsed-out views over the same bytes —
/// the index uses them, but write-back always operates on the byte range
/// (PRS-05, PRS-06, D-05, D-06).
#[derive(Debug, Clone)]
pub struct RawBlock {
    /// TAB-indent count. `0` = top-level bullet, `1` = nested once, etc.
    /// `u8::MAX` is a sentinel for the page-prelude block.
    pub depth: u8,
    /// Absolute byte offset into the source file.
    pub byte_offset: usize,
    /// Length in bytes, inclusive of all continuation lines, drawers,
    /// properties and the trailing newline.
    pub byte_length: usize,
    /// Full raw text of the block (UTF-8). Equals
    /// `source[byte_offset..byte_offset + byte_length]`.
    pub raw: String,
    /// `key:: value` block properties found inside this block. Parsed for
    /// the index; `raw` already contains them verbatim.
    pub properties: Vec<(String, String)>,
    /// Drawers (`:LOGBOOK: ... :END:` and friends) found inside this
    /// block. Opaque blobs; byte ranges are relative to the source file.
    pub drawers: Vec<RawDrawer>,
}

/// A Logseq-style drawer. Treated as an opaque blob: we record name and
/// byte range, but never look inside (PRS-06, D-06).
#[derive(Debug, Clone)]
pub struct RawDrawer {
    /// Drawer name (e.g. `"LOGBOOK"`). Stored without surrounding colons.
    pub name: String,
    /// Absolute byte offset into the source file (start of `:NAME:` line).
    pub byte_offset: usize,
    /// Length in bytes, inclusive of the `:END:` closing line.
    pub byte_length: usize,
}

/// Segment a markdown source buffer into raw blocks.
///
/// **Stub:** Plan 01-01 ships the empty body so the round-trip CI gate is
/// RED before any parser implementation lands. Plan 01-02 replaces this
/// with the real state machine. The signature is the stable contract
/// consumed by [`crate::parser`] callers and the round-trip test.
pub fn segment(_source: &[u8]) -> Vec<RawBlock> {
    Vec::new()
}
