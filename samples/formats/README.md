# CSV biçim örnekleri

Satırlar uydurma. Emir numaraları, adresler ve tutarlar gerçek hesaba ait değil.
Başlıklar, borsanın kendi yardım sayfası veya dosyayı gerçekten okumuş bir içe aktarıcının yayımladığı kolon listesinden alındı. Masaüstü uygulaması bu başlıkları otomatik tanır. Aynı kolon kümesi iki biçime uyarsa dosyayı almadan önce sorar. Fiyatı olmayan hesap hareketleri (yatırma, Gate hesap dökümü, Bitget vadeli) tanınır ve işleme sokulmaz.

KuCoin kolon adındaki saat dilimi, export sırasında seçilen dilime göre değişir (`UTC+00:00`, `UTC+03:00`, …). Örnekte UTC yazıyor.

## Binance

| Dosya | Ne | Başlık kaynağı |
| --- | --- | --- |
| `../sample-trades.csv` | Spot işlem, klasik `Time` başlığı | Uygulamanın mevcut örneği |
| `binance-spot-trade-legacy.csv` | Aynı spot işlem, eski `Date(UTC)` başlığı | [vaultd Binance importer](https://github.com/Davincc77/vaultd/blob/main/vaultd/importers/binance.py) |
| `binance-spot-trade-order-no.csv` | Güncel spot Trade History (parçalı fill, AOR boş) | Kullanıcının 2026 exportu ve uygulama testi |
| `binance-spot-order-history.csv` | Spot emir dışa aktarımı (`Market`, `Fee Coin`) | [CryptoLinC Binance spot](https://support.cryptolinc.com/hc/ja/articles/8028369666703) |
| `binance-transaction-history.csv` | Cüzdan hareketi: alış iki satır (coin artar, USDT azalır) artı ücret | [Binance Developer Community](https://dev.binance.vision/t/transaction-history-through-api/21935), [rotki örneği](https://github.com/rotki/rotki/blob/develop/rotkehlchen/tests/data/binance_history.csv) |
| `binance-deposit-history.csv` | Yatırma / çekme | vaultd importer |
| `binance-usdm-futures-trades.csv` | USDⓈ-M vadeli işlem geçmişi | [CryptoLinC Binance futures](https://support.cryptolinc.com/hc/ja/articles/8028422071055) |

## Diğer borsalar

| Dosya | Ne | Başlık kaynağı |
| --- | --- | --- |
| `bybit-spot-trade-history.csv` | Bybit spot işlem | [CryptoLinC Bybit](https://support.cryptolinc.com/hc/ja/articles/8519343437455) |
| `kucoin-spot-filled-orders.csv` | KuCoin dolu spot emir | [CryptoLinC KuCoin](https://support.cryptolinc.com/hc/ja/articles/8812064760847) |
| `okx-order-history.csv` | OKX emir geçmişi | [OKX: download order history](https://www.okx.com/en-us/help/how-to-check-download-order-history-position-history-and-trading-history) |
| `okx-trading-history.csv` | OKX işlem / bakiye hareketi | Aynı OKX yardım sayfası |
| `coinbase-transaction-history.csv` | Coinbase işlem raporu | [J.S. Held](https://www.jsheld.com/uploads/Decrypting-Coinbase-Accounts-A-Guide-for-Forensic-Analysts.pdf), Coinbase rapor açıklamaları |
| `kraken-trades.csv` | Kraken gerçekleşen işlem (`trades.csv`) | [Kraken trades fields](https://support.kraken.com/articles/360001184886-how-to-interpret-trades-history-fields), [CryptoLinC Kraken](https://support.cryptolinc.com/hc/ja/articles/8029744460047) |
| `kraken-ledgers.csv` | Kraken defter: bir işlem iki satır, aynı `refid` | [Kraken ledger vs trades](https://support.kraken.com/articles/115000302707-differences-between-ledger-and-trades-history) |
| `gate-spot-trade-history.csv` | Gate spot işlem | [CryptoLinC Gate spot](https://support.cryptolinc.com/hc/ja/articles/8029904903311) |
| `gate-account-bill.csv` | Gate hesap hareketi | Aynı Gate sayfası (`unified_account`) |
| `bitget-spot-bills.csv` | Bitget spot hesap hareketi. Alış, USDT satırı ve coin satırı olarak ikiye bölünür | [CryptoLinC Bitget spot](https://support.cryptolinc.com/hc/ja/articles/8377513679247) |
| `bitget-usdt-m-futures.csv` | Bitget USDT-M vadeli | [CryptoLinC Bitget futures](https://support.cryptolinc.com/hc/ja/articles/8378208839695) |

MEXC için güvenilir, sabit bir CSV başlığı bulamadım. Uydurma kolon yazılmadı.

Kraken’in herkese açık bant verisi (`timestamp,price,volume,type,order_type,misc,trade_id`) hesap ekstresi değildir. Buraya konmadı.
