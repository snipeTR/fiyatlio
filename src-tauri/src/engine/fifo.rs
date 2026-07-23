// src-tauri/src/engine/fifo.rs
//
// First-In-First-Out cost basis — the default method.
//
// Algorithm (AGENTS.md §6.3), implemented in Phase 1:
//   * One `VecDeque<Lot>` per pair.
//   * BUY  -> push_back a new lot. Same-second BUYs are never merged; each fill
//             is its own lot, in CSV order.
//   * SELL -> consume from the front. A partially consumed lot has its
//             `qty_remaining` reduced and stays at the front; a fully consumed
//             lot is popped. Cost splits strictly proportionally
//             (`unit_cost_quote * consumed_qty`), which is exact with `Decimal`.
//   * Every SELL emits one `RealizedEvent` carrying the lot-by-lot breakdown.
//   * Oversell (sell exceeds tracked inventory) follows `OversellPolicy`.
//
// Golden expectation locked by tests: the 3-row fixture yields
// Realized P&L = 554.56 USDT, remaining 0.3 ETH at 2202.20 avg cost.

// TODO(phase-1): pub struct FifoEngine; impl PnlEngine for FifoEngine { … }
