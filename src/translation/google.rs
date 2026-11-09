use reqwest::Client;
use std::time::Duration;

pub struct GoogleTranslateClient {
    client: Client,
}

impl GoogleTranslateClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Google Translate public endpoint ile ucretsiz ve anahtarsiz cevirir.
    pub async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            source_lang,
            target_lang,
            urlencoding(trimmed)
        );

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| format!("Google Translate ag hatasi: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Google Translate HTTP hatasi: {}", resp.status()));
        }

        let json_val = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Google Translate yanit cozumlenemedi: {}", e))?;

        // Yanit yapisi: [[[ "Cevrilen Cumle", "Orijinal Cumle", null, null, 1 ], ...], ...]
        let mut result_text = String::new();
        if let Some(sentences) = json_val.get(0).and_then(|v| v.as_array()) {
            for sentence in sentences {
                if let Some(part) = sentence.get(0).and_then(|v| v.as_str()) {
                    result_text.push_str(part);
                }
            }
        }

        if result_text.is_empty() {
            Err("Google Translate metin cikartilamadi".to_string())
        } else {
            Ok(result_text)
        }
    }
}

fn urlencoding(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}
