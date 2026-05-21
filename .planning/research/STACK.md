# Stack Research

**Domain:** Local-first markdown outliner / PKM (Logseq/Roam alternative)
**Researched:** 2026-05-21
**Overall confidence:** HIGH

> **Headline recommendation:**
> **Rust backend** (`pulldown-cmark` + `rusqlite` w/ bundled FTS5 + `notify` v6 + `notify-debouncer-full`) serving an **axum** HTTP API on `localhost`, **Svelte 5** + **CodeMirror 6** frontend (one CM6 instance per editing block, a separate `markdown-it` renderer for read-only blocks), wrapped later in **Tauri 2** (current stable: 2.9.x).
>
> Runner-up backend: Go (`goldmark` + `modernc.org/sqlite` + `fsnotify`) wrapped in **Wails v2** (NOT v3 — still alpha as of May 2026).

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **Rust** (stable toolchain) | 1.85+ | Backend language | Smallest binary footprint of viable options; zero-GC = predictable cold-start latency (Foliom's Core Value); same language across native core + Tauri shell (no FFI tax); ecosystem leader for desktop-app cores in 2025/26. | HIGH |
| **pulldown-cmark** | 0.13.x | Markdown parser (CommonMark + GFM) | Event-stream API with `into_offset_iter()` exposes **byte spans** for every event — exactly what RF-21 (extract `[[link]]`/`#tag` only from text nodes) and per-block parsing (RF-16) need. Fastest pure-Rust CommonMark parser; zero unsafe. Used by `mdbook`, docs.rs. | HIGH |
| **rusqlite** | 0.39.x | SQLite binding | Synchronous, ergonomic, **bundles SQLite via the `bundled` feature** (no system dep), **FTS5 enabled via the `bundled-full` or `fts5` feature** (CONFIRMED — official feature flag). Synchronous fits the "one process owns IO" model (no async runtime ceremony for a single-user app). | HIGH |
| **notify** + **notify-debouncer-full** | notify 6.1+, debouncer-full 0.3+ | Cross-platform FS watcher with debounce + rename tracking | `notify-debouncer-full` provides the **`RecommendedCache`** that tracks file IDs across renames (macOS atomic-save quirk — VS Code, Obsidian, Syncthing all rename-to-replace). Required for RF-40/RF-41 not to corrupt the index on external edits. | HIGH |
| **axum** | 0.7.x | HTTP server for localhost UI delivery | Tokio-native, layered router, trivial SSE/WebSocket for pushing watcher events to the UI (RF-40). Tauri 2 has an official `tauri-plugin-localhost` that mounts an axum-compatible server inside the webview wrapper — same crate in dev (cargo run) and prod (Tauri shell). | HIGH |
| **tokio** | 1.40+ | Async runtime | Required by axum/notify-debouncer; single-threaded `current_thread` flavour is enough (single-user, low concurrency). | HIGH |
| **Svelte 5** | 5.37+ | Frontend framework | Compile-time reactivity → tiny runtime (smaller than React/Solid for a same-size app). Runes API (`$state`, `$derived`) is stable. Bundle size matters for cold start of the served UI. | HIGH |
| **CodeMirror 6** | 6.x (modular: `@codemirror/state` 6.5+, `@codemirror/view` 6.30+, `@codemirror/lang-markdown` 6.3+) | Block-level editor | The only editor that supports the *one-block-as-mini-editor* pattern cheaply: each focused block is its own `EditorView` instance with markdown language support; on blur, instance is destroyed and replaced by rendered HTML. Modular packages keep bundle small. | HIGH |
| **markdown-it** | 14.x | Read-only block renderer | Mature, fast, plugin ecosystem for the `[[link]]` / `#tag` / `#[[composite tag]]` custom inline rules required by RF-20, RF-52. Per-block rendering is its sweet spot. | HIGH |
| **Tauri 2** | 2.9.x (current stable) | Desktop wrapper (M4) | Native WebView (WebView2/WKWebView/WebKitGTK) → ~3–10 MB installer vs Electron's ~80 MB. Already on v2 stable for ~18 months; mature plugin ecosystem. `tauri-plugin-localhost` lets the same axum server back the desktop build. | HIGH |

### Supporting Libraries (Rust backend)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` + `serde_json` | 1.x | JSON serialization for HTTP API | Always |
| `tower-http` | 0.6.x | CORS + static asset serving for the SPA | Always |
| `blake3` | 1.5+ | Fast content hash for `files.hash` (RF-03) | Use instead of SHA-256: ~5–10× faster, plenty collision-resistant for cache-key purposes |
| `walkdir` | 2.5+ | Recursive directory scan (RF-01) | Always — handles RF-53 ignored-folders cleanly via `filter_entry` |
| `rusqlite_migration` | 1.3+ | Schema versioning via `user_version` | Schema will evolve across M0–M4; pin migrations early |
| `time` | 0.3+ | Journal date formatting (`May 21st, 2026` — RF-55) | Avoid `chrono`'s heavier deps |
| `dashmap` | 6.x | Lock-free recent-writes cache (RF-05 "ignore own writes") | Track last-written hashes to suppress watcher self-triggers |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logging | Always |

### Frontend Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@codemirror/lang-markdown` | 6.3+ | Markdown grammar + GFM toggles | Always for the active block |
| `@codemirror/commands` | 6.7+ | Keymaps + `history` for undo | Always |
| `@lezer/markdown` | 1.3+ | Underlying parser CodeMirror uses; exposes tree for custom decorations if needed | If you need to highlight `[[link]]` inside CM6 differently |
| `markdown-it-attrs` (optional) | 4.x | Inline attribute syntax | Only if you decide to support `{.class}` callouts |
| `vite` | 5.4+ | Dev server + bundler | Always — pairs cleanly with Svelte 5 and Tauri |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo-watch` | Auto-rebuild during M0–M3 headless dev | `cargo watch -x 'run -- index ./sample'` |
| `cargo-nextest` | Test runner | 30–50% faster than `cargo test`; better filtering for the parser test suite (RF-50/51 cases) |
| `insta` | Snapshot tests for parser output | Critical for round-trip tests (tree → markdown → tree) |
| `criterion` | Benchmarks | Pin RNF-01 numbers from M0 (cold start vs grafo size) |
| Vite + `vite-plugin-svelte` | Frontend build | — |
| `@tauri-apps/cli` 2.x | Desktop packaging (M4 only) | Don't install until M4 — keep the loop tight on the headless core |

## Installation

### Rust workspace (backend)

```toml
# Cargo.toml (root workspace)
[workspace]
members = ["crates/core", "crates/server", "crates/cli"]

# crates/core/Cargo.toml — pure logic, no IO bindings
[dependencies]
pulldown-cmark    = { version = "0.13", default-features = false, features = ["html"] }
serde             = { version = "1", features = ["derive"] }
serde_json        = "1"
blake3            = "1"
time              = { version = "0.3", features = ["formatting", "parsing"] }
tracing           = "0.1"

# crates/server/Cargo.toml — IO + HTTP
[dependencies]
foliom-core           = { path = "../core" }
rusqlite              = { version = "0.39", features = ["bundled", "blob"] } # bundled SQLite includes FTS5 by default since 3.46
# If you want belt-and-suspenders, use "bundled-full" which also enables FTS5/JSON1/RTree explicitly.
rusqlite_migration    = "1"
notify                = "6"
notify-debouncer-full = "0.3"
walkdir               = "2"
dashmap               = "6"
axum                  = "0.7"
tokio                 = { version = "1", features = ["rt", "macros", "signal", "sync"] }
tower-http            = { version = "0.6", features = ["cors", "fs", "trace"] }
tracing-subscriber    = "0.3"
```

### Frontend

```bash
# Scaffold
npm create vite@latest foliom-ui -- --template svelte-ts
cd foliom-ui

# CodeMirror 6 (only the modules we use — keep bundle small)
npm install @codemirror/state @codemirror/view @codemirror/commands \
            @codemirror/language @codemirror/lang-markdown \
            @lezer/markdown

# Read-only renderer + plugins
npm install markdown-it

# Dev
npm install -D svelte typescript vite
```

### Desktop (M4 only)

```bash
cargo install tauri-cli --version "^2.0"
# In src-tauri/Cargo.toml:
#   tauri = { version = "2", features = [] }
#   tauri-plugin-localhost = "2"
```

---

## Architectural Notes (load-bearing for the roadmap)

### 1. Backend serves UI on localhost — same code path in web & desktop builds

In dev: `cargo run --bin foliom-server` boots axum on `127.0.0.1:7345`, Vite dev server on `5173` proxies `/api` → backend.
In M4 (Tauri): same axum server is started inside the Tauri process via `tauri-plugin-localhost`, webview points at `http://localhost:7345`. **No code fork between web and desktop frontends.** This is the entire reason to put a real HTTP layer in (vs Tauri IPC commands) — the web build stays a first-class citizen.

### 2. Per-block CodeMirror 6 instances are cheap *but not free*

Pattern that works (used by Obsidian's "Live Preview", by many Roam/Logseq clones):
- Rendered blocks live in Svelte components with `markdown-it` output (`{@html ...}`).
- On focus: destroy the rendered node, mount a fresh `EditorView` with `EditorState.create({ doc: rawBlockText, extensions: [...] })`. Caret position computed from click coordinates via `view.posAtCoords()` (note: PRD §12.2 already accepts "fim do bloco" as v1 fallback — fine).
- On blur / Enter: capture `view.state.doc.toString()`, destroy the view, swap back to rendered HTML.

**Cost:** ~1–3 ms to mount/unmount one CM6 instance on a modern laptop. Acceptable because only one block is ever in edit mode (RF-11).

**Pitfall:** sharing a single `EditorView` between blocks (move it across DOM nodes) is fragile — CM6's `parent` is bound at creation. **Always create fresh, never reuse.**

### 3. `notify` + `notify-debouncer-full` is the only correct choice

The raw `notify` crate emits per-platform events (FSEvents on macOS coalesces; inotify on Linux fires separate IN_MOVED_FROM/IN_MOVED_TO; ReadDirectoryChangesW on Windows is yet different). The `notify-debouncer-full` add-on:
- Debounces with a configurable window (use 250–500 ms for a notes app — fast enough to feel live, slow enough to absorb editor save-thrash).
- Tracks **file identity across renames** via the `RecommendedCache`, so VS Code's "atomic write" (write tmp → rename over) doesn't look like delete-then-create.

Combined with the dashmap of "hashes I just wrote myself" (RF-05), this is the loop-proof setup.

### 4. SQLite FTS5 is bundled — don't link the system lib

`rusqlite` with `features = ["bundled"]` (or `"bundled-full"`) compiles SQLite from source. As of SQLite 3.46+ (what bundled ships) **FTS5 is compiled in by default**. No system SQLite version mismatches across Linux distros; no DLL hunt on Windows. Trade-off: ~2–4 s extra in first `cargo build`, then cached. Worth it.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **Rust** | **Go** | If team has zero Rust experience and M0–M3 timeline is the binding constraint. Go gets you to a working headless indexer ~30–40% faster. Foliom's bet is Rust because cold-start/RAM is the *Core Value* — Go's GC pauses (even small ones) and larger binary undermine the differentiator. |
| **pulldown-cmark** | **comrak** | If you need a **mutable AST** that you walk/mutate before re-serializing (e.g., for a future markdown formatter). comrak is the only Rust parser with that. For Foliom's "parse → extract refs → throw AST away" flow, pulldown-cmark's event stream is faster and lighter. |
| **rusqlite** (sync) | `sqlx` (async) | Skip. `sqlx` shines for *server* workloads with many concurrent connections. Single-user app: one writer + a few readers. Sync rusqlite in `tokio::task::spawn_blocking` is simpler and faster here. |
| **Svelte 5** | **Solid** | Solid is equally valid (similar bundle size, fine-grained reactivity). Pick Solid if the team is React-fluent and wants JSX. Svelte wins on raw bundle size and on devs who haven't pre-committed to JSX. |
| **Svelte 5** | **React 19** | Use React only if ecosystem (date pickers, command palettes like `cmdk`) outweighs ~30–50 KB of extra runtime. For a single-user PKM app with custom UI, that ecosystem isn't load-bearing. |
| **markdown-it** | **micromark** + **mdast-util-*** (unified ecosystem) | unified is more architecturally correct (AST-based, plugin chain) but heavier and more ceremony. Use unified if you plan to share parser logic between TS frontend and a TS backend — N/A here since backend is Rust. |
| **Tauri 2** | **Wails v3** | **DO NOT use Wails v3 yet** — as of May 2026 it is still tagged `v3.0.0-alpha.1`. If forced into the Go path, use **Wails v2** (stable). |
| **axum** | Tauri IPC commands only | Skip the localhost server, expose everything as `#[tauri::command]`. Cuts ~200 KB binary but kills the "same UI runs in plain browser" property (which is the entire dev-loop story for M1–M3 before M4). Not worth it. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **Electron** | The exact pain Foliom is built to escape. ~80 MB installer, ~150–300 MB idle RAM, slow cold start. Mentioned explicitly in PRD §2 and §3.2 as the anti-pattern. | Tauri 2 |
| **Wails v3 (alpha)** | Still `v3.0.0-alpha.1` in May 2026 — API churn, breaking changes likely between alpha milestones. Tied to the runner-up Go path, but even there prefer v2. | Wails v2 (if Go); Tauri 2 (preferred) |
| **`sqlx` for this project** | Compile-time SQL checking is great for big schemas with many tables; Foliom has ~6 tables. The async overhead (connection pool, await everywhere) buys nothing for a single-writer local DB. | `rusqlite` sync, wrapped in `spawn_blocking` only at the HTTP handler boundary |
| **`go-sqlite3` (mattn)** | If you go Go, this CGo binding hurts cross-compilation (need C toolchain per target). | `modernc.org/sqlite` — pure-Go, FTS5 enabled, cross-compiles cleanly. Slower than CGo but adequate for single-user. |
| **`chokidar` (Node) or any JS-side FS watcher** | The backend owns IO (architectural decision, PRD §5.4). Watching from the frontend would require browser FS API permissions which the PRD explicitly avoids. | `notify` on the Rust side, pushed to UI via SSE/WebSocket. |
| **WYSIWYG markdown editors (Milkdown, ProseMirror with MD round-trip, TipTap with MD)** | RF-13 forbids reconstructing markdown from HTML. These editors fight that requirement. | Plain CM6 textarea-like editing on the active block; render-only display on inactive blocks. |
| **Tantivy / MeiliSearch / other dedicated search engines** | Overkill. SQLite FTS5 indexes 5–10k notes in <1 s, queries in single-digit ms. PRD explicitly chose FTS5 (RF-30). Adding a second index doubles the "what's the source of truth?" problem. | SQLite FTS5 only. |
| **`#tag`/`[[link]]` extraction via regex on raw markdown** | Will match inside code fences, ATX headings, hex colors, URLs — exactly the bug RF-21 calls out. | Walk the pulldown-cmark event stream, only inspect `Event::Text` events that are not inside `Tag::CodeBlock` / `Tag::Heading` (track via a small state stack). |
| **Tauri 1.x** | EOL trajectory; v2 is the supported line. | Tauri 2 from day one of M4. |
| **Polling-based file watchers** | Battery drain on laptops; missed events on fast saves. | `notify` (event-driven, native APIs). |

---

## The Key Open Decision: Rust vs Go (resolved)

PROJECT.md §Key Decisions lists this as Pending. Research verdict:

| Dimension | Rust (recommend) | Go (runner-up) |
|-----------|------------------|----------------|
| Cold start (Core Value!) | Zero GC warmup, ~5–20 ms process start, ~1–2 ms per file `stat`+`hash` | ~30–80 ms runtime warmup, GC tuning required at 5k+ files for steady-state |
| Idle RAM | 8–25 MB typical | 25–60 MB typical (GC overhead) |
| Binary size | 5–12 MB stripped | 8–15 MB stripped |
| Markdown parser quality | pulldown-cmark (events + spans) **or** comrak (AST) — both first-class | goldmark — solid, AST-only |
| SQLite | rusqlite (mature, FTS5 bundled) | modernc.org/sqlite pure-Go (fine) **or** mattn/go-sqlite3 (CGo, cross-compile pain) |
| Watcher | notify + notify-debouncer-full (rename-safe) | fsnotify (must hand-roll debounce + rename tracking) |
| Desktop shell | **Tauri 2 stable** (same lang as core, no FFI) | Wails v2 stable (v3 alpha — avoid) |
| Developer iteration speed | Slower compile (~3–8 s incremental) | Fast compile (~0.5–2 s) |
| Talent pool | Smaller, but PKM/dev-tool space skews Rust-native | Larger |

**Tie-breaker:** the Core Value (`cold start rápido e baixo uso de memória`) is the *raison d'être* of the project. Go is fine, but every benchmark you'll cite to justify Foliom over Logseq will be a Rust benchmark. Build in the language that lets the marketing match the artifact.

**Pick Go only if** the implementer is significantly more productive in Go and timeline > differentiation.

---

## Stack Patterns by Variant

**If you go Rust (recommended):**
- Workspace with `core` (no-IO, parser + tree model + serializer) and `server` (IO + axum) and optional `cli` (M0 headless tests).
- Async only at the HTTP boundary; everything below is sync.
- `tauri-plugin-localhost` in M4; no code rewrite.

**If you go Go (runner-up):**
- `goldmark` with the `extension.GFM` + a custom `parser.InlineParser` for `[[link]]` and `#[[tag]]`.
- `modernc.org/sqlite` — register FTS5 explicitly in your `CREATE TABLE` (it's compiled in).
- `fsnotify` + your own debouncer (300 ms window) + own rename tracker keyed by `os.Stat` inode/file-id. Budget 1–2 days for this; it is the single largest correctness risk on the Go path.
- `chi` or `gin` for HTTP.
- M4: **Wails v2** (NOT v3).

**If targeting only desktop (no localhost web build):**
- Drop axum, expose `#[tauri::command]` handlers. Saves ~200 KB binary. **Not recommended** — kills the dev loop (you can't iterate the UI in a plain browser tab during M1–M3).

---

## Version Compatibility (Rust path)

| Crate | Version | Compatible With | Notes |
|-------|---------|-----------------|-------|
| `rusqlite` 0.39 | `bundled` feature | SQLite 3.46+ embedded | FTS5 compiled in |
| `notify` 6.1 | `notify-debouncer-full` 0.3 | Pair them; debouncer pins notify version |
| `axum` 0.7 | `tokio` 1.40+, `tower-http` 0.6 | All on the 2024-era line |
| `tauri` 2.9 | Rust 1.78+ MSRV | WebView2 (Windows), WKWebView (macOS), webkit2gtk-4.1 (Linux) |
| `@codemirror/*` 6.x | `@lezer/markdown` 1.3+ | Modular — only install what you use |
| Svelte 5.37 | Vite 5.4+, TypeScript 5.5+ | Runes are stable |

---

## Sources

- **Context7** `/websites/rs_tauri_2_9_5` — Tauri 2.9.5 stable confirmed (HIGH)
- **Context7** `/websites/v3_wails_io` — confirmed Wails v3 at `v3.0.0-alpha.1` (HIGH — alpha disqualifies it)
- **Context7** `/websites/v2_tauri_app` — `tauri-plugin-localhost` pattern verified (HIGH)
- **Context7** `/pulldown-cmark/pulldown-cmark` — `into_offset_iter()` byte-span API verified for RF-21 needs (HIGH)
- **Context7** `/notify-rs/notify` — `notify-debouncer-full` + `RecommendedCache` rename tracking verified (HIGH)
- **Context7** `/websites/rs_rusqlite_0_39_0_rusqlite` — rusqlite 0.39.0 current; `bundled` / `bundled-full` features (HIGH)
- **Context7** `/sveltejs/svelte` — Svelte 5.37 / runes stable (HIGH)
- **Context7** `/codemirror/website` — CM6 modular architecture, per-instance editor pattern (HIGH)
- **Context7** `/yuin/goldmark` — Go markdown parser (for Go runner-up) (HIGH)
- **Context7** `/websites/pkg_go_dev_modernc_org_sqlite` — pure-Go SQLite, FTS5 supported (HIGH)
- **Context7** `/fsnotify/fsnotify` — Go watcher (HIGH)
- PRD `PRD-outliner-markdown.md` §5 (architecture decisions), §6 (functional requirements), §9 (proposed stack) — drove the choice criteria.

**Confidence:** HIGH across the recommended path. The single material risk flag is Wails v3 alpha — already mitigated by recommending Tauri 2 as primary and Wails v2 as the Go-path fallback.

---
*Stack research for: local-first markdown outliner (Logseq/Roam alternative)*
*Researched: 2026-05-21*
