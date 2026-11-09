use crate::config::{AppConfig, KeyStatus, PreferredEngine};
use crate::ocr::sanitize_turkish_translation;
use crate::translation::cache::TranslationCache;
use crate::translation::deepl::{DeepLClient, DeepLError};
use crate::translation::google::GoogleTranslateClient;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Kota dolan anahtar bu sure sonra tekrar denenir (DeepL kotasi aylik sifirlanir)
const QUOTA_COOLDOWN: Duration = Duration::from_secs(30 * 60);
/// Yetkisiz anahtar bu sure sonra (veya anahtar metni degisince hemen) tekrar denenir
const AUTH_COOLDOWN: Duration = Duration::from_secs(10 * 60);
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(5);
const OTHER_ERROR_COOLDOWN: Duration = Duration::from_secs(30);
const NETWORK_COOLDOWN: Duration = Duration::from_secs(10);

pub struct TranslationPool {
    deepl: DeepLClient,
    google: GoogleTranslateClient,
    cache: Arc<Mutex<TranslationCache>>,
    /// Slot -> (anahtar metni, bekleme bitis zamani). Sadece bellekte tutulur;
    /// uygulama yeniden acildiginda tum anahtarlar tekrar denenir.
    blocked: Mutex<HashMap<usize, (String, Instant)>>,
}

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub translated_text: String,
    pub used_engine: String, // Ornek: "DeepL Slot 1", "Google Translate"
}

impl TranslationPool {
    pub fn new() -> Self {
        Self::with_deepl(DeepLClient::new())
    }

    fn with_deepl(deepl: DeepLClient) -> Self {
        Self {
            deepl,
            google: GoogleTranslateClient::new(),
            cache: Arc::new(Mutex::new(TranslationCache::new(500))),
            blocked: Mutex::new(HashMap::new()),
        }
    }

    /// Anahtar hala bekleme suresindeyse true doner.
    fn is_blocked(&self, idx: usize, key: &str) -> bool {
        let mut map = match self.blocked.lock() {
            Ok(m) => m,
            Err(_) => return false,
        };
        match map.get(&idx) {
            Some((k, until)) if k == key && Instant::now() < *until => true,
            Some(_) => {
                map.remove(&idx);
                false
            }
            None => false,
        }
    }

    fn block(&self, idx: usize, key: &str, dur: Duration) {
        if let Ok(mut map) = self.blocked.lock() {
            map.insert(idx, (key.to_string(), Instant::now() + dur));
        }
    }

    /// Tekil bir anahtari test eder (Test Et butonu icin)
    pub async fn test_key(&self, key: &str) -> Result<String, String> {
        match self.deepl.check_usage(key).await {
            Ok(usage) => Ok(format!(
                "Basarili. Kullanim: {} / {} karakter",
                usage.character_count, usage.character_limit
            )),
            Err(DeepLError::QuotaExceeded(msg)) => Err(format!("Kota Doldu: {}", msg)),
            Err(DeepLError::InvalidAuth(msg)) => Err(format!("Gecersiz Anahtar: {}", msg)),
            Err(DeepLError::RateLimited(msg)) => Err(format!("Hiz Siniri: {}", msg)),
            Err(DeepLError::Network(msg)) => Err(format!("Ag Hatasi: {}", msg)),
            Err(DeepLError::Other(code, msg)) => Err(format!("HTTP {}: {}", code, msg)),
        }
    }

    /// Altyaziyi cevirir. Havuzdaki 10 anahtari sirayla dener, hata alanlari pasife alir.
    pub async fn translate(
        &self,
        text: &str,
        config: &mut AppConfig,
    ) -> Result<TranslationResult, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(TranslationResult {
                translated_text: String::new(),
                used_engine: "Bos Metin".to_string(),
            });
        }

        // 1. Onbellek kontrolu (0 ms)
        if let Ok(cache) = self.cache.lock() {
            if let Some(cached) = cache.get(trimmed) {
                return Ok(TranslationResult {
                    translated_text: cached,
                    used_engine: "Onbellek".to_string(),
                });
            }
        }

        let mut errors_log = Vec::new();
        let total_keys = config.deepl_keys.len();

        // Eger kullanici dogrudan "Sadece Google Translate" sectiyse DeepL havuzunu hic deneme
        if config.preferred_engine == PreferredEngine::GoogleOnly {
            match self
                .google
                .translate(trimmed, &config.source_lang, &config.target_lang)
                .await
            {
                Ok(raw_translated) => {
                    let translated = sanitize_turkish_translation(&raw_translated);
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(trimmed, translated.clone());
                    }
                    return Ok(TranslationResult {
                        translated_text: translated,
                        used_engine: "Google Translate".to_string(),
                    });
                }
                Err(err) => {
                    return Err(format!("Google Translate hatasi: {}", err));
                }
            }
        }

        // 2. DeepL Havuzunda don
        // Aktif anahtardan baslayarak sirayla 10 anahtari tara
        let start_idx = config.active_key_index;
        for attempt in 0..total_keys {
            let idx = (start_idx + attempt) % total_keys;
            let key_entry = &config.deepl_keys[idx];

            if key_entry.key.trim().is_empty() {
                continue;
            }

            let key_str = key_entry.key.trim().to_string();

            // Bekleme suresindeki (kota/yetki/hiz hatasi almis) anahtari pas gec.
            // Sure dolunca veya anahtar metni degisince tekrar denenir.
            if self.is_blocked(idx, &key_str) {
                continue;
            }
            let label = if key_entry.label.is_empty() {
                format!("Slot {}", idx + 1)
            } else {
                key_entry.label.clone()
            };

            match self
                .deepl
                .translate(&key_str, trimmed, &config.source_lang, &config.target_lang)
                .await
            {
                Ok(raw_translated) => {
                    let translated = sanitize_turkish_translation(&raw_translated);
                    // Basarili: Anahtari guncelle
                    config.active_key_index = idx;
                    config.deepl_keys[idx].status = KeyStatus::Active;
                    config.deepl_keys[idx].characters_used += trimmed.chars().count();

                    // Diger hazir anahtarlari 'Ready' durumunda tut
                    for (i, k) in config.deepl_keys.iter_mut().enumerate() {
                        if i != idx && k.status == KeyStatus::Active {
                            k.status = KeyStatus::Ready;
                        }
                    }

                    // Onbellege yaz
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(trimmed, translated.clone());
                    }

                    return Ok(TranslationResult {
                        translated_text: translated,
                        used_engine: format!("DeepL [{}]", label),
                    });
                }
                Err(DeepLError::QuotaExceeded(msg)) => {
                    errors_log.push(format!("{}: Kota Doldu ({})", label, msg));
                    config.deepl_keys[idx].status = KeyStatus::QuotaExceeded;
                    self.block(idx, &key_str, QUOTA_COOLDOWN);
                }
                Err(DeepLError::InvalidAuth(msg)) => {
                    errors_log.push(format!("{}: Gecersiz Anahtar ({})", label, msg));
                    config.deepl_keys[idx].status = KeyStatus::InvalidAuth;
                    self.block(idx, &key_str, AUTH_COOLDOWN);
                }
                Err(DeepLError::RateLimited(msg)) => {
                    errors_log.push(format!("{}: Hiz Limiti ({})", label, msg));
                    self.block(idx, &key_str, RATE_LIMIT_COOLDOWN);
                }
                Err(DeepLError::Network(msg)) => {
                    errors_log.push(format!("{}: Ag Hatasi ({})", label, msg));
                    config.deepl_keys[idx].status = KeyStatus::NetworkError;
                    self.block(idx, &key_str, NETWORK_COOLDOWN);
                    // Ag yoksa diger anahtarlari denemek sadece gecikme yaratir
                    break;
                }
                Err(DeepLError::Other(code, msg)) => {
                    errors_log.push(format!("{}: HTTP {} ({})", label, code, msg));
                    self.block(idx, &key_str, OTHER_ERROR_COOLDOWN);
                }
            }
        }

        // 3. Eger tum DeepL anahtarlari basarisiz olduysa ve Google Translate yedegi aktifse
        let allow_google_fallback = config.preferred_engine == PreferredEngine::DeepLFirst && config.fallback_to_google;
        if allow_google_fallback {
            match self
                .google
                .translate(trimmed, &config.source_lang, &config.target_lang)
                .await
            {
                Ok(raw_translated) => {
                    let translated = sanitize_turkish_translation(&raw_translated);
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(trimmed, translated.clone());
                    }
                    return Ok(TranslationResult {
                        translated_text: translated,
                        used_engine: "Google Translate (Yedek)".to_string(),
                    });
                }
                Err(err) => {
                    errors_log.push(format!("Google Translate de basarisiz: {}", err));
                }
            }
        }

        Err(format!("Ceviri basarisiz: {}", errors_log.join(" | ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Sahte DeepL sunucusu: anahtar adina gore 456 / 403 / 429 / 200 doner.
    /// Donus: (taban URL, gelen istek sayaci)
    fn spawn_mock_deepl() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let hits3 = hits2.clone();
                std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 2048];
                    // Basliklar + govde gelene kadar oku
                    loop {
                        let n = stream.read(&mut tmp).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        if let Some(pos) = text.find("\r\n\r\n") {
                            let cl = text
                                .lines()
                                .find_map(|l| {
                                    l.to_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            if buf.len() >= pos + 4 + cl {
                                break;
                            }
                        }
                    }
                    hits3.fetch_add(1, Ordering::SeqCst);
                    let req = String::from_utf8_lossy(&buf).to_string();
                    let (status, body) = if req.contains("quota-key") {
                        ("456 Quota Exceeded", String::new())
                    } else if req.contains("bad-key") {
                        ("403 Forbidden", String::new())
                    } else if req.contains("rate-key") {
                        ("429 Too Many Requests", String::new())
                    } else {
                        (
                            "200 OK",
                            r#"{"translations":[{"text":"Merhaba dunya"}]}"#.to_string(),
                        )
                    };
                    let resp = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                });
            }
        });
        (format!("http://{}", addr), hits)
    }

    fn cfg_with_keys(keys: &[&str]) -> AppConfig {
        let mut cfg = AppConfig {
            fallback_to_google: false,
            ..AppConfig::default()
        };
        for (i, k) in keys.iter().enumerate() {
            cfg.deepl_keys[i].key = k.to_string();
        }
        cfg
    }

    #[tokio::test]
    async fn test_pool_empty_text_returns_immediately() {
        let pool = TranslationPool::new();
        let mut cfg = AppConfig::default();
        let res = pool.translate("   ", &mut cfg).await.unwrap();
        assert_eq!(res.translated_text, "");
        assert_eq!(res.used_engine, "Bos Metin");
    }

    #[tokio::test]
    async fn test_pool_fallback_to_google_when_no_deepl_keys() {
        let pool = TranslationPool::new();
        let mut cfg = AppConfig {
            fallback_to_google: true,
            ..Default::default()
        };
        // Tum anahtarlar bos
        for k in &mut cfg.deepl_keys {
            k.key.clear();
        }

        let res = pool.translate("Hello", &mut cfg).await;
        // Internet baglantisi varsa Google Translate uzerinden cevirir
        if let Ok(r) = res {
            assert!(
                r.translated_text.to_lowercase().contains("merhaba")
                    || !r.translated_text.is_empty()
            );
            assert_eq!(r.used_engine, "Google Translate (Yedek)");
        }
    }

    #[tokio::test]
    async fn test_failover_quota_then_invalid_then_success() {
        let (url, hits) = spawn_mock_deepl();
        let pool = TranslationPool::with_deepl(DeepLClient::with_base_url(&url));
        let mut cfg = cfg_with_keys(&["quota-key", "bad-key", "good-key"]);

        let res = pool.translate("Hello world", &mut cfg).await.unwrap();
        assert_eq!(res.translated_text, "Merhaba dunya");
        assert_eq!(res.used_engine, "DeepL [Slot 3]");
        assert_eq!(cfg.active_key_index, 2);
        assert_eq!(cfg.deepl_keys[0].status, KeyStatus::QuotaExceeded);
        assert_eq!(cfg.deepl_keys[1].status, KeyStatus::InvalidAuth);
        assert_eq!(cfg.deepl_keys[2].status, KeyStatus::Active);
        assert_eq!(
            cfg.deepl_keys[2].characters_used,
            "Hello world".chars().count()
        );
        assert_eq!(hits.load(Ordering::SeqCst), 3);

        // Ikinci (farkli) metinde bloklu anahtarlar tekrar denenmez
        let res2 = pool.translate("Another line", &mut cfg).await.unwrap();
        assert_eq!(res2.used_engine, "DeepL [Slot 3]");
        assert_eq!(hits.load(Ordering::SeqCst), 4);

        // Ayni metin onbellekten gelir, sunucuya istek gitmez
        let res3 = pool.translate("Another line", &mut cfg).await.unwrap();
        assert_eq!(res3.used_engine, "Onbellek");
        assert_eq!(hits.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_all_keys_fail_without_fallback_returns_error() {
        let (url, _hits) = spawn_mock_deepl();
        let pool = TranslationPool::with_deepl(DeepLClient::with_base_url(&url));
        let mut cfg = cfg_with_keys(&["quota-key", "bad-key", "rate-key"]);
        let err = pool.translate("Hello", &mut cfg).await.unwrap_err();
        assert!(
            err.contains("Kota Doldu")
                && err.contains("Gecersiz Anahtar")
                && err.contains("Hiz Limiti")
        );
    }

    #[tokio::test]
    async fn test_blocked_key_is_retried_after_key_changes() {
        let (url, _hits) = spawn_mock_deepl();
        let pool = TranslationPool::with_deepl(DeepLClient::with_base_url(&url));
        let mut cfg = cfg_with_keys(&["bad-key"]);
        assert!(pool.translate("Hello", &mut cfg).await.is_err());
        assert!(pool.is_blocked(0, "bad-key"));

        // Kullanici anahtari duzeltince bekleme beklenmeden tekrar denenir
        cfg.deepl_keys[0].key = "good-key".to_string();
        let res = pool.translate("Hello", &mut cfg).await.unwrap();
        assert_eq!(res.used_engine, "DeepL [Slot 1]");
    }

    #[tokio::test]
    async fn test_fuzzy_cache_prevents_repeated_api_calls_on_ocr_jitter() {
        let (url, hits) = spawn_mock_deepl();
        let pool = TranslationPool::with_deepl(DeepLClient::with_base_url(&url));
        let mut cfg = cfg_with_keys(&["good-key"]);

        // 1. Ilk okuma API'ye gider (hits = 1)
        let r1 = pool
            .translate("We need to get out of here right now!", &mut cfg)
            .await
            .unwrap();
        assert_eq!(r1.used_engine, "DeepL [Slot 1]");
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // 2. Noktalama/bosluk farkli ayni cumle -> Onbellekten doner, API'ye gitmez (hits = 1)
        let r2 = pool
            .translate("we need to get out of here right now...", &mut cfg)
            .await
            .unwrap();
        assert_eq!(r2.used_engine, "Onbellek");
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // 3. OCR titremesi (1-2 harf yanlis okuma) -> Fuzzy Onbellekten doner, API'ye gitmez (hits = 1)
        let r3 = pool
            .translate("We need t0 get out of here right n0w!", &mut cfg)
            .await
            .unwrap();
        assert_eq!(r3.used_engine, "Onbellek");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_block_expires_and_persisted_status_does_not_block() {
        let pool = TranslationPool::new();
        pool.block(0, "k", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));
        assert!(!pool.is_blocked(0, "k"));
        // Diskten yuklenmis QuotaExceeded durumu (bellekte kayit yok) engel olmamali
        assert!(!pool.is_blocked(3, "k"));
    }
}
