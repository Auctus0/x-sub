use image::{GrayImage, RgbaImage};

/// 1. Algoritma: Oyun Altyazilari Icin Renk ve Parlaklik Filtresi (Luminance & Color Mask)
///
/// Oyunlardaki hareketli, patlamali ve karisik arka planlari temizleyerek
/// sadece beyaz ve sari altyazi metinlerini izole eder.
/// Oyun goruntusu uzerindeki parlak altyazi metnini (Beyaz / Sari / Acik Gri)
/// karmasik oyun arka planindan (cim, patlama, gökyüzü, duvar dokusu) ayirip
/// yuksek kontrastli RGBA goruntuye donusturur.
/// Windows OCR motorunun arka plan dokularini Almanca/yabanci harf olarak halusinasyon
/// gormesini engeller.
pub fn isolate_subtitle_for_ocr(img: &RgbaImage) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let mut out = RgbaImage::new(w, h);

    for (x, y, pixel) in img.enumerate_pixels() {
        let r = pixel[0] as i32;
        let g = pixel[1] as i32;
        let b = pixel[2] as i32;

        let lum = (r * 299 + g * 587 + b * 114) / 1000;
        let max_c = r.max(g).max(b);
        let min_c = r.min(g).min(b);
        let sat_diff = max_c - min_c;

        // 1. Beyaz / Acik gri altyazi pikselleri (Yuksek parlaklik, dusuk renk doygunlugu)
        let is_white_sub = lum >= 160 && sat_diff <= 45;

        // 2. Sari / Altin sarisi altyazi pikselleri (Yuksek R ve G, dusuk B)
        let is_yellow_sub = r >= 165 && g >= 140 && b <= 135 && (r - b) >= 45;

        // 3. Acik mavi / Turkuaz konusmaci isimleri (azi oyunlarda)
        let is_cyan_sub = b >= 180 && g >= 170 && lum >= 165;

        if is_white_sub || is_yellow_sub || is_cyan_sub {
            // Metin pikselini saf beyaz yap
            out.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
        } else {
            // Arka plani saf siyah yap
            out.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
    }

    out
}

pub fn binarize_subtitle_image(img: &RgbaImage) -> GrayImage {
    let (w, h) = (img.width(), img.height());
    let mut gray = GrayImage::new(w, h);

    for (x, y, pixel) in img.enumerate_pixels() {
        let r = pixel[0] as u32;
        let g = pixel[1] as u32;
        let b = pixel[2] as u32;

        // Parlaklik (Luminance) hesabi: Y = 0.299R + 0.587G + 0.114B
        let lum = (r * 299 + g * 587 + b * 114) / 1000;

        // Kural 1: Beyaz ve acik gri metinler (Yuksek parlaklik ve dusuk doygunluk)
        let is_white =
            lum > 175 && (r as i32 - g as i32).abs() < 35 && (g as i32 - b as i32).abs() < 35;

        // Kural 2: Sari ve altin sarisi oyun altyazilari (Yuksek Kirmizi ve Yesil, dusuk Mavi)
        let is_yellow = r > 165 && g > 145 && b < 125;

        if is_white || is_yellow {
            gray.put_pixel(x, y, image::Luma([255])); // Metin pikseli (Beyaz)
        } else {
            gray.put_pixel(x, y, image::Luma([0])); // Arka plan (Siyah)
        }
    }

    gray
}

/// 2. Algoritma: Hizli Fark Hash'i (dHash - Difference Hashing)
///
/// Piksel piksel karsilastirmak yerine goruntuyu 9x8'e kucultup 64-bit gradyan ozetini cikarir.
/// Bu sayede arka plan hareket etse bile altyazi degismediyse Hamming uzakligi cok dusuk cikar.
/// Calisma suresi: < 0.05 ms (50 mikrosaniye)
pub fn compute_dhash(img: &RgbaImage) -> u64 {
    // 9 genislik, 8 yukseklik boyutuna bicubic/nearest olcekleme
    let target_w = 9u32;
    let target_h = 8u32;

    let src_w = img.width();
    let src_h = img.height();

    if src_w == 0 || src_h == 0 {
        return 0;
    }

    // 9x8 gri tonlamali ornek matris
    let mut matrix = [0u8; 9 * 8];

    for y in 0..target_h {
        let src_y = (y * src_h / target_h).min(src_h - 1);
        for x in 0..target_w {
            let src_x = (x * src_w / target_w).min(src_w - 1);
            let pixel = img.get_pixel(src_x, src_y);
            let lum =
                (pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000;
            matrix[(y * target_w + x) as usize] = lum as u8;
        }
    }

    // Soldan saga fark kontrolu ile 64-bit hash olustur
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let left = matrix[y * 9 + x];
            let right = matrix[y * 9 + (x + 1)];
            if left > right {
                hash |= 1 << (y * 8 + x);
            }
        }
    }

    hash
}

/// Iki dHash arasindaki Hamming mesafesini (farkli bit sayisini) dondurur (0 - 64 arasi).
#[inline(always)]
pub fn hamming_distance(h1: u64, h2: u64) -> u32 {
    (h1 ^ h2).count_ones()
}

/// 3. Algoritma: Hizli Levenshtein Benzerlik Orani (Fuzzy String Similarity)
///
/// Ardisik OCR okumalarindaki tek tuk karakter gurultulerini tespit eder (orn: "Hello!" vs "Hello!.").
/// Benzerlik 0.0 ile 1.0 arasinda bir oran dondurur.
pub fn calculate_text_similarity(s1: &str, s2: &str) -> f32 {
    let s1 = s1.trim();
    let s2 = s2.trim();

    if s1 == s2 {
        return 1.0;
    }
    if s1.is_empty() || s2.is_empty() {
        return 0.0;
    }

    let dist = levenshtein_distance(s1, s2);
    let max_len = s1.chars().count().max(s2.chars().count());

    1.0 - (dist as f32 / max_len as f32)
}

/// O(min(N, M)) bellek optimizasyonlu Levenshtein hesaplayici
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();

    let len1 = chars1.len();
    let len2 = chars2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row: Vec<usize> = vec![0; len2 + 1];

    for (i, &c1) in chars1.iter().enumerate() {
        curr_row[0] = i + 1;
        for j in 0..len2 {
            let cost = usize::from(c1 != chars2[j]);
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }
        prev_row.copy_from_slice(&curr_row);
    }

    prev_row[len2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_dhash_stability() {
        let mut img1 = RgbaImage::new(100, 50);
        let mut img2 = RgbaImage::new(100, 50);

        for x in 0..100 {
            for y in 0..50 {
                img1.put_pixel(x, y, Rgba([x as u8, y as u8, 100, 255]));
                // img2 cok hafif gurultulu ayni goruntu
                img2.put_pixel(x, y, Rgba([x as u8 + 2, y as u8 + 2, 102, 255]));
            }
        }

        let h1 = compute_dhash(&img1);
        let h2 = compute_dhash(&img2);
        let dist = hamming_distance(h1, h2);

        // Hafif gurultude dHash bit degisimi 0 veya cok kucuk olmalidir
        assert!(dist <= 3, "Hamming distance was {}", dist);
    }

    #[test]
    fn test_text_similarity_fuzzy() {
        // Tek karakterlik OCR gurultuleri
        let sim1 = calculate_text_similarity("Where are you going?", "Where are you going?..");
        assert!(sim1 > 0.85);

        let sim2 = calculate_text_similarity("PlayStation 5 Console", "PlayStation 5 Console");
        assert_eq!(sim2, 1.0);

        let sim3 = calculate_text_similarity("Completely different sentence", "Another story");
        assert!(sim3 < 0.3);
    }

    #[test]
    fn test_binarize_subtitle_image() {
        let mut img = RgbaImage::new(10, 10);
        // Beyaz metin pikseli
        img.put_pixel(2, 2, Rgba([255, 255, 255, 255]));
        // Sari metin pikseli
        img.put_pixel(3, 3, Rgba([255, 220, 20, 255]));
        // Koyu yesil zemin (agac/cimen)
        img.put_pixel(4, 4, Rgba([20, 120, 30, 255]));

        let binarized = binarize_subtitle_image(&img);

        assert_eq!(binarized.get_pixel(2, 2)[0], 255); // Beyaz metin gecti
        assert_eq!(binarized.get_pixel(3, 3)[0], 255); // Sari metin gecti
        assert_eq!(binarized.get_pixel(4, 4)[0], 0); // Yesil zemin elendi
    }
}
