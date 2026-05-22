# ACPT-05 Portability Manual Verification Checklist

**Requirement:** ACPT-05 — `.md` files written by Foliom must open without warnings or visible
diffs in Obsidian and VS Code, on a corpus exercised by real edits across Phase 3.

**Automated portion:** CI job `phase-3-acpt-05` covers byte invariants, Foliom-metadata grep,
and ACPT-01 corpus replay. See `.github/workflows/ci.yml`.

**Manual portion:** This document. Execute once during `/gsd-verify-work` for Phase 3.
The automated portion does NOT replace this checklist — Obsidian/VS Code rendering bugs
(e.g. a stray blank line changing heading levels, a property block being rendered as a
code fence) are invisible to byte-level checks but visible to a human.

---

## Step 1 — Generate the post-edit corpus

Run the acceptance test with the keep-tempdir flag so the post-edit files are accessible:

```bash
ACPT05_KEEP_TEMPDIR=1 cargo test -p foliom-cli --test portability_acpt_05 -- --nocapture
```

The post-edit corpus will be written to `/tmp/foliom-acpt05/`.

If the script is available:

```bash
ACPT05_KEEP_TEMPDIR=1 bash scripts/acpt05_inspect.sh
```

---

## Step 2 — Obsidian verification

1. Open Obsidian.
2. Open `/tmp/foliom-acpt05/` as a new vault (File → Open Folder as Vault).
3. For each file in the table below, open it and check:
   - **No popup warnings** on open (e.g. "Cannot parse this file", "Plugin error").
   - **No errors in the developer console** (Ctrl+Shift+I → Console tab — look for red errors
     mentioning file names).
   - **Preview mode** (Ctrl+E) shows the expected content: bullets render as a list, code fences
     render as code blocks, properties like `id:: ...` appear as properties or as text
     (NOT as broken HTML).
   - **No visible content differences** between what Foliom renders and what Obsidian renders
     for the same raw markdown.

### Failure criteria — Obsidian

A result is a **FAIL** if any of the following appear:
- A popup dialog mentioning file parse errors.
- Console errors attributing failures to a specific file.
- A bullet block rendered as a paragraph (indentation lost).
- A code fence block rendered as inline code or raw backtick text.
- A `:LOGBOOK:` / `:END:` block rendered as visible markdown (should be invisible or a custom
  block depending on the Logseq plugin, but NOT broken syntax).
- A `key:: value` property rendered as `key:: value` visible text in preview (should be a
  property row or at minimum the text should be the same characters verbatim).

---

## Step 3 — VS Code verification

1. Open VS Code.
2. Open `/tmp/foliom-acpt05/` as a folder (File → Open Folder).
3. For each file in the table below, open it and check:
   - **No encoding banner** ("File contains invalid UTF-8 characters", etc.).
   - **No line-ending banner** ("Mixed line endings detected: CRLF/LF").
   - **Markdown preview** (Ctrl+Shift+V) shows the same content as Foliom's block renderer:
     bullets, code fences, and properties are intact.

### Failure criteria — VS Code

A result is a **FAIL** if any of the following appear:
- Encoding warning banner at the top of the editor.
- Line-ending warning banner.
- A file that opens as garbled characters (would indicate a non-UTF-8 write).

---

## Verification Table

Fill in this table during `/gsd-verify-work`. Mark ✓ (pass) or ✗ (fail + describe below).

| File | Obsidian opens? | Obsidian preview ok? | VS Code encoding banner? | VS Code preview ok? |
|------|-----------------|----------------------|--------------------------|---------------------|
| `pages/01-simple-bullets.md` (edited) | | | | |
| `pages/page_with_code_drawer_props.md` (curated) | | | | |
| `journals/2026_05_22.md` (curated journal) | | | | |
| `pages/ACPT05BrandNewPage.md` (created during test) | | | | |
| `pages/ACPT05Renamed.md` (renamed from ACPT05Target) | | | | |
| `pages/02-fence-in-bullet.md` (unchanged, control) | | | | |
| `pages/03-block-properties.md` (unchanged, control) | | | | |
| `pages/04-logbook-drawer.md` (unchanged, control) | | | | |

**Control files** (last 3 rows) are files that the test did NOT edit. They should be byte-identical
to the pre-edit fixtures and must pass. If a control file fails, the bug is in corpus setup
(copy_dir_all), not in Foliom's write path.

---

## Failure follow-up

If any row is ✗:

1. Note the file name, the tool (Obsidian or VS Code), and what you saw.
2. Run `xxd /tmp/foliom-acpt05/pages/<file>.md | head -5` to inspect byte header.
3. Run `file /tmp/foliom-acpt05/pages/<file>.md` to confirm encoding.
4. Run `cat -A /tmp/foliom-acpt05/pages/<file>.md | head -10` to spot CRLF (`^M$`).
5. Open an issue or re-plan the affected mutation handler.

---

## Automated CI Coverage

The `phase-3-acpt-05` CI job in `.github/workflows/ci.yml` runs:

```
cargo test -p foliom-cli --test portability_acpt_05 -- --nocapture
```

This covers (automatically, without human intervention):
- No `\r\n` in any post-edit `.md` file.
- No UTF-8 BOM in any post-edit `.md` file.
- All `.md` files are valid UTF-8.
- Zero new occurrences of `id::`, `((`, `<!-- foliom`, `.foliom-`, `foliom_uuid` per file.
- `pulldown_cmark::Parser::new` does not panic on any post-edit file.
- ACPT-01 round-trip: `segment(bytes)` → slice-concat → byte-equal for every file.

The CI job does NOT cover:
- Obsidian-specific rendering (no headless Obsidian in CI).
- VS Code-specific rendering or banner display.
- Visual equivalence between Foliom's renderer and an external tool's renderer.

These three items are covered by this manual checklist.

---

## Verifier Sign-off

After completing the table:

1. If all rows are ✓: add a "Manual ACPT-05 Verification" section to
   `.planning/phases/03-outliner-editor/03-07-SUMMARY.md` confirming the pass.
2. If any row is ✗: file a deviation note in the SUMMARY and open a follow-up plan.

**Verifier:** _(fill in name/date when executed)_
**Result:** PENDING (awaiting /gsd-verify-work execution)
