//! `walkdir`-driven scanner that enumerates `.md` files under a notes
//! root, respecting [`IgnoreSet`].
//!
//! Security-load-bearing properties (T-05-01 / T-05-02):
//!
//! * `WalkDir::follow_links(false)` — symlinks are NEVER followed.
//! * `filter_entry` prunes ignored directories (and any dotdir at any
//!   depth) BEFORE descent, so an ignored subtree contributes zero IO.
//! * Errors from walkdir (permission denied, transient races) are
//!   logged via `tracing::warn!` and the entry is silently dropped.
//!   This is the [`T-05-05`] mitigation — we must never panic from
//!   the iterator.
//!
//! Per-entry the scanner emits a [`ScanEntry`] carrying the absolute
//! filesystem path, mtime in nanoseconds since the Unix epoch, and
//! size in bytes. Hashing is the indexer's concern (Plan 01-06);
//! per RESEARCH §Reindex Algorithm the indexer trusts `(mtime, size)`
//! first and only hashes when they look unchanged.
//!
//! [`T-05-05`]: ../../README

use std::fs::Metadata;
use std::path::{Path, PathBuf};

use super::ignore::IgnoreSet;

/// One survivor of the walk: an absolute filesystem path plus the
/// metadata Plan 01-06's indexer needs to decide whether the file is
/// up-to-date in the DB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: PathBuf,
    pub mtime_ns: i64,
    pub size: u64,
}

/// Recursively walk `root`, yielding one [`ScanEntry`] per `.md` file
/// that survives `ignore`. Never follows symlinks. Swallows IO errors
/// after logging them via `tracing::warn!`.
pub fn walk<'a>(
    root: &Path,
    ignore: &'a IgnoreSet,
) -> impl Iterator<Item = ScanEntry> + 'a {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(move |e| {
            // Only prune at directory level — files inherit their parent's
            // verdict by virtue of having been descended into.
            if e.file_type().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    if ignore.is_ignored(name) {
                        return false;
                    }
                    // Skip dotdirs at any depth, but never the root itself
                    // (which could have a `.` as its file_name when the
                    // caller passes a relative `Path::new(".")`).
                    if name.starts_with('.') && name != "." && name != ".." {
                        // Don't filter the root itself.
                        if e.depth() > 0 {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .filter_map(|res| match res {
            Ok(e) => Some(e),
            Err(err) => {
                tracing::warn!(error = %err, "walkdir error — dropping entry");
                None
            }
        })
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .filter_map(|e| {
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(err) => {
                    tracing::warn!(path = %e.path().display(), error = %err, "metadata failed");
                    return None;
                }
            };
            let mtime_ns = mtime_ns_from_meta(&meta)?;
            Some(ScanEntry {
                path: e.into_path(),
                mtime_ns,
                size: meta.len(),
            })
        })
}

/// Extract mtime as nanoseconds since the Unix epoch.
///
/// On Unix we read `mtime` + `mtime_nsec` directly via `MetadataExt` —
/// this is the highest-resolution form and never allocates.
///
/// On Windows we fall through to `Metadata::modified()` + `SystemTime`
/// arithmetic. Resolution is filesystem-dependent (NTFS = 100 ns,
/// FAT = 2 s); good enough for the cache-key purpose.
#[cfg(unix)]
fn mtime_ns_from_meta(meta: &Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    let secs = meta.mtime();
    let nsecs = meta.mtime_nsec();
    secs.checked_mul(1_000_000_000)?.checked_add(nsecs as i64)
}

#[cfg(windows)]
fn mtime_ns_from_meta(meta: &Metadata) -> Option<i64> {
    use std::time::UNIX_EPOCH;
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(dur.as_nanos()).ok()
}

#[cfg(not(any(unix, windows)))]
fn mtime_ns_from_meta(meta: &Metadata) -> Option<i64> {
    use std::time::UNIX_EPOCH;
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(dur.as_nanos()).ok()
}
