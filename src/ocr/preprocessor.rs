use image::RgbaImage;

/// Iki goruntu arasindaki degisim oranini hesaplar (0.0 - 1.0 arasi).
/// Eger degisim esigin altindaysa gereksiz yere OCR calistirilmaz.
pub fn calculate_image_diff(prev: &RgbaImage, curr: &RgbaImage, threshold: f32) -> (bool, f32) {
    if prev.width() != curr.width() || prev.height() != curr.height() {
        return (true, 1.0);
    }

    let total_pixels = (prev.width() * prev.height()) as f32;
    if total_pixels == 0.0 {
        return (false, 0.0);
    }

    let prev_raw = prev.as_raw();
    let curr_raw = curr.as_raw();

    let mut diff_count = 0usize;
    let stride = 4 * 2; // Her 2 pikselde bir ornekleme (performans optimizasyonu)

    for i in (0..prev_raw.len()).step_by(stride) {
        if i + 3 < prev_raw.len() && i + 3 < curr_raw.len() {
            let dr = (prev_raw[i] as i32 - curr_raw[i] as i32).abs();
            let dg = (prev_raw[i + 1] as i32 - curr_raw[i + 1] as i32).abs();
            let db = (prev_raw[i + 2] as i32 - curr_raw[i + 2] as i32).abs();

            if dr > 30 || dg > 30 || db > 30 {
                diff_count += 1;
            }
        }
    }

    let sampled_pixels = total_pixels / 2.0;
    let diff_ratio = diff_count as f32 / sampled_pixels;

    (diff_ratio >= threshold, diff_ratio)
}

/// Windows OCR motorunun oyun arka plani parazitlerinden urettigi Almanca/Fransizca/Ispanyolca
/// aksanli harfleri (ä, ß, é, ñ, vb.) temiz Ingilizce ASCII karsiliklarina donusturur.
pub fn normalize_english_ocr_char(c: char) -> Option<char> {
    match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => Some(c),
        ' ' | '.' | ',' | '!' | '?' | '\'' | '"' | '-' | ':' | ';' | '(' | ')' | '/' => Some(c),
        '’' | '‘' | '`' | '´' => Some('\''),
        '“' | '”' | '«' | '»' => Some('"'),
        '—' | '–' => Some('-'),
        // Aksanli / umlaut harflerin Ingilizce ASCII karsiliklari (OCR halusinasyonu onleme)
        'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' | 'æ' => Some('a'),
        'Ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Å' | 'Æ' => Some('A'),
        'ë' | 'é' | 'è' | 'ê' => Some('e'),
        'Ë' | 'É' | 'È' | 'Ê' => Some('E'),
        'ï' | 'í' | 'ì' | 'î' => Some('i'),
        'Ï' | 'Í' | 'Ì' | 'Î' => Some('I'),
        'ö' | 'ó' | 'ò' | 'ô' | 'õ' | 'ø' => Some('o'),
        'Ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ø' => Some('O'),
        'ü' | 'ú' | 'ù' | 'û' => Some('u'),
        'Ü' | 'Ú' | 'Ù' | 'Û' => Some('U'),
        'ñ' => Some('n'),
        'Ñ' => Some('N'),
        'ç' => Some('c'),
        'Ç' => Some('C'),
        'ý' | 'ÿ' => Some('y'),
        'Ý' | 'Ÿ' => Some('Y'),
        'ß' => Some('B'), // OCR genelde B harfini ß olarak yanlis okur
        _ => None,
    }
}

/// OCR sonrasi elde edilen Ingilizce metni temizler, yabanci/bozuk karakterleri ve
/// kenar parazit kelimelerini ayiklar.
pub fn clean_ocr_text(raw_text: &str) -> String {
    // Eger metinde acikca Turkce'ye ozgu harfler varsa (ı, ş, ğ, İ), AntiGarbageFilter'in
    // bunu yakalayabilmesi icin oldugu gibi birak!
    if raw_text
        .chars()
        .any(|c| matches!(c, 'ı' | 'ş' | 'Ş' | 'ğ' | 'Ğ' | 'İ'))
    {
        return raw_text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }

    let mut cleaned_lines = Vec::new();

    for line in raw_text.lines() {
        // 1. Karakter bazinda Ingilizce ASCII normalizasyonu (Almanca ä, ö, ü, ß vb. temizlenir)
        let mut ascii_line = String::with_capacity(line.len());
        for ch in line.chars() {
            if ch == '…' {
                ascii_line.push_str("...");
            } else if let Some(mapped) = normalize_english_ocr_char(ch) {
                ascii_line.push(mapped);
            } else if ch.is_whitespace() {
                ascii_line.push(' ');
            }
        }

        let trimmed = ascii_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Cok kisa ve anlamsiz tek/iki karakterli parazit satirlari ele (ornegin '|', '.', '_a')
        if trimmed.len() <= 2 && !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }

        // 2. Kelime bazinda filtreleme: Oyun arka planindan (doku, agac, tabela, mini-harita)
        // yanlislikla okunan anlamsiz karakter dizilerini ("xqz", "1a#", vb.) ayikla
        let mut valid_words = Vec::new();
        for raw_word in trimmed.split_whitespace() {
            let core = raw_word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            if core.is_empty() {
                continue;
            }

            // Tek harfli kelimeler Ingilizce'de sadece "I", "a", "A" veya rakam olabilir.
            // OCR'in kenardan okudugu tekil "x", "z", "j", "q", "v" gibi parazitleri at!
            if core.len() == 1 {
                let ch = core.chars().next().unwrap();
                if !matches!(ch, 'I' | 'a' | 'A' | '0'..='9') {
                    continue;
                }
            }

            // Icinde hic sesli harf olmayan 4+ harfli sessiz harf yiginlarini ("strk", "bcdf") at
            let has_vowel = core
                .chars()
                .any(|c| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y'));
            let all_digits = core.chars().all(|c| c.is_ascii_digit());
            if core.len() >= 4 && !has_vowel && !all_digits {
                continue;
            }

            // Hem harf hem rakam karisik anlamsiz parazitleri ("1TA5", "a9b") ele
            // (Ancak "9am", "5th", "1st", "2nd", "3rd", "4k" gibi gercek kelimelere izin ver)
            let has_alpha = core.chars().any(|c| c.is_ascii_alphabetic());
            let has_digit = core.chars().any(|c| c.is_ascii_digit());
            if has_alpha && has_digit {
                let lower = core.to_ascii_lowercase();
                let is_valid_alnum = lower.ends_with("st")
                    || lower.ends_with("nd")
                    || lower.ends_with("rd")
                    || lower.ends_with("th")
                    || lower.ends_with("am")
                    || lower.ends_with("pm")
                    || lower.ends_with('k')
                    || lower.ends_with('m');
                if !is_valid_alnum {
                    continue;
                }
            }

            valid_words.push(raw_word);
        }

        if !valid_words.is_empty() {
            cleaned_lines.push(valid_words.join(" "));
        }
    }

    cleaned_lines.join(" ")
}

/// Ceviri motorlarindan (Google / DeepL) donen Turkce metindeki bozuk HTML karakterlerini,
/// sifir genislikli bosluklari (\u{200B}) ve yabanci parazitleri temizler.
pub fn sanitize_turkish_translation(text: &str) -> String {
    let mut s = text
        .replace('\u{200B}', "")
        .replace('\u{200C}', "")
        .replace('\u{200D}', "")
        .replace('\u{FEFF}', "")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    // Eger ceviri icinde Almanca/yabanci bozuk harfler kaldiysa Turkce/ASCII karsiligina cevir
    s = s
        .chars()
        .map(|c| match c {
            'ä' | 'á' | 'à' | 'â' => 'a',
            'Ä' | 'Á' | 'À' | 'Â' => 'A',
            'ë' | 'é' | 'è' | 'ê' => 'e',
            'Ë' | 'É' | 'È' | 'Ê' => 'E',
            'ß' => 's',
            _ => c,
        })
        .collect();

    // Fazla bosluklari temizle
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn test_clean_ocr_text() {
        let raw = "   | \nHello   World!  \n   _   \n  This is   a test. ";
        let cleaned = clean_ocr_text(raw);
        assert_eq!(cleaned, "Hello World! This is a test.");
    }

    #[test]
    fn test_clean_german_and_accented_ocr_glitches() {
        let glitchy = "Wäke üp, Sämurai! ßetween y z";
        let cleaned = clean_ocr_text(glitchy);
        assert_eq!(cleaned, "Wake up, Samurai! Between");
    }

    #[test]
    fn test_sanitize_turkish_translation() {
        let raw_tr = "Üzgün\u{200b} \u{200b}olma. &quot;Daha iyi ol.&quot;";
        let clean_tr = sanitize_turkish_translation(raw_tr);
        assert_eq!(clean_tr, "Üzgün olma. \"Daha iyi ol.\"");
    }

    #[test]
    fn test_calculate_image_diff_identical() {
        let img1 = RgbaImage::from_pixel(100, 50, Rgba([20, 20, 20, 255]));
        let img2 = RgbaImage::from_pixel(100, 50, Rgba([20, 20, 20, 255]));
        let (changed, diff) = calculate_image_diff(&img1, &img2, 0.05);
        assert!(!changed);
        assert_eq!(diff, 0.0);
    }

    #[test]
    fn test_calculate_image_diff_changed() {
        let img1 = RgbaImage::from_pixel(100, 50, Rgba([20, 20, 20, 255]));
        let img2 = RgbaImage::from_pixel(100, 50, Rgba([200, 200, 200, 255]));
        let (changed, diff) = calculate_image_diff(&img1, &img2, 0.05);
        assert!(changed);
        assert!(diff > 0.5);
    }
}
