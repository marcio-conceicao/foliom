//! Foliom desktop shell — Tauri 2 entry point (DSK-01).
//!
//! Orchestração:
//!   1. Lê o caminho do vault do tauri-plugin-store (`config.json`, chave `vault_root`).
//!   2. Se não houver caminho armazenado, exibe um seletor de pastas nativo via
//!      tauri-plugin-dialog e persiste a escolha no store.
//!   3. Inicia o servidor axum em uma thread OS separada (D-50-01):
//!      `std::thread::spawn` — NUNCA `tauri::async_runtime::spawn` pois
//!      `serve::run()` constrói seu próprio runtime tokio (Pitfall 2).
//!   4. Aguarda `BOUND_PORT` OnceLock (D-50-02, Pitfall 3): polling com 20ms
//!      de sleep, timeout de 5 s (250 tentativas).
//!   5. Abre a janela WebView apontando para `http://127.0.0.1:<port>/`
//!      usando `WebviewUrl::External` (NOT tauri-plugin-localhost — Critical Finding).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::anyhow;
use tauri::{Manager, WebviewUrl};
use tauri::webview::WebviewWindowBuilder;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_store::StoreExt;

use foliom_cli::cmd::serve::{BOUND_PORT, ServeArgs, run as serve_run};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            // ---- 1. Lê vault_root armazenado ----
            let store = app.store("config.json")?;
            let root: PathBuf = match store.get("vault_root") {
                Some(v) => {
                    // O store persiste como serde_json::Value::String
                    match v.as_str() {
                        Some(s) => PathBuf::from(s),
                        None => {
                            return Err(anyhow!("vault_root no store não é uma string").into());
                        }
                    }
                }
                None => {
                    // ---- 2. Seletor de pasta nativo (apenas no primeiro lançamento) ----
                    // blocking_pick_folder() usa sync_channel internamente — não deve ser
                    // chamado na main thread. O setup hook do Tauri NÃO é a main thread,
                    // então é seguro aqui.
                    // API verificada em tauri-plugin-dialog 2.7.1 src/lib.rs linha 723.
                    let file_path = app
                        .dialog()
                        .file()
                        .blocking_pick_folder()
                        .ok_or_else(|| anyhow!("Nenhuma pasta selecionada — encerrando"))?;

                    // FilePath::into_path() converte para PathBuf
                    // (tauri-plugin-fs FilePath enum — desktop variant = FilePath::Path)
                    let chosen: PathBuf = file_path
                        .into_path()
                        .map_err(|e| anyhow!("caminho inválido: {e}"))?;

                    // Persiste para lançamentos futuros
                    store.set(
                        "vault_root",
                        serde_json::Value::String(
                            chosen.to_string_lossy().into_owned(),
                        ),
                    );
                    store.save()?;
                    chosen
                }
            };

            // ---- 3. Inicia servidor axum em thread OS separada (Pitfall 2) ----
            // CRÍTICO: usar std::thread::spawn, NUNCA tauri::async_runtime::spawn.
            // serve_run() constrói seu próprio tokio::runtime::Builder::new_current_thread()
            // e chama .block_on(). Aninhá-lo no runtime Tauri causaria pânico em produção.
            let root_clone = root.clone();
            std::thread::spawn(move || {
                if let Err(e) = serve_run(ServeArgs {
                    root: root_clone,
                    port: 0, // O SO escolhe uma porta livre (D-50-02)
                    open: false,
                    full: false,
                }) {
                    tracing::error!(error = %e, "servidor axum encerrou com erro");
                }
            });

            // ---- 4. Aguarda BOUND_PORT (Pitfall 3) ----
            // BOUND_PORT é escrito em serve::run() após bind_loopback() retornar,
            // ANTES de rt.block_on() — disponível em < 100ms na prática.
            let port = {
                let mut attempts = 0u32;
                loop {
                    if let Some(p) = BOUND_PORT.get() {
                        break *p;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    attempts += 1;
                    if attempts > 250 {
                        return Err(
                            anyhow!("timeout: servidor axum não iniciou em 5s").into(),
                        );
                    }
                }
            };

            // ---- 5. Abre a janela WebView ----
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
