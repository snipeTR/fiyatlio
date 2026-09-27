"""Ortak sayı yardımcıları. Motor float kullanmaz; yuvarlama yalnız çıktıda."""

from __future__ import annotations

import re
from decimal import ROUND_HALF_UP, Decimal, InvalidOperation

# Bu eşiğin altındaki bakiye yok sayılır (spec: qty < 1e-8).
DUST = Decimal("1e-8")
QTY_PLACES = 8
QUOTE_PLACES = 8

_ASSET_RE = re.compile(
    r"^([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)([A-Za-z][A-Za-z0-9]*)$"
)


def D(value: Decimal | int | str) -> Decimal:
    if isinstance(value, Decimal):
        return value
    return Decimal(str(value))


def snap(qty: Decimal) -> Decimal:
    """Toz bakiyeyi tam sıfır yap. Eşik katı küçüktür: 1e-8 sıfır değildir."""
    if abs(qty) < DUST:
        return Decimal("0")
    return qty


def parse_decimal(raw: str) -> Decimal:
    text = raw.strip().replace(" ", "")
    if not text:
        raise InvalidOperation("boş sayı")
    # Binance nokta kullanır. Yalnız virgül varsa ondalık kabul et.
    if "," in text and "." not in text:
        text = text.replace(",", ".")
    elif "," in text and "." in text:
        text = text.replace(",", "")
    return Decimal(text)


def split_qty_asset(raw: str, expected: list[str] | None = None) -> tuple[Decimal, str]:
    """'0.075PAXG' veya '0.075 PAXG' → (Decimal, 'PAXG').

    Varlık 1INCH gibi rakamla başlayabiliyor. Pair'den gelen beklenen
    varlık, sondan eşleşirse o tercih edilir; aksi halde harfle başlayan
    sonek ayrıştırılır.
    """
    text = raw.strip()
    if not text:
        raise ValueError("boş miktar")
    hints = sorted({a.upper() for a in (expected or []) if a}, key=len, reverse=True)
    upper = text.upper()
    for asset in hints:
        if upper.endswith(asset):
            number = text[: len(text) - len(asset)].strip()
            if number:
                return parse_decimal(number), asset
    compact = re.sub(r"\s+", "", text)
    match = _ASSET_RE.fullmatch(compact)
    if not match:
        raise ValueError(f"miktar ayrıştırılamadı: {raw}")
    return parse_decimal(match.group(1)), match.group(2).upper()


def quantize(value: Decimal, places: int) -> Decimal:
    step = Decimal(1).scaleb(-places)
    return value.quantize(step, rounding=ROUND_HALF_UP)


def fmt_plain(value: Decimal | None, places: int = 8) -> str:
    """CSV için noktalı ondalık, gereksiz sıfır yok."""
    if value is None:
        return ""
    text = f"{quantize(value, places):.{places}f}"
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    if text in ("", "-0"):
        return "0"
    return text


def fmt_tr(value: Decimal | None, places: int = 8) -> str:
    """İnsan metni: 3.341,1889"""
    if value is None:
        return "—"
    negative = value < 0
    text = fmt_plain(abs(value), places)
    if "." in text:
        whole, frac = text.split(".")
    else:
        whole, frac = text, ""
    groups: list[str] = []
    while whole:
        groups.append(whole[-3:])
        whole = whole[:-3]
    whole_tr = ".".join(reversed(groups)) if groups else "0"
    body = f"{whole_tr},{frac}" if frac else whole_tr
    return f"-{body}" if negative else body


def fmt_signed_tr(value: Decimal | None, places: int = 4) -> str:
    if value is None:
        return "—"
    text = fmt_tr(value, places)
    if value > 0:
        return f"+{text}"
    return text


def fmt_pct(value: Decimal | None) -> str:
    if value is None:
        return "—"
    if value > 0:
        return f"+%{fmt_tr(value, 2)}"
    if value < 0:
        return f"-%{fmt_tr(abs(value), 2)}"
    return f"%{fmt_tr(value, 2)}"
