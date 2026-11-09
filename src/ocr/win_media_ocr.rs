use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

pub struct WinMediaOcr {
    engine: OcrEngine,
    #[allow(dead_code)]
    current_lang: String,
}

impl WinMediaOcr {
    /// Sistemde mevcut OCR dillerini listeler (orn: ["en-US", "tr"])
    pub fn get_available_languages() -> Vec<String> {
        let mut list = Vec::new();
        if let Ok(langs) = OcrEngine::AvailableRecognizerLanguages() {
            if let Ok(size) = langs.Size() {
                for i in 0..size {
                    if let Ok(lang) = langs.GetAt(i) {
                        if let Ok(tag) = lang.LanguageTag() {
                            list.push(tag.to_string());
                        }
                    }
                }
            }
        }
        list
    }

    /// Belirtilen dil kodu icin (orn: "en" veya "en-US") OCR motorunu baslatir.
    /// Eger bulunamazsa sistem profil diline veya mevcut ilk dile duser.
    pub fn new(preferred_lang: &str) -> Result<Self, String> {
        let available = Self::get_available_languages();
        if available.is_empty() {
            return Err("Sistemde yuklu hicbir Windows OCR dil paketi bulunamadi.".to_string());
        }

        // 1. Oncelik: Tam eslesen dil (orn: "en-US" veya "en-GB")
        let matched_tag = available
            .iter()
            .find(|tag| tag.to_lowercase() == "en-us")
            .or_else(|| {
                available.iter().find(|tag| {
                    tag.to_lowercase()
                        .starts_with(&preferred_lang.to_lowercase())
                })
            })
            .cloned();

        let engine = if let Some(tag) = &matched_tag {
            let hstring = HSTRING::from(tag.as_str());
            let lang = Language::CreateLanguage(&hstring)
                .map_err(|e| format!("Dil olusturulamadi ({}): {:?}", tag, e))?;
            OcrEngine::TryCreateFromLanguage(&lang)
                .map_err(|e| format!("OCR motoru olusturulamadi ({}): {:?}", tag, e))?
        } else {
            // Sistemde Ingilizce OCR yoksa profil dilinden degil, direkt en-US olusturmayi dene
            let en_str = HSTRING::from("en-US");
            if let Ok(en_lang) = Language::CreateLanguage(&en_str) {
                if let Ok(eng) = OcrEngine::TryCreateFromLanguage(&en_lang) {
                    eng
                } else {
                    OcrEngine::TryCreateFromUserProfileLanguages()
                        .map_err(|e| format!("Profil diliyle OCR olusturulamadi: {:?}", e))?
                }
            } else {
                OcrEngine::TryCreateFromUserProfileLanguages()
                    .map_err(|e| format!("Profil diliyle OCR olusturulamadi: {:?}", e))?
            }
        };

        let current_lang = matched_tag.unwrap_or_else(|| {
            if let Ok(lang) = engine.RecognizerLanguage() {
                lang.LanguageTag()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|_| "Bilinmeyen".to_string())
            } else {
                "Bilinmeyen".to_string()
            }
        });

        Ok(Self {
            engine,
            current_lang,
        })
    }

    #[allow(dead_code)]
    pub fn current_language(&self) -> &str {
        &self.current_lang
    }

    /// RGBA formatindaki ham piksel tamponunu okuyup metin dondurur.
    pub fn recognize_rgba(
        &self,
        rgba_pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<String, String> {
        if width == 0 || height == 0 || rgba_pixels.is_empty() {
            return Ok(String::new());
        }

        // RGBA -> BGRA donusumu (Windows SoftwareBitmap Bgra8 formatini tercih eder)
        let mut bgra_pixels = Vec::with_capacity(rgba_pixels.len());
        #[allow(clippy::chunks_exact_to_as_chunks)]
        for chunk in rgba_pixels.chunks_exact(4) {
            bgra_pixels.push(chunk[2]); // B
            bgra_pixels.push(chunk[1]); // G
            bgra_pixels.push(chunk[0]); // R
            bgra_pixels.push(chunk[3]); // A
        }

        let writer = DataWriter::new().map_err(|e| format!("DataWriter hatasi: {:?}", e))?;
        writer
            .WriteBytes(&bgra_pixels)
            .map_err(|e| format!("Byte yazma hatasi: {:?}", e))?;
        let buffer = writer
            .DetachBuffer()
            .map_err(|e| format!("Buffer detach hatasi: {:?}", e))?;

        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            width as i32,
            height as i32,
        )
        .map_err(|e| format!("SoftwareBitmap olusturulamadi: {:?}", e))?;

        let async_op = self
            .engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| format!("RecognizeAsync cagirisi basarisiz: {:?}", e))?;

        // Senkron bloklayarak sonucu al (Arka plan is parcaciginda calisir)
        let ocr_result = async_op
            .get()
            .map_err(|e| format!("OCR sonucu alinamadi: {:?}", e))?;

        let text = ocr_result
            .Text()
            .map_err(|e| format!("OCR metin okuma hatasi: {:?}", e))?;

        Ok(text.to_string())
    }
}
