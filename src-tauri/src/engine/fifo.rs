// src-tauri/src/engine/fifo.rs
//
// First-In-First-Out cost basis — the default method.
//
// Algorithm (AGENTS.md §6.3):
//   * One `VecDeque<Lot>` per pair.
//   * BUY  -> push_back a new lot. Same-second BUYs are never merged; each fill
//             is its own lot, in CSV order.
//   * SELL -> consume from the front. A partially consumed lot has its
//             quantity and cost reduced and stays at the front; a fully consumed
//             lot is popped.
//   * Every SELL emits one `RealizedEvent` carrying the lot-by-lot breakdown.
//   * Oversell (sell exceeds tracked inventory) follows `OversellPolicy`.
//
// Golden expectation locked by tests: the 3-row fixture yields
// Realized P&L = 554.56 USDT, remaining 0.3 ETH at 2202.20 avg cost.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::{
    apply_oversell_policy, buy_effect, finish, sell_effect, EngineOutput, PairBook, PnlEngine,
};
use crate::error::AppResult;
use crate::models::{
    Lot, LotQueue, MatchedLot, OversellPolicy, PnlMethod, RealizedEvent, Side, Trade,
};

pub struct FifoEngine;

impl PnlEngine for FifoEngine {
    fn method(&self) -> PnlMethod {
        PnlMethod::Fifo
    }

    fn replay(&self, trades: &[Trade], policy: OversellPolicy) -> AppResult<EngineOutput> {
        // BTreeMap rather than HashMap: output ordering must be reproducible
        // across runs, and pair symbols sort naturally.
        let mut books: BTreeMap<String, PairBook> = BTreeMap::new();
        let mut queues: BTreeMap<String, LotQueue> = BTreeMap::new();
        let mut realized_events = Vec::new();

        for trade in trades {
            let book = books
                .entry(trade.pair.clone())
                .or_insert_with(|| PairBook::new(trade));
            book.record(trade);

            let queue = queues.entry(trade.pair.clone()).or_default();

            match trade.side {
                Side::Buy => {
                    let (net_qty, cost) = buy_effect(trade);
                    // The parser rejects a base-asset fee that would consume the
                    // whole fill, so `net_qty` is positive here. Guarding anyway
                    // keeps a zero-quantity lot out of the queue if that ever
                    // changes.
                    if net_qty > Decimal::ZERO {
                        queue
                            .0
                            .push_back(Lot::new(net_qty, cost, trade.timestamp_utc, trade.id));
                    }
                }

                Side::Sell => {
                    let (sold_qty, gross_proceeds) = sell_effect(trade);

                    // How much of this disposal real inventory can back.
                    let available = queue.total_qty();
                    let covered_qty = sold_qty.min(available);
                    let (effective_qty, proceeds) =
                        apply_oversell_policy(policy, sold_qty, gross_proceeds, covered_qty);

                    let mut matched_lots: Vec<MatchedLot> = Vec::new();
                    let mut cost_basis = Decimal::ZERO;
                    let mut outstanding = effective_qty;

                    // Walk the queue front-to-back until the disposal is filled.
                    while outstanding > Decimal::ZERO {
                        let Some(front) = queue.0.front_mut() else {
                            break;
                        };

                        let take = outstanding.min(front.qty_remaining);
                        let buy_trade_id = front.trade_id;
                        let acquired_at = front.acquired_at;

                        // `consume` returns the exact cost charged, so cost is
                        // conserved even when the proportional split rounds.
                        let slice_cost = front.consume(take);
                        cost_basis += slice_cost;
                        outstanding -= take;

                        matched_lots.push(MatchedLot {
                            buy_trade_id: Some(buy_trade_id),
                            acquired_at: Some(acquired_at),
                            qty: take,
                            unit_cost_quote: if take.is_zero() {
                                Decimal::ZERO
                            } else {
                                slice_cost / take
                            },
                            cost: slice_cost,
                            synthetic: false,
                        });

                        if front.qty_remaining <= Decimal::ZERO {
                            queue.0.pop_front();
                        }
                    }

                    // Anything still outstanding had no tracked cost basis.
                    let oversell_qty = outstanding;
                    let oversell = oversell_qty > Decimal::ZERO
                        || (policy == OversellPolicy::IgnoreExcess && sold_qty > covered_qty);

                    if oversell_qty > Decimal::ZERO {
                        // ZeroCostBasis: the excess is matched at zero cost and
                        // flagged. (Under IgnoreExcess `effective_qty` was already
                        // capped at `covered_qty`, so this branch is unreachable
                        // there.)
                        matched_lots.push(MatchedLot {
                            buy_trade_id: None,
                            acquired_at: None,
                            qty: oversell_qty,
                            unit_cost_quote: Decimal::ZERO,
                            cost: Decimal::ZERO,
                            synthetic: true,
                        });
                    }

                    if oversell {
                        book.position.has_oversell = true;
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

        // Fold the surviving inventory into each position.
        for (pair, queue) in queues {
            if let Some(book) = books.get_mut(&pair) {
                book.position.qty = queue.total_qty();
                book.position.total_cost = queue.total_cost();
                book.position.open_lots = queue.0.into_iter().collect();
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
