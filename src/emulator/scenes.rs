use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone)]
pub struct GameScene {
    pub game_name: &'static str,
    pub character: &'static str,
    pub english_dialog: &'static str,
    pub expected_turkish: &'static str,
    pub text_color: Rgba<u8>, // Beyaz veya Sari
}

pub fn get_standard_game_scenes() -> Vec<GameScene> {
    vec![
        GameScene {
            game_name: "Red Dead Redemption 2",
            character: "Dutch van der Linde",
            english_dialog: "We just need one more score, Arthur. Have some goddamn faith!",
            expected_turkish:
                "Sadece bir vurguna daha ihtiyacimiz var Arthur. Birazcik inancin olsun!",
            text_color: Rgba([255, 255, 255, 255]), // Sinematik beyaz
        },
        GameScene {
            game_name: "God of War Ragnarok",
            character: "Kratos",
            english_dialog: "Do not be sorry. Be better.",
            expected_turkish: "Ozur dileme. Daha iyi ol.",
            text_color: Rgba([255, 235, 120, 255]), // Oyun ici altin sarisi
        },
        GameScene {
            game_name: "The Witcher 3: Wild Hunt",
            character: "Geralt of Rivia",
            english_dialog: "Evil is evil. Lesser, greater, middling, it makes no difference.",
            expected_turkish: "Kotuluk kotuluktur. Azi, cogu, ortasi fark etmez.",
            text_color: Rgba([255, 255, 255, 255]),
        },
        GameScene {
            game_name: "Cyberpunk 2077",
            character: "Johnny Silverhand",
            english_dialog: "Wake up, Samurai. We have a city to burn.",
            expected_turkish: "Uyan Samuray. Yakacak bir sehrimiz var.",
            text_color: Rgba([255, 230, 80, 255]), // Neon sari
        },
        GameScene {
            game_name: "The Last of Us Part I",
            character: "Joel Miller",
            english_dialog: "No matter what, you keep finding something to fight for.",
            expected_turkish: "Ne olursa olsun, ugrunda savasacak bir sey bulmaya devam edersin.",
            text_color: Rgba([255, 255, 255, 255]),
        },
        // GTA tarzi altyazilar: karakter adi/tirnak yok, kisa ve argo agirlikli replikler.
        // Not: GTA 6 replikleri burada ornek amaclidir (oyunun resmi metni degildir).
        GameScene {
            game_name: "GTA 6 (ornek)",
            character: "Lucia",
            english_dialog: "No, no, no! Floor it, they're right behind us!",
            expected_turkish: "Hayir, hayir, hayir! Gazi kok, hemen arkamizdalar!",
            text_color: Rgba([255, 255, 255, 255]),
        },
        GameScene {
            game_name: "GTA 6 (ornek)",
            character: "Jason",
            english_dialog: "Welcome to Vice City. Keep your head down and your mouth shut.",
            expected_turkish: "Vice City'ye hos geldin. Basini egik tut, agzini kapali.",
            text_color: Rgba([255, 255, 255, 255]),
        },
        GameScene {
            game_name: "GTA 6 (ornek)",
            character: "Telefon",
            english_dialog: "Hey, you still owe me five grand. Don't make me come find you.",
            expected_turkish: "Hey, bana hala bes bin borcun var. Seni aramama neden olma.",
            text_color: Rgba([255, 235, 120, 255]),
        },
    ]
}

pub struct GameScreenRenderer {
    font: Option<FontArc>,
}

impl GameScreenRenderer {
    pub fn new() -> Self {
        let font_bytes = std::fs::read(r"C:\Windows\Fonts\arial.ttf")
            .or_else(|_| std::fs::read(r"C:\Windows\Fonts\segoeui.ttf"));

        let font = match font_bytes {
            Ok(bytes) => FontArc::try_from_vec(bytes).ok(),
            Err(_) => None,
        };

        Self { font }
    }

    pub fn font(&self) -> Option<&FontArc> {
        self.font.as_ref()
    }

    /// 1200x180 boyutunda gercekci bir oyun altyazi karesi uretir
    pub fn render_scene(&self, scene: &GameScene) -> RgbaImage {
        let w = 1200u32;
        let h = 180u32;
        let mut img = RgbaImage::new(w, h);

        // Koyu sinematik zemin (hafif gradyanli)
        for x in 0..w {
            for y in 0..h {
                let dark_lum = ((y as f32 / h as f32) * 15.0) as u8 + 10;
                img.put_pixel(x, y, Rgba([dark_lum, dark_lum, dark_lum + 2, 255]));
            }
        }

        // Altyazi yari saydam siyah kapsul kutusu (PS5 / oyun stili)
        let pad_x = 80u32;
        let pad_y = 35u32;
        for x in pad_x..(w - pad_x) {
            for y in pad_y..(h - pad_y) {
                img.put_pixel(x, y, Rgba([12, 12, 14, 255]));
            }
        }

        if let Some(font) = &self.font {
            // Oyunlardaki gibi sadece replik cizilir (karakter adi/tirnak yok)
            let full_text = scene.english_dialog.to_string();

            let font_size = 30.0_f32;
            let text_y = 75;
            let text_x = 45;

            // 1. Siyah kenar golgesi (Drop-Shadow)
            self.draw_text(
                &mut img,
                font,
                &full_text,
                font_size,
                text_x + 2,
                text_y + 2,
                Rgba([0, 0, 0, 255]),
            );

            // 2. Ana parlak yazi
            self.draw_text(
                &mut img,
                font,
                &full_text,
                font_size,
                text_x,
                text_y,
                scene.text_color,
            );
        }

        img
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &self,
        img: &mut RgbaImage,
        font: &FontArc,
        text: &str,
        size: f32,
        start_x: i32,
        start_y: i32,
        color: Rgba<u8>,
    ) {
        let scale = PxScale::from(size);
        let scaled_font = font.as_scaled(scale);
        let mut cursor_x = start_x as f32;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            let advance = scaled_font.h_advance(glyph_id);

            if let Some(glyph) = scaled_font.outline_glyph(
                font.glyph_id(c)
                    .with_scale_and_position(scale, ab_glyph::point(cursor_x, start_y as f32)),
            ) {
                let bounds = glyph.px_bounds();
                glyph.draw(|gx, gy, coverage| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;

                    if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height()
                    {
                        let px = px as u32;
                        let py = py as u32;
                        let alpha = (coverage * (color[3] as f32)) as u8;

                        if alpha > 10 {
                            let orig = img.get_pixel(px, py);
                            let a_norm = alpha as f32 / 255.0;
                            let r =
                                (color[0] as f32 * a_norm + orig[0] as f32 * (1.0 - a_norm)) as u8;
                            let g =
                                (color[1] as f32 * a_norm + orig[1] as f32 * (1.0 - a_norm)) as u8;
                            let b =
                                (color[2] as f32 * a_norm + orig[2] as f32 * (1.0 - a_norm)) as u8;
                            img.put_pixel(px, py, Rgba([r, g, b, 255]));
                        }
                    }
                });
            }
            cursor_x += advance;
        }
    }
}
