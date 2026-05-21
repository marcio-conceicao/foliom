//! Filesystem scanner: walks the notes root, applies the IDX-01 ignore list,
//! reads `:hidden` from `logseq/config.edn` when present, and emits one
//! `ScanEntry` per surviving `.md` file.
//!
//! The scanner is the IO entry point for the indexer (Plan 01-06). It is
//! deliberately split into three small modules:
//!
//! * [`ignore`] — the hard-coded `DEFAULT_LOGSEQ_IGNORES` and `IgnoreSet`
//!   wrapper that the walker consults at every directory entry.
//! * [`walk`] — the `walkdir::filter_entry`-driven walker that produces
//!   `ScanEntry { path, mtime_ns, size }`.
//! * [`config_edn`] — a deliberately minimal regex-based extractor for the
//!   single `:hidden` key Phase 1 cares about. Documented scope; Phase 2
//!   replaces this if the renderer ever needs more keys.

pub mod config_edn;
pub mod ignore;
pub mod walk;

pub use ignore::{DEFAULT_LOGSEQ_IGNORES, IgnoreSet};
pub use walk::{ScanEntry, walk};
