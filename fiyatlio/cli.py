"""Komut satırı. Örnek:

python -m fiyatlio analyze islemler.csv --holdings PAXG=0.671466 --price PAXG=4279.78
"""

from __future__ import annotations

import argparse
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path

from rich.console import Console
from rich.table import Table
from rich.text import Text

from fiyatlio import __version__
from fiyatlio.cycles import DEFAULT_FLAT_PCT, DEFAULT_FLAT_QTY, Analysis, run_analysis
from fiyatlio.parse_binance import parse_paths
from fiyatlio.report import write_reports
from fiyatlio.util import fmt_pct, fmt_signed_tr, fmt_tr, parse_decimal


def _utf8_stdio() -> None:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is None:
            continue
        try:
            reconfigure(encoding="utf-8")
        except (OSError, ValueError):
            pass


def parse_assignments(raw: str | None, label: str) -> dict[str, Decimal]:
    """PAXG=0.671466,ETH=0.2 → sözlük. Boşsa boş sözlük."""
    found: dict[str, Decimal] = {}
    if not raw:
        return found
    for part in raw.split(","):
        piece = part.strip()
        if not piece:
            continue
        if "=" not in piece:
            raise SystemExit(f"{label} hatalı: '{piece}'. Beklenen VARLIK=SAYI")
        key, value = piece.split("=", 1)
        key = key.strip().upper()
        if not key:
            raise SystemExit(f"{label} hatalı: '{piece}'")
        try:
            found[key] = parse_decimal(value)
        except (InvalidOperation, ValueError):
            raise SystemExit(f"{label} sayısı okunamadı: '{piece}'") from None
    return found


def analyze_files(
    paths: list[str | Path],
    *,
    method: str = "fifo",
    holdings: dict[str, Decimal] | None = None,
    prices: dict[str, Decimal] | None = None,
    flat_pct: Decimal = DEFAULT_FLAT_PCT,
    flat_qty: Decimal = DEFAULT_FLAT_QTY,
    allow_missing_lots: bool = False,
) -> Analysis:
    parsed = parse_paths(list(paths))
    names = [Path(path).name for path in paths]
    return run_analysis(
        parsed.fills,
        parsed.conversions,
        parsed.warnings,
        method=method,
        holdings=holdings or {},
        prices=prices or {},
        flat_pct=flat_pct,
        flat_qty=flat_qty,
        allow_missing_lots=allow_missing_lots,
        source_names=names,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="fiyatlio",
        description="Binance spot işlem geçmişinden maliyet, çuval ve dönem analizi. Çevrimdışı.",
    )
    parser.add_argument("--version", action="version", version=f"fiyatlio {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    analyze = sub.add_parser("analyze", help="CSV işle, raporları yaz")
    analyze.add_argument("trades", nargs="+", help="Binance Spot Trade History CSV (bir veya daha fazla)")
    analyze.add_argument(
        "--holdings",
        default="",
        help="Cüzdandaki miktar. Örnek: PAXG=0.671466,ETH=0.2",
    )
    analyze.add_argument(
        "--price",
        default="",
        help="Anlık fiyat (quote cinsinden). Örnek: PAXG=4279.78. Verilmezse açık K/Z yok.",
    )
    analyze.add_argument(
        "--method",
        default="fifo",
        choices=["fifo", "lifo", "avg"],
        help="Maliyet yöntemi. Varsayılan: fifo",
    )
    analyze.add_argument("--out", default="out", help="Rapor klasörü. Varsayılan: out")
    analyze.add_argument("--flat-pct", default=str(DEFAULT_FLAT_PCT), help="Sıfırlama, zirve envanterin bu oranı. Varsayılan 0.005")
    analyze.add_argument("--flat-qty", default=str(DEFAULT_FLAT_QTY), help="Sıfırlama mutlak miktar eşiği. Varsayılan 0.0001")
    analyze.add_argument(
        "--allow-missing-lots",
        action="store_true",
        help="Envanter yetmezse satış fiyatından hayali lot aç. Varsayılan: açma, eksi bakiyeyi işaretle.",
    )
    return parser


def _print_summary(analysis: Analysis, out_dir: Path, console: Console) -> None:
    console.print(f"[bold]Fiyatlio[/bold] {analysis.method.upper()} · rapor: {out_dir}")
    if not analysis.ledgers:
        console.print("[yellow]Spot işlem yok. Çeviriler varsa raporda listelenir.[/yellow]")
    table = Table(title="Varlık özeti", show_lines=False)
    for column in ("Varlık", "Kalan", "Maliyet", "Ort. maliyet", "Gerçekleşen", "Açık K/Z"):
        table.add_column(column)
    for ledger in analysis.ledgers:
        pnl = ledger.open_pnl()
        pnl_text = Text(
            "—" if pnl is None else f"{fmt_signed_tr(pnl, 4)} {ledger.quote}",
            style="green" if pnl and pnl > 0 else ("red" if pnl and pnl < 0 else ""),
        )
        table.add_row(
            f"{ledger.base}/{ledger.quote}",
            fmt_tr(ledger.mark_qty(), 8),
            f"{fmt_tr(ledger.total_cost, 4)} {ledger.quote}",
            fmt_tr(ledger.avg_cost, 4),
            f"{fmt_signed_tr(ledger.realized, 4)} {ledger.quote}",
            pnl_text,
        )
    if analysis.ledgers:
        console.print(table)
    for ledger in analysis.ledgers:
        if not ledger.position_open() or not ledger.lots:
            continue
        lots = Table(title=f"{ledger.base} kalan lotlar")
        for column in ("Zaman", "Emir", "Miktar", "Birim maliyet", "Maliyet"):
            lots.add_column(column)
        for lot in ledger.lots:
            lots.add_row(
                lot.time.strftime("%Y-%m-%d %H:%M:%S"),
                lot.order_no,
                fmt_tr(lot.qty, 8),
                fmt_tr(lot.unit_cost_quote, 4),
                fmt_tr(lot.cost, 4),
            )
        console.print(lots)
        if ledger.price is not None and ledger.open_pnl_pct() is not None:
            console.print(
                f"{ledger.base} açık K/Z: {fmt_signed_tr(ledger.open_pnl(), 4)} {ledger.quote} "
                f"({fmt_pct(ledger.open_pnl_pct())})"
            )
    for ledger in analysis.ledgers:
        for note in ledger.notes:
            console.print(f"[dim]• {note}[/dim]")
    warnings = list(analysis.warnings)
    for ledger in analysis.ledgers:
        warnings.extend(ledger.warnings)
    shown = []
    for text in warnings:
        if text not in shown:
            shown.append(text)
    if shown:
        console.print(f"[yellow]Uyarı ({len(shown)})[/yellow]")
        for text in shown[:12]:
            console.print(f"  • {text}")
        if len(shown) > 12:
            console.print(f"  • … {len(shown) - 12} uyarı daha, rapor dosyasında.")


def main(argv: list[str] | None = None) -> int:
    _utf8_stdio()
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command != "analyze":
        parser.print_help()
        return 2
    try:
        flat_pct = parse_decimal(args.flat_pct)
        flat_qty = parse_decimal(args.flat_qty)
    except (InvalidOperation, ValueError):
        raise SystemExit("Eşik sayısı okunamadı (--flat-pct / --flat-qty).") from None
    if flat_pct < 0 or flat_qty < 0:
        raise SystemExit("Eşikler negatif olamaz.")

    holdings = parse_assignments(args.holdings, "Cüzdan")
    prices = parse_assignments(args.price, "Fiyat")
    analysis = analyze_files(
        args.trades,
        method=args.method,
        holdings=holdings,
        prices=prices,
        flat_pct=flat_pct,
        flat_qty=flat_qty,
        allow_missing_lots=args.allow_missing_lots,
    )
    console = Console()
    if not analysis.ledgers and not analysis.conversions:
        console.print("[red]İşlenecek satır yok.[/red]")
        for text in analysis.warnings:
            console.print(f"  • {text}")
        return 1
    out_dir = write_reports(analysis, args.out)
    _print_summary(analysis, out_dir, console)
    console.print(
        "Dosyalar: ozet.md, donemler.csv, filller.csv, kalan_lotlar.csv, "
        "varlik_ozeti.csv, rapor.html, rapor.xlsx"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
