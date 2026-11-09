# x-sub

Ekrandaki veya belirli bir uygulama penceresindeki (oyunlar, Capture Card / OBS Studio akislari, PS Remote Play, medya oynaticilar) Ingilizce altyazilari gercek zamanli yakalayip OCR ile okuyan, Turkceye ceviren ve ekranin uzerine seffaf katman (click-through overlay) olarak cizen Windows masaustu araci.

Tamamen Rust ile yazilmistir; harici bir OCR sunucusuna veya Python/Tesseract kurulumuna ihtiyac duymaz.

## Mimari ve Teknik Ozellikler

- **Sifir Harici Bagimlilikli OCR (`Windows.Media.Ocr`):** Windows 10 ve 11 icinde gomulu gelen WinRT OCR motorunu dogrudan kullanir.
- **64-bit dHash Degisim Algilama:** Yakalanan altyazi bolgesi (ROI) uzerinde her karede `0.10 us` altinda algisal hash (dHash) ve piksel fark analizi calistirir. Ekrandaki metin degismedigi surece OCR ve ceviri katmanini tetiklemez.
- **Goruntu On-Isleme (CLAHE + Otsu Binarization):** Oyun sahnelerindeki dusuk kontrastli veya parlak arka planli altyazilari OCR oncesinde izole eder.
- **Halusinasyon ve Cop Metin Filtresi (Anti-Garbage Filter):** Ekranda altyazi yokken oyun arayuzu, nisan imleci veya masaustu ogelerinden kaynaklanan sembol dizilerini ve tekrarlayan kelime dongulerini (`a-z` harf orani, ardisik tekrar ve kelime uzunluk kurallariyla) eler.
- **10 Slotlu DeepL API Havuzu ve Otomatik Failover:** 10 adede kadar DeepL API anahtari tanimlanabilir. Aktif anahtar kota sinirina (`HTTP 456`), yetki hatasina (`HTTP 403`) veya hiz limitine (`HTTP 429`) takildiginda bekleme suresi (cooldown) atayarak kesintisiz bir sonraki anahtara gecer.
- **Google Translate Yedeklemesi ve LRU Onbellek:** DeepL anahtarlari tukendiginde veya kullanici tercih ettiginde Google Translate motoruna gecer. Tekrar eden cumleler bellek ici LRU onbellekten (`0 ms`) doner.
- **DWM Seffaf Katman ve `WDA_EXCLUDEFROMCAPTURE`:** Altyazi katmani Windows DWM uzerinden tam seffaf cizilir, `MousePassthrough` ile fare tiklamalarini arkadaki oyuna gecirir ve `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` sayesinde kendi ekran yakalama dongusune girmez.

## Gereksinimler

- Windows 10 (1809+) veya Windows 11
- Windows dil ayarlarinda kaynak dil (varsayilan: Ingilizce `en-US`) OCR paketinin yuklu olmasi
- Derleme icin: Rust 1.75+ (MSVC toolchain)

## Derleme ve Calistirma

```powershell
cargo build --release
```

Derlenen tekil calistirilabilir dosya `target\release\x-sub.exe` konumunda olusur.

### Normal Arayuz Modu

```powershell
.\target\release\x-sub.exe
```

### Komut Satiri Test ve Benchmark Modlari

Gercek zamanli altyazi emulasyonu ve OCR/ceviri dogrulama testi:
```powershell
.\target\release\x-sub.exe --test-emulator
```

Gercek Windows penceresi uzerinden canli DWM yakalama saha testi:
```powershell
.\target\release\x-sub.exe --live-field-test
```

Algoritma performans testi (dHash, piksel farki, Otsu binarization, Levenshtein benzerlik, havuz failover suresi):
```powershell
.\target\release\x-sub.exe --benchmark
```

## Kullanim

1. **Kaynak Secimi:** `PANEL` sekmesinden yakalanacak monitoru veya spesifik uygulama penceresini (ornegin OBS Studio, PS Remote Play veya oyun penceresi) secin.
2. **Altyazi Bolgesi (ROI):** `Altyazi Alanini Sec` butonuna tiklayarak ekranda altyazilarin ciktigi dikdortgen bolgeyi fare ile cizin.
3. **Ceviri Motoru:** `DEEPL (10)` sekmesinden varsa DeepL API anahtarlarinizi ekleyin veya dogrudan `Google Translate` modunu secin.
4. **Baslatma:** Ust bardaki `BASLAT` butonuna basin.

> Not: Kullanici ayarlari ve API anahtarlari yerel olarak calisma dizinindeki `config.json` dosyasina kaydedilir ve versiyon kontrolune dahil edilmez.

## Proje Yapisi

```text
src/
├── main.rs                 # Giris noktasi ve CLI arguman yonlendiricisi
├── app.rs                  # Ana egui durumu, viewport yonetimi ve arka plan isci (worker) dongusu
├── config.rs               # Yapilandirma semasi (config.json serilestirme)
├── theme.rs                # Arayuz renk ve stil tanimlari
├── benchmark.rs            # Görüntü isleme ve failover hiz testleri
├── capture/
│   └── screen.rs           # xcap tabanli monitor/pencere yakalama ve DPI olcekleme
├── ocr/
│   ├── win_media_ocr.rs    # Windows.Media.Ocr WinRT baglantisi
│   ├── preprocessor.rs     # Kontrast germe, Otsu esikleme ve metin temizleme
│   ├── algorithms.rs       # 64-bit dHash, piksel farki ve Levenshtein benzerlik hesaplari
│   └── anti_garbage.rs     # OCR halusinasyon ve gurultu filtreleme kurallari
├── translation/
│   ├── pool.rs             # 10 slotlu anahtar rotasyonu, cooldown ve failover yonetimi
│   ├── deepl.rs            # DeepL REST API istemcisi
│   ├── google.rs           # Google Translate istemcisi
│   └── cache.rs            # Bellek ici ceviri onbellegi
├── ui/
│   ├── control_window.rs   # Ana kontrol paneli sekmeleri
│   ├── roi_selector.rs     # Ekranda interaktif bolge secim katmani
│   ├── subtitle_overlay.rs # Seffaf, tiklama gecirgen altyazi penceresi
│   └── key_manager_view.rs # DeepL anahtar havuzu arayuzu
└── emulator/               # Otonom testler icin sentetik oyun sahnesi ureticisi
```

## Lisans

MIT
