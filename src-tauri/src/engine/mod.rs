// src-tauri/src/engine/mod.rs
//
// Cost-basis engines. Both `fifo` and `avg_cost` implement the same `PnlEngine`
// trait, which is what makes the Settings toggle a pure re-computation with no
// branching anywhere else in the codebase (AGENTS.md §6.6).

pub mod avg_cost;
pub mod fifo;

// TODO(phase-1):
// pub trait PnlEngine {
//     fn method(&self) -> PnlMethod;
//     /// Replays `trades` (already sorted by `(timestamp, file_index, row_index)`)
//     /// and returns the per-pair positions plus every realized event, in order.
//     fn replay(&self, trades: &[Trade], policy: OversellPolicy) -> AppResult<EngineOutput>;
// }
//
// pub struct EngineOutput {
//     pub positions: Vec<Position>,
//     pub realized_events: Vec<RealizedEvent>,
// }
//
// Invariant every engine must uphold, asserted in tests (AGENTS.md §8d):
//     total_buy_cost == matched_cost_basis + remaining_lot_cost
