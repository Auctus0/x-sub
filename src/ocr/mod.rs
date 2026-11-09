pub mod algorithms;
pub mod anti_garbage;
pub mod preprocessor;
pub mod win_media_ocr;

pub use algorithms::{
    binarize_subtitle_image, calculate_text_similarity, compute_dhash, hamming_distance,
    isolate_subtitle_for_ocr,
};
pub use anti_garbage::AntiGarbageFilter;
pub use preprocessor::{calculate_image_diff, clean_ocr_text, sanitize_turkish_translation};
pub use win_media_ocr::WinMediaOcr;
