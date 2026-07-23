// src-tauri/src/parser.rs
//
// CSV ingestion: raw Binance export rows -> validated `Trade` values.
//
// Contract (AGENTS.md §5), implemented in Phase 1:
//   1. `Time` is `%Y-%m-%d %H:%M:%S`, interpreted as UTC.
//   2. `Executed` / `Amount` / `Fee` are "<number><ASSET>"; thousands separators
//      are stripped before `Decimal::from_str`.
//   3. `Pair` is split into base/quote by longest-suffix match against
//      `models::QUOTE_ASSETS`.
//   4. `Side` is BUY/SELL, case-insensitive.
//   5. No numeric field may be negative or zero — except `Fee`, which may be zero.
//   6. A bad row never aborts the import: it becomes a `models::RowError`.
//
// A header mismatch, by contrast, rejects the whole file (`AppError::CsvHeader`).

// TODO(phase-1): pub fn split_amount(raw: &str) -> Option<AmountWithAsset>
// TODO(phase-1): pub fn split_pair(pair: &str) -> Option<(String, String)>
// TODO(phase-1): pub fn parse_file(path: &Path, file_index: usize) -> AppResult<(Vec<Trade>, Vec<RowError>)>
// TODO(phase-1): pub fn merge_and_sort(files: Vec<...>, deduplicate: bool) -> (Vec<Trade>, ImportSummary)
