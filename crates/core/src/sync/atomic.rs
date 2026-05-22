//! Placeholder — implemented in Plan 03-01 Task 2 (GREEN).
//!
//! Splitting Task 1 / Task 2 keeps the RED→GREEN commits crisp: Task 1
//! lands `SelfWriteSet` + the module wiring; Task 2 fills this file with
//! `atomic_write_md`.

use std::io;
use std::path::Path;

use super::SelfWriteSet;

/// Stub. Real impl in Task 2.
#[allow(dead_code)]
pub fn atomic_write_md(
    _target: &Path,
    _contents: &[u8],
    _self_writes: &SelfWriteSet,
) -> io::Result<[u8; 32]> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic_write_md: implemented in Plan 03-01 Task 2",
    ))
}
