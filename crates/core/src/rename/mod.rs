//! Page rename orchestrator — WAL journal + SQL + file rewrites.
//!
//! Entry point: [`rename_page`] — called from the HTTP handler via
//! `spawn_blocking`.
//!
//! Recovery entry point: [`replay_journal`] — called at server boot if the
//! journal file contains any incomplete entries (boot path in `cmd::serve::mod.rs`).
//!
//! Crash safety contract (T-03-20):
//!   1. Journal entry appended (+ fsync) BEFORE any SQL or file mutation.
//!   2. SQL transaction committed. Entry marked `sql_committed`.
//!   3. Each backlink file rewritten via `atomic_write_md`. Op marked `applied`.
//!   4. Page file renamed on disk. Entry marked `file_renamed`.
//!   5. Journal entry removed.
//!
//! If the process is killed between steps 2 and 5, replay-on-boot picks up
//! from the first un-applied op.

pub mod journal;

pub use journal::{Journal, JournalEntry, JournalOp};

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use crate::path::RelativePath;
use crate::sync::{SelfWriteSet, atomic_write_md};

/// Windows-reserved file-name characters and device names.
///
/// Per 03-RESEARCH Pitfall 6 + T-03-21: reject these before touching the FS.
const RESERVED_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Result returned to the HTTP handler.
#[derive(Debug)]
pub struct RenameResult {
    /// Number of files that had their backlink text rewritten.
    pub rewritten_count: u32,
    /// Non-fatal warnings (e.g. bytes didn't match old or new text).
    pub warnings: Vec<String>,
}

/// Errors from rename operations.
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("page '{0}' not found")]
    NotFound(String),
    #[error("target name '{0}' already exists as a backed page")]
    TargetExists(String),
    #[error("invalid page name: {0}")]
    InvalidName(String),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("journal error: {0}")]
    Journal(String),
}

/// Validate that `name` is safe to use as a page name and file-system component.
///
/// Rejects Windows-reserved chars, Windows-reserved device names (case-insensitive),
/// NUL byte, path separators, and empty names. Returns `Err(RenameError::InvalidName)`
/// on failure.
pub fn validate_page_name(name: &str) -> Result<(), RenameError> {
    if name.is_empty() {
        return Err(RenameError::InvalidName("name cannot be empty".into()));
    }
    if name.contains('\0') {
        return Err(RenameError::InvalidName("name contains NUL byte".into()));
    }
    for ch in RESERVED_CHARS {
        if name.contains(*ch) {
            return Err(RenameError::InvalidName(format!(
                "name contains reserved character '{ch}'"
            )));
        }
    }
    let upper = name.to_uppercase();
    // Strip dot-extension for device-name check (CON.md → CON)
    let stem = upper.split('.').next().unwrap_or(&upper);
    for &reserved in RESERVED_NAMES {
        if stem == reserved {
            return Err(RenameError::InvalidName(format!(
                "'{name}' is a Windows-reserved device name"
            )));
        }
    }
    // Reject leading/trailing dots and spaces (Windows edge cases)
    if name.starts_with('.') || name.ends_with('.') || name.starts_with(' ') || name.ends_with(' ') {
        return Err(RenameError::InvalidName(
            "name cannot start or end with '.' or ' '".into(),
        ));
    }
    Ok(())
}

/// Backlink occurrence enumerated from `refs + blocks + files`.
#[derive(Debug)]
struct BacklinkOccurrence {
    /// Relative path of the file containing the reference.
    file_path: String,
    /// Byte offset in the file where the `[[OldName]]` text starts.
    byte_offset: usize,
    /// Byte length of the `[[OldName]]` text.
    byte_length: usize,
    /// Exact text at those bytes (e.g. `[[OldName]]` or `[[OldName|alias]]`).
    old_text: String,
    /// Replacement text (e.g. `[[NewName]]` or `[[NewName|alias]]`).
    new_text: String,
}

/// Find all `[[old_name]]` and `[[old_name|alias]]` occurrences in all files
/// that reference `page_id`, using `blocks.raw` + byte offsets.
///
/// This uses a text scan of each referencing block's `raw` field to locate
/// the exact byte positions within the file. Because blocks know their
/// `(byte_offset, byte_length)` in the file, we can compute absolute
/// positions.
fn enumerate_backlinks(
    conn: &Connection,
    page_id: i64,
    old_name: &str,
    new_name: &str,
) -> Result<Vec<BacklinkOccurrence>, rusqlite::Error> {
    // Query: all (file_path, block.byte_offset, block.raw) for blocks that
    // reference `page_id` via refs table.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT f.path, b.byte_offset, b.raw \
         FROM refs r \
         JOIN blocks b ON b.id = r.source_block \
         JOIN pages p ON p.id = b.page_id \
         JOIN files f ON f.id = p.file_id \
         WHERE r.target_page = ?1",
    )?;
    let rows: Vec<(String, i64, String)> = stmt
        .query_map(params![page_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut occurrences = Vec::new();
    for (file_path, block_byte_offset, raw) in rows {
        // Scan raw for [[old_name]] and [[old_name|alias]] patterns
        let raw_bytes = raw.as_bytes();
        let mut pos = 0usize;
        while pos < raw_bytes.len() {
            // Find `[[`
            if let Some(open) = find_bytes(raw_bytes, b"[[", pos) {
                let after_open = open + 2;
                // Find the next `]]`
                if let Some(close) = find_bytes(raw_bytes, b"]]", after_open) {
                    let inner = &raw[after_open..close];
                    // inner is either "OldName" or "OldName|alias"
                    let page_part = inner.split('|').next().unwrap_or(inner);
                    if page_part.eq_ignore_ascii_case(old_name) {
                        let abs_offset = block_byte_offset as usize + open;
                        let chunk_len = close + 2 - open; // includes [[ and ]]
                        let old_text = &raw[open..close + 2];
                        // Build new_text: keep the alias if present
                        let new_text = if inner.contains('|') {
                            let alias = &inner[inner.find('|').unwrap()..];
                            format!("[[{new_name}{alias}]]")
                        } else {
                            format!("[[{new_name}]]")
                        };
                        occurrences.push(BacklinkOccurrence {
                            file_path: file_path.clone(),
                            byte_offset: abs_offset,
                            byte_length: chunk_len,
                            old_text: old_text.to_string(),
                            new_text,
                        });
                    }
                    pos = close + 2;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
    Ok(occurrences)
}

fn find_bytes(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack[start..].windows(needle.len()).position(|w| w == needle).map(|p| p + start)
}

/// State needed by replay (mirrors the subset of AppState we need).
///
/// Defined as a trait to avoid importing the HTTP layer from core.
pub trait RenameState {
    fn db(&self) -> &Arc<Mutex<crate::storage::Db>>;
    fn root(&self) -> &Path;
    fn journal(&self) -> &Arc<Journal>;
    fn self_writes(&self) -> &Arc<SelfWriteSet>;
}

/// Replay any pending journal entries — called at server boot.
///
/// For each pending entry:
///   - For each op where `applied = false`:
///     - Read the file, check bytes at (offset, length).
///     - If `old_text` matches: splice in `new_text`, `atomic_write_md`.
///     - If `new_text` already matches: already applied — mark done, continue.
///     - Otherwise: log warning, mark as applied-skipped.
///   - If `file_renamed = false` and both old and new paths are accessible:
///     - Perform the disk rename.
///   - Remove the journal entry.
pub fn replay_journal<S: RenameState>(state: &S) -> Result<(), RenameError> {
    let journal = state.journal();
    let pending = journal.pending().map_err(|e| RenameError::Journal(e.to_string()))?;
    if pending.is_empty() {
        return Ok(());
    }
    for mut entry in pending {
        let root = state.root();
        let self_writes = state.self_writes();
        let mut warnings: Vec<String> = Vec::new();

        // Apply each pending op.
        for op in &mut entry.ops {
            if op.applied {
                continue;
            }
            let abs_path = RelativePath::from_storage_str(&op.file).to_filesystem(root);
            let file_bytes = match std::fs::read(&abs_path) {
                Ok(b) => b,
                Err(e) => {
                    let msg = format!("replay: cannot read {:?}: {e}", abs_path);
                    tracing::warn!("{}", msg);
                    warnings.push(msg);
                    op.applied = true; // mark as done to not block on missing file
                    continue;
                }
            };

            let start = op.byte_offset;
            let end = start + op.byte_length;
            if end > file_bytes.len() {
                let msg = format!(
                    "replay: op offset {start}..{end} out of range for {:?} (len {})",
                    abs_path,
                    file_bytes.len()
                );
                tracing::warn!("{}", msg);
                warnings.push(msg);
                op.applied = true;
                continue;
            }
            let window = &file_bytes[start..end];
            if window == op.old_text.as_bytes() {
                // Apply splice.
                let mut new_bytes = Vec::with_capacity(
                    file_bytes.len() - op.byte_length + op.new_text.len(),
                );
                new_bytes.extend_from_slice(&file_bytes[..start]);
                new_bytes.extend_from_slice(op.new_text.as_bytes());
                new_bytes.extend_from_slice(&file_bytes[end..]);
                atomic_write_md(&abs_path, &new_bytes, self_writes)
                    .map_err(RenameError::Io)?;
                op.applied = true;
                tracing::info!(file = %op.file, "replay: applied op {} → {}", op.old_text, op.new_text);
            } else if window == op.new_text.as_bytes() {
                // Already applied.
                op.applied = true;
                tracing::info!(file = %op.file, "replay: op already applied (idempotent)");
            } else {
                let msg = format!(
                    "replay: bytes at {}..{} in {:?} match neither old_text {:?} nor new_text {:?} — skipping",
                    start, end, abs_path, op.old_text, op.new_text
                );
                tracing::warn!("{}", msg);
                warnings.push(msg);
                op.applied = true;
            }
        }

        // Rename the page file if not yet done.
        if !entry.file_renamed {
            if entry.old_file.is_empty() && entry.new_file.is_empty() {
                // No file to rename (page may have been unresolved).
                entry.file_renamed = true;
            } else {
                let old_abs = RelativePath::from_storage_str(&entry.old_file).to_filesystem(root);
                let new_abs = RelativePath::from_storage_str(&entry.new_file).to_filesystem(root);
                if old_abs.exists() {
                    rename_file_on_disk(&old_abs, &new_abs).map_err(RenameError::Io)?;
                    entry.file_renamed = true;
                } else if new_abs.exists() {
                    // Already renamed.
                    entry.file_renamed = true;
                } else {
                    // Neither file exists — the rename already happened or the file was
                    // removed by another tool. Mark as done to allow cleanup.
                    tracing::warn!(
                        old_file = %entry.old_file,
                        new_file = %entry.new_file,
                        "replay: neither old nor new file exists — marking file_renamed=true"
                    );
                    entry.file_renamed = true;
                }
            }
        }

        // Remove the journal entry once everything is done.
        if entry.file_renamed && entry.ops.iter().all(|op| op.applied) {
            journal.remove_entry(&entry.id).map_err(|e| RenameError::Journal(e.to_string()))?;
        } else {
            // Update with partial progress.
            journal.update_entries(&[entry]).map_err(|e| RenameError::Journal(e.to_string()))?;
        }
    }
    Ok(())
}

/// Perform the disk rename, handling the case-only rename quirk on Windows.
///
/// On Windows, renaming `Foo.md` → `foo.md` is a no-op because NTFS is
/// case-insensitive. We use a two-step rename via a temp name to force
/// the FS to see two distinct operations.
pub fn rename_file_on_disk(old: &Path, new: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let old_name = old.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let new_name = new.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if old_name.to_uppercase() == new_name.to_uppercase() && old_name != new_name {
            // Case-only rename: two-step via __foliom_rename_tmp__
            let parent = old.parent().unwrap_or(old);
            let tmp = parent.join("__foliom_rename_tmp__");
            std::fs::rename(old, &tmp)?;
            std::fs::rename(&tmp, new)?;
            return Ok(());
        }
    }
    std::fs::rename(old, new)
}

use std::io;

/// Rename a page: update SQL, rewrite backlinks, rename the file.
///
/// Called from `POST /api/pages/:name/rename` handler via `spawn_blocking`.
#[allow(clippy::too_many_arguments)]
pub fn rename_page(
    conn: &mut Connection,
    root: &Path,
    journal: &Journal,
    self_writes: &SelfWriteSet,
    old_name: &str,
    new_name: &str,
    rewrite_backlinks: bool,
) -> Result<RenameResult, RenameError> {
    validate_page_name(new_name)?;

    // Generate a UUID-ish id for this journal entry.
    let entry_id = format!("rename-{}-{}", old_name, new_name);

    // 1. Look up old page.
    let old_row: Option<(i64, Option<i64>, String)> = conn
        .query_row(
            "SELECT id, file_id, kind FROM pages WHERE name = ?1 COLLATE NOCASE",
            params![old_name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let (old_page_id, old_file_id, _old_kind) = old_row
        .ok_or_else(|| RenameError::NotFound(old_name.to_string()))?;

    // 2. Check collision.
    let target_row: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT id, file_id FROM pages WHERE name = ?1 COLLATE NOCASE AND id != ?2",
            params![new_name, old_page_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let merge_unresolved_id: Option<i64> = match target_row {
        Some((_, Some(_))) => {
            // Target is a backed page — reject.
            return Err(RenameError::TargetExists(new_name.to_string()));
        }
        Some((unresolved_id, None)) => Some(unresolved_id),
        None => None,
    };

    // 3. Resolve old file path.
    let old_file_path: Option<String> = if let Some(fid) = old_file_id {
        conn.query_row(
            "SELECT path FROM files WHERE id = ?1",
            params![fid],
            |r| r.get(0),
        ).ok()
    } else {
        None
    };

    // 4. Compute new file path (same directory as old).
    let new_file_path = old_file_path.as_ref().map(|p| {
        // Replace the filename component: `pages/OldName.md` → `pages/NewName.md`
        let slash = p.rfind('/');
        match slash {
            Some(idx) => format!("{}/{}.md", &p[..idx], new_name),
            None => format!("{}.md", new_name),
        }
    });

    // 5. Enumerate backlink occurrences BEFORE SQL mutation.
    let occurrences = enumerate_backlinks(conn, old_page_id, old_name, new_name)?;

    // 6. Build journal entry and append (fsync).
    let ops: Vec<JournalOp> = occurrences
        .iter()
        .map(|occ| JournalOp {
            file: occ.file_path.clone(),
            old_text: occ.old_text.clone(),
            new_text: occ.new_text.clone(),
            byte_offset: occ.byte_offset,
            byte_length: occ.byte_length,
            applied: false,
        })
        .collect();

    let journal_entry = JournalEntry {
        id: entry_id.clone(),
        started: "now".to_string(), // wall clock not critical for recovery
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        page_id: old_page_id,
        old_file: old_file_path.clone().unwrap_or_default(),
        new_file: new_file_path.clone().unwrap_or_default(),
        ops: ops.clone(),
        sql_committed: false,
        file_renamed: false,
    };
    journal.append(&journal_entry).map_err(RenameError::Io)?;

    // 7. SQL transaction.
    {
        let tx = conn.transaction()?;

        // Merge step FIRST: delete the unresolved target page before renaming the
        // source page, because `pages.name` has a UNIQUE(NOCASE) index — updating
        // source to the target name while the target row still exists causes a
        // UNIQUE constraint failure.
        if let Some(unresolved_id) = merge_unresolved_id {
            // Re-point refs from the unresolved page to old_page_id.
            // Use INSERT OR IGNORE because `refs(source_block, type, target_page)` is
            // a PRIMARY KEY — a block may already ref old_page_id with the same type.
            tx.execute(
                "INSERT OR IGNORE INTO refs (source_block, type, target_page) \
                 SELECT source_block, type, ?1 FROM refs WHERE target_page = ?2",
                params![old_page_id, unresolved_id],
            )?;
            // Delete the stale refs pointing to the now-merged unresolved page.
            tx.execute(
                "DELETE FROM refs WHERE target_page = ?1",
                params![unresolved_id],
            )?;
            // Delete the unresolved page row (no backing file — safe to cascade-delete).
            tx.execute("DELETE FROM pages WHERE id = ?1", params![unresolved_id])?;
        }

        // Now rename the page — UNIQUE constraint is clear.
        tx.execute(
            "UPDATE pages SET name = ?1 WHERE id = ?2",
            params![new_name, old_page_id],
        )?;

        // Update files.path if the page has a backing file.
        if let (Some(fid), Some(new_path)) = (old_file_id, &new_file_path) {
            tx.execute(
                "UPDATE files SET path = ?1 WHERE id = ?2",
                params![new_path, fid],
            )?;
        }

        tx.commit()?;
    }

    // 8. Mark SQL committed in journal.
    {
        let mut updated = journal_entry.clone();
        updated.sql_committed = true;
        journal.update_entries(&[updated]).map_err(|e| RenameError::Journal(e.to_string()))?;
    }

    // 9. Rewrite backlinks (if requested).
    let mut rewritten_count = 0u32;
    let mut warnings: Vec<String> = Vec::new();
    let mut updated_ops = ops.clone();

    if rewrite_backlinks {
        // Group ops by file path for efficient batch rewrites.
        let mut file_to_ops: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, op) in updated_ops.iter().enumerate() {
            file_to_ops.entry(op.file.clone()).or_default().push(i);
        }

        for (file_rel, op_indices) in &file_to_ops {
            let abs_path = RelativePath::from_storage_str(file_rel).to_filesystem(root);
            let mut file_bytes = match std::fs::read(&abs_path) {
                Ok(b) => b,
                Err(e) => {
                    let msg = format!("backlink rewrite: cannot read {:?}: {e}", abs_path);
                    tracing::warn!("{}", msg);
                    warnings.push(msg);
                    continue;
                }
            };

            // Sort ops in reverse byte order so earlier ops don't shift later ones.
            let mut sorted_indices = op_indices.clone();
            sorted_indices.sort_by(|&a, &b| {
                updated_ops[b].byte_offset.cmp(&updated_ops[a].byte_offset)
            });

            let mut file_changed = false;
            for &idx in &sorted_indices {
                let op = &updated_ops[idx];
                let start = op.byte_offset;
                let end = start + op.byte_length;
                if end > file_bytes.len() {
                    let msg = format!(
                        "backlink rewrite: offset {start}..{end} out of range for {:?}",
                        abs_path
                    );
                    warnings.push(msg);
                    continue;
                }
                let window = &file_bytes[start..end];
                if window == op.old_text.as_bytes() {
                    // Splice in new_text.
                    let new_text_bytes = op.new_text.as_bytes();
                    let mut new_bytes = Vec::with_capacity(
                        file_bytes.len() - op.byte_length + new_text_bytes.len(),
                    );
                    new_bytes.extend_from_slice(&file_bytes[..start]);
                    new_bytes.extend_from_slice(new_text_bytes);
                    new_bytes.extend_from_slice(&file_bytes[end..]);
                    file_bytes = new_bytes;
                    file_changed = true;
                    updated_ops[idx].applied = true;
                } else if window == op.new_text.as_bytes() {
                    // Already applied (idempotent).
                    updated_ops[idx].applied = true;
                } else {
                    let msg = format!(
                        "backlink rewrite: bytes at {start}..{end} in {:?} match neither old nor new text",
                        abs_path
                    );
                    warnings.push(msg);
                    updated_ops[idx].applied = true; // mark done to not block
                }
            }

            if file_changed {
                atomic_write_md(&abs_path, &file_bytes, self_writes)
                    .map_err(RenameError::Io)?;
                rewritten_count += 1;
                // Update journal after each file write.
                let mut je = journal_entry.clone();
                je.ops = updated_ops.clone();
                je.sql_committed = true;
                journal.update_entries(&[je]).map_err(|e| RenameError::Journal(e.to_string()))?;
            }
        }
    } else {
        // Mark all ops as applied=true (intentionally not rewriting).
        for op in &mut updated_ops {
            op.applied = true;
        }
    }

    // 10. Rename the file on disk.
    if let (Some(old_path), Some(new_path)) = (&old_file_path, &new_file_path) {
        let old_abs = RelativePath::from_storage_str(old_path).to_filesystem(root);
        let new_abs = RelativePath::from_storage_str(new_path).to_filesystem(root);
        if old_abs.exists() {
            rename_file_on_disk(&old_abs, &new_abs).map_err(RenameError::Io)?;
        }
    }

    // 11. Remove journal entry (fully applied).
    journal.remove_entry(&entry_id).map_err(|e| RenameError::Journal(e.to_string()))?;

    Ok(RenameResult {
        rewritten_count,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::{ReindexMode, reindex};
    use crate::storage::Db;
    use crate::sync::SelfWriteSet;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn merge_unresolved_works() {
        let tmp = tempfile::tempdir().unwrap();
        let pages_dir = tmp.path().join("pages");
        std::fs::create_dir_all(&pages_dir).unwrap();
        std::fs::write(pages_dir.join("Source.md"), "- Source page\n").unwrap();
        std::fs::write(pages_dir.join("Linker.md"), "- Links to [[Ghost]]\n").unwrap();

        let mut db = Db::open(tmp.path()).unwrap();
        reindex(&mut db, tmp.path(), ReindexMode::Full).unwrap();

        let self_writes = Arc::new(SelfWriteSet::new(Duration::from_secs(30)));
        let journal = Journal::open_for_root(tmp.path()).unwrap();

        let conn = db.conn_mut();
        let result = rename_page(conn, tmp.path(), &journal, &self_writes, "Source", "Ghost", false);
        assert!(result.is_ok(), "merge should succeed: {:?}", result);
    }
}
