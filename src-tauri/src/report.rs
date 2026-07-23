// src-tauri/src/report.rs
//
// Self-contained HTML report generation.
//
// "Self-contained" is a hard requirement: the file must render identically after
// being emailed to an accountant on a machine with no network. So there are no
// external stylesheets, no web fonts, and no <script> tags — CSS is inlined and
// charts arrive as base64 PNG data URIs that the frontend produced with ECharts'
// `getDataURL()`.
//
// PDF is deliberately NOT generated here. Opening the report in a window and
// letting the user print to PDF uses the OS print pipeline, which is the only
// approach that works across platforms without bundling a headless browser.

use std::path::Path;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{AnalysisResult, Language, PnlMethod};

/// A chart rendered by the frontend and handed over for embedding.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartImage {
    pub title: String,
    /// `data:image/png;base64,...` produced by ECharts' `getDataURL()`.
    pub data_uri: String,
}

impl ChartImage {
    /// Only PNG/JPEG data URIs are embeddable.
    ///
    /// This is the one place where content crosses from the frontend into a file
    /// that gets shared onward, so anything that is not obviously an image is
    /// dropped rather than written into the document.
    fn is_safe(&self) -> bool {
        self.data_uri.starts_with("data:image/png;base64,")
            || self.data_uri.starts_with("data:image/jpeg;base64,")
    }
}

/// Report options chosen in the UI.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportOptions {
    pub language: Language,
    #[serde(default)]
    pub charts: Vec<ChartImage>,
}

/// Everything the report writes, in both supported languages.
struct Labels {
    title: &'static str,
    generated_at: &'static str,
    method: &'static str,
    method_fifo: &'static str,
    method_avg: &'static str,
    summary: &'static str,
    realized: &'static str,
    unrealized: &'static str,
    portfolio_value: &'static str,
    win_rate: &'static str,
    total_trades: &'static str,
    total_fees: &'static str,
    portfolio: &'static str,
    pair: &'static str,
    quantity: &'static str,
    avg_cost: &'static str,
    current_price: &'static str,
    market_value: &'static str,
    total_pnl: &'static str,
    closed_positions: &'static str,
    charts: &'static str,
    warnings: &'static str,
    oversell_note: &'static str,
    excluded_note: &'static str,
    fee_note: &'static str,
    stale_note: &'static str,
    data_range: &'static str,
    rows_imported: &'static str,
    rows_skipped: &'static str,
    rows_duplicate: &'static str,
    no_data: &'static str,
    price_time: &'static str,
    approx: &'static str,
}

const TR: Labels = Labels {
    title: "Fiyatlio Raporu",
    generated_at: "Oluşturulma",
    method: "Yöntem",
    method_fifo: "FIFO",
    method_avg: "Ortalama Maliyet",
    summary: "Özet",
    realized: "Gerçekleşen K/Z",
    unrealized: "Gerçekleşmemiş K/Z",
    portfolio_value: "Portföy Değeri",
    win_rate: "Kazanç Oranı",
    total_trades: "Toplam İşlem",
    total_fees: "Toplam Komisyon",
    portfolio: "Portföy",
    pair: "Parite",
    quantity: "Miktar",
    avg_cost: "Ort. Maliyet",
    current_price: "Güncel Fiyat",
    market_value: "Piyasa Değeri",
    total_pnl: "Toplam K/Z",
    closed_positions: "Kapanan Pozisyonlar",
    charts: "Grafikler",
    warnings: "Uyarılar",
    oversell_note: "Elde olandan fazla satış tespit edildi. Aşan miktar sıfır maliyet esasıyla eşleştirildi; bu bakiye büyük olasılıkla transfer, convert veya staking yoluyla geldi ve CSV'de yer almıyor.",
    excluded_note: "Aşağıdaki pariteler USDT'ye çevrilemediği için toplamlara dahil edilmedi:",
    fee_note: "BNB gibi üçüncü bir varlıkta ödenen komisyonlar maliyet tabanına katılmaz (işlem anındaki tarihsel fiyat bilinmiyor); güncel fiyatla yaklaşık olarak gösterilir.",
    stale_note: "Fiyatlar canlı değil, önbellekten alındı.",
    data_range: "Veri Aralığı",
    rows_imported: "İşlenen satır",
    rows_skipped: "Atlanan satır",
    rows_duplicate: "Mükerrer satır",
    no_data: "Veri yok",
    price_time: "Fiyat zamanı",
    approx: "yaklaşık",
};

const EN: Labels = Labels {
    title: "Fiyatlio Report",
    generated_at: "Generated",
    method: "Method",
    method_fifo: "FIFO",
    method_avg: "Average Cost",
    summary: "Summary",
    realized: "Realized P&L",
    unrealized: "Unrealized P&L",
    portfolio_value: "Portfolio Value",
    win_rate: "Win Rate",
    total_trades: "Total Trades",
    total_fees: "Total Fees",
    portfolio: "Portfolio",
    pair: "Pair",
    quantity: "Quantity",
    avg_cost: "Avg Cost",
    current_price: "Current Price",
    market_value: "Market Value",
    total_pnl: "Total P&L",
    closed_positions: "Closed Positions",
    charts: "Charts",
    warnings: "Warnings",
    oversell_note: "Sales exceeding tracked inventory were detected. The excess was matched at a zero cost basis; that balance most likely arrived through a transfer, convert or staking reward and is absent from the CSV.",
    excluded_note: "The following pairs were excluded from the totals because their quote asset could not be converted to USDT:",
    fee_note: "Fees paid in a third asset such as BNB are not folded into cost basis (the historical rate at fill time is unknown); they are shown approximately at the current price.",
    stale_note: "Prices are not live — they were restored from cache.",
    data_range: "Data Range",
    rows_imported: "Rows imported",
    rows_skipped: "Rows skipped",
    rows_duplicate: "Duplicate rows",
    no_data: "No data",
    price_time: "Price timestamp",
    approx: "approx.",
};

fn labels(language: Language) -> &'static Labels {
    match language {
        Language::Tr => &TR,
        Language::En => &EN,
    }
}

/// Escapes text that will be placed into HTML element content or an attribute.
///
/// Pair symbols, file paths and asset codes all originate from user files, so
/// none of them are trusted to be markup-free.
fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// USDT-style formatting: 2 decimals.
fn money(value: Decimal) -> String {
    format!("{:.2}", value.round_dp(2))
}

/// Coin-quantity formatting: up to 8 decimals, trailing zeros trimmed so a whole
/// number does not read as `1.00000000`.
fn qty(value: Decimal) -> String {
    let rounded = value.round_dp(8).normalize();
    rounded.to_string()
}

fn pnl_class(value: Decimal) -> &'static str {
    if value > Decimal::ZERO {
        "profit"
    } else if value < Decimal::ZERO {
        "loss"
    } else {
        "flat"
    }
}

fn optional_money(value: Option<Decimal>, fallback: &str) -> String {
    value.map(money).unwrap_or_else(|| fallback.to_string())
}

/// Builds the complete HTML document.
pub fn render(analysis: &AnalysisResult, options: &ReportOptions) -> String {
    let l = labels(options.language);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

    let method_name = match analysis.method {
        PnlMethod::Fifo => l.method_fifo,
        PnlMethod::AverageCost => l.method_avg,
    };

    let mut html = String::with_capacity(16 * 1024);

    html.push_str(&format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  :root {{
    --fg: #0f172a; --muted: #64748b; --border: #e2e8f0; --bg: #ffffff;
    --card: #f8fafc; --profit: #15803d; --loss: #b91c1c; --warn: #b45309;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; padding: 32px; background: var(--bg); color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    font-size: 14px; line-height: 1.5;
  }}
  h1 {{ font-size: 24px; margin: 0 0 4px; }}
  h2 {{ font-size: 17px; margin: 32px 0 12px; padding-bottom: 6px; border-bottom: 1px solid var(--border); }}
  .meta {{ color: var(--muted); font-size: 12px; margin-bottom: 24px; }}
  .cards {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; }}
  .card {{ background: var(--card); border: 1px solid var(--border); border-radius: 10px; padding: 14px 16px; }}
  .card .label {{ color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; }}
  .card .value {{ font-size: 20px; font-weight: 600; margin-top: 4px; font-variant-numeric: tabular-nums; }}
  table {{ width: 100%; border-collapse: collapse; margin-top: 8px; font-variant-numeric: tabular-nums; }}
  th, td {{ text-align: right; padding: 7px 10px; border-bottom: 1px solid var(--border); }}
  th:first-child, td:first-child {{ text-align: left; }}
  th {{ color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .04em; font-weight: 600; }}
  .profit {{ color: var(--profit); }}
  .loss {{ color: var(--loss); }}
  .flat {{ color: var(--muted); }}
  .note {{ background: #fffbeb; border: 1px solid #fde68a; color: var(--warn);
           border-radius: 8px; padding: 10px 14px; margin: 10px 0; font-size: 13px; }}
  .chart {{ margin: 16px 0; page-break-inside: avoid; }}
  .chart img {{ max-width: 100%; height: auto; border: 1px solid var(--border); border-radius: 8px; }}
  .chart .caption {{ color: var(--muted); font-size: 12px; margin-top: 4px; }}
  .empty {{ color: var(--muted); font-style: italic; }}
  @media print {{
    body {{ padding: 0; }}
    h2 {{ page-break-after: avoid; }}
    table {{ page-break-inside: auto; }}
    tr {{ page-break-inside: avoid; }}
  }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="meta">{generated}: {now} &nbsp;·&nbsp; {method_label}: {method_name}</div>
"#,
        lang = match options.language {
            Language::Tr => "tr",
            Language::En => "en",
        },
        title = escape(l.title),
        generated = escape(l.generated_at),
        method_label = escape(l.method),
        method_name = escape(method_name),
        now = now,
    ));

    // --- summary cards ----------------------------------------------------
    let s = &analysis.summary;
    html.push_str(&format!(
        "<h2>{}</h2>\n<div class=\"cards\">\n",
        escape(l.summary)
    ));

    let cards: [(&str, String, &str); 6] = [
        (
            l.realized,
            format!("{} USDT", money(s.total_realized_pnl_usdt)),
            pnl_class(s.total_realized_pnl_usdt),
        ),
        (
            l.unrealized,
            format!("{} USDT", money(s.total_unrealized_pnl_usdt)),
            pnl_class(s.total_unrealized_pnl_usdt),
        ),
        (
            l.portfolio_value,
            format!("{} USDT", money(s.total_portfolio_value_usdt)),
            "",
        ),
        (
            l.win_rate,
            format!(
                "{}%  ({}/{})",
                money(s.win_rate),
                s.winning_events,
                s.total_sell_events
            ),
            "",
        ),
        (l.total_trades, s.total_trades.to_string(), ""),
        (
            l.total_fees,
            format!("≈ {} USDT", money(s.total_fees_usdt_approx)),
            "",
        ),
    ];

    for (label, value, class) in cards {
        html.push_str(&format!(
            "<div class=\"card\"><div class=\"label\">{}</div><div class=\"value {}\">{}</div></div>\n",
            escape(label),
            class,
            escape(&value)
        ));
    }
    html.push_str("</div>\n");

    // --- import metadata --------------------------------------------------
    let range = match (
        analysis.import.first_trade_at,
        analysis.import.last_trade_at,
    ) {
        (Some(first), Some(last)) => {
            format!("{} — {}", first.format("%Y-%m-%d"), last.format("%Y-%m-%d"))
        }
        _ => l.no_data.to_string(),
    };
    html.push_str(&format!(
        "<div class=\"meta\" style=\"margin-top:16px\">{}: {} &nbsp;·&nbsp; {}: {} &nbsp;·&nbsp; {}: {} &nbsp;·&nbsp; {}: {}</div>\n",
        escape(l.data_range),
        escape(&range),
        escape(l.rows_imported),
        analysis.import.valid_rows,
        escape(l.rows_skipped),
        analysis.import.invalid_rows,
        escape(l.rows_duplicate),
        analysis.import.duplicate_rows,
    ));

    // --- warnings ---------------------------------------------------------
    let mut notes: Vec<String> = Vec::new();
    if s.has_oversell {
        notes.push(escape(l.oversell_note));
    }
    if !s.excluded_pairs.is_empty() {
        notes.push(format!(
            "{} {}",
            escape(l.excluded_note),
            escape(&s.excluded_pairs.join(", "))
        ));
    }
    if s.fees_conversion_partial || s.total_fees_by_asset.keys().any(|a| a != "USDT") {
        notes.push(escape(l.fee_note));
    }
    if let Some(snapshot) = &s.price_snapshot {
        if snapshot.stale {
            notes.push(escape(l.stale_note));
        }
    }

    if !notes.is_empty() {
        html.push_str(&format!("<h2>{}</h2>\n", escape(l.warnings)));
        for note in notes {
            html.push_str(&format!("<div class=\"note\">{note}</div>\n"));
        }
    }

    // --- portfolio --------------------------------------------------------
    let open: Vec<_> = analysis
        .positions
        .iter()
        .filter(|v| v.position.qty > Decimal::ZERO)
        .collect();
    let closed: Vec<_> = analysis
        .positions
        .iter()
        .filter(|v| v.position.qty <= Decimal::ZERO)
        .collect();

    html.push_str(&format!("<h2>{}</h2>\n", escape(l.portfolio)));
    if open.is_empty() {
        html.push_str(&format!("<p class=\"empty\">{}</p>\n", escape(l.no_data)));
    } else {
        html.push_str(&format!(
            "<table><thead><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr></thead><tbody>\n",
            escape(l.pair),
            escape(l.quantity),
            escape(l.avg_cost),
            escape(l.current_price),
            escape(l.market_value),
            escape(l.realized),
            escape(l.unrealized),
            escape(l.total_pnl),
        ));

        for v in &open {
            let realized = v.position.realized_pnl;
            let unrealized = v.unrealized_pnl.unwrap_or(Decimal::ZERO);
            let total = realized + unrealized;
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td></tr>\n",
                escape(&v.position.pair),
                escape(&qty(v.position.qty)),
                escape(&money(v.position.avg_cost)),
                escape(&optional_money(v.current_price, "—")),
                escape(&optional_money(v.market_value_quote, "—")),
                pnl_class(realized),
                escape(&money(realized)),
                pnl_class(unrealized),
                escape(&v.unrealized_pnl.map(money).unwrap_or_else(|| "—".into())),
                pnl_class(total),
                escape(&money(total)),
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    if !closed.is_empty() {
        html.push_str(&format!("<h2>{}</h2>\n", escape(l.closed_positions)));
        html.push_str(&format!(
            "<table><thead><tr><th>{}</th><th>{}</th></tr></thead><tbody>\n",
            escape(l.pair),
            escape(l.realized),
        ));
        for v in &closed {
            html.push_str(&format!(
                "<tr><td>{}</td><td class=\"{}\">{}</td></tr>\n",
                escape(&v.position.pair),
                pnl_class(v.position.realized_pnl),
                escape(&money(v.position.realized_pnl)),
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    // --- charts -----------------------------------------------------------
    let usable: Vec<&ChartImage> = options.charts.iter().filter(|c| c.is_safe()).collect();
    if !usable.is_empty() {
        html.push_str(&format!("<h2>{}</h2>\n", escape(l.charts)));
        for chart in usable {
            html.push_str(&format!(
                "<div class=\"chart\"><img src=\"{}\" alt=\"{}\"><div class=\"caption\">{}</div></div>\n",
                escape(&chart.data_uri),
                escape(&chart.title),
                escape(&chart.title),
            ));
        }
    }

    // --- price provenance -------------------------------------------------
    if let Some(snapshot) = &s.price_snapshot {
        html.push_str(&format!(
            "<div class=\"meta\" style=\"margin-top:24px\">{}: {} UTC{}</div>\n",
            escape(l.price_time),
            snapshot.fetched_at.format("%Y-%m-%d %H:%M:%S"),
            if snapshot.stale {
                format!(" ({})", escape(l.approx))
            } else {
                String::new()
            }
        ));
    }

    html.push_str("</body>\n</html>\n");
    html
}

/// Renders the report and writes it to `path`, returning the path written.
pub fn write(analysis: &AnalysisResult, options: &ReportOptions, path: &Path) -> AppResult<String> {
    let html = render(analysis, options);
    std::fs::write(path, html).map_err(|source| AppError::FileWrite {
        path: path.to_string_lossy().to_string(),
        source,
    })?;
    Ok(path.to_string_lossy().to_string())
}

/// Date-stamped default file name for the save dialog.
pub fn suggested_filename() -> String {
    format!(
        "fiyatlio-report-{}.html",
        chrono::Local::now().format("%Y-%m-%d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::build_analysis;
    use crate::models::{ImportSummary, Settings};

    fn empty_report(language: Language) -> String {
        let analysis = build_analysis(&[], &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");
        render(
            &analysis,
            &ReportOptions {
                language,
                charts: Vec::new(),
            },
        )
    }

    #[test]
    fn report_is_self_contained() {
        let html = empty_report(Language::Tr);
        // No network dependencies of any kind, and no executable content.
        assert!(!html.contains("<script"), "report must not contain scripts");
        assert!(
            !html.contains("http://"),
            "report must not reference remote hosts"
        );
        assert!(
            !html.contains("https://"),
            "report must not reference remote hosts"
        );
        assert!(html.contains("<style>"), "styles must be inlined");
    }

    #[test]
    fn report_renders_in_both_languages() {
        assert!(empty_report(Language::Tr).contains("Fiyatlio Raporu"));
        assert!(empty_report(Language::En).contains("Fiyatlio Report"));
    }

    #[test]
    fn markup_in_user_data_is_escaped() {
        let evil = escape("<script>alert('x')</script>");
        assert!(!evil.contains('<'));
        assert!(evil.contains("&lt;script&gt;"));
    }

    #[test]
    fn only_image_data_uris_are_embedded() {
        let safe = ChartImage {
            title: "ok".into(),
            data_uri: "data:image/png;base64,AAAA".into(),
        };
        let unsafe_html = ChartImage {
            title: "bad".into(),
            data_uri: "data:text/html;base64,PHNjcmlwdD4=".into(),
        };
        let remote = ChartImage {
            title: "remote".into(),
            data_uri: "https://example.com/chart.png".into(),
        };

        assert!(safe.is_safe());
        assert!(!unsafe_html.is_safe());
        assert!(!remote.is_safe());

        let analysis = build_analysis(&[], &ImportSummary::empty(), &Settings::default(), None)
            .expect("analysis builds");
        let html = render(
            &analysis,
            &ReportOptions {
                language: Language::En,
                charts: vec![safe, unsafe_html, remote],
            },
        );

        assert!(html.contains("data:image/png;base64,AAAA"));
        assert!(!html.contains("data:text/html"));
        assert!(!html.contains("example.com"));
    }

    #[test]
    fn quantities_drop_trailing_zeros_but_money_keeps_two_places() {
        assert_eq!(qty(rust_decimal_macros::dec!(1.00000000)), "1");
        assert_eq!(qty(rust_decimal_macros::dec!(0.26810000)), "0.2681");
        assert_eq!(money(rust_decimal_macros::dec!(1234.5)), "1234.50");
    }
}
