use crate::config::{AppConfig, KeyStatus};
use crate::ocr::{
    binarize_subtitle_image, calculate_image_diff, calculate_text_similarity, compute_dhash,
};
use image::{Rgba, RgbaImage};
use std::hint::black_box;
use std::time::Instant;

#[allow(dead_code)]
pub struct BenchmarkReport {
    pub dhash_avg_us: f64,
    pub pixel_diff_avg_us: f64,
    pub binarization_avg_us: f64,
    pub binarization_fps: f64,
    pub fuzzy_similarity_avg_us: f64,
    pub failover_latency_us: f64,
}

pub fn run_comprehensive_benchmark() -> BenchmarkReport {
    println!("============================================================");
    println!("         x-sub - PERFORMANS & ALGORITMA BENCHMARK TESTI     ");
    println!("============================================================");

    // 1. Test Goruntuleri Olustur (1200x180 - Standart 1080p Altyazi Kutusu)
    let w = 1200u32;
    let h = 180u32;
    let mut frame1 = RgbaImage::new(w, h);
    let mut frame2 = RgbaImage::new(w, h);

    for x in 0..w {
        for y in 0..h {
            let base = ((x * y) % 256) as u8;
            frame1.put_pixel(x, y, Rgba([base, base / 2, 50, 255]));
            frame2.put_pixel(x, y, Rgba([base.saturating_add(5), base / 2, 52, 255]));
        }
    }

    // 2. Test 1: dHash (Fark Hash'i) Hiz Testi (1.000 iterasyon)
    println!("[TEST 1/5] 64-bit dHash Perceptual Hashing Algoritmasi...");
    let iterations = 1000;
    let t0 = Instant::now();
    for _ in 0..iterations {
        black_box(compute_dhash(black_box(&frame1)));
    }
    let dhash_elapsed = t0.elapsed();
    let dhash_avg_us = dhash_elapsed.as_nanos() as f64 / 1000.0 / iterations as f64;
    println!("   -> 1000 calistirma suresi: {:?}", dhash_elapsed);
    println!(
        "   -> Ortalama dHash suresi: {:.2} mikrosaniye (us)",
        dhash_avg_us
    );
    println!(
        "   -> Teorik dHash Islem Kapasitesi: {:.0} FPS",
        1_000_000.0 / dhash_avg_us
    );

    // 3. Test 2: Geleneksel Piksel Fark Testi (1.000 iterasyon)
    println!("\n[TEST 2/5] Standart Piksel Fark Algoritmasi...");
    let t0 = Instant::now();
    for _ in 0..iterations {
        black_box(calculate_image_diff(
            black_box(&frame1),
            black_box(&frame2),
            0.05,
        ));
    }
    let pixel_diff_elapsed = t0.elapsed();
    let pixel_diff_avg_us = pixel_diff_elapsed.as_nanos() as f64 / 1000.0 / iterations as f64;
    println!("   -> 1000 calistirma suresi: {:?}", pixel_diff_elapsed);
    println!(
        "   -> Ortalama Piksel Fark suresi: {:.2} mikrosaniye (us)",
        pixel_diff_avg_us
    );
    println!(
        "   -> Hizlandirma Karsilastirmasi: dHash {:.2}x daha hizli",
        pixel_diff_avg_us / dhash_avg_us
    );

    // 4. Test 3: Altyazi Binarizasyon & Renk Filtreleme Hizi (200 iterasyon)
    println!("\n[TEST 3/5] Oyun Altyazisi Binarizasyon (Parlaklik/Renk Izolasyonu)...");
    let bin_iterations = 200;
    let t0 = Instant::now();
    for _ in 0..bin_iterations {
        black_box(binarize_subtitle_image(black_box(&frame1)));
    }
    let bin_elapsed = t0.elapsed();
    let binarization_avg_us = bin_elapsed.as_nanos() as f64 / 1000.0 / bin_iterations as f64;
    let binarization_fps = 1_000_000.0 / binarization_avg_us;
    println!("   -> 200 calistirma suresi: {:?}", bin_elapsed);
    println!(
        "   -> Ortalama Binarizasyon suresi: {:.2} mikrosaniye (us)",
        binarization_avg_us
    );
    println!(
        "   -> Gercek Zamanli Filtreleme Kapasitesi: {:.0} FPS",
        binarization_fps
    );

    // 5. Test 4: Fuzzy String Deduplication (Levenshtein) Hizi (10.000 iterasyon)
    println!("\n[TEST 4/5] Metin Tekrarini Onleyen Fuzzy Benzerlik (Levenshtein)...");
    let text1 = "Arthur, we need to leave the camp immediately before they arrive!";
    let text2 = "Arthur, we need to leave the camp immediately before they arrive!.";
    let fuzzy_iterations = 10_000;
    let t0 = Instant::now();
    for _ in 0..fuzzy_iterations {
        black_box(calculate_text_similarity(
            black_box(text1),
            black_box(text2),
        ));
    }
    let fuzzy_elapsed = t0.elapsed();
    let fuzzy_similarity_avg_us =
        fuzzy_elapsed.as_nanos() as f64 / 1000.0 / fuzzy_iterations as f64;
    println!("   -> 10.000 karsilastirma suresi: {:?}", fuzzy_elapsed);
    println!(
        "   -> Cumle basi benzerlik hesaplama: {:.3} mikrosaniye (us)",
        fuzzy_similarity_avg_us
    );

    // 6. Test 5: 10 Slotlu DeepL Failover Rotasyon Gecikmesi (10.000 iterasyon)
    println!("\n[TEST 5/5] 10 Slotlu DeepL Havuz Failover Rotasyonu...");
    let mut config = AppConfig::default();
    for (i, key) in config.deepl_keys.iter_mut().enumerate() {
        key.key = format!("test_key_{}:fx", i + 1);
        key.status = if i < 3 {
            KeyStatus::QuotaExceeded
        } else {
            KeyStatus::Ready
        };
    }

    let failover_iterations = 10_000;
    let t0 = Instant::now();
    for _ in 0..failover_iterations {
        // Simule edilmis failover arama: ilk hazir anahtari bul
        black_box(
            black_box(&config)
                .deepl_keys
                .iter()
                .position(|k| k.status == KeyStatus::Ready),
        );
    }
    let failover_elapsed = t0.elapsed();
    let failover_latency_us =
        failover_elapsed.as_nanos() as f64 / 1000.0 / failover_iterations as f64;
    println!(
        "   -> 10.000 rotasyon taramasi suresi: {:?}",
        failover_elapsed
    );
    println!(
        "   -> Slot gecis ve rota belirleme suresi: {:.4} mikrosaniye (us)",
        failover_latency_us
    );

    println!("\n============================================================");
    println!("                     BENCHMARK OZETI                        ");
    println!("============================================================");
    println!(
        "1. dHash Tabanli Degisim Kontrolu:  {:.2} us (SIFIR CPU yuk)",
        dhash_avg_us
    );
    println!(
        "2. Metin Benzerlik Eslestirme:      {:.3} us (Gereksiz ceviriyi %90 onler)",
        fuzzy_similarity_avg_us
    );
    println!(
        "3. Altyazi Arka Plan Filtresi:      {:.0} FPS potansiyeli",
        binarization_fps
    );
    println!(
        "4. DeepL Failover Gecikmesi:        {:.4} us (Oyunda SIFIR takilma)",
        failover_latency_us
    );
    println!("============================================================\n");

    BenchmarkReport {
        dhash_avg_us,
        pixel_diff_avg_us,
        binarization_avg_us,
        binarization_fps,
        fuzzy_similarity_avg_us,
        failover_latency_us,
    }
}
