//! Atomic temp+rename write for Foliom's user `.md` files.
//!
//! Every Phase 3 mutation handler routes through [`atomic_write_md`]. The
//! guarantee: readers (Foliom or external — Obsidian, VS Code, watcher)
//! observe **either** the old bytes **or** the full new bytes, never a
//! partial buffer. Achieved by writing to a sibling temp file (same FS as
//! target → same inode device → `rename(2)` / `MoveFileExW` is atomic) and
//! `fsync`ing before the rename.
//!
//! Threats mitigated (see PLAN 03-01 `<threat_model>`):
//!   - T-03-01 Tampering: partial write visible to readers.
//!   - T-03-02 DoS: Windows AV briefly locking the target during rename.
//!   - T-03-03 Repudiation: Foliom's own write echoed by Phase 4 watcher
//!     (mitigated via [`SelfWriteSet::register`] called BEFORE the rename).
//!
//! Cross-filesystem rename produces [`std::io::ErrorKind::CrossesDevices`]
//! (the temp file must live on the same FS as the target — we always
//! create it in `target.parent()`). The mutation handler in 03-03
//! translates this to an HTTP 500 with a diagnostic message.

use std::io::{self, Write};
use std::path::Path;

use super::SelfWriteSet;

/// Maximum retry attempts on transient Windows persist failures.
/// Matches 03-RESEARCH §2 budget (3 retries × 50/100/200ms = 350ms).
#[cfg(windows)]
const RETRY_MAX: u32 = 3;
#[cfg(windows)]
const RETRY_BASE_MS: u64 = 50;

// Test-only counter so the Windows AV-retry smoke test can assert the
// retry loop fired. Reset by each call to `atomic_write_md`.
#[cfg(test)]
thread_local! {
    pub(crate) static LAST_PERSIST_ATTEMPTS: std::cell::Cell<u32> =
        const { std::cell::Cell::new(0) };
}

/// Atomically replace `target` with `contents`.
///
/// Steps (see 03-RESEARCH §2):
///   1. Compute BLAKE3 hash of `contents`.
///   2. Register the hash in `self_writes` BEFORE rename (closes the race
///      with a Phase 4 watcher that might observe the write before we
///      record it).
///   3. Create a `NamedTempFile` in `target.parent()` (same-FS guarantee).
///   4. Write + `sync_all` the temp file.
///   5. Persist (rename) onto `target`, retrying on Windows
///      `PermissionDenied` / `Other` up to [`RETRY_MAX`] times with
///      exponential backoff. Unix has no retry — `rename(2)` is either
///      atomic or fatally broken.
///   6. (Unix only) `sync_all` the parent directory for crash safety.
///   7. Return the hash so the caller can persist it to `files.hash`.
pub fn atomic_write_md(
    target: &Path,
    contents: &[u8],
    self_writes: &SelfWriteSet,
) -> io::Result<[u8; 32]> {
    #[cfg(test)]
    LAST_PERSIST_ATTEMPTS.with(|c| c.set(0));

    // (1) Hash up front — needed for register() and as the return value.
    let hash: [u8; 32] = blake3::hash(contents).into();

    // (2) Register BEFORE rename. If we crash mid-write the rename never
    //     happens and the registry entry expires via TTL — no leak.
    self_writes.register(hash);

    // (3) Resolve the parent dir. Without a parent we cannot create a
    //     sibling temp file → cannot guarantee same-FS atomicity.
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic_write_md: target has no parent directory",
        )
    })?;
    // Surface "parent missing" before we burn a tempfile creation. This
    // also turns the cross-FS path into a clear NotFound when the caller
    // passes a fabricated path during tests.
    if !parent.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "atomic_write_md: parent directory does not exist: {}",
                parent.display()
            ),
        ));
    }

    // (4) Write to a same-directory temp file and fsync before rename.
    //     NamedTempFile::new_in keeps the temp on the target's FS, which
    //     is what makes rename atomic and avoids ErrorKind::CrossesDevices.
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;

    // Consume the NamedTempFile into a TempPath so we can re-persist on
    // retry — `NamedTempFile::persist` consumes self; `TempPath::persist`
    // hands the TempPath back inside `PathPersistError` (Assumption A9
    // verified against tempfile 3.x: `PathPersistError { error, path }`).
    // `mut` is only needed on Windows where the retry loop reassigns
    // `temp_path` from `PathPersistError::path`. On unix we fail-fast.
    #[cfg(windows)]
    let mut temp_path = tmp.into_temp_path();
    #[cfg(not(windows))]
    let temp_path = tmp.into_temp_path();
    #[allow(unused_mut)]
    let mut attempt: u32 = 0;
    loop {
        match temp_path.persist(target) {
            Ok(()) => break,
            Err(err) => {
                // `err` is PathPersistError on tempfile 3.x — gives us back
                // the TempPath so we can retry without rewriting contents.
                let io_err = err.error;
                #[cfg(windows)]
                {
                    temp_path = err.path;
                }
                #[cfg(not(windows))]
                {
                    // Silence the dead-store warning on unix where we
                    // never re-enter the loop.
                    let _ = err.path;
                }

                #[cfg(test)]
                LAST_PERSIST_ATTEMPTS.with(|c| c.set(c.get() + 1));

                #[cfg(windows)]
                {
                    let retryable = matches!(
                        io_err.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::Other
                    );
                    if !retryable || attempt >= RETRY_MAX {
                        return Err(io_err);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(
                        RETRY_BASE_MS << attempt,
                    ));
                    attempt += 1;
                }
                #[cfg(not(windows))]
                {
                    // unix: rename(2) failures are not transient — fail fast.
                    let _ = &mut attempt;
                    return Err(io_err);
                }
            }
        }
    }

    // (6) Parent fsync on unix — protects the rename across power loss on
    //     ext4-without-barriers and similar. Best-effort: if the directory
    //     handle can't be opened, the rename already succeeded and the
    //     window is small, so we don't escalate the error.
    #[cfg(unix)]
    {
        if let Ok(parent_dir) = std::fs::File::open(parent) {
            let _ = parent_dir.sync_all();
        }
    }

    Ok(hash)
}

#[cfg(test)]
#[path = "__tests__/atomic_test.rs"]
mod atomic_test;
