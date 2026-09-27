"""Markdown, CSV, HTML ve Excel raporları. Metinler Türkçe, dosya UTF-8."""

from __future__ import annotations

import csv
import html
from datetime import datetime
from decimal import Decimal
from pathlib import Path

import pandas as pd

from fiyatlio.cycles import Analysis, LedgerResult
from fiyatlio.inventory import Consumption
from fiyatlio.util import fmt_pct, fmt_plain, fmt_signed_tr, fmt_tr, quantize

_DONEM_COLS = [
    "varlik",
    "quote",
    "donem_no",
    "acilis",
    "kapanis",
    "durum",
    "alinan_qty",
    "odenen_quote",
    "satilan_qty",
    "alinan_quote",
    "ortalama_alis",
    "ortalama_satis",
    "gerceklesen_pnl",
    "gerceklesen_pnl_yuzde",
    "komisyon_base",
    "komisyon_quote",
    "komisyon_diger",
    "kalan_qty",
    "kalan_maliyet",
    "kalan_ortalama",
    "olaylar",
]

_FILL_COLS = [
    "zaman",
    "emir_no",
    "parite",
    "varlik",
    "quote",
    "taraf",
    "fiyat",
    "executed_qty",
    "amount_qty",
    "fee_qty",
    "fee_asset",
    "net_etki",
    "quote_harcama",
    "quote_gelir",
    "birim_maliyet",
    "birim_gelir",
    "event_type",
    "donem_no",
    "gerceklesen_pnl",
    "tuketilen_lotlar",
    "uyari",
    "dosya",
    "satir",
]

_LOT_COLS = [
    "varlik",
    "quote",
    "miktar",
    "birim_maliyet",
    "maliyet",
    "fill_fiyat",
    "zaman",
    "emir_no",
    "parite",
    "sentetik",
]

_VARLIK_COLS = [
    "varlik",
    "quote",
    "net_qty",
    "cuzdan_qty",
    "toplam_maliyet",
    "agirlikli_maliyet",
    "gerceklesen_pnl",
    "anlik_fiyat",
    "deger",
    "acik_pnl",
    "acik_pnl_yuzde",
    "uyarilar",
]

_EMIR_COLS = [
    "zaman",
    "emir_no",
    "parite",
    "taraf",
    "fill_sayisi",
    "executed_qty",
    "quote_qty",
    "vwap",
    "fee_base",
    "fee_quote",
    "fee_diger",
    "event_type",
    "donem_no",
    "gerceklesen_pnl",
]


def write_reports(analysis: Analysis, out_dir: str | Path) -> Path:
    target = Path(out_dir)
    target.mkdir(parents=True, exist_ok=True)
    donemler = _donem_rows(analysis)
    filller = _fill_rows(analysis)
    lotlar = _lot_rows(analysis)
    varliklar = _varlik_rows(analysis)
    emirler = _emir_rows(analysis)

    _write_csv(target / "donemler.csv", donemler, _DONEM_COLS)
    _write_csv(target / "filller.csv", filller, _FILL_COLS)
    _write_csv(target / "kalan_lotlar.csv", lotlar, _LOT_COLS)
    _write_csv(target / "varlik_ozeti.csv", varliklar, _VARLIK_COLS)
    (target / "ozet.md").write_text(_markdown(analysis), encoding="utf-8")
    (target / "rapor.html").write_text(_html(analysis), encoding="utf-8")
    _write_xlsx(target / "rapor.xlsx", donemler, filller, lotlar, varliklar, emirler, analysis)
    return target


def _write_csv(path: Path, rows: list[dict], columns: list[str]) -> None:
    with path.open("w", encoding="utf-8-sig", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow({key: _csv_cell(row.get(key)) for key in columns})


def _csv_cell(value: object) -> str:
    if value is None:
        return ""
    if isinstance(value, Decimal):
        return fmt_plain(value, 8)
    if isinstance(value, datetime):
        return value.strftime("%Y-%m-%d %H:%M:%S")
    return str(value)


def _xlsx_cell(value: object) -> object:
    if isinstance(value, Decimal):
        return float(quantize(value, 8))
    if isinstance(value, datetime):
        return value.strftime("%Y-%m-%d %H:%M:%S")
    return value


def _frame(rows: list[dict], columns: list[str]) -> pd.DataFrame:
    data = [{key: _xlsx_cell(row.get(key)) for key in columns} for row in rows]
    return pd.DataFrame(data, columns=columns)


def _write_xlsx(path, donemler, filller, lotlar, varliklar, emirler, analysis: Analysis) -> None:
    uyarilar = [{"uyari": text} for text in _unique(analysis.warnings)]
    for ledger in analysis.ledgers:
        for text in ledger.warnings:
            uyarilar.append({"uyari": text})
    ceviriler = [
        {
            "zaman": item.time,
            "emir_no": item.order_no,
            "parite": item.pair,
            "taraf": item.side,
            "fiyat": item.price,
            "executed_qty": item.executed_qty,
            "amount_qty": item.amount_qty,
            "fee_qty": item.fee_qty,
            "fee_asset": item.fee_asset or "",
        }
        for item in analysis.conversions
    ]
    with pd.ExcelWriter(path, engine="openpyxl") as writer:
        _frame(varliklar, _VARLIK_COLS).to_excel(writer, sheet_name="Varlık Özeti", index=False)
        _frame(donemler, _DONEM_COLS).to_excel(writer, sheet_name="Dönemler", index=False)
        _frame(emirler, _EMIR_COLS).to_excel(writer, sheet_name="Emirler", index=False)
        _frame(filller, _FILL_COLS).to_excel(writer, sheet_name="Filller", index=False)
        _frame(lotlar, _LOT_COLS).to_excel(writer, sheet_name="Kalan Lotlar", index=False)
        pd.DataFrame(uyarilar or [{"uyari": ""}]).to_excel(writer, sheet_name="Uyarılar", index=False)
        pd.DataFrame(ceviriler or [{"zaman": ""}]).to_excel(writer, sheet_name="Çeviriler", index=False)


def _donem_rows(analysis: Analysis) -> list[dict]:
    rows: list[dict] = []
    for ledger in analysis.ledgers:
        for cycle in ledger.cycles:
            rows.append(
                {
                    "varlik": cycle.asset,
                    "quote": cycle.quote,
                    "donem_no": cycle.number,
                    "acilis": cycle.opened,
                    "kapanis": cycle.close_label,
                    "durum": cycle.status,
                    "alinan_qty": cycle.bought_qty,
                    "odenen_quote": cycle.bought_quote,
                    "satilan_qty": cycle.sold_qty,
                    "alinan_quote": cycle.sold_quote,
                    "ortalama_alis": cycle.avg_buy,
                    "ortalama_satis": cycle.avg_sell,
                    "gerceklesen_pnl": cycle.realized,
                    "gerceklesen_pnl_yuzde": cycle.realized_pct,
                    "komisyon_base": cycle.fee_base,
                    "komisyon_quote": cycle.fee_quote,
                    "komisyon_diger": cycle.fee_other,
                    "kalan_qty": cycle.remaining_qty,
                    "kalan_maliyet": cycle.remaining_cost,
                    "kalan_ortalama": cycle.remaining_avg,
                    "olaylar": " ; ".join(cycle.events),
                }
            )
    return rows


def _fill_rows(analysis: Analysis) -> list[dict]:
    rows: list[dict] = []
    for ledger in analysis.ledgers:
        for view in ledger.fills:
            fill = view.fill
            result = view.result
            rows.append(
                {
                    "zaman": fill.time,
                    "emir_no": fill.order_no,
                    "parite": fill.pair,
                    "varlik": fill.base_asset,
                    "quote": fill.quote_asset,
                    "taraf": fill.side,
                    "fiyat": fill.price,
                    "executed_qty": fill.executed_qty,
                    "amount_qty": fill.amount_qty,
                    "fee_qty": fill.fee_qty,
                    "fee_asset": fill.fee_asset or "",
                    "net_etki": result.added_qty - result.sale_qty - result.missing_qty,
                    "quote_harcama": result.added_quote,
                    "quote_gelir": result.sale_proceeds,
                    "birim_maliyet": result.unit_cost,
                    "birim_gelir": result.unit_proceeds,
                    "event_type": view.event_type,
                    "donem_no": view.cycle_no if view.cycle_no is not None else "",
                    "gerceklesen_pnl": result.realized,
                    "tuketilen_lotlar": _tuketim(result.consumptions),
                    "uyari": result.warning or "",
                    "dosya": fill.source_file,
                    "satir": fill.source_row,
                }
            )
    return rows


def _lot_rows(analysis: Analysis) -> list[dict]:
    rows: list[dict] = []
    for ledger in analysis.ledgers:
        for lot in ledger.lots:
            rows.append(
                {
                    "varlik": lot.base_asset,
                    "quote": lot.quote_asset,
                    "miktar": lot.qty,
                    "birim_maliyet": lot.unit_cost_quote,
                    "maliyet": lot.cost,
                    "fill_fiyat": lot.fill_price,
                    "zaman": lot.time,
                    "emir_no": lot.order_no,
                    "parite": lot.pair,
                    "sentetik": "evet" if lot.synthetic else "hayır",
                }
            )
    return rows


def _varlik_rows(analysis: Analysis) -> list[dict]:
    rows: list[dict] = []
    for ledger in analysis.ledgers:
        notes = ledger.warnings + ledger.notes
        rows.append(
            {
                "varlik": ledger.base,
                "quote": ledger.quote,
                "net_qty": ledger.net_qty,
                "cuzdan_qty": ledger.wallet_qty,
                "toplam_maliyet": ledger.total_cost,
                "agirlikli_maliyet": ledger.avg_cost,
                "gerceklesen_pnl": ledger.realized,
                "anlik_fiyat": ledger.price,
                "deger": ledger.market_value(),
                "acik_pnl": ledger.open_pnl(),
                "acik_pnl_yuzde": ledger.open_pnl_pct(),
                "uyarilar": " ; ".join(_unique(notes)),
            }
        )
    return rows


def _emir_rows(analysis: Analysis) -> list[dict]:
    rows: list[dict] = []
    for ledger in analysis.ledgers:
        for order in ledger.orders:
            rows.append(
                {
                    "zaman": order.time,
                    "emir_no": order.order_no,
                    "parite": order.pair,
                    "taraf": order.side,
                    "fill_sayisi": order.fill_count,
                    "executed_qty": order.executed_qty,
                    "quote_qty": order.quote_qty,
                    "vwap": order.vwap,
                    "fee_base": order.fee_base,
                    "fee_quote": order.fee_quote,
                    "fee_diger": order.fee_other,
                    "event_type": order.event_type,
                    "donem_no": order.cycle_no if order.cycle_no is not None else "",
                    "gerceklesen_pnl": order.realized,
                }
            )
    return rows


def _tuketim(items: list[Consumption]) -> str:
    parts: list[str] = []
    for item in items:
        suffix = ""
        if item.kind == "fee":
            suffix = " komisyon"
        elif item.synthetic:
            suffix = " hayali"
        parts.append(
            f"emir {item.order_no} {fmt_plain(item.qty, 8)} @ {fmt_plain(item.unit_cost, 8)}{suffix}"
        )
    return " | ".join(parts)


def _unique(items: list[str]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []
    for item in items:
        if item and item not in seen:
            seen.add(item)
            out.append(item)
    return out


def _markdown(analysis: Analysis) -> str:
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    lines = [
        "# Fiyatlio özet",
        "",
        f"- **Yöntem:** {analysis.method.upper()}",
        f"- **Üretim:** {now}",
        f"- **Dosyalar:** {', '.join(analysis.files) if analysis.files else '—'}",
        f"- **Sıfırlama eşiği:** zirvenin %{fmt_plain(analysis.flat_pct * 100, 4)} altı "
        f"ve {fmt_plain(analysis.flat_qty, 8)} adetten küçük",
        f"- **Eksik lot:** {'satış fiyatından hayali lot' if analysis.allow_missing_lots else 'hayali lot yok, eksi bakiye işaretlenir'}",
        "",
        "Hesap fill fill yapılır. Alış maliyeti ödenen quote / net alınan base "
        "(komisyon dahil). Quote'lar birbirine çevrilmez. Fiyat verilmezse açık kâr/zarar yazılmaz.",
        "",
    ]
    if analysis.warnings:
        lines.append("## Uyarılar")
        lines.append("")
        for text in _unique(analysis.warnings):
            lines.append(f"- {text}")
        lines.append("")

    if not analysis.ledgers:
        lines.append("İşlenecek spot işlem yok.")
        lines.append("")
    for ledger in analysis.ledgers:
        lines.extend(_md_ledger(ledger))
    if analysis.conversions:
        lines.append("## Çeviriler (PnL dışı)")
        lines.append("")
        lines.append("Stable-stable çiftler maliyet motoruna girmedi.")
        lines.append("")
        lines.append("| Zaman | Parite | Taraf | Miktar | Fiyat |")
        lines.append("| --- | --- | --- | --- | --- |")
        for item in analysis.conversions:
            lines.append(
                f"| {item.time.strftime('%Y-%m-%d %H:%M:%S')} | {item.pair} | {item.side} | "
                f"{fmt_tr(item.executed_qty, 8)} | {fmt_tr(item.price, 8)} |"
            )
        lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("Fiyatlio v2. Çevrimdışı hesap. Yatırım tavsiyesi değildir.")
    lines.append("")
    return "\n".join(lines)


def _md_ledger(ledger: LedgerResult) -> list[str]:
    closed = sum(1 for cycle in ledger.cycles if cycle.status == "KAPALI")
    opened = len(ledger.cycles) - closed
    lines = [
        f"## {ledger.base} / {ledger.quote}",
        "",
        f"{len(ledger.cycles)} dönem ({closed} kapalı, {opened} açık).",
        "",
    ]
    for note in _unique(ledger.notes + ledger.warnings):
        lines.append(f"> {note}")
        lines.append("")

    if ledger.position_open():
        lines.append("### Açık pozisyon")
        lines.append("")
        lines.append(f"- Eldeki miktar (dosya): **{fmt_tr(ledger.net_qty, 8)} {ledger.base}**")
        if ledger.wallet_qty is not None:
            lines.append(f"- Cüzdan: **{fmt_tr(ledger.wallet_qty, 8)} {ledger.base}**")
        lines.append(f"- Ağırlıklı maliyet: **{fmt_tr(ledger.avg_cost, 4)} {ledger.quote}**")
        lines.append(f"- Toplam maliyet: **{fmt_tr(ledger.total_cost, 4)} {ledger.quote}**")
        if ledger.price is None:
            lines.append("- Anlık fiyat verilmedi. Açık kâr/zarar hesaplanmadı.")
        else:
            lines.append(f"- Anlık fiyat: **{fmt_tr(ledger.price, 4)} {ledger.quote}**")
            lines.append(f"- Değer: **{fmt_tr(ledger.market_value(), 4)} {ledger.quote}**")
            lines.append(
                f"- Açık K/Z: **{fmt_signed_tr(ledger.open_pnl(), 4)} {ledger.quote}** "
                f"({fmt_pct(ledger.open_pnl_pct())})"
            )
        lines.append(f"- Gerçekleşen K/Z (tüm dönemler): **{fmt_signed_tr(ledger.realized, 4)} {ledger.quote}**")
        lines.append("")
        lines.append("### Kalan lotlar")
        lines.append("")
        if ledger.lots:
            lines.append("| Zaman | Emir | Miktar | Birim maliyet | Maliyet | Fill fiyatı |")
            lines.append("| --- | --- | --- | --- | --- | --- |")
            for lot in ledger.lots:
                lines.append(
                    f"| {lot.time.strftime('%Y-%m-%d %H:%M:%S')} | {lot.order_no} | "
                    f"{fmt_tr(lot.qty, 8)} | {fmt_tr(lot.unit_cost_quote, 4)} | "
                    f"{fmt_tr(lot.cost, 4)} | {fmt_tr(lot.fill_price, 4)} |"
                )
        else:
            lines.append("Açık miktar var ama maliyet lotu yok (eksik lot veya cüzdan farkı).")
        lines.append("")
    else:
        lines.append(f"Açık pozisyon yok. Gerçekleşen K/Z: **{fmt_signed_tr(ledger.realized, 4)} {ledger.quote}**.")
        lines.append("")

    lines.append("### Dönemler")
    lines.append("")
    if not ledger.cycles:
        lines.append("Dönem yok.")
        lines.append("")
        return lines
    lines.append(
        "| Dönem | Açılış | Kapanış | Alınan | Ödenen | Satılan | Gelen | "
        "Gerçekleşen K/Z | Kalan |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for cycle in ledger.cycles:
        lines.append(
            f"| {cycle.number} | {cycle.opened.strftime('%Y-%m-%d %H:%M:%S')} | {cycle.close_label} | "
            f"{fmt_tr(cycle.bought_qty, 8)} | {fmt_tr(cycle.bought_quote, 4)} | "
            f"{fmt_tr(cycle.sold_qty, 8)} | {fmt_tr(cycle.sold_quote, 4)} | "
            f"{fmt_signed_tr(cycle.realized, 4)} ({fmt_pct(cycle.realized_pct)}) | "
            f"{fmt_tr(cycle.remaining_qty, 8)} |"
        )
    lines.append("")
    for cycle in ledger.cycles:
        lines.append(f"#### Dönem {cycle.number} ({cycle.status})")
        lines.append("")
        lines.append(
            f"Ortalama alış {fmt_tr(cycle.avg_buy, 4)} {cycle.quote}, "
            f"ortalama satış {fmt_tr(cycle.avg_sell, 4)} {cycle.quote}. "
            f"Komisyon: {fmt_tr(cycle.fee_base, 8)} {cycle.asset} + "
            f"{fmt_tr(cycle.fee_quote, 4)} {cycle.quote}."
        )
        lines.append("")
        if cycle.events:
            for event in cycle.events:
                lines.append(f"- {event}")
        else:
            lines.append("- Olay yok.")
        lines.append("")
    return lines


def _html(analysis: Analysis) -> str:
    coins = []
    for ledger in analysis.ledgers:
        label = f"{ledger.base}/{ledger.quote}"
        if label not in coins:
            coins.append(label)
    options = ['<option value="tumu">Tümü</option>']
    for coin in coins:
        options.append(f'<option value="{html.escape(coin)}">{html.escape(coin)}</option>')
    body = [_html_summary(analysis), _html_warnings(analysis)]
    for ledger in analysis.ledgers:
        body.append(_html_ledger(ledger))
    if analysis.conversions:
        body.append(_html_conversions(analysis))
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    files = ", ".join(analysis.files) if analysis.files else "—"
    return f"""<!DOCTYPE html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fiyatlio rapor</title>
<style>
:root {{
  --bg: #0f1419;
  --card: #1a212b;
  --line: #2c3847;
  --text: #e7ecf3;
  --muted: #93a0b0;
  --gold: #e0b15a;
  --green: #3dd68c;
  --red: #ff6b6b;
  --chip: #243140;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0; background: var(--bg); color: var(--text);
  font: 15px/1.5 "Segoe UI", system-ui, sans-serif;
}}
header {{
  padding: 28px 32px 18px; border-bottom: 1px solid var(--line);
  display: flex; justify-content: space-between; gap: 24px; flex-wrap: wrap;
}}
h1 {{ margin: 0 0 6px; font-size: 28px; letter-spacing: 0.04em; }}
h1 span {{ color: var(--gold); }}
h2 {{ margin: 0 0 12px; font-size: 20px; }}
h3 {{ margin: 18px 0 8px; font-size: 16px; color: var(--gold); }}
.meta {{ color: var(--muted); margin: 0; }}
main {{ padding: 24px 32px 64px; max-width: 1280px; }}
.toolbar {{ display: flex; gap: 12px; align-items: center; }}
select {{
  background: var(--chip); color: var(--text); border: 1px solid var(--line);
  border-radius: 8px; padding: 8px 12px; font: inherit;
}}
.card {{
  background: var(--card); border: 1px solid var(--line); border-radius: 14px;
  padding: 18px 18px 8px; margin: 0 0 18px;
}}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 10px; }}
.stat {{ background: #121820; border-radius: 10px; padding: 10px 12px; }}
.stat .k {{ color: var(--muted); font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; }}
.stat .v {{ font-size: 18px; font-variant-numeric: tabular-nums; }}
.pos {{ color: var(--green); }}
.neg {{ color: var(--red); }}
table {{ width: 100%; border-collapse: collapse; margin: 8px 0 16px; font-variant-numeric: tabular-nums; }}
th, td {{ text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }}
th {{ color: var(--muted); font-weight: 600; font-size: 12px; text-transform: uppercase; }}
tr:hover td {{ background: rgba(224, 177, 90, 0.05); }}
.olay {{ color: var(--muted); font-size: 13px; margin: 0 0 4px; }}
.warn {{ color: #f0c674; }}
footer {{ color: var(--muted); font-size: 13px; padding: 8px 2px 24px; }}
.wrap {{ overflow-x: auto; }}
</style>
</head>
<body>
<header>
  <div>
    <h1>Fiyat<span>lio</span></h1>
    <p class="meta">Yöntem {html.escape(analysis.method.upper())} · {html.escape(now)} · {html.escape(files)}</p>
  </div>
  <div class="toolbar">
    <label for="coin">Varlık</label>
    <select id="coin" onchange="filtrele()">{''.join(options)}</select>
  </div>
</header>
<main>
{''.join(body)}
<footer>Fiyatlio v2. Çevrimdışı hesap. Yatırım tavsiyesi değildir. Fiyat verilmezse yalnız maliyet gösterilir.</footer>
</main>
<script>
function filtrele() {{
  const sec = document.getElementById("coin").value;
  document.querySelectorAll("[data-coin]").forEach((el) => {{
    el.hidden = sec !== "tumu" && el.dataset.coin !== sec;
  }});
}}
</script>
</body>
</html>
"""


def _pnl_class(value: Decimal | None) -> str:
    if value is None or value == 0:
        return ""
    return "pos" if value > 0 else "neg"


def _stat(label: str, value: str, css: str = "") -> str:
    return (
        f'<div class="stat"><div class="k">{html.escape(label)}</div>'
        f'<div class="v {css}">{value}</div></div>'
    )


def _html_summary(analysis: Analysis) -> str:
    rows = []
    for ledger in analysis.ledgers:
        coin = f"{ledger.base}/{ledger.quote}"
        pnl = ledger.open_pnl()
        rows.append(
            "<tr data-coin=\"{}\">".format(html.escape(coin))
            + "".join(
                [
                    f"<td>{html.escape(coin)}</td>",
                    f"<td>{html.escape(fmt_tr(ledger.net_qty, 8))}</td>",
                    f"<td>{html.escape(fmt_tr(ledger.wallet_qty, 8))}</td>",
                    f"<td>{html.escape(fmt_tr(ledger.avg_cost, 4))}</td>",
                    f"<td>{html.escape(fmt_tr(ledger.total_cost, 4))}</td>",
                    f"<td class=\"{_pnl_class(ledger.realized)}\">{html.escape(fmt_signed_tr(ledger.realized, 4))}</td>",
                    f"<td>{html.escape(fmt_tr(ledger.price, 4))}</td>",
                    f"<td class=\"{_pnl_class(pnl)}\">{html.escape(fmt_signed_tr(pnl, 4))}</td>",
                    f"<td>{html.escape(fmt_pct(ledger.open_pnl_pct()))}</td>",
                ]
            )
            + "</tr>"
        )
    table = _table(
        ["Varlık", "Dosya kalan", "Cüzdan", "Ağırlıklı maliyet", "Toplam maliyet",
         "Gerçekleşen", "Fiyat", "Açık K/Z", "Açık %"],
        rows,
    )
    return f'<section class="card"><h2>Varlık özeti</h2><div class="wrap">{table}</div></section>'


def _html_warnings(analysis: Analysis) -> str:
    items = _unique(analysis.warnings)
    for ledger in analysis.ledgers:
        items.extend(text for text in ledger.warnings if text not in items)
    if not items:
        return ""
    lis = "".join(f"<li class=\"warn\">{html.escape(text)}</li>" for text in items)
    return '<section class="card"><h2>Uyarılar</h2><ul>' + lis + "</ul></section>"


def _html_ledger(ledger: LedgerResult) -> str:
    coin = f"{ledger.base}/{ledger.quote}"
    stats = [
        _stat("Dosya kalan", f"{html.escape(fmt_tr(ledger.net_qty, 8))} {ledger.base}"),
        _stat("Ağırlıklı maliyet", f"{html.escape(fmt_tr(ledger.avg_cost, 4))} {ledger.quote}"),
        _stat("Toplam maliyet", f"{html.escape(fmt_tr(ledger.total_cost, 4))} {ledger.quote}"),
    ]
    if ledger.wallet_qty is not None:
        stats.insert(1, _stat("Cüzdan", f"{html.escape(fmt_tr(ledger.wallet_qty, 8))} {ledger.base}"))
    if ledger.price is None:
        stats.append(_stat("Açık K/Z", "Fiyat yok"))
    else:
        stats.append(_stat("Anlık fiyat", f"{html.escape(fmt_tr(ledger.price, 4))} {ledger.quote}"))
        stats.append(_stat("Değer", f"{html.escape(fmt_tr(ledger.market_value(), 4))} {ledger.quote}"))
        stats.append(
            _stat(
                "Açık K/Z",
                f"{html.escape(fmt_signed_tr(ledger.open_pnl(), 4))} {ledger.quote} "
                f"({html.escape(fmt_pct(ledger.open_pnl_pct()))})",
                _pnl_class(ledger.open_pnl()),
            )
        )
    lot_rows = []
    for lot in ledger.lots:
        lot_rows.append(
            "<tr>"
            f"<td>{lot.time.strftime('%Y-%m-%d %H:%M:%S')}</td>"
            f"<td>{html.escape(lot.order_no)}</td>"
            f"<td>{html.escape(fmt_tr(lot.qty, 8))}</td>"
            f"<td>{html.escape(fmt_tr(lot.unit_cost_quote, 4))}</td>"
            f"<td>{html.escape(fmt_tr(lot.cost, 4))}</td>"
            f"<td>{html.escape(fmt_tr(lot.fill_price, 4))}</td>"
            "</tr>"
        )
    lot_table = _table(
        ["Zaman", "Emir", "Miktar", "Birim maliyet", "Maliyet", "Fill fiyatı"],
        lot_rows,
    ) if lot_rows else "<p>Kalan lot yok.</p>"

    cycle_rows = []
    stories = []
    for cycle in ledger.cycles:
        cycle_rows.append(
            "<tr>"
            f"<td>{cycle.number}</td>"
            f"<td>{cycle.opened.strftime('%Y-%m-%d %H:%M:%S')}</td>"
            f"<td>{html.escape(cycle.close_label)}</td>"
            f"<td>{html.escape(fmt_tr(cycle.bought_qty, 8))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.bought_quote, 4))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.sold_qty, 8))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.sold_quote, 4))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.avg_buy, 4))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.avg_sell, 4))}</td>"
            f"<td class=\"{_pnl_class(cycle.realized)}\">{html.escape(fmt_signed_tr(cycle.realized, 4))} "
            f"({html.escape(fmt_pct(cycle.realized_pct))})</td>"
            f"<td>{html.escape(fmt_tr(cycle.fee_base, 8))} + {html.escape(fmt_tr(cycle.fee_quote, 4))}</td>"
            f"<td>{html.escape(fmt_tr(cycle.remaining_qty, 8))}</td>"
            "</tr>"
        )
        olay = "".join(f"<p class=\"olay\">{html.escape(event)}</p>" for event in cycle.events)
        stories.append(f"<h3>Dönem {cycle.number} · {html.escape(cycle.status)}</h3>{olay}")

    order_rows = []
    for order in ledger.orders:
        order_rows.append(
            "<tr>"
            f"<td>{order.time.strftime('%Y-%m-%d %H:%M:%S')}</td>"
            f"<td>{html.escape(order.order_no or '-')}</td>"
            f"<td>{html.escape(order.side)}</td>"
            f"<td>{order.fill_count}</td>"
            f"<td>{html.escape(fmt_tr(order.executed_qty, 8))}</td>"
            f"<td>{html.escape(fmt_tr(order.vwap, 4))}</td>"
            f"<td>{html.escape(fmt_tr(order.quote_qty, 4))}</td>"
            f"<td>{html.escape(order.event_type)}</td>"
            f"<td>{order.cycle_no or '—'}</td>"
            f"<td class=\"{_pnl_class(order.realized)}\">{html.escape(fmt_signed_tr(order.realized, 4))}</td>"
            "</tr>"
        )
    return f"""
<section class="card" data-coin="{html.escape(coin)}">
  <h2>{html.escape(coin)}</h2>
  <div class="grid">{''.join(stats)}</div>
  <h3>Kalan lotlar</h3>
  <div class="wrap">{lot_table}</div>
  <h3>Dönemler</h3>
  <div class="wrap">{_table(
      ["No", "Açılış", "Kapanış", "Alınan", "Ödenen", "Satılan", "Gelen",
       "Ort. alış", "Ort. satış", "Gerçekleşen", "Komisyon", "Kalan"],
      cycle_rows,
  )}</div>
  {''.join(stories)}
  <h3>Emirler</h3>
  <div class="wrap">{_table(
      ["Zaman", "Emir", "Taraf", "Fill", "Miktar", "VWAP", "Quote", "Etiket", "Dönem", "Gerçekleşen"],
      order_rows,
  )}</div>
</section>
"""


def _html_conversions(analysis: Analysis) -> str:
    rows = []
    for item in analysis.conversions:
        rows.append(
            "<tr data-coin=\"ceviri\">"
            f"<td>{item.time.strftime('%Y-%m-%d %H:%M:%S')}</td>"
            f"<td>{html.escape(item.pair)}</td>"
            f"<td>{html.escape(item.side)}</td>"
            f"<td>{html.escape(fmt_tr(item.executed_qty, 8))}</td>"
            f"<td>{html.escape(fmt_tr(item.amount_qty, 4))}</td>"
            "</tr>"
        )
    table = _table(["Zaman", "Parite", "Taraf", "Miktar", "Tutar"], rows)
    return f'<section class="card"><h2>Çeviriler (PnL dışı)</h2><div class="wrap">{table}</div></section>'


def _table(headers: list[str], rows: list[str]) -> str:
    head = "".join(f"<th>{html.escape(item)}</th>" for item in headers)
    body = "".join(rows) if rows else f"<tr><td colspan=\"{len(headers)}\">Kayıt yok.</td></tr>"
    return f"<table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>"
