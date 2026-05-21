//! DB-location resolver — D-13 / IDX-06.
//!
//! Resolves the per-OS path of the SQLite DB file that backs a given notes
//! root. The DB MUST live outside the notes folder (PITFALLS §DB-em-folder-sync
//! — sync clients corrupt SQLite WAL files when they trip over them) and MUST
//! be deterministic per notes-root so reopening the same notes folder reuses
//! the same index file.
//!
//! Strategy:
//!   1. Canonicalize the notes-root path (resolves symlinks, makes it absolute).
//!   2. Stringify to UTF-8 (lossy ok — input came from the user, not the DB).
//!   3. Replace Windows `\` with `/` so the same logical path produces the same
//!      hash regardless of platform.
//!   4. NFC-normalize (so macOS-NFD vs Linux-NFC don't bifurcate the cache).
//!   5. BLAKE3 the resulting string (D-16), take the first 16 hex chars.
//!   6. Place the file under the per-platform user data directory:
//!         Linux   — `$XDG_DATA_HOME/foliom/` (fallback `$HOME/.local/share/foliom/`)
//!         macOS   — `$HOME/Library/Application Support/foliom/`
//!         Windows — `%LOCALAPPDATA%\foliom\`
//!   7. `mkdir -p` the `foliom/` directory so SQLite's `Connection::open` can
//!      create the .db file without a separate I/O dance.
//!
//! Per RESEARCH §DB Location: hand-rolled, NOT the `directories` crate — keeps
//! the dependency surface small and makes the `XDG_DATA_HOME` override
//! explicit (CI tests rely on it).

use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use super::StorageError;

/// Resolve the SQLite DB path for the given notes-root.
///
/// Side effect: creates the parent `foliom/` directory if it does not exist.
/// Does NOT create the `.db` file itself — that is `rusqlite::Connection::open`'s job.
pub fn resolve_db_path(notes_root: &Path) -> Result<PathBuf, StorageError> {
    let abs = notes_root.canonicalize()?;

    // Stable string form across platforms: forward-slash everywhere, NFC-normalized.
    let with_forward_slash = abs.to_string_lossy().replace('\\', "/");
    let nfc: String = with_forward_slash.nfc().collect();

    let hash = blake3::hash(nfc.as_bytes());
    let hex16: String = hash.to_hex().as_str().chars().take(16).collect();

    let base_dir = data_dir()?;
    let foliom_dir = base_dir.join("foliom");
    std::fs::create_dir_all(&foliom_dir)?;
    Ok(foliom_dir.join(format!("{}.db", hex16)))
}

#[cfg(target_os = "linux")]
fn data_dir() -> Result<PathBuf, StorageError> {
    // POSIX: an empty XDG_DATA_HOME is treated the same as unset.
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg));
        }
    }
    let home = std::env::var("HOME").map_err(|_| StorageError::NoHomeDir)?;
    Ok(PathBuf::from(home).join(".local/share"))
}

#[cfg(target_os = "macos")]
fn data_dir() -> Result<PathBuf, StorageError> {
    let home = std::env::var("HOME").map_err(|_| StorageError::NoHomeDir)?;
    Ok(PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(target_os = "windows")]
fn data_dir() -> Result<PathBuf, StorageError> {
    let local = std::env::var("LOCALAPPDATA").map_err(|_| StorageError::NoAppData)?;
    Ok(PathBuf::from(local))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialize all env-mutating tests in this module. `cargo test` runs unit
    /// tests in parallel by default, and concurrent `set_var` on the same key
    /// produces flaky results (other threads see your scratch value).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Set `XDG_DATA_HOME` to `temp` for the duration of the closure (Linux only).
    /// SAFETY: tests run single-threaded under `cargo test` only when explicitly
    /// requested; the env-var dance is acceptable because each test snapshot/restores.
    #[cfg(target_os = "linux")]
    fn with_xdg<F: FnOnce()>(temp: &Path, f: F) {
        let prev = std::env::var("XDG_DATA_HOME").ok();
        // SAFETY: tests in this module are run serially via `-- --test-threads=1`
        // when targetting env-mutating cases. For local-only Linux dev this is fine.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", temp);
        }
        f();
        unsafe {
            match prev {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn resolves_under_xdg_data_home_when_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let xdg = TempDir::new().unwrap();
        let notes = TempDir::new().unwrap();
        with_xdg(xdg.path(), || {
            let db = resolve_db_path(notes.path()).unwrap();
            assert!(
                db.starts_with(xdg.path().join("foliom")),
                "db path {:?} should be under {:?}/foliom",
                db,
                xdg.path()
            );
            assert!(db.extension().and_then(|s| s.to_str()) == Some("db"));
        });
    }

    #[test]
    fn deterministic_for_same_notes_root() {
        let notes = TempDir::new().unwrap();
        let a = resolve_db_path(notes.path()).unwrap();
        let b = resolve_db_path(notes.path()).unwrap();
        assert_eq!(a, b, "same notes_root must produce same DB filename");
    }

    #[test]
    fn differs_for_different_notes_roots() {
        let n1 = TempDir::new().unwrap();
        let n2 = TempDir::new().unwrap();
        let a = resolve_db_path(n1.path()).unwrap();
        let b = resolve_db_path(n2.path()).unwrap();
        assert_ne!(a, b, "different notes_roots must produce different DB filenames");
    }

    #[test]
    fn db_path_is_outside_notes_root() {
        let notes = TempDir::new().unwrap();
        let db = resolve_db_path(notes.path()).unwrap();
        // IDX-06 / T-04-01: the canonical anti-foot-gun — DB must never live
        // inside the notes folder where a sync client would touch it.
        let notes_canon = notes.path().canonicalize().unwrap();
        assert!(
            !db.starts_with(&notes_canon),
            "DB path {:?} is inside notes_root {:?} — IDX-06 violation",
            db,
            notes_canon
        );
    }

    #[test]
    fn filename_is_16_hex_chars_plus_db_extension() {
        let notes = TempDir::new().unwrap();
        let db = resolve_db_path(notes.path()).unwrap();
        let stem = db.file_stem().unwrap().to_str().unwrap();
        assert_eq!(stem.len(), 16, "stem should be 16 hex chars, got {:?}", stem);
        assert!(
            stem.chars().all(|c| c.is_ascii_hexdigit()),
            "stem should be hex, got {:?}",
            stem
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn empty_xdg_falls_back_to_home_local_share() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let notes = TempDir::new().unwrap();
        let prev_xdg = std::env::var("XDG_DATA_HOME").ok();
        let prev_home = std::env::var("HOME").ok();
        let fake_home = TempDir::new().unwrap();
        // SAFETY: see with_xdg().
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "");
            std::env::set_var("HOME", fake_home.path());
        }
        let result = resolve_db_path(notes.path());
        // Restore before assertion so a panic doesn't poison sibling tests.
        unsafe {
            match prev_xdg {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
        let db = result.unwrap();
        assert!(
            db.starts_with(fake_home.path().join(".local/share/foliom")),
            "with empty XDG_DATA_HOME, db should be under $HOME/.local/share/foliom; got {:?}",
            db
        );
    }
}
