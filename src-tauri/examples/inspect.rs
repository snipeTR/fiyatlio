// src-tauri/examples/inspect.rs
//
// Diagnostic CLI: prints how a real trade-history CSV decomposes, so a
// surprising number on the dashboard can be traced to its cause without
// clicking through the UI.
//
//   cargo run --example inspect -- "path\to\history.csv"
//
// Reads only; writes nothing.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::PathBuf;

use fiyatlio_lib::analysis::build_analysis;
use fiyatlio_lib::engine::engine_for;
use fiyatlio_lib::models::{OversellPolicy, PnlMethod, Settings};
use fiyatlio_lib::parser;
use rust_decimal::Decimal;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example inspect -- <csv path>");
        std::process::exit(2);
    };

    let parsed = match parser::parse_file(&PathBuf::from(&path), 0) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("parse failed: {e}");
            std::process::exit(1);
        }
    };
    let (trades, import) = parser::merge(vec![parsed], true);

    println!("== import ==");
    println!("  total rows      : {}", import.total_rows);
    println!("  valid           : {}", import.valid_rows);
    println!("  skipped         : {}", import.invalid_rows);
    println!("  duplicates      : {}", import.duplicate_rows);
    println!("  pairs           : {}", import.pairs.len());
    println!(
        "  range           : {:?} .. {:?}",
        import.first_trade_at.map(|t| t.date_naive()),
        import.last_trade_at.map(|t| t.date_naive())
    );

    let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
    for error in &import.errors {
        *by_reason.entry(error.reason_key.as_str()).or_default() += 1;
    }
    for (reason, count) in &by_reason {
        println!("    {reason}: {count}");
    }

    let mut quote_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut fee_asset_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for trade in &trades {
        *quote_counts.entry(trade.quote_asset.as_str()).or_default() += 1;
        *fee_asset_counts
            .entry(trade.fee_asset.as_str())
            .or_default() += 1;
    }
    println!("  quote assets    : {quote_counts:?}");
    println!("  fee assets      : {fee_asset_counts:?}");

    for policy in [OversellPolicy::ZeroCostBasis, OversellPolicy::IgnoreExcess] {
        for method in [PnlMethod::Fifo, PnlMethod::AverageCost] {
            let output = engine_for(method)
                .replay(&trades, policy)
                .expect("replay succeeds");

            let realized: Decimal = output.realized_events.iter().map(|e| e.pnl).sum();
            let oversells: Vec<_> = output
                .realized_events
                .iter()
                .filter(|e| e.oversell)
                .collect();
            let oversell_pnl: Decimal = oversells.iter().map(|e| e.pnl).sum();

            println!("\n== {method:?} / {policy:?} ==");
            println!("  realized (own quote, summed naively) : {realized}");
            println!(
                "  oversell events                      : {} of {}",
                oversells.len(),
                output.realized_events.len()
            );
            println!("  P&L attributable to oversell events   : {oversell_pnl}");

            let mut per_pair: BTreeMap<&str, (Decimal, usize)> = BTreeMap::new();
            for event in &output.realized_events {
                let entry = per_pair.entry(event.pair.as_str()).or_default();
                entry.0 += event.pnl;
                if event.oversell {
                    entry.1 += 1;
                }
            }
            let mut rows: Vec<_> = per_pair.into_iter().collect();
            rows.sort_by_key(|(_, (pnl, _))| Reverse(*pnl));
            println!("  top pairs by realized P&L:");
            for (pair, (pnl, oversell_count)) in rows.iter().take(12) {
                println!("    {pair:<12} {pnl:>18}   oversell events: {oversell_count}");
            }
        }
    }

    // What the dashboard would show with no price data at all.
    let analysis =
        build_analysis(&trades, &import, &Settings::default(), None).expect("analysis builds");
    println!("\n== dashboard (no prices) ==");
    println!(
        "  realized total (USDT-convertible only) : {}",
        analysis.summary.total_realized_pnl_usdt
    );
    println!("  excluded pairs : {:?}", analysis.summary.excluded_pairs);
    println!("  has oversell   : {}", analysis.summary.has_oversell);
}
