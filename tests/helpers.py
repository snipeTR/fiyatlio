"""Test fill fabrikası. Komisyon verilmezse alışta base, satışta quote varsayılmaz; fee 0 kalır."""

from __future__ import annotations

from datetime import datetime
from decimal import Decimal

from fiyatlio.cycles import DEFAULT_FLAT_PCT, DEFAULT_FLAT_QTY, LedgerResult, analyze_ledger
from fiyatlio.parse_binance import Fill, split_pair

_ROW = 0


def F(
    when: str,
    pair: str,
    side: str,
    price: str,
    executed: str,
    amount: str,
    fee: str = "0",
    fee_asset: str | None = None,
    order: str = "1",
) -> Fill:
    global _ROW
    _ROW += 1
    base, quote = split_pair(pair)
    fee_qty = Decimal(fee)
    asset = fee_asset
    if fee_qty == 0:
        asset = None
    elif asset is None:
        asset = base if side == "BUY" else quote
    return Fill(
        order_no=str(order),
        time=datetime.strptime(when, "%Y-%m-%d %H:%M:%S"),
        pair=pair,
        side=side,
        price=Decimal(price),
        executed_qty=Decimal(executed),
        executed_asset=base,
        amount_qty=Decimal(amount),
        amount_asset=quote,
        fee_qty=fee_qty,
        fee_asset=asset,
        aor_pair=None,
        aor_rate=None,
        source_file="test.csv",
        source_row=_ROW,
        base_asset=base,
        quote_asset=quote,
        is_conversion=base in {"USDT", "USDC", "FDUSD"} and quote in {"USDT", "USDC", "FDUSD"},
    )


def ledger(fills: list[Fill], method: str = "fifo", allow: bool = False) -> LedgerResult:
    return analyze_ledger(
        fills,
        method=method,
        flat_pct=DEFAULT_FLAT_PCT,
        flat_qty=DEFAULT_FLAT_QTY,
        allow_missing_lots=allow,
    )


def events(result: LedgerResult) -> list[str]:
    return [view.event_type for view in result.fills]
