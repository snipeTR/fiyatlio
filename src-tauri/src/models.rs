// src-tauri/src/models.rs
//
// Core domain model for Fiyatlio.
//
// DESIGN RULES (see AGENTS.md §2):
//   * Every monetary / quantity value is a `rust_decimal::Decimal`. Never f64.
//   * Every `Decimal` that crosses the IPC boundary is serialized as a STRING via
//     `#[serde(with = "rust_decimal::serde::str")]`, because JSON numbers are IEEE-754
//     doubles in JavaScript and would silently lose precision on 8-decimal crypto
//     quantities.
//   * All timestamps are UTC. Local-time rendering happens in the frontend only.
//   * Field names are serialized as camelCase so the TypeScript side stays idiomatic.

use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Known quote assets, ordered by **descending length** so that suffix matching
/// resolves `FDUSD` before `USD`-like shorter candidates and `USDT` before `TRY`
/// can never mis-fire. Order matters — do not sort this list alphabetically.
///
/// Used by `parser::split_pair` to break `ETHUSDT` into `("ETH", "USDT")`.
pub const QUOTE_ASSETS: &[&str] = &[
    "FDUSD", // 5
    "USDT", "USDC", "TUSD", "BUSD", "USDP", // 4
    "DAI", "TRY", "EUR", "GBP", "BRL", // 3
    "BTC", "ETH", "BNB", // 3 (crypto quotes)
];

/// The asset every dashboard total is normalised into.
pub const REPORTING_ASSET: &str = "USDT";

// ---------------------------------------------------------------------------
// Side
// ---------------------------------------------------------------------------

/// Trade direction. Parsed case-insensitively from the CSV `Side` column;
/// anything other than BUY/SELL makes the row invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    /// Case-insensitive parse. Returns `None` for anything unrecognised so the
    /// caller can turn it into a `RowError` instead of panicking.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "BUY" => Some(Side::Buy),
            "SELL" => Some(Side::Sell),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
}

// ---------------------------------------------------------------------------
// Raw CSV row
// ---------------------------------------------------------------------------

/// Verbatim CSV record, before any validation. Every field stays a `String`
/// because the numeric columns carry a trailing asset code (`0.2681ETH`) and the
/// timestamp needs explicit UTC interpretation.
///
/// Expected header (exact, case-sensitive):
/// `Time,Pair,Side,Price,Executed,Amount,Fee`
#[derive(Debug, Clone, Deserialize)]
pub struct RawTradeRow {
    #[serde(rename = "Time")]
    pub time: String,
    #[serde(rename = "Pair")]
    pub pair: String,
    #[serde(rename = "Side")]
    pub side: String,
    #[serde(rename = "Price")]
    pub price: String,
    /// Base-asset quantity with unit suffix, e.g. `0.2681ETH`.
    #[serde(rename = "Executed")]
    pub executed: String,
    /// Quote-asset gross amount with unit suffix, e.g. `499.944837USDT`.
    #[serde(rename = "Amount")]
    pub amount: String,
    /// Fee with unit suffix. May be denominated in the base, quote, or a third
    /// asset (typically BNB). May be zero.
    #[serde(rename = "Fee")]
    pub fee: String,
}

/// The exact header Binance emits. `parser` rejects a file whose header differs.
pub const EXPECTED_CSV_HEADER: [&str; 7] =
    ["Time", "Pair", "Side", "Price", "Executed", "Amount", "Fee"];

/// A numeric field split into its value and its asset code (`0.2681ETH` →
/// `(0.2681, "ETH")`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmountWithAsset {
    pub value: Decimal,
    pub asset: String,
}

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

/// A single validated fill. This is the only input the P&L engines accept.
///
/// `file_index` + `row_index` form the stable tie-breaker used when several
/// fills share the same second: sorting by `(timestamp_utc, file_index,
/// row_index)` preserves the original CSV order, which is what FIFO requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Monotonic id assigned after the global sort. Referenced by
    /// `RealizedEvent` and `Lot` for auditability.
    pub id: u64,

    pub timestamp_utc: DateTime<Utc>,

    /// Raw pair symbol as it appears in the CSV, e.g. `ETHUSDT`.
    pub pair: String,
    /// Left-hand side of the pair, e.g. `ETH`.
    pub base_asset: String,
    /// Right-hand side of the pair, e.g. `USDT`. P&L for this pair is always
    /// denominated in this asset.
    pub quote_asset: String,

    pub side: Side,

    /// Execution price, expressed in quote per 1 base.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,

    /// `Executed` column value — the gross base quantity, BEFORE fee deduction.
    /// Fee handling (AGENTS.md §6.4) is applied by the engine, not here.
    #[serde(with = "rust_decimal::serde::str")]
    pub qty: Decimal,

    /// `Amount` column value — the gross quote amount, BEFORE fee adjustment.
    #[serde(with = "rust_decimal::serde::str")]
    pub quote_amount: Decimal,

    /// Fee magnitude. Always >= 0; zero is legal.
    #[serde(with = "rust_decimal::serde::str")]
    pub fee_amount: Decimal,
    /// Asset the fee was charged in. Compare against `base_asset` / `quote_asset`
    /// to pick the fee rule; anything else is a "third asset" fee (BNB).
    pub fee_asset: String,

    /// Absolute path of the CSV this row came from (shown in the UI + report).
    pub source_file: String,
    /// Index of the source file in the user's selection order (0-based).
    pub file_index: usize,
    /// 1-based data row number inside that file (header excluded), so error
    /// messages match what the user sees in a spreadsheet.
    pub row_index: usize,
}

impl Trade {
    /// Classifies the fee so the engines can branch without repeating string
    /// comparisons.
    pub fn fee_kind(&self) -> FeeKind {
        if self.fee_amount.is_zero() {
            FeeKind::None
        } else if self.fee_asset == self.base_asset {
            FeeKind::Base
        } else if self.fee_asset == self.quote_asset {
            FeeKind::Quote
        } else {
            FeeKind::ThirdAsset
        }
    }

    /// Deduplication key: two rows from overlapping exports describing the very
    /// same fill are byte-identical across all of these fields (AGENTS.md §6.2).
    /// Genuine partial fills within the same second differ in `qty`.
    pub fn dedup_key(&self) -> DedupKey {
        DedupKey {
            timestamp_utc: self.timestamp_utc,
            pair: self.pair.clone(),
            side: self.side,
            price: self.price,
            qty: self.qty,
            quote_amount: self.quote_amount,
            fee_amount: self.fee_amount,
            fee_asset: self.fee_asset.clone(),
        }
    }

    /// Canonical ordering key. `sort_by_key` with this preserves CSV order for
    /// same-second fills because `Vec::sort_*` is stable and the key itself
    /// already encodes file/row position.
    pub fn sort_key(&self) -> (DateTime<Utc>, usize, usize) {
        (self.timestamp_utc, self.file_index, self.row_index)
    }
}

/// Which asset a trade's fee was charged in, relative to that trade's pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeeKind {
    /// Fee is exactly zero — nothing to adjust.
    None,
    /// Fee charged in the base asset (e.g. ETH fee on an ETHUSDT trade).
    Base,
    /// Fee charged in the quote asset (e.g. USDT fee on an ETHUSDT trade).
    Quote,
    /// Fee charged in an unrelated asset, typically BNB. v1 does NOT fold this
    /// into cost basis — the historical BNB price at fill time is unknown, so any
    /// conversion would be a guess. It is accumulated in `fees_by_asset` instead
    /// and shown with an "≈" marker on the dashboard.
    ThirdAsset,
}

/// Value used to detect byte-identical duplicate rows across overlapping exports.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DedupKey {
    pub timestamp_utc: DateTime<Utc>,
    pub pair: String,
    pub side: Side,
    pub price: Decimal,
    pub qty: Decimal,
    pub quote_amount: Decimal,
    pub fee_amount: Decimal,
    pub fee_asset: String,
}

// ---------------------------------------------------------------------------
// Lot (FIFO queue element)
// ---------------------------------------------------------------------------

/// An open inventory lot: base quantity still on hand, plus the quote-denominated
/// unit cost it was acquired at.
///
/// Cost is stored as a **unit** cost rather than a total so that a partial
/// consumption is an exact proportional split (`unit_cost * consumed_qty`) with
/// no accumulated rounding drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lot {
    /// Base quantity still unmatched. Strictly > 0 while the lot is in the queue.
    #[serde(with = "rust_decimal::serde::str")]
    pub qty_remaining: Decimal,

    /// Cost per 1 unit of base, in the pair's quote asset, fees already folded in
    /// per AGENTS.md §6.4.
    #[serde(with = "rust_decimal::serde::str")]
    pub unit_cost_quote: Decimal,

    pub acquired_at: DateTime<Utc>,

    /// BUY trade that opened this lot — lets the UI trace a realized event back
    /// to the exact source row.
    pub trade_id: u64,
}

impl Lot {
    /// Remaining cost carried by this lot.
    pub fn remaining_cost(&self) -> Decimal {
        self.unit_cost_quote * self.qty_remaining
    }
}

/// Per-pair FIFO inventory. A plain newtype so the engines cannot accidentally
/// push to the wrong end of the queue.
#[derive(Debug, Clone, Default)]
pub struct LotQueue(pub VecDeque<Lot>);

impl LotQueue {
    pub fn total_qty(&self) -> Decimal {
        self.0.iter().map(|l| l.qty_remaining).sum()
    }

    pub fn total_cost(&self) -> Decimal {
        self.0.iter().map(Lot::remaining_cost).sum()
    }
}

// ---------------------------------------------------------------------------
// Realized events
// ---------------------------------------------------------------------------

/// One lot consumed (fully or partially) by a SELL. Kept for auditability: the
/// report lists exactly which acquisitions funded each disposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedLot {
    /// BUY trade that opened the consumed lot. `None` for the synthetic
    /// zero-cost lot created by an oversell.
    pub buy_trade_id: Option<u64>,
    pub acquired_at: Option<DateTime<Utc>>,

    #[serde(with = "rust_decimal::serde::str")]
    pub qty: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unit_cost_quote: Decimal,
    /// `qty * unit_cost_quote`, precomputed for the UI.
    #[serde(with = "rust_decimal::serde::str")]
    pub cost: Decimal,
    /// True when this slice came from the zero-cost synthetic lot.
    pub synthetic: bool,
}

/// A completed disposal. Emitted once per SELL trade by both engines.
///
/// All monetary fields are denominated in `quote_asset` — conversion to USDT for
/// dashboard totals happens later and separately (AGENTS.md §6.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealizedEvent {
    pub sell_trade_id: u64,
    pub timestamp: DateTime<Utc>,
    pub pair: String,
    pub base_asset: String,
    pub quote_asset: String,

    /// Base quantity disposed of.
    #[serde(with = "rust_decimal::serde::str")]
    pub qty: Decimal,

    /// Net quote received, after the SELL-side fee rules.
    #[serde(with = "rust_decimal::serde::str")]
    pub proceeds: Decimal,

    /// Quote cost of the matched inventory.
    #[serde(with = "rust_decimal::serde::str")]
    pub cost_basis: Decimal,

    /// `proceeds - cost_basis`.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,

    /// Lot-by-lot breakdown. Empty for the Average Cost engine, which has no
    /// discrete lots — it reports a single synthetic slice instead.
    pub matched_lots: Vec<MatchedLot>,

    /// True when the sell exceeded tracked inventory and the excess was matched
    /// at zero cost. Surfaced as a persistent warning banner in the UI.
    pub oversell: bool,

    /// Base quantity that had no tracked cost basis (0 unless `oversell`).
    #[serde(with = "rust_decimal::serde::str")]
    pub oversell_qty: Decimal,
}

impl RealizedEvent {
    /// Win-rate numerator predicate: a "winning" disposal is a strictly positive
    /// P&L event.
    pub fn is_win(&self) -> bool {
        self.pnl > Decimal::ZERO
    }
}

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// Aggregated state of one trading pair after the whole trade history has been
/// replayed.
///
/// A position with `qty == 0` but non-zero `realized_pnl` is a *closed* position
/// and is rendered in its own UI section rather than being dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub pair: String,
    pub base_asset: String,
    pub quote_asset: String,

    /// Base quantity still held (sum of remaining lot quantities).
    #[serde(with = "rust_decimal::serde::str")]
    pub qty: Decimal,

    /// Quote cost still carried by the open inventory.
    #[serde(with = "rust_decimal::serde::str")]
    pub total_cost: Decimal,

    /// `total_cost / qty`, or zero when the position is flat.
    #[serde(with = "rust_decimal::serde::str")]
    pub avg_cost: Decimal,

    /// Sum of every `RealizedEvent.pnl` for this pair, in the quote asset.
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,

    /// Fees accumulated per asset. Base/quote fees are already reflected in cost
    /// basis and proceeds; they are tracked here purely for reporting. Third-asset
    /// (BNB) fees appear ONLY here — see `FeeKind::ThirdAsset`.
    /// `BTreeMap` keeps the report output deterministically ordered.
    pub fees_by_asset: BTreeMap<String, Decimal>,

    /// Number of BUY / SELL trades that touched this pair.
    pub buy_count: usize,
    pub sell_count: usize,

    /// True if any disposal on this pair triggered the oversell path.
    pub has_oversell: bool,

    /// Remaining open lots, oldest first. Powers the "cost basis breakdown"
    /// drawer in the portfolio table. Empty for the Average Cost engine.
    pub open_lots: Vec<Lot>,
}

impl Position {
    /// Creates an empty position for `pair`. Engines fill it in as they replay.
    pub fn new(pair: &str, base_asset: &str, quote_asset: &str) -> Self {
        Self {
            pair: pair.to_owned(),
            base_asset: base_asset.to_owned(),
            quote_asset: quote_asset.to_owned(),
            qty: Decimal::ZERO,
            total_cost: Decimal::ZERO,
            avg_cost: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            fees_by_asset: BTreeMap::new(),
            buy_count: 0,
            sell_count: 0,
            has_oversell: false,
            open_lots: Vec::new(),
        }
    }

    /// True when the pair has been fully sold down but still carries history.
    pub fn is_closed(&self) -> bool {
        self.qty.is_zero()
    }

    /// Adds `amount` to the running fee total for `asset`.
    pub fn add_fee(&mut self, asset: &str, amount: Decimal) {
        if amount.is_zero() {
            return;
        }
        *self
            .fees_by_asset
            .entry(asset.to_owned())
            .or_insert(Decimal::ZERO) += amount;
    }

    /// Recomputes `avg_cost` from `total_cost` / `qty`, guarding division by zero.
    pub fn refresh_avg_cost(&mut self) {
        self.avg_cost = if self.qty.is_zero() {
            Decimal::ZERO
        } else {
            self.total_cost / self.qty
        };
    }
}

// ---------------------------------------------------------------------------
// Import results
// ---------------------------------------------------------------------------

/// A single rejected CSV row. Import never aborts on these — valid rows are kept
/// and the failures are listed in the UI (AGENTS.md §5.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub file: String,
    pub file_index: usize,
    /// 1-based data row number (header excluded).
    pub row_index: usize,
    /// i18n key so the frontend can translate, e.g. `error.row.unknownQuoteAsset`.
    pub reason_key: String,
    /// Human-readable detail (raw offending value, parser message). Not translated.
    pub detail: String,
    /// Verbatim row content, for the "show raw" toggle.
    pub raw: String,
}

/// Per-file counters shown in the post-import summary panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileImportStats {
    pub file: String,
    pub file_index: usize,
    pub total_rows: usize,
    pub valid_rows: usize,
    pub invalid_rows: usize,
    pub duplicate_rows: usize,
}

/// Result of importing one or more CSV files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub files: Vec<FileImportStats>,
    pub total_rows: usize,
    pub valid_rows: usize,
    pub invalid_rows: usize,
    /// Rows dropped by the deduplication pass (0 when dedup is disabled).
    pub duplicate_rows: usize,
    /// Earliest / latest trade timestamp across all accepted rows.
    pub first_trade_at: Option<DateTime<Utc>>,
    pub last_trade_at: Option<DateTime<Utc>>,
    /// Distinct pairs found, sorted.
    pub pairs: Vec<String>,
    pub errors: Vec<RowError>,
}

// ---------------------------------------------------------------------------
// Prices
// ---------------------------------------------------------------------------

/// One symbol's last price as returned by `/api/v3/ticker/price`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceQuote {
    pub symbol: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
}

/// A price set plus the moment it was fetched. Persisted to disk via
/// `tauri-plugin-store` so a fully offline launch can still show last-known values
/// together with an explicit "as of …" timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceSnapshot {
    /// symbol -> price
    pub prices: BTreeMap<String, Decimal>,
    pub fetched_at: DateTime<Utc>,
    /// True when this snapshot came from the cache because the network call
    /// failed — drives the UI's stale/offline indicator.
    pub stale: bool,
    /// Symbols the API rejected (delisted / renamed). Reported to the user.
    pub failed_symbols: Vec<String>,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Cost-basis method. Both variants are implemented behind the same
/// `engine::PnlEngine` trait, so switching re-runs the full computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PnlMethod {
    #[default]
    Fifo,
    AverageCost,
}

/// What to do when a SELL exceeds the tracked inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OversellPolicy {
    /// Match the excess at zero cost and flag the event. Reflects reality when the
    /// balance arrived via transfer / convert / staking, outside the CSV.
    #[default]
    ZeroCostBasis,
    /// Ignore the excess quantity entirely: only the portion covered by tracked
    /// inventory produces P&L.
    IgnoreExcess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    #[default]
    Tr,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

/// Minimum allowed auto-refresh interval, in seconds. Enforced in both the
/// settings UI and the backend so we stay well inside Binance's rate limits.
pub const MIN_AUTO_REFRESH_SECS: u32 = 30;

/// User-visible configuration, persisted via `tauri-plugin-store`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub pnl_method: PnlMethod,
    pub oversell_policy: OversellPolicy,
    /// Drop byte-identical rows coming from overlapping exports.
    pub deduplicate: bool,
    /// Off by default — the app is offline-first.
    pub auto_refresh: bool,
    /// Clamped to `>= MIN_AUTO_REFRESH_SECS` on load.
    pub auto_refresh_secs: u32,
    pub language: Language,
    pub theme: ThemeMode,
    /// Paths of the CSVs loaded last session, offered on next launch.
    pub last_files: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pnl_method: PnlMethod::default(),
            oversell_policy: OversellPolicy::default(),
            deduplicate: true,
            auto_refresh: false,
            auto_refresh_secs: MIN_AUTO_REFRESH_SECS,
            language: Language::default(),
            theme: ThemeMode::default(),
            last_files: Vec::new(),
        }
    }
}

impl Settings {
    /// Applies invariants that must hold no matter what was read from disk.
    pub fn sanitize(&mut self) {
        if self.auto_refresh_secs < MIN_AUTO_REFRESH_SECS {
            self.auto_refresh_secs = MIN_AUTO_REFRESH_SECS;
        }
    }
}

// ---------------------------------------------------------------------------
// Computed output
// ---------------------------------------------------------------------------

/// Per-pair valuation, produced by combining a `Position` with a `PriceSnapshot`.
/// Kept separate from `Position` because it depends on live prices, while
/// `Position` depends only on the CSV history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionValuation {
    pub position: Position,

    /// Current price in the pair's quote asset. `None` when the symbol could not
    /// be priced at all (never fetched and not in cache).
    #[serde(with = "rust_decimal::serde::str_option")]
    pub current_price: Option<Decimal>,

    /// `qty * current_price`, in the quote asset.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub market_value_quote: Option<Decimal>,

    /// `market_value_quote - total_cost`.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub unrealized_pnl: Option<Decimal>,

    /// Market value converted to USDT. `None` for a non-USDT quote whose own
    /// USDT rate is unavailable — those rows are excluded from dashboard totals
    /// and listed separately (AGENTS.md §6.7).
    #[serde(with = "rust_decimal::serde::str_option")]
    pub market_value_usdt: Option<Decimal>,

    #[serde(with = "rust_decimal::serde::str_option")]
    pub realized_pnl_usdt: Option<Decimal>,

    #[serde(with = "rust_decimal::serde::str_option")]
    pub unrealized_pnl_usdt: Option<Decimal>,

    /// False when the quote asset could not be converted to USDT.
    pub convertible_to_usdt: bool,
}

/// Everything the dashboard cards need, all normalised to USDT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    #[serde(with = "rust_decimal::serde::str")]
    pub total_realized_pnl_usdt: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_unrealized_pnl_usdt: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_portfolio_value_usdt: Decimal,

    /// Winning SELL events / total SELL events, as a percentage 0–100.
    #[serde(with = "rust_decimal::serde::str")]
    pub win_rate: Decimal,
    pub winning_events: usize,
    pub total_sell_events: usize,

    pub total_trades: usize,

    /// Fee totals per asset (exact, un-converted).
    pub total_fees_by_asset: BTreeMap<String, Decimal>,
    /// Approximate USDT value of all fees using CURRENT prices. Always presented
    /// with an "≈" marker because third-asset fees have no historical rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub total_fees_usdt_approx: Decimal,
    /// True when any fee could not be converted, so the UI can qualify the number.
    pub fees_conversion_partial: bool,

    /// Pairs left out of the totals because their quote asset had no USDT rate.
    pub excluded_pairs: Vec<String>,

    pub has_oversell: bool,
    pub price_snapshot: Option<PriceSnapshot>,
}

/// The full computed state handed to the frontend after an import or a settings
/// change. One command returns this; the UI derives every view from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub method: PnlMethod,
    pub summary: DashboardSummary,
    pub positions: Vec<PositionValuation>,
    pub realized_events: Vec<RealizedEvent>,
    pub import: ImportSummary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn quote_assets_are_ordered_by_descending_length() {
        // Suffix matching correctness depends on this ordering: FDUSD must be
        // tested before USDT-style shorter candidates.
        let lengths: Vec<usize> = QUOTE_ASSETS.iter().map(|q| q.len()).collect();
        assert!(
            lengths.windows(2).all(|w| w[0] >= w[1]),
            "QUOTE_ASSETS must be sorted by descending length, got {lengths:?}"
        );
    }

    #[test]
    fn side_parses_case_insensitively() {
        assert_eq!(Side::parse("buy"), Some(Side::Buy));
        assert_eq!(Side::parse(" SELL "), Some(Side::Sell));
        assert_eq!(Side::parse("short"), None);
    }

    #[test]
    fn lot_remaining_cost_is_exact() {
        let lot = Lot {
            qty_remaining: dec!(0.3),
            unit_cost_quote: dec!(2202.20),
            acquired_at: Utc::now(),
            trade_id: 1,
        };
        assert_eq!(lot.remaining_cost(), dec!(660.660));
    }

    #[test]
    fn position_avg_cost_guards_zero_quantity() {
        let mut p = Position::new("ETHUSDT", "ETH", "USDT");
        p.refresh_avg_cost();
        assert_eq!(p.avg_cost, Decimal::ZERO);

        p.qty = dec!(0.3);
        p.total_cost = dec!(660.66);
        p.refresh_avg_cost();
        assert_eq!(p.avg_cost, dec!(2202.20));
    }

    #[test]
    fn settings_clamp_auto_refresh_interval() {
        let mut s = Settings {
            auto_refresh_secs: 5,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.auto_refresh_secs, MIN_AUTO_REFRESH_SECS);
    }
}
