"""Binance Spot Trade History CSV okuyucu.

Desteklenen başlıklar:
  Order No, Time, Pair, Side, Price, Executed, Amount, Fee,
  AOR Conversion Pair, AOR Conversion Rate
ve eski export: Time, Pair, Side, Price, Executed, Amount, Fee.
"""

from __future__ import annotations

import csv
import io
import re
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal, InvalidOperation
from pathlib import Path

from fiyatlio.util import parse_decimal, split_qty_asset

# Uzun quote önce denenir (FDUSD, USDT, USD sırası bozulmasın).
KNOWN_QUOTES: tuple[str, ...] = (
    "FDUSD",
    "USDT",
    "USDC",
    "TUSD",
    "BUSD",
    "USDP",
    "BIDR",
    "IDRT",
    "USDS",
    "TRY",
    "EUR",
    "BTC",
    "ETH",
    "BNB",
    "DAI",
    "BRL",
    "AUD",
    "GBP",
    "RUB",
    "DOGE",
    "TRX",
    "XRP",
    "SOL",
    "USD",
)

# Stable-stable çiftler PnL motoruna girmez.
STABLES = frozenset({"USDT", "USDC", "FDUSD", "TUSD", "BUSD", "USDP", "DAI", "USD", "USDS"})

_COLUMN_MAP = {
    "orderno": "order_no",
    "orderid": "order_no",
    "time": "time",
    "dateutc": "time",
    "date": "time",
    "pair": "pair",
    "market": "pair",
    "symbol": "pair",
    "side": "side",
    "price": "price",
    "executed": "executed",
    "amount": "amount",
    "fee": "fee",
    "commission": "fee",
    "aorconversionpair": "aor_pair",
    "aorconversionrate": "aor_rate",
}

_TIME_FORMATS = ("%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S.%f")


@dataclass(frozen=True)
class Fill:
    order_no: str
    time: datetime
    pair: str
    side: str
    price: Decimal
    executed_qty: Decimal
    executed_asset: str
    amount_qty: Decimal
    amount_asset: str
    fee_qty: Decimal
    fee_asset: str | None
    aor_pair: str | None
    aor_rate: Decimal | None
    source_file: str
    source_row: int
    base_asset: str
    quote_asset: str
    is_conversion: bool


@dataclass
class ParseResult:
    fills: list[Fill]
    conversions: list[Fill]
    warnings: list[str]


def split_pair(pair: str) -> tuple[str, str]:
    """PAXGUSDT → (PAXG, USDT). Bilinen quote sonekleri, uzun olan önce."""
    symbol = pair.strip().upper()
    for quote in KNOWN_QUOTES:
        if symbol.endswith(quote) and len(symbol) > len(quote):
            return symbol[: -len(quote)], quote
    raise ValueError(f"quote çözülemedi: {pair}")


def _norm_header(name: str | None) -> str:
    if name is None:
        return ""
    return re.sub(r"[^a-z0-9]", "", name.strip().lower())


def _parse_time(raw: str) -> datetime:
    text = raw.strip()
    if text.endswith("Z"):
        text = text[:-1]
    for fmt in _TIME_FORMATS:
        try:
            return datetime.strptime(text, fmt)
        except ValueError:
            continue
    raise ValueError(f"zaman ayrıştırılamadı: {raw}")


def _order_sort_value(order_no: str) -> tuple[int, int | str]:
    if not order_no:
        return (1, "")
    try:
        return (0, int(order_no))
    except ValueError:
        return (1, order_no)


def fill_sort_key(fill: Fill) -> tuple:
    return (fill.time, _order_sort_value(fill.order_no), fill.source_file, fill.source_row)


def _decode(path: Path) -> str:
    raw = path.read_bytes()
    for encoding in ("utf-8-sig", "utf-8", "cp1254", "latin-1"):
        try:
            return raw.decode(encoding)
        except UnicodeDecodeError:
            continue
    return raw.decode("utf-8", errors="replace")


def _reader(text: str) -> csv.DictReader:
    first = ""
    for line in text.splitlines():
        if line.strip():
            first = line
            break
    delimiter = ";" if first.count(";") > first.count(",") else ","
    return csv.DictReader(io.StringIO(text), delimiter=delimiter)


def _cell(row: dict[str, str], key: str) -> str:
    value = row.get(key, "")
    if value is None:
        return ""
    return str(value).strip()


def parse_file(path: str | Path) -> ParseResult:
    file_path = Path(path)
    if not file_path.is_file():
        raise FileNotFoundError(f"Dosya yok: {file_path}")
    text = _decode(file_path)
    if not text.strip():
        return ParseResult([], [], [f"{file_path.name}: dosya boş"])

    reader = _reader(text)
    if not reader.fieldnames:
        return ParseResult([], [], [f"{file_path.name}: başlık satırı yok"])

    mapped: dict[str, str] = {}
    for original in reader.fieldnames:
        key = _COLUMN_MAP.get(_norm_header(original))
        if key and key not in mapped:
            mapped[key] = original

    missing = [name for name in ("time", "pair", "side", "price", "executed", "amount") if name not in mapped]
    if missing:
        return ParseResult(
            [],
            [],
            [f"{file_path.name}: zorunlu kolon yok ({', '.join(missing)})"],
        )

    fills: list[Fill] = []
    conversions: list[Fill] = []
    warnings: list[str] = []
    # DictReader ilk satırı başlık sayar; Excel satır numarası 2'den başlar.
    for offset, raw_row in enumerate(reader, start=2):
        row = {key: _cell(raw_row, original) for key, original in mapped.items()}
        if not any(row.values()):
            continue
        try:
            fill = _parse_row(row, file_path.name, offset)
        except (ValueError, InvalidOperation) as exc:
            warnings.append(f"{file_path.name} satır {offset} atlandı: {exc}")
            continue
        gross = fill.price * fill.executed_qty
        if fill.amount_qty > 0:
            gap = abs(gross - fill.amount_qty)
            if gap > max(Decimal("0.01"), fill.amount_qty * Decimal("0.001")):
                warnings.append(
                    f"{file_path.name} satır {offset}: tutar ({fill.amount_qty}) "
                    f"fiyat×miktar ({gross}) ile uyuşmuyor, kolonlar olduğu gibi kullanıldı"
                )
        if fill.is_conversion:
            conversions.append(fill)
        else:
            fills.append(fill)
    return ParseResult(fills, conversions, warnings)


def parse_paths(paths: list[str | Path]) -> ParseResult:
    fills: list[Fill] = []
    conversions: list[Fill] = []
    warnings: list[str] = []
    for path in paths:
        part = parse_file(path)
        fills.extend(part.fills)
        conversions.extend(part.conversions)
        warnings.extend(part.warnings)
    fills.sort(key=fill_sort_key)
    conversions.sort(key=fill_sort_key)
    return ParseResult(fills, conversions, warnings)


def _parse_row(row: dict[str, str], source: str, line_no: int) -> Fill:
    pair = row.get("pair", "").upper()
    if not pair:
        raise ValueError("pair boş")
    base, quote = split_pair(pair)
    side = row.get("side", "").upper()
    if side not in ("BUY", "SELL"):
        raise ValueError(f"taraf BUY/SELL değil: {row.get('side', '')}")
    price = parse_decimal(row.get("price", ""))
    if price < 0:
        raise ValueError("fiyat negatif")
    executed_qty, executed_asset = split_qty_asset(row.get("executed", ""), [base])
    amount_qty, amount_asset = split_qty_asset(row.get("amount", ""), [quote])
    if executed_qty < 0 or amount_qty < 0:
        raise ValueError("miktar negatif")
    if executed_qty == 0:
        raise ValueError("executed sıfır")
    if executed_asset != base:
        raise ValueError(f"executed varlığı {executed_asset}, pair base {base}")
    if amount_asset != quote:
        raise ValueError(f"amount varlığı {amount_asset}, pair quote {quote}")

    fee_raw = row.get("fee", "")
    fee_qty = Decimal("0")
    fee_asset: str | None = None
    if fee_raw and fee_raw not in {"0", "0.0", "0.00"}:
        fee_qty, fee_asset = split_qty_asset(fee_raw, [base, quote, "BNB"])
        if fee_qty < 0:
            raise ValueError("komisyon negatif")

    aor_pair = row.get("aor_pair") or None
    aor_rate: Decimal | None = None
    aor_raw = row.get("aor_rate", "")
    if aor_raw:
        aor_rate = parse_decimal(aor_raw)

    return Fill(
        order_no=row.get("order_no", ""),
        time=_parse_time(row.get("time", "")),
        pair=pair,
        side=side,
        price=price,
        executed_qty=executed_qty,
        executed_asset=executed_asset,
        amount_qty=amount_qty,
        amount_asset=amount_asset,
        fee_qty=fee_qty,
        fee_asset=fee_asset,
        aor_pair=aor_pair,
        aor_rate=aor_rate,
        source_file=source,
        source_row=line_no,
        base_asset=base,
        quote_asset=quote,
        is_conversion=base in STABLES and quote in STABLES,
    )
