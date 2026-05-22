//! `foliom serve <root>` — Phase 2 subcommand (D-22..D-25).
//!
//! Boots a local HTTP server on `127.0.0.1:<port>` that exposes a read-only
//! API over the Phase 1 SQLite index. Orchestration:
//!   1. Open `Db` for the notes root (resolves XDG/AppData path per D-13).
//!   2. Run `indexer::reindex` on startup — fatal on error (D-22: serving a
//!      stale index is worse than refusing to start; T-02-02 mitigation).
//!   3. Build the axum router with health, host-allowlist middleware
//!      (T-02-01: DNS rebinding mitigation), and compression/trace layers.
//!   4. Bind `127.0.0.1:<port>` (loopback only — T-02-01). On `AddrInUse`
//!      with a non-zero requested port, fall back to OS-assigned `:0`.
//!   5. `axum::serve` on a single-threaded tokio runtime (D-25) with
//!      `ctrl_c` graceful shutdown.

pub mod browser;
pub mod dto;
pub mod embed;
pub mod format;
pub mod middleware;
pub mod routes;
pub mod state;
pub mod watcher;

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use clap::Args;
use foliom_core::indexer::{ReindexMode, reindex};
use foliom_core::rename::{Journal, replay_journal};
use foliom_core::storage::Db;
use foliom_core::sync::SelfWriteSet;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::cmd::serve::dto::WatcherEvent;
use crate::cmd::serve::routes::build_router;
use crate::cmd::serve::state::AppState;
use crate::cmd::serve::watcher::spawn_watcher;

/// Porta TCP que `serve::run()` realmente vinculou.
///
/// Escrita após `bind_loopback()` retornar, ANTES de `rt.block_on(...)`.
/// O shell Tauri lê este valor via polling para construir a URL do WebView.
pub static BOUND_PORT: OnceLock<u16> = OnceLock::new();

/// Argumentos do subcomando `foliom serve`.
///
/// Identifiers stay English per Phase 1 invariant; user-facing help text
/// is Portuguese per CLAUDE.md.
#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Raiz do diretório de notas (deve existir).
    pub root: PathBuf,

    /// Porta TCP em 127.0.0.1 onde o servidor escuta.
    /// Default 7345; se ocupada, cai para uma porta livre escolhida pelo SO.
    #[arg(long, default_value_t = 7345)]
    pub port: u16,

    /// Abre o navegador padrão na URL do servidor após o boot (best-effort).
    #[arg(long, default_value_t = false)]
    pub open: bool,

    /// Força reindex completo no startup (ignora cache de mtime/tamanho).
    #[arg(long, default_value_t = false)]
    pub full: bool,
}

/// Entry point dispatched from `main.rs`. Synchronous wrapper that builds
/// a single-threaded tokio runtime (D-25) and blocks on the async core.
pub fn run(args: ServeArgs) -> Result<()> {
    // ---- 1. Open DB ----
    let mut db = Db::open(&args.root)
        .with_context(|| format!("abrindo índice para a raiz {:?}", args.root))?;

    // ---- 2. Open rename journal + replay BEFORE reindex ----
    // T-03-20: after a crash mid-rename, disk files may be partially rewritten.
    // replay_journal repairs disk state so that the subsequent reindex sees
    // consistent files. Inverting this order leaves the index stale until the
    // next watcher event or restart.
    let journal = Arc::new(
        Journal::open_for_root(&args.root)
            .with_context(|| format!("abrindo rename journal para {:?}", args.root))?,
    );

    let self_writes = Arc::new(SelfWriteSet::default());
    let (watcher_tx, _watcher_rx) = broadcast::channel::<WatcherEvent>(64);
    let watcher_tx = Arc::new(watcher_tx);
    let db_arc = Arc::new(Mutex::new(db));

    let state = AppState {
        db: db_arc.clone(),
        root: args.root.clone(),
        self_writes: self_writes.clone(),
        journal: journal.clone(),
        watcher_tx: watcher_tx.clone(),
    };

    // Replay journal first — fixes any partially-renamed files on disk.
    replay_journal(&state).with_context(|| "replay do rename journal no startup")?;

    // ---- 3. Startup reindex after journal replay (fatal on error — D-22 / T-02-02) ----
    let mode = if args.full {
        ReindexMode::Full
    } else {
        ReindexMode::Incremental
    };
    {
        let mut db = state.db.lock().expect("db not poisoned");
        let stats = reindex(&mut db, &args.root, mode)
            .with_context(|| format!("reindex no startup para {:?}", args.root))?;
        tracing::info!(
            scanned = stats.scanned,
            added = stats.added,
            modified = stats.modified,
            unchanged = stats.unchanged,
            deleted = stats.deleted,
            "reindex no startup concluído"
        );
    }

    // Phase 4: start filesystem watcher (SNC-03 + SNC-04).
    // Read FOLIOM_DEBOUNCE_MS for power-user override (D-40-01).
    let debounce_ms: u64 = std::env::var("FOLIOM_DEBOUNCE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    spawn_watcher(
        args.root.clone(),
        state.self_writes.clone(),
        watcher_tx,
        db_arc,
        debounce_ms,
    )
    .with_context(|| format!("iniciando watcher para {:?}", args.root))?;

    let app = build_router(state);

    // ---- 4. Bind loopback with fallback ----
    let std_listener = bind_loopback(args.port)?;
    std_listener
        .set_nonblocking(true)
        .context("definindo listener TCP como non-blocking")?;
    let bound = std_listener
        .local_addr()
        .context("obtendo endereço local do listener")?;
    let url = format!("http://{bound}");

    // Publica a porta antes de rt.block_on para o shell Tauri (D-50-02).
    // `let _ =` suprime o aviso de Err caso run() seja chamado duas vezes
    // (não acontece em produção, mas evita um warning de compilação).
    let _ = BOUND_PORT.set(bound.port());

    // Startup banner (stdout, sempre visível — não gated por RUST_LOG).
    println!("Foliom servindo em {url} — Ctrl+C para parar");

    if args.open {
        browser::try_open(&url);
    }

    // ---- 5. Run axum on a single-threaded tokio runtime ----
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("inicializando runtime tokio (current_thread)")?;

    rt.block_on(async move {
        let listener = TcpListener::from_std(std_listener)
            .context("convertendo std TcpListener para tokio")?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("axum::serve")?;
        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

/// Try to bind `127.0.0.1:port`. If the requested port is non-zero and
/// `AddrInUse`, log a warning and fall back to `127.0.0.1:0` so the OS
/// picks a free port.
fn bind_loopback(port: u16) -> Result<StdTcpListener> {
    let requested = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match StdTcpListener::bind(requested) {
        Ok(l) => Ok(l),
        Err(err) if port != 0 && err.kind() == io::ErrorKind::AddrInUse => {
            tracing::warn!(
                requested = %requested,
                "porta ocupada; caindo para porta livre escolhida pelo SO"
            );
            let fallback = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
            StdTcpListener::bind(fallback)
                .with_context(|| format!("bind fallback em {fallback}"))
        }
        Err(err) => Err(err).with_context(|| format!("bind em {requested}")),
    }
}

/// Resolve when Ctrl+C is received. Cross-platform via `tokio::signal::ctrl_c`
/// (works on Linux/macOS via SIGINT and on Windows via Ctrl+Break / Ctrl+C).
async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "falha ao registrar handler de ctrl_c");
    }
    tracing::info!("sinal de shutdown recebido; encerrando");
}
