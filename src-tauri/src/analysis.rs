// src-tauri/src/analysis.rs
//
// Joins the pure, price-independent engine output with a live price snapshot to
// produce the single `AnalysisResult` the whole UI renders from.
//
// The split matters: engine output depends only on the CSV history and is
// deterministic, while everything in this module depends on prices that change
// between refreshes. Keeping them apart means a price refresh never re-runs the
// cost-basis replay, and a method change never needs the network.
//
// Currency handling (AGENTS.md §6.7): P&L for a pair is always denominated in
// that pair's own quote asset. Conversion to USDT for dashboard totals happens
// here, at the CURRENT rate — which is an approximation for realized P&L that
// was earned in, say, BTC at a different historical rate. Pairs whose quote
// asset cannot be priced in USDT are excluded from the totals and listed
// separately rather than being folded in at a guessed rate.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::binance::{conversion_routes, USD_PEGGED_FALLBACK};
use crate::engine::{engine_for, EngineOutput};
use crate::error::AppResult;
use crate::models::{
    AnalysisResult, DashboardSummary, ImportSummary, Position, PositionValuation, PriceSnapshot,
    Settings, Trade,
};

/// Every Binance symbol needed to value `positions` and convert their totals.
///
/// Three groups:
///   * the pair itself, for open positions (a flat position needs no price);
///   * `<quote>USDT`, to bring a non-USDT quote asset into the reporting currency;
///   * `<feeAsset>USDT`, for the approximate fee total on the dashboard.
pub fn required_symbols(positions: &[Position]) -> Vec<String> {
    let mut symbols: BTreeSet<String> = BTreeSet::new();

    for position in positions {
        if position.qty > Decimal::ZERO {
            symbols.insert(position.pair.clone());
        }
        for route in conversion_routes(&position.quote_asset) {
            symbols.insert(route.symbol);
        }
        for asset in position.fees_by_asset.keys() {
            for route in conversion_routes(asset) {
                symbols.insert(route.symbol);
            }
        }
    }

    symbols.into_iter().collect()
}

/// Rate that converts one unit of `asset` into USDT, or `None` when unavailable.
///
/// Tries every live route first — including the inverted `USDT<fiat>` form —
/// and only then falls back to the peg for a retired USD stablecoin. Order
/// matters: a real market always beats an assumed one.
fn usdt_rate(asset: &str, prices: &BTreeMap<String, Decimal>) -> Option<Decimal> {
    let routes = conversion_routes(asset);
    if routes.is_empty() {
        // The reporting asset converts to itself.
        return Some(Decimal::ONE);
    }

    for route in routes {
        let Some(price) = prices.get(&route.symbol) else {
            continue;
        };
        if !route.invert {
            return Some(*price);
        }
        if !price.is_zero() {
            return Some(Decimal::ONE / *price);
        }
    }

    let upper = asset.trim().to_ascii_uppercase();
    if USD_PEGGED_FALLBACK.contains(&upper.as_str()) {
        return Some(Decimal::ONE);
    }

    None
}

/// Builds the full analysis for the current data set.
///
/// `prices` is optional: with no snapshot at all (first launch, fully offline)
/// every valuation field comes back `None` and the UI shows the position without
/// a market value rather than showing a wrong one.
pub fn build_analysis(
    trades: &[Trade],
    import: &ImportSummary,
    settings: &Settings,
    prices: Option<&PriceSnapshot>,
) -> AppResult<AnalysisResult> {
    let engine = engine_for(settings.pnl_method);
    let output: EngineOutput = engine.replay(trades, settings.oversell_policy)?;

    let empty = BTreeMap::new();
    let price_map = prices.map(|s| &s.prices).unwrap_or(&empty);

    let mut valuations: Vec<PositionValuation> = Vec::with_capacity(output.positions.len());
    let mut excluded_pairs: Vec<String> = Vec::new();

    let mut total_realized_usdt = Decimal::ZERO;
    let mut total_unrealized_usdt = Decimal::ZERO;
    let mut total_value_usdt = Decimal::ZERO;

    for position in output.positions {
        let current_price = price_map.get(&position.pair).copied();
        let market_value_quote = current_price.map(|price| position.qty * price);
        let unrealized_pnl = market_value_quote.map(|value| value - position.total_cost);

        let rate = usdt_rate(&position.quote_asset, price_map);
        let convertible = rate.is_some();
        if !convertible {
            excluded_pairs.push(position.pair.clone());
        }

        let market_value_usdt = match (market_value_quote, rate) {
            (Some(value), Some(rate)) => Some(value * rate),
            _ => None,
        };
        let realized_pnl_usdt = rate.map(|rate| position.realized_pnl * rate);
        let unrealized_pnl_usdt = match (unrealized_pnl, rate) {
            (Some(pnl), Some(rate)) => Some(pnl * rate),
            _ => None,
        };

        // Only convertible pairs contribute to the headline numbers; the rest are
        // reported separately so the totals are never quietly incomplete.
        total_realized_usdt += realized_pnl_usdt.unwrap_or(Decimal::ZERO);
        total_unrealized_usdt += unrealized_pnl_usdt.unwrap_or(Decimal::ZERO);
        total_value_usdt += market_value_usdt.unwrap_or(Decimal::ZERO);

        valuations.push(PositionValuation {
            position,
            current_price,
            market_value_quote,
            unrealized_pnl,
            market_value_usdt,
            realized_pnl_usdt,
            unrealized_pnl_usdt,
            convertible_to_usdt: convertible,
        });
    }

    // Open positions first (largest value at the top), closed ones after — the
    // order the portfolio table renders in.
    valuations.sort_by(|a, b| {
        let a_open = a.position.qty > Decimal::ZERO;
        let b_open = b.position.qty > Decimal::ZERO;
        b_open
            .cmp(&a_open)
            .then_with(|| {
                b.market_value_usdt
                    .unwrap_or(Decimal::ZERO)
                    .cmp(&a.market_value_usdt.unwrap_or(Decimal::ZERO))
            })
            .then_with(|| a.position.pair.cmp(&b.position.pair))
    });

    // --- fees -------------------------------------------------------------
    let mut total_fees_by_asset: BTreeMap<String, Decimal> = BTreeMap::new();
    for valuation in &valuations {
        for (asset, amount) in &valuation.position.fees_by_asset {
            *total_fees_by_asset
                .entry(asset.clone())
                .or_insert(Decimal::ZERO) += *amount;
        }
    }

    let mut total_fees_usdt_approx = Decimal::ZERO;
    let mut fees_conversion_partial = false;
    for (asset, amount) in &total_fees_by_asset {
        match usdt_rate(asset, price_map) {
            Some(rate) => total_fees_usdt_approx += *amount * rate,
            // A third-asset fee with no live rate cannot be valued at all. The UI
            // qualifies the total rather than pretending the fee was zero.
            None => fees_conversion_partial = true,
        }
    }

    // --- win rate ---------------------------------------------------------
    let total_sell_events = output.realized_events.len();
    let winning_events = output
        .realized_events
        .iter()
        .filter(|event| event.is_win())
        .count();

    let win_rate = if total_sell_events == 0 {
        Decimal::ZERO
    } else {
        Decimal::from(winning_events) * Decimal::ONE_HUNDRED / Decimal::from(total_sell_events)
    };

    // --- oversell attribution --------------------------------------------
    // Quantifying this is what turns "why is my profit so large?" into an
    // answerable question, so the numbers are computed here rather than left
    // for the UI to derive.
    let mut oversell_event_count = 0usize;
    let mut oversell_pnl_usdt = Decimal::ZERO;
    let mut oversell_qty_by_asset: BTreeMap<String, Decimal> = BTreeMap::new();

    for event in &output.realized_events {
        if !event.oversell {
            continue;
        }
        oversell_event_count += 1;

        if let Some(rate) = usdt_rate(&event.quote_asset, price_map) {
            oversell_pnl_usdt += event.pnl * rate;
        }
        *oversell_qty_by_asset
            .entry(event.base_asset.clone())
            .or_insert(Decimal::ZERO) += event.oversell_qty;
    }

    let has_oversell = oversell_event_count > 0;

    let summary = DashboardSummary {
        total_realized_pnl_usdt: total_realized_usdt,
        total_unrealized_pnl_usdt: total_unrealized_usdt,
        total_portfolio_value_usdt: total_value_usdt,
        win_rate,
        winning_events,
        total_sell_events,
        total_trades: trades.len(),
        total_fees_by_asset,
        total_fees_usdt_approx,
        fees_conversion_partial,
        excluded_pairs,
        has_oversell,
        oversell_event_count,
        oversell_pnl_usdt,
        oversell_qty_by_asset,
        price_snapshot: prices.cloned(),
    };

    Ok(AnalysisResult {
        method: settings.pnl_method,
        summary,
        positions: valuations,
        realized_events: output.realized_events,
        import: import.clone(),
    })
}

/// The analysis for "no CSV loaded yet".
pub fn empty_analysis(
    settings: &Settings,
    prices: Option<&PriceSnapshot>,
) -> AppResult<AnalysisResult> {
    build_analysis(&[], &ImportSummary::empty(), settings, prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PnlMethod, Side};
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    fn trade(
        id: u64,
        pair: &str,
        base: &str,
        quote: &str,
        side: Side,
        qty: &str,
        amount: &str,
    ) -> Trade {
        Trade {
            id,
            timestamp_utc: Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            pair: pair.into(),
            base_asset: base.into(),
            quote_asset: quote.into(),
            side,
            price: dec!(1),
            qty: qty.parse().expect("qty"),
            quote_amount: amount.parse().expect("amount"),
            fee_amount: Decimal::ZERO,
            fee_asset: quote.into(),
            source_file: "t.csv".into(),
            file_index: 0,
            row_index: id as usize + 1,
        }
    }

    fn snapshot(pairs: &[(&str, Decimal)]) -> PriceSnapshot {
        PriceSnapshot {
            prices: pairs.iter().map(|(s, p)| ((*s).to_string(), *p)).collect(),
            fetched_at: Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
            stale: false,
            failed_symbols: Vec::new(),
        }
    }

    #[test]
    fn values_an_open_position_against_the_live_price() {
        let trades = vec![trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "2", "4000")];
        let prices = snapshot(&[("ETHUSDT", dec!(2500))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        let position = &result.positions[0];
        assert_eq!(position.market_value_quote, Some(dec!(5000)));
        assert_eq!(position.unrealized_pnl, Some(dec!(1000)));
        assert_eq!(result.summary.total_portfolio_value_usdt, dec!(5000));
        assert_eq!(result.summary.total_unrealized_pnl_usdt, dec!(1000));
    }

    #[test]
    fn converts_a_non_usdt_quote_through_its_own_usdt_pair() {
        // 0.5 ETH held in an ETHBTC position, ETH/BTC = 0.04, BTC/USDT = 60000.
        let trades = vec![trade(0, "ETHBTC", "ETH", "BTC", Side::Buy, "0.5", "0.02")];
        let prices = snapshot(&[("ETHBTC", dec!(0.04)), ("BTCUSDT", dec!(60000))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        let position = &result.positions[0];
        assert!(position.convertible_to_usdt);
        assert_eq!(position.market_value_quote, Some(dec!(0.020)));
        assert_eq!(position.market_value_usdt, Some(dec!(1200.000)));
        assert!(result.summary.excluded_pairs.is_empty());
    }

    #[test]
    fn a_try_quoted_pair_converts_through_the_inverted_usdttry_symbol() {
        // 100 BNB held in a BNBTRY position, BNB/TRY = 30,000, USDT/TRY = 40.
        // The USDT value is 100 * 30000 / 40 = 75,000.
        let trades = vec![trade(
            0,
            "BNBTRY",
            "BNB",
            "TRY",
            Side::Buy,
            "100",
            "2000000",
        )];
        let prices = snapshot(&[("BNBTRY", dec!(30000)), ("USDTTRY", dec!(40))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        let position = &result.positions[0];
        assert!(
            position.convertible_to_usdt,
            "TRY must convert via USDTTRY, not the non-existent TRYUSDT"
        );
        assert_eq!(position.market_value_usdt, Some(dec!(75000)));
        assert!(result.summary.excluded_pairs.is_empty());
    }

    #[test]
    fn a_retired_usd_stablecoin_falls_back_to_its_peg() {
        // BUSDUSDT no longer trades, but BUSD redeemed 1:1 for its whole life.
        let trades = vec![trade(0, "TRXBUSD", "TRX", "BUSD", Side::Buy, "1000", "100")];
        let prices = snapshot(&[("TRXBUSD", dec!(0.2))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        assert!(result.positions[0].convertible_to_usdt);
        assert_eq!(result.positions[0].market_value_usdt, Some(dec!(200.0)));
    }

    #[test]
    fn a_live_market_always_beats_the_peg_fallback() {
        // If BUSDUSDT ever quotes again, that quote wins over the assumed 1.0.
        let trades = vec![trade(0, "TRXBUSD", "TRX", "BUSD", Side::Buy, "1000", "100")];
        let prices = snapshot(&[("TRXBUSD", dec!(0.2)), ("BUSDUSDT", dec!(0.98))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        assert_eq!(result.positions[0].market_value_usdt, Some(dec!(196.00)));
    }

    #[test]
    fn excludes_a_pair_whose_quote_asset_cannot_be_priced_in_usdt() {
        let trades = vec![trade(0, "ETHBTC", "ETH", "BTC", Side::Buy, "0.5", "0.02")];
        // ETHBTC is priced, but BTCUSDT is missing — the position can be valued
        // in BTC yet not converted.
        let prices = snapshot(&[("ETHBTC", dec!(0.04))]);

        let result = build_analysis(
            &trades,
            &ImportSummary::empty(),
            &Settings::default(),
            Some(&prices),
        )
        .expect("analysis builds");

        let position = &result.positions[0];
        assert!(!position.convertible_to_usdt);
        assert_eq!(position.market_value_quote, Some(dec!(0.020)));
        assert_eq!(position.market_value_usdt, None);
        // Excluded, not folded in at a guessed rate.
        assert_eq!(result.summary.total_portfolio_value_usdt, Decimal::ZERO);
        assert_eq!(result.summary.excluded_pairs, vec!["ETHBTC".to_string()]);
    }

    #[test]
    fn with_no_price_snapshot_positions_have_no_valuation() {
        let trades = vec![trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "2", "4000")];
        let result = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        let position = &result.positions[0];
        assert_eq!(position.current_price, None);
        assert_eq!(position.market_value_quote, None);
        assert_eq!(position.unrealized_pnl, None);
        // Cost basis is still known — it never depended on prices.
        assert_eq!(position.position.total_cost, dec!(4000));
    }

    #[test]
    fn oversell_contribution_is_quantified_separately() {
        // Sell 1 ETH with only 0.4 ETH of tracked acquisition behind it. Under
        // the default zero-cost policy the uncovered 0.6 produces P&L out of
        // nothing, and the summary has to say so.
        let trades = vec![
            trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "0.4", "800"),
            trade(1, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "2500"),
        ];
        let result = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        assert!(result.summary.has_oversell);
        assert_eq!(result.summary.oversell_event_count, 1);
        // 2500 proceeds against 800 of real cost.
        assert_eq!(result.summary.total_realized_pnl_usdt, dec!(1700));
        // ...all of which came from an oversell event.
        assert_eq!(result.summary.oversell_pnl_usdt, dec!(1700));
        assert_eq!(
            result.summary.oversell_qty_by_asset.get("ETH"),
            Some(&dec!(0.6))
        );
    }

    #[test]
    fn a_clean_history_reports_no_oversell_contribution() {
        let trades = vec![
            trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "2", "4000"),
            trade(1, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "2500"),
        ];
        let result = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        assert!(!result.summary.has_oversell);
        assert_eq!(result.summary.oversell_event_count, 0);
        assert_eq!(result.summary.oversell_pnl_usdt, Decimal::ZERO);
        assert!(result.summary.oversell_qty_by_asset.is_empty());
    }

    #[test]
    fn win_rate_counts_profitable_disposals() {
        let trades = vec![
            trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "2", "4000"),
            trade(1, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "2500"),
            trade(2, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "1000"),
        ];
        let result = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        assert_eq!(result.summary.total_sell_events, 2);
        assert_eq!(result.summary.winning_events, 1);
        assert_eq!(result.summary.win_rate, dec!(50));
    }

    #[test]
    fn required_symbols_skips_flat_positions_and_usdt_self_conversion() {
        let trades = vec![
            trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "1", "2000"),
            trade(1, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "2500"),
            trade(2, "ETHBTC", "ETH", "BTC", Side::Buy, "1", "0.04"),
        ];
        let result = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        let positions: Vec<Position> = result
            .positions
            .iter()
            .map(|v| v.position.clone())
            .collect();
        let symbols = required_symbols(&positions);

        // ETHUSDT is flat, so its own price is not needed; USDT never needs a
        // conversion pair; ETHBTC is open and BTC needs BTCUSDT.
        assert_eq!(symbols, vec!["BTCUSDT".to_string(), "ETHBTC".to_string()]);
    }

    #[test]
    fn switching_method_changes_the_totals_without_touching_prices() {
        let trades = vec![
            trade(0, "ETHUSDT", "ETH", "USDT", Side::Buy, "1", "2000"),
            trade(1, "ETHUSDT", "ETH", "USDT", Side::Buy, "1", "3000"),
            trade(2, "ETHUSDT", "ETH", "USDT", Side::Sell, "1", "4000"),
        ];

        let fifo = build_analysis(&trades, &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");

        let avg_settings = Settings {
            pnl_method: PnlMethod::AverageCost,
            ..Settings::default()
        };
        let avg = build_analysis(&trades, &ImportSummary::empty(), &avg_settings, None)
            .expect("analysis builds");

        // FIFO consumes the 2000 lot: 4000 - 2000 = 2000.
        assert_eq!(fifo.summary.total_realized_pnl_usdt, dec!(2000));
        // Average cost charges 2500: 4000 - 2500 = 1500.
        assert_eq!(avg.summary.total_realized_pnl_usdt, dec!(1500));
    }
}
