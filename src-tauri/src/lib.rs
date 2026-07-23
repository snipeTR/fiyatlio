// src-tauri/src/lib.rs
//
// Module tree + Tauri application builder.
//
// All logic lives in this library crate (not in `main.rs`) so that integration
// tests under `src-tauri/tests/` can link against it directly.

pub mod binance;
pub mod commands;
pub mod engine;
pub mod error;
pub mod models;
pub mod parser;

use tracing_subscriber::EnvFilter;

/// Initialises structured logging. Level is controlled by the `FIYATLIO_LOG`
/// environment variable (e.g. `FIYATLIO_LOG=debug`), defaulting to `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_env("FIYATLIO_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    // `try_init` instead of `init`: a second call (e.g. from a test harness)
    // must not panic. Failure here is non-fatal — the app still runs, silently.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

/// Entry point shared by `main.rs` and any future embedder.
///
/// # Panics
/// Only if Tauri itself cannot start (missing WebView2 runtime, corrupt config).
/// That is unrecoverable and happens before any user data exists, so aborting
/// with a clear message is the correct behaviour — this is the one sanctioned
/// exception to the no-panic rule in AGENTS.md §2.
pub fn run() {
    init_tracing();
    tracing::info!("starting Fiyatlio v{}", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            // TODO(phase-2): import_csv_files, compute_analysis, refresh_prices,
            // load_settings, save_settings, export_trades, generate_report
        ])
        .run(tauri::generate_context!())
        .expect("fatal: Tauri failed to start");
}
