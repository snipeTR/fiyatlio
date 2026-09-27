"""Dönem (cycle) ve fill etiketleri.

Bir dönem, uzun bakiye ~0 iken başlar ve tekrar ~0 olunca kapanır.
Sıfırlama: kalan < max_envanter × flat_pct VE kalan < flat_qty.

Fill'ler zaman sırasında, teker teker işlenir. Aynı Order No tek emirdir;
etiket emrin son haline göre tekilleştirilir (ikinci fill EK_SATIS olmaz).

Satış etiketi önceliği: FAZLA_SATIS > SIFIRLAMA > EK_SATIS > KISMI_SATIS.
Alış etiketi önceliği: ILK_ALIM > YENIDEN_ALIM > EK_ALIM.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from decimal import Decimal

from fiyatlio.inventory import ApplyResult, Consumption, InventoryBook, Lot
from fiyatlio.parse_binance import Fill, fill_sort_key
from fiyatlio.util import DUST, fmt_plain, snap

DEFAULT_FLAT_PCT = Decimal("0.005")
DEFAULT_FLAT_QTY = Decimal("0.0001")

_SELL_PRIORITY = ("FAZLA_SATIS", "SIFIRLAMA", "EK_SATIS", "KISMI_SATIS")
_BUY_PRIORITY = ("ILK_ALIM", "YENIDEN_ALIM", "EK_ALIM")


@dataclass
class Cycle:
    asset: str
    quote: str
    pair: str
    number: int
    opened: datetime
    closed: datetime | None = None
    status: str = "AÇIK"
    bought_qty: Decimal = Decimal("0")
    bought_quote: Decimal = Decimal("0")
    sold_qty: Decimal = Decimal("0")
    sold_quote: Decimal = Decimal("0")
    sold_cost: Decimal = Decimal("0")
    realized: Decimal = Decimal("0")
    fee_base: Decimal = Decimal("0")
    fee_quote: Decimal = Decimal("0")
    fee_other: Decimal = Decimal("0")
    fee_other_asset: str | None = None
    remaining_qty: Decimal = Decimal("0")
    remaining_cost: Decimal = Decimal("0")
    events: list[str] = field(default_factory=list)
    dust_cost: Decimal = Decimal("0")

    @property
    def avg_buy(self) -> Decimal | None:
        if self.bought_qty <= 0:
            return None
        return self.bought_quote / self.bought_qty

    @property
    def avg_sell(self) -> Decimal | None:
        if self.sold_qty <= 0:
            return None
        return self.sold_quote / self.sold_qty

    @property
    def realized_pct(self) -> Decimal | None:
        if self.sold_cost <= 0:
            return None
        return self.realized / self.sold_cost * Decimal("100")

    @property
    def remaining_avg(self) -> Decimal | None:
        if self.remaining_qty <= 0:
            return None
        return self.remaining_cost / self.remaining_qty

    @property
    def close_label(self) -> str:
        if self.closed is None:
            return "AÇIK"
        return self.closed.strftime("%Y-%m-%d %H:%M:%S")


@dataclass
class OrderView:
    order_no: str
    time: datetime
    pair: str
    base: str
    quote: str
    side: str
    executed_qty: Decimal
    quote_qty: Decimal
    vwap: Decimal
    fee_base: Decimal
    fee_quote: Decimal
    fee_other: Decimal
    fee_other_asset: str | None
    event_type: str
    cycle_no: int | None
    realized: Decimal
    fill_count: int


@dataclass
class FillView:
    fill: Fill
    event_type: str
    cycle_no: int | None
    result: ApplyResult

    @property
    def consumptions(self) -> list[Consumption]:
        return self.result.consumptions


@dataclass
class LedgerResult:
    base: str
    quote: str
    pair: str
    cycles: list[Cycle]
    orders: list[OrderView]
    fills: list[FillView]
    lots: list[Lot]
    deficit: Decimal = Decimal("0")
    warnings: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)
    wallet_qty: Decimal | None = None
    price: Decimal | None = None

    @property
    def net_qty(self) -> Decimal:
        held = sum((lot.qty for lot in self.lots), Decimal("0"))
        return snap(held - self.deficit)

    @property
    def total_cost(self) -> Decimal:
        return sum((lot.cost for lot in self.lots), Decimal("0"))

    @property
    def realized(self) -> Decimal:
        return sum((cycle.realized for cycle in self.cycles), Decimal("0"))

    @property
    def avg_cost(self) -> Decimal | None:
        if self.net_qty <= 0:
            return None
        return self.total_cost / self.net_qty

    def mark_qty(self) -> Decimal:
        if self.wallet_qty is not None:
            return self.wallet_qty
        return self.net_qty

    def position_open(self) -> bool:
        return self.mark_qty() > DUST or self.total_cost > 0 or self.deficit > 0

    def market_value(self) -> Decimal | None:
        if self.price is None or not self.position_open():
            return None
        return self.mark_qty() * self.price

    def open_pnl(self) -> Decimal | None:
        value = self.market_value()
        if value is None:
            return None
        return value - self.total_cost

    def open_pnl_pct(self) -> Decimal | None:
        pnl = self.open_pnl()
        if pnl is None or self.total_cost <= 0:
            return None
        return pnl / self.total_cost * Decimal("100")


@dataclass
class Analysis:
    method: str
    files: list[str]
    ledgers: list[LedgerResult]
    conversions: list[Fill]
    warnings: list[str]
    holdings: dict[str, Decimal]
    prices: dict[str, Decimal]
    flat_pct: Decimal
    flat_qty: Decimal
    allow_missing_lots: bool


def is_flat(qty: Decimal, max_qty: Decimal, flat_pct: Decimal, flat_qty: Decimal) -> bool:
    """Kalan, dönemin zirvesinin flat_pct altı ve flat_qty altıysa sıfır sayılır."""
    if qty < 0:
        return False
    if qty <= DUST:
        return True
    return qty < (max_qty * flat_pct) and qty < flat_qty


def _order_key(fill: Fill) -> str:
    if fill.order_no:
        return f"{fill.pair}|{fill.order_no}"
    return f"{fill.source_file}|{fill.source_row}"


def _accumulate(cycle: Cycle | None, result: ApplyResult) -> None:
    if cycle is None:
        return
    cycle.bought_qty += result.added_qty
    cycle.bought_quote += result.added_quote
    cycle.sold_qty += result.sale_qty
    cycle.sold_quote += result.sale_proceeds
    cycle.sold_cost += result.sale_cost
    cycle.realized += result.realized
    cycle.fee_base += result.fee_base
    cycle.fee_quote += result.fee_quote
    cycle.fee_other += result.fee_other
    if result.fee_other_asset:
        cycle.fee_other_asset = result.fee_other_asset


def _unify(events: list[str], side: str) -> str:
    order = _SELL_PRIORITY if side == "SELL" else _BUY_PRIORITY
    for name in order:
        if name in events:
            return name
    return events[-1]


def analyze_ledger(
    fills: list[Fill],
    *,
    method: str,
    flat_pct: Decimal,
    flat_qty: Decimal,
    allow_missing_lots: bool,
) -> LedgerResult:
    if not fills:
        raise ValueError("boş defter")
    fills = sorted(fills, key=fill_sort_key)
    base = fills[0].base_asset
    quote = fills[0].quote_asset
    pair = fills[0].pair
    book = InventoryBook(method, allow_missing_lots)
    cycles: list[Cycle] = []
    views: list[FillView] = []
    warnings: list[str] = []

    cycle: Cycle | None = None
    ever_closed = False
    max_qty = Decimal("0")
    # Bu dönemde görülen satış emirleri. Aynı emrin ikinci fill'i yeni satış değildir.
    sell_orders: list[str] = []

    for fill in fills:
        qty_before = book.qty
        result = book.apply(fill)
        qty_after = book.qty
        if result.warning:
            warnings.append(result.warning)
        key = _order_key(fill)

        if fill.side == "BUY":
            if cycle is None and qty_before <= DUST and qty_after > DUST:
                event = "YENIDEN_ALIM" if ever_closed else "ILK_ALIM"
                cycle = Cycle(
                    asset=base,
                    quote=quote,
                    pair=pair,
                    number=len(cycles) + 1,
                    opened=fill.time,
                )
                cycles.append(cycle)
                max_qty = qty_after
                sell_orders = []
            else:
                event = "EK_ALIM"
                if cycle is not None and qty_after > max_qty:
                    max_qty = qty_after
            cycle_no = cycle.number if cycle is not None else None
            _accumulate(cycle, result)
        else:
            new_sell_order = key not in sell_orders
            if result.shortage or qty_before <= DUST:
                event = "FAZLA_SATIS"
            elif cycle is not None and is_flat(qty_after, max_qty, flat_pct, flat_qty):
                event = "SIFIRLAMA"
            elif cycle is not None and sell_orders and new_sell_order:
                event = "EK_SATIS"
            elif cycle is not None and not new_sell_order:
                event = "KISMI_SATIS"
            else:
                event = "KISMI_SATIS"

            if cycle is not None and new_sell_order:
                sell_orders.append(key)

            cycle_no = cycle.number if cycle is not None else None
            _accumulate(cycle, result)

            if event == "SIFIRLAMA" and cycle is not None:
                leftover = book.qty
                if leftover > 0:
                    abandoned = book.write_off_dust()
                    cycle.dust_cost += abandoned
                    cycle.sold_cost += abandoned
                    cycle.realized -= abandoned
                    if abandoned > 0:
                        warnings.append(
                            f"Toz sıfırlandı: {fmt_plain(leftover, 8)} {base} "
                            f"({fmt_plain(abandoned, 8)} {quote}) dönem kapanırken silindi."
                        )
                cycle.closed = fill.time
                cycle.status = "KAPALI"
                cycle.remaining_qty = Decimal("0")
                cycle.remaining_cost = Decimal("0")
                ever_closed = True
                cycle = None
                max_qty = Decimal("0")
                sell_orders = []

        views.append(FillView(fill=fill, event_type=event, cycle_no=cycle_no, result=result))

    orders = _build_orders(views, cycles)
    if cycle is not None:
        cycle.remaining_qty = book.qty
        cycle.remaining_cost = book.total_cost()

    if book.deficit > 0:
        warnings.append(
            f"Eksi bakiye: {base} defteri -{fmt_plain(book.deficit, 8)} {base}. "
            "Maliyetsiz satış fazlası çuvala lot olarak yazılmadı."
        )

    return LedgerResult(
        base=base,
        quote=quote,
        pair=pair,
        cycles=cycles,
        orders=orders,
        fills=views,
        lots=list(book.lots),
        deficit=book.deficit,
        warnings=warnings,
    )


def _build_orders(views: list[FillView], cycles: list[Cycle]) -> list[OrderView]:
    """Aynı emrin fill'lerini birleştir, tek etiket bas, dönem hikâyesine bir satır yaz."""
    groups: list[list[FillView]] = []
    index: dict[str, list[FillView]] = {}
    for view in views:
        key = _order_key(view.fill)
        bucket = index.get(key)
        if bucket is None:
            bucket = []
            index[key] = bucket
            groups.append(bucket)
        bucket.append(view)

    by_number = {item.number: item for item in cycles}
    orders: list[OrderView] = []
    for group in groups:
        side = group[0].fill.side
        final = _unify([item.event_type for item in group], side)
        for item in group:
            item.event_type = final
        head = group[0].fill
        executed = sum((item.fill.executed_qty for item in group), Decimal("0"))
        quote_qty = sum((item.fill.amount_qty for item in group), Decimal("0"))
        cycle_no = next((item.cycle_no for item in group if item.cycle_no is not None), None)
        order = OrderView(
            order_no=head.order_no,
            time=head.time,
            pair=head.pair,
            base=head.base_asset,
            quote=head.quote_asset,
            side=side,
            executed_qty=executed,
            quote_qty=quote_qty,
            vwap=(quote_qty / executed) if executed > 0 else Decimal("0"),
            fee_base=sum((item.result.fee_base for item in group), Decimal("0")),
            fee_quote=sum((item.result.fee_quote for item in group), Decimal("0")),
            fee_other=sum((item.result.fee_other for item in group), Decimal("0")),
            fee_other_asset=next(
                (item.result.fee_other_asset for item in group if item.result.fee_other_asset),
                None,
            ),
            event_type=final,
            cycle_no=cycle_no,
            realized=sum((item.result.realized for item in group), Decimal("0")),
            fill_count=len(group),
        )
        orders.append(order)
        target = by_number.get(cycle_no) if cycle_no is not None else None
        if target is not None:
            target.events.append(
                f"{order.time.strftime('%Y-%m-%d %H:%M:%S')} {final} "
                f"{side} {fmt_plain(executed, 8)} @ {fmt_plain(order.vwap, 8)} "
                f"emir {order.order_no or '-'}"
            )
    return orders


def run_analysis(
    fills: list[Fill],
    conversions: list[Fill],
    parse_warnings: list[str],
    *,
    method: str,
    holdings: dict[str, Decimal],
    prices: dict[str, Decimal],
    flat_pct: Decimal,
    flat_qty: Decimal,
    allow_missing_lots: bool,
    source_names: list[str],
) -> Analysis:
    grouped: dict[tuple[str, str], list[Fill]] = {}
    for fill in fills:
        grouped.setdefault((fill.base_asset, fill.quote_asset), []).append(fill)

    ledgers = [
        analyze_ledger(
            bucket,
            method=method,
            flat_pct=flat_pct,
            flat_qty=flat_qty,
            allow_missing_lots=allow_missing_lots,
        )
        for bucket in grouped.values()
    ]
    ledgers.sort(key=lambda item: (item.base, item.quote))

    warnings = list(parse_warnings)
    by_base: dict[str, list[LedgerResult]] = {}
    for ledger in ledgers:
        by_base.setdefault(ledger.base, []).append(ledger)

    for base, group in by_base.items():
        if len(group) > 1:
            quotes = ", ".join(item.quote for item in group)
            text = (
                f"{base} birden fazla quote ile işlem görmüş ({quotes}). "
                "Defterler ayrı tutuldu, USDT'ye çevrilmedi."
            )
            warnings.append(text)
            for item in group:
                item.warnings.append(text)

    for base, qty in holdings.items():
        group = by_base.get(base)
        if not group:
            warnings.append(
                f"Cüzdan override {base}={fmt_plain(qty, 8)} için işlem yok, yok sayıldı."
            )
            continue
        target = group[0]
        if len(group) > 1:
            usdt = next((item for item in group if item.quote == "USDT"), None)
            target = usdt or max(group, key=lambda item: item.net_qty)
            warnings.append(
                f"Cüzdan {base}={fmt_plain(qty, 8)} çoklu quote nedeniyle {target.pair} defterine yazıldı."
            )
        target.wallet_qty = qty
        gap = abs(qty - target.net_qty)
        limit = max(Decimal("0.001"), abs(target.net_qty) * flat_pct)
        if gap > limit:
            target.warnings.append(
                f"Cüzdan ({fmt_plain(qty, 8)} {base}) ile dosya kalanı "
                f"({fmt_plain(target.net_qty, 8)} {base}) farklı (fark {fmt_plain(gap, 8)})."
            )
        elif gap > DUST:
            target.notes.append(
                f"Cüzdan {fmt_plain(qty, 8)} {base}, dosya {fmt_plain(target.net_qty, 8)} {base}. "
                "Fark toz eşiğinde, maliyet lotlardan."
            )

    for base, price in prices.items():
        group = by_base.get(base, [])
        if not group:
            warnings.append(f"Fiyat {base}={fmt_plain(price, 8)} için işlem yok.")
            continue
        for item in group:
            item.price = price
            if item.quote not in {"USDT", "USDC", "FDUSD", "USD", "USDS"}:
                item.notes.append(
                    f"Anlık fiyat {fmt_plain(price, 8)}, {item.quote} defterine quote çevrilmeden uygulandı."
                )

    for conversion in conversions:
        warnings.append(
            f"Çeviri (PnL dışı): {conversion.pair} {conversion.side} "
            f"{fmt_plain(conversion.executed_qty, 8)} @ {fmt_plain(conversion.price, 8)}"
        )

    return Analysis(
        method=method,
        files=source_names,
        ledgers=ledgers,
        conversions=conversions,
        warnings=warnings,
        holdings=holdings,
        prices=prices,
        flat_pct=flat_pct,
        flat_qty=flat_qty,
        allow_missing_lots=allow_missing_lots,
    )
