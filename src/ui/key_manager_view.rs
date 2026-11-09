use crate::config::{AppConfig, KeyStatus, PreferredEngine, MAX_DEEPL_KEYS};
use crate::theme::{
    COLOR_BG_CARD, COLOR_BORDER_CARD, COLOR_STATUS_ERR, COLOR_STATUS_OK, COLOR_STATUS_WARN,
    COLOR_TEXT_BRIGHT, COLOR_TEXT_MUTED, COLOR_TEXT_NORMAL,
};
use egui::{RichText, Ui};

pub struct KeyManagerView {
    pub show_raw_keys: [bool; MAX_DEEPL_KEYS],
    pub test_result_message: Option<(usize, Result<String, String>)>,
}

impl Default for KeyManagerView {
    fn default() -> Self {
        Self {
            show_raw_keys: [false; MAX_DEEPL_KEYS],
            test_result_message: None,
        }
    }
}

impl KeyManagerView {
    pub fn render(
        &mut self,
        ui: &mut Ui,
        config: &mut AppConfig,
        test_trigger: &mut Option<(usize, String)>,
    ) {
        ui.vertical(|ui| {
            // Motor Secenegi tek satirda kompakt
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Motor:")
                        .size(11.5)
                        .strong()
                        .color(COLOR_TEXT_BRIGHT),
                );
                if ui
                    .radio_value(
                        &mut config.preferred_engine,
                        PreferredEngine::DeepLFirst,
                        RichText::new("DeepL + Google Yedek")
                            .size(11.0)
                            .color(COLOR_TEXT_NORMAL),
                    )
                    .changed()
                {
                    config.fallback_to_google = true;
                    let _ = config.save();
                }
                if ui
                    .radio_value(
                        &mut config.preferred_engine,
                        PreferredEngine::GoogleOnly,
                        RichText::new("Sadece Google Translate")
                            .size(11.0)
                            .color(COLOR_TEXT_NORMAL),
                    )
                    .changed()
                {
                    let _ = config.save();
                }
                if ui
                    .radio_value(
                        &mut config.preferred_engine,
                        PreferredEngine::DeepLOnly,
                        RichText::new("Sadece DeepL")
                            .size(11.0)
                            .color(COLOR_TEXT_NORMAL),
                    )
                    .changed()
                {
                    config.fallback_to_google = false;
                    let _ = config.save();
                }
            });

            ui.add_space(4.0);

            // Tablo Cercevesi
            egui::Frame::NONE
                .fill(COLOR_BG_CARD)
                .stroke(egui::Stroke::new(1.0_f32, COLOR_BORDER_CARD))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    // 10 Slotun Satirlari (Kompakt)
                    let mut should_save = false;
                    for i in 0..config.deepl_keys.len() {
                        let is_active = i == config.active_key_index;
                        let key_entry = &mut config.deepl_keys[i];

                        ui.horizontal(|ui| {
                            ui.set_min_height(20.0);

                            let slot_label = if is_active {
                                RichText::new(format!("#{}*", i + 1))
                                    .size(11.0)
                                    .strong()
                                    .color(COLOR_STATUS_OK)
                            } else {
                                RichText::new(format!("#{}", i + 1))
                                    .size(11.0)
                                    .color(COLOR_TEXT_MUTED)
                            };
                            ui.add_sized([28.0, 19.0], egui::Label::new(slot_label));

                            if ui
                                .add_sized(
                                    [75.0, 19.0],
                                    egui::TextEdit::singleline(&mut key_entry.label)
                                        .hint_text(format!("Slot {}", i + 1)),
                                )
                                .changed()
                            {
                                should_save = true;
                            }

                            let show_raw = self.show_raw_keys[i];
                            let key_resp = ui.add_sized(
                                [175.0, 19.0],
                                egui::TextEdit::singleline(&mut key_entry.key)
                                    .password(!show_raw)
                                    .hint_text("DeepL API Key..."),
                            );
                            if key_resp.changed() {
                                should_save = true;
                            }

                            let toggle_btn = if show_raw { "Gizle" } else { "Goster" };
                            if ui
                                .add_sized(
                                    [42.0, 19.0],
                                    egui::Button::new(RichText::new(toggle_btn).size(10.0)),
                                )
                                .clicked()
                            {
                                self.show_raw_keys[i] = !self.show_raw_keys[i];
                            }

                            let (status_text, status_color) = match key_entry.status {
                                KeyStatus::Active => ("AKTIF", COLOR_STATUS_OK),
                                KeyStatus::Ready => ("HAZIR", COLOR_TEXT_NORMAL),
                                KeyStatus::QuotaExceeded => ("KOTA DOLDU", COLOR_STATUS_WARN),
                                KeyStatus::InvalidAuth => ("GECERSIZ", COLOR_STATUS_ERR),
                                KeyStatus::NetworkError => ("AG HATASI", COLOR_STATUS_ERR),
                                KeyStatus::Untested => {
                                    if key_entry.key.trim().is_empty() {
                                        ("BOS", COLOR_TEXT_MUTED)
                                    } else {
                                        ("BEKLIYOR", COLOR_TEXT_MUTED)
                                    }
                                }
                            };
                            ui.add_sized(
                                [75.0, 19.0],
                                egui::Label::new(
                                    RichText::new(status_text)
                                        .size(10.0)
                                        .strong()
                                        .color(status_color),
                                ),
                            );

                            ui.add_sized(
                                [68.0, 19.0],
                                egui::Label::new(
                                    RichText::new(format!("{} harf", key_entry.characters_used))
                                        .size(10.0)
                                        .color(COLOR_TEXT_MUTED),
                                ),
                            );

                            let is_empty = key_entry.key.trim().is_empty();
                            if ui
                                .add_enabled(
                                    !is_empty,
                                    egui::Button::new(RichText::new("Test").size(10.0)),
                                )
                                .clicked()
                            {
                                *test_trigger = Some((i, key_entry.key.clone()));
                            }

                            if ui
                                .add_enabled(
                                    !is_empty,
                                    egui::Button::new(RichText::new("Sil").size(10.0)),
                                )
                                .clicked()
                            {
                                key_entry.key.clear();
                                key_entry.status = KeyStatus::Untested;
                                key_entry.characters_used = 0;
                                should_save = true;
                            }
                        });
                    }
                    if should_save {
                        let _ = config.save();
                    }
                });

            if let Some((slot_idx, res)) = &self.test_result_message {
                ui.add_space(4.0);
                match res {
                    Ok(msg) => {
                        ui.label(
                            RichText::new(format!("Slot {}: {}", slot_idx + 1, msg))
                                .size(11.0)
                                .color(COLOR_STATUS_OK),
                        );
                    }
                    Err(err) => {
                        ui.label(
                            RichText::new(format!("Slot {}: {}", slot_idx + 1, err))
                                .size(11.0)
                                .color(COLOR_STATUS_ERR),
                        );
                    }
                }
            }
        });
    }
}
