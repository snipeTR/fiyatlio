// src-tauri/src/engine/mod.rs
//
// Cost-basis engines. Both `fifo` and `avg_cost` implement the same `PnlEngine`
// trait, which is what makes the Settings toggle a pure re-computation with no
// branching anywhere else in the codebase (AGENTS.md §6.6).
//
// This module also owns the fee rules, because getting them subtly different
// between the two engines would be the easiest way to make the numbers disagree.

pub mod avg_cost;
pub mod fifo;

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::error::AppResult;
use crate::models::{FeeKind, OversellPolicy, PnlMethod, Position, RealizedEvent, Side, Trade};

/// Result of replaying a trade history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOutput {
    /// One entry per pair, ordered by pair symbol for deterministic output.
    pub positions: Vec<Position>,
    /// Every disposal, in chronological order.
    pub realized_events: Vec<RealizedEvent>,
}

/// A cost-basis method.
pub trait PnlEngine {
    fn method(&self) -> PnlMethod;

    /// Replays `trades` — which must already be sorted by
    /// `(timestamp_utc, file_index, row_index)`, as [`crate::parser::merge`]
    /// guarantees — and reports the resulting positions and realized events.
    fn replay(&self, trades: &[Trade], policy: OversellPolicy) -> AppResult<EngineOutput>;
}

/// Returns the engine for `method`. The only place the method is branched on.
pub fn engine_for(method: PnlMethod) -> Box<dyn PnlEngine> {
    match method {
        PnlMethod::Fifo => Box::new(fifo::FifoEngine),
        PnlMethod::AverageCost => Box::new(avg_cost::AverageCostEngine),
    }
}

// ---------------------------------------------------------------------------
// Fee rules (AGENTS.md §6.4)
// ---------------------------------------------------------------------------

/// Net base quantity acquired by a BUY, and the quote cost to book against it.
///
/// | fee asset | quantity          | cost             |
/// |-----------|-------------------|------------------|
/// | base      | `executed - fee`  | `amount`         |
/// | quote     | `executed`        | `amount + fee`   |
/// | other/none| `executed`        | `amount`         |
///
/// The third-asset case (typically BNB) deliberately leaves cost untouched: the
/// historical BNB price at fill time is not in the CSV, so folding it in would
/// mean inventing a rate. Those fees are reported separately instead.
pub fn buy_effect(trade: &Trade) -> (Decimal, Decimal) {
    match trade.fee_kind() {
        FeeKind::Base => (trade.qty - trade.fee_amount, trade.quote_amount),
        FeeKind::Quote => (trade.qty, trade.quote_amount + trade.fee_amount),
        FeeKind::ThirdAsset | FeeKind::None => (trade.qty, trade.quote_amount),
    }
}

/// Base quantity disposed of by a SELL, and the net quote proceeds.
///
/// | fee asset | quantity   | proceeds                |
/// |-----------|------------|-------------------------|
/// | quote     | `executed` | `amount - fee`          |
/// | base      | `executed` | `amount - fee * price`  |
/// | other/none| `executed` | `amount`                |
///
/// A base-asset fee on a SELL is rare. The quantity leaving inventory is still
/// the full `executed` amount; the fee is valued at the fill price — the one
/// moment where that conversion is exact — and deducted from proceeds.
pub fn sell_effect(trade: &Trade) -> (Decimal, Decimal) {
    match trade.fee_kind() {
        FeeKind::Quote => (trade.qty, trade.quote_amount - trade.fee_amount),
        FeeKind::Base => (
            trade.qty,
            trade.quote_amount - trade.fee_amount * trade.price,
        ),
        FeeKind::ThirdAsset | FeeKind::None => (trade.qty, trade.quote_amount),
    }
}

// ---------------------------------------------------------------------------
// Shared bookkeeping
// ---------------------------------------------------------------------------

/// Per-pair accumulator shared by both engines: the parts of `Position` that do
/// not depend on how cost basis is tracked.
pub(crate) struct PairBook {
    pub position: Position,
}

impl PairBook {
    pub fn new(trade: &Trade) -> Self {
        Self {
            position: Position::new(&trade.pair, &trade.base_asset, &trade.quote_asset),
        }
    }

    /// Records the side counter and the fee, for every trade regardless of how
    /// that fee was (or was not) folded into cost basis.
    pub fn record(&mut self, trade: &Trade) {
        match trade.side {
            Side::Buy => self.position.buy_count += 1,
            Side::Sell => self.position.sell_count += 1,
        }
        self.position.add_fee(&trade.fee_asset, trade.fee_amount);
    }
}

/// Collects finished books into the deterministic, pair-sorted output shape.
pub(crate) fn finish(books: BTreeMap<String, PairBook>) -> Vec<Position> {
    books
        .into_values()
        .map(|book| {
            let mut position = book.position;
            position.refresh_avg_cost();
            position
        })
        .collect()
}

/// Splits an oversell into the part real inventory can cover and the excess,
/// applying `policy`.
///
/// Returns `(effective_qty, effective_proceeds)`. Under
/// [`OversellPolicy::IgnoreExcess`] the proceeds are scaled to the covered
/// fraction — otherwise the uncovered quantity would contribute revenue with no
/// matching cost and inflate P&L exactly as much as the zero-cost policy does,
/// which would make the two policies indistinguishable.
pub(crate) fn apply_oversell_policy(
    policy: OversellPolicy,
    sold_qty: Decimal,
    proceeds: Decimal,
    covered_qty: Decimal,
) -> (Decimal, Decimal) {
    match policy {
        OversellPolicy::ZeroCostBasis => (sold_qty, proceeds),
        OversellPolicy::IgnoreExcess => {
            if sold_qty.is_zero() {
                (Decimal::ZERO, Decimal::ZERO)
            } else {
                (covered_qty, proceeds * covered_qty / sold_qty)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn trade(side: Side, qty: &str, amount: &str, fee: &str, fee_asset: &str) -> Trade {
        Trade {
            id: 0,
            timestamp_utc: Utc::now(),
            pair: "ETHUSDT".into(),
            base_asset: "ETH".into(),
            quote_asset: "USDT".into(),
            side,
            price: dec!(2000),
            qty: qty.parse().expect("qty"),
            quote_amount: amount.parse().expect("amount"),
            fee_amount: fee.parse().expect("fee"),
            fee_asset: fee_asset.into(),
            source_file: "t.csv".into(),
            file_index: 0,
            row_index: 1,
        }
    }

    #[test]
    fn buy_with_quote_fee_adds_the_fee_to_cost() {
        let (qty, cost) = buy_effect(&trade(Side::Buy, "1.0", "2000", "2", "USDT"));
        assert_eq!(qty, dec!(1.0));
        assert_eq!(cost, dec!(2002));
    }

    #[test]
    fn buy_with_base_fee_reduces_the_acquired_quantity() {
        let (qty, cost) = buy_effect(&trade(Side::Buy, "1.0", "2000", "0.001", "ETH"));
        assert_eq!(qty, dec!(0.999));
        assert_eq!(cost, dec!(2000));
    }

    #[test]
    fn buy_with_third_asset_fee_leaves_cost_basis_untouched() {
        let (qty, cost) = buy_effect(&trade(Side::Buy, "1.0", "2000", "0.05", "BNB"));
        assert_eq!(qty, dec!(1.0));
        assert_eq!(cost, dec!(2000));
    }

    #[test]
    fn sell_with_quote_fee_reduces_proceeds() {
        let (qty, proceeds) = sell_effect(&trade(Side::Sell, "1.2", "3000", "3", "USDT"));
        assert_eq!(qty, dec!(1.2));
        assert_eq!(proceeds, dec!(2997));
    }

    #[test]
    fn sell_with_base_fee_values_it_at_the_fill_price() {
        // 0.001 ETH at 2000 USDT = 2 USDT deducted from 3000.
        let (qty, proceeds) = sell_effect(&trade(Side::Sell, "1.2", "3000", "0.001", "ETH"));
        assert_eq!(qty, dec!(1.2));
        assert_eq!(proceeds, dec!(2998));
    }

    #[test]
    fn ignore_excess_policy_scales_proceeds_to_the_covered_fraction() {
        let (qty, proceeds) = apply_oversell_policy(
            OversellPolicy::IgnoreExcess,
            dec!(1.0),
            dec!(2500),
            dec!(0.5),
        );
        assert_eq!(qty, dec!(0.5));
        assert_eq!(proceeds, dec!(1250));
    }

    #[test]
    fn zero_cost_policy_keeps_the_full_disposal() {
        let (qty, proceeds) = apply_oversell_policy(
            OversellPolicy::ZeroCostBasis,
            dec!(1.0),
            dec!(2500),
            dec!(0.5),
        );
        assert_eq!(qty, dec!(1.0));
        assert_eq!(proceeds, dec!(2500));
    }
}
