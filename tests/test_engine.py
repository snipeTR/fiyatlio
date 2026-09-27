"""Sentetik senaryolar: maliyet, etiket, komisyon, çoklu fill, fazla satış, iki dönem."""

from __future__ import annotations

from datetime import datetime, timedelta
from decimal import Decimal

from tests.helpers import F, events, ledger


def test_tek_alim_satis_yok():
    result = ledger([F("2026-03-01 10:00:00", "ETHUSDT", "BUY", "2000", "1.5", "3000", order="1")])
    assert result.net_qty == Decimal("1.5")
    assert result.total_cost == Decimal("3000")
    assert result.avg_cost == Decimal("2000")
    assert len(result.lots) == 1
    assert result.lots[0].unit_cost_quote == Decimal("2000")
    assert events(result) == ["ILK_ALIM"]
    assert result.cycles[0].status == "AÇIK"
    assert result.cycles[0].remaining_qty == Decimal("1.5")


def test_al_sat_sifirlama_realized():
    net = Decimal("1") - Decimal("0.001")
    sell_amount = net * Decimal("1100")
    sell_fee = sell_amount * Decimal("0.001")
    result = ledger(
        [
            F("2026-03-01 10:00:00", "ETHUSDT", "BUY", "1000", "1", "1000", fee="0.001", fee_asset="ETH", order="1"),
            F(
                "2026-03-02 10:00:00",
                "ETHUSDT",
                "SELL",
                "1100",
                str(net),
                str(sell_amount),
                fee=str(sell_fee),
                fee_asset="USDT",
                order="2",
            ),
        ]
    )
    assert result.net_qty == Decimal("0")
    assert result.lots == []
    proceeds = sell_amount - sell_fee
    assert result.realized == proceeds - Decimal("1000")
    assert events(result) == ["ILK_ALIM", "SIFIRLAMA"]
    assert result.cycles[0].status == "KAPALI"
    assert result.cycles[0].remaining_qty == Decimal("0")


def test_al_ek_al_kismi_sat_ek_al_etiketleri():
    result = ledger(
        [
            F("2026-04-01 10:00:00", "SOLUSDT", "BUY", "100", "1", "100", order="A"),
            F("2026-04-02 10:00:00", "SOLUSDT", "BUY", "120", "1", "120", order="B"),
            F("2026-04-03 10:00:00", "SOLUSDT", "SELL", "130", "0.5", "65", order="C"),
            F("2026-04-04 10:00:00", "SOLUSDT", "BUY", "110", "0.4", "44", order="D"),
        ]
    )
    assert events(result) == ["ILK_ALIM", "EK_ALIM", "KISMI_SATIS", "EK_ALIM"]
    assert result.cycles[0].status == "AÇIK"
    assert result.net_qty == Decimal("1.9")
    # FIFO: ilk lottan 0.5 gitti, 0.5 kaldı. Sonra 1 ve 0.4 duruyor.
    assert result.lots[0].qty == Decimal("0.5")
    assert result.lots[0].unit_cost_quote == Decimal("100")
    assert result.lots[1].qty == Decimal("1")
    assert result.lots[1].unit_cost_quote == Decimal("120")
    assert result.lots[2].qty == Decimal("0.4")
    assert result.realized == (Decimal("130") - Decimal("100")) * Decimal("0.5")


def test_ikinci_satis_ek_satis():
    result = ledger(
        [
            F("2026-04-01 10:00:00", "SOLUSDT", "BUY", "100", "1", "100", order="A"),
            F("2026-04-02 10:00:00", "SOLUSDT", "SELL", "110", "0.2", "22", order="B"),
            F("2026-04-03 10:00:00", "SOLUSDT", "SELL", "120", "0.2", "24", order="C"),
        ]
    )
    assert events(result) == ["ILK_ALIM", "KISMI_SATIS", "EK_SATIS"]
    assert result.cycles[0].status == "AÇIK"


def test_fee_base_ve_fee_quote():
    alis = ledger(
        [F("2026-05-01 10:00:00", "BTCUSDT", "BUY", "100", "2", "200", fee="0.002", fee_asset="BTC", order="1")]
    )
    assert alis.net_qty == Decimal("1.998")
    assert alis.total_cost == Decimal("200")
    assert alis.lots[0].unit_cost_quote == Decimal("200") / Decimal("1.998")

    quote_fee = ledger(
        [F("2026-05-01 11:00:00", "BTCUSDT", "BUY", "100", "1", "100", fee="1", fee_asset="USDT", order="1")]
    )
    assert quote_fee.net_qty == Decimal("1")
    assert quote_fee.total_cost == Decimal("101")
    assert quote_fee.avg_cost == Decimal("101")

    sat_quote = ledger(
        [
            F("2026-05-02 10:00:00", "BTCUSDT", "BUY", "100", "1", "100", order="1"),
            F("2026-05-02 11:00:00", "BTCUSDT", "SELL", "110", "1", "110", fee="1.1", fee_asset="USDT", order="2"),
        ]
    )
    assert sat_quote.net_qty == Decimal("0")
    assert sat_quote.realized == Decimal("108.9") - Decimal("100")
    assert sat_quote.fills[1].result.unit_proceeds == Decimal("108.9")

    sat_base = ledger(
        [
            F("2026-05-03 10:00:00", "BTCUSDT", "BUY", "100", "1.001", "100.1", order="1"),
            F(
                "2026-05-03 11:00:00",
                "BTCUSDT",
                "SELL",
                "110",
                "1",
                "110",
                fee="0.001",
                fee_asset="BTC",
                order="2",
            ),
        ]
    )
    assert sat_base.net_qty == Decimal("0")
    # Satılan 1 BTC getirisi 110, maliyet 100. Base komisyon 0.001 × 100 zarar.
    assert sat_base.realized == Decimal("10") - Decimal("0.1")


def test_ayni_emir_cok_fill():
    result = ledger(
        [
            F("2026-06-01 12:00:00", "ETHUSDT", "BUY", "100", "0.4", "40", order="9"),
            F("2026-06-01 12:00:01", "ETHUSDT", "BUY", "110", "0.6", "66", order="9"),
            F("2026-06-02 12:00:00", "ETHUSDT", "SELL", "120", "0.3", "36", order="10"),
            F("2026-06-02 12:00:01", "ETHUSDT", "SELL", "130", "0.2", "26", order="10"),
        ]
    )
    assert events(result) == ["ILK_ALIM", "ILK_ALIM", "KISMI_SATIS", "KISMI_SATIS"]
    assert len(result.orders) == 2
    buy = result.orders[0]
    assert buy.fill_count == 2
    assert buy.executed_qty == Decimal("1")
    assert buy.quote_qty == Decimal("106")
    assert buy.vwap == Decimal("106")
    assert buy.event_type == "ILK_ALIM"
    sell = result.orders[1]
    assert sell.fill_count == 2
    assert sell.event_type == "KISMI_SATIS"
    assert sell.executed_qty == Decimal("0.5")
    # İkinci fill yeni satış sayılmadı; dönem hâlâ açık, kalan 0.5.
    assert result.net_qty == Decimal("0.5")
    assert result.cycles[0].status == "AÇIK"


def test_fazla_satis_uyarisi_ve_hayali_lot():
    fills = [
        F("2026-07-01 10:00:00", "ETHUSDT", "BUY", "100", "1", "100", order="1"),
        F("2026-07-02 10:00:00", "ETHUSDT", "SELL", "150", "2", "300", order="2"),
    ]
    result = ledger(fills)
    assert events(result) == ["ILK_ALIM", "FAZLA_SATIS"]
    assert result.net_qty == Decimal("-1")
    assert result.deficit == Decimal("1")
    assert result.lots == []
    assert result.realized == Decimal("50")
    assert any("Eksik lot" in text for text in result.warnings)
    assert any("Hayali lot açılmadı" in text for text in result.warnings)

    covered = ledger(fills, allow=True)
    assert covered.net_qty == Decimal("0")
    assert covered.deficit == Decimal("0")
    assert covered.realized == Decimal("50")
    assert any("hayali lotla kapatıldı" in text for text in covered.warnings)
    assert covered.fills[1].event_type == "FAZLA_SATIS"


def test_iki_donem_sifirlama_ve_yeniden_alim():
    result = ledger(
        [
            F("2026-08-01 10:00:00", "BTCUSDT", "BUY", "10", "1", "10", order="1"),
            F("2026-08-02 10:00:00", "BTCUSDT", "SELL", "12", "1", "12", order="2"),
            F("2026-08-03 10:00:00", "BTCUSDT", "BUY", "11", "2", "22", order="3"),
            F("2026-08-04 10:00:00", "BTCUSDT", "SELL", "13", "2", "26", order="4"),
        ]
    )
    assert events(result) == ["ILK_ALIM", "SIFIRLAMA", "YENIDEN_ALIM", "SIFIRLAMA"]
    assert len(result.cycles) == 2
    assert all(cycle.status == "KAPALI" for cycle in result.cycles)
    assert result.net_qty == Decimal("0")
    assert result.realized == Decimal("2") + Decimal("4")
    assert result.cycles[1].number == 2
    assert "YENIDEN_ALIM" in result.cycles[1].events[0]


def test_lifo_ve_ortalama():
    fills = [
        F("2026-09-01 10:00:00", "ETHUSDT", "BUY", "100", "1", "100", order="1"),
        F("2026-09-01 11:00:00", "ETHUSDT", "BUY", "300", "1", "300", order="2"),
        F("2026-09-01 12:00:00", "ETHUSDT", "SELL", "250", "1", "250", order="3"),
    ]
    fifo = ledger(fills, method="fifo")
    assert fifo.lots[0].unit_cost_quote == Decimal("300")
    assert fifo.realized == Decimal("150")

    lifo = ledger(fills, method="lifo")
    assert lifo.lots[0].unit_cost_quote == Decimal("100")
    assert lifo.realized == Decimal("-50")

    avg = ledger(fills, method="avg")
    assert len(avg.lots) == 1
    assert avg.lots[0].unit_cost_quote == Decimal("200")
    assert avg.net_qty == Decimal("1")
    assert avg.total_cost == Decimal("200")
    assert avg.realized == Decimal("50")


def test_toz_esigi_donemi_kapatir_biraz_ustu_kapatmaz():
    kapanan = ledger(
        [
            F("2026-01-01 00:00:00", "ETHUSDT", "BUY", "100", "1", "100", order="1"),
            F("2026-01-02 00:00:00", "ETHUSDT", "SELL", "100", "0.99995", "99.995", order="2"),
        ]
    )
    assert events(kapanan) == ["ILK_ALIM", "SIFIRLAMA"]
    assert kapanan.net_qty == Decimal("0")

    acik = ledger(
        [
            F("2026-01-01 00:00:00", "ETHUSDT", "BUY", "100", "1", "100", order="1"),
            F("2026-01-02 00:00:00", "ETHUSDT", "SELL", "100", "0.9998", "99.98", order="2"),
        ]
    )
    assert events(acik) == ["ILK_ALIM", "KISMI_SATIS"]
    assert acik.net_qty == Decimal("0.0002")
    assert acik.cycles[0].status == "AÇIK"


def test_binlerce_fill_saniyeler_icinde():
    start = datetime(2024, 1, 1, 0, 0, 0)
    fills = []
    for index in range(1000):
        bought = start + timedelta(minutes=index * 2)
        sold = bought + timedelta(minutes=1)
        stamp = "%Y-%m-%d %H:%M:%S"
        fills.append(F(bought.strftime(stamp), "BTCUSDT", "BUY", "100", "1", "100", order=f"b{index}"))
        fills.append(F(sold.strftime(stamp), "BTCUSDT", "SELL", "101", "1", "101", order=f"s{index}"))
    import time

    t0 = time.perf_counter()
    result = ledger(fills)
    elapsed = time.perf_counter() - t0
    assert elapsed < 5, elapsed
    assert result.net_qty == Decimal("0")
    assert len(result.cycles) == 1000
    assert result.realized == Decimal("1000")
