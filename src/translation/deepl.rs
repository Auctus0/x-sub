use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
pub enum DeepLError {
    QuotaExceeded(String), // HTTP 456
    InvalidAuth(String),   // HTTP 403
    RateLimited(String),   // HTTP 429
    Network(String),
    Other(u16, String),
}

#[derive(Serialize)]
struct TranslateRequest<'a> {
    text: Vec<&'a str>,
    target_lang: &'a str,
    source_lang: &'a str,
}

#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<TranslationItem>,
}

#[derive(Deserialize)]
struct TranslationItem {
    text: String,
}

#[derive(Deserialize)]
pub struct UsageResponse {
    pub character_count: usize,
    pub character_limit: usize,
}

pub struct DeepLClient {
    client: Client,
    base_url_override: Option<String>,
}

impl DeepLClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url_override: None,
        }
    }

    /// Test amacli: istekleri verilen taban URL'ye yonlendirir.
    #[cfg(test)]
    pub fn with_base_url(base_url: &str) -> Self {
        let mut c = Self::new();
        c.base_url_override = Some(base_url.trim_end_matches('/').to_string());
        c
    }

    fn get_base_url(&self, api_key: &str) -> String {
        if let Some(url) = &self.base_url_override {
            return url.clone();
        }
        if api_key.trim().ends_with(":fx") {
            "https://api-free.deepl.com/v2".to_string()
        } else {
            "https://api.deepl.com/v2".to_string()
        }
    }

    /// Anahtarin gecerliligini ve bakiye/kullanimini test eder
    pub async fn check_usage(&self, api_key: &str) -> Result<UsageResponse, DeepLError> {
        let trimmed_key = api_key.trim();
        if trimmed_key.is_empty() {
            return Err(DeepLError::InvalidAuth("Anahtar bos".to_string()));
        }

        let url = format!("{}/usage", self.get_base_url(trimmed_key));
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", trimmed_key))
            .send()
            .await
            .map_err(|e| DeepLError::Network(format!("Ag hatasi: {}", e)))?;

        let status = resp.status().as_u16();
        if status == 200 {
            let usage = resp
                .json::<UsageResponse>()
                .await
                .map_err(|e| DeepLError::Network(format!("JSON cozumleme hatasi: {}", e)))?;
            Ok(usage)
        } else if status == 403 {
            Err(DeepLError::InvalidAuth(
                "Gecersiz API anahtari (403)".to_string(),
            ))
        } else if status == 456 {
            Err(DeepLError::QuotaExceeded("Kota doldu (456)".to_string()))
        } else if status == 429 {
            Err(DeepLError::RateLimited("Cok fazla istek (429)".to_string()))
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(DeepLError::Other(status, text))
        }
    }

    /// Metni Turkceye cevirir
    pub async fn translate(
        &self,
        api_key: &str,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, DeepLError> {
        let trimmed_key = api_key.trim();
        if trimmed_key.is_empty() {
            return Err(DeepLError::InvalidAuth("Anahtar bos".to_string()));
        }

        let url = format!("{}/translate", self.get_base_url(trimmed_key));
        let body = TranslateRequest {
            text: vec![text],
            target_lang,
            source_lang,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", trimmed_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| DeepLError::Network(format!("Ag hatasi: {}", e)))?;

        let status = resp.status().as_u16();
        if status == 200 {
            let res = resp
                .json::<TranslateResponse>()
                .await
                .map_err(|e| DeepLError::Network(format!("JSON cozumleme hatasi: {}", e)))?;
            if let Some(first) = res.translations.into_iter().next() {
                Ok(first.text)
            } else {
                Ok(String::new())
            }
        } else if status == 456 {
            Err(DeepLError::QuotaExceeded("Kota doldu (456)".to_string()))
        } else if status == 403 {
            Err(DeepLError::InvalidAuth(
                "Gecersiz API anahtari (403)".to_string(),
            ))
        } else if status == 429 {
            Err(DeepLError::RateLimited("Cok fazla istek (429)".to_string()))
        } else {
            let body_text = resp.text().await.unwrap_or_default();
            Err(DeepLError::Other(status, body_text))
        }
    }
}
