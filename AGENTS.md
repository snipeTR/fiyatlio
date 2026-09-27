# AGENTS.md — Fiyatlio

> Bu dosya, projede çalışan tüm yapay zeka ajanları (Claude Code, Codex, Cursor, Copilot Workspace vb.)
> için tek kaynak-doğruluk (single source of truth) talimat dosyasıdır.
> `CLAUDE.md` bu dosyaya işaret eder; talimat değişikliği **yalnızca burada** yapılır.

---

## 1. Proje kimliği

| Alan | Değer |
|---|---|
| Ürün adı | **Fiyatlio** (FIYAT + portfolio) |
| Bundle identifier | `com.fiyatlio.app` |
| Rust crate | `fiyatlio` / lib adı `fiyatlio_lib` |
| npm paket adı | `fiyatlio` |
| Pencere başlığı | `Fiyatlio — Binance Spot Analiz` |
| Rapor başlığı | `Fiyatlio Report` |

Bu isim projenin **her yerinde** aynen kullanılır. Yeni bir dosyada farklı bir isim (fifolio, spotrail vb.) görürsen bu bir hatadır, düzelt.

**Amaç:** Binance'ten indirilen spot trade geçmişi CSV'lerini okuyup FIFO (varsayılan) veya Average Cost
yöntemiyle realized/unrealized P&L hesaplayan, canlı fiyatlarla portföy değeri gösteren, grafik ve rapor
üreten **offline-first** masaüstü uygulaması. **API key gerekmez**, yalnızca Binance public endpoint'leri.

---

## 2. Değişmez kurallar (non-negotiable)

1. **Parasal hesaplarda asla `f64` / `f32` kullanma.** Tüm miktar, fiyat, maliyet, P&L değerleri
   `rust_decimal::Decimal`. Frontend'e **string** olarak serialize edilir
   (`#[serde(with = "rust_decimal::serde::str")]`) — JSON `number` precision kaybettirir.
2. **Production kodunda `unwrap()` / `expect()` / `panic!()` yok.** Yalnızca `#[cfg(test)]` blokları serbest.
   Her fallible yol `Result<T, AppError>` döner.
3. **Hesaplama daima UTC.** Yerel saate çevirme yalnızca görüntüleme katmanında (frontend).
4. **HTTP istekleri yalnızca Rust tarafında** (`reqwest`). Frontend'den `fetch` ile dış servise gidilmez
   (CSP/CORS). Frontend sadece `invoke()` çağırır.
5. **Hiçbir hata sessizce yutulmaz.** Loglanır (`tracing`) + kullanıcıya çevrilmiş mesajla gösterilir.
6. **Proje klasörü dışına dosya yazılmaz.** Geçici çıktılar dahil her şey repo içinde kalır
   (kullanıcının bilinçli olarak seçtiği rapor/export kaydetme konumu hariç).
7. Kod yorumları **İngilizce**; kullanıcıya görünen tüm metinler i18n dosyalarında **TR + EN**.
8. Determinizm: aynı CSV girdisi → bit düzeyinde aynı çıktı. Rastgelelik, sistem saati bağımlılığı yok
   (fiyat çekme hariç, o da zaman damgalanır).

---

## 3. Teknoloji yığını

- **Tauri 2** (Rust backend)
- **SvelteKit 2 + Svelte 5 (runes) + TypeScript strict**, `adapter-static`, `ssr = false`, `prerender = true`
- **TailwindCSS 4** (`@tailwindcss/vite`, CSS-first config — `tailwind.config.js` **yok**) + **shadcn-svelte**
- **Apache ECharts** (canvas renderer)
- Rust: `csv`, `serde`, `chrono`, `reqwest` (rustls-tls), `tokio`, `rust_decimal`, `thiserror`, `tracing`
- Tauri pluginleri: `dialog`, `fs` (scope'lu), `store`, `opener`
- Dark mode varsayılan, light/system seçilebilir

---

## 3.1 Sapmalar ve gerekçeleri

Prompt'taki teknoloji seçimlerinden bilinçli olarak ayrıldığımız noktalar:

| Karar | Gerekçe |
|---|---|
| **shadcn-svelte CLI kullanılmadı**, `$lib/components/ui/` altındaki bileşenler elle yazıldı | `components.json` hazır duruyor, istenirse `npx shadcn-svelte add …` ile bileşen çekilebilir. Ama uygulamanın ihtiyacı olan yüzey (Card, Button, StatCard, Field, Icon) beş küçük dosya; CLI'ın interaktif init adımına ve registry sürüm bağımlılığına build'i bağlamamak için elle yazıldı. Tema token'ları shadcn ile birebir aynı, sonradan geçiş sorunsuz. |
| **`svelte-i18n` yerine elle yazılmış rune tabanlı i18n** | İki locale, lazy loading yok, pluralization kuralı yok. Kütüphane karşılığında Svelte 5'in senkron rune modeliyle çatışan bir async init dansı gelirdi. `en.ts`, `tr.ts`'in anahtar kümesiyle tiplendiği için eksik anahtar **compile hatası**. |
| **İkon paketi yok**, `Icon.svelte` içinde inline SVG path'leri | ~15 glif için bir paket bağımlılığı ve tree-shaking takibi gereksiz. |
| **Virtualization elle yazıldı** (`routes/trades`) | Satır yüksekliği sabit; problem diziyi scroll offset'e göre dilimlemeye indirgeniyor. Ölçüm makinesi olan bir kütüphane gereksiz. |
| **Rapor/export yazımı Rust `std::fs` ile**, `tauri-plugin-fs` ile değil | Yol zaten native save dialog'undan geliyor; plugin scope'u frontend fs API'si için anlamlı. Plugin yine de kayıtlı ve capability'de tanımlı. |
| **`get_trades` ayrı command**, `AnalysisResult` içinde değil | Büyük geçmiş on binlerce satır. Fiyat yenilemesi trade'leri değiştirmediği için her refresh'te IPC'den geçirmek refresh maliyetini domine ederdi. |

## 4. Klasör düzeni

```
fiyatlio/
├── AGENTS.md                      # ← bu dosya (tek talimat kaynağı)
├── CLAUDE.md                      # AGENTS.md'ye pointer
├── README.md
├── package.json / svelte.config.js / vite.config.ts / tsconfig.json / components.json
├── src/                           # SvelteKit frontend
│   ├── app.html · app.css · app.d.ts
│   ├── lib/
│   │   ├── components/
│   │   │   ├── ui/                # Card · Button · StatCard · Field · Icon
│   │   │   ├── Chart.svelte       # ECharts sarmalayıcı (+ rapor için snapshot)
│   │   │   ├── FileDrop.svelte    # native dosya seçici + Tauri drag&drop
│   │   │   ├── PriceStatus.svelte # canlı / stale / hiç göstergesi
│   │   │   ├── Sidebar.svelte
│   │   │   └── Toasts.svelte
│   │   ├── charts/registry.ts     # mount'lu grafiklerin PNG snapshot kaydı
│   │   ├── stores/app.svelte.ts   # rune tabanlı tek state kaynağı
│   │   ├── i18n/                  # index.ts · tr.ts · en.ts
│   │   ├── api.ts                 # invoke sarmalayıcıları + hata normalizasyonu
│   │   ├── format.ts              # locale'e duyarlı gösterim (sadece gösterim!)
│   │   ├── types.ts               # Rust modellerinin TS aynası
│   │   └── utils.ts
│   └── routes/                    # /  /portfolio  /trades  /charts  /reports  /settings
├── static/
└── src-tauri/
    ├── Cargo.toml · build.rs · tauri.conf.json
    ├── capabilities/default.json  # Tauri 2 izin sistemi
    ├── icons/
    ├── tests/fixtures/            # golden CSV fixture'ları
    └── src/
        ├── main.rs                # ince giriş noktası
        ├── lib.rs                 # modül ağacı + Tauri builder
        ├── error.rs               # AppError (thiserror)
        ├── models.rs              # Trade, Lot, Position, RealizedEvent, Settings…
        ├── parser.rs              # CSV → Trade
        ├── engine/
        │   ├── mod.rs             # PnlEngine trait + fee kuralları
        │   ├── fifo.rs            # FIFO motoru (varsayılan)
        │   └── avg_cost.rs        # Average Cost motoru
        ├── analysis.rs            # engine çıktısı + fiyat → AnalysisResult
        ├── binance.rs             # public price API (retry, sembol ayıklama)
        ├── state.rs               # AppState + store kalıcılığı
        ├── export.rs              # CSV / JSON dışa aktarım
        ├── report.rs              # self-contained HTML rapor
        └── commands.rs            # #[tauri::command] katmanı
```

### 4.1 Command yüzeyi

| Command | Döner | Not |
|---|---|---|
| `app_version` | `String` | |
| `get_settings` / `set_settings` | `Settings` / `AnalysisResult` | set, diske yazar **ve** yeniden hesaplar |
| `get_analysis` | `AnalysisResult` | açılışta UI'ı doldurur |
| `get_trades` | `Vec<Trade>` | ayrı tutuluyor — bkz. §3.1 |
| `import_csv_files` | `AnalysisResult` | fiyat çekmez; ardından `refresh_prices` çağrılır |
| `clear_data` | `AnalysisResult` | |
| `refresh_prices` | `AnalysisResult` | **async**; ağ hatasında cache'e düşer, hata döndürmez |
| `suggested_export_filename` / `export_data` | `String` | |
| `suggested_report_filename` / `generate_report` | `String` | |

Mutasyon yapan her command tam `AnalysisResult` döner. Delta döndürmek, UI'ın önbelleklediği
rakamların backend'in defteriyle ayrışması sınıfından hataları davet ederdi; tam sonuç dönmek
bu sınıfı tümüyle ortadan kaldırıyor ve hesaplama zaten milisaniyenin altında.

---

## 5. Veri modeli ve CSV parse kuralları

Binance spot trade history export formatı:

```
Time,Pair,Side,Price,Executed,Amount,Fee
2026-07-14 17:09:20,ETHUSDT,BUY,1864.77,0.2681ETH,499.944837USDT,0.0002681ETH
```

**Parse kuralları — hepsi zorunlu:**

1. `Time`: `%Y-%m-%d %H:%M:%S`, **UTC** kabul edilir (`NaiveDateTime` → `DateTime<Utc>`).
2. `Executed` / `Amount` / `Fee` = **sayı + varlık kodu** bitişik (`0.2681ETH`).
   Sayısal kısım ile varlık kodu ayrıştırılır; binlik ayırıcı virgüller (`1,234.56USDT`) temizlenir.
3. `Pair` → base/quote ayrıştırması **suffix eşleştirme** ile; liste **uzunluğa göre azalan** sıralı
   olmalı ki `FDUSD`, `USD`ten önce eşleşsin:
   `["FDUSD","USDT","USDC","TUSD","BUSD","USDP","DAI","TRY","EUR","GBP","BRL","BTC","ETH","BNB"]`
4. `Side`: yalnız `BUY` / `SELL` (case-insensitive).
5. Sayısal alanlar negatif/sıfır olamaz — **Fee sıfır olabilir**.
6. **Bozuk satır import'u durdurmaz**: geçerliler işlenir, bozuklar `(satır no, sebep)` ile raporlanır.
7. **Rakamla başlayan ticker'lar** (`1INCH`, `1000SATS`) sayı/varlık ayrımını belirsiz yapar:
   `"12.51INCH"` hem `12.5 × 1INCH` hem `12.51 × INCH` okunabilir. Bu yüzden `Executed`/`Amount`
   ayrıştırılırken varlık **pair'den bilinerek** suffix olarak sıyrılır (`split_amount_expecting`);
   ilk-harf taraması yalnızca fee gibi varlığı önceden bilinmeyen alanlarda fallback'tir.
8. Zorunlu ek ret sebepleri: `Executed`/`Amount` varlığı pair ile uyuşmuyorsa **assetMismatch**;
   BUY'da base varlıkta ödenen fee tüm fill'i yutuyorsa (`fee >= executed`) **feeExceedsQuantity**.
   İkincisi olmasa açılacak lot sıfır miktarlı olur, o satırın maliyeti sahipsiz kalır ve §8(d)
   korunum invariantı kırılırdı.
9. Header karşılaştırması **büyük/küçük harf duyarsız ama sıra duyarlıdır** (kolonlar kendi birimini
   taşımadığı için sırası değişmiş bir dosya makul görünen saçmalık üretir). UTF-8 BOM temizlenir.

---

## 6. İş kuralları (hesaplama motorunun sözleşmesi)

### 6.1 Sıralama
Trade'ler `timestamp_utc` artan sıralanır. Stable sort + tie-breaker `(file_index, row_index)` —
aynı saniyedeki parçalı fill'lerde CSV'deki orijinal sıra korunur. **Birleştirme/toplulaştırma yapılmaz**;
aynı saniyedeki her BUY ayrı lot.

### 6.2 Çoklu CSV + duplicate
`(timestamp, pair, side, price, qty, quote_amount, fee)` birebir aynı satırlar duplicate sayılır →
varsayılan: tekilleştir + "N mükerrer satır atlandı" uyarısı. Ayarlardan kapatılabilir.

**Tekilleştirme yalnızca DOSYALAR ARASINDA yapılır.** Bu bir detay değil: tek bir Binance export'u
aynı fill'i iki kez listelemez, dolayısıyla *aynı dosya içindeki* birebir aynı satırlar gerçek
ayrı fill'lerdir — algoritmik olarak parçalanmış bir emir, aynı saniyeye damgalanmış on iki adet
özdeş `0.0016ETH` alımı üretir. Gerçek bir kullanıcı export'unda bunları birleştirmek **893 satırın
180'ini** siliyordu. Dosyalar arasında ise aynı satırlar export'ların örtüştüğü anlamına gelir;
her grup için **en çok kopya bildiren dosya** tam olarak korunur, diğerlerininki atılır.

### 6.2.1 Oversell'in görünürlüğü
`DashboardSummary` oversell'i yalnız bayrakla geçmez; `oversell_event_count`, `oversell_pnl_usdt`
ve `oversell_qty_by_asset` alanlarıyla **niceliksel** olarak raporlar. Sebep: Convert/transfer/
Earn ile gelen bakiyelerin satışı, sıfır maliyet politikasıyla manşet rakamı görünür bir sebep
olmadan şişirebilir. Kullanıcının "bu kâr nereden geliyor?" sorusunu ekranda cevaplayabilmesi için
bu rakamlar hesaplanır ve uyarı bandında gösterilir.

### 6.3 FIFO
Pair başına `VecDeque<Lot>`. BUY → push_back. SELL → front'tan eşleştir; kısmi tüketimde maliyet
**oransal** bölünür, tam tüketimde lot pop edilir. Her SELL bir `RealizedEvent` üretir ve
**hangi lottan ne kadar eşleştiği** (`matched_lots`) izlenebilirlik için saklanır.

**Lot maliyeti birim değil, TOPLAM olarak saklanır** (`Lot::cost_remaining` otoritedir;
`unit_cost_quote` yalnızca görüntüleme içindir). Sebep: `Decimal` bölmesi ancak sonuç 28 anlamlı
haneye sığdığında kesindir — `2000 / 0.999` sığmaz. Dilimler `unit_cost × miktar` ile fiyatlansaydı
yuvarlama hatası sızar ve §8(d) invariantı birebir `Decimal` karşılaştırmasında düşerdi.
`Lot::consume` her dilime yüklenen maliyeti **düşerek** verir; lot tamamen tükendiğinde kalan
maliyetin tamamını iade eder. Böylece korunum kurgu gereği sağlanır, yuvarlama artığı lotta kalır.
`avg_cost` motorundaki havuz da aynı şekilde çalışır.

### 6.4 Fee muhasebesi
| Durum | Kural |
|---|---|
| BUY + fee **base** varlıkta | net edinilen = `executed − fee`; maliyet = `amount` |
| BUY + fee **quote** varlıkta | maliyet = `amount + fee`; miktar = `executed` |
| SELL + fee **quote** varlıkta | net gelir = `amount − fee` |
| SELL + fee **base** varlıkta | satılan = `executed`; gelirden `fee × price` düşülür |
| Fee **üçüncü varlıkta** (BNB) | v1'de maliyet tabanına **katılmaz** — tarihsel BNB fiyatı yok. `fees_by_asset` altında ayrı toplanır, dashboard'da güncel fiyatla "≈" ibaresiyle gösterilir. Bu davranış kod yorumunda **ve** UI tooltip'inde açıkça yazılır. |

### 6.5 Oversell
SELL, kuyruktaki toplam miktarı aşarsa (CSV dışı transfer/convert/staking kaynaklı bakiye):
aşan kısım **sıfır maliyet esaslı** eşleştirilir, `RealizedEvent.oversell = true` işaretlenir,
UI'da kalıcı uyarı bandı + Reports'ta liste gösterilir.
Alternatif politika (Settings): "aşan kısmı yok say".

### 6.6 Average Cost
Pair başına tek havuz (`total_qty`, `total_cost`). SELL'de maliyet = `total_cost × (satılan / total_qty)`.
İki motor da aynı `PnlEngine` trait'ini implemente eder; yöntem değişimi tüm rakamları yeniden hesaplar.

### 6.7 USDT dışı quote
P&L **daima kendi quote varlığında** hesaplanır. Dashboard toplamı için quote'un güncel USDT fiyatıyla
çevrilir; çevrilemeyenler toplamın **dışında** bırakılır ve "USDT'ye çevrilemedi" notuyla listelenir.

**Çevrim yönü kritiktir.** Binance bazı fiat'ları USDT'yi *baz* alarak listeler: parite `USDTTRY`'dir,
`TRYUSDT` diye bir sembol yoktur. `<varlık>USDT` biçimini varsaymak, o para biriminde tutulan tüm
pozisyonları sessizce toplamların dışına atar. `binance::USDT_BASE_FIAT` listesindeki varlıklar için
ters sembol kullanılır ve oran `1 / fiyat` olarak alınır. Liste açıkça tutulur, çünkü var olmayan bir
sembol istemek batch endpoint'ten 400 döndürür ve her yenilemede yavaş tek-tek fallback'i tetikler.

Emekliye ayrılmış USD sabit paraları (`binance::USD_PEGGED_FALLBACK`, şu an yalnızca `BUSD`) canlı
piyasası kalmadığı için 1.0 kuruyla değerlenir — bu uydurulmuş bir fiyat değil, ömrü boyunca 1:1
geçerli olmuş bir sabitlemenin ifadesidir. **Yalnızca** canlı bir piyasa cevap vermediğinde uygulanır.

> **Bilinen sınır:** Realized P&L'in USDT'ye çevrimi **bugünkü** kurla yapılır. 2021'de kazanılmış
> bir TRY kârı bugünkü USDT/TRY ile çevrildiğinde tarihsel gerçeği yansıtmaz. Doğrusu, işlem anındaki
> kuru (`/api/v3/klines`) kullanmaktır; v1'de yapılmıyor.

### 6.8 Unrealized P&L
`kalan_miktar × güncel_fiyat − kalan_lot_maliyeti`. Fiyat çekilemezse önbellekteki son fiyat kullanılır
ve UI'da fiyatın zaman damgası ("… itibarıyla") gösterilir.

---

## 7. Binance public API

- Toplu fiyat: `GET https://api.binance.com/api/v3/ticker/price?symbols=["ETHUSDT","BTCUSDT"]`
  (`symbols` JSON array, **URL-encode edilir**). Tüm açık pozisyonlar **tek istekte** çekilir.
- Timeout **5 sn**, başarısızlıkta **2 kez** exponential backoff retry, sonra cache'ten devam + stale göstergesi.
- Fiyatlar + çekilme zamanı `tauri-plugin-store` ile diske yazılır (tam offline açılış senaryosu).
- Manuel Refresh + opsiyonel auto-refresh (**varsayılan kapalı**, minimum 30 sn, ayar değişince interval
  temiz iptal — memory leak yok).
- Geçersiz sembolde API 400 döner: sembolleri tek tek deneyip sorunluları ayıkla, kullanıcıya bildir.

---

## 8. Test sözleşmesi (golden values — değiştirilemez)

Fixture: `src-tauri/tests/fixtures/golden_basic.csv`

```
Time,Pair,Side,Price,Executed,Amount,Fee
2026-01-01 10:00:00,ETHUSDT,BUY,2000,1.0ETH,2000USDT,2USDT
2026-01-02 10:00:00,ETHUSDT,BUY,2200,0.5ETH,1100USDT,1.1USDT
2026-01-03 10:00:00,ETHUSDT,SELL,2500,1.2ETH,3000USDT,3USDT
```

**FIFO beklentisi:** lot1 maliyet `2002`, lot2 maliyet `1101.1`; net gelir `2997`;
eşleşen maliyet `2002 + 1101.1 × 0.4 = 2442.44`; **Realized P&L = `554.56`**;
kalan `0.3 ETH`, kalan maliyet `660.66`, avg cost `2202.20`.

**Average Cost beklentisi:** havuz `1.5 ETH / 3103.1 USDT`; satış maliyeti `2482.48`;
**Realized P&L = `514.52`**; kalan `0.3 ETH / 620.62`.

Ek zorunlu testler:
- (a) BUY fee'si base varlıkta → `1.0 ETH` executed, `0.001 ETH` fee → lot **`0.999 ETH`**
- (b) aynı saniyede 3 parçalı BUY + 1 SELL → sıra korunumu
- (c) oversell bayrağı
- (d) **invariant**: `toplam_maliyet(BUY) == eşleşen_maliyet + kalan_lot_maliyeti` (her senaryoda)
- (e) bozuk satır atlanması

Karşılaştırmalar `Decimal` ile **birebir eşitlik** (`assert_eq!`) — epsilon kullanma, Decimal deterministiktir.

---

## 9. Faz planı

| Faz | İçerik | Doğrulama |
|---|---|---|
| **0** | İskelet: Cargo.toml, tauri.conf.json, capabilities, SvelteKit+Tailwind+shadcn kurulumu, `models.rs`, `error.rs` | `npm run tauri dev` boş pencere açar |
| **1** | Rust çekirdek: parser, FIFO + Average Cost motorları, golden testler | `cargo test` yeşil · `cargo clippy -- -D warnings` temiz |
| **2** | Binance servisi + cache + tüm Tauri command'ları + hata katmanı | örnek `invoke` çağrıları |
| **3** | UI temeli: layout, sidebar, dosya yükleme, Dashboard, Portfolio, All Trades, i18n, tema | örnek CSV ile uçtan uca akış |
| **4** | Charts (ECharts ×4) + Settings | — |
| **5** | HTML rapor + PDF (`window.print()`) + CSV/JSON export + son oturum hatırlama | — |
| **6** | Windows build: `tauri icon`, NSIS/MSI, WebView2 / uzun path sorunları | kurulabilir paket |

Her fazın sonunda dur ve kullanıcının **"devam"** demesini bekle.

---

## 10. Çalışma tarzı (ajan davranışı)

- Kod her zaman **tam dosya** olarak verilir; her bloğun başında dosya yolu yorumu bulunur
  (ör. `// src-tauri/src/engine/fifo.rs`). **Diff/parça verme** — değişen dosyayı komple tekrar ver.
- Emin olunmayan API/sürüm detayında **varsayım açıkça belirtilir** ve çalışma anında nasıl
  doğrulanacağı söylenir.
- Sorun çıkarsa **en az bir alternatif çözüm** önerilir.
- Uzun işlemler (büyük CSV parse, rapor üretimi) UI'ı kilitlemez: async command + loading state.
- Kısayolculuk yok. `TODO` bırakılıyorsa faz numarasıyla etiketlenir: `// TODO(phase-3): …`

---

## 11. Komut referansı

```bash
npm install                      # frontend bağımlılıkları
npm run dev                      # sadece Vite (tarayıcıda, Tauri API'siz)
npm run tauri dev                # tam uygulama (dev)
npm run tauri build              # Windows NSIS + MSI paketi
npm run check                    # svelte-check (TS strict)

cd src-tauri
cargo test                       # golden testler dahil
cargo clippy -- -D warnings      # uyarı = hata
cargo fmt
```

---

## 12. Definition of Done

1. `cargo test` tüm testlerle geçiyor, golden değerler **birebir** tutuyor
2. `cargo clippy -- -D warnings` temiz
3. Örnek CSV yüklendiğinde Dashboard / Portfolio / Trades / Charts **tutarlı** rakamlar gösteriyor
4. İnternet kapalıyken uygulama çökmeden, stale fiyat göstergesiyle çalışıyor
5. FIFO ⇄ Average Cost geçişi rakamları anında ve doğru güncelliyor
6. TR/EN geçişi **tüm** metinleri kapsıyor
7. HTML rapor üretiliyor, PDF kaydetme akışı çalışıyor
8. `npm run tauri build` Windows'ta kurulabilir paket üretiyor

---

## 13. Python komut satırı

Masaüstü uygulamanın yanında repo kökünde çevrimdışı bir CLI vardır: `fiyatlio/` paketi, `python -m fiyatlio analyze`.

- Para hesabı `decimal.Decimal`. Float32 yok.
- CLI yorumları, yardım metni ve raporlar **Türkçe**. Bu paket için §2.7'deki İngilizce yorum kuralı geçerli değildir. Tauri / Svelte tarafı İngilizce yorumda kalır.
- Yöntemler: `fifo` (varsayılan), `lifo`, `avg`.
- Gerçek işlem CSV'si commitlenmez. `Binance-Spot-Trade-History-*.csv`, `out/` ve kişisel export'lar `.gitignore` altındadır.
- Yayınlanan örnek `samples/paxg_ornek.csv` kısa bir kesittir. Emir numaraları `100001`–`100006` ile değiştirilmiştir. Testler bu dosyadaki maliyet bandını (±0.001 PAXG / ±1 USDT) korur.
- CLI testi: repo kökünden `pytest`. Masaüstü testleri `src-tauri` altında `cargo test` olmaya devam eder.
