# Fiyatlio

Binance spot trade geçmişi CSV'lerinden **FIFO** (varsayılan) veya **Average Cost** yöntemiyle
realized / unrealized P&L hesaplayan, offline-first masaüstü uygulaması.

> **API key gerekmez.** Yalnızca Binance'in public fiyat endpoint'i kullanılır.
> Trade geçmişiniz makinenizden dışarı çıkmaz.

| | |
|---|---|
| Backend | Rust + Tauri 2 |
| Frontend | SvelteKit 2 · Svelte 5 (runes) · TypeScript strict |
| UI | TailwindCSS 4 + shadcn-svelte, dark mode varsayılan |
| Grafik | Apache ECharts |
| Para birimi | `rust_decimal::Decimal` — **hiçbir yerde `f64` yok** |

Ajan talimatlarının tamamı: **[AGENTS.md](AGENTS.md)**

Aynı depoda masaüstü uygulamanın yanında çevrimdışı bir **Python komut satırı** da var.
Ağ çağrısı yok. Fiyat verilmezse açık kâr/zarar yazılmaz. Dönemler trader dilinde etiketlenir
(`ILK_ALIM`, `EK_ALIM`, `KISMI_SATIS`, `SIFIRLAMA`, `YENIDEN_ALIM`, `EK_SATIS`, `FAZLA_SATIS`).

### Komut satırı

Python 3.11+.

```bash
pip install -r requirements.txt
python -m fiyatlio analyze islemler.csv --method fifo --out out
```

Cüzdan ve anlık fiyat isteğe bağlıdır:

```bash
python -m fiyatlio analyze samples/paxg_ornek.csv --holdings PAXG=0.671466 --price PAXG=4279.78 --method fifo --out out
```

| Bayrak | Anlamı |
| --- | --- |
| `--method fifo` | `lifo` veya `avg`. Varsayılan `fifo` |
| `--holdings VARLIK=MIKTAR` | Cüzdan override. Örnek: `PAXG=0.671466,ETH=0.2` |
| `--price VARLIK=FIYAT` | Anlık fiyat. Yoksa açık K/Z hesaplanmaz |
| `--flat-pct` / `--flat-qty` | Sıfırlama eşiği. Varsayılan `0.005` ve `0.0001` |
| `--allow-missing-lots` | Eksik satışta hayali lot. Varsayılan: eksi bakiye, hayali lot yok |

Çıktı: `ozet.md`, `donemler.csv`, `filller.csv`, `kalan_lotlar.csv`, `varlik_ozeti.csv`, `rapor.html`, `rapor.xlsx`.

`samples/paxg_ornek.csv` gerçek bir exportun kısa kesitidir. Emir numaraları `100001`–`100006` yapılmıştır. Tam hesap geçmişi bu depoda yoktur. Kesit, bir kapanmış PAXG dönemi ve Mart 2026'daki açık çuvalı içerir. FIFO kalan kabaca 0,1928 PAXG @ 5150,14, 0,3617 @ 5110,80 ve 0,1169 @ 4275,38 olur. Toplam maliyet yaklaşık 3341,19 USDT. Cüzdan 0,671466 ve fiyat 4279,78 ile açık zarar yaklaşık −467 USDT'tir.

Test: `pytest`

---

## Neler var

- **İki maliyet yöntemi:** FIFO (varsayılan) ve Ortalama Maliyet. Ayarlardan geçiş tüm rakamları
  anında yeniden hesaplar. Her satış için hangi lottan ne kadar eşleştiği saklanır — rakam denetlenebilir.
- **Gerçekleşen / gerçekleşmemiş K/Z**, portföy değeri, kazanç oranı, komisyon dökümü.
- **Dört grafik** (ECharts): işlem zaman çizelgesi (zoom'lu scatter), kümülatif K/Z (step-line),
  parite bazında K/Z, portföy dağılımı. Tema ile senkron.
- **On binlerce satır** için sanallaştırılmış işlem tablosu; parite / yön / tarih aralığı / serbest
  metin filtreleri.
- **Offline-first:** internet yokken son bilinen fiyatlarla, "canlı değil" göstergesiyle çalışır.
- **Self-contained HTML rapor** + sistem yazdırma akışıyla PDF; trades ve realized event'ler için
  CSV/JSON export.
- **TR / EN** tam çeviri, koyu/açık/sistem teması.

---

## Kurulum

### 1. Ön koşullar (Windows)

```powershell
winget install --id Rustlang.Rustup -e
winget install --id OpenJS.NodeJS.LTS -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Kurulum sonrası **yeni bir terminal aç** (PATH yenilensin) ve doğrula:

```bash
rustc --version && cargo --version && node --version && npm --version
```

WebView2 Runtime Windows 10/11'de genelde hazır gelir; yoksa
[Microsoft WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) üzerinden kurulur.

### 2. Bağımlılıklar

```bash
npm install
```

### 3. shadcn-svelte bileşen kayıt defteri

```bash
npx shadcn-svelte@latest init
```

`components.json` zaten hazır — init sadece ikon paketini ve `$lib/components/ui` iskeletini ekler.
Sonrasında bileşenler tek tek çekilir:

```bash
npx shadcn-svelte@latest add button card table badge select tabs dialog tooltip
```

### 4. Uygulama ikonları

```bash
npm run tauri icon app-icon.png
```

`app-icon.png` deponun kökündeki 1024×1024 yer tutucudur; kendi logonuzla değiştirin.

---

## Geliştirme

```bash
npm run tauri dev
```

## Test ve lint

```bash
cd src-tauri && cargo test
cd src-tauri && cargo clippy -- -D warnings
npm run check
```

## Windows paketi

```bash
npm run tauri build
```

Çıktılar: `src-tauri/target/release/bundle/nsis/*.exe` ve `.../msi/*.msi`

### Build sorunları ve çözümleri

| Belirti | Sebep | Çözüm |
|---|---|---|
| `` `icons/icon.ico` not found; required for generating a Windows Resource file `` | İkonlar üretilmemiş | `npm run tauri icon app-icon.png`. İkonlar depoya commit'li olduğu için normalde bu adım gerekmez; yalnızca `src-tauri/icons/` silinmişse çıkar. |
| `link.exe not found` / `error: linker 'link.exe' not found` | MSVC build tools yok | `winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| Uygulama açılıyor ama pencere bembeyaz | WebView2 Runtime yok | [WebView2 Evergreen](https://developer.microsoft.com/microsoft-edge/webview2/) kurun. Bundle `downloadBootstrapper` modunda olduğu için kurulum sırasında otomatik de inebilir. |
| `The system cannot find the path specified` (derleme sırasında, derin `target/` yollarında) | Windows 260 karakter yol limiti | `git config --system core.longpaths true` **ve** `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled = 1`. Alternatif: projeyi `C:\src\fiyatlio` gibi kısa bir yola taşıyın. |
| `failed to run custom build command for tauri` | `tauri.conf.json` ile `frontendDist` uyuşmuyor | Önce `npm run build` çalıştırıp `build/` klasörünün oluştuğunu doğrulayın. |
| NSIS installer Türkçe/İngilizce dil seçmiyor | — | `tauri.conf.json` → `bundle.windows.nsis.languages` dizisinde tanımlı; ek dil eklenebilir. |
| Antivirüs `target\debug\deps\*.exe` dosyasını siliyor, `cargo test` exit 101 veriyor | **Yanlış pozitif.** Kaspersky'de `VHO:` ön eki sezgisel tespit demektir (imza eşleşmesi değil). Yeni derlenmiş, imzasız, sıkıştırılmış bir .exe'nin `deps` altında saniyeler içinde belirmesi bu heuristiğe takılıyor. | `src-tauri\target` klasörünü antivirüs dışlamalarına ekleyin. Bu klasörde yalnızca derleme çıktıları bulunur. |

### Kod imzalama

Kod imzalama yapılandırılmadığı için hem Windows SmartScreen ilk çalıştırmada uyarı gösterir hem de
bazı antivirüsler derleme çıktılarını sezgisel olarak işaretler. İmzalamak için `tauri.conf.json` →
`bundle.windows.certificateThumbprint` alanı kullanılır.


## CI / otomatik derleme

| Workflow | Tetikleyici | Yaptığı iş |
|---|---|---|
| [`ci.yml`](.github/workflows/ci.yml) | her push + PR | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (Windows) + `svelte-check` & frontend build (Linux) |
| [`build.yml`](.github/workflows/build.yml) | `main`'e push, `v*` tag, manuel | `tauri build` → NSIS `.exe` + `.msi` artifact olarak yüklenir; tag'de ayrıca **draft release** oluşturulur |

Sürüm yayınlamak için:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

> Windows runner dakikaları private repo'da **2× ücretlendirilir**. Bu yüzden pahalı olan
> `tauri build` her push'ta değil, yalnızca `main` / tag / manuel tetiklemede çalışır.
> İkonlar depoya commit'lidir — CI'da `tauri icon` adımına gerek yoktur.

---

## CSV formatı

Binance → Emirler → Spot → İşlem Geçmişi → Dışa Aktar.

Masaüstü uygulaması şu başlığı okur:

```
Time,Pair,Side,Price,Executed,Amount,Fee
2026-07-14 17:09:20,ETHUSDT,BUY,1864.77,0.2681ETH,499.944837USDT,0.0002681ETH
```

Komut satırı buna ek olarak güncel exportu da okur: `Order No`, `AOR Conversion Pair`, `AOR Conversion Rate`. Aynı emir numarası birden fazla fill olabilir. `USDCUSDT` gibi stable-stable çiftler çeviridir, PnL motoruna girmez.

Zaman damgaları sıralama için olduğu gibi kullanılır. Bozuk satır import'u durdurmaz.

Denemek için:

- [`samples/sample-trades.csv`](samples/sample-trades.csv) — masaüstü uygulamanın çok pariteli küçük örneği
- [`samples/paxg_ornek.csv`](samples/paxg_ornek.csv) — anonim PAXG kesiti (emir numaraları değiştirildi)
- [`samples/paxg_referans.csv`](samples/paxg_referans.csv) — aynı açık çuvalın yuvarlak, sentetik hali

Gerçek `Binance-Spot-Trade-History-*.csv` dosyalarını repoya koymayın.

## Bilinen sınırlar (v1)

- **BNB ile ödenen komisyonlar maliyet tabanına katılmaz.** Trade anındaki tarihsel BNB
  fiyatı elimizde olmadığı için doğru dönüşüm yapılamaz; bu komisyonlar varlık bazında ayrı
  toplanır ve dashboard'da güncel fiyatla "≈" ibaresiyle gösterilir.
- Transfer / convert / staking ile gelen bakiyeler CSV'de yer almaz. Bu bakiyelerin satışı
  **oversell** olarak işaretlenir ve aşan kısım sıfır maliyetle eşleştirilir (ayarlardan
  değiştirilebilir).
- USDT dışı quote'lu paritelerde P&L kendi quote varlığında hesaplanır; USDT'ye çevrilemeyen
  pariteler dashboard toplamına dahil edilmez ve ayrıca listelenir.
- **Realized P&L'in USDT'ye çevrimi bugünkü kurla yapılır.** 2021'de kazanılmış bir TRY kârı,
  bugünkü USDT/TRY kuruyla çevrildiğinde tarihsel gerçeği yansıtmaz. Doğrusu işlem anındaki kuru
  kullanmaktır (`/api/v3/klines`); v1'de yapılmıyor. Tek quote varlığı USDT olan hesaplarda bu sorun
  hiç ortaya çıkmaz.
- Export'un dosya adındaki saat dilimi (ör. `UTC+3`) okunmaz; zaman damgaları **UTC** kabul edilir.
  Hesaplamayı etkilemez (sıralama korunur), yalnızca gösterilen saatleri ve tarih filtrelerinin
  sınırlarını kaydırır.

## Lisans

MIT
