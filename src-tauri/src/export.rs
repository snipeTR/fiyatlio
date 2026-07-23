// src-tauri/src/export.rs
//
// CSV and JSON export for the trade list and the realized-event list.
//
// Decimals are written as plain strings in both formats. A spreadsheet reading
// the CSV keeps full precision, and a JSON consumer never has to deal with a
// float that lost the last satoshi.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::{RealizedEvent, Trade};

/// What to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportKind {
    Trades,
    RealizedEvents,
}

/// Output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        }
    }
}

/// Default file name, date-stamped so repeated exports do not collide.
pub fn suggested_filename(kind: ExportKind, format: ExportFormat) -> String {
    let stem = match kind {
        ExportKind::Trades => "fiyatlio-trades",
        ExportKind::RealizedEvents => "fiyatlio-realized-events",
    };
    let date = chrono::Utc::now().format("%Y-%m-%d");
    format!("{stem}-{date}.{}", format.extension())
}

fn write_file(path: &Path, contents: &str) -> AppResult<()> {
    std::fs::write(path, contents).map_err(|source| AppError::FileWrite {
        path: path.to_string_lossy().to_string(),
        source,
    })
}

/// Serialises trades to CSV.
///
/// The layout deliberately splits the units out into their own columns rather
/// than reproducing Binance's glued `0.2681ETH` form — the export is meant to be
/// analysed, not round-tripped back into the importer.
fn trades_to_csv(trades: &[Trade]) -> AppResult<String> {
    let mut writer = csv::Writer::from_writer(Vec::new());

    writer
        .write_record([
            "id",
            "timestamp_utc",
            "pair",
            "base_asset",
            "quote_asset",
            "side",
            "price",
            "quantity",
            "quote_amount",
            "fee_amount",
            "fee_asset",
            "source_file",
            "row_index",
        ])
        .map_err(|e| AppError::Report(e.to_string()))?;

    for trade in trades {
        writer
            .write_record([
                trade.id.to_string(),
                trade.timestamp_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
                trade.pair.clone(),
                trade.base_asset.clone(),
                trade.quote_asset.clone(),
                trade.side.as_str().to_string(),
                trade.price.to_string(),
                trade.qty.to_string(),
                trade.quote_amount.to_string(),
                trade.fee_amount.to_string(),
                trade.fee_asset.clone(),
                trade.source_file.clone(),
                trade.row_index.to_string(),
            ])
            .map_err(|e| AppError::Report(e.to_string()))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| AppError::Report(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| AppError::Report(e.to_string()))
}

fn events_to_csv(events: &[RealizedEvent]) -> AppResult<String> {
    let mut writer = csv::Writer::from_writer(Vec::new());

    writer
        .write_record([
            "sell_trade_id",
            "timestamp_utc",
            "pair",
            "quote_asset",
            "quantity",
            "proceeds",
            "cost_basis",
            "pnl",
            "oversell",
            "oversell_qty",
            "matched_lot_count",
        ])
        .map_err(|e| AppError::Report(e.to_string()))?;

    for event in events {
        writer
            .write_record([
                event.sell_trade_id.to_string(),
                event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                event.pair.clone(),
                event.quote_asset.clone(),
                event.qty.to_string(),
                event.proceeds.to_string(),
                event.cost_basis.to_string(),
                event.pnl.to_string(),
                event.oversell.to_string(),
                event.oversell_qty.to_string(),
                event.matched_lots.len().to_string(),
            ])
            .map_err(|e| AppError::Report(e.to_string()))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| AppError::Report(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| AppError::Report(e.to_string()))
}

/// Writes `trades` or `events` to `path` in the requested format.
pub fn export(
    kind: ExportKind,
    format: ExportFormat,
    path: &Path,
    trades: &[Trade],
    events: &[RealizedEvent],
) -> AppResult<()> {
    let contents = match (kind, format) {
        (ExportKind::Trades, ExportFormat::Csv) => trades_to_csv(trades)?,
        (ExportKind::RealizedEvents, ExportFormat::Csv) => events_to_csv(events)?,
        (ExportKind::Trades, ExportFormat::Json) => {
            serde_json::to_string_pretty(trades).map_err(|e| AppError::Report(e.to_string()))?
        }
        (ExportKind::RealizedEvents, ExportFormat::Json) => {
            serde_json::to_string_pretty(events).map_err(|e| AppError::Report(e.to_string()))?
        }
    };

    write_file(path, &contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Side;
    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    fn sample_trade() -> Trade {
        Trade {
            id: 7,
            timestamp_utc: Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            pair: "ETHUSDT".into(),
            base_asset: "ETH".into(),
            quote_asset: "USDT".into(),
            side: Side::Buy,
            price: dec!(2000),
            qty: dec!(1.00000000),
            quote_amount: dec!(2000),
            fee_amount: dec!(0.00026810),
            fee_asset: "ETH".into(),
            source_file: "a.csv".into(),
            file_index: 0,
            row_index: 3,
        }
    }

    #[test]
    fn csv_export_preserves_full_decimal_precision() {
        let csv = trades_to_csv(&[sample_trade()]).expect("csv builds");
        // Trailing zeros are part of the recorded precision and must survive.
        assert!(csv.contains("0.00026810"), "fee precision lost in:\n{csv}");
        assert!(csv.contains("1.00000000"), "qty precision lost in:\n{csv}");
        assert!(csv.contains("ETHUSDT,ETH,USDT,BUY"));
    }

    #[test]
    fn json_export_writes_decimals_as_strings() {
        let json = serde_json::to_string(&[sample_trade()]).expect("json builds");
        // Quoted, not bare — a bare number would be parsed as an IEEE-754 double
        // by any JavaScript consumer.
        assert!(json.contains("\"price\":\"2000\""), "{json}");
        assert!(json.contains("\"feeAmount\":\"0.00026810\""), "{json}");
    }

    #[test]
    fn suggested_filenames_are_date_stamped_and_typed() {
        let name = suggested_filename(ExportKind::Trades, ExportFormat::Csv);
        assert!(name.starts_with("fiyatlio-trades-"));
        assert!(name.ends_with(".csv"));

        let name = suggested_filename(ExportKind::RealizedEvents, ExportFormat::Json);
        assert!(name.starts_with("fiyatlio-realized-events-"));
        assert!(name.ends_with(".json"));
    }
}
