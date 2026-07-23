// src-tauri/tests/pipeline.rs
//
// End-to-end wiring test: CSV on disk -> parser -> engine -> analysis -> report
// and exports written to real files.
//
// The golden tests in `golden.rs` prove the arithmetic. This one proves the
// pieces are actually connected — the class of bug where every unit passes but
// the report renders an empty table because a field was never populated.
//
// Prices are supplied as a fixed snapshot rather than fetched: a test that
// depends on the live exchange is a test that fails on a train.

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use fiyatlio_lib::analysis::{build_analysis, required_symbols};
use fiyatlio_lib::export::{self, ExportFormat, ExportKind};
use fiyatlio_lib::models::{Language, PnlMethod, Position, PriceSnapshot, Settings};
use fiyatlio_lib::parser;
use fiyatlio_lib::report::{self, ChartImage, ReportOptions};
use rust_decimal_macros::dec;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Per-test scratch directory under `target/`, so nothing is written outside the
/// project tree and parallel tests cannot collide.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-output")
        .join(name);
    std::fs::create_dir_all(&dir).expect("scratch directory must be creatable");
    dir
}

fn snapshot() -> PriceSnapshot {
    let mut prices = BTreeMap::new();
    prices.insert("ETHUSDT".to_string(), dec!(3000));
    PriceSnapshot {
        prices,
        fetched_at: Utc.with_ymd_and_hms(2026, 7, 23, 12, 0, 0).unwrap(),
        stale: false,
        failed_symbols: Vec::new(),
    }
}

#[test]
fn full_pipeline_from_csv_to_report_and_exports() {
    // --- import -----------------------------------------------------------
    let parsed = parser::parse_file(&fixture("golden_basic.csv"), 0).expect("fixture parses");
    let (trades, import) = parser::merge(vec![parsed], true);

    assert_eq!(trades.len(), 3);
    assert_eq!(import.valid_rows, 3);
    assert_eq!(import.pairs, vec!["ETHUSDT".to_string()]);
    assert!(import.first_trade_at.is_some());

    // --- analyse ----------------------------------------------------------
    let prices = snapshot();
    let analysis = build_analysis(&trades, &import, &Settings::default(), Some(&prices))
        .expect("analysis builds");

    assert_eq!(analysis.method, PnlMethod::Fifo);
    // Golden FIFO realized P&L, now carried all the way through to the summary.
    assert_eq!(analysis.summary.total_realized_pnl_usdt, dec!(554.56));
    // 0.3 ETH still held, marked at 3000.
    assert_eq!(analysis.summary.total_portfolio_value_usdt, dec!(900.0));
    // 900 market value against 660.66 of remaining cost.
    assert_eq!(analysis.summary.total_unrealized_pnl_usdt, dec!(239.34));
    assert_eq!(analysis.summary.total_trades, 3);
    assert_eq!(analysis.summary.total_sell_events, 1);
    assert_eq!(analysis.summary.winning_events, 1);
    assert_eq!(analysis.summary.win_rate, dec!(100));
    assert!(!analysis.summary.has_oversell);
    assert!(analysis.summary.excluded_pairs.is_empty());

    // Fees were all in USDT, so the approximate total is in fact exact here.
    assert_eq!(analysis.summary.total_fees_usdt_approx, dec!(6.1));
    assert!(!analysis.summary.fees_conversion_partial);

    // The symbol list the price refresh would request.
    let positions: Vec<Position> = analysis
        .positions
        .iter()
        .map(|valuation| valuation.position.clone())
        .collect();
    assert_eq!(required_symbols(&positions), vec!["ETHUSDT".to_string()]);

    // --- report -----------------------------------------------------------
    let dir = scratch("pipeline");
    let report_path = dir.join("report.html");

    let written = report::write(
        &analysis,
        &ReportOptions {
            language: Language::En,
            charts: vec![ChartImage {
                title: "Cumulative P&L".into(),
                data_uri: "data:image/png;base64,AAAA".into(),
            }],
        },
        &report_path,
    )
    .expect("report writes");

    assert_eq!(written, report_path.to_string_lossy());
    let html = std::fs::read_to_string(&report_path).expect("report is readable");

    // The numbers actually reached the document, not just the struct.
    assert!(html.contains("554.56"), "realized P&L missing from report");
    assert!(html.contains("ETHUSDT"), "position row missing from report");
    assert!(
        html.contains("data:image/png;base64,AAAA"),
        "chart not embedded"
    );
    assert!(html.contains("Fiyatlio Report"));
    // Self-contained: no remote references at all.
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));

    // --- exports ----------------------------------------------------------
    let trades_csv = dir.join("trades.csv");
    export::export(
        ExportKind::Trades,
        ExportFormat::Csv,
        &trades_csv,
        &trades,
        &analysis.realized_events,
    )
    .expect("trades csv writes");

    let csv = std::fs::read_to_string(&trades_csv).expect("csv is readable");
    // Header plus three data rows plus the trailing newline.
    assert_eq!(csv.lines().count(), 4);
    assert!(csv.starts_with("id,timestamp_utc,pair,"));

    let events_json = dir.join("events.json");
    export::export(
        ExportKind::RealizedEvents,
        ExportFormat::Json,
        &events_json,
        &trades,
        &analysis.realized_events,
    )
    .expect("events json writes");

    let json = std::fs::read_to_string(&events_json).expect("json is readable");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed.as_array().map(Vec::len), Some(1));

    // Decimals survive as JSON strings, never as bare numbers — that is what
    // stops a JavaScript consumer silently rounding them through a double.
    let pnl = parsed[0]["pnl"].as_str().expect("pnl must be a string");
    // Compared as a Decimal, not as text: the trailing scale is an artifact of
    // the arithmetic (`554.560` here), while the VALUE is the contract.
    assert_eq!(
        rust_decimal::Decimal::from_str_exact(pnl).expect("pnl parses"),
        dec!(554.56)
    );
}

#[test]
fn method_switch_recomputes_the_whole_analysis() {
    let parsed = parser::parse_file(&fixture("golden_basic.csv"), 0).expect("fixture parses");
    let (trades, import) = parser::merge(vec![parsed], true);
    let prices = snapshot();

    let fifo = build_analysis(&trades, &import, &Settings::default(), Some(&prices))
        .expect("analysis builds");

    let avg_settings = Settings {
        pnl_method: PnlMethod::AverageCost,
        ..Settings::default()
    };
    let avg =
        build_analysis(&trades, &import, &avg_settings, Some(&prices)).expect("analysis builds");

    assert_eq!(fifo.summary.total_realized_pnl_usdt, dec!(554.56));
    assert_eq!(avg.summary.total_realized_pnl_usdt, dec!(514.52));

    // Quantity held is method-independent, so portfolio value must not move...
    assert_eq!(
        fifo.summary.total_portfolio_value_usdt,
        avg.summary.total_portfolio_value_usdt
    );
    // ...while unrealized P&L shifts by exactly the cost the methods disagree on.
    assert_eq!(
        avg.summary.total_unrealized_pnl_usdt - fifo.summary.total_unrealized_pnl_usdt,
        dec!(40.04)
    );
}

#[test]
fn the_shipped_sample_csv_imports_cleanly() {
    // `samples/sample-trades.csv` is the file the README tells users to try
    // first. If it ever produced a skipped row or an oversell warning, the very
    // first impression of the app would be an error banner.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("samples")
        .join("sample-trades.csv");

    let parsed = parser::parse_file(&path, 0).expect("sample parses");
    let (trades, import) = parser::merge(vec![parsed], true);

    assert_eq!(
        import.invalid_rows, 0,
        "sample must not contain rejected rows: {:?}",
        import.errors
    );
    assert_eq!(import.duplicate_rows, 0);
    assert_eq!(import.valid_rows, trades.len());

    // It is meant to exercise the interesting paths, so assert it still does.
    assert!(import.pairs.len() >= 5, "sample should cover several pairs");
    assert!(
        trades.iter().any(|t| t.quote_asset != "USDT"),
        "sample should include a non-USDT quote pair"
    );
    assert!(
        trades.iter().any(|t| t.fee_asset == t.base_asset),
        "sample should include a base-asset fee"
    );

    let analysis =
        build_analysis(&trades, &import, &Settings::default(), None).expect("analysis builds");
    assert!(
        !analysis.summary.has_oversell,
        "sample must not trigger the oversell warning"
    );
}

#[test]
fn analysis_survives_having_no_prices_at_all() {
    // The fully-offline first launch: cost basis is known, market value is not,
    // and nothing panics or reports a bogus zero valuation.
    let parsed = parser::parse_file(&fixture("golden_basic.csv"), 0).expect("fixture parses");
    let (trades, import) = parser::merge(vec![parsed], true);

    let analysis =
        build_analysis(&trades, &import, &Settings::default(), None).expect("analysis builds");

    assert_eq!(analysis.summary.total_realized_pnl_usdt, dec!(554.56));
    assert_eq!(analysis.summary.total_portfolio_value_usdt, dec!(0));
    assert!(analysis.positions[0].current_price.is_none());
    assert!(analysis.positions[0].market_value_quote.is_none());
    assert!(analysis.summary.price_snapshot.is_none());

    // And the report still renders.
    let dir = scratch("offline");
    let path = dir.join("offline-report.html");
    report::write(
        &analysis,
        &ReportOptions {
            language: Language::Tr,
            charts: Vec::new(),
        },
        &path,
    )
    .expect("report writes without prices");

    let html = std::fs::read_to_string(&path).expect("report is readable");
    assert!(html.contains("554.56"));
}
