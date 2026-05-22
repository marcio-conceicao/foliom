//! Write-Ahead Journal for rename operations.
//!
//! Each rename that touches multiple files writes a JSON-Lines entry to the
//! journal BEFORE any file is modified. This makes the rename crash-recoverable:
//! on next boot, any incomplete journal entry is replayed to complete the
//! remaining file rewrites.
//!
//! Journal format: one `JournalEntry` JSON per line (newline-delimited).
//! Location: `$XDG_DATA_HOME/foliom/<root-hash>.rename-journal`
//!
//! Recovery is idempotent per-op: each op checks whether the file currently
//! contains `old_text` at `(byte_offset, byte_length)`:
//!   - matches `old_text` → apply splice, mark op `applied: true`.
//!   - matches `new_text` → already applied, mark op `applied: true`, skip.
//!   - matches neither → log a warning, mark op `applied: skipped`.
//!
//! T-03-20 / T-03-25 mitigations.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage::location::resolve_journal_path;

/// A single rename operation — one `[[OldName]]` occurrence in one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalOp {
    /// Relative path (NFC + forward-slash) of the file that contains this ref.
    pub file: String,
    /// Exact bytes that should be at `(byte_offset..byte_offset+byte_length)`.
    pub old_text: String,
    /// Replacement bytes.
    pub new_text: String,
    /// Byte offset in the file where `old_text` starts.
    pub byte_offset: usize,
    /// Byte length of `old_text`.
    pub byte_length: usize,
    /// Set to `true` once the splice has been written to disk.
    pub applied: bool,
}

/// A single rename journal entry. The fields form the recovery checklist:
/// once `sql_committed = true` the SQL is final and we must drive forward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// UUID-v4 identifying this operation (avoids duplicate-entry bugs on retry).
    pub id: String,
    /// ISO 8601 timestamp when the rename started.
    pub started: String,
    /// Old page name.
    pub old_name: String,
    /// New page name.
    pub new_name: String,
    /// SQL `pages.id` of the renamed page (for the replay merge step).
    pub page_id: i64,
    /// Relative path of the page's own file before rename.
    pub old_file: String,
    /// Relative path after rename.
    pub new_file: String,
    /// Per-occurrence backlink rewrite ops.
    pub ops: Vec<JournalOp>,
    /// Set to `true` once the SQL transaction (name + refs update) committed.
    pub sql_committed: bool,
    /// Set to `true` once the page's own file has been renamed on disk.
    pub file_renamed: bool,
}

/// Journal handle — thin wrapper around the file path.
///
/// Operations are append-only during normal execution. Recovery reads,
/// updates in-place (via atomic overwrite), then clears on completion.
pub struct Journal {
    path: PathBuf,
}

impl Journal {
    /// Open (or create) the journal file for the given notes root.
    ///
    /// The journal lives next to the DB file in the foliom data directory,
    /// named `<root-hash>.rename-journal`.
    pub fn open_for_root(notes_root: &Path) -> io::Result<Self> {
        let path = resolve_journal_path(notes_root)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(Journal { path })
    }

    /// Direct constructor (tests only / explicit path).
    pub fn open_at(path: PathBuf) -> Self {
        Journal { path }
    }

    /// Append a new entry to the journal and `fsync` before returning.
    ///
    /// Uses `O_APPEND` so concurrent processes (unlikely in single-user app,
    /// but correct) don't clobber each other.
    pub fn append(&self, entry: &JournalEntry) -> io::Result<()> {
        let mut line = serde_json::to_string(entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push('\n');
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        f.write_all(line.as_bytes())?;
        f.sync_all()?;
        Ok(())
    }

    /// Read all incomplete entries from the journal.
    ///
    /// An entry is "pending" if `sql_committed = true` AND (any op has
    /// `applied = false` OR `file_renamed = false`). Pre-SQL entries are
    /// discarded — without SQL commit the operation never started from the
    /// DB's perspective.
    pub fn pending(&self) -> io::Result<Vec<JournalEntry>> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<JournalEntry>(line) {
                Ok(entry) => {
                    if entry.sql_committed
                        && (!entry.file_renamed || entry.ops.iter().any(|op| !op.applied))
                    {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    tracing::warn!(line = %line, error = %e, "malformed journal entry — skipping");
                }
            }
        }
        Ok(entries)
    }

    /// Rewrite the journal file with updated entries (atomic overwrite).
    ///
    /// Used by `mark_applied`, `mark_sql_committed`, etc. to update flags
    /// after each step completes.
    pub fn update_entries(&self, entries: &[JournalEntry]) -> io::Result<()> {
        let mut new_content = String::new();
        for entry in entries {
            let mut line = serde_json::to_string(entry)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            line.push('\n');
            new_content.push_str(&line);
        }

        // Read existing lines to preserve entries we didn't touch.
        let existing_text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };

        // Build id-set of entries being updated.
        let update_ids: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.id.as_str()).collect();

        // Keep existing entries that are NOT in the update set.
        let mut output = String::new();
        for line in existing_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(e) = serde_json::from_str::<JournalEntry>(line) {
                if !update_ids.contains(e.id.as_str()) {
                    output.push_str(line);
                    output.push('\n');
                }
            }
        }
        // Add updated entries.
        output.push_str(&new_content);

        // Atomic overwrite.
        atomic_write_journal(&self.path, output.as_bytes())?;
        Ok(())
    }

    /// Remove an entry by id (once fully applied).
    pub fn remove_entry(&self, entry_id: &str) -> io::Result<()> {
        let existing_text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut output = String::new();
        for line in existing_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<JournalEntry>(line) {
                Ok(e) if e.id == entry_id => {} // skip — removing this entry
                _ => {
                    output.push_str(line);
                    output.push('\n');
                }
            }
        }
        atomic_write_journal(&self.path, output.as_bytes())?;
        Ok(())
    }

    /// Truncate the journal to zero bytes (used when all entries are complete).
    pub fn clear(&self) -> io::Result<()> {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path)?;
        Ok(())
    }

    /// Returns the underlying path (for tests).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Atomic overwrite of the journal file (temp-file + rename).
fn atomic_write_journal(target: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "journal has no parent dir")
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;
    tmp.persist(target)
        .map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}
