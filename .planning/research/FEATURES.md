# Feature Research

**Domain:** Local-first markdown outliner / PKM (ex-Logseq power user)
**Researched:** 2026-05-21
**Confidence:** MEDIUM-HIGH (based on training-data knowledge of Logseq, Roam, Workflowy, Dynalist, Obsidian, Tana, Athens; WebSearch unavailable so no live community validation — recommend the user sanity-check the "differentiator vs nice-to-have" calls against their own daily workflow)

> **Reading note for downstream consumer (REQUIREMENTS.md):** the PRD already covers a solid baseline. This document focuses on **(a) gaps the PRD does not mention but a Logseq refugee will notice on day 1**, and **(b) explicit anti-features**, because the user's stated risk is scope creep, not scope omission. Every gap is tagged with a proposed RF-number slot for easy lifting into the PRD.

---

## Feature Landscape

### Table Stakes (Users Expect These — Missing = Day-1 Friction)

These are features a Logseq user uses without thinking. If absent on M2 release, the user will bounce back to Logseq within a week.

| Feature | Why Expected | Complexity | PRD Status |
|---------|--------------|------------|------------|
| Block-by-block editor, one block in edit | Core Logseq/Roam UX | M | RF-11 covered |
| Tab/Shift-Tab indent/outdent | Universal outliner shortcut | S | RF-15 covered |
| Enter = sibling block, Shift+Enter = newline in block | Roam/Logseq convention | S | RF-14 covered |
| Backspace at block start = merge with previous | Standard | S | RF-15 covered |
| Arrow keys cross block boundaries | Standard | S | RF-15 covered |
| `[[page]]` and `#tag` recognition | Core PKM | M | RF-20 covered |
| Backlinks panel on every page | Core PKM | M | RF-22 covered |
| Full-text search | Baseline | M | RF-30 covered |
| External-edit pickup (watcher) | Local-first promise | M | RF-40 covered |
| **Block folding (collapse/expand children)** | Every outliner has it; without it, long pages are unusable | M | **GAP — propose RF-17** |
| **Zoom into block** (focus mode: click bullet to make it the page root) | Defining outliner feature since Workflowy 2010; Logseq has it; backbone of "outline" mental model | M | **GAP — propose RF-18** |
| **Bullet click navigation** (click bullet dot → zoom; alt-click → sidebar open) | Logseq/Roam standard interaction | S | **GAP — propose RF-19** |
| **Autocomplete for `[[page]]`** (typeahead as user types `[[`) | Without this, linking is unusable at scale (you forget exact page names) | M | **GAP — propose RF-24** |
| **Autocomplete for `#tag`** | Same as above for tags | M | **GAP — propose RF-25** |
| **Page creation on link click for non-existent pages** | PRD mentions "create page inexistente ao primeiro uso, se aplicável" — needs to be definitive | S | RF-23 partially covered (make non-conditional) |
| **Journal / daily notes navigation** (today, prev day, next day, calendar picker) | Logseq's daily-notes loop is the #1 workflow; PRD mentions journal pages but not navigation UX | M | **GAP — propose RF-26** |
| **"Today" landing page** on app open | Logseq opens to today's journal — this is the muscle memory | S | **GAP — propose RF-27** |
| **Sidebar with page list / recents / favorites** | All outliners have it; without it the app feels like a single-page viewer | M | **GAP — propose RF-28** |
| **Right-side pane / "open in sidebar"** (alt-click link opens page in side pane without losing context) | Logseq/Roam staple for cross-referencing while editing | M-L | **GAP — propose RF-29** (acceptable to defer to v1.1) |
| **Code fence syntax highlighting in rendered blocks** | Without it, technical notes look broken; PRD §12.5 marks as open | M | Open decision §12.5 — **recommend: YES for M1** |
| **GFM tables rendered correctly** | Listed in §12.5 open decisions | M | **Recommend: YES for M1** (read), edit by raw OK |
| **Block context menu** (delete block, copy block as markdown, move to page) | Standard right-click expectation | M | **GAP — propose RF-31** |
| **Undo/redo** (at least per-block, ideally cross-block) | Non-negotiable for an editor | M-L | **GAP — propose RF-32 (CRITICAL)** |
| **Copy/cut/paste blocks** (preserve hierarchy when pasting indented markdown) | Power-user must-have for restructuring notes | M | **GAP — propose RF-33** |
| **Drag-and-drop block reordering** | Logseq has it; without it, restructuring requires cut-paste-indent dance | L | **GAP — propose RF-34** (acceptable to ship M2 without, but flag) |
| **Visible indentation guides** (vertical lines connecting parent-child) | Visual cue all outliners provide | S | **GAP — propose RF-35** |
| **Theme: light + dark mode** | 2026 baseline | S | **GAP — propose RF-36** |
| **Keyboard shortcut to focus search** (`Ctrl/Cmd+K` or `Ctrl+Shift+F`) | Universal | S | Implicit in RF-30 — make explicit |
| **Page rename** (and update all `[[backlinks]]` that point to it) | Without backlink rewrite, renaming corrupts the graph | L | **GAP — propose RF-37 (CRITICAL)** |
| **Block properties displayed as opaque pills, not raw `key:: value`** | PRD says "preserve opaque" but doesn't say how to render them — raw `key:: value` lines in render mode look ugly | S-M | **GAP — clarify RF-54 rendering** |

---

### Differentiators (Why Switch From Logseq Specifically)

The user already has Logseq. They will not switch for parity. These are the wedges.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Sub-second cold start on 5k+ note graph** | The whole reason this project exists (PROJECT.md Core Value) | L | Already RNF-01 — this IS the differentiator |
| **Memory footprint <300MB at rest with full graph indexed** | Logseq commonly sits at 1-2GB | L | RNF-02 |
| **Lazy block loading — only visible blocks in RAM** | Enables editing huge journals without lag | M | Implicit in architecture |
| **Zero metadata pollution in `.md` files** (no `id::` injected by app) | Logseq's file-based mode injects `id::` properties everywhere over time; user explicitly wants clean files | S (just don't write them) | Already a fixed decision §5.6 |
| **Honest Logseq-compatibility mode** (open existing graph, don't corrupt on first save) | Migration friction is the #1 reason users don't try alternatives | M | RF-50 through RF-56 cover this — already strong |
| **Instant page open** (no "loading graph" spinner) | Logseq's worst UX moment | M | Follows from lazy loading |
| **Resilient watcher** that survives Syncthing rename storms | Logseq's watcher panics on bulk file changes from sync | M | RF-40 + needs explicit "bulk change debounce" |
| **Plain-text-friendly: app stays usable while editing same file in VS Code** | True local-first — most "local-first" apps lock the file | M | Watcher must handle external edits during local edit session |
| **Round-trip stability: open file → save with no edits → byte-identical output** | Critical for git workflows; Logseq fails this | M-L | Implicit in RF-13/RF-41 — should be explicit acceptance test |
| **Per-folder graph (point at any folder, no special init)** | Logseq requires graph initialization; pointing at any md folder should "just work" | S | Aligns with RF-01 |
| **Search results show block in context with parent crumbs** | Logseq search is flat; seeing block-in-tree is much more useful | M | Refinement of RF-31 |
| **Predictable, documented `.md` output format** | User can write their own scripts against the files | S (docs) | Differentiator vs Obsidian's quirks |

---

### Anti-Features (Do NOT Build — Explicit Refusals)

These will be requested. Say no with reasoning ready.

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Block-level references `((uuid))`** | Roam/Logseq users used to transclusion | Requires writing IDs into `.md` files → breaks the canonical-files promise (PROJECT.md §5.6) | Document-level `[[page]]` links + quote/copy |
| **WYSIWYG editor with HTML→MD round-trip** | "It would be smoother" | Lossy round-trip; conflicts with "raw markdown is source of truth" (RF-13) | Block-level swap render↔raw (already designed) |
| **Plugin system** | Logseq has a thriving plugin ecosystem | 5-10x scope; security; perf claims become unverifiable | Document file format → users script externally |
| **Real-time collaboration / multi-user** | Notion envy | Requires CRDT, server, auth, conflict resolution — kills "local-first single-user" simplicity | Delegate to Syncthing/git |
| **Mobile app** | "Use it on phone" | Different input model (no keyboard), Tauri mobile is immature, doubles QA | Read-only PWA later, maybe |
| **Built-in sync service** | "Just works like iCloud" | Server, account, billing, encryption — kills the project | Syncthing/Dropbox/git (RF doc this prominently) |
| **Whiteboards / Excalidraw / canvas** | Logseq/Obsidian both have it | Different data model (not markdown); huge scope | Ignore `.excalidraw` files (RF-53 already does) |
| **PDF annotation / highlights as blocks** | Logseq feature | Requires PDF renderer, asset storage, sidecar files | Out of v1 |
| **Audio/video recording in-app** | Some PKM tools have it | Asset storage, encoding, mobile-adjacent | Out |
| **Spaced repetition / flashcards (`{{cloze}}`)** | Logseq `/cards` plugin | SR algorithm + scheduler + review UI = whole app | Out; render `{{cloze}}` as literal text (PRD §12.10) |
| **Encryption at rest** | Privacy ask | Files are user's filesystem — let OS/disk encryption handle it | Document: use FileVault/LUKS/BitLocker |
| **Account / cloud features** | Modern app expectation | Violates local-first | None — that's the point |
| **Block-level versioning / history in-app** | "Undo across sessions" | Git already does this; reimplementing is huge | Document git workflow |
| **AI chat over your notes** | Trendy in 2026 | Massive scope; either local LLM (perf nightmare) or cloud (privacy violation) | Defer to v2+ explicit milestone |
| **Custom CSS / themes marketplace** | Obsidian envy | UX maintenance burden | Single light/dark theme well-done |
| **Multiple graphs / workspaces** | Logseq feature | Adds context-switching UI; user has one notes folder | Out of v1 — relaunch app pointed at different folder |
| **Embeds (`{{video url}}`, `{{tweet}}`, `{{youtube}}`)** | Logseq macros | Each is a mini-renderer; security (iframes) | Render as literal text (PRD §12.10 leans this way) |
| **Org-mode support** | Some users have mixed graphs | Different parser, different conventions | Markdown-only per PRD §3.2 |
| **Full text editing of multiple blocks at once** ("show all raw") | Bulk-edit ask | Conflicts with one-block-edit invariant (RF-11) | Edit underlying `.md` in external editor — supported by design |
| **Real-time linting / spellcheck inline** | Editor expectation | Browser-native spellcheck on the textarea is free; full linter is scope | Rely on textarea spellcheck |

---

### Gaps in PRD (Not Listed, Should Be)

Consolidated list of "things a Logseq user will look for on day 1 and not find in the PRD". Numbered as proposed PRD additions:

| Proposed RF | Feature | Category | Complexity | Why |
|-------------|---------|----------|------------|-----|
| **RF-17** | Block folding (collapse/expand children) | Table stakes | M | Long pages unusable without this |
| **RF-18** | Zoom into block (block as page root) | Table stakes | M | Defining outliner feature |
| **RF-19** | Bullet-dot click = zoom; alt-click = sidebar | Table stakes | S | Standard interaction |
| **RF-24** | `[[page]]` autocomplete typeahead | Table stakes | M | Linking unusable without it |
| **RF-25** | `#tag` autocomplete typeahead | Table stakes | M | Same |
| **RF-26** | Daily-journal navigation (today, ±1 day, calendar) | Table stakes | M | Logseq's primary loop |
| **RF-27** | App opens to today's journal by default | Table stakes | S | Muscle memory |
| **RF-28** | Sidebar: page list + recents + favorites | Table stakes | M | Navigation backbone |
| **RF-29** | Right pane "open in sidebar" for links | Differentiator polish | M-L | Defer to v1.1 OK |
| **RF-31** | Block context menu (copy as md, delete, move) | Table stakes | M | Standard expectation |
| **RF-32** | Undo/redo (per-block min; cross-block ideal) | **CRITICAL table stakes** | M-L | Non-negotiable |
| **RF-33** | Copy/cut/paste blocks with hierarchy | Table stakes | M | Power-user restructuring |
| **RF-34** | Drag-and-drop block reordering | Table stakes | L | Can defer to M2.5 |
| **RF-35** | Visible indentation guides | Table stakes | S | Visual cue |
| **RF-36** | Light + dark theme toggle | Table stakes | S | 2026 baseline |
| **RF-37** | Page rename rewrites `[[backlinks]]` | **CRITICAL table stakes** | L | Otherwise rename corrupts graph |
| **RF-38** | Slash-command menu (`/`) for block actions | Differentiator | M-L | Logseq/Notion have it; not strictly required but UX boost. **Acceptable to skip in v1.** |
| **RF-39** | Keyboard shortcut palette (`Ctrl+Shift+P`) | Differentiator | M | Power-user feature |
| **RF-42** | Recently-modified pages view | Table stakes | S | Easy follow-on from index |
| **RF-43** | Render block-properties (`key:: value`) as pills, not raw | Polish | S-M | Clarify RF-54 |
| **RF-44** | Round-trip stability test (open → no-op save → byte-identical) | Acceptance criterion | M | Critical for git users |
| **RF-45** | Graceful handling of bulk file changes (Syncthing storm) | Differentiator | M | Logseq fails this |
| **RF-46** | Bidirectional cursor preservation across render↔edit | Polish | S | §12.2 open decision — needs answer |
| **RF-47** | Explicit "create page" flow when clicking unknown `[[link]]` | Table stakes | S | RF-23 needs definitive answer |
| **RF-48** | Search keyboard shortcut (Cmd/Ctrl+K) explicit | Table stakes | S | Make implicit explicit |
| **RF-49** | TODO/DONE rendering as checkbox if user opts in | Differentiator (low-risk) | M | §12.9 open decision. Suggest: render as checkbox in M2.1, do not implement agenda/scheduling in v1 |

**Explicit graph view non-recommendation:** The user asked about "page graph view" (the spider-web visualization). **Anti-feature for v1.** It is visually impressive but rarely used productively, requires a graph rendering lib (Sigma/D3) and a layout algorithm, and the user did not list it as a Logseq feature they miss. Defer to v2 if ever.

---

## Feature Dependencies

```
RF-37 Page rename with backlink rewrite
    └─requires──> RF-22 backlinks index (already in PRD)
    └─requires──> RF-41 markdown re-serialization (already in PRD)
    └─enhances──> RF-23 navigation

RF-24/25 Autocomplete [[page]] / #tag
    └─requires──> Pages index (RF-02)
    └─requires──> Tags index (RF-02)
    └─enhances──> RF-20 linking

RF-17 Block folding
    └─requires──> Block tree structure (RF-10)
    └─enhances──> RF-18 zoom (both manipulate visible block subset)

RF-18 Zoom into block
    └─requires──> Stable block identity within page (derive from path-index, NOT uuid in file)
    └─tension─with──> "no IDs in .md" decision (§5.6) — zoom state must be ephemeral (URL fragment, not file)

RF-32 Undo/redo
    └─requires──> Edit transaction log (in-memory; lost on reload acceptable for v1)
    └─should─precede──> RF-34 drag-drop (drag without undo = data loss risk)

RF-26 Daily journal navigation
    └─requires──> RF-55 journal title format (already in PRD)
    └─requires──> RF-27 today landing
    └─enhances──> Linking pattern (journal entries link to topical pages)

RF-29 Right-pane "open in sidebar"
    └─requires──> RF-28 sidebar layout
    └─requires──> Page rendering decoupled from main viewport

Code-fence syntax highlighting (§12.5)
    └─requires──> Highlight library (Shiki / highlight.js / Prism)
    └─tension──> bundle size (Shiki is heavy) — Prism or starry-night recommended for footprint

RF-44 Round-trip stability
    └─requires──> RF-41 + RF-50/RF-51/RF-54 (TAB indent, multiline continuation, block properties opaque)
    └─is──> the test that proves Logseq-compatibility (RF-50..56) works
```

### Key Tension Points

- **Zoom (RF-18) vs no-IDs-in-files (§5.6):** Resolved by making zoom state ephemeral (URL like `/page/foo#block=3.1.2` where `3.1.2` is the indent path, not a UUID). Acceptable but fragile if blocks reorder — document as known limitation.
- **Drag-drop (RF-34) vs undo (RF-32):** Ship undo first or risk data loss complaints.
- **Render highlighting vs bundle size:** Choose lightweight highlighter (Prism / starry-night / lazy-loaded Shiki).
- **Autocomplete (RF-24/25) needs the index always-warm:** Aligns with PRD's "index everything, lazy-load content" — autocomplete queries the always-loaded names index, not content. Cheap.

---

## MVP Definition

### M1 — Read-Only UI (minimum useful)

Goal: open Logseq graph, browse it, find things. **No editing.** This alone is already useful — it's a fast, read-only viewer.

**Must-have for M1:**
- [ ] RF-01..04: indexing, incremental reindex
- [ ] RF-10..13 *read path only*: render blocks as tree (no edit state machine yet — read mode is the default)
- [ ] RF-17: **block folding** (read-only viewer is unusable without it)
- [ ] RF-18: **zoom into block** (URL-addressable)
- [ ] RF-19: bullet-click interactions
- [ ] RF-20..23: link/tag recognition, navigation, backlinks
- [ ] RF-26: journal navigation (today / prev / next / calendar)
- [ ] RF-27: app opens on today's journal
- [ ] RF-28: sidebar with page list + recents
- [ ] RF-30..31: FTS search with context
- [ ] RF-35: indentation guides
- [ ] RF-36: light + dark theme
- [ ] RF-48: Cmd/Ctrl+K search shortcut
- [ ] RF-50..56: Logseq base compatibility on **read** (TAB indent, multiline continuation, `#[[composite tag]]`, journal title format, ignored folders, opaque properties displayed as pills)
- [ ] Code-fence syntax highlighting (decide §12.5)
- [ ] GFM tables render

**Defer to M2 (editor milestone):** all write paths.
**Defer to v1.x:** RF-29 right pane, favorites, RF-39 command palette.

**M1 is shippable on its own** as a "fast Logseq viewer" — useful even before edit lands. Encourages early dogfood.

### M2 — Editor (minimum to replace Logseq for daily use)

**Must-have for M2:**
- [ ] RF-11..16: full edit state machine (render↔raw swap, Tab/Shift+Tab/Enter/Shift+Enter/Backspace/arrows)
- [ ] RF-24, RF-25: autocomplete for `[[page]]` and `#tag` (without these, linking is painful enough that user reverts to Logseq)
- [ ] RF-31: block context menu (at minimum: delete block, copy as markdown)
- [ ] RF-32: **undo/redo** (non-negotiable)
- [ ] RF-33: copy/cut/paste blocks preserving hierarchy
- [ ] RF-37: **page rename with backlink rewrite** (without this, rename is forbidden = bad UX)
- [ ] RF-40..41: watcher + serialize tree → markdown
- [ ] RF-44: round-trip stability test (CI gate, not user-facing)
- [ ] RF-47: explicit page-creation flow on unknown link click
- [ ] RF-46: cursor preservation policy decided and implemented (even if "always end of block" — answer §12.2)

**Acceptable to defer to M2.1:**
- [ ] RF-34: drag-and-drop reordering (cut/paste works as substitute)
- [ ] RF-49: TODO/DONE checkbox rendering
- [ ] RF-43: pill rendering for block properties (raw display acceptable but ugly)

**Hard out of M2:**
- Slash commands (RF-38), command palette (RF-39), right-pane (RF-29), graph view, plugins, mobile, anything in anti-features.

### v1.x (post-M2, before "official 1.0")

- [ ] RF-29: right-pane "open in sidebar"
- [ ] RF-34: drag-and-drop block reordering
- [ ] RF-38: slash-command menu
- [ ] RF-39: command palette (Cmd+Shift+P)
- [ ] RF-42: recently-modified view, favorites pinning
- [ ] RF-49: TODO state cycling
- [ ] M3: watcher hardening (already a milestone) + Syncthing-storm survival
- [ ] M4: Tauri desktop packaging

### Future (v2+ — explicit non-commitments)

- Slash command extensibility, queries (`{{query}}`), graph view, mobile read-only PWA, AI features. List them so they have a known "no for now" home.

---

## Feature Prioritization Matrix

Only PRD-gap features listed (the original PRD RFs are already prioritized into milestones by §10).

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| RF-17 Block folding | HIGH | MEDIUM | **P1 (M1)** |
| RF-18 Zoom into block | HIGH | MEDIUM | **P1 (M1)** |
| RF-19 Bullet-dot interactions | MEDIUM | LOW | **P1 (M1)** |
| RF-24 [[page]] autocomplete | HIGH | MEDIUM | **P1 (M2)** |
| RF-25 #tag autocomplete | HIGH | MEDIUM | **P1 (M2)** |
| RF-26 Journal navigation | HIGH | MEDIUM | **P1 (M1)** |
| RF-27 Open to today | HIGH | LOW | **P1 (M1)** |
| RF-28 Sidebar | HIGH | MEDIUM | **P1 (M1)** |
| RF-31 Block context menu | MEDIUM | MEDIUM | **P1 (M2)** |
| RF-32 Undo/redo | HIGH (CRITICAL) | MEDIUM-HIGH | **P1 (M2)** |
| RF-33 Block copy/paste with hierarchy | HIGH | MEDIUM | **P1 (M2)** |
| RF-34 Drag-and-drop | MEDIUM | HIGH | P2 (M2.1) |
| RF-35 Indent guides | MEDIUM | LOW | **P1 (M1)** |
| RF-36 Dark mode | HIGH | LOW | **P1 (M1)** |
| RF-37 Page rename + rewrite backlinks | HIGH (CRITICAL) | HIGH | **P1 (M2)** |
| RF-38 Slash commands | MEDIUM | MEDIUM-HIGH | P2 (v1.x) |
| RF-39 Command palette | MEDIUM | MEDIUM | P2 (v1.x) |
| RF-29 Right-pane | MEDIUM | MEDIUM-HIGH | P2 (v1.x) |
| RF-43 Property pills | LOW | LOW-MEDIUM | P2 |
| RF-44 Round-trip stability test | HIGH (infra) | MEDIUM | **P1 (M2)** |
| RF-45 Bulk-change resilience | MEDIUM | MEDIUM | P2 (M3) |
| RF-46 Cursor preservation | MEDIUM | LOW | **P1 (M2)** — answer §12.2 |
| RF-47 Page creation flow | MEDIUM | LOW | **P1 (M2)** |
| RF-48 Search shortcut | HIGH | LOW | **P1 (M1)** |
| RF-49 TODO/DONE checkbox | MEDIUM | MEDIUM | P2 (v1.x) — answer §12.9 |
| Graph view | LOW | HIGH | **P3 — recommend never for v1** |

**Priority key:**
- **P1**: Required for the milestone listed in parentheses
- **P2**: Should ship in v1.x post-M2
- **P3**: Defer to v2 or never

---

## Competitor Feature Analysis

| Feature | Logseq | Roam | Workflowy | Obsidian | Tana | Foliom (recommend) |
|---------|--------|------|-----------|----------|------|--------------------|
| Local `.md` files canonical | Yes (file mode) | No (DB) | No (cloud) | Yes | No (cloud) | **Yes** (core diff) |
| Block-by-block editor | Yes | Yes | Yes | No (full doc) | Yes | **Yes** |
| Zoom into block | Yes | Yes | Yes (defining) | No | Yes | **Yes (M1)** |
| Block folding | Yes | Yes | Yes | Limited | Yes | **Yes (M1)** |
| Daily journal | Yes | Yes | No | Plugin | Yes | **Yes (M1)** |
| `[[page]]` links | Yes | Yes | No (mirrors) | Yes | Yes | **Yes** |
| `#tag` first-class | Yes | Yes | Yes | Yes | Yes (supertags) | **Yes** |
| Backlinks | Yes | Yes | No | Yes | Yes | **Yes** |
| Block refs `((uuid))` | Yes (pollutes files) | Yes | "Mirrors" | Block IDs (opt-in) | Yes | **NO** (canonical decision) |
| Drag-drop blocks | Yes | Yes | Yes | Limited | Yes | **Yes (M2.1)** |
| Slash commands | Yes | Yes | No | Plugin | Yes | **Defer (v1.x)** |
| Plugins | Yes (huge) | Yes | No | Yes (huge) | No | **NO** (anti-feature) |
| Graph view | Yes | Yes | No | Yes | No | **NO** (low value, high cost) |
| Mobile | Yes | Yes | Yes | Yes | Yes | **NO v1** |
| Real-time sync | Yes (paid) | Yes | Yes | Plugin (paid) | Yes | **NO** (Syncthing) |
| Cold-start speed on 5k notes | **Slow** | N/A (web) | Fast (cloud) | Fast | N/A (web) | **Sub-second target — THE WEDGE** |
| RAM at rest, 5k notes | **~1-2 GB** | N/A | N/A | ~300-500 MB | N/A | **<300 MB target** |
| Round-trip clean .md | **Pollutes (id::)** | N/A | N/A | Mostly clean | N/A | **Byte-stable target** |

**Strategic takeaway:** Foliom's differentiation is **not features** — it is **performance + file purity + Logseq-compat for migration**. Feature parity with Logseq on the *outliner-essential subset* is the price of admission; speed and clean files are the product. Therefore the feature scope MUST stay small (PRD scope + the table-stakes gaps listed here). Every feature past that bar dilutes the performance budget.

---

## Sources & Confidence Notes

- Training-data knowledge of Logseq (file-based mode), Roam Research, Workflowy, Dynalist, Obsidian, Tana, Athens Research, RemNote. **HIGH confidence** on which features each tool has.
- Logseq performance pain points (slow cold start, high RAM, file pollution with `id::`): **HIGH confidence** — widely-reported across HN, GitHub issues, r/logseq through 2024-2025; user's PROJECT.md confirms first-hand.
- "Drag-drop deferrable" call: **MEDIUM confidence** — some users consider it non-negotiable. Cut/paste-blocks (RF-33) is the safety net.
- "Graph view = anti-feature" call: **MEDIUM confidence** — visually appealing, surveyed users say "rarely used productively" but some love it. Defensible to defer rather than refuse permanently.
- "Slash commands defer to v1.x" call: **MEDIUM confidence** — heavy Logseq users rely on `/template`, `/today`, etc. If user surfaces specific commands they use daily, promote selected ones to M2.
- WebSearch was not available in this session; **recommend the user spot-check** the differentiator list against their own daily Logseq workflow before locking REQUIREMENTS.md. Specifically: do you use slash commands daily? Do you use the graph view? Do you use `((block refs))` (already declared out of scope but worth confirming you can live without)?

---

## Quality Gate Checklist

- [x] Categories are clear (table stakes vs differentiators vs anti-features) — three distinct sections
- [x] Complexity noted for each feature (S/M/L equivalent: LOW/MEDIUM/HIGH)
- [x] Dependencies between features identified — see Dependencies section
- [x] PRD gaps explicitly called out — see "Gaps in PRD" section with proposed RF numbers
