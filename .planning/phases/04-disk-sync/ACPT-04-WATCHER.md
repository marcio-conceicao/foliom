# Phase 4 — Disk Sync: Manual Acceptance Checklist

**Plan:** 04-03
**Requirements covered:** SNC-03, SNC-04, SNC-06
**Automated coverage:** `phase-4-watcher-smoke` CI job (Linux inotify, VS Code-style rename, SSE within 1.5s); `cargo nextest run -p foliom-cli --test watcher_integration` (SNC-03/SNC-04 integration tests, all platforms)

## Purpose

Manual acceptance checklist for Phase 4 scenarios that cannot be fully automated in CI:
- Windows ReadDirectoryChangesW watcher behavior
- Syncthing-storm simulation (requires real Syncthing or scripted concurrent writes)
- VS Code atomic-rename save flow (verifies the rename-tracking in notify-debouncer-full)
- Conflict banner user interaction (requires a human to click UI elements)
- Self-write suppression end-to-end (verifies the DashMap suppression path in production)

---

## Acceptance Scenarios

### Scenario 1: VS Code atomic-rename save (Linux / macOS / Windows)

**Prerequisites:** `foliom serve <your-notes-folder>` running; browser tab open showing any page.

**Steps:**
1. Open any `.md` file from your notes folder in VS Code.
2. Edit one line (add a word, change punctuation).
3. Save with Cmd+S / Ctrl+S. VS Code performs a temp-file → atomic rename on all platforms.
4. Observe the Foliom browser tab within ~1s.

**Expected:**
- The page content in the browser updates to match the saved file within ~1s.
- No "File changed externally" banner appears (no block was in edit mode).
- The watcher-status pill in the Sidebar footer stays green throughout.
- Server log shows a `pages_updated` SSE event dispatched (check with `RUST_LOG=info`).

**Accept criteria:** Browser content matches saved file. No spurious banner. No duplicate events (check browser DevTools → Network → filter `/api/watch/events` → EventStream tab).

---

### Scenario 2: Syncthing-style concurrent write storm (Linux / macOS)

**Prerequisites:** `foliom serve <notes-folder>` running; a notes folder with at least 10 `.md` files.

**Steps:**
1. Open browser DevTools → Network → filter by `/api/watch/events` → click the SSE request → EventStream tab.
2. In a terminal, run:
   ```bash
   for i in $(seq 1 10); do
     echo "- Sync update $i at $(date)" > "<notes-folder>/pages/sync_storm_$i.md"
   done
   ```
3. Wait 2s, then count the `pages_updated` events in the EventStream tab.

**Expected:**
- Between 1 and 3 `pages_updated` SSE events (NOT 10 individual events).
- No UI freeze or runaway reindex log entries.
- Browser tab shows updated content for whichever page is currently open.

**Accept criteria:** Event count in the EventStream tab is 1–3 within 2s. Server is responsive after the storm (try navigating to another page — it should load normally).

---

### Scenario 3: Windows ReadDirectoryChangesW recovery

**Prerequisites:** Windows 11 native (not WSL2 — the notes folder must be on a Windows NTFS path, e.g. `C:\Users\<you>\notes`); `foliom.exe serve C:\Users\<you>\notes` running.

**Steps:**
1. Start `foliom.exe serve C:\Users\<you>\notes --port 7345` in a terminal. Open the browser.
2. Simulate a large external change by running a `git checkout` or batch-copy that modifies 50+ `.md` files simultaneously:
   ```powershell
   # Example: copy a large batch into the notes folder
   1..50 | ForEach-Object { "- bulk line $_" | Out-File "C:\Users\<you>\notes\pages\bulk_$_.md" }
   ```
3. Check the server terminal output for any error or recovery log lines.
4. Observe the browser.

**Expected:**
- Server does NOT crash. It continues running.
- Server log shows either `pages_updated` SSE events (if ReadDirectoryChangesW kept up) OR `"watcher error"` followed by a full `index_reset`.
- If `index_reset` fires: the browser tab shows an `index_reset` event in DevTools EventStream, then reloads the current page.
- After recovery, a subsequent single-file edit (open one `.md` file, add a line, save) triggers a `pages_updated` event within 1s — watcher re-armed successfully.

**Accept criteria:** No server crash. Browser remains functional. Single-file edits work normally after the storm.

---

### Scenario 4: Conflict banner — file changed externally while editing (SNC-06)

**Prerequisites:** `foliom serve` running; browser tab open on any page with at least one block.

**Steps:**
1. Click a block to enter edit mode (CM6 editor appears with a cursor).
2. Type a few characters but do NOT save (do not blur, do not press Enter).
3. While still in edit mode, edit the same `.md` file externally (VS Code, `echo`, `vim`, etc.).
4. Wait ~1s.
5. Observe the banner in the browser.
6. Try continuing to type in the editor — it should remain responsive.
7. Click the "Reload" button in the banner.

**Expected:**
- A non-blocking banner appears above the editor within ~1s of the external edit: "External edit detected — [Reload discards your edit]" (or equivalent wording from Phase 3 StaleConflict banner).
- Continuing to type in the editor is unblocked.
- Clicking Reload: the editor closes and the page reloads with the externally-written content.

**Accept criteria:** Banner appears within 1s. Editor remains interactive. Reload restores external content.

---

### Scenario 5: Self-write suppression — no banner after saving your own edit

**Prerequisites:** `foliom serve` running; browser tab open on any page.

**Steps:**
1. Click a block to enter edit mode.
2. Edit some text.
3. Blur the block (click outside) or press Enter to confirm the edit. Foliom calls `PUT /api/blocks/:id` and writes the `.md` file via `atomic_write_md`.
4. Wait 1s.

**Expected:**
- NO "File changed externally" banner appears.
- The page does NOT reload unexpectedly.
- The watcher server log shows "self-write suppressed" (or no event logged for this path) rather than a `pages_updated` dispatch.

**Accept criteria:** Banner does not appear after own-write. Page stays in view. No watcher event visible in DevTools EventStream for this edit.

---

## Sign-Off Table

| Scenario | Platform | Result | Date | Tester |
|----------|----------|--------|------|--------|
| 1. VS Code atomic save | Linux / WSL2 | [ ] | | |
| 1. VS Code atomic save | macOS | [ ] | | |
| 1. VS Code atomic save | Windows 11 native | [ ] | | |
| 2. Syncthing write storm | Linux / WSL2 | [ ] | | |
| 2. Syncthing write storm | macOS | [ ] | | |
| 3. Windows RDC recovery | Windows 11 native | [ ] | | |
| 4. Conflict banner | Linux / WSL2 | [ ] | | |
| 4. Conflict banner | Windows 11 native | [ ] | | |
| 5. Self-write suppression | Linux / WSL2 | [ ] | | |
| 5. Self-write suppression | Windows 11 native | [ ] | | |

---

## Notes

### WSL2 inotify caveat

inotify works for files on the WSL2 filesystem (`/home/...`, `/tmp/...`). For notes on the Windows filesystem (`/mnt/c/...`), `notify` may fall back to polling or miss rapid-succession events. This is a known Linux kernel limitation for 9P mounts.

**Recommendation:** Keep your notes folder on the WSL2 filesystem (`/home/<user>/notes/`) rather than `/mnt/c/` for best watcher responsiveness. The Phase 4 CI smoke tests run on ubuntu-latest (real Linux kernel, no WSL2), so CI results do not reflect the `/mnt/c` scenario.

### Windows watcher re-arm

After a `ReadDirectoryChangesW` error, `spawn_watcher` calls `debouncer.watcher().watch()` again. Check the server log for a "watcher error — re-arming watcher on root" message (RUST_LOG=debug). If the re-arm fails (e.g., due to path permissions), the server logs the error and continues running without watcher coverage — the user must restart `foliom serve` in that case.

### SSE latency baseline

Based on Phase 4 integration tests (`external_write_detected` test, `watcher_integration.rs`):
- Typical latency on Linux: debounce window (300ms) + coalescing tick (300ms) = ~600ms
- CI gate: `pages_updated` event arrives within 1.5s (includes server warmup + curl startup overhead)
- See `PERF-BASELINE.md` (Phase 2) for cold-start and RSS baselines.

### How to observe SSE events

While `foliom serve` is running:
```bash
# Terminal: stream raw SSE
curl -N http://127.0.0.1:7345/api/watch/events

# Browser: DevTools → Network → XHR/Fetch or All → filter by /api/watch/events
# Click the request → EventStream tab (Chrome) or Response tab (Firefox)
```
