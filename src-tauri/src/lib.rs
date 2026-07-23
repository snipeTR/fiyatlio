// src-tauri/src/lib.rs
//
// Module tree + Tauri application builder.
//
// All logic lives in this library crate (not in `main.rs`) so that integration
// tests under `src-tauri/tests/` can link against it directly.

pub mod analysis;
pub mod binance;
pub mod commands;
pub mod engine;
pub mod error;
pub mod export;
pub mod models;
pub mod parser;
pub mod report;
pub mod state;

use tauri::Manager;
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
        .setup(|app| {
            // A client that cannot be built means TLS is unavailable. Rather than
            // refusing to launch — the app is useful offline — fall back to a
            // client aimed at the real API and let each fetch fail individually.
            let client = binance::PriceClient::new().unwrap_or_else(|e| {
                tracing::error!("HTTP client could not be built, prices will be unavailable: {e}");
                binance::PriceClient::with_base_url(binance::BINANCE_API_BASE)
                    .expect("a client with no TLS requirements must build")
            });

            let handle = app.handle();
            let app_state = state::AppState::new(client);

            // Restore settings and the last price snapshot before the first frame
            // renders, so the UI never flashes defaults.
            let settings = state::load_settings(handle);
            let prices = state::load_prices(handle);
            app_state.with(|data| {
                data.settings = settings;
                data.prices = prices;
            })?;

            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::get_settings,
            commands::set_settings,
            commands::get_analysis,
            commands::get_trades,
            commands::import_csv_files,
            commands::clear_data,
            commands::refresh_prices,
            commands::suggested_export_filename,
            commands::export_data,
            commands::suggested_report_filename,
            commands::generate_report,
        ])
        .run(tauri::generate_context!())
        .expect("fatal: Tauri failed to start");
}
