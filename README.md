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

Binance → Orders → Spot Order → Trade History → Export

```
Time,Pair,Side,Price,Executed,Amount,Fee
2026-07-14 17:09:20,ETHUSDT,BUY,1864.77,0.2681ETH,499.944837USDT,0.0002681ETH
```

Zaman damgaları **UTC** kabul edilir, arayüzde yerel saate çevrilir.
Bozuk satırlar import'u durdurmaz; atlanır ve satır numarasıyla listelenir.

## Bilinen sınırlar (v1)

- **BNB ile ödenen komisyonlar maliyet tabanına katılmaz.** Trade anındaki tarihsel BNB
  fiyatı elimizde olmadığı için doğru dönüşüm yapılamaz; bu komisyonlar varlık bazında ayrı
  toplanır ve dashboard'da güncel fiyatla "≈" ibaresiyle gösterilir.
- Transfer / convert / staking ile gelen bakiyeler CSV'de yer almaz. Bu bakiyelerin satışı
  **oversell** olarak işaretlenir ve aşan kısım sıfır maliyetle eşleştirilir (ayarlardan
  değiştirilebilir).
- USDT dışı quote'lu paritelerde P&L kendi quote varlığında hesaplanır; USDT'ye çevrilemeyen
  pariteler dashboard toplamına dahil edilmez ve ayrıca listelenir.

## Lisans

MIT
