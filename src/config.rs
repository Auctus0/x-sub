use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const MAX_DEEPL_KEYS: usize = 10;
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepLKeyEntry {
    pub key: String,
    pub label: String,
    pub characters_used: usize,
    pub status: KeyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    Ready,
    Active,
    QuotaExceeded, // HTTP 456
    InvalidAuth,   // HTTP 403
    NetworkError,
    Untested,
}

impl KeyStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyStatus::Ready => "Hazir",
            KeyStatus::Active => "Aktif",
            KeyStatus::QuotaExceeded => "Kota Doldu (456)",
            KeyStatus::InvalidAuth => "Gecersiz Anahtar (403)",
            KeyStatus::NetworkError => "Baglanti Hatasi",
            KeyStatus::Untested => "Test Edilmedi",
        }
    }
}

impl Default for DeepLKeyEntry {
    fn default() -> Self {
        Self {
            key: String::new(),
            label: String::new(),
            characters_used: 0,
            status: KeyStatus::Untested,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleRoi {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for SubtitleRoi {
    fn default() -> Self {
        // 1920x1080 ekran varsayimi ile alt orta bolge
        Self {
            x: 360,
            y: 860,
            width: 1200,
            height: 180,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub font_size: f32,
    pub bg_opacity: f32,         // 0.0 - 1.0
    pub text_color_rgb: [u8; 3], // varsayilan [255, 255, 255]
    pub bg_color_rgb: [u8; 3],   // varsayilan [20, 20, 20]
    pub window_x: f32,
    pub window_y: f32,
    pub window_w: f32,
    pub window_h: f32,
    pub is_locked: bool, // Mouse pass-through ve yer degistirilemezlik
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            bg_opacity: 0.0, // Varsayilan olarak %0 (tamamen seffaf arka plan)
            text_color_rgb: [255, 255, 255],
            bg_color_rgb: [18, 18, 18],
            window_x: 360.0,
            window_y: 840.0,
            window_w: 1200.0,
            window_h: 160.0,
            is_locked: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreferredEngine {
    DeepLFirst,
    GoogleOnly,
    DeepLOnly,
}

impl Default for PreferredEngine {
    fn default() -> Self {
        PreferredEngine::DeepLFirst
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub preferred_engine: PreferredEngine,
    pub deepl_keys: Vec<DeepLKeyEntry>,
    pub active_key_index: usize,
    pub fallback_to_google: bool,
    pub source_lang: String,
    pub target_lang: String,
    pub capture_interval_ms: u64,
    pub pixel_diff_threshold: f32,
    pub roi: SubtitleRoi,
    pub overlay: OverlayConfig,
    pub target_monitor_index: usize,
    pub target_window_title: Option<String>,
    #[serde(default = "default_auto_clear")]
    pub auto_clear_subtitles: bool,
    #[serde(default = "default_clear_delay_ms")]
    pub clear_delay_ms: u64,
    #[serde(default = "default_max_hold_timeout")]
    pub max_hold_timeout_secs: u64,
}

fn default_auto_clear() -> bool {
    true
}

fn default_clear_delay_ms() -> u64 {
    350
}

fn default_max_hold_timeout() -> u64 {
    6
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut keys = Vec::with_capacity(MAX_DEEPL_KEYS);
        for i in 1..=MAX_DEEPL_KEYS {
            keys.push(DeepLKeyEntry {
                key: String::new(),
                label: format!("Slot {}", i),
                characters_used: 0,
                status: KeyStatus::Untested,
            });
        }

        Self {
            preferred_engine: PreferredEngine::DeepLFirst,
            deepl_keys: keys,
            active_key_index: 0,
            fallback_to_google: true,
            source_lang: "en".to_string(),
            target_lang: "tr".to_string(),
            capture_interval_ms: 120,
            pixel_diff_threshold: 0.02,
            roi: SubtitleRoi::default(),
            overlay: OverlayConfig::default(),
            target_monitor_index: 0,
            target_window_title: None,
            auto_clear_subtitles: true,
            clear_delay_ms: 350,
            max_hold_timeout_secs: 6,
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = Path::new(CONFIG_FILE_NAME);
        if path.exists() {
            if let Ok(data) = fs::read_to_string(path) {
                match serde_json::from_str::<AppConfig>(&data) {
                    Ok(mut cfg) => {
                        // Maksimum 10 anahtari garanti et
                        while cfg.deepl_keys.len() < MAX_DEEPL_KEYS {
                            let i = cfg.deepl_keys.len() + 1;
                            cfg.deepl_keys.push(DeepLKeyEntry {
                                key: String::new(),
                                label: format!("Slot {}", i),
                                characters_used: 0,
                                status: KeyStatus::Untested,
                            });
                        }
                        if cfg.deepl_keys.len() > MAX_DEEPL_KEYS {
                            cfg.deepl_keys.truncate(MAX_DEEPL_KEYS);
                        }
                        return cfg;
                    }
                    Err(_) => {
                        // Bozuk dosya: API anahtarlari kaybolmasin diye uzerine yazmadan once yedekle
                        let _ = fs::copy(path, "config.json.bak");
                    }
                }
            }
        }
        let default_cfg = AppConfig::default();
        let _ = default_cfg.save();
        default_cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON serilestirme hatasi: {}", e))?;
        fs::write(CONFIG_FILE_NAME, json).map_err(|e| format!("Dosya yazma hatasi: {}", e))?;
        Ok(())
    }
}
