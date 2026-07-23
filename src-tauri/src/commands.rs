// src-tauri/src/commands.rs
//
// The `#[tauri::command]` surface — the only way the frontend reaches the
// backend. Every command returns `Result<T, AppError>` so the frontend can run a
// single error-handling layer.
//
// Phase 0 ships one trivial command purely to prove the IPC wiring; the real
// command set lands in Phase 2.

use crate::error::AppResult;

/// Returns the application version string shown in the UI footer / report header.
#[tauri::command]
pub fn app_version() -> AppResult<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

// TODO(phase-2): import_csv_files(paths: Vec<String>) -> AnalysisResult
// TODO(phase-2): recompute(method: PnlMethod) -> AnalysisResult
// TODO(phase-2): refresh_prices(symbols: Vec<String>) -> PriceSnapshot
// TODO(phase-2): load_settings() / save_settings(settings: Settings)
// TODO(phase-5): export_trades(format, path) / generate_report(path)
