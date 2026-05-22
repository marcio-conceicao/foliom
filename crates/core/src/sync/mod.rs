//! Phase 3 plan 03-01 — disk-write primitives.
//!
//! This module is the only path in `crates/core` that creates or replaces
//! user `.md` files. Every Phase 3 mutation handler (block edit, page
//! rename, etc.) routes through [`atomic_write_md`]; the [`SelfWriteSet`]
//! lets the Phase 4 watcher recognise (and ignore) Foliom's own writes
//! so it doesn't re-index a file we just authored.
//!
//! See `.planning/phases/03-outliner-editor/03-RESEARCH.md` §2 for the
//! design rationale and `03-01-PLAN.md` for the contract.

pub mod atomic;
pub mod self_writes;

pub use atomic::atomic_write_md;
pub use self_writes::SelfWriteSet;
