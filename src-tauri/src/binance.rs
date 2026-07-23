// src-tauri/src/binance.rs
//
// Binance public market-data client. No API key is required or accepted.
//
// Contract (AGENTS.md §7), implemented in Phase 2:
//   * Batch endpoint: GET /api/v3/ticker/price?symbols=["ETHUSDT","BTCUSDT"]
//     The `symbols` value is a JSON array and MUST be URL-encoded.
//   * 5 s timeout; on failure retry twice with exponential backoff; then fall back
//     to the cached `PriceSnapshot` and mark it `stale`.
//   * A 400 from the batch call means at least one symbol is unknown — retry the
//     symbols individually to isolate the offenders into `failed_symbols`.
//   * Every successful fetch is persisted through `tauri-plugin-store` so a fully
//     offline launch can still render last-known values with an "as of" stamp.
//
// All HTTP happens here, in Rust — the frontend never calls out directly
// (AGENTS.md §2.4).

/// Base URL for Binance public REST endpoints.
pub const BINANCE_API_BASE: &str = "https://api.binance.com";

/// Batch ticker price path.
pub const TICKER_PRICE_PATH: &str = "/api/v3/ticker/price";

/// Per-request timeout.
pub const REQUEST_TIMEOUT_SECS: u64 = 5;

/// Retry attempts after the initial try.
pub const MAX_RETRIES: u32 = 2;

// TODO(phase-2): pub struct PriceClient { http: reqwest::Client }
// TODO(phase-2): pub async fn fetch_prices(&self, symbols: &[String]) -> AppResult<PriceSnapshot>
// TODO(phase-2): pub fn conversion_symbol(quote_asset: &str) -> Option<String>  // e.g. BTC -> BTCUSDT
