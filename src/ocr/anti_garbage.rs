/// Cop metinleri, OCR gurultu dongulerini ve halusinasyonlari tespit eden filtre.
/// Ekranda oyun altyazisi yerine yanlislikla tarayici, masaustu veya sistem
/// yazilari okundugunda olusan tekrarlari ("Isletme Isletme Isletme...") ve
/// sembol corbalarini ("io•.49 / 1TA5 »zi") eler.
pub struct AntiGarbageFilter;

impl AntiGarbageFilter {
    /// "00:04", "8:21", "9.11.2026", "3.19" gibi saat/tarih/sayac benzeri belirteci mi?
    fn is_clock_or_date(token: &str) -> bool {
        let t = token.trim_matches(|c: char| !c.is_ascii_digit());
        if t.is_empty()
            || !t
                .chars()
                .all(|c| c.is_ascii_digit() || c == ':' || c == '.' || c == '/')
        {
            return false;
        }
        let groups: Vec<&str> = t.split([':', '.', '/']).collect();
        groups.len() >= 2 && groups.iter().all(|g| !g.is_empty() && g.len() <= 4)
    }

    /// Metnin cevrilmeye deger gecerli bir altyazi olup olmadigini dogrular.
    /// Gecersizse (cop/gurultu ise) `false` dondurur.
    pub fn is_valid_subtitle(text: &str) -> (bool, &'static str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return (false, "Bos metin");
        }

        // Kural 1: Minimum mantikli uzunluk (en az 3 karakter)
        if trimmed.chars().count() < 3 {
            return (false, "Cok kisa metin");
        }

        // Kural 1b: En az 2 harf iceren gercek bir kelime olmali ("123", "a1", "--" gibi parazitleri ele)
        let has_real_word = trimmed
            .split_whitespace()
            .any(|w| w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2);
        if !has_real_word {
            return (false, "Gecerli kelime bulunamadi");
        }

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.is_empty() {
            return (false, "Kelime bulunamadi");
        }

        // Kural 2: Kelime Tekrari Dongusu (Loop Detection)
        // Kelimeler noktalama ve buyuk/kucuk harften arindirilarak karsilastirilir.
        // GTA tarzi "No, no, no!" / "Run! Run! Run!" / "Ha ha ha" gibi gercek
        // unlem tekrarlari gecerlidir; OCR dongusu ("Isletme Isletme Isletme...")
        // ise noktalamasiz ve uzundur.
        let normalized: Vec<String> = words
            .iter()
            .map(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
                    // "İ".to_lowercase() = "i" + birlestirici nokta (U+0307); nokta atilir
                    .replace('\u{307}', "")
            })
            .collect();

        let mut max_consecutive_repeat = 1;
        let mut current_repeat = 1;
        let mut repeat_has_punct = false;
        let mut loop_has_punct = false;
        for i in 1..words.len() {
            if !normalized[i].is_empty() && normalized[i] == normalized[i - 1] {
                current_repeat += 1;
                if words[i - 1].ends_with(|c: char| !c.is_alphanumeric()) {
                    repeat_has_punct = true;
                }
                if current_repeat > max_consecutive_repeat {
                    max_consecutive_repeat = current_repeat;
                    loop_has_punct = repeat_has_punct;
                }
            } else {
                current_repeat = 1;
                repeat_has_punct = false;
            }
        }
        let loop_limit = if loop_has_punct { 6 } else { 4 };
        if max_consecutive_repeat >= loop_limit {
            return (false, "Tekrarlayan kelime dongusu tespit edildi");
        }

        // Uzun metinde tek bir kelime baskinsa (>= %60) cop kabul et
        if words.len() >= 8 {
            let mut word_counts = std::collections::HashMap::new();
            for w in &normalized {
                if !w.is_empty() {
                    *word_counts.entry(w.as_str()).or_insert(0usize) += 1;
                }
            }
            for (_w, count) in word_counts {
                if count as f32 / words.len() as f32 >= 0.6 {
                    return (false, "Asiri baskin kelime tekrari");
                }
            }
        }
        // Kural 2b: Masaustu/gorev cubugu/tarayici parazitleri
        // Saat ("00:04"), tarih ("9.11.2026"), "8:21 / 821" gibi sayac ve "/" dizileri
        // oyun altyazisi degildir.
        let digit_tokens = words
            .iter()
            .filter(|w| w.chars().any(|c| c.is_ascii_digit()))
            .count();
        let clock_like = words.iter().filter(|w| Self::is_clock_or_date(w)).count();
        let slash_count = trimmed.matches('/').count();
        if clock_like >= 1 && digit_tokens >= 2 {
            return (false, "Saat/tarih iceren masaustu yazisi");
        }
        if digit_tokens >= 3 && digit_tokens * 4 >= words.len() {
            return (false, "Cok fazla sayi iceren parazit metin");
        }
        if slash_count >= 2 && digit_tokens >= 1 {
            return (false, "Sayac/ayirici dizisi (masaustu paraziti)");
        }

        // Kaynak dil Ingilizce: ingilizce altyazida bulunmayan Turkce harfler (ı, ş, ğ, İ)
        // ekranda zaten Turkce bir yazi (tarayici, gorev cubugu, ceviri katmani) oldugunu gosterir.
        // Katmanin kendi ciktisini tekrar okuyup cevirmesini de engeller.
        if trimmed
            .chars()
            .any(|c| matches!(c, 'ı' | 'ş' | 'Ş' | 'ğ' | 'Ğ' | 'İ'))
        {
            return (false, "Zaten Turkce metin (kaynak dil Ingilizce)");
        }

        // Kural 3: Sembol ve Garip Karakter Orani
        // Ornek: "ıo•.49 / ıo•.44/1 TA5 »zı" gibi parazitler
        // Bosluk ve normal noktalama ("No, no, no!") orana dahil edilmez; aksi halde
        // kisa replikler haksiz yere "sembol paraziti" sayilir.
        let total_chars = trimmed
            .chars()
            .filter(|c| {
                !c.is_whitespace()
                    && !matches!(
                        c,
                        '.' | ','
                            | '!'
                            | '?'
                            | '\''
                            | '"'
                            | '-'
                            | ':'
                            | ';'
                            | '’'
                            | '“'
                            | '”'
                            | '…'
                    )
            })
            .count();
        let mut alphanumeric_chars = 0;
        let mut suspicious_symbols = 0;

        for c in trimmed.chars() {
            if c.is_alphabetic() || c.is_numeric() {
                alphanumeric_chars += 1;
            } else if matches!(c, '•' | '»' | '«' | '§' | '©' | '®' | '™' | '`' | '~' | '¶')
            {
                suspicious_symbols += 1;
            }
        }

        if total_chars > 0 {
            let alpha_ratio = alphanumeric_chars as f32 / total_chars as f32;
            if alpha_ratio < 0.60 {
                return (false, "Dusuk harf/sayi orani (asiri sembol paraziti)");
            }
            if suspicious_symbols >= 2 {
                return (false, "Supheli OCR parazit karakterleri");
            }
        }

        (true, "Gecerli altyazi")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_repetitive_garbage() {
        // Kullanicinin ekran goruntusundeki gercek ornek
        let garbage = "İşletme İşletme İşletme İşletme İşletme Bu sistem 46 dakika önce";
        let (valid, reason) = AntiGarbageFilter::is_valid_subtitle(garbage);
        assert!(!valid);
        assert_eq!(reason, "Tekrarlayan kelime dongusu tespit edildi");
    }

    #[test]
    fn test_filter_symbol_soup() {
        let garbage = "ıo•.49 / ıo•.44/1 TA5 ıo•.44/1 »zı";
        let (valid, _) = AntiGarbageFilter::is_valid_subtitle(garbage);
        assert!(!valid);
    }

    #[test]
    fn test_allow_real_game_subtitles() {
        let valid_sub1 = "We need to get the women and children to safety immediately.";
        let (valid1, _) = AntiGarbageFilter::is_valid_subtitle(valid_sub1);
        assert!(valid1);

        let valid_sub2 = "Do not be sorry. Be better.";
        let (valid2, _) = AntiGarbageFilter::is_valid_subtitle(valid_sub2);
        assert!(valid2);
    }

    #[test]
    fn test_allow_repeated_interjections() {
        for s in [
            "No, no, no!",
            "Run! Run! Run!",
            "Ha ha ha",
            "Go, go, go, go! Get in the car!",
        ] {
            let (valid, reason) = AntiGarbageFilter::is_valid_subtitle(s);
            assert!(valid, "'{}' reddedildi: {}", s, reason);
        }
    }

    #[test]
    fn test_reject_ocr_word_loops() {
        for s in [
            "Isletme Isletme Isletme Isletme Isletme",
            "İşletme İŞLETME işletme İşletme",
            "bar bar bar bar bar bar bar bar bar x",
        ] {
            let (valid, _) = AntiGarbageFilter::is_valid_subtitle(s);
            assert!(!valid, "'{}' kabul edildi", s);
        }
    }

    #[test]
    fn test_reject_desktop_clock_and_counters() {
        for s in [
            "9 a.ıo/8:21 » Aynı zamanda Sözen de küçümsüyor 3:1 5/8:21 / 821 doğru 3.19 / 8:21 00:04 9.11.2026",
            "Show desktop 00:04 9.11.2026",
            "5/8:21 / 821 3.19 / 8:21",
            "Aynı zamanda Sözen de küçümsüyor",
        ] {
            let (valid, _) = AntiGarbageFilter::is_valid_subtitle(s);
            assert!(!valid, "'{}' kabul edildi", s);
        }
    }

    #[test]
    fn test_allow_subtitles_with_single_number() {
        for s in [
            "It's 3:30 in the morning, Lucia.",
            "We have 5 minutes. Move!",
            "Meet me at pier 9.",
        ] {
            let (valid, reason) = AntiGarbageFilter::is_valid_subtitle(s);
            assert!(valid, "'{}' reddedildi: {}", s, reason);
        }
    }
}
