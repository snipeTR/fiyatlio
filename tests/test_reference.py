"""PAXG referans senaryosu. Sapma ±0.001 PAXG veya ±1 USDT ise başarısız."""

from __future__ import annotations

from datetime import datetime
from decimal import Decimal
from pathlib import Path

from fiyatlio.cli import analyze_files

ROOT = Path(__file__).resolve().parents[1]
SAMPLE = ROOT / "samples" / "paxg_referans.csv"
ORNEK = ROOT / "samples" / "paxg_ornek.csv"
QTY_TOL = Decimal("0.001")
USDT_TOL = Decimal("1")


def _close(actual: Decimal, expected: Decimal, tol: Decimal, label: str) -> None:
    gap = abs(actual - expected)
    assert gap <= tol, f"{label}: {actual} beklenen {expected} ± {tol} (fark {gap})"


def test_paxg_fifo_referans():
    analysis = analyze_files(
        [SAMPLE],
        method="fifo",
        holdings={"PAXG": Decimal("0.671466")},
        prices={"PAXG": Decimal("4279.78")},
    )
    assert len(analysis.ledgers) == 1
    ledger = analysis.ledgers[0]
    assert ledger.base == "PAXG"
    assert ledger.quote == "USDT"

    _close(ledger.net_qty, Decimal("0.671517"), QTY_TOL, "dosya net")
    _close(ledger.total_cost, Decimal("3341.7"), USDT_TOL, "toplam maliyet")
    assert ledger.avg_cost is not None
    _close(ledger.avg_cost, Decimal("4976.7"), USDT_TOL, "ağırlıklı maliyet")
    assert ledger.open_pnl() is not None
    _close(ledger.open_pnl(), Decimal("-468"), USDT_TOL, "açık zarar")
    value = ledger.market_value()
    assert value is not None
    _close(value, Decimal("2873.73"), Decimal("0.01"), "cüzdan değeri")
    pct = ledger.open_pnl_pct()
    assert pct is not None and Decimal("-15") < pct < Decimal("-13")

    def bucket(unit: Decimal) -> Decimal:
        return sum(
            (lot.qty for lot in ledger.lots if abs(lot.unit_cost_quote - unit) <= USDT_TOL),
            Decimal("0"),
        )

    _close(bucket(Decimal("5150.14")), Decimal("0.1929"), QTY_TOL, "3 Mart artığı")
    _close(bucket(Decimal("5110.80")), Decimal("0.3617"), QTY_TOL, "9 Mart lotu")
    _close(bucket(Decimal("4275.38")), Decimal("0.1169"), QTY_TOL, "23 Mart lotu")
    assert all(lot.time >= datetime(2026, 3, 1) for lot in ledger.lots)
    assert len(ledger.lots) == 4  # 23 Mart aynı emir, iki fill, iki lot

    orders = {order.order_no: order for order in ledger.orders}
    assert orders["2001"].event_type == "YENIDEN_ALIM"
    assert orders["2002"].event_type == "EK_ALIM"
    assert orders["2003"].event_type == "KISMI_SATIS"
    assert orders["650006707"].event_type == "EK_ALIM"
    assert orders["650006707"].fill_count == 2
    assert orders["650006707"].executed_qty == Decimal("0.117")

    assert len(ledger.cycles) == 4
    open_cycles = [cycle for cycle in ledger.cycles if cycle.status == "AÇIK"]
    assert len(open_cycles) == 1
    assert open_cycles[0].remaining_qty == ledger.net_qty
    assert all(cycle.remaining_qty == 0 for cycle in ledger.cycles if cycle.status == "KAPALI")
    # Küçük cüzdan farkı toz notu; büyük uyarı değil.
    assert not any("farklı (fark" in text for text in ledger.warnings)


def test_paxg_ornek_anonim():
    """Anonim örnek: emir numaraları değişmiş, kesit gerçek fill matematiğini korur."""
    analysis = analyze_files(
        [ORNEK],
        method="fifo",
        holdings={"PAXG": Decimal("0.671466")},
        prices={"PAXG": Decimal("4279.78")},
    )
    ledger = analysis.ledgers[0]
    _close(ledger.net_qty, Decimal("0.671517"), QTY_TOL, "dosya net")
    _close(ledger.total_cost, Decimal("3341.7"), USDT_TOL, "toplam maliyet")
    assert ledger.avg_cost is not None
    _close(ledger.avg_cost, Decimal("4976.7"), USDT_TOL, "ağırlıklı maliyet")
    assert ledger.open_pnl() is not None
    _close(ledger.open_pnl(), Decimal("-468"), USDT_TOL, "açık zarar")

    def bucket(unit: Decimal) -> Decimal:
        return sum(
            (lot.qty for lot in ledger.lots if abs(lot.unit_cost_quote - unit) <= USDT_TOL),
            Decimal("0"),
        )

    _close(bucket(Decimal("5150.14")), Decimal("0.1929"), QTY_TOL, "3 Mart artığı")
    _close(bucket(Decimal("5110.80")), Decimal("0.3617"), QTY_TOL, "9 Mart")
    _close(bucket(Decimal("4275.38")), Decimal("0.1169"), QTY_TOL, "23 Mart")

    orders = {order.order_no: order for order in ledger.orders}
    assert orders["100003"].event_type == "YENIDEN_ALIM"
    assert orders["100003"].executed_qty == Decimal("0.748")
    assert orders["100004"].event_type == "EK_ALIM"
    assert orders["100004"].executed_qty == Decimal("0.3621")
    assert orders["100005"].event_type == "KISMI_SATIS"
    assert orders["100005"].executed_qty == Decimal("0.5545")
    assert orders["100006"].event_type == "EK_ALIM"
    assert orders["100006"].executed_qty == Decimal("0.117")
    assert ledger.cycles[-1].status == "AÇIK"
    assert all(lot.time >= datetime(2026, 3, 1) for lot in ledger.lots)
