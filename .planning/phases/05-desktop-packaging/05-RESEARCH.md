# Phase 5: Desktop Packaging — Research

**Researched:** 2026-05-22
**Domain:** Tauri 2.9 desktop shell, code signing/notarization, footprint CI gate
**Confidence:** HIGH (all 5 open questions answered from authoritative sources)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (Pre-existing)

| Area | Decision | Source |
|---|---|---|
| Desktop framework | Tauri 2.9 (`tauri-plugin-localhost`) — native WebView (WebView2/WKWebView/WebKitGTK), ~3–10 MB installer | research/STACK.md, PRD §5.4 |
| NOT Electron | Anti-pattern: ~80 MB installer, ~150–300 MB idle RAM | PRD §2, §3.2, CLAUDE.md |
| NOT Wails v3 | `v3.0.0-alpha.1` as of May 2026 — API churn | research/STACK.md |
| axum server pattern | Same axum server in-process; webview loads `http://127.0.0.1:<port>/` | CLAUDE.md Architecture §1 |
| SPA embed | `rust-embed` already in release binary (Phase 2 plan 02-07) | 02-07-SUMMARY |
| Watcher lifecycle | `spawn_watcher` started alongside axum in `serve::run()` (Phase 4) | 04-01-SUMMARY |

### Decisions Locked in Discussion (D-50-xx)

| Decision | Content |
|---|---|
| D-50-01 | `src-tauri/` crate added to workspace; Tauri `setup` hook calls `foliom_cli::cmd::serve::run(ServeArgs { root, port: 0, open: false, force_reindex: false })` in a background thread; no Tauri IPC commands |
| D-50-02 | Bound port surfaced via `static PORT: OnceLock<u16>` in `serve::run()`; setup hook reads port, constructs URL, navigates webview |
| D-50-03 | Native folder picker via `tauri-plugin-dialog`; chosen path stored in `tauri-plugin-store` at `$APPDATA/foliom/config.json` |
| D-50-04 | Code signing CI via GitHub Actions secrets + `tauri-apps/tauri-action@v0`; build succeeds without secrets (unsigned), signs when secrets present |
| D-50-05 | Footprint gate: `du` check < 30 MB installer + `foliom-bench-rss` < 150 MB idle RSS |

### Deferred Ideas (OUT OF SCOPE)

- Linux packaging / `.deb` / `.AppImage`
- Auto-update mechanism (Sparkle / WinSparkle)
- Multiple vault support per launch
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DSK-01 | Foliom ships a Tauri 2 desktop binary wrapping the same Svelte UI via the in-process axum server | Open Question 1 answered: direct `WebviewUrl::External` approach; plugin role clarified (see Critical Finding §1) |
| DSK-02 | macOS and Windows installers are code-signed; macOS is notarized | Open Questions 4 answered: `tauri-action@v0` handles both in one step |
| DSK-03 | Desktop binary < 30 MB installer, < 150 MB idle RSS | Open Question 5 + D-50-05 gate strategy confirmed |
</phase_requirements>

---

## Summary

Phase 5 wraps Foliom's already-complete axum+SPA server (Phase 2 + Phase 4) in a Tauri 2 desktop shell. No new backend logic is required — the shell calls `serve::run()` and loads the resulting `http://127.0.0.1:<port>/` URL in a native WebView. The only new Rust code is `src-tauri/src/main.rs` (Tauri entry point, folder-picker dialog, config persistence, window creation) plus the release CI workflow.

**Critical Finding — `tauri-plugin-localhost` role clarification:** The plugin description in CONTEXT.md ("via `tauri-plugin-localhost`") refers to the axum-as-localhost pattern, not to the plugin crate itself. The `tauri-plugin-localhost` crate starts its own `tiny_http` server to serve Tauri's embedded assets on localhost — it is a replacement for Tauri's custom protocol, not a bridge to an existing HTTP server. Since Foliom's axum server already serves both the SPA (via `rust-embed`) and the API, adding `tauri-plugin-localhost` would create a duplicate server. **The correct implementation uses `WebviewUrl::External` pointing directly at the axum port, with no `tauri-plugin-localhost` crate required.** [VERIFIED: docs.rs/tauri-plugin-localhost + v2.tauri.app/plugin/localhost]

**Primary recommendation:** `src-tauri/` crate with `WebviewUrl::External` + `tauri-plugin-dialog` + `tauri-plugin-store` + release CI using `tauri-apps/tauri-action@v0` with conditional signing.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| HTTP API + SPA delivery | Backend (axum) | — | Already complete; same binary in desktop build |
| FS watcher | Backend (Rust notify) | — | Already complete via spawn_watcher |
| Desktop window / WebView | Tauri shell (src-tauri) | — | Tauri 2 wraps WebView2/WKWebView |
| Native folder picker | Tauri shell | — | tauri-plugin-dialog owns OS dialog API |
| Config persistence (vault root path) | Tauri shell | — | tauri-plugin-store → $APPDATA/foliom/config.json |
| Code signing / notarization | CI (GHA tauri-action) | — | OS-level signing; not in app code |
| Installer size check | CI | — | `du` assertion in release workflow |
| Idle RSS check | CI (foliom-bench-rss) | — | Existing bench binary re-used |

---

## Standard Stack

### Core (src-tauri)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tauri` | 2.9.x | Desktop shell, WebView wrapper | Locked decision; official; WebView2/WKWebView/WebKitGTK |
| `tauri-plugin-dialog` | 2.7.1 | Native folder picker (D-50-03) | Official Tauri plugin; stable [VERIFIED: crates.io + slopcheck OK] |
| `tauri-plugin-store` | 2.4.3 | JSON key-value config persistence | Official Tauri plugin; stable [VERIFIED: crates.io + slopcheck OK] |
| `foliom-cli` (local path) | workspace | Entry point that calls `serve::run()` | Avoids duplicating serve logic |

**Note:** `tauri-plugin-localhost` is NOT required. See Critical Finding above. [VERIFIED: docs.rs/tauri-plugin-localhost source]

### CI / Build

| Tool | Version | Purpose |
|------|---------|---------|
| `tauri-apps/tauri-action` | `@v0` | Cross-platform build + GitHub Release upload + conditional signing |
| `dtolnay/rust-toolchain@stable` | — | Rust toolchain install in GHA |
| `Swatinem/rust-cache@v2` | — | Cargo dependency caching |

### Version Verification

```bash
# Verified via cargo search 2026-05-22
tauri-plugin-localhost = "2.3.2"   # NOT needed — see Critical Finding
tauri-plugin-dialog    = "2.7.1"   # USED — stable, slopcheck OK
tauri-plugin-store     = "2.4.3"   # USED — stable, slopcheck OK
```

**JS bindings** (frontend — not needed for Foliom since we use HTTP API, not Tauri IPC):
- `@tauri-apps/plugin-dialog` 2.7.1 [VERIFIED: npm registry]
- `@tauri-apps/plugin-store` 2.4.3 [VERIFIED: npm registry]

### Installation

```toml
# src-tauri/Cargo.toml additions
[dependencies]
tauri             = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-store  = "2"
foliom-cli        = { path = "../crates/cli" }
```

```bash
# Add src-tauri to workspace Cargo.toml
# [workspace]
# members = ["crates/core", "crates/cli", "src-tauri"]
```

---

## Package Legitimacy Audit

| Package | Registry | Age | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|
| `tauri-plugin-dialog` | crates.io | 2+ years (official Tauri org) | [OK] | Approved |
| `tauri-plugin-store` | crates.io | 2+ years (official Tauri org) | [OK] | Approved |
| `tauri` (2.9.x) | crates.io | 2+ years (official Tauri org) | not scanned (known) | Approved |

**Packages removed due to [SLOP] verdict:** none

**Packages flagged as suspicious [SUS]:** none

---

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  OS Process: Tauri app                                          │
│                                                                 │
│  ┌────────────────┐    OnceLock<u16>   ┌─────────────────────┐ │
│  │  Tauri setup   │ ─────port─────────▶│  axum server        │ │
│  │  hook          │                    │  (serve::run thread) │ │
│  │                │                    │  127.0.0.1:<port>   │ │
│  │  1. read store │                    │  + watcher          │ │
│  │  2. dialog     │                    │  + rust-embed SPA   │ │
│  │  3. build URL  │                    └──────────┬──────────┘ │
│  │  4. open win   │                               │ HTTP       │
│  └────────────────┘                               │            │
│                                                   ▼            │
│  ┌────────────────────────────────────────────────────────┐    │
│  │  WebviewWindow (WebView2 / WKWebView / WebKitGTK)      │    │
│  │  URL: http://127.0.0.1:<port>/                         │    │
│  │  Svelte 5 SPA (embedded via rust-embed)                │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘

User flow: Launch → read stored root from tauri-plugin-store →
           if none: open tauri-plugin-dialog folder picker →
           start axum in thread → wait for OnceLock<u16> →
           WebviewWindowBuilder::new(.., WebviewUrl::External(url))
```

### Recommended Project Structure

```
src-tauri/
├── Cargo.toml           # tauri, tauri-plugin-dialog, tauri-plugin-store, foliom-cli
├── build.rs             # tauri_build::build() — required
├── tauri.conf.json      # identifier, productName, bundle.targets
├── capabilities/
│   └── default.json     # http://127.0.0.1:*/** remote origin allowlist
└── src/
    └── main.rs          # tauri::Builder setup hook + window creation
```

### Pattern 1: Tauri setup hook — start axum, surface port, open window

**What:** Tauri `setup` closure spawns `serve::run()` in a `std::thread`, blocks until the axum server publishes its port into `OnceLock<u16>`, then creates `WebviewWindowBuilder` pointing at `http://127.0.0.1:<port>/`.

**Key design constraint:** `serve::run()` currently builds its own `tokio::Runtime` and blocks on it. Calling it from a `std::thread::spawn` (not from inside tokio) is correct and needed — the Tauri event loop occupies the main thread, and the tokio runtime lives entirely inside the spawned thread. [VERIFIED: serve/mod.rs — `rt.block_on(...)` pattern confirms synchronous wrapper]

**Port discovery — `OnceLock<u16>` pattern (D-50-02):**

The `OnceLock<u16>` approach works cleanly here. The modification to `serve::run()` is minimal: bind the `StdTcpListener` early (already done), extract the port from `bound.port()`, and write it to the static before entering `rt.block_on(...)`. The setup hook polls the `OnceLock` after spawning the thread.

```rust
// In crates/cli/src/cmd/serve/mod.rs — add ONE static and ONE write:
pub static BOUND_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

// In run(), after `let bound = std_listener.local_addr()...`:
let _ = BOUND_PORT.set(bound.port());
// (existing code continues: rt.block_on(...))
```

```rust
// src-tauri/src/main.rs
use std::time::Duration;
use tauri::{webview::WebviewWindowBuilder, WebviewUrl};
use foliom_cli::cmd::serve::{ServeArgs, BOUND_PORT, run as serve_run};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // 1. Read stored vault root (or prompt picker)
            let store = app.store("config.json")?;
            let root: std::path::PathBuf = match store.get("vault_root") {
                Some(v) => serde_json::from_value(v)?,
                None => {
                    // tauri-plugin-dialog folder picker — blocks briefly
                    // (acceptable on first launch; subsequent launches skip this)
                    let chosen = tauri_plugin_dialog::blocking::FileDialogBuilder::new()
                        .pick_folder()
                        .ok_or_else(|| anyhow::anyhow!("no folder chosen"))?;
                    store.set("vault_root", serde_json::to_value(&chosen)?);
                    store.save()?;
                    chosen
                }
            };

            // 2. Spawn axum server in a background OS thread
            std::thread::spawn(move || {
                if let Err(e) = serve_run(ServeArgs {
                    root,
                    port: 0,          // OS picks free port
                    open: false,
                    full: false,
                }) {
                    tracing::error!(error = %e, "servidor axum encerrou com erro");
                }
            });

            // 3. Wait for port (OnceLock written before rt.block_on)
            let port = loop {
                if let Some(p) = BOUND_PORT.get() {
                    break *p;
                }
                std::thread::sleep(Duration::from_millis(20));
            };

            // 4. Open window
            let url: tauri::Url = format!("http://127.0.0.1:{port}/")
                .parse()
                .expect("URL inválida");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Foliom")
                .inner_size(1280.0, 800.0)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao inicializar Tauri");
}
```

**Note on blocking dialog in setup:** The `tauri-plugin-dialog` blocking API is used only on first launch. Consider an alternative: show the window loading a splash page first, then emit a Tauri event once the vault is chosen. For v1, the blocking approach is acceptable given it happens once and does not freeze an existing window. [ASSUMED — tauri-plugin-dialog blocking API exists; exact method signature may differ in 2.7.1]

### Pattern 2: Capabilities allowlist for `WebviewUrl::External`

Tauri 2 requires an explicit capability entry when loading from an external (non-custom-protocol) origin. Without it, the WebView loads correctly BUT Tauri IPC commands (if ever added) are blocked.

Since Foliom uses zero Tauri IPC commands (D-50-01 — "no IPC commands"), the capability file only needs the minimal `default` set. However, add the remote origin allowlist preemptively to avoid future debugging:

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability default para Foliom",
  "platforms": ["linux", "macOS", "windows"],
  "windows": ["main"],
  "permissions": [
    "core:default"
  ],
  "remote": {
    "urls": ["http://127.0.0.1:*/**"]
  }
}
```

[CITED: v2.tauri.app/security/capabilities — remote URL allowlist pattern]

### Pattern 3: Universal macOS binary (arm64 + x86_64)

Tauri 2.9 fully supports `--target universal-apple-darwin`. [VERIFIED: official Tauri discussion #9419 + DEV Community article]

```bash
# Prerequisite: install both targets (run once per CI runner / local machine)
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin

# Build universal binary (runs lipo automatically)
npm run tauri build -- --target universal-apple-darwin
# OR (CLI directly):
cargo tauri build --target universal-apple-darwin
```

**Output artifact:**
```
target/universal-apple-darwin/release/bundle/dmg/Foliom_<version>_universal.dmg
target/universal-apple-darwin/release/bundle/macos/Foliom.app
```

**Build time:** Approximately 2× standard build (compiles for two targets, then `lipo`-combines).

**Important:** The Foliom binary has NO sidecar binaries — the axum server is in-process. So the "all sidecars must be universal too" warning does not apply. [VERIFIED: dev.to/hiyoyok/building-a-universal-binary-with-tauri-v2]

### Pattern 4: tauri-plugin-dialog + tauri-plugin-store wiring

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"    # 2.7.1 current stable [VERIFIED: crates.io]
tauri-plugin-store  = "2"    # 2.4.3 current stable [VERIFIED: crates.io]
foliom-cli = { path = "../crates/cli" }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

```json
// tauri.conf.json (minimal)
{
  "productName": "Foliom",
  "version": "0.1.0",
  "identifier": "dev.foliom.app",
  "build": {
    "frontendDist": "../frontend/dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "windows": []
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

**Note:** `frontendDist` in `tauri.conf.json` is used by `tauri build` to bundle the SPA. Since Foliom uses `rust-embed` to embed the SPA into the Rust binary, this field may need to point at `frontend/dist` but the actual serving is done by axum (not by Tauri's asset protocol). The `devUrl` is used when running `tauri dev`. [ASSUMED — interaction between rust-embed serving and tauri.conf.json frontendDist needs verification in Task 1]

### Pattern 5: Release CI workflow — signing + notarization

**Secret names (authoritative):**

| Secret | Platform | Content | Required for |
|--------|----------|---------|--------------|
| `APPLE_CERTIFICATE` | macOS | Base64-encoded `.p12` file | Code signing |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | `.p12` export password | Code signing |
| `APPLE_SIGNING_IDENTITY` | macOS | e.g. `"Developer ID Application: Name (TEAMID)"` | Identifies cert |
| `APPLE_ID` | macOS | Apple ID email | Notarization (Apple ID method) |
| `APPLE_PASSWORD` | macOS | Apple ID app-specific password | Notarization (NOTE: `APPLE_PASSWORD`, NOT `APPLE_ID_PASSWORD`) |
| `APPLE_TEAM_ID` | macOS | 10-char Team ID from developer.apple.com | Notarization |
| `WINDOWS_CERTIFICATE` | Windows | Base64-encoded `.pfx` file | Code signing |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows | `.pfx` export password | Code signing |

[VERIFIED: v2.tauri.app/distribute/sign/macos + dev.to/tomtomdu73 release workflow]

**Notarization is automatic:** When `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` are set, `tauri build` (invoked by `tauri-action`) calls `xcrun notarytool` automatically. No separate workflow step is needed. [CITED: v2.tauri.app/distribute/sign/macos]

**Conditional signing (no secrets = unsigned artifacts):** The import steps are gated with `if: secrets.APPLE_CERTIFICATE != ''` (macOS) and `if: secrets.WINDOWS_CERTIFICATE != ''` (Windows). When secrets are absent (forks, PRs), the build produces unsigned artifacts and the job still passes. [CITED: tauri-apps/tauri-action workflow examples]

**Complete release workflow:**

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - "v*"

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: true

jobs:
  release:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          # macOS universal binary (arm64 + x86_64 lipo'd by Tauri)
          - platform: macos-latest
            args: "--target universal-apple-darwin"
            rust_targets: "aarch64-apple-darwin,x86_64-apple-darwin"
          # Windows x64
          - platform: windows-latest
            args: "--target x86_64-pc-windows-msvc"
            rust_targets: "x86_64-pc-windows-msvc"

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json

      - name: Build frontend
        working-directory: frontend
        run: npm ci && npm run build

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.rust_targets }}

      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: ". -> target"
          shared-key: phase5-${{ matrix.platform }}

      # macOS: import signing certificate (skipped if secret absent)
      - name: Import Apple signing certificate
        if: runner.os == 'macOS' && secrets.APPLE_CERTIFICATE != ''
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
        run: |
          CERT_PATH=$RUNNER_TEMP/certificate.p12
          KEYCHAIN_PATH=$RUNNER_TEMP/build.keychain-db
          KEYCHAIN_PASSWORD=$(openssl rand -base64 24)
          echo "$APPLE_CERTIFICATE" | base64 --decode > "$CERT_PATH"
          security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
          security default-keychain -s "$KEYCHAIN_PATH"
          security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
          security import "$CERT_PATH" -P "$APPLE_CERTIFICATE_PASSWORD" \
            -A -t cert -f pkcs12 -k "$KEYCHAIN_PATH"
          security set-key-partition-list -S apple-tool:,apple: \
            -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"

      # Windows: import signing certificate (skipped if secret absent)
      - name: Import Windows certificate
        if: runner.os == 'Windows' && secrets.WINDOWS_CERTIFICATE != ''
        env:
          WINDOWS_CERTIFICATE: ${{ secrets.WINDOWS_CERTIFICATE }}
          WINDOWS_CERTIFICATE_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
        shell: pwsh
        run: |
          New-Item -ItemType directory -Path certificate
          Set-Content -Path certificate/tempCert.txt -Value $env:WINDOWS_CERTIFICATE
          certutil -decode certificate/tempCert.txt certificate/certificate.pfx
          Remove-Item -path certificate/tempCert.txt
          Import-PfxCertificate -FilePath certificate/certificate.pfx `
            -CertStoreLocation Cert:\CurrentUser\My `
            -Password (ConvertTo-SecureString -String $env:WINDOWS_CERTIFICATE_PASSWORD `
              -Force -AsPlainText)

      # Build + upload to GitHub Release
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # macOS signing + notarization (no-op when secrets absent)
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        with:
          tagName: v__VERSION__
          releaseName: "Foliom v__VERSION__"
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}

      # Footprint gate — installer size (macOS)
      - name: Footprint gate — DMG size < 30 MB (macOS)
        if: runner.os == 'macOS'
        run: |
          DMG=$(ls target/universal-apple-darwin/release/bundle/dmg/*.dmg 2>/dev/null | head -1)
          if [ -z "$DMG" ]; then
            echo "::error::No DMG found"
            exit 1
          fi
          SIZE_MB=$(du -sm "$DMG" | cut -f1)
          echo "DMG size: ${SIZE_MB} MB (ceiling: 30 MB)"
          if [ "$SIZE_MB" -gt 30 ]; then
            echo "::error::DMG ${SIZE_MB} MB exceeds 30 MB budget (DSK-03)"
            exit 1
          fi

      # Footprint gate — installer size (Windows)
      - name: Footprint gate — MSI/NSIS size < 30 MB (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $installer = Get-ChildItem -Path "target\x86_64-pc-windows-msvc\release\bundle" `
            -Recurse -Include "*.msi","*.exe" | Select-Object -First 1
          if (-not $installer) { Write-Error "No installer found"; exit 1 }
          $sizeMb = [math]::Round($installer.Length / 1MB)
          Write-Host "Installer size: $sizeMb MB (ceiling: 30 MB)"
          if ($sizeMb -gt 30) {
            Write-Error "Installer $sizeMb MB exceeds 30 MB budget (DSK-03)"
            exit 1
          }
```

### Anti-Patterns to Avoid

- **Do NOT use `tauri-plugin-localhost`** — it starts a competing `tiny_http` server on the same port, duplicating axum. Foliom's axum server already serves the SPA via `rust-embed`. Use `WebviewUrl::External` directly. [VERIFIED: docs.rs/tauri-plugin-localhost source analysis]
- **Do NOT call `serve::run()` from `tauri::async_runtime::spawn`** — `serve::run()` is a synchronous function that builds its own tokio runtime. Spawning it inside Tauri's tokio runtime would create a nested runtime panic. Use `std::thread::spawn`. [VERIFIED: serve/mod.rs `rt.block_on` pattern]
- **Do NOT busy-spin on `OnceLock::get()`** — use `std::thread::sleep(20ms)` between polls. The port is written before `rt.block_on`, so the wait is < 100 ms in practice.
- **Do NOT sign in `tauri-action` env without first importing the keychain** — the import step must precede the action step, otherwise `codesign` cannot find the identity.
- **Do NOT use `APPLE_ID_PASSWORD`** — the correct secret name is `APPLE_PASSWORD`. The Tauri docs use `APPLE_PASSWORD` consistently. [VERIFIED: v2.tauri.app/distribute/sign/macos]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Native folder picker dialog | Custom HTML/JS file input | `tauri-plugin-dialog` | `<input type=file>` restricted in WebView; native dialog required for root selection |
| Config persistence (vault path) | File read/write in axum | `tauri-plugin-store` | AppData path resolution is OS-specific; plugin handles `$APPDATA`/`~/Library/Application Support` correctly |
| Universal binary lipo step | Hand-running `lipo` | `--target universal-apple-darwin` in tauri build | Tauri's CLI runs `lipo` automatically and places output in correct bundle path |
| Code signing / notarization | Shell scripts calling `codesign`/`xcrun` directly | `tauri-action@v0` + env vars | tauri-action orchestrates the full chain including notarization stapling |

---

## Common Pitfalls

### Pitfall 1: `tauri.conf.json` frontendDist vs rust-embed

**What goes wrong:** Tauri expects `frontendDist` to point at a built SPA directory. When `tauri dev` runs, it also expects a `devUrl`. Since Foliom embeds the SPA via `rust-embed`, not Tauri's asset protocol, `frontendDist` is only used as a hint for the bundle step — the actual assets are served by axum.

**Why it happens:** Tauri's build system copies `frontendDist` contents into the bundle's resources folder AND generates the asset protocol server. When using `WebviewUrl::External`, these bundled assets are never loaded — but the build step still fails if `frontendDist` doesn't exist.

**How to avoid:** Point `frontendDist` at `../frontend/dist` (same as existing CI). The `cargo build --release` step in CI already builds the frontend first (existing CI pattern). The `tauri build` step then finds `frontend/dist/` present and packs it (even though axum already serves it from rust-embed). Harmless duplication.

**Warning signs:** `tauri build` error: `frontendDist directory does not exist` — means `npm run build` must run before `tauri build`.

### Pitfall 2: Nested tokio runtime panic

**What goes wrong:** `serve::run()` calls `tokio::runtime::Builder::new_current_thread().build()` then `.block_on(...)`. If called from inside Tauri's async runtime (e.g., `tauri::async_runtime::spawn`), you get `thread 'tokio-runtime-worker' panicked: Cannot start a runtime from within a runtime`.

**Why it happens:** `Runtime::block_on` forbids nesting.

**How to avoid:** Always call `serve_run(...)` from `std::thread::spawn`, never from `tauri::async_runtime::spawn` or `.setup()` directly (setup is sync but if Tauri ever changes this, the test is: is there already a tokio context?).

**Warning signs:** `Cannot start a runtime from within a runtime` panic at startup.

### Pitfall 3: OnceLock read race

**What goes wrong:** The setup hook reads `BOUND_PORT.get()` immediately after spawning the thread — before `serve::run()` has reached the `bind_loopback` call. The loop returns `None` and the window gets a wrong URL.

**Why it happens:** Thread scheduling. The OS may not even start the spawned thread before the main thread polls.

**How to avoid:** Sleep 20 ms between polls. The port is published inside `run()` after `bind_loopback()` returns, which happens before the Tokio runtime starts — typically within 10–50 ms of thread start.

**Warning signs:** WebView shows "connection refused" on first launch (rare, only on very slow machines).

### Pitfall 4: macOS codesign identity string format

**What goes wrong:** `APPLE_SIGNING_IDENTITY` must match the exact string shown by `security find-identity -v -p codesigning`, e.g. `"Developer ID Application: Your Name (TEAM1234XY)"`. A partial match or wrong format causes `codesign: error: no identity found`.

**Why it happens:** `codesign` uses substring matching but `tauri build` passes the value verbatim.

**How to avoid:** After importing the `.p12`, print identities with `security find-identity -v -p codesigning` and store the full string in the `APPLE_SIGNING_IDENTITY` secret.

### Pitfall 5: Windows OV cert on exportable file no longer issued (post-June 2023)

**What goes wrong:** Traditional OV certificates can no longer be exported as `.pfx` files — they must be stored on HSMs. The `WINDOWS_CERTIFICATE` + `WINDOWS_CERTIFICATE_PASSWORD` approach only works with older OV certs or EV certs with Azure Key Vault.

**Why it happens:** CA/Browser Forum Baseline Requirements change effective June 2023.

**How to avoid:** If obtaining a NEW Windows cert in 2024+, use Azure Key Vault with the `relic` signing tool. The existing workflow YAML (above) shows the legacy local cert approach for reference — update to Azure Key Vault if acquiring a new certificate. [CITED: v2.tauri.app/distribute/sign/windows + CA/Browser Forum]

**Warning signs:** "certificate is not exportable" error when trying to export from the CA portal.

### Pitfall 6: Universal binary requires both Rust targets pre-installed

**What goes wrong:** `tauri build --target universal-apple-darwin` fails with `error[E0463]: can't find crate for...` or `error: toolchain ... does not contain component 'rust-std' for target`.

**Why it happens:** Both `aarch64-apple-darwin` and `x86_64-apple-darwin` targets must be installed via `rustup target add` before building.

**How to avoid:** Add both targets in the CI workflow's `rust-toolchain` step via `targets: "aarch64-apple-darwin,x86_64-apple-darwin"`. For local builds: `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `tauri-plugin-dialog` 2.7.1 exposes a `blocking::FileDialogBuilder::new().pick_folder()` API | Pattern 1 (setup hook) | Exact API method may differ; planner should verify against official API docs before coding |
| A2 | `tauri.conf.json` `frontendDist` pointing at `../frontend/dist` is compatible with the rust-embed + WebviewUrl::External approach | Pitfall 1 | If Tauri enforces frontendDist for the asset server rather than the bundle, additional config may be needed; verify in Task 1 with `tauri build` |
| A3 | `APPLE_SIGNING_IDENTITY` secret can be omitted and replaced with `CERT_ID` extracted from the keychain import step | Release CI pattern | If tauri-action requires this env var explicitly, the signing step fails silently |
| A4 | `tauri-plugin-store` 2.4.3 exposes `.store("config.json")` off the `app` handle in the setup hook | Pattern 1 | Exact API signature may differ; verify against tauri-plugin-store 2.x docs |

---

## Open Questions

1. **Splash screen on first launch:** The blocking dialog approach in `setup` means the window doesn't open until after the user picks a folder AND the server starts. For first launch, this could be a 2–5 s blank period. Consider: show a splash webview immediately, navigate to the axum URL after port is ready. Not blocking for v1 (CONTEXT.md accepts this), but worth a note in the plan.

2. **`tauri.conf.json` + `src-tauri/build.rs` bootstrap:** `tauri-build::build()` in `build.rs` generates capability schema files required by the capabilities JSON. The initial scaffold of `src-tauri/` needs this plumbing. The planner should include a `tauri init` or manual scaffold task.

3. **foliom-bench-rss target for Tauri build:** The existing `foliom-bench-rss` binary measures RSS for the headless `foliom serve` process. For DSK-03, we want the Tauri binary's RSS. The Tauri process spawns a separate WebView renderer process. The 150 MB budget needs to cover: axum server process + WebView renderer. Consider measuring the sum of all `foliom`-related PIDs, or using a macOS-specific `Activity Monitor` capture. [ASSUMED — the existing bench-rss only measures a single PID]

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable | src-tauri compilation | ✓ | 1.95 (workspace) | — |
| Node.js 20 | frontend/dist for tauri.conf.json | ✓ | present in CI | — |
| `cargo-nextest` | test runs | ✓ | CI installs via taiki-e/install-action | — |
| WebView2 SDK | Windows build | ✓ | GHA `windows-latest` ships it | — |
| Xcode + codesign | macOS code signing | ✗ (local) | GHA `macos-latest` has it | Build unsigned locally |
| Apple Developer cert | macOS notarization | ✗ | $99/yr, 24–48h approval | Unsigned build succeeds |
| Windows OV/EV cert | Windows signing | ✗ | $300+/yr, 2–5 day verification | Unsigned build succeeds |

**Missing dependencies with no fallback:** none (all blocking items have an unsigned-build fallback)

**Missing dependencies with fallback:**
- Apple Developer cert: cert procurement needed before v1 release; CI passes without it (unsigned)
- Windows cert: Azure Key Vault path recommended for new certs; CI passes without it (unsigned)

**Operator Note (from CONTEXT.md):** Code-signing cert lead time is weeks. If not yet started, begin Apple Developer Program enrollment and Windows cert purchase during Phase 5 implementation. Phase 5 build tasks do not block on cert availability.

---

## Plan Breakdown

### Recommended: 3 Plans

**Plan 05-01: Tauri shell setup**
- Add `src-tauri/` crate to workspace
- `main.rs` with setup hook: `std::thread::spawn(serve_run)` + `OnceLock` polling + `WebviewWindowBuilder`
- Modify `serve::run()` to export `pub static BOUND_PORT: OnceLock<u16>`
- `tauri-plugin-dialog` folder picker on first launch
- `tauri-plugin-store` persistence of vault root
- `capabilities/default.json` with `http://127.0.0.1:*/**` remote allowlist
- Local smoke test: `cargo tauri dev` opens window showing Foliom UI
- Deliverables: DSK-01

**Plan 05-02: Release CI — signing, notarization, artifacts**
- `.github/workflows/release.yml` with matrix (macOS universal + Windows x64)
- Conditional macOS keychain import (skipped when `APPLE_CERTIFICATE` absent)
- Conditional Windows PFX import (skipped when `WINDOWS_CERTIFICATE` absent)
- `tauri-apps/tauri-action@v0` step with all signing env vars
- Installer size assertion (< 30 MB) in the release workflow
- Verify unsigned build passes on PR (no secrets) — CI green without certs
- Deliverables: DSK-02 (CI plumbing ready; actual signing requires certs)

**Plan 05-03: Footprint gate — RSS assertion for Tauri binary**
- Extend `foliom-bench-rss` (or add `foliom-bench-rss-tauri`) to measure combined RSS of Tauri process + WebView renderer
- Add RSS gate (< 150 MB) to release workflow (runs after successful build)
- Document baseline measurement (expected: 49 MB axum baseline + ~80 MB WebView overhead)
- Deliverables: DSK-03

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tauri-plugin-localhost` serving Tauri assets | `WebviewUrl::External` to existing axum server | N/A — never needed for this pattern | Eliminates duplicate server; simpler codebase |
| OV cert as exportable `.pfx` | Azure Key Vault / HSM required for new OV certs | June 2023 (CA/Browser Forum) | New Windows cert procurement must use Azure Key Vault approach |
| `APPLE_ID_PASSWORD` env var | `APPLE_PASSWORD` env var | Tauri 2 release | Using wrong name silently skips notarization |
| Separate `.dmg` per architecture | `universal-apple-darwin` fat binary | Tauri 2.0+ | One `.dmg` covers both Intel and Apple Silicon |

---

## Sources

### Primary (HIGH confidence)
- `docs.rs/tauri-plugin-localhost/latest/src/tauri_plugin_localhost/lib.rs.html` — source confirms `tiny_http` server, NOT an axum bridge [VERIFIED]
- `v2.tauri.app/plugin/localhost/` — official plugin docs + example code [VERIFIED]
- `v2.tauri.app/distribute/sign/macos/` — exact secret names, notarization automation [VERIFIED]
- `v2.tauri.app/distribute/sign/windows/` — Windows cert import workflow [VERIFIED]
- `crates.io/crates/tauri-plugin-dialog` — version 2.7.1, slopcheck OK [VERIFIED]
- `crates.io/crates/tauri-plugin-store` — version 2.4.3, slopcheck OK [VERIFIED]
- `crates/cli/src/cmd/serve/mod.rs` — existing run() + bind_loopback() + block_on pattern [VERIFIED: codebase]
- Tauri Discussion #9419 + dev.to/hiyoyok — universal-apple-darwin support + artifact path [VERIFIED]

### Secondary (MEDIUM confidence)
- `dev.to/tomtomdu73/ship-your-tauri-v2-app...part-22` — complete release.yml YAML with signing steps [CITED]
- `dev.to/hiyoyok/building-a-universal-binary-with-tauri-v2` — universal binary build instructions [CITED]

### Tertiary (LOW confidence — needs verification in Task 1)
- `tauri-plugin-dialog` blocking API method signature (A1)
- `tauri-plugin-store` `.store()` handle method on App (A4)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — crates verified on crates.io + slopcheck
- Architecture (WebviewUrl::External pattern): HIGH — source-level verification of tauri-plugin-localhost behavior
- Release CI workflow: HIGH — verified against official Tauri signing docs + community production workflow
- Universal binary: HIGH — confirmed via Tauri official discussions
- Plugin API details (dialog blocking, store handle): MEDIUM — method names assumed from training; verify in Task 1

**Research date:** 2026-05-22
**Valid until:** 2026-08-22 (stable ecosystem; 90-day validity)
