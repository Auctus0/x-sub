use crate::config::{AppConfig, SubtitleRoi};
use crate::theme::COLOR_TEXT_MUTED;
use egui::{Color32, CornerRadius, Margin, Stroke, Ui, ViewportCommand};

pub struct SubtitleOverlay;

impl SubtitleOverlay {
    fn sync_overlay_to_roi(config: &mut AppConfig, ppp: f32) {
        let scale = ppp.max(1.0);
        config.roi = SubtitleRoi {
            x: (config.overlay.window_x.max(0.0) * scale).round() as u32,
            y: (config.overlay.window_y.max(0.0) * scale).round() as u32,
            width: (config.overlay.window_w.max(60.0) * scale).round() as u32,
            height: (config.overlay.window_h.max(30.0) * scale).round() as u32,
        };
    }

    pub fn render(
        ui: &mut Ui,
        ctx: &egui::Context,
        config: &mut AppConfig,
        current_subtitle: &str,
    ) {
        let mut should_save = false;
        let ppp = ctx.pixels_per_point().max(1.0);

        // Pencere metin uzerinden suruklendiginde veya fare tekerlegiyle boyutlandirildiginda
        // hem overlay hem de OCR okuma bolgesini (config.roi) her zaman anlik senkronize et!
        if let Some(r) = ctx.input(|i| i.viewport().outer_rect) {
            if (config.overlay.window_x - r.min.x).abs() > 0.5
                || (config.overlay.window_y - r.min.y).abs() > 0.5
            {
                config.overlay.window_x = r.min.x.max(0.0);
                config.overlay.window_y = r.min.y.max(0.0);
                Self::sync_overlay_to_roi(config, ppp);
                should_save = true;
            }
        }

        let scroll_y = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() > 0.1 {
            let shift_held = ctx.input(|i| i.modifiers.shift);
            if shift_held {
                config.overlay.font_size =
                    (config.overlay.font_size + scroll_y * 0.1).clamp(12.0, 64.0);
            } else {
                config.overlay.window_w =
                    (config.overlay.window_w + scroll_y * 1.5).clamp(200.0, 3840.0);
                config.overlay.window_h =
                    (config.overlay.window_h + scroll_y * 0.6).clamp(50.0, 1080.0);
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::Vec2::new(
                    config.overlay.window_w,
                    config.overlay.window_h,
                )));
                Self::sync_overlay_to_roi(config, ppp);
            }
            should_save = true;
        }

        let bg_alpha = (config.overlay.bg_opacity.clamp(0.0, 1.0) * 255.0) as u8;
        let bg_color = Color32::from_rgba_unmultiplied(
            config.overlay.bg_color_rgb[0],
            config.overlay.bg_color_rgb[1],
            config.overlay.bg_color_rgb[2],
            bg_alpha,
        );

        let text_color = Color32::from_rgb(
            config.overlay.text_color_rgb[0],
            config.overlay.text_color_rgb[1],
            config.overlay.text_color_rgb[2],
        );

        let is_empty = current_subtitle.trim().is_empty();

        // Eger altyazi bos ise ve kilitli oyun modundaysak ekranda hicbir sey cizme (%100 saydam/gorunmez)
        if is_empty && config.overlay.is_locked {
            return;
        }

        // Sinematik Altyazi Kutusu (Arka plan %0 seffafsa hicbir dikdortgen/cerceve doldurma!)
        let effective_fill = if bg_alpha == 0 {
            Color32::TRANSPARENT
        } else {
            bg_color
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                // Hicbir cerceve (Stroke::NONE) veya ust bar cizmeden dogrudan metni goster
                let frame_resp = egui::Frame::NONE
                    .fill(effective_fill)
                    .corner_radius(CornerRadius::same(6))
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            if is_empty {
                                let dragged = Self::draw_clean_subtitle_text(
                                    ui,
                                    "Altyazi Alani (Metinden tutup surukleyin, tekerlekle boyutlandirin)",
                                    config.overlay.font_size,
                                    COLOR_TEXT_MUTED,
                                    bg_alpha,
                                );
                                if dragged {
                                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                                }
                            } else {
                                let dragged = Self::draw_clean_subtitle_text(
                                    ui,
                                    current_subtitle,
                                    config.overlay.font_size,
                                    text_color,
                                    bg_alpha,
                                );
                                if dragged {
                                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                                }
                            }
                        });
                    });

                // Metin veya kutu uzerinden her zaman dogrudan tutup surukleme imkani
                let drag_area = ui.interact(
                    frame_resp.response.rect,
                    egui::Id::new("subtitle_box_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_area.hovered() || drag_area.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::Grab);
                }
                if drag_area.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
            });

        if should_save {
            let _ = config.save();
        }
    }

    /// Seffaf (%0) katman uzerinde hicbir cerceve olmadan temiz metin cizer
    /// ve metnin dogrudan fareyle tutulup suruklenmesini saglar.
    fn draw_clean_subtitle_text(
        ui: &mut Ui,
        text: &str,
        font_size: f32,
        text_color: Color32,
        bg_alpha: u8,
    ) -> bool {
        let font_id = egui::FontId::new(
            font_size.round(),
            egui::FontFamily::Name("subtitle_bold".into()),
        );
        let max_width = ui.available_width().max(120.0);

        let galley = ui
            .painter()
            .layout(text.to_string(), font_id, Color32::WHITE, max_width);
        let (rect, resp) = ui.allocate_exact_size(galley.size(), egui::Sense::click_and_drag());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        let painter = ui.painter();

        // Tam sayi piksel koordinati (keskin ve bozulmasiz glif kenarlari icin)
        let pos = egui::pos2(rect.min.x.round(), rect.min.y.round());

        // Sadece eger kullanici cok dusuk bir opaklik (%1 - %20 arasi) sectiyse satir bazli kapsul ciz,
        // %0 (Tam Seffaf) sectiginde ise HICBIR siyah/gri kutu cizme!
        if bg_alpha > 0 && bg_alpha < 50 {
            for row in &galley.rows {
                if row.rect.width() > 1.0 {
                    let row_screen_rect = row
                        .rect
                        .translate(pos.to_vec2())
                        .expand2(egui::vec2(10.0, 3.0));
                    painter.rect_filled(
                        row_screen_rect,
                        CornerRadius::same(5),
                        Color32::from_black_alpha(bg_alpha * 3),
                    );
                }
            }
        }

        // 1. Tek katmanli net alt golge (ust uste binip fontu kalinlastirmaz / bozmaz)
        painter.galley(
            pos + egui::vec2(1.0, 1.0),
            galley.clone(),
            Color32::from_rgba_premultiplied(0, 0, 0, 255),
        );

        // 2. Ustte ana parlak metin (tek gecis, puruzsuz anti-aliasing)
        painter.galley(pos, galley, text_color);

        resp.drag_started()
    }
}
