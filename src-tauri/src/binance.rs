// src-tauri/src/binance.rs
//
// Binance public market-data client. No API key is required or accepted.
//
// Contract (AGENTS.md §7):
//   * Batch endpoint: GET /api/v3/ticker/price?symbols=["ETHUSDT","BTCUSDT"]
//     The `symbols` value is a JSON array and MUST be URL-encoded.
//   * 5 s timeout; on failure retry twice with exponential backoff; then the
//     caller falls back to the cached snapshot and marks it stale.
//   * A 400 from the batch call means at least one symbol is unknown — retry the
//     symbols individually to isolate the offenders into `failed_symbols`.
//
// All HTTP happens here, in Rust — the frontend never calls out directly
// (AGENTS.md §2.4).

use std::collections::BTreeMap;
use std::time::Duration;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::REPORTING_ASSET;

/// Base URL for Binance public REST endpoints.
pub const BINANCE_API_BASE: &str = "https://api.binance.com";

/// Batch ticker price path.
pub const TICKER_PRICE_PATH: &str = "/api/v3/ticker/price";

/// Per-request timeout.
pub const REQUEST_TIMEOUT_SECS: u64 = 5;

/// Retry attempts after the initial try.
pub const MAX_RETRIES: u32 = 2;

/// Base delay for the exponential backoff between retries.
const RETRY_BASE_DELAY_MS: u64 = 250;

/// Binance rejects oversized query strings, and a very long `symbols` array is
/// also a heavier weight against the rate limit. Requests are chunked well below
/// any practical limit.
const MAX_SYMBOLS_PER_REQUEST: usize = 100;

/// Raw `/ticker/price` element. Prices arrive as strings, which is exactly what
/// we want — parsing straight into `Decimal` avoids ever touching a float.
#[derive(Debug, Deserialize)]
struct TickerPrice {
    symbol: String,
    price: String,
}

/// What one price fetch produced.
#[derive(Debug, Default, Clone)]
pub struct FetchOutcome {
    pub prices: BTreeMap<String, Decimal>,
    /// Symbols Binance does not recognise (delisted, renamed, or never existed).
    pub failed_symbols: Vec<String>,
}

/// Thin wrapper over a configured `reqwest::Client`.
///
/// The client is reused across refreshes so connections are pooled; building one
/// per request would add a TLS handshake to every poll.
pub struct PriceClient {
    http: reqwest::Client,
    base_url: String,
}

impl PriceClient {
    pub fn new() -> AppResult<Self> {
        Self::with_base_url(BINANCE_API_BASE)
    }

    /// Constructs a client pointed at `base_url`. Tests use this to aim at a
    /// local stub instead of the live exchange.
    pub fn with_base_url(base_url: &str) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(concat!("fiyatlio/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Fetches last prices for `symbols`.
    ///
    /// Duplicates are collapsed and order is normalised, so callers can pass a
    /// naively built list. An empty input short-circuits without any I/O.
    pub async fn fetch_prices(&self, symbols: &[String]) -> AppResult<FetchOutcome> {
        let mut unique: Vec<String> = symbols
            .iter()
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        unique.sort_unstable();
        unique.dedup();

        if unique.is_empty() {
            return Ok(FetchOutcome::default());
        }

        let mut outcome = FetchOutcome::default();

        for chunk in unique.chunks(MAX_SYMBOLS_PER_REQUEST) {
            match self.request_batch(chunk).await {
                Ok(tickers) => {
                    Self::absorb(&mut outcome, chunk, tickers);
                }
                // A 400 means at least one symbol in the batch is unknown, but
                // not which one. Falling back to individual requests isolates the
                // offenders instead of discarding a whole batch of good prices.
                Err(AppError::BinanceStatus { status: 400, .. }) => {
                    tracing::warn!(
                        "batch price request rejected; isolating unknown symbols one by one"
                    );
                    self.fetch_individually(chunk, &mut outcome).await;
                }
                Err(other) => return Err(other),
            }
        }

        Ok(outcome)
    }

    /// Records the tickers that came back and flags anything the response omitted.
    fn absorb(outcome: &mut FetchOutcome, requested: &[String], tickers: Vec<TickerPrice>) {
        for ticker in tickers {
            match Decimal::from_str_exact(&ticker.price) {
                Ok(price) => {
                    outcome.prices.insert(ticker.symbol, price);
                }
                Err(e) => {
                    tracing::warn!("undecodable price for {}: {e}", ticker.symbol);
                    outcome.failed_symbols.push(ticker.symbol);
                }
            }
        }

        for symbol in requested {
            if !outcome.prices.contains_key(symbol) && !outcome.failed_symbols.contains(symbol) {
                outcome.failed_symbols.push(symbol.clone());
            }
        }
    }

    /// One request per symbol. Only reached after a batch 400.
    async fn fetch_individually(&self, symbols: &[String], outcome: &mut FetchOutcome) {
        for symbol in symbols {
            match self.request_batch(std::slice::from_ref(symbol)).await {
                Ok(tickers) => Self::absorb(outcome, std::slice::from_ref(symbol), tickers),
                Err(e) => {
                    tracing::warn!("symbol {symbol} could not be priced: {e}");
                    outcome.failed_symbols.push(symbol.clone());
                }
            }
        }
    }

    /// Issues one batch request, retrying transient failures with exponential
    /// backoff.
    ///
    /// A 4xx (other than 429) is not retried: the request itself is wrong, so
    /// repeating it just burns rate limit. 429 and 5xx are retried.
    async fn request_batch(&self, symbols: &[String]) -> AppResult<Vec<TickerPrice>> {
        let url = format!("{}{}", self.base_url, TICKER_PRICE_PATH);
        // Serialised as a JSON array; `query` percent-encodes it for us.
        let symbols_param = serde_json::to_string(symbols)
            .map_err(|e| AppError::Internal(format!("cannot encode symbol list: {e}")))?;

        let mut last_error = AppError::Network("no attempt was made".into());

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                // 250ms, then 500ms.
                let delay = RETRY_BASE_DELAY_MS * 2u64.pow(attempt - 1);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            let response = self
                .http
                .get(&url)
                .query(&[("symbols", symbols_param.as_str())])
                .send()
                .await;

            match response {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        return response
                            .json::<Vec<TickerPrice>>()
                            .await
                            .map_err(|e| AppError::BinanceDecode(e.to_string()));
                    }

                    let body = response.text().await.unwrap_or_default();
                    let error = AppError::BinanceStatus {
                        status: status.as_u16(),
                        body,
                    };

                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    if !retryable {
                        return Err(error);
                    }
                    last_error = error;
                }
                Err(e) => {
                    last_error = if e.is_timeout() {
                        AppError::Network(format!("timeout after {REQUEST_TIMEOUT_SECS}s"))
                    } else {
                        AppError::Network(e.to_string())
                    };
                }
            }
        }

        Err(last_error)
    }
}

/// Fiat currencies that Binance quotes with USDT as the **base**, i.e. the
/// tradable symbol is `USDTTRY`, not `TRYUSDT`.
///
/// This distinction is not cosmetic. Assuming the `<asset>USDT` form for these
/// silently drops every position quoted in them out of the dashboard totals —
/// on a real Turkish account that meant an 8,500 TRY realized gain simply not
/// being counted. Listing the exceptions explicitly is also what keeps us from
/// firing a request for a symbol that does not exist: an unknown symbol makes
/// the batch endpoint return 400 and forces the slow one-by-one fallback on
/// every single refresh.
const USDT_BASE_FIAT: &[&str] = &[
    "TRY", "BRL", "ARS", "RUB", "UAH", "ZAR", "PLN", "RON", "CZK", "JPY", "MXN", "COP", "NGN",
];

/// USD-pegged stablecoins that no longer have a live USDT market.
///
/// BUSD was wound down, so `BUSDUSDT` returns nothing — yet a history that
/// traded on BUSD pairs is otherwise unvaluable. These redeemed 1:1 against USD
/// for their whole life, so a 1.0 rate is a statement of the peg rather than an
/// invented price. It is applied ONLY when no live market answers, and never in
/// preference to a real quote.
pub const USD_PEGGED_FALLBACK: &[&str] = &["BUSD"];

/// How to turn a price for `symbol` into a rate for the asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionRoute {
    pub symbol: String,
    /// True when the symbol quotes USDT in the asset, so the rate is `1 / price`.
    pub invert: bool,
}

/// Symbols that can convert `asset` into USDT, in preference order.
///
/// Empty when `asset` already *is* the reporting asset — that keeps callers from
/// ever requesting the non-existent `USDTUSDT`.
pub fn conversion_routes(asset: &str) -> Vec<ConversionRoute> {
    let asset = asset.trim().to_ascii_uppercase();
    if asset == REPORTING_ASSET || asset.is_empty() {
        return Vec::new();
    }

    if USDT_BASE_FIAT.contains(&asset.as_str()) {
        return vec![ConversionRoute {
            symbol: format!("{REPORTING_ASSET}{asset}"),
            invert: true,
        }];
    }

    vec![ConversionRoute {
        symbol: format!("{asset}{REPORTING_ASSET}"),
        invert: false,
    }]
}

/// Convenience for the common direct case. `None` for the reporting asset.
pub fn conversion_symbol(asset: &str) -> Option<String> {
    conversion_routes(asset)
        .into_iter()
        .next()
        .map(|route| route.symbol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn conversion_symbol_skips_the_reporting_asset_itself() {
        assert_eq!(conversion_symbol("USDT"), None);
        assert_eq!(conversion_symbol("btc"), Some("BTCUSDT".to_string()));
        assert_eq!(conversion_symbol("BNB"), Some("BNBUSDT".to_string()));
    }

    #[test]
    fn fiat_quoted_in_usdt_uses_the_inverted_symbol() {
        // Binance lists USDTTRY, never TRYUSDT.
        let routes = conversion_routes("TRY");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].symbol, "USDTTRY");
        assert!(routes[0].invert);

        // EUR is the other way round and must not be inverted.
        let routes = conversion_routes("EUR");
        assert_eq!(routes[0].symbol, "EURUSDT");
        assert!(!routes[0].invert);
    }

    #[test]
    fn the_reporting_asset_needs_no_route() {
        assert!(conversion_routes("USDT").is_empty());
        assert!(conversion_routes("  ").is_empty());
    }

    #[test]
    fn absorb_flags_symbols_the_response_left_out() {
        let mut outcome = FetchOutcome::default();
        let requested = vec!["ETHUSDT".to_string(), "GONEUSDT".to_string()];
        let tickers = vec![TickerPrice {
            symbol: "ETHUSDT".into(),
            price: "2500.10".into(),
        }];

        PriceClient::absorb(&mut outcome, &requested, tickers);

        assert_eq!(outcome.prices.get("ETHUSDT"), Some(&dec!(2500.10)));
        assert_eq!(outcome.failed_symbols, vec!["GONEUSDT".to_string()]);
    }

    #[test]
    fn absorb_rejects_an_undecodable_price_instead_of_guessing() {
        let mut outcome = FetchOutcome::default();
        let requested = vec!["ETHUSDT".to_string()];
        let tickers = vec![TickerPrice {
            symbol: "ETHUSDT".into(),
            price: "not-a-number".into(),
        }];

        PriceClient::absorb(&mut outcome, &requested, tickers);

        assert!(outcome.prices.is_empty());
        assert_eq!(outcome.failed_symbols, vec!["ETHUSDT".to_string()]);
    }

    #[tokio::test]
    async fn an_empty_symbol_list_performs_no_io() {
        // `with_base_url` points somewhere unroutable: if this did hit the
        // network the test would fail rather than silently pass.
        let client = PriceClient::with_base_url("http://127.0.0.1:1").expect("client builds");
        let outcome = client.fetch_prices(&[]).await.expect("no-op succeeds");
        assert!(outcome.prices.is_empty());
        assert!(outcome.failed_symbols.is_empty());
    }
}
