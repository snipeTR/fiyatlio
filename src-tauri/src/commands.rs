// src-tauri/src/commands.rs
//
// The `#[tauri::command]` surface — the only way the frontend reaches the
// backend. Every command returns `Result<T, AppError>` so the frontend can run a
// single error-handling layer.
//
// Most commands return a full `AnalysisResult` rather than a delta. The whole
// computation over a realistic history is sub-millisecond, and returning the
// complete state removes an entire class of bug where the UI's cached numbers
// drift out of step with the backend's.
//
// Locking discipline: `AppState::with` takes a `std::sync::Mutex`. Async commands
// must clone what they need, let the guard drop, then await. Never hold it across
// an `.await`.

use std::path::PathBuf;

use chrono::Utc;
use tauri::{AppHandle, Runtime, State};

use crate::analysis::{build_analysis, required_symbols};
use crate::error::{AppError, AppResult};
use crate::export::{self, ExportFormat, ExportKind};
use crate::models::{AnalysisResult, Position, PriceSnapshot, Settings, Trade};
use crate::parser;
use crate::report::{self, ReportOptions};
use crate::state::{self, AppState};

/// Recomputes the analysis from whatever is currently in state.
fn current_analysis(state: &AppState) -> AppResult<AnalysisResult> {
    state.with(|data| {
        build_analysis(
            &data.trades,
            &data.import,
            &data.settings,
            data.prices.as_ref(),
        )
    })?
}

/// Returns the application version shown in the UI footer and report header.
#[tauri::command]
pub fn app_version() -> AppResult<String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

/// Current settings, as restored from disk at startup.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.with(|data| data.settings.clone())
}

/// Persists `settings` and returns the analysis recomputed under them.
///
/// Changing the P&L method or the oversell policy re-runs the whole replay, which
/// is why this returns a fresh `AnalysisResult` instead of just acknowledging.
#[tauri::command]
pub fn set_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: Settings,
) -> AppResult<AnalysisResult> {
    let mut settings = settings;
    settings.sanitize();

    state.with(|data| data.settings = settings.clone())?;
    state::save_settings(&app, &settings)?;

    current_analysis(&state)
}

/// The analysis for the current data set. Called on startup to hydrate the UI.
#[tauri::command]
pub fn get_analysis(state: State<'_, AppState>) -> AppResult<AnalysisResult> {
    current_analysis(&state)
}

/// The raw trade list, for the All Trades table and the timeline chart.
///
/// Deliberately NOT part of `AnalysisResult`: a large history is tens of
/// thousands of rows, and shipping them across IPC on every price refresh would
/// dominate the cost of a refresh that changes none of them. Trades only change
/// on import or clear, so the frontend fetches them exactly then.
#[tauri::command]
pub fn get_trades(state: State<'_, AppState>) -> AppResult<Vec<Trade>> {
    state.with(|data| data.trades.clone())
}

/// Parses `paths`, replaces the in-memory trade set, and returns the analysis.
///
/// Prices are not fetched here — the frontend calls `refresh_prices` next. That
/// keeps a large import responsive and keeps the app usable with no network.
#[tauri::command]
pub fn import_csv_files<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> AppResult<AnalysisResult> {
    if paths.is_empty() {
        return Err(AppError::NoData);
    }

    let deduplicate = state.with(|data| data.settings.deduplicate)?;
    let (trades, import) = parser::import_files(&paths, deduplicate)?;

    tracing::info!(
        "imported {} trades from {} file(s), {} skipped, {} duplicates",
        trades.len(),
        paths.len(),
        import.invalid_rows,
        import.duplicate_rows
    );

    // Remember the selection so the next launch can offer to restore it.
    let settings = state.with(|data| {
        data.trades = trades;
        data.import = import;
        data.settings.last_files = paths;
        data.settings.clone()
    })?;
    state::save_settings(&app, &settings)?;

    current_analysis(&state)
}

/// Drops the loaded data set, returning the app to its empty state.
#[tauri::command]
pub fn clear_data(state: State<'_, AppState>) -> AppResult<AnalysisResult> {
    state.with(|data| {
        data.trades.clear();
        data.import = crate::models::ImportSummary::empty();
    })?;
    current_analysis(&state)
}

/// Fetches live prices for every symbol the current positions need.
///
/// On failure the cached snapshot is kept and flagged `stale`, so the UI keeps
/// showing last-known values with an explicit "as of" timestamp instead of going
/// blank. The command itself still succeeds — an unreachable exchange is a normal
/// condition for an offline-first app, not an error the user must dismiss.
#[tauri::command]
pub async fn refresh_prices<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<AnalysisResult> {
    // Compute the symbol list and release the lock before any I/O.
    let symbols = {
        let analysis = current_analysis(&state)?;
        let positions: Vec<Position> = analysis
            .positions
            .into_iter()
            .map(|valuation| valuation.position)
            .collect();
        required_symbols(&positions)
    };

    if symbols.is_empty() {
        return current_analysis(&state);
    }

    match state.client.fetch_prices(&symbols).await {
        Ok(outcome) => {
            let snapshot = PriceSnapshot {
                prices: outcome.prices,
                fetched_at: Utc::now(),
                stale: false,
                failed_symbols: outcome.failed_symbols,
            };

            if !snapshot.failed_symbols.is_empty() {
                tracing::warn!("unpriceable symbols: {:?}", snapshot.failed_symbols);
            }

            state.with(|data| data.prices = Some(snapshot.clone()))?;

            // Persisting here is what makes a fully offline next launch useful.
            if let Err(e) = state::save_prices(&app, &snapshot) {
                tracing::warn!("price cache could not be persisted: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("price refresh failed, falling back to cache: {e}");
            state.with(|data| {
                if let Some(prices) = data.prices.as_mut() {
                    prices.stale = true;
                }
            })?;
        }
    }

    current_analysis(&state)
}

/// Suggested file name for an export, so the frontend's save dialog can prefill.
#[tauri::command]
pub fn suggested_export_filename(kind: ExportKind, format: ExportFormat) -> AppResult<String> {
    Ok(export::suggested_filename(kind, format))
}

/// Writes trades or realized events to `path`.
#[tauri::command]
pub fn export_data(
    state: State<'_, AppState>,
    kind: ExportKind,
    format: ExportFormat,
    path: String,
) -> AppResult<String> {
    let analysis = current_analysis(&state)?;
    let trades = state.with(|data| data.trades.clone())?;

    if trades.is_empty() {
        return Err(AppError::NoData);
    }

    let target = PathBuf::from(&path);
    export::export(kind, format, &target, &trades, &analysis.realized_events)?;
    Ok(path)
}

/// Suggested file name for the HTML report.
#[tauri::command]
pub fn suggested_report_filename() -> AppResult<String> {
    Ok(report::suggested_filename())
}

/// Renders the self-contained HTML report to `path` and returns that path.
///
/// Chart images are produced by the frontend (ECharts `getDataURL()`) and passed
/// in, because the backend has no renderer — and bundling one would mean shipping
/// a headless browser.
#[tauri::command]
pub fn generate_report(
    state: State<'_, AppState>,
    path: String,
    options: ReportOptions,
) -> AppResult<String> {
    let analysis = current_analysis(&state)?;
    report::write(&analysis, &options, &PathBuf::from(&path))
}
