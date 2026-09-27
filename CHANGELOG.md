# Değişiklik günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr-TR/1.1.0/) tarzındadır.
Sürüm numarası `package.json`, `src-tauri/Cargo.toml` ve `src-tauri/tauri.conf.json` ile aynıdır.

Bir sürüm yayınlamak için bu üç dosyadaki numarayı yükseltin, aşağıya o sürüme ait bir bölüm ekleyin ve `vX.Y.Z` etiketini gönderin. GitHub Actions Windows kurulumunu derleyip bu dosyayla birlikte release açar.

## [Unreleased]

## [0.3.0] - 2026-09-27

### Eklenen

- Binance spot (eski `Date(UTC)` ve güncel `Order No`), emir geçmişi, hesap hareketi, yatırma/çekme ve USDⓈ-M vadeli başlıkları tanınır.
- Bybit, KuCoin, OKX, Coinbase, Kraken, Gate ve Bitget exportları tanınır. Örnekler `samples/formats/` altındadır.
- Aynı başlık birden fazla biçime uyarsa uygulama dosyayı almadan önce kullanıcıya sorar.

## [0.2.0] - 2026-09-27

### Düzeltilen

- Masaüstü uygulaması güncel Binance spot exportunu da okur. `Order No,Time,Pair,Side,Price,Executed,Amount,Fee,AOR Conversion Pair,AOR Conversion Rate` başlığı artık reddedilmez.
- Eski `Time,Pair,Side,Price,Executed,Amount,Fee` başlığı da duruyor. Kolon sırası önemli değil; eşleşme ada göredir.

## [0.1.0] - 2026-09-27

### Eklenen

- Windows masaüstü uygulaması: Binance spot CSV, FIFO ve ortalama maliyet, grafikler, HTML rapor.
- Çevrimdışı Python komut satırı: `fifo` / `lifo` / `avg`, dönem etiketleri, CSV, HTML ve Excel çıktısı.
- Anonim PAXG örnek dosyası (`samples/paxg_ornek.csv`). Emir numaraları değiştirilmiştir.

### Bilinen

- Kurulum dosyası imzasızdır. Windows SmartScreen ilk çalıştırmada uyarı gösterebilir.
