// src-tauri/src/parser.rs
//
// CSV ingestion: a known exchange export -> validated `Trade` values.
//
// Two Binance spot layouts are registered today. A further exchange is another
// `CsvFormat`: the set of header labels it writes, and which of those labels
// fill Time / Pair / Side / Price / Executed / Amount / Fee. Matching is by
// column name, not by column position.
//
// Contract (AGENTS.md §5):
//   1. `Time` is `%Y-%m-%d %H:%M:%S`, interpreted as UTC.
//   2. `Executed` / `Amount` / `Fee` are "<number><ASSET>"; thousands separators
//      are stripped before parsing.
//   3. `Pair` is split into base/quote by longest-suffix match against
//      `models::QUOTE_ASSETS`.
//   4. `Side` is BUY/SELL, case-insensitive.
//   5. No numeric field may be negative or zero — except `Fee`, which may be zero.
//   6. A bad row never aborts the import: it becomes a `models::RowError`.
//
// A header mismatch, by contrast, rejects the whole file (`AppError::CsvHeader`).

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

use chrono::{DateTime, NaiveDateTime, Utc};
use rust_decimal::Decimal;

use crate::error::{AppError, AppResult};
use crate::formats::{self, RowOutcome};
use crate::models::{
    AmountWithAsset, DedupKey, FileImportStats, ImportSummary, RawTradeRow, RowError, Side, Trade,
    QUOTE_ASSETS,
};

/// Timestamp layout used by every Binance spot trade-history export.
pub const CSV_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

// ---------------------------------------------------------------------------
// Scalar parsing
// ---------------------------------------------------------------------------

/// Parses a bare decimal, tolerating thousands separators (`1,234.56`).
///
/// Uses `from_str_exact` rather than `from_str`: a value too precise for
/// `Decimal` should surface as a rejected row, not be silently rounded into the
/// books.
pub fn parse_decimal(raw: &str) -> Option<Decimal> {
    let cleaned = raw.trim().replace(',', "");
    if cleaned.is_empty() {
        return None;
    }
    Decimal::from_str_exact(&cleaned).ok()
}

/// Splits `"0.2681ETH"` into `(0.2681, "ETH")` without knowing the asset up front.
///
/// The numeric part ends at the first ASCII letter. This is the fallback path —
/// prefer [`split_amount_expecting`] whenever the asset is known, because assets
/// whose ticker begins with a digit (`1INCH`, `1000SATS`) make the naive split
/// ambiguous: `"12.51INCH"` is `12.5 × 1INCH`, not `12.51 × INCH`.
pub fn split_amount(raw: &str) -> Option<AmountWithAsset> {
    let cleaned = raw.trim().replace(',', "");
    let split_at = cleaned
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(cleaned.len());
    let (number, asset) = cleaned.split_at(split_at);

    Some(AmountWithAsset {
        value: Decimal::from_str_exact(number.trim()).ok()?,
        asset: asset.trim().to_ascii_uppercase(),
    })
}

/// Splits `"<number><ASSET>"` by first trying each asset in `candidates` as a
/// literal suffix, falling back to [`split_amount`] when none matches.
///
/// Candidates are tried in the order given, so callers must pass the more
/// specific asset first. Resolving the suffix from the pair rather than from the
/// string shape is what makes `1INCH`-style tickers parse correctly.
pub fn split_amount_expecting(raw: &str, candidates: &[&str]) -> Option<AmountWithAsset> {
    let cleaned = raw.trim().replace(',', "");
    let upper = cleaned.to_ascii_uppercase();

    for candidate in candidates {
        if candidate.is_empty() || !upper.ends_with(candidate) {
            continue;
        }
        let number = &cleaned[..cleaned.len() - candidate.len()];
        if number.trim().is_empty() {
            continue;
        }
        if let Ok(value) = Decimal::from_str_exact(number.trim()) {
            return Some(AmountWithAsset {
                value,
                asset: (*candidate).to_string(),
            });
        }
    }

    split_amount(&cleaned)
}

/// Splits a pair symbol into `(base, quote)` by longest-suffix match.
///
/// `QUOTE_ASSETS` is ordered by descending length (asserted by a unit test in
/// `models`), so `ETHFDUSD` resolves to `("ETH", "FDUSD")` and never to a shorter
/// accidental match. Returns `None` when no known quote asset matches, or when
/// the match would leave an empty base.
pub fn split_pair(pair: &str) -> Option<(String, String)> {
    let symbol = pair.trim().to_ascii_uppercase();

    QUOTE_ASSETS.iter().find_map(|quote| {
        if symbol.len() > quote.len() && symbol.ends_with(quote) {
            Some((
                symbol[..symbol.len() - quote.len()].to_string(),
                (*quote).to_string(),
            ))
        } else {
            None
        }
    })
}

/// Parses a naive `YYYY-MM-DD HH:MM:SS` timestamp as UTC.
///
/// Binance exports carry no offset; treating them as UTC is the documented
/// assumption of the whole application (AGENTS.md §2.3).
pub fn parse_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(raw.trim(), CSV_TIME_FORMAT)
        .ok()
        .map(|naive| naive.and_utc())
}

// ---------------------------------------------------------------------------
// Row validation
// ---------------------------------------------------------------------------

/// Reasons a row can be rejected. Each maps to an i18n key defined in both
/// `tr.json` and `en.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowReject {
    BadTimestamp,
    UnknownQuoteAsset,
    BadSide,
    BadPrice,
    BadExecuted,
    BadAmount,
    BadFee,
    NonPositive,
    AssetMismatch,
    FeeExceedsQuantity,
}

impl RowReject {
    fn key(self) -> &'static str {
        match self {
            RowReject::BadTimestamp => "error.row.badTimestamp",
            RowReject::UnknownQuoteAsset => "error.row.unknownQuoteAsset",
            RowReject::BadSide => "error.row.badSide",
            RowReject::BadPrice => "error.row.badPrice",
            RowReject::BadExecuted => "error.row.badExecuted",
            RowReject::BadAmount => "error.row.badAmount",
            RowReject::BadFee => "error.row.badFee",
            RowReject::NonPositive => "error.row.nonPositive",
            RowReject::AssetMismatch => "error.row.assetMismatch",
            RowReject::FeeExceedsQuantity => "error.row.feeExceedsQuantity",
        }
    }
}

/// Validates one raw row into a `Trade`.
///
/// `id` is a placeholder here; real ids are assigned in [`merge`] after the
/// global sort, so they are monotonic in chronological order.
fn validate_row(
    row: &RawTradeRow,
    file: &str,
    file_index: usize,
    row_index: usize,
) -> Result<Trade, (RowReject, String)> {
    let timestamp_utc =
        parse_timestamp(&row.time).ok_or_else(|| (RowReject::BadTimestamp, row.time.clone()))?;

    let (base_asset, quote_asset) =
        split_pair(&row.pair).ok_or_else(|| (RowReject::UnknownQuoteAsset, row.pair.clone()))?;

    let side = Side::parse(&row.side).ok_or_else(|| (RowReject::BadSide, row.side.clone()))?;

    let price =
        parse_decimal(&row.price).ok_or_else(|| (RowReject::BadPrice, row.price.clone()))?;

    // The base asset is the only sensible suffix for `Executed`.
    let executed = split_amount_expecting(&row.executed, &[base_asset.as_str()])
        .ok_or_else(|| (RowReject::BadExecuted, row.executed.clone()))?;

    // ...and the quote asset for `Amount`.
    let amount = split_amount_expecting(&row.amount, &[quote_asset.as_str()])
        .ok_or_else(|| (RowReject::BadAmount, row.amount.clone()))?;

    // The fee may be denominated in the quote asset, the base asset, or an
    // unrelated one (typically BNB). Quote is tried first because it is by far
    // the most common, then base; anything else falls through to the generic
    // split.
    let fee = split_amount_expecting(&row.fee, &[quote_asset.as_str(), base_asset.as_str()])
        .ok_or_else(|| (RowReject::BadFee, row.fee.clone()))?;

    if price <= Decimal::ZERO {
        return Err((RowReject::NonPositive, format!("Price={price}")));
    }
    if executed.value <= Decimal::ZERO {
        return Err((
            RowReject::NonPositive,
            format!("Executed={}", executed.value),
        ));
    }
    if amount.value <= Decimal::ZERO {
        return Err((RowReject::NonPositive, format!("Amount={}", amount.value)));
    }
    // Fee is the one field allowed to be zero, but never negative.
    if fee.value < Decimal::ZERO {
        return Err((RowReject::NonPositive, format!("Fee={}", fee.value)));
    }

    if executed.asset != base_asset {
        return Err((
            RowReject::AssetMismatch,
            format!("Executed={} (expected {base_asset})", executed.asset),
        ));
    }
    if amount.asset != quote_asset {
        return Err((
            RowReject::AssetMismatch,
            format!("Amount={} (expected {quote_asset})", amount.asset),
        ));
    }

    // A zero fee may legitimately be written without a unit ("0"). Attribute it
    // to the quote asset so downstream `fee_kind()` classification stays sane.
    let fee_asset = if fee.asset.is_empty() && fee.value.is_zero() {
        quote_asset.clone()
    } else if fee.asset.is_empty() {
        return Err((RowReject::BadFee, row.fee.clone()));
    } else {
        fee.asset
    };

    // A base-asset fee is deducted from the acquired quantity (AGENTS.md §6.4).
    // If it swallowed the whole fill there would be no lot to open, which would
    // strand this row's cost and break the conservation invariant — reject here
    // rather than let the engine paper over it.
    if side == Side::Buy && fee_asset == base_asset && fee.value >= executed.value {
        return Err((
            RowReject::FeeExceedsQuantity,
            format!("Fee={} >= Executed={}", fee.value, executed.value),
        ));
    }

    Ok(Trade {
        id: 0,
        timestamp_utc,
        pair: row.pair.trim().to_ascii_uppercase(),
        base_asset,
        quote_asset,
        side,
        price,
        qty: executed.value,
        quote_amount: amount.value,
        fee_amount: fee.value,
        fee_asset,
        source_file: file.to_string(),
        file_index,
        row_index,
    })
}

// ---------------------------------------------------------------------------
// File parsing
// ---------------------------------------------------------------------------

/// Everything one CSV file contributed to the import.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub trades: Vec<Trade>,
    pub errors: Vec<RowError>,
    pub stats: FileImportStats,
}

/// Reads and validates a single CSV file from disk.
pub fn parse_file(path: &Path, file_index: usize) -> AppResult<ParsedFile> {
    let display = path.to_string_lossy().to_string();

    if !path.exists() {
        return Err(AppError::FileNotFound(display));
    }

    let bytes = std::fs::read(path).map_err(|source| AppError::FileRead {
        path: display.clone(),
        source,
    })?;

    parse_bytes(&bytes, &display, file_index)
}

pub fn parse_file_with(
    path: &Path,
    file_index: usize,
    format_id: Option<&str>,
) -> AppResult<ParsedFile> {
    let display = path.to_string_lossy().to_string();
    if !path.exists() {
        return Err(AppError::FileNotFound(display));
    }
    let bytes = std::fs::read(path).map_err(|source| AppError::FileRead {
        path: display.clone(),
        source,
    })?;
    parse_bytes_with(&bytes, &display, file_index, format_id)
}

/// Validates an in-memory CSV buffer. Separated from [`parse_file`] so tests can
/// exercise the parser without touching the filesystem.
pub fn parse_bytes(bytes: &[u8], display_path: &str, file_index: usize) -> AppResult<ParsedFile> {
    parse_bytes_with(bytes, display_path, file_index, None)
}

pub fn parse_bytes_with(
    bytes: &[u8],
    display_path: &str,
    file_index: usize,
    format_id: Option<&str>,
) -> AppResult<ParsedFile> {
    // Excel-written exports often carry a UTF-8 BOM, which would otherwise become
    // part of the first header name and fail the header check.
    let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    parse_reader(body, display_path, file_index, format_id)
}

/// Core parsing routine.
pub fn parse_reader<R: Read>(
    reader: R,
    display_path: &str,
    file_index: usize,
    format_id: Option<&str>,
) -> AppResult<ParsedFile> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .trim(csv::Trim::All)
        .from_reader(reader);

    // Cloned rather than borrowed: `records()` needs its own mutable borrow of
    // the reader, and each record is deserialized against these headers.
    let headers = csv_reader
        .headers()
        .map_err(|source| AppError::Csv {
            path: display_path.to_string(),
            source,
        })?
        .clone();

    // A file matches a registered exchange when its header names are that
    // format's column set. Position does not matter: each column is named.
    let found: Vec<String> = headers.iter().map(|h| h.trim().to_string()).collect();
    let hits = formats::matching(&found);
    let choices = || {
        hits.iter()
            .map(|format| crate::error::FormatChoice {
                id: format.id.to_string(),
                label_key: format.label_key.to_string(),
            })
            .collect()
    };
    let format = if hits.is_empty() {
        return Err(AppError::CsvHeader {
            path: display_path.to_string(),
            expected: formats::accepted_labels(),
            found: found.join(","),
        });
    } else if let Some(id) = format_id {
        hits.iter()
            .copied()
            .find(|format| format.id == id)
            .ok_or_else(|| AppError::AmbiguousFormat {
                path: display_path.to_string(),
                options: choices(),
            })?
    } else if hits.len() == 1 {
        hits[0]
    } else {
        return Err(AppError::AmbiguousFormat {
            path: display_path.to_string(),
            options: choices(),
        });
    };

    let header_names: Vec<String> = headers.iter().map(|name| name.to_string()).collect();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut total_rows = 0usize;

    for (offset, record) in csv_reader.records().enumerate() {
        let row_index = offset + 1;
        total_rows += 1;
        let record = match record {
            Ok(record) => record,
            Err(source) => {
                errors.push(RowError {
                    file: display_path.to_string(),
                    file_index,
                    row_index,
                    reason_key: "error.row.malformed".to_string(),
                    detail: source.to_string(),
                    raw: String::new(),
                });
                continue;
            }
        };
        let mut cells = HashMap::new();
        for (name, value) in header_names.iter().zip(record.iter()) {
            cells.insert(formats::header_key(name), value.to_string());
        }
        rows.push((row_index, cells));
    }

    let mut trades = Vec::new();
    for (row_index, outcome) in formats::interpret(format, &rows) {
        match outcome {
            RowOutcome::Skip => {}
            RowOutcome::Invalid(detail) => errors.push(RowError {
                file: display_path.to_string(),
                file_index,
                row_index,
                reason_key: "error.row.malformed".to_string(),
                detail,
                raw: String::new(),
            }),
            RowOutcome::Trade(row) => match validate_row(&row, display_path, file_index, row_index)
            {
                Ok(trade) => trades.push(trade),
                Err((reject, detail)) => errors.push(RowError {
                    file: display_path.to_string(),
                    file_index,
                    row_index,
                    reason_key: reject.key().to_string(),
                    detail,
                    raw: String::new(),
                }),
            },
        }
    }

    let stats = FileImportStats {
        file: display_path.to_string(),
        file_index,
        total_rows,
        valid_rows: trades.len(),
        invalid_rows: errors.len(),
        // Duplicates are only detectable across the whole selection; filled in by
        // `merge`.
        duplicate_rows: 0,
    };

    Ok(ParsedFile {
        trades,
        errors,
        stats,
    })
}

// ---------------------------------------------------------------------------
// Merging
// ---------------------------------------------------------------------------

/// Merges every parsed file into one chronologically ordered trade list.
///
/// Ordering is `(timestamp_utc, file_index, row_index)`. That tie-breaker is what
/// preserves the original CSV order for partial fills that share a second, which
/// FIFO depends on (AGENTS.md §6.1).
///
/// Deduplication, when enabled, is **strictly cross-file**.
///
/// This is not a detail. A single Binance export never lists the same fill
/// twice, so byte-identical rows *within one file* are genuine separate fills —
/// an algorithmic order split into equal slices produces runs like twelve
/// identical `0.0016ETH` buys stamped to the same second. Collapsing those would
/// silently delete real trades and understate the position.
///
/// Across files the same rows mean the exports overlap. For each identical-row
/// group the file that reported the most occurrences is kept in full and the
/// other files' copies are dropped, which is correct whether the overlap is
/// partial or total.
pub fn merge(files: Vec<ParsedFile>, deduplicate: bool) -> (Vec<Trade>, ImportSummary) {
    let mut stats: Vec<FileImportStats> = Vec::with_capacity(files.len());
    let mut errors: Vec<RowError> = Vec::new();
    let mut trades: Vec<Trade> = Vec::new();

    for file in files {
        stats.push(file.stats);
        errors.extend(file.errors);
        trades.extend(file.trades);
    }

    trades.sort_by_key(|trade| trade.sort_key());

    let mut duplicate_rows = 0usize;
    if deduplicate && stats.len() > 1 {
        // Group every identical row by which file it came from.
        let mut groups: HashMap<DedupKey, BTreeMap<usize, Vec<usize>>> = HashMap::new();
        for (position, trade) in trades.iter().enumerate() {
            groups
                .entry(trade.dedup_key())
                .or_default()
                .entry(trade.file_index)
                .or_default()
                .push(position);
        }

        let mut discard = vec![false; trades.len()];
        for per_file in groups.into_values() {
            // Present in only one file: every occurrence is a real fill.
            if per_file.len() < 2 {
                continue;
            }

            // Keep whichever file listed the most copies — a partial export
            // must not truncate a fuller one. Ties go to the file the user
            // selected first.
            let keep = per_file
                .iter()
                .map(|(file_index, positions)| (positions.len(), Reverse(*file_index)))
                .max()
                .map(|(_, Reverse(file_index))| file_index)
                .unwrap_or(0);

            for (file_index, positions) in &per_file {
                if *file_index == keep {
                    continue;
                }
                for position in positions {
                    discard[*position] = true;
                }
            }
        }

        let mut kept = Vec::with_capacity(trades.len());
        for (position, trade) in trades.into_iter().enumerate() {
            if discard[position] {
                if let Some(entry) = stats.get_mut(trade.file_index) {
                    entry.duplicate_rows += 1;
                    entry.valid_rows = entry.valid_rows.saturating_sub(1);
                }
                duplicate_rows += 1;
            } else {
                kept.push(trade);
            }
        }
        trades = kept;
    }

    // Ids are assigned only now, so they are monotonic in chronological order and
    // stable for anything that references a trade (realized events, lots, UI).
    for (index, trade) in trades.iter_mut().enumerate() {
        trade.id = index as u64;
    }

    let mut pairs: Vec<String> = trades.iter().map(|t| t.pair.clone()).collect();
    pairs.sort_unstable();
    pairs.dedup();

    let summary = ImportSummary {
        total_rows: stats.iter().map(|s| s.total_rows).sum(),
        valid_rows: trades.len(),
        invalid_rows: errors.len(),
        duplicate_rows,
        first_trade_at: trades.first().map(|t| t.timestamp_utc),
        last_trade_at: trades.last().map(|t| t.timestamp_utc),
        pairs,
        files: stats,
        errors,
    };

    (trades, summary)
}

/// Convenience entry point: parse every path, then merge.
///
/// A header-level failure in any single file aborts the whole import, because a
/// file we cannot interpret at all would silently skew every total.
pub fn import_files(paths: &[String], deduplicate: bool) -> AppResult<(Vec<Trade>, ImportSummary)> {
    import_files_with(paths, deduplicate, &HashMap::new())
}

pub fn import_files_with(
    paths: &[String],
    deduplicate: bool,
    format_overrides: &HashMap<String, String>,
) -> AppResult<(Vec<Trade>, ImportSummary)> {
    let mut parsed = Vec::with_capacity(paths.len());
    for (file_index, path) in paths.iter().enumerate() {
        let format_id = format_overrides.get(path).map(String::as_str);
        parsed.push(parse_file_with(Path::new(path), file_index, format_id)?);
    }
    Ok(merge(parsed, deduplicate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn parse(csv: &str) -> ParsedFile {
        parse_bytes(csv.as_bytes(), "test.csv", 0).expect("header must be valid")
    }

    #[test]
    fn splits_amount_with_asset_suffix() {
        let parsed = split_amount("0.2681ETH").expect("parses");
        assert_eq!(parsed.value, dec!(0.2681));
        assert_eq!(parsed.asset, "ETH");
    }

    #[test]
    fn strips_thousands_separators() {
        let parsed = split_amount("1,234.56USDT").expect("parses");
        assert_eq!(parsed.value, dec!(1234.56));
        assert_eq!(parsed.asset, "USDT");
    }

    #[test]
    fn expected_asset_disambiguates_digit_leading_tickers() {
        // The naive split reads "12.51INCH" as 12.51 INCH; knowing the base asset
        // is 1INCH resolves it correctly.
        let naive = split_amount("12.51INCH").expect("parses");
        assert_eq!(naive.value, dec!(12.51));
        assert_eq!(naive.asset, "INCH");

        let informed = split_amount_expecting("12.51INCH", &["1INCH"]).expect("parses");
        assert_eq!(informed.value, dec!(12.5));
        assert_eq!(informed.asset, "1INCH");
    }

    #[test]
    fn splits_pair_longest_quote_suffix_first() {
        assert_eq!(
            split_pair("ETHFDUSD"),
            Some(("ETH".to_string(), "FDUSD".to_string()))
        );
        assert_eq!(
            split_pair("ETHUSDT"),
            Some(("ETH".to_string(), "USDT".to_string()))
        );
        assert_eq!(
            split_pair("ETHBTC"),
            Some(("ETH".to_string(), "BTC".to_string()))
        );
        assert_eq!(
            split_pair("1INCHUSDT"),
            Some(("1INCH".to_string(), "USDT".to_string()))
        );
        assert_eq!(split_pair("XYZQQQ"), None);
        // A bare quote asset has no base half.
        assert_eq!(split_pair("USDT"), None);
    }

    #[test]
    fn timestamps_are_interpreted_as_utc() {
        let parsed = parse_timestamp("2026-01-01 10:00:00").expect("parses");
        assert_eq!(parsed.to_rfc3339(), "2026-01-01T10:00:00+00:00");
        assert!(parse_timestamp("not-a-timestamp").is_none());
    }

    #[test]
    fn rejects_a_file_whose_header_does_not_match() {
        let err = parse_bytes(b"Date,Symbol,Side\n", "bad.csv", 0)
            .expect_err("header mismatch must reject the file");
        assert_eq!(err.kind(), "csvHeader");
    }

    #[test]
    fn every_format_sample_is_recognized() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/formats");
        let mut seen = 0usize;
        for entry in std::fs::read_dir(&dir).expect("samples/formats") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("csv") {
                continue;
            }
            let bytes = std::fs::read(&path).expect("read sample");
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            parse_bytes(&bytes, &name, 0).unwrap_or_else(|err| panic!("{name}: {err}"));
            seen += 1;
        }
        assert!(seen >= 16, "expected the format samples, found {seen}");
    }

    #[test]
    fn accepts_the_current_binance_export_with_order_no() {
        let csv = "Order No,Time,Pair,Side,Price,Executed,Amount,Fee,AOR Conversion Pair,AOR Conversion Rate\n\
                   650006707,2026-03-23 11:49:10,PAXGUSDT,BUY,4271.1,0.075PAXG,320.3325USDT,0.000075PAXG,,\n";
        let parsed = parse(csv);
        assert!(parsed.errors.is_empty());
        assert_eq!(parsed.trades.len(), 1);
        assert_eq!(parsed.trades[0].pair, "PAXGUSDT");
        assert_eq!(parsed.trades[0].qty, dec!(0.075));
        assert_eq!(parsed.trades[0].quote_amount, dec!(320.3325));
        assert_eq!(parsed.trades[0].fee_asset, "PAXG");
    }

    #[test]
    fn column_order_does_not_matter_inside_a_known_format() {
        let csv = "Fee,Amount,Executed,Price,Side,Pair,Time\n\
                   0.000075PAXG,320.3325USDT,0.075PAXG,4271.1,BUY,PAXGUSDT,2026-03-23 11:49:10\n";
        let parsed = parse(csv);
        assert_eq!(parsed.trades.len(), 1);
        assert_eq!(parsed.trades[0].qty, dec!(0.075));
    }

    #[test]
    fn accepts_a_header_that_differs_only_in_case_and_bom() {
        let csv = "\u{feff}time,pair,side,price,executed,amount,fee\n\
                   2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,2USDT\n";
        let parsed = parse_bytes(csv.as_bytes(), "bom.csv", 0).expect("header must be accepted");
        assert_eq!(parsed.trades.len(), 1);
    }

    #[test]
    fn bad_rows_are_skipped_not_fatal() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-05-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,2USDT\n\
                   2026-05-01 10:00:01,XYZQQQ,BUY,10,1.0XYZ,10QQQ,0QQQ\n\
                   2026-05-01 10:00:02,ETHUSDT,SHORT,2000,1.0ETH,2000USDT,0USDT\n\
                   not-a-timestamp,ETHUSDT,BUY,2000,1.0ETH,2000USDT,0USDT\n\
                   2026-05-01 10:00:04,ETHUSDT,BUY,-2000,1.0ETH,2000USDT,0USDT\n\
                   2026-05-01 10:00:05,ETHUSDT,SELL,2500,1.0ETH,2500USDT,0USDT\n";

        let parsed = parse(csv);
        assert_eq!(parsed.trades.len(), 2);
        assert_eq!(parsed.errors.len(), 4);

        let keys: Vec<&str> = parsed
            .errors
            .iter()
            .map(|e| e.reason_key.as_str())
            .collect();
        assert_eq!(
            keys,
            vec![
                "error.row.unknownQuoteAsset",
                "error.row.badSide",
                "error.row.badTimestamp",
                "error.row.nonPositive",
            ]
        );
        // Row numbers are 1-based and exclude the header.
        assert_eq!(parsed.errors[0].row_index, 2);
        assert_eq!(parsed.errors[3].row_index, 5);
    }

    #[test]
    fn rejects_a_base_fee_that_swallows_the_whole_fill() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,1.0ETH\n";
        let parsed = parse(csv);
        assert!(parsed.trades.is_empty());
        assert_eq!(parsed.errors[0].reason_key, "error.row.feeExceedsQuantity");
    }

    #[test]
    fn zero_fee_without_a_unit_is_attributed_to_the_quote_asset() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,0\n";
        let parsed = parse(csv);
        assert_eq!(parsed.trades[0].fee_asset, "USDT");
        assert_eq!(parsed.trades[0].fee_amount, Decimal::ZERO);
    }

    #[test]
    fn merge_sorts_chronologically_and_keeps_csv_order_within_a_second() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-03-01 12:00:00,ETHUSDT,BUY,2000,0.4ETH,800USDT,0USDT\n\
                   2026-03-01 12:00:00,ETHUSDT,BUY,2100,0.3ETH,630USDT,0USDT\n\
                   2026-03-01 12:00:00,ETHUSDT,BUY,2200,0.3ETH,660USDT,0USDT\n";

        let (trades, summary) = merge(vec![parse(csv)], true);

        assert_eq!(trades.len(), 3);
        // Ids are assigned post-sort, so they encode chronological position.
        assert_eq!(
            trades.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        // Same-second fills keep their file order, distinguished by price.
        assert_eq!(
            trades.iter().map(|t| t.price).collect::<Vec<_>>(),
            vec![dec!(2000), dec!(2100), dec!(2200)]
        );
        assert_eq!(summary.duplicate_rows, 0);
        assert_eq!(summary.pairs, vec!["ETHUSDT".to_string()]);
    }

    #[test]
    fn merge_deduplicates_identical_rows_from_overlapping_exports() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,2USDT\n";

        let first = parse_bytes(csv.as_bytes(), "a.csv", 0).expect("valid");
        let second = parse_bytes(csv.as_bytes(), "b.csv", 1).expect("valid");

        let (trades, summary) = merge(vec![first, second], true);
        assert_eq!(trades.len(), 1);
        assert_eq!(summary.duplicate_rows, 1);
        assert_eq!(summary.valid_rows, 1);
        // The surviving row is the one from the first file in selection order.
        assert_eq!(trades[0].file_index, 0);
    }

    #[test]
    fn merge_keeps_duplicates_when_deduplication_is_disabled() {
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,2USDT\n";

        let first = parse_bytes(csv.as_bytes(), "a.csv", 0).expect("valid");
        let second = parse_bytes(csv.as_bytes(), "b.csv", 1).expect("valid");

        let (trades, summary) = merge(vec![first, second], false);
        assert_eq!(trades.len(), 2);
        assert_eq!(summary.duplicate_rows, 0);
    }

    #[test]
    fn identical_rows_within_one_file_are_real_fills_not_duplicates() {
        // An algorithmic order split into equal slices produces runs of
        // byte-identical rows stamped to the same second. A single Binance
        // export never lists one fill twice, so collapsing these would delete
        // real trades — observed on a live export where it would have dropped
        // 180 of 893 rows.
        let row = "2025-11-14 10:22:22,ETHUSDT,BUY,3213.09,0.0016ETH,5.140944USDT,0.0000016ETH\n";
        let csv = format!(
            "Time,Pair,Side,Price,Executed,Amount,Fee\n{}",
            row.repeat(12)
        );

        let (trades, summary) = merge(vec![parse(&csv)], true);

        assert_eq!(
            trades.len(),
            12,
            "identical same-file fills must all survive"
        );
        assert_eq!(summary.duplicate_rows, 0);
        assert_eq!(summary.valid_rows, 12);
    }

    #[test]
    fn cross_file_overlap_keeps_the_fuller_file() {
        // File A saw the order as 3 slices; file B's export caught only 2 of
        // them. Keeping the richer file avoids truncating a real fill.
        let row = "2025-11-14 10:22:22,ETHUSDT,BUY,3213.09,0.0016ETH,5.140944USDT,0.0000016ETH\n";
        let header = "Time,Pair,Side,Price,Executed,Amount,Fee\n";

        let thin = parse_bytes(
            format!("{header}{}", row.repeat(2)).as_bytes(),
            "thin.csv",
            0,
        )
        .expect("valid");
        let full = parse_bytes(
            format!("{header}{}", row.repeat(3)).as_bytes(),
            "full.csv",
            1,
        )
        .expect("valid");

        let (trades, summary) = merge(vec![thin, full], true);

        assert_eq!(trades.len(), 3);
        assert_eq!(summary.duplicate_rows, 2);
        // The survivors all came from the file that reported the most copies.
        assert!(trades.iter().all(|trade| trade.file_index == 1));
    }

    #[test]
    fn same_second_partial_fills_are_not_treated_as_duplicates() {
        // Real partial fills share a timestamp but differ in quantity.
        let csv = "Time,Pair,Side,Price,Executed,Amount,Fee\n\
                   2026-03-01 12:00:00,ETHUSDT,BUY,2000,0.4ETH,800USDT,0USDT\n\
                   2026-03-01 12:00:00,ETHUSDT,BUY,2000,0.3ETH,600USDT,0USDT\n";

        let (trades, summary) = merge(vec![parse(csv)], true);
        assert_eq!(trades.len(), 2);
        assert_eq!(summary.duplicate_rows, 0);
    }
}
