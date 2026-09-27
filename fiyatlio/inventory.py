"""Lot kuyruğu: FIFO, LIFO ve hareketli ortalama.

Alış birim maliyeti = ödenen quote / net alınan base (komisyon dahil).
Satış realized = (net birim gelir - lot birim maliyeti) × tüketilen miktar.
Net birim gelir = (Amount - quote komisyonu) / Executed.

Short yok. Envanter yetmezse uyarı basılır; hayali lot yalnızca
allow_missing_lots ile, satış fiyatından açılır.
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
from decimal import Decimal

from fiyatlio.parse_binance import Fill
from fiyatlio.util import DUST, fmt_plain, snap

METHODS = ("fifo", "lifo", "avg")


@dataclass
class Lot:
    """Maliyet `cost` alanında durur. Birim fiyat cost/qty'dir; çarpım artığı bırakmaz."""

    qty: Decimal
    cost: Decimal
    fill_price: Decimal
    time: datetime
    order_no: str
    pair: str
    quote_asset: str
    base_asset: str
    synthetic: bool = False

    @property
    def unit_cost_quote(self) -> Decimal:
        if self.qty == 0:
            return Decimal("0")
        return self.cost / self.qty


@dataclass(frozen=True)
class Consumption:
    order_no: str
    time: datetime
    qty: Decimal
    unit_cost: Decimal
    cost: Decimal
    realized: Decimal
    kind: str  # sale | fee | synthetic
    synthetic: bool = False


@dataclass
class ApplyResult:
    qty_before: Decimal
    qty_after: Decimal
    added_qty: Decimal = Decimal("0")
    added_quote: Decimal = Decimal("0")
    sale_qty: Decimal = Decimal("0")
    sale_proceeds: Decimal = Decimal("0")
    sale_cost: Decimal = Decimal("0")
    realized: Decimal = Decimal("0")
    missing_qty: Decimal = Decimal("0")
    consumptions: list[Consumption] = field(default_factory=list)
    fee_base: Decimal = Decimal("0")
    fee_quote: Decimal = Decimal("0")
    fee_other: Decimal = Decimal("0")
    fee_other_asset: str | None = None
    unit_cost: Decimal | None = None
    unit_proceeds: Decimal | None = None
    warning: str | None = None
    # Satış, eldeki lottan fazlaysa (hayali lot açılsa bile) True.
    shortage: bool = False


class InventoryBook:
    def __init__(self, method: str = "fifo", allow_missing_lots: bool = False):
        if method not in METHODS:
            raise ValueError(f"yöntem fifo, lifo veya avg olmalı: {method}")
        self.method = method
        self.allow_missing_lots = allow_missing_lots
        self.lots: deque[Lot] = deque()
        # Pozitifse, maliyeti bilinmeyen satış fazlası (eksi bakiye).
        self.deficit = Decimal("0")

    @property
    def qty(self) -> Decimal:
        held = sum((lot.qty for lot in self.lots), Decimal("0"))
        return snap(held - self.deficit)

    def total_cost(self) -> Decimal:
        return sum((lot.cost for lot in self.lots), Decimal("0"))

    def apply(self, fill: Fill) -> ApplyResult:
        if fill.side == "BUY":
            return self._buy(fill)
        if fill.side == "SELL":
            return self._sell(fill)
        raise ValueError(f"bilinmeyen taraf: {fill.side}")

    def write_off_dust(self) -> Decimal:
        """Dönem sıfırlanırken eşiğin altında kalan lotun maliyetini düş."""
        cost = self.total_cost()
        self.lots.clear()
        return cost

    def _fee_buckets(self, fill: Fill) -> tuple[Decimal, Decimal, Decimal, str | None]:
        if fill.fee_qty <= 0 or not fill.fee_asset:
            return Decimal("0"), Decimal("0"), Decimal("0"), None
        if fill.fee_asset == fill.base_asset:
            return fill.fee_qty, Decimal("0"), Decimal("0"), None
        if fill.fee_asset == fill.quote_asset:
            return Decimal("0"), fill.fee_qty, Decimal("0"), None
        return Decimal("0"), Decimal("0"), fill.fee_qty, fill.fee_asset

    def _buy(self, fill: Fill) -> ApplyResult:
        qty_before = self.qty
        fee_base, fee_quote, fee_other, other_asset = self._fee_buckets(fill)
        net_base = snap(fill.executed_qty - fee_base)
        quote_spent = fill.amount_qty + fee_quote
        warning = None
        if net_base <= 0:
            warning = (
                f"Alış net miktarı sıfır veya negatif ({fill.pair} emir {fill.order_no}). "
                "Komisyon executed kadar veya daha büyük."
            )
            net_base = Decimal("0")

        unit_cost = (quote_spent / net_base) if net_base > 0 else None
        added_qty = net_base
        added_quote = quote_spent

        # Eksi bakiyeyi kapatan kısım yeni lot olmaz; maliyeti çuvala yazılmaz.
        if self.deficit > 0 and net_base > 0 and unit_cost is not None:
            cover = self.deficit if self.deficit <= net_base else net_base
            self.deficit = snap(self.deficit - cover)
            added_qty = snap(net_base - cover)
            added_quote = unit_cost * added_qty
            if cover > 0:
                warning = (
                    f"Eksik bakiye kapatıldı: {fmt_plain(cover, 8)} {fill.base_asset} "
                    f"(emir {fill.order_no}). Bu kısım çuvala lot olarak girmedi."
                )

        if added_qty > 0 and unit_cost is not None:
            fresh = Lot(
                qty=added_qty,
                cost=added_quote,
                fill_price=fill.price,
                time=fill.time,
                order_no=fill.order_no or f"satır-{fill.source_row}",
                pair=fill.pair,
                quote_asset=fill.quote_asset,
                base_asset=fill.base_asset,
            )
            self._add_lot(fresh)

        if fee_other > 0 and warning is None:
            warning = (
                f"Komisyon {fmt_plain(fee_other, 8)} {other_asset} maliyete katılmadı "
                f"(emir {fill.order_no}). Tarihsel {other_asset} fiyatı yok."
            )

        return ApplyResult(
            qty_before=qty_before,
            qty_after=self.qty,
            added_qty=added_qty,
            added_quote=added_quote,
            fee_base=fee_base,
            fee_quote=fee_quote,
            fee_other=fee_other,
            fee_other_asset=other_asset,
            unit_cost=unit_cost,
            warning=warning,
        )

    def _sell(self, fill: Fill) -> ApplyResult:
        qty_before = self.qty
        fee_base, fee_quote, fee_other, other_asset = self._fee_buckets(fill)
        net_proceeds = fill.amount_qty - fee_quote
        if net_proceeds < 0:
            net_proceeds = Decimal("0")
        unit_proceeds = net_proceeds / fill.executed_qty if fill.executed_qty > 0 else Decimal("0")

        sale_cons, sale_missing = self._consume(fill.executed_qty, unit_proceeds, kind="sale")
        fee_cons: list[Consumption] = []
        fee_missing = Decimal("0")
        if fee_base > 0:
            # Base komisyon miktarı düşürür, quote getirmez.
            fee_cons, fee_missing = self._consume(fee_base, Decimal("0"), kind="fee")

        raw_missing = snap(sale_missing + fee_missing)
        consumptions = list(sale_cons) + list(fee_cons)
        warning_parts: list[str] = []

        if raw_missing > 0:
            if self.allow_missing_lots:
                # Spec: hayali lot satış fiyatından açılır. Komisyon payının geliri 0'dır.
                if sale_missing > 0:
                    phantom_cost = fill.price * sale_missing
                    consumptions.append(
                        Consumption(
                            order_no="HAYALI",
                            time=fill.time,
                            qty=sale_missing,
                            unit_cost=fill.price,
                            cost=phantom_cost,
                            realized=unit_proceeds * sale_missing - phantom_cost,
                            kind="synthetic",
                            synthetic=True,
                        )
                    )
                if fee_missing > 0:
                    phantom_cost = fill.price * fee_missing
                    consumptions.append(
                        Consumption(
                            order_no="HAYALI",
                            time=fill.time,
                            qty=fee_missing,
                            unit_cost=fill.price,
                            cost=phantom_cost,
                            realized=-phantom_cost,
                            kind="synthetic",
                            synthetic=True,
                        )
                    )
                warning_parts.append(
                    f"Eksik lot: {fmt_plain(raw_missing, 8)} {fill.base_asset} satış fiyatından "
                    f"({fmt_plain(fill.price, 8)} {fill.quote_asset}) hayali lotla kapatıldı "
                    f"(emir {fill.order_no})."
                )
            else:
                self.deficit = snap(self.deficit + raw_missing)
                warning_parts.append(
                    f"Eksik lot: {fmt_plain(raw_missing, 8)} {fill.base_asset} için elde lot yoktu. "
                    f"Hayali lot açılmadı, bakiye eksi işaretlendi (emir {fill.order_no})."
                )

        if fee_other > 0:
            warning_parts.append(
                f"Satış komisyonu {fmt_plain(fee_other, 8)} {other_asset} gelire yansıtılmadı "
                f"(emir {fill.order_no})."
            )

        realized = sum((c.realized for c in consumptions), Decimal("0"))
        sale_qty = sum((c.qty for c in consumptions if c.kind == "sale"), Decimal("0"))
        # Hayali lot satış kısmını da "eşleşmiş" sayar; eksi bakiyede sayılmaz.
        if self.allow_missing_lots and sale_missing > 0:
            sale_qty = fill.executed_qty
        sale_cost = sum((c.cost for c in consumptions), Decimal("0"))
        sale_proceeds = unit_proceeds * sale_qty

        return ApplyResult(
            qty_before=qty_before,
            qty_after=self.qty,
            sale_qty=sale_qty,
            sale_proceeds=sale_proceeds,
            sale_cost=sale_cost,
            realized=realized,
            missing_qty=Decimal("0") if self.allow_missing_lots else raw_missing,
            consumptions=consumptions,
            fee_base=fee_base,
            fee_quote=fee_quote,
            fee_other=fee_other,
            fee_other_asset=other_asset,
            unit_proceeds=unit_proceeds,
            warning=" ".join(warning_parts) or None,
            shortage=raw_missing > 0 or qty_before <= DUST,
        )

    def _add_lot(self, lot: Lot) -> None:
        if self.method == "avg" and self.lots:
            current = self.lots[0]
            total_qty = current.qty + lot.qty
            current.fill_price = (
                current.qty * current.fill_price + lot.qty * lot.fill_price
            ) / total_qty
            current.cost += lot.cost
            current.qty = total_qty
            # Ortalamada açılış zamanı ve emir ilk lotta kalır.
            return
        self.lots.append(lot)

    def _consume(
        self, qty: Decimal, unit_proceeds: Decimal, kind: str
    ) -> tuple[list[Consumption], Decimal]:
        left = qty
        done: list[Consumption] = []
        while left > DUST and self.lots:
            lot = self.lots[-1] if self.method == "lifo" else self.lots[0]
            finish = lot.qty <= left + DUST
            take = lot.qty if finish else left
            # Lot bitiyorsa kalan maliyeti olduğu gibi al; kısmi tüketimde orantıla.
            removed = lot.cost if finish else (lot.cost * take / lot.qty)
            unit = removed / take if take > 0 else Decimal("0")
            done.append(
                Consumption(
                    order_no=lot.order_no,
                    time=lot.time,
                    qty=take,
                    unit_cost=unit,
                    cost=removed,
                    realized=unit_proceeds * take - removed,
                    kind=kind,
                    synthetic=lot.synthetic,
                )
            )
            if finish:
                if self.method == "lifo":
                    self.lots.pop()
                else:
                    self.lots.popleft()
            else:
                lot.qty = snap(lot.qty - take)
                lot.cost -= removed
            left = snap(left - take)
        return done, left
