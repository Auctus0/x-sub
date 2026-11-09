use crate::config::{AppConfig, SubtitleRoi};
use crate::theme::{
    COLOR_ACCENT_BLUE, COLOR_BG_CARD, COLOR_BORDER_CARD, COLOR_STATUS_OK, COLOR_TEXT_BRIGHT,
    COLOR_TEXT_NORMAL,
};
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    None,
    DrawingNew,
    Move,
    ResizeNW,
    ResizeN,
    ResizeNE,
    ResizeE,
    ResizeSE,
    ResizeS,
    ResizeSW,
    ResizeW,
}

pub struct RoiSelectorView {
    pub current_rect: Option<Rect>,
    pub drag_mode: DragMode,
    pub drag_start_pos: Option<Pos2>,
    pub initial_rect: Option<Rect>,
    /// Dis ekran karartma opakligi (%0 = tamamen seffaf, %100 = simsiyah)
    pub backdrop_opacity: f32,
    /// Secim kutusunun ic arka plan opakligi (%0 = tamamen seffaf)
    pub box_fill_opacity: f32,
}

impl Default for RoiSelectorView {
    fn default() -> Self {
        Self {
            current_rect: None,
            drag_mode: DragMode::None,
            drag_start_pos: None,
            initial_rect: None,
            backdrop_opacity: 0.0, // Varsayilan olarak %0 (hicbir siyahlik yok, tamamen seffaf overlay)
            box_fill_opacity: 0.08, // Sadece secilen mavi kutu cok hafif belli olsun (%8)
        }
    }
}

const HANDLE_SIZE: f32 = 8.0;
const HIT_MARGIN: f32 = 10.0;

impl RoiSelectorView {
    pub fn open_with_config(&mut self, config: &AppConfig, ppp: f32) {
        let scale = ppp.max(1.0);
        self.current_rect = Some(Rect::from_min_size(
            Pos2::new(config.roi.x as f32 / scale, config.roi.y as f32 / scale),
            Vec2::new(
                config.roi.width as f32 / scale,
                config.roi.height as f32 / scale,
            ),
        ));
        self.drag_mode = DragMode::None;
        self.drag_start_pos = None;
        self.initial_rect = None;
    }

    fn hit_test(rect: Rect, pos: Pos2) -> DragMode {
        let min = rect.min;
        let max = rect.max;

        let near_left = (pos.x - min.x).abs() <= HIT_MARGIN;
        let near_right = (pos.x - max.x).abs() <= HIT_MARGIN;
        let near_top = (pos.y - min.y).abs() <= HIT_MARGIN;
        let near_bottom = (pos.y - max.y).abs() <= HIT_MARGIN;

        let within_x = pos.x >= min.x - HIT_MARGIN && pos.x <= max.x + HIT_MARGIN;
        let within_y = pos.y >= min.y - HIT_MARGIN && pos.y <= max.y + HIT_MARGIN;

        if near_left && near_top {
            DragMode::ResizeNW
        } else if near_right && near_top {
            DragMode::ResizeNE
        } else if near_right && near_bottom {
            DragMode::ResizeSE
        } else if near_left && near_bottom {
            DragMode::ResizeSW
        } else if near_top && within_x {
            DragMode::ResizeN
        } else if near_bottom && within_x {
            DragMode::ResizeS
        } else if near_left && within_y {
            DragMode::ResizeW
        } else if near_right && within_y {
            DragMode::ResizeE
        } else if rect.contains(pos) {
            DragMode::Move
        } else {
            // Kutu disina yanlislikla tiklandiginda secili alani bozmamak icin None dondur
            DragMode::None
        }
    }

    fn sync_rect_to_config(&self, rect: Rect, ppp: f32, config: &mut AppConfig) {
        if rect.width() >= 30.0 && rect.height() >= 15.0 {
            config.roi = SubtitleRoi {
                x: (rect.min.x.max(0.0) * ppp).round() as u32,
                y: (rect.min.y.max(0.0) * ppp).round() as u32,
                width: (rect.width() * ppp).round() as u32,
                height: (rect.height() * ppp).round() as u32,
            };
            config.overlay.window_x = rect.min.x.max(0.0);
            config.overlay.window_y = rect.min.y.max(0.0);
            config.overlay.window_w = rect.width().max(260.0);
            config.overlay.window_h = rect.height().max(80.0);
        }
    }

    fn cursor_for_mode(mode: DragMode) -> CursorIcon {
        match mode {
            DragMode::ResizeNW | DragMode::ResizeSE => CursorIcon::ResizeNwSe,
            DragMode::ResizeNE | DragMode::ResizeSW => CursorIcon::ResizeNeSw,
            DragMode::ResizeN | DragMode::ResizeS => CursorIcon::ResizeVertical,
            DragMode::ResizeE | DragMode::ResizeW => CursorIcon::ResizeHorizontal,
            DragMode::Move => CursorIcon::Grab,
            DragMode::DrawingNew => CursorIcon::Crosshair,
            DragMode::None => CursorIcon::Default,
        }
    }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        config: &mut AppConfig,
        is_open: &mut bool,
    ) {
        let ppp = ctx.pixels_per_point().max(1.0);
        if self.current_rect.is_none() {
            self.open_with_config(config, ppp);
        }

        let screen_rect = ctx.input(|i| i.screen_rect());

        // Ust kontrol cubugu alani (her cozunurluge ve DPI olcegine tam sigacak sekilde dinamik genislik)
        let bar_w = (screen_rect.width() - 32.0).clamp(480.0, 960.0);
        let top_bar_rect = Rect::from_min_size(
            Pos2::new((screen_rect.center().x - bar_w * 0.5).max(16.0), 16.0),
            Vec2::new(bar_w, 72.0),
        );

        // Klavyeden Escape ile iptal, Enter ile kaydet
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            *is_open = false;
            self.drag_mode = DragMode::None;
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(r) = self.current_rect {
                self.apply_and_close(r, ppp, config, is_open);
                return;
            }
        }

        let pointer = ctx.input(|i| i.pointer.clone());
        let interact_pos = pointer.interact_pos();
        let hover_pos = pointer.hover_pos();

        // Fare tekerlegi (Scroll) ile kutuyu hizlica buyutup kucultme ve aninda config'e kaydetme
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta.abs() > 0.1 {
            if let Some(mut r) = self.current_rect {
                let step = scroll_delta * 0.5;
                let new_w = (r.width() + step * 2.0).clamp(40.0, screen_rect.width());
                let new_h = (r.height() + step).clamp(20.0, screen_rect.height());
                let center = r.center();
                r = Rect::from_center_size(center, Vec2::new(new_w, new_h)).intersect(screen_rect);
                self.current_rect = Some(r);
                self.sync_rect_to_config(r, ppp, config);
                let _ = config.save();
            }
        }

        // Imlec gorunumunu guncelle
        if let Some(pos) = hover_pos {
            if !top_bar_rect.contains(pos) {
                let mode = if self.drag_mode != DragMode::None {
                    self.drag_mode
                } else if let Some(r) = self.current_rect {
                    Self::hit_test(r, pos)
                } else {
                    DragMode::DrawingNew
                };
                ctx.set_cursor_icon(Self::cursor_for_mode(mode));
            }
        }

        // Kutu icine cift tiklandiginda da hizlica kaydet ve kapat
        if pointer.button_double_clicked(egui::PointerButton::Primary) {
            if let (Some(pos), Some(r)) = (interact_pos, self.current_rect) {
                if !top_bar_rect.contains(pos) && r.contains(pos) {
                    self.apply_and_close(r, ppp, config, is_open);
                    return;
                }
            }
        }

        // Fare basma / surukleme / birakma islemleri
        if pointer.primary_pressed() {
            if let Some(pos) = interact_pos {
                if !top_bar_rect.contains(pos) {
                    let mode = if let Some(r) = self.current_rect {
                        Self::hit_test(r, pos)
                    } else {
                        DragMode::DrawingNew
                    };
                    if mode != DragMode::None {
                        self.drag_mode = mode;
                        self.drag_start_pos = Some(pos);
                        self.initial_rect = self.current_rect;
                        if mode == DragMode::DrawingNew {
                            self.current_rect = Some(Rect::from_two_pos(pos, pos));
                        }
                    }
                }
            }
        } else if pointer.primary_down() && self.drag_mode != DragMode::None {
            if let (Some(pos), Some(start)) = (interact_pos, self.drag_start_pos) {
                let delta = pos - start;
                match self.drag_mode {
                    DragMode::DrawingNew => {
                        self.current_rect = Some(Rect::from_two_pos(start, pos));
                    }
                    DragMode::Move => {
                        if let Some(init) = self.initial_rect {
                            let mut moved = init.translate(delta);
                            // Ekran sinirlarinda tut
                            if moved.min.x < 0.0 {
                                moved = moved.translate(Vec2::new(-moved.min.x, 0.0));
                            }
                            if moved.min.y < 0.0 {
                                moved = moved.translate(Vec2::new(0.0, -moved.min.y));
                            }
                            if moved.max.x > screen_rect.max.x {
                                moved =
                                    moved.translate(Vec2::new(screen_rect.max.x - moved.max.x, 0.0));
                            }
                            if moved.max.y > screen_rect.max.y {
                                moved =
                                    moved.translate(Vec2::new(0.0, screen_rect.max.y - moved.max.y));
                            }
                            self.current_rect = Some(moved);
                            self.sync_rect_to_config(moved, ppp, config);
                        }
                    }
                    _ => {
                        if let Some(init) = self.initial_rect {
                            let mut min = init.min;
                            let mut max = init.max;

                            match self.drag_mode {
                                DragMode::ResizeNW => {
                                    min.x = (init.min.x + delta.x).min(max.x - 30.0);
                                    min.y = (init.min.y + delta.y).min(max.y - 20.0);
                                }
                                DragMode::ResizeN => {
                                    min.y = (init.min.y + delta.y).min(max.y - 20.0);
                                }
                                DragMode::ResizeNE => {
                                    max.x = (init.max.x + delta.x).max(min.x + 30.0);
                                    min.y = (init.min.y + delta.y).min(max.y - 20.0);
                                }
                                DragMode::ResizeE => {
                                    max.x = (init.max.x + delta.x).max(min.x + 30.0);
                                }
                                DragMode::ResizeSE => {
                                    max.x = (init.max.x + delta.x).max(min.x + 30.0);
                                    max.y = (init.max.y + delta.y).max(min.y + 20.0);
                                }
                                DragMode::ResizeS => {
                                    max.y = (init.max.y + delta.y).max(min.y + 20.0);
                                }
                                DragMode::ResizeSW => {
                                    min.x = (init.min.x + delta.x).min(max.x - 30.0);
                                    max.y = (init.max.y + delta.y).max(min.y + 20.0);
                                }
                                DragMode::ResizeW => {
                                    min.x = (init.min.x + delta.x).min(max.x - 30.0);
                                }
                                _ => {}
                            }

                            let new_rect = Rect::from_min_max(min, max);
                            self.current_rect = Some(new_rect);
                            self.sync_rect_to_config(new_rect, ppp, config);
                        }
                    }
                }
            }
        } else if pointer.primary_released() && self.drag_mode != DragMode::None {
            if let Some(r) = self.current_rect {
                // Eger yeni cizim cok kucukse eski haline dondur
                if r.width() < 30.0 || r.height() < 15.0 {
                    if let Some(init) = self.initial_rect {
                        self.current_rect = Some(init);
                        self.sync_rect_to_config(init, ppp, config);
                    }
                } else {
                    self.sync_rect_to_config(r, ppp, config);
                    let _ = config.save();
                }
            }
            self.drag_mode = DragMode::None;
            self.drag_start_pos = None;
        }

        let painter = ui.painter();

        // 1. Secim kutusu disinda kalan alanlari ayarlanabilir seffaflikla ciz (Cutout/Delik teknigi)
        // boylece secilen alanin arkasi (oyun ekrani) 100% net gorulur!
        let backdrop_alpha = (self.backdrop_opacity.clamp(0.0, 0.95) * 255.0) as u8;
        let backdrop_color = Color32::from_black_alpha(backdrop_alpha);

        if let Some(rect) = self.current_rect {
            if backdrop_alpha > 0 {
                // Ust bant
                if rect.min.y > screen_rect.min.y {
                    painter.rect_filled(
                        Rect::from_min_max(
                            screen_rect.min,
                            Pos2::new(screen_rect.max.x, rect.min.y),
                        ),
                        CornerRadius::ZERO,
                        backdrop_color,
                    );
                }
                // Alt bant
                if rect.max.y < screen_rect.max.y {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(screen_rect.min.x, rect.max.y),
                            screen_rect.max,
                        ),
                        CornerRadius::ZERO,
                        backdrop_color,
                    );
                }
                // Sol bant
                if rect.min.x > screen_rect.min.x {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(screen_rect.min.x, rect.min.y),
                            Pos2::new(rect.min.x, rect.max.y),
                        ),
                        CornerRadius::ZERO,
                        backdrop_color,
                    );
                }
                // Sag bant
                if rect.max.x < screen_rect.max.x {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(rect.max.x, rect.min.y),
                            Pos2::new(screen_rect.max.x, rect.max.y),
                        ),
                        CornerRadius::ZERO,
                        backdrop_color,
                    );
                }
            }

            // 2. Secim kutusunun kendi ic seffafligi
            let box_alpha = (self.box_fill_opacity.clamp(0.0, 0.90) * 255.0) as u8;
            if box_alpha > 0 {
                painter.rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    Color32::from_rgba_unmultiplied(0, 122, 204, box_alpha),
                );
            }

            // 3. Secim kutusu kenarligi ve tutamaclar (Handles)
            painter.rect_stroke(
                rect,
                CornerRadius::ZERO,
                Stroke::new(2.0_f32, Color32::from_rgb(0x00, 0x98, 0xff)),
                StrokeKind::Outside,
            );

            // 8 adet boyutlandirma tutamaci (kose ve kenar ortalari)
            let handle_positions = [
                rect.left_top(),
                Pos2::new(rect.center().x, rect.min.y),
                rect.right_top(),
                Pos2::new(rect.max.x, rect.center().y),
                rect.right_bottom(),
                Pos2::new(rect.center().x, rect.max.y),
                rect.left_bottom(),
                Pos2::new(rect.min.x, rect.center().y),
            ];

            for hp in handle_positions {
                let h_rect = Rect::from_center_size(hp, Vec2::splat(HANDLE_SIZE * 2.0));
                painter.rect_filled(
                    h_rect,
                    CornerRadius::same(2),
                    Color32::from_rgb(0x00, 0x98, 0xff),
                );
                painter.rect_stroke(
                    h_rect,
                    CornerRadius::same(2),
                    Stroke::new(1.0_f32, Color32::WHITE),
                    StrokeKind::Inside,
                );
            }

            // Koordinat ve boyut bilgisi etiketi (Fiziksel ekran pikseli)
            let info_text = format!(
                "X: {}  Y: {}  |  Genislik: {} px  Yukseklik: {} px  (Tasi / Kenarlardan ve Tekerlekle Boyutlandir)",
                (rect.min.x.max(0.0) * ppp).round() as i32,
                (rect.min.y.max(0.0) * ppp).round() as i32,
                (rect.width() * ppp).round() as i32,
                (rect.height() * ppp).round() as i32
            );
            let label_pos = Pos2::new(
                rect.min.x.clamp(10.0, (screen_rect.max.x - 520.0).max(10.0)),
                if rect.min.y > 105.0 {
                    rect.min.y - 26.0
                } else {
                    rect.max.y + 8.0
                },
            );
            let galley = painter.layout_no_wrap(
                info_text,
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            let bg_label_rect = Rect::from_min_size(
                label_pos - Vec2::new(6.0, 3.0),
                galley.size() + Vec2::new(12.0, 6.0),
            );
            painter.rect_filled(
                bg_label_rect,
                CornerRadius::same(4),
                Color32::from_black_alpha(190),
            );
            painter.galley(label_pos, galley, Color32::WHITE);
        } else if backdrop_alpha > 0 {
            painter.rect_filled(screen_rect, CornerRadius::ZERO, backdrop_color);
        }

        // 4. Ust Kontrol ve Seffaflik Ayar Cubugu
        painter.rect_filled(top_bar_rect, CornerRadius::same(6), COLOR_BG_CARD);
        painter.rect_stroke(
            top_bar_rect,
            CornerRadius::same(6),
            Stroke::new(1.5_f32, COLOR_BORDER_CARD),
            StrokeKind::Outside,
        );

        ui.allocate_new_ui(UiBuilder::new().max_rect(top_bar_rect.shrink(8.0)), |ui| {
            ui.style_mut().spacing.slider_width = 75.0;
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(
                            "Altyazi Bolgesi: Kutu icinden tasiyin, kenarlardan veya fare tekerlegiyle boyutlandirin",
                        )
                        .size(12.0)
                        .strong()
                        .color(COLOR_TEXT_BRIGHT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Iptal (ESC)").size(11.5))
                            .clicked()
                        {
                            *is_open = false;
                            self.drag_mode = DragMode::None;
                        }

                        ui.add_space(4.0);

                        if ui
                            .button(
                                RichText::new("Onayla ve Kaydet (ENTER / Cift Tik)")
                                    .size(11.5)
                                    .strong()
                                    .color(COLOR_STATUS_OK),
                            )
                            .clicked()
                        {
                            if let Some(r) = self.current_rect {
                                self.apply_and_close(r, ppp, config, is_open);
                            }
                        }
                    });
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Dis Karartma:")
                            .size(11.0)
                            .color(COLOR_TEXT_NORMAL),
                    );
                    let mut bg_pct = (self.backdrop_opacity * 100.0) as u32;
                    if ui
                        .add(egui::Slider::new(&mut bg_pct, 0..=85).text("%"))
                        .changed()
                    {
                        self.backdrop_opacity = bg_pct as f32 / 100.0;
                    }

                    if ui.button(RichText::new("Tam Seffaf").size(10.5)).clicked() {
                        self.backdrop_opacity = 0.0;
                        self.box_fill_opacity = 0.0;
                    }

                    ui.separator();

                    ui.label(
                        RichText::new("Kutu Ici:")
                            .size(11.0)
                            .color(COLOR_TEXT_NORMAL),
                    );
                    let mut box_pct = (self.box_fill_opacity * 100.0) as u32;
                    if ui
                        .add(egui::Slider::new(&mut box_pct, 0..=60).text("%"))
                        .changed()
                    {
                        self.box_fill_opacity = box_pct as f32 / 100.0;
                    }

                    ui.separator();

                    ui.label(
                        RichText::new("Ceviri Arka Plani:")
                            .size(11.0)
                            .color(COLOR_ACCENT_BLUE),
                    );
                    let mut ov_pct = (config.overlay.bg_opacity * 100.0) as u32;
                    if ui
                        .add(egui::Slider::new(&mut ov_pct, 0..=100).text("%"))
                        .changed()
                    {
                        config.overlay.bg_opacity = ov_pct as f32 / 100.0;
                    }
                });
            });
        });
    }

    fn apply_and_close(
        &mut self,
        rect: Rect,
        ppp: f32,
        config: &mut AppConfig,
        is_open: &mut bool,
    ) {
        if rect.width() >= 20.0 && rect.height() >= 10.0 {
            let phys_x = (rect.min.x.max(0.0) * ppp).round() as u32;
            let phys_y = (rect.min.y.max(0.0) * ppp).round() as u32;
            let phys_w = (rect.width() * ppp).round() as u32;
            let phys_h = (rect.height() * ppp).round() as u32;

            config.roi = SubtitleRoi {
                x: phys_x,
                y: phys_y,
                width: phys_w,
                height: phys_h,
            };

            // Kullanici kutuyu cizip onayladiginda, ceviri altyazisi katmanini da
            // varsayilan olarak ayni konuma ve boyuta hizala (ekran disinda kalmasin!)
            config.overlay.window_x = rect.min.x.max(0.0);
            config.overlay.window_y = rect.min.y.max(0.0);
            config.overlay.window_w = rect.width().max(300.0);
            config.overlay.window_h = rect.height().max(90.0);

            let _ = config.save();
        }
        *is_open = false;
        self.drag_mode = DragMode::None;
        self.drag_start_pos = None;
    }
}
