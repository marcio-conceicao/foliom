//! `foliom serve <root>` — Phase 2 subcommand (D-22..D-25).
//!
//! Boots a local HTTP server on `127.0.0.1:<port>` that exposes a read-only
//! API over the Phase 1 SQLite index. This module owns the orchestration:
//!   1. Open `Db` for the notes root (resolves XDG/AppData path per D-13).
//!   2. Run `indexer::reindex` on startup — fatal on error (D-22: serving a
//!      stale index is worse than refusing to start).
//!   3. Build the axum router with health, host-allowlist middleware, and
//!      compression/trace layers.
//!   4. Bind `127.0.0.1:<port>` (loopback only — T-02-01). On `AddrInUse`
//!      with a non-zero requested port, fall back to OS-assigned `:0`.
//!   5. `axum::serve` on a single-threaded tokio runtime (D-25) with
//!      `ctrl_c` graceful shutdown.
//!
//! Task 1 of plan 02-01 wires the clap surface; Task 2 fleshes out `run`.

pub mod browser;
pub mod middleware;
pub mod routes;
pub mod state;

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

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

/// Entry point dispatched from `main.rs`. Task 2 fills this in.
pub fn run(_args: ServeArgs) -> Result<()> {
    // Task 1 stub — Task 2 implements the orchestration described above.
    Ok(())
}
