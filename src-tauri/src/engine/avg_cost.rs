// src-tauri/src/engine/avg_cost.rs
//
// Weighted-average cost basis — the alternative method.
//
// Algorithm (AGENTS.md §6.6):
//   * One pool per pair: (`total_qty`, `total_cost`). No discrete lots.
//   * BUY  -> total_qty += net_qty; total_cost += net_cost.
//   * SELL -> cost_basis = total_cost * (sold_qty / total_qty), then both
//             totals are reduced accordingly. Order within a second does not
//             change the result, but trades are still processed in CSV order so
//             the emitted event stream lines up with FIFO's.
//   * `RealizedEvent.matched_lots` holds a single synthetic slice (there are no
//     real lots to attribute to), flagged `synthetic: true`.
//
// Golden expectation locked by tests: the same 3-row fixture yields
// Realized P&L = 514.52 USDT, remaining 0.3 ETH / 620.62 USDT.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::{
    apply_oversell_policy, buy_effect, finish, sell_effect, EngineOutput, PairBook, PnlEngine,
};
use crate::error::AppResult;
use crate::models::{MatchedLot, OversellPolicy, PnlMethod, RealizedEvent, Side, Trade};

/// The running pool for one pair.
#[derive(Debug, Default, Clone)]
struct Pool {
    qty: Decimal,
    cost: Decimal,
}

impl Pool {
    /// Removes `qty` from the pool and returns the exact cost charged.
    ///
    /// Draining the pool completely hands back the whole remaining cost, so the
    /// rounding introduced by a proportional split can never strand value in a
    /// pool whose quantity has reached zero — the same conservation argument as
    /// `Lot::consume`.
    fn consume(&mut self, qty: Decimal) -> Decimal {
        let take = qty.min(self.qty);
        if take <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let cost = if take == self.qty {
            std::mem::replace(&mut self.cost, Decimal::ZERO)
        } else {
            let slice = self.cost * take / self.qty;
            self.cost -= slice;
            slice
        };

        self.qty -= take;
        cost
    }
}

pub struct AverageCostEngine;

impl PnlEngine for AverageCostEngine {
    fn method(&self) -> PnlMethod {
        PnlMethod::AverageCost
    }

    fn replay(&self, trades: &[Trade], policy: OversellPolicy) -> AppResult<EngineOutput> {
        let mut books: BTreeMap<String, PairBook> = BTreeMap::new();
        let mut pools: BTreeMap<String, Pool> = BTreeMap::new();
        let mut realized_events = Vec::new();

        for trade in trades {
            let book = books
                .entry(trade.pair.clone())
                .or_insert_with(|| PairBook::new(trade));
            book.record(trade);

            let pool = pools.entry(trade.pair.clone()).or_default();

            match trade.side {
                Side::Buy => {
                    let (net_qty, cost) = buy_effect(trade);
                    if net_qty > Decimal::ZERO {
                        pool.qty += net_qty;
                        pool.cost += cost;
                    }
                }

                Side::Sell => {
                    let (sold_qty, gross_proceeds) = sell_effect(trade);

                    let covered_qty = sold_qty.min(pool.qty);
                    let (effective_qty, proceeds) =
                        apply_oversell_policy(policy, sold_qty, gross_proceeds, covered_qty);

                    // `consume` clamps to what the pool holds, so under
                    // ZeroCostBasis the uncovered excess simply contributes no
                    // cost — exactly the intended zero-cost matching.
                    let cost_basis = pool.consume(effective_qty);

                    let oversell = sold_qty > covered_qty;
                    if oversell {
                        book.position.has_oversell = true;
                    }

                    // There are no discrete lots to attribute to, so the whole
                    // disposal is reported as one synthetic slice. When part of it
                    // was an oversell, the zero-cost excess is split out so the
                    // report can still show it separately.
                    let mut matched_lots = Vec::new();
                    if covered_qty > Decimal::ZERO {
                        matched_lots.push(MatchedLot {
                            buy_trade_id: None,
                            acquired_at: None,
                            qty: covered_qty.min(effective_qty),
                            unit_cost_quote: if covered_qty.is_zero() {
                                Decimal::ZERO
                            } else {
                                cost_basis / covered_qty
                            },
                            cost: cost_basis,
                            synthetic: true,
                        });
                    }
                    if effective_qty > covered_qty {
                        matched_lots.push(MatchedLot {
                            buy_trade_id: None,
                            acquired_at: None,
                            qty: effective_qty - covered_qty,
                            unit_cost_quote: Decimal::ZERO,
                            cost: Decimal::ZERO,
                            synthetic: true,
                        });
                    }

                    realized_events.push(RealizedEvent {
                        sell_trade_id: trade.id,
                        timestamp: trade.timestamp_utc,
                        pair: trade.pair.clone(),
                        base_asset: trade.base_asset.clone(),
                        quote_asset: trade.quote_asset.clone(),
                        qty: effective_qty,
                        proceeds,
                        cost_basis,
                        pnl: proceeds - cost_basis,
                        matched_lots,
                        oversell,
                        oversell_qty: sold_qty - covered_qty,
                    });
                }
            }
        }

        for (pair, pool) in pools {
            if let Some(book) = books.get_mut(&pair) {
                book.position.qty = pool.qty;
                book.position.total_cost = pool.cost;
                // Average Cost has no lot history to expose.
                book.position.open_lots = Vec::new();
            }
        }

        for event in &realized_events {
            if let Some(book) = books.get_mut(&event.pair) {
                book.position.realized_pnl += event.pnl;
            }
        }

        Ok(EngineOutput {
            positions: finish(books),
            realized_events,
        })
    }
}
