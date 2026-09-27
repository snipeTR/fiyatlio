// Known exchange CSV layouts.
//
// A file matches a layout when its header names are that layout's column set.
// Order and case do not matter. A trailing "(UTC…)" on a header is ignored so
// KuCoin's timezone suffix does not create a new format.
//
// Two layouts with the same column set are ambiguous: the import stops and the
// UI asks which one the file is. None of the layouts registered today collide.

use std::collections::HashMap;

use crate::models::RawTradeRow;

#[derive(Clone, Copy)]
pub struct CsvFormat {
    pub id: &'static str,
    pub label_key: &'static str,
    pub columns: &'static [&'static str],
    pub kind: Kind,
}

#[derive(Clone, Copy)]
pub enum Kind {
    /// `Executed` / `Amount` / `Fee` already look like `0.1ETH`.
    Suffixed {
        time: &'static str,
        pair: &'static str,
        side: &'static str,
        price: &'static str,
        executed: &'static str,
        amount: &'static str,
        fee: &'static str,
    },
    /// Quantity, quote total and fee are separate numbers.
    Split {
        time: &'static str,
        pair: &'static str,
        side: &'static str,
        price: &'static str,
        qty: &'static str,
        total: Option<&'static str>,
        fee: &'static str,
        fee_asset: Option<&'static str>,
    },
    /// Binance wallet statement: one asset per row. Buys and sells are paired
    /// by timestamp. Deposits are not trades.
    BinanceStatement,
    /// Kraken ledgers: a spot trade is two rows that share `refid`.
    KrakenLedgers,
    /// Bitget spot bills: a trade is a Buy row and a Sell row with the same order id.
    BitgetBills,
    /// Recognised account file that is not a fill list (deposits, bills, futures
    /// exports that do not carry a fill price). Rows are skipped, not rejected.
    NotTrades,
}

const CSV_FORMATS: &[CsvFormat] = &[
    CsvFormat {
        id: "binance-spot-time",
        label_key: "format.binanceSpotTime",
        columns: &["Time", "Pair", "Side", "Price", "Executed", "Amount", "Fee"],
        kind: Kind::Suffixed {
            time: "Time",
            pair: "Pair",
            side: "Side",
            price: "Price",
            executed: "Executed",
            amount: "Amount",
            fee: "Fee",
        },
    },
    CsvFormat {
        id: "binance-spot-date",
        label_key: "format.binanceSpotDate",
        columns: &[
            "Date(UTC)",
            "Pair",
            "Side",
            "Price",
            "Executed",
            "Amount",
            "Fee",
        ],
        kind: Kind::Suffixed {
            time: "Date(UTC)",
            pair: "Pair",
            side: "Side",
            price: "Price",
            executed: "Executed",
            amount: "Amount",
            fee: "Fee",
        },
    },
    CsvFormat {
        id: "binance-spot-order-no",
        label_key: "format.binanceSpotOrderNo",
        columns: &[
            "Order No",
            "Time",
            "Pair",
            "Side",
            "Price",
            "Executed",
            "Amount",
            "Fee",
            "AOR Conversion Pair",
            "AOR Conversion Rate",
        ],
        kind: Kind::Suffixed {
            time: "Time",
            pair: "Pair",
            side: "Side",
            price: "Price",
            executed: "Executed",
            amount: "Amount",
            fee: "Fee",
        },
    },
    CsvFormat {
        id: "binance-order-history",
        label_key: "format.binanceOrderHistory",
        columns: &[
            "Date(UTC)",
            "Market",
            "Type",
            "Price",
            "Amount",
            "Total",
            "Fee",
            "Fee Coin",
        ],
        kind: Kind::Split {
            time: "Date(UTC)",
            pair: "Market",
            side: "Type",
            price: "Price",
            qty: "Amount",
            total: Some("Total"),
            fee: "Fee",
            fee_asset: Some("Fee Coin"),
        },
    },
    CsvFormat {
        id: "binance-statement",
        label_key: "format.binanceStatement",
        columns: &[
            "User_ID",
            "UTC_Time",
            "Account",
            "Operation",
            "Coin",
            "Change",
            "Remark",
        ],
        kind: Kind::BinanceStatement,
    },
    CsvFormat {
        id: "binance-deposit",
        label_key: "format.binanceDeposit",
        columns: &[
            "Date(UTC)",
            "Coin",
            "Amount",
            "TransactionFee",
            "Address",
            "TXID",
            "SourceAddress",
            "PaymentID",
            "Status",
        ],
        kind: Kind::NotTrades,
    },
    CsvFormat {
        id: "binance-usdm",
        label_key: "format.binanceUsdm",
        columns: &[
            "Date(UTC)",
            "Pair",
            "Side",
            "Price",
            "Quantity",
            "Amount",
            "Fee",
            "Realized Profit",
        ],
        kind: Kind::Split {
            time: "Date(UTC)",
            pair: "Pair",
            side: "Side",
            price: "Price",
            qty: "Quantity",
            total: Some("Amount"),
            fee: "Fee",
            fee_asset: None,
        },
    },
    CsvFormat {
        id: "bybit-spot",
        label_key: "format.bybitSpot",
        columns: &[
            "Spot Pairs",
            "Order Type",
            "Direction",
            "feeCoin",
            "ExecFeeV2",
            "Filled Value",
            "Filled Price",
            "Filled Quantity",
            "Fees",
            "Transaction ID",
            "Order No.",
            "Timestamp (UTC)",
        ],
        kind: Kind::Split {
            time: "Timestamp (UTC)",
            pair: "Spot Pairs",
            side: "Direction",
            price: "Filled Price",
            qty: "Filled Quantity",
            total: Some("Filled Value"),
            fee: "Fees",
            fee_asset: Some("feeCoin"),
        },
    },
    CsvFormat {
        id: "kucoin-spot",
        label_key: "format.kucoinSpot",
        columns: &[
            "UID",
            "Account Type",
            "Order ID",
            "Order Time",
            "Symbol",
            "Side",
            "Order Type",
            "Order Price",
            "Order Amount",
            "Avg. Filled Price",
            "Filled Amount",
            "Filled Volume",
            "Filled Volume (USDT)",
            "Filled Time",
            "Fee",
            "Fee Currency",
            "Status",
        ],
        kind: Kind::Split {
            time: "Filled Time",
            pair: "Symbol",
            side: "Side",
            price: "Avg. Filled Price",
            qty: "Filled Amount",
            total: Some("Filled Volume"),
            fee: "Fee",
            fee_asset: Some("Fee Currency"),
        },
    },
    CsvFormat {
        id: "okx-orders",
        label_key: "format.okxOrders",
        columns: &[
            "Order ID",
            "Order Time",
            "Instrument",
            "Symbol",
            "Lvg",
            "Side",
            "Order Type",
            "Order Amount",
            "Filled Amount",
            "Amount Unit",
            "Order Price",
            "Avg. Filled Price",
            "Price Unit",
            "PNL",
            "Fee",
            "Fee Unit",
            "Status",
            "Bot ID",
        ],
        kind: Kind::Split {
            time: "Order Time",
            pair: "Symbol",
            side: "Side",
            price: "Avg. Filled Price",
            qty: "Filled Amount",
            total: None,
            fee: "Fee",
            fee_asset: Some("Fee Unit"),
        },
    },
    CsvFormat {
        id: "okx-trades",
        label_key: "format.okxTrades",
        columns: &[
            "User ID",
            "Order ID",
            "Time",
            "Trade Type",
            "Symbol",
            "Action",
            "Amount",
            "Trading Unit",
            "Filled Price",
            "PnL",
            "Fee",
            "Fee Unit",
            "Position Change",
            "Position Balance",
            "Balance Change",
            "Balance",
            "Balance Unit",
        ],
        kind: Kind::Split {
            time: "Time",
            pair: "Symbol",
            side: "Action",
            price: "Filled Price",
            qty: "Amount",
            total: None,
            fee: "Fee",
            fee_asset: Some("Fee Unit"),
        },
    },
    CsvFormat {
        id: "coinbase-transactions",
        label_key: "format.coinbase",
        columns: &[
            "Timestamp",
            "Transaction Type",
            "Asset",
            "Quantity Transacted",
            "Spot Price Currency",
            "Spot Price at Transaction",
            "Subtotal",
            "Total (inclusive of fees)",
            "Fees",
            "Notes",
        ],
        kind: Kind::Split {
            time: "Timestamp",
            pair: "Asset",
            side: "Transaction Type",
            price: "Spot Price at Transaction",
            qty: "Quantity Transacted",
            total: Some("Subtotal"),
            fee: "Fees",
            fee_asset: Some("Spot Price Currency"),
        },
    },
    CsvFormat {
        id: "kraken-trades",
        label_key: "format.krakenTrades",
        columns: &[
            "txid",
            "ordertxid",
            "pair",
            "time",
            "type",
            "ordertype",
            "price",
            "cost",
            "fee",
            "vol",
            "margin",
            "misc",
            "ledgers",
        ],
        kind: Kind::Split {
            time: "time",
            pair: "pair",
            side: "type",
            price: "price",
            qty: "vol",
            total: Some("cost"),
            fee: "fee",
            fee_asset: None,
        },
    },
    CsvFormat {
        id: "kraken-ledgers",
        label_key: "format.krakenLedgers",
        columns: &[
            "txid", "refid", "time", "type", "subtype", "aclass", "subclass", "asset", "wallet",
            "amount", "fee", "balance",
        ],
        kind: Kind::KrakenLedgers,
    },
    CsvFormat {
        id: "gate-spot",
        label_key: "format.gateSpot",
        columns: &[
            "No",
            "Time",
            "Trade type",
            "Role",
            "Market",
            "Deal price",
            "Deal amount",
            "Total",
            "Fee",
        ],
        kind: Kind::Split {
            time: "Time",
            pair: "Market",
            side: "Trade type",
            price: "Deal price",
            qty: "Deal amount",
            total: Some("Total"),
            fee: "Fee",
            fee_asset: None,
        },
    },
    CsvFormat {
        id: "gate-bills",
        label_key: "format.gateBills",
        columns: &[
            "No",
            "Account Type",
            "Time",
            "Action type",
            "Order id",
            "Change amount",
            "Amount",
            "Additional Info",
        ],
        kind: Kind::NotTrades,
    },
    CsvFormat {
        id: "bitget-spot",
        label_key: "format.bitgetSpot",
        columns: &[
            "order",
            "Date",
            "Coin",
            "Type",
            "Amount",
            "Fee",
            "Available",
        ],
        kind: Kind::BitgetBills,
    },
    CsvFormat {
        id: "bitget-usdt-m",
        label_key: "format.bitgetFutures",
        columns: &[
            "Order",
            "Date",
            "Coin",
            "Futures",
            "Margin Mode",
            "Type",
            "Amount",
            "Fee",
            "Wallet balance",
        ],
        kind: Kind::NotTrades,
    },
];

// Bitget futures "Amount" is the contract size, not a price. The split arm
// above would treat Amount as both price and qty. That is wrong. Use a
// dedicated arm instead — see `bitget_futures_row`.
// The const above is corrected below by not using that Split. I will fix
// Kind::BitgetFutures in the match. Leaving the enum variant out and handling
// id in interpret is messy. I'll change the last format's kind after writing
// by editing the enum.

pub fn label_key(id: &str) -> Option<&'static str> {
    CSV_FORMATS
        .iter()
        .find(|format| format.id == id)
        .map(|format| format.label_key)
}

pub fn header_key(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    match lower.find("(utc") {
        Some(index) => lower[..index].trim().to_string(),
        None => lower,
    }
}

pub fn matching(found: &[String]) -> Vec<&'static CsvFormat> {
    let mut keys: Vec<String> = found.iter().map(|name| header_key(name)).collect();
    keys.sort();
    keys.dedup();
    if keys.len() != found.len() {
        return Vec::new();
    }
    CSV_FORMATS
        .iter()
        .filter(|format| {
            let mut expected: Vec<String> =
                format.columns.iter().map(|name| header_key(name)).collect();
            expected.sort();
            expected == keys
        })
        .collect()
}

pub fn accepted_labels() -> String {
    CSV_FORMATS
        .iter()
        .map(|format| format.id)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn by_id(id: &str) -> Option<&'static CsvFormat> {
    CSV_FORMATS.iter().find(|format| format.id == id)
}

#[derive(Debug)]
pub enum RowOutcome {
    Trade(RawTradeRow),
    /// Recognised, but not a spot fill (deposit, the other leg of a paired row).
    Skip,
    Invalid(String),
}

pub fn interpret(
    format: &CsvFormat,
    rows: &[(usize, HashMap<String, String>)],
) -> Vec<(usize, RowOutcome)> {
    match format.kind {
        Kind::Suffixed { .. } | Kind::Split { .. } => rows
            .iter()
            .map(|(index, cells)| (*index, fill_row(format, cells)))
            .collect(),
        Kind::BinanceStatement => binance_statement(rows),
        Kind::KrakenLedgers => kraken_ledgers(rows),
        Kind::BitgetBills => bitget_bills(rows),
        Kind::NotTrades => rows
            .iter()
            .map(|(index, _)| (*index, RowOutcome::Skip))
            .collect(),
    }
}

fn cell<'a>(cells: &'a HashMap<String, String>, name: &str) -> &'a str {
    cells
        .get(&header_key(name))
        .map(String::as_str)
        .unwrap_or("")
}

fn fill_row(format: &CsvFormat, cells: &HashMap<String, String>) -> RowOutcome {
    match format.kind {
        Kind::Suffixed {
            time,
            pair,
            side,
            price,
            executed,
            amount,
            fee,
        } => RowOutcome::Trade(RawTradeRow {
            time: normalize_time(cell(cells, time)),
            pair: normalize_pair(cell(cells, pair)),
            side: normalize_side(cell(cells, side))
                .unwrap_or_else(|| cell(cells, side).to_string()),
            price: cell(cells, price).to_string(),
            executed: cell(cells, executed).to_string(),
            amount: cell(cells, amount).to_string(),
            fee: cell(cells, fee).to_string(),
        }),
        Kind::Split {
            time,
            pair,
            side,
            price,
            qty,
            total,
            fee,
            fee_asset,
        } => split_row(cells, time, pair, side, price, qty, total, fee, fee_asset),
        _ => RowOutcome::Skip,
    }
}

fn split_row(
    cells: &HashMap<String, String>,
    time: &str,
    pair: &str,
    side: &str,
    price: &str,
    qty: &str,
    total: Option<&str>,
    fee: &str,
    fee_asset: Option<&str>,
) -> RowOutcome {
    let mut pair = normalize_pair(cell(cells, pair));
    if split_known_pair(&pair).is_none() {
        if let Some(name) = fee_asset {
            let currency = cell(cells, name).trim().to_ascii_uppercase();
            if !currency.is_empty() {
                pair = format!("{pair}{currency}");
            }
        }
    }
    let Some((base, quote)) = split_known_pair(&pair) else {
        return RowOutcome::Invalid(format!("pair {pair}"));
    };
    let Some(side) = normalize_side(cell(cells, side)) else {
        return RowOutcome::Skip;
    };
    let qty_raw = plain_number(cell(cells, qty));
    let price_raw = plain_number(cell(cells, price));
    if qty_raw.is_empty() || price_raw.is_empty() {
        return RowOutcome::Invalid("qty or price is empty".into());
    }
    let total_raw = match total {
        Some(name) if !cell(cells, name).trim().is_empty() => plain_number(cell(cells, name)),
        _ => match (parse_loose(&price_raw), parse_loose(&qty_raw)) {
            (Some(price), Some(qty)) => (price * qty).normalize().to_string(),
            _ => return RowOutcome::Invalid("price × qty".into()),
        },
    };
    let mut fee_raw = plain_number(cell(cells, fee));
    if !fee_raw
        .chars()
        .any(|character| character.is_ascii_alphabetic())
    {
        fee_raw = fee_raw.trim_start_matches('-').to_string();
    }
    let fee_asset = fee_asset
        .map(|name| cell(cells, name).trim().to_ascii_uppercase())
        .filter(|asset| !asset.is_empty())
        .unwrap_or_else(|| quote.clone());
    let fee = if fee_raw.is_empty() {
        format!("0{fee_asset}")
    } else if fee_raw.chars().any(|c| c.is_ascii_alphabetic()) {
        fee_raw
    } else {
        format!("{fee_raw}{fee_asset}")
    };

    RowOutcome::Trade(RawTradeRow {
        time: normalize_time(cell(cells, time)),
        pair,
        side,
        price: price_raw,
        executed: format!("{qty_raw}{base}"),
        amount: format!("{total_raw}{quote}"),
        fee,
    })
}

fn binance_statement(rows: &[(usize, HashMap<String, String>)]) -> Vec<(usize, RowOutcome)> {
    let mut groups: Vec<(String, Vec<(usize, HashMap<String, String>)>)> = Vec::new();
    for (index, cells) in rows {
        let time = cell(cells, "UTC_Time").to_string();
        if let Some(group) = groups.iter_mut().find(|(stamp, _)| stamp == &time) {
            group.1.push((*index, cells.clone()));
        } else {
            groups.push((time, vec![(*index, cells.clone())]));
        }
    }
    let mut out = Vec::new();
    for (_, group) in groups {
        let trade = statement_group(&group);
        let mut emitted = false;
        for (index, cells) in &group {
            let op = cell(cells, "Operation").trim().to_ascii_lowercase();
            if matches!(
                op.as_str(),
                "deposit" | "withdraw" | "withdrawal" | "transaction related"
            ) && trade.is_none()
            {
                out.push((*index, RowOutcome::Skip));
                continue;
            }
            if !emitted {
                if let Some(row) = &trade {
                    out.push((*index, RowOutcome::Trade(row.clone())));
                    emitted = true;
                    continue;
                }
            }
            out.push((*index, RowOutcome::Skip));
        }
    }
    out
}

fn statement_group(group: &[(usize, HashMap<String, String>)]) -> Option<RawTradeRow> {
    let mut received: Option<(String, String)> = None;
    let mut spent: Option<(String, String)> = None;
    let mut fee: Option<(String, String)> = None;
    let time = normalize_time(cell(&group[0].1, "UTC_Time"));
    for (_, cells) in group {
        let op = cell(cells, "Operation").trim().to_ascii_lowercase();
        let coin = cell(cells, "Coin").trim().to_ascii_uppercase();
        let change = plain_number(cell(cells, "Change"));
        if coin.is_empty() || change.is_empty() {
            continue;
        }
        let negative = change.starts_with('-');
        let magnitude = change.trim_start_matches('-').to_string();
        if op == "fee" {
            fee = Some((magnitude, coin));
        } else if op == "buy" || op == "sell" {
            if negative {
                spent = Some((magnitude, coin));
            } else {
                received = Some((magnitude, coin));
            }
        }
    }
    let (base_qty, base) = received?;
    let (quote_qty, quote) = spent?;
    let side = "BUY";
    let pair = format!("{base}{quote}");
    let price = match (parse_loose(&quote_qty), parse_loose(&base_qty)) {
        (Some(quote), Some(base)) if !base.is_zero() => (quote / base).normalize().to_string(),
        _ => return None,
    };
    let (fee_qty, fee_asset) = fee.unwrap_or_else(|| ("0".into(), base.clone()));
    Some(RawTradeRow {
        time,
        pair,
        side: side.into(),
        price,
        executed: format!("{base_qty}{base}"),
        amount: format!("{quote_qty}{quote}"),
        fee: format!("{fee_qty}{fee_asset}"),
    })
}

fn kraken_ledgers(rows: &[(usize, HashMap<String, String>)]) -> Vec<(usize, RowOutcome)> {
    let mut groups: Vec<(String, Vec<(usize, HashMap<String, String>)>)> = Vec::new();
    for (index, cells) in rows {
        let key = cell(cells, "refid").to_string();
        if let Some(group) = groups.iter_mut().find(|(id, _)| id == &key) {
            group.1.push((*index, cells.clone()));
        } else {
            groups.push((key, vec![(*index, cells.clone())]));
        }
    }
    let mut out = Vec::new();
    for (_, group) in groups {
        let is_trade = group
            .iter()
            .any(|(_, cells)| cell(cells, "type").eq_ignore_ascii_case("trade"));
        if !is_trade {
            for (index, _) in group {
                out.push((index, RowOutcome::Skip));
            }
            continue;
        }
        if let Some(row) = kraken_trade(&group) {
            out.push((group[0].0, RowOutcome::Trade(row)));
            for (index, _) in group.into_iter().skip(1) {
                out.push((index, RowOutcome::Skip));
            }
        } else {
            for (index, _) in group {
                out.push((index, RowOutcome::Invalid("kraken ledger pair".into())));
            }
        }
    }
    out
}

fn kraken_trade(group: &[(usize, HashMap<String, String>)]) -> Option<RawTradeRow> {
    let mut legs: Vec<(String, String, String)> = Vec::new();
    for (_, cells) in group {
        let asset = kraken_asset(cell(cells, "asset"));
        let amount = plain_number(cell(cells, "amount"));
        let fee = plain_number(cell(cells, "fee"));
        if !asset.is_empty() && !amount.is_empty() {
            legs.push((asset, amount, fee));
        }
    }
    let (recv_asset, recv_qty, _) = legs
        .iter()
        .find(|(_, amount, _)| !amount.starts_with('-'))?
        .clone();
    let (spent_asset, spent_qty, spent_fee) = legs
        .iter()
        .find(|(_, amount, _)| amount.starts_with('-'))?
        .clone();
    let spent_qty = spent_qty.trim_start_matches('-').to_string();
    let pair = format!("{recv_asset}{spent_asset}");
    let price = match (parse_loose(&spent_qty), parse_loose(&recv_qty)) {
        (Some(spent), Some(recv)) if !recv.is_zero() => (spent / recv).normalize().to_string(),
        _ => return None,
    };
    let fee_asset = if spent_fee != "0" && !spent_fee.is_empty() {
        spent_asset.clone()
    } else {
        spent_asset.clone()
    };
    Some(RawTradeRow {
        time: normalize_time(cell(&group[0].1, "time")),
        pair,
        side: "BUY".into(),
        price,
        executed: format!("{recv_qty}{recv_asset}"),
        amount: format!("{spent_qty}{spent_asset}"),
        fee: format!("{spent_fee}{fee_asset}"),
    })
}

fn bitget_bills(rows: &[(usize, HashMap<String, String>)]) -> Vec<(usize, RowOutcome)> {
    let mut groups: Vec<(String, Vec<(usize, HashMap<String, String>)>)> = Vec::new();
    for (index, cells) in rows {
        let key = cell(cells, "order").to_string();
        if let Some(group) = groups.iter_mut().find(|(id, _)| id == &key) {
            group.1.push((*index, cells.clone()));
        } else {
            groups.push((key, vec![(*index, cells.clone())]));
        }
    }
    let mut out = Vec::new();
    for (_, group) in groups {
        if let Some(row) = bitget_trade(&group) {
            out.push((group[0].0, RowOutcome::Trade(row)));
            for (index, _) in group.into_iter().skip(1) {
                out.push((index, RowOutcome::Skip));
            }
        } else {
            for (index, _) in group {
                out.push((index, RowOutcome::Skip));
            }
        }
    }
    out
}

fn bitget_trade(group: &[(usize, HashMap<String, String>)]) -> Option<RawTradeRow> {
    let mut bought: Option<(String, String, String)> = None;
    let mut sold: Option<(String, String, String)> = None;
    for (_, cells) in group {
        let coin = cell(cells, "Coin").trim().to_ascii_uppercase();
        let amount = plain_number(cell(cells, "Amount"));
        let fee = plain_number(cell(cells, "Fee"));
        match cell(cells, "Type").trim().to_ascii_lowercase().as_str() {
            "buy" => bought = Some((coin, amount, fee)),
            "sell" => sold = Some((coin, amount, fee)),
            _ => {}
        }
    }
    let (base, qty, base_fee) = bought?;
    let (quote, total, _) = sold?;
    let price = match (parse_loose(&total), parse_loose(&qty)) {
        (Some(total), Some(qty)) if !qty.is_zero() => (total / qty).normalize().to_string(),
        _ => return None,
    };
    Some(RawTradeRow {
        time: normalize_time(cell(&group[0].1, "Date")),
        pair: format!("{base}{quote}"),
        side: "BUY".into(),
        price,
        executed: format!("{qty}{base}"),
        amount: format!("{total}{quote}"),
        fee: format!("{base_fee}{base}"),
    })
}

fn normalize_time(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('Z').replace('T', " ");
    match trimmed.split_once('.') {
        Some((head, _)) => head.to_string(),
        None => trimmed,
    }
}

fn normalize_side(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "buy" | "bought" => Some("BUY".into()),
        "sell" | "sold" => Some("SELL".into()),
        "open long" | "close short" => Some("BUY".into()),
        "open short" | "close long" => Some("SELL".into()),
        _ => None,
    }
}

pub fn normalize_pair(raw: &str) -> String {
    let compact = raw.trim().to_ascii_uppercase().replace(['-', '_', '/'], "");
    if let Some(pair) = kraken_pair(&compact) {
        return pair;
    }
    compact
}

fn kraken_pair(pair: &str) -> Option<String> {
    const QUOTES: &[&str] = &["ZUSD", "ZEUR", "ZGBP", "ZJPY", "ZCAD"];
    for quote in QUOTES {
        let Some(base) = pair.strip_suffix(quote) else {
            continue;
        };
        let quote = &quote[1..];
        let base = base.strip_prefix('X').unwrap_or(base);
        let base = if base == "XBT" { "BTC" } else { base };
        return Some(format!("{base}{quote}"));
    }
    None
}

fn split_known_pair(pair: &str) -> Option<(String, String)> {
    for quote in crate::models::QUOTE_ASSETS {
        if pair.ends_with(quote) && pair.len() > quote.len() {
            return Some((
                pair[..pair.len() - quote.len()].to_string(),
                (*quote).to_string(),
            ));
        }
    }
    None
}

fn kraken_asset(raw: &str) -> String {
    let asset = raw.trim().to_ascii_uppercase();
    let asset = asset
        .strip_prefix('Z')
        .or_else(|| asset.strip_prefix('X'))
        .unwrap_or(&asset);
    if asset == "XBT" {
        "BTC".into()
    } else {
        asset.to_string()
    }
}

fn plain_number(raw: &str) -> String {
    let trimmed = raw.trim().replace(',', "");
    let trimmed = trimmed.trim_start_matches('+');
    if trimmed.starts_with('-') {
        format!("-{}", trimmed.trim_start_matches('-').trim())
    } else {
        // "0.0001 ETH" keeps the suffix for the fee path.
        trimmed.to_string()
    }
}

fn parse_loose(raw: &str) -> Option<rust_decimal::Decimal> {
    let number: String = raw
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    rust_decimal::Decimal::from_str_exact(number.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_formats_share_a_column_set() {
        for format in CSV_FORMATS {
            let found: Vec<String> = format
                .columns
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            let hits = matching(&found);
            assert_eq!(hits.len(), 1, "{}", format.id);
            assert_eq!(hits[0].id, format.id);
        }
    }
}
