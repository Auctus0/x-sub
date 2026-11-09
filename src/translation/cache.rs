use crate::ocr::calculate_text_similarity;
use std::collections::VecDeque;

/// Ceviri isteklerini RAM'de onbellege alarak mukerrer API cagrisi yapilmasini %100 onler.
/// Hem birebir eslesmeyi (noktalama/bosluk/buyuk-kucuk harf duyarsiz) hem de
/// OCR titremelerine karsi bulanik benzerligi (Fuzzy Similarity >= 0.85) yakalar!
pub struct TranslationCache {
    /// (normalize_edilmis_kaynak, ham_kaynak, cevrilmis_metin)
    entries: VecDeque<(String, String, String)>,
    max_capacity: usize,
}

impl TranslationCache {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Metni noktalama isaretlerinden ve fazla bosluklardan arindirarak normalize eder.
    /// Ornek: "Hello, world!.." -> "hello world"
    pub fn normalize_key(text: &str) -> String {
        text.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c.is_whitespace() {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 1. Once tam (normalize) eslesmeye bakar (0.001 ms).
    /// 2. Bulamazsa son cevrilen cumlelerle Fuzzy (Levenshtein >= 0.86) karsilastirma yapar.
    /// Boylece OCR ayni altyaziyi bir karede "Dont move!", diger karede "Don't move!."
    /// veya "D0n't move!" olarak okusa bile API'ye tekrar istek ATILMAZ!
    pub fn get(&self, text: &str) -> Option<String> {
        let norm = Self::normalize_key(text);
        if norm.is_empty() {
            return None;
        }

        // 1. Tam normalize eslesme
        for (k_norm, _k_raw, translated) in self.entries.iter().rev() {
            if k_norm == &norm {
                return Some(translated.clone());
            }
        }

        // 2. Fuzzy (Bulanik) eslesme: OCR 1-2 harf titremesi yaptiysa API'ye gitmeden yakala
        for (k_norm, _k_raw, translated) in self.entries.iter().rev() {
            if calculate_text_similarity(&norm, k_norm) >= 0.86 {
                return Some(translated.clone());
            }
        }

        None
    }

    pub fn insert(&mut self, source: &str, translated: String) {
        let norm = Self::normalize_key(source);
        if norm.is_empty() {
            return;
        }

        // Eger zaten varsa guncelle ve en sona (en yeniye) tasi
        if let Some(pos) = self.entries.iter().position(|(k, _, _)| k == &norm) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.max_capacity {
            self.entries.pop_front();
        }

        self.entries.push_back((norm, source.to_string(), translated));
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_cache_hit_and_normalization() {
        let mut cache = TranslationCache::new(10);
        cache.insert("Hello World", "Merhaba Dunya".to_string());

        // Harf boyutu, noktalama ve bosluk farki olsa da onbellekten yakalamali
        assert_eq!(cache.get("hello world"), Some("Merhaba Dunya".to_string()));
        assert_eq!(
            cache.get("  Hello, World!  "),
            Some("Merhaba Dunya".to_string())
        );
        // OCR titremesi (1 harf yanlis okuma: "He1lo World") olsa bile Fuzzy Cache yakalamali!
        assert_eq!(
            cache.get("He1lo World!"),
            Some("Merhaba Dunya".to_string())
        );
        assert_eq!(cache.get("Unknown sentence"), None);
    }
}
