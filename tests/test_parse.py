"""CSV okuma: gerçek kolonlar, eski export, stable çeviri, 1INCH soneki."""

from __future__ import annotations

from decimal import Decimal

from fiyatlio.cli import analyze_files
from fiyatlio.parse_binance import parse_file, split_pair, split_qty_asset


def test_pair_ve_paxg_satiri(tmp_path):
    from fiyatlio.parse_binance import split_qty_asset

    assert split_qty_asset("320.3325USDT", ["USDT"]) == (Decimal("320.3325"), "USDT")
    assert split_pair("PAXGUSDT") == ("PAXG", "USDT")
    assert split_pair("ETHBTC") == ("ETH", "BTC")
    assert split_pair("USDCUSDT") == ("USDC", "USDT")
    assert split_pair("SOLUSDC") == ("SOL", "USDC")
    qty, asset = split_qty_asset("0.075PAXG", ["PAXG"])
    assert qty == Decimal("0.075")
    assert asset == "PAXG"
    qty, asset = split_qty_asset("10.51INCH", ["1INCH"])
    assert asset == "1INCH"
    assert qty == Decimal("10.5")

    path = tmp_path / "spot.csv"
    path.write_text(
        "Order No,Time,Pair,Side,Price,Executed,Amount,Fee,AOR Conversion Pair,AOR Conversion Rate\n"
        "650006707,2026-03-23 11:49:10,PAXGUSDT,BUY,4271.1,0.075PAXG,320.3325USDT,0.000075PAXG,,\n"
        "1,2026-03-24 10:00:00,USDCUSDT,BUY,1,25USDC,25USDT,0USDT,,\n"
        "2,2026-03-24 11:00:00,SOLUSDT,BUY,150,1SOL,150USDT,0.001SOL,,\n"
        "3,2026-03-24 12:00:00,SOLUSDC,BUY,150,2SOL,300USDC,0.002SOL,,\n"
        "bozuk,satir\n",
        encoding="utf-8",
    )
    parsed = parse_file(path)
    assert len(parsed.fills) == 3
    assert len(parsed.conversions) == 1
    assert parsed.conversions[0].pair == "USDCUSDT"
    assert parsed.fills[0].executed_qty == Decimal("0.075")
    assert parsed.fills[0].fee_qty == Decimal("0.000075")
    assert parsed.fills[0].fee_asset == "PAXG"
    assert parsed.fills[0].amount_qty == Decimal("320.3325")
    assert any("atlandı" in text for text in parsed.warnings)

    analysis = analyze_files([path])
    pairs = {(item.base, item.quote) for item in analysis.ledgers}
    assert pairs == {("PAXG", "USDT"), ("SOL", "USDT"), ("SOL", "USDC")}
    assert any("birden fazla quote" in text for text in analysis.warnings)
    assert any("Çeviri" in text for text in analysis.warnings)


def test_eski_baslik_order_no_yok(tmp_path):
    path = tmp_path / "eski.csv"
    path.write_text(
        "Time,Pair,Side,Price,Executed,Amount,Fee\n"
        "2026-07-14 17:09:20,ETHUSDT,BUY,1864.77,0.2681ETH,499.944837USDT,0.0002681ETH\n",
        encoding="utf-8",
    )
    parsed = parse_file(path)
    assert len(parsed.fills) == 1
    fill = parsed.fills[0]
    assert fill.order_no == ""
    assert fill.base_asset == "ETH"
    assert fill.executed_qty == Decimal("0.2681")
    net = Decimal("0.2681") - Decimal("0.0002681")
    assert fill.amount_qty == Decimal("499.944837")
    analysis = analyze_files([path])
    assert analysis.ledgers[0].net_qty == net
    assert analysis.ledgers[0].total_cost == Decimal("499.944837")
