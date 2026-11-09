use crate::config::AppConfig;
use crate::emulator::scenes::{get_standard_game_scenes, GameScreenRenderer};
use crate::ocr::{calculate_text_similarity, clean_ocr_text, AntiGarbageFilter, WinMediaOcr};
use crate::translation::TranslationPool;
use std::time::Instant;

/// Tum sahneler gercekten dogrulanirsa `true` doner (OCR benzerligi + ceviri sagligi).
pub async fn run_automated_emulator_test() -> bool {
    println!("================================================================================");
    println!("          x-sub - OYUN EMULATORU VE GERCEK ZAMANLI DOGRULAMA TESTI              ");
    println!("================================================================================");
    println!("Populer oyunlardan alinan farkli sahneler uzerinde uctan uca test yapiliyor:\n");

    let renderer = GameScreenRenderer::new();
    let scenes = get_standard_game_scenes();
    let mut config = AppConfig {
        fallback_to_google: true,
        ..Default::default()
    };

    let pool = TranslationPool::new();

    // OCR motorunu baslat
    let ocr = match WinMediaOcr::new("en") {
        Ok(engine) => engine,
        Err(err) => {
            println!("[HATA] Windows OCR motoru baslatilamadi: {}", err);
            return false;
        }
    };

    let mut passed_count = 0;
    let total_scenes = scenes.len();

    for (index, scene) in scenes.iter().enumerate() {
        println!(
            "--------------------------------------------------------------------------------"
        );
        println!(
            "[SAHNE {}/{}] Oyun: {} | Karakter: {}",
            index + 1,
            total_scenes,
            scene.game_name,
            scene.character
        );
        println!("   [Orijinal Metin]     : \"{}\"", scene.english_dialog);

        // 1. Kareyi Emulasyonda Uret
        let frame = renderer.render_scene(scene);

        // 2. OCR ile Oku
        let t_ocr = Instant::now();
        let raw_ocr = ocr
            .recognize_rgba(frame.as_raw(), frame.width(), frame.height())
            .unwrap_or_default();
        let ocr_ms = t_ocr.elapsed().as_millis();
        let cleaned_ocr = clean_ocr_text(&raw_ocr);

        println!(
            "   [OCR Algilanan]      : \"{}\" (Sure: {}ms)",
            cleaned_ocr, ocr_ms
        );

        // 3. Cop ve Halusinasyon Filtresi Kontrolu
        let (is_valid, reason) = AntiGarbageFilter::is_valid_subtitle(&cleaned_ocr);
        if !is_valid {
            println!(
                "   [FILTRE REDDETTI]    : Guvensiz/Cop metin tespit edildi: {}",
                reason
            );
            println!("   [DURUM]              : BASARISIZ [FAIL] (gecerli altyazi reddedildi)");
            continue;
        }

        // 4. Turkceye Cevir
        let t_trans = Instant::now();
        match pool.translate(&cleaned_ocr, &mut config).await {
            Ok(trans_res) => {
                let trans_ms = t_trans.elapsed().as_millis();
                println!(
                    "   [Turkce Ceviri]      : \"{}\"",
                    trans_res.translated_text
                );
                println!(
                    "   [Ceviri Motoru]      : {} (Sure: {}ms)",
                    trans_res.used_engine, trans_ms
                );
                println!("   [Beklenen Turkce Anlam]: \"{}\"", scene.expected_turkish);

                // Dogrulama: OCR orijinal metne yeterince yakin mi, ceviri saglikli mi?
                let similarity = calculate_text_similarity(&cleaned_ocr, scene.english_dialog);
                let translated = trans_res.translated_text.trim();
                let mut problems: Vec<String> = Vec::new();
                if similarity < 0.90 {
                    problems.push(format!("OCR benzerligi dusuk ({:.2} < 0.90)", similarity));
                }
                if translated.is_empty() {
                    problems.push("ceviri bos".to_string());
                }
                if translated.contains('\u{FFFD}') {
                    problems.push("ceviride bozuk karakter var".to_string());
                }
                if translated.eq_ignore_ascii_case(scene.english_dialog) {
                    problems.push("ceviri orijinalle ayni (cevrilmemis)".to_string());
                }
                println!("   [OCR Benzerlik]      : {:.2}", similarity);
                if problems.is_empty() {
                    println!("   [DURUM]              : BASARILI [OK]");
                    passed_count += 1;
                } else {
                    println!(
                        "   [DURUM]              : BASARISIZ [FAIL] ({})",
                        problems.join("; ")
                    );
                }
            }
            Err(err) => {
                println!("   [CEVIRI HATASI]      : {}", err);
                println!("   [DURUM]              : BASARISIZ [FAIL]");
            }
        }
    }

    println!("================================================================================");
    println!("                             TEST RAPORU SONUCU                                 ");
    println!("================================================================================");
    println!("Toplam Test Edilen Sahne: {}", total_scenes);
    println!(
        "Basarili Sahne Sayisi   : {} / {}",
        passed_count, total_scenes
    );
    if passed_count == total_scenes {
        println!("SONUC: TUM SAHNELER DOGRULANDI [OK]");
    } else {
        println!("SONUC: BAZI SAHNELER DOGRULANAMADI [FAIL]");
    }
    println!("================================================================================\n");
    passed_count == total_scenes
}
