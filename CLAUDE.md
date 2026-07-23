# CLAUDE.md

Bu projenin tüm ajan talimatları tek bir dosyada tutulur: **[AGENTS.md](AGENTS.md)**.

Çalışmaya başlamadan önce `AGENTS.md` dosyasının tamamını oku. Talimat değişiklikleri
**yalnızca** o dosyada yapılır; burada içerik çoğaltma.

Hızlı hatırlatma (detaylar AGENTS.md'de):

- Parasal hesaplarda `f64` **yasak** — `rust_decimal::Decimal` kullan, frontend'e string serialize et.
- Production kodunda `unwrap()` / `expect()` / `panic!()` **yok** — `Result<T, AppError>`.
- Hesaplama UTC; yerel saat sadece görüntülemede.
- HTTP yalnızca Rust tarafında (`reqwest`); frontend sadece `invoke()`.
- Kod yorumları İngilizce, UI metinleri i18n'de TR + EN.
- Proje klasörü dışına dosya yazma.
- Kod tam dosya olarak verilir, diff olarak değil.
- Her fazın sonunda dur, kullanıcının "devam" demesini bekle.
