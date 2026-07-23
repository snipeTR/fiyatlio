// src-tauri/tests/golden.rs
//
// End-to-end tests over the CSV fixtures: parser -> engine -> positions.
//
// The expected numbers here are the contract from AGENTS.md §8 and must not be
// "adjusted" to match an implementation change. If a change moves these values,
// either the change is wrong or the contract needs an explicit, deliberate
// revision.
//
// Every comparison is exact `Decimal` equality — no epsilons. `Decimal` is
// deterministic, and the engines are written so cost is conserved to the last
// digit.

use std::path::PathBuf;

use fiyatlio_lib::engine::{engine_for, EngineOutput};
use fiyatlio_lib::models::{OversellPolicy, PnlMethod, Position, Side, Trade};
use fiyatlio_lib::parser;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn load(name: &str) -> Vec<Trade> {
    let parsed = parser::parse_file(&fixture(name), 0)
        .unwrap_or_else(|e| panic!("fixture {name} must parse: {e}"));
    let (trades, _) = parser::merge(vec![parsed], true);
    trades
}

fn run(name: &str, method: PnlMethod, policy: OversellPolicy) -> EngineOutput {
    engine_for(method)
        .replay(&load(name), policy)
        .expect("replay must succeed")
}

fn only_position(output: &EngineOutput) -> &Position {
    assert_eq!(output.positions.len(), 1, "fixture should touch one pair");
    &output.positions[0]
}

/// AGENTS.md §8(d): every unit of cost that entered through a BUY must end up
/// either matched against a disposal or still sitting in open inventory.
fn assert_cost_is_conserved(trades: &[Trade], output: &EngineOutput) {
    let booked_cost: Decimal = trades
        .iter()
        .filter(|t| t.side == Side::Buy)
        .map(|t| fiyatlio_lib::engine::buy_effect(t).1)
        .sum();

    let matched: Decimal = output.realized_events.iter().map(|e| e.cost_basis).sum();
    let remaining: Decimal = output.positions.iter().map(|p| p.total_cost).sum();

    assert_eq!(
        booked_cost,
        matched + remaining,
        "cost conservation violated: booked {booked_cost} != matched {matched} + remaining {remaining}"
    );
}

// ---------------------------------------------------------------------------
// Golden values — FIFO
// ---------------------------------------------------------------------------

#[test]
fn fifo_golden_basic() {
    let output = run(
        "golden_basic.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );

    assert_eq!(output.realized_events.len(), 1);
    let event = &output.realized_events[0];

    // lot1 cost 2000 + 2 USDT fee = 2002; lot2 cost 1100 + 1.1 = 1101.1.
    // Sell 1.2 ETH: all of lot1 plus 0.2 of lot2 -> 2002 + 1101.1 * 0.4 = 2442.44.
    assert_eq!(event.proceeds, dec!(2997));
    assert_eq!(event.cost_basis, dec!(2442.44));
    assert_eq!(event.pnl, dec!(554.56));
    assert!(!event.oversell);

    // The lot-by-lot breakdown is what makes the number auditable.
    assert_eq!(event.matched_lots.len(), 2);
    assert_eq!(event.matched_lots[0].buy_trade_id, Some(0));
    assert_eq!(event.matched_lots[0].qty, dec!(1.0));
    assert_eq!(event.matched_lots[0].cost, dec!(2002));
    assert_eq!(event.matched_lots[1].buy_trade_id, Some(1));
    assert_eq!(event.matched_lots[1].qty, dec!(0.2));
    assert_eq!(event.matched_lots[1].cost, dec!(440.44));

    let position = only_position(&output);
    assert_eq!(position.qty, dec!(0.3));
    assert_eq!(position.total_cost, dec!(660.66));
    assert_eq!(position.avg_cost, dec!(2202.20));
    assert_eq!(position.realized_pnl, dec!(554.56));
    assert_eq!(position.buy_count, 2);
    assert_eq!(position.sell_count, 1);
    assert!(!position.has_oversell);

    // Only the first lot was fully consumed, so one lot remains open.
    assert_eq!(position.open_lots.len(), 1);
    assert_eq!(position.open_lots[0].trade_id, 1);
    assert_eq!(position.open_lots[0].qty_remaining, dec!(0.3));
    assert_eq!(position.open_lots[0].cost_remaining, dec!(660.66));

    // All fees were paid in USDT: 2 + 1.1 + 3.
    assert_eq!(position.fees_by_asset.get("USDT"), Some(&dec!(6.1)));

    assert_cost_is_conserved(&load("golden_basic.csv"), &output);
}

// ---------------------------------------------------------------------------
// Golden values — Average Cost
// ---------------------------------------------------------------------------

#[test]
fn average_cost_golden_basic() {
    let output = run(
        "golden_basic.csv",
        PnlMethod::AverageCost,
        OversellPolicy::ZeroCostBasis,
    );

    assert_eq!(output.realized_events.len(), 1);
    let event = &output.realized_events[0];

    // Pool: 1.5 ETH / 3103.1 USDT. Selling 1.2 charges 3103.1 * 1.2 / 1.5.
    assert_eq!(event.proceeds, dec!(2997));
    assert_eq!(event.cost_basis, dec!(2482.48));
    assert_eq!(event.pnl, dec!(514.52));

    let position = only_position(&output);
    assert_eq!(position.qty, dec!(0.3));
    assert_eq!(position.total_cost, dec!(620.62));
    assert_eq!(position.realized_pnl, dec!(514.52));
    // Average Cost keeps no lot history.
    assert!(position.open_lots.is_empty());

    assert_cost_is_conserved(&load("golden_basic.csv"), &output);
}

#[test]
fn the_two_methods_disagree_by_the_documented_amount() {
    // Guards against the engines accidentally collapsing into the same code path.
    let fifo = run(
        "golden_basic.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );
    let avg = run(
        "golden_basic.csv",
        PnlMethod::AverageCost,
        OversellPolicy::ZeroCostBasis,
    );

    assert_eq!(
        fifo.realized_events[0].pnl - avg.realized_events[0].pnl,
        dec!(40.04)
    );
}

// ---------------------------------------------------------------------------
// Fee rules
// ---------------------------------------------------------------------------

#[test]
fn base_asset_fee_reduces_the_lot_quantity_not_the_cost() {
    let output = run(
        "base_asset_fee.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );

    let position = only_position(&output);
    // 1.0 ETH executed, 0.001 ETH fee -> the lot opens at 0.999 ETH...
    assert_eq!(position.qty, dec!(0.999));
    // ...while the full 2000 USDT paid stays in cost basis.
    assert_eq!(position.total_cost, dec!(2000));
    assert_eq!(position.fees_by_asset.get("ETH"), Some(&dec!(0.001)));

    // 2000 / 0.999 does not terminate — this is precisely the case a unit-cost
    // representation would round away.
    assert_cost_is_conserved(&load("base_asset_fee.csv"), &output);
}

// ---------------------------------------------------------------------------
// Same-second ordering
// ---------------------------------------------------------------------------

#[test]
fn same_second_partial_fills_are_matched_in_csv_order() {
    let output = run(
        "same_second_partials.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );

    let event = &output.realized_events[0];
    // Selling 0.5 must take all of the 0.4 lot, then 0.1 from the 0.3 lot —
    // in the order the rows appear, not price order.
    assert_eq!(event.matched_lots.len(), 2);
    assert_eq!(event.matched_lots[0].buy_trade_id, Some(0));
    assert_eq!(event.matched_lots[0].qty, dec!(0.4));
    assert_eq!(event.matched_lots[0].cost, dec!(800));
    assert_eq!(event.matched_lots[1].buy_trade_id, Some(1));
    assert_eq!(event.matched_lots[1].qty, dec!(0.1));
    assert_eq!(event.matched_lots[1].cost, dec!(210));

    assert_eq!(event.cost_basis, dec!(1010));
    assert_eq!(event.proceeds, dec!(1250));
    assert_eq!(event.pnl, dec!(240));

    let position = only_position(&output);
    assert_eq!(position.qty, dec!(0.5));
    assert_eq!(position.total_cost, dec!(1080));
    // The partially consumed lot stays at the front, ahead of the untouched one.
    assert_eq!(position.open_lots.len(), 2);
    assert_eq!(position.open_lots[0].trade_id, 1);
    assert_eq!(position.open_lots[0].qty_remaining, dec!(0.2));
    assert_eq!(position.open_lots[1].trade_id, 2);

    assert_cost_is_conserved(&load("same_second_partials.csv"), &output);
}

// ---------------------------------------------------------------------------
// Oversell
// ---------------------------------------------------------------------------

#[test]
fn oversell_zero_cost_basis_flags_and_matches_the_excess_at_zero() {
    let output = run(
        "oversell.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );

    let event = &output.realized_events[0];
    assert!(event.oversell);
    assert_eq!(event.oversell_qty, dec!(0.5));
    assert_eq!(event.qty, dec!(1.0));
    assert_eq!(event.proceeds, dec!(2500));
    assert_eq!(event.cost_basis, dec!(1000));
    assert_eq!(event.pnl, dec!(1500));

    // The synthetic zero-cost slice is reported alongside the real lot.
    assert_eq!(event.matched_lots.len(), 2);
    assert!(!event.matched_lots[0].synthetic);
    assert!(event.matched_lots[1].synthetic);
    assert_eq!(event.matched_lots[1].buy_trade_id, None);
    assert_eq!(event.matched_lots[1].cost, Decimal::ZERO);

    let position = only_position(&output);
    assert!(position.has_oversell);
    assert_eq!(position.qty, Decimal::ZERO);
    assert_eq!(position.total_cost, Decimal::ZERO);
    // A flat position still carries its history — the UI lists it under
    // "closed positions" rather than dropping it.
    assert!(position.is_closed());
    assert_eq!(position.realized_pnl, dec!(1500));
}

#[test]
fn oversell_ignore_excess_counts_only_the_covered_portion() {
    let output = run(
        "oversell.csv",
        PnlMethod::Fifo,
        OversellPolicy::IgnoreExcess,
    );

    let event = &output.realized_events[0];
    assert!(event.oversell);
    assert_eq!(event.oversell_qty, dec!(0.5));
    // Only the half backed by inventory is counted, proceeds scaled to match.
    assert_eq!(event.qty, dec!(0.5));
    assert_eq!(event.proceeds, dec!(1250));
    assert_eq!(event.cost_basis, dec!(1000));
    assert_eq!(event.pnl, dec!(250));
    assert_eq!(event.matched_lots.len(), 1);
    assert!(!event.matched_lots[0].synthetic);
}

#[test]
fn oversell_behaves_identically_under_average_cost() {
    let zero = run(
        "oversell.csv",
        PnlMethod::AverageCost,
        OversellPolicy::ZeroCostBasis,
    );
    assert_eq!(zero.realized_events[0].pnl, dec!(1500));
    assert!(zero.realized_events[0].oversell);

    let ignore = run(
        "oversell.csv",
        PnlMethod::AverageCost,
        OversellPolicy::IgnoreExcess,
    );
    assert_eq!(ignore.realized_events[0].pnl, dec!(250));

    // With a single lot there is nothing for FIFO to order differently, so both
    // methods must agree exactly.
    let fifo = run(
        "oversell.csv",
        PnlMethod::Fifo,
        OversellPolicy::ZeroCostBasis,
    );
    assert_eq!(zero.realized_events[0].pnl, fifo.realized_events[0].pnl);
}

// ---------------------------------------------------------------------------
// Malformed input
// ---------------------------------------------------------------------------

#[test]
fn malformed_rows_are_skipped_and_the_rest_still_computes() {
    let parsed = parser::parse_file(&fixture("malformed_rows.csv"), 0).expect("file parses");
    let (trades, summary) = parser::merge(vec![parsed], true);

    assert_eq!(summary.total_rows, 6);
    assert_eq!(summary.valid_rows, 2);
    assert_eq!(summary.invalid_rows, 4);
    assert_eq!(trades.len(), 2);

    // The two surviving rows are a complete round trip: buy 1 ETH at 2000
    // (+2 USDT fee), sell it at 2500.
    let output = engine_for(PnlMethod::Fifo)
        .replay(&trades, OversellPolicy::ZeroCostBasis)
        .expect("replay must succeed");

    assert_eq!(output.realized_events.len(), 1);
    assert_eq!(output.realized_events[0].cost_basis, dec!(2002));
    assert_eq!(output.realized_events[0].proceeds, dec!(2500));
    assert_eq!(output.realized_events[0].pnl, dec!(498));
    assert_cost_is_conserved(&trades, &output);
}

// ---------------------------------------------------------------------------
// Invariant sweep
// ---------------------------------------------------------------------------

#[test]
fn cost_is_conserved_across_every_fixture_method_and_policy() {
    let fixtures = [
        "golden_basic.csv",
        "base_asset_fee.csv",
        "same_second_partials.csv",
        "oversell.csv",
        "malformed_rows.csv",
    ];

    for name in fixtures {
        let trades = load(name);
        for method in [PnlMethod::Fifo, PnlMethod::AverageCost] {
            for policy in [OversellPolicy::ZeroCostBasis, OversellPolicy::IgnoreExcess] {
                let output = engine_for(method)
                    .replay(&trades, policy)
                    .expect("replay must succeed");

                assert_cost_is_conserved(&trades, &output);

                // Realized P&L on each position must equal the sum of its events.
                for position in &output.positions {
                    let from_events: Decimal = output
                        .realized_events
                        .iter()
                        .filter(|e| e.pair == position.pair)
                        .map(|e| e.pnl)
                        .sum();
                    assert_eq!(
                        position.realized_pnl, from_events,
                        "{name} / {method:?} / {policy:?}: position P&L must equal its events"
                    );
                }
            }
        }
    }
}

#[test]
fn switching_method_never_changes_quantities_only_cost_attribution() {
    // Both engines see the same fills, so the surviving quantity must match even
    // though the cost split does not.
    for name in ["golden_basic.csv", "same_second_partials.csv"] {
        let fifo = run(name, PnlMethod::Fifo, OversellPolicy::ZeroCostBasis);
        let avg = run(name, PnlMethod::AverageCost, OversellPolicy::ZeroCostBasis);

        let fifo_qty: Vec<Decimal> = fifo.positions.iter().map(|p| p.qty).collect();
        let avg_qty: Vec<Decimal> = avg.positions.iter().map(|p| p.qty).collect();
        assert_eq!(
            fifo_qty, avg_qty,
            "{name}: quantities must be method-agnostic"
        );
    }
}
