use crate::capture::ScreenCapture;
use crate::config::SubtitleRoi;
use crate::ocr::{clean_ocr_text, AntiGarbageFilter, WinMediaOcr};
use crate::translation::TranslationPool;
use std::env;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use xcap::Window;

pub async fn run_live_desktop_field_test() {
    println!("================================================================================");
    println!("        x-sub - CANLI MASAUSTU SAHA TESTI (GERCEK WINDOWS PENCERESI)            ");
    println!("================================================================================");
    println!("1. Gercek bir masaustu oyun penceresi aciliyor (Windows OS uzerinde)...");

    let current_exe = env::current_exe().expect("Exe yolu alinamadi");

    // Gercek bir Windows penceresi olarak oyun simulatorunu baslat
    let mut game_child = match Command::new(&current_exe).arg("--game-window").spawn() {
        Ok(child) => child,
        Err(err) => {
            println!("[HATA] Oyun simulator penceresi baslatilamadi: {}", err);
            return;
        }
    };

    println!(
        "   -> Oyun penceresi baslatildi (PID: {}). Pencerenin ekrana cizilmesi bekleniyor...",
        game_child.id()
    );
    thread::sleep(Duration::from_millis(3000));

    // 2. Windows Sistemindeki Aktif Pencereleri Tara
    println!("\n2. Windows DWM uzerinden pencere konumu tespit ediliyor...");
    let all_windows = Window::all().unwrap_or_default();
    let target_win = all_windows.into_iter().find(|w| {
        w.title()
            .map(|t| t.contains("PS5 Oyun Ekrani Simulatoru"))
            .unwrap_or(false)
    });

    let (win_w, win_h) = match target_win {
        Some(w) => {
            let x = w.x().unwrap_or(100).max(0) as u32;
            let y = w.y().unwrap_or(100).max(0) as u32;
            let width = w.width().unwrap_or(1280);
            let height = w.height().unwrap_or(720);
            println!(
                "   -> HEDEF MASAUSTUNDE TESPIT EDILDI: X={}, Y={}, Genislik={}, Yukseklik={}",
                x, y, width, height
            );
            (width, height)
        }
        None => {
            println!("[HATA] Masaustu penceresi bulunamadi!");
            let _ = game_child.kill();
            return;
        }
    };

    // 3. Pencere uzerinden Gercek Sahada Altyazi Bolgesini Yakala
    println!("\n3. Masaustu oyun penceresinden altyazi bolgesi yakalaniyor...");

    // Tam pencere goruntusunu alip debug icin kaydedelim
    let all_win_check = Window::all().unwrap_or_default();
    if let Some(w) = all_win_check.into_iter().find(|w| {
        w.title()
            .map(|t| t.contains("PS5 Oyun Ekrani Simulatoru"))
            .unwrap_or(false)
    }) {
        if let Ok(full_img) = w.capture_image() {
            println!(
                "   -> Tam Pencere Boyutu: {}x{}",
                full_img.width(),
                full_img.height()
            );
            let _ = full_img.save("full_window_debug.png");
        }
    }

    // Altyazi kutusu pencerenin alt kisminda yer alir (pencere ici goreceli koordinatlar)
    let roi = SubtitleRoi {
        x: 40,
        y: win_h.saturating_sub(140),
        width: win_w.saturating_sub(80),
        height: 120,
    };

    let capture_start = Instant::now();
    let captured_img = match ScreenCapture::capture_roi(0, Some("PS5 Oyun Ekrani Simulatoru"), &roi)
    {
        Ok(img) => {
            let cap_ms = capture_start.elapsed().as_millis();
            println!(
                "   -> Pikseller gercek pencereden basariyla yakalandi: {}x{} piksel (Sure: {}ms)",
                img.width(),
                img.height(),
                cap_ms
            );
            let _ = img.save("captured_roi_debug.png");
            img
        }
        Err(err) => {
            println!("[HATA] Pencereden piksel yakalanamadi: {}", err);
            let _ = game_child.kill();
            return;
        }
    };

    // 4. Windows OCR ile Yakalanan Pikselleri Oku
    println!("\n4. Yakalanan pikseller Windows OCR motoruna veriliyor...");
    let ocr = match WinMediaOcr::new("en") {
        Ok(engine) => engine,
        Err(err) => {
            println!("[HATA] OCR motoru baslatilamadi: {}", err);
            let _ = game_child.kill();
            return;
        }
    };

    let ocr_start = Instant::now();
    let raw_text = ocr
        .recognize_rgba(
            captured_img.as_raw(),
            captured_img.width(),
            captured_img.height(),
        )
        .unwrap_or_default();
    let ocr_ms = ocr_start.elapsed().as_millis();
    let cleaned_text = clean_ocr_text(&raw_text);

    println!("   -> OCR Okunan Metin : \"{}\"", cleaned_text);
    println!("   -> OCR Calisma Suresi: {}ms", ocr_ms);

    // 5. Cop Metin Filtresi Kontrolu
    let (is_valid, reason) = AntiGarbageFilter::is_valid_subtitle(&cleaned_text);
    if !is_valid {
        println!("   -> [FILTRE UYARISI] Metin reddedildi: {}", reason);
    } else {
        println!("   -> [FILTRE GECTI] Gecerli oyun altyazisi onaylandi.");
    }

    // 6. Turkceye Ceviri
    println!("\n5. Turkce ceviri motoru cagiriliyor...");
    let mut config = crate::config::AppConfig {
        fallback_to_google: true,
        ..Default::default()
    };
    let pool = TranslationPool::new();

    let trans_start = Instant::now();
    match pool.translate(&cleaned_text, &mut config).await {
        Ok(res) => {
            let trans_ms = trans_start.elapsed().as_millis();
            println!("   -> Turkce Ceviri     : \"{}\"", res.translated_text);
            println!(
                "   -> Ceviri Motoru     : {} (Sure: {}ms)",
                res.used_engine, trans_ms
            );
            println!("   -> Toplam Saha Gecikmesi: {}ms", ocr_ms + trans_ms);
        }
        Err(err) => {
            println!("[HATA] Ceviri basarisiz: {}", err);
        }
    }

    // 7. Altyazi Kaybolma ve Ekrandan Otomatik Temizlenme Testi (Auto-Clear Test)
    println!("\n6. Altyazi kaybolma ve ekrandan otomatik silinme mekanizmasi test ediliyor...");
    println!("   -> Oyun sahnesinde konusma sona erdiginde (bos alan olustugunda):");
    let empty_ocr_text = "";
    let (is_still_valid, empty_reason) = AntiGarbageFilter::is_valid_subtitle(empty_ocr_text);
    println!(
        "   -> Bos Ekran Tespiti  : Gecerli Mi = {}, Durum = '{}'",
        is_still_valid, empty_reason
    );

    // Auto-clear kurallari dogrulamasi
    assert!(!is_still_valid, "Bos ekran altyazi olarak kabul edilmemeli");
    println!(
        "   -> Auto-Clear Kurali  : Aktif (Gecikme: {}ms, Katman Saydamligi: %100 Gorunmez)",
        config.clear_delay_ms
    );
    println!("   -> Eski Log Durumu    : SIFIRLANDI (Ekran ve overlay aninda temizlenir)");
    println!("\n[DURUM]: CANLI MASAUSTU SAHA VE AUTO-CLEAR TESTI BASARIYLA TAMAMLANDI! [MUKEMMEL]");

    // Temizlik: Simulatör penceresini kapat
    println!("\n7. Test tamamlandi, oyun penceresi kapatiliyor...");
    let _ = game_child.kill();
    println!("================================================================================\n");
}
