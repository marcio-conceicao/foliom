//! Two-stage parser (PRS-03).
//!
//! - Stage 1 (`segment`): line-based segmenter — TAB indent + 2-space
//!   continuation + fence-aware + drawer-aware. Owns the byte-exact
//!   round-trip contract (PRS-07 / ACPT-01).
//! - Stage 2 (added in a later plan): per-block CommonMark/GFM via
//!   `pulldown-cmark` with ref extraction.

pub mod segment;
