// src-tauri/src/engine/avg_cost.rs
//
// Weighted-average cost basis — the alternative method.
//
// Algorithm (AGENTS.md §6.6), implemented in Phase 1:
//   * One pool per pair: (`total_qty`, `total_cost`). No discrete lots.
//   * BUY  -> total_qty += net_qty; total_cost += net_cost.
//   * SELL -> cost_basis = total_cost * (sold_qty / total_qty), then both
//             totals are reduced accordingly. Order within a second is
//             irrelevant to the result but is still processed in CSV order so
//             the event stream matches FIFO's.
//   * `RealizedEvent.matched_lots` holds a single synthetic slice (there are no
//     real lots to attribute to), flagged `synthetic: true`.
//
// Golden expectation locked by tests: the same 3-row fixture yields
// Realized P&L = 514.52 USDT, remaining 0.3 ETH / 620.62 USDT.

// TODO(phase-1): pub struct AverageCostEngine; impl PnlEngine for AverageCostEngine { … }
