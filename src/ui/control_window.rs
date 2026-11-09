use crate::capture::{MonitorInfo, WindowInfo};
use crate::config::{AppConfig, PreferredEngine, SubtitleRoi};
use crate::emulator::scenes::{get_standard_game_scenes, GameScene};
use crate::ocr::WinMediaOcr;
use crate::theme::{
    COLOR_ACCENT_BLUE, COLOR_BG_CARD, COLOR_BORDER_CARD, COLOR_STATUS_ERR, COLOR_STATUS_OK,
    COLOR_STATUS_WARN, COLOR_TEXT_BRIGHT, COLOR_TEXT_MUTED, COLOR_TEXT_NORMAL,
};
use crate::ui::key_manager_view::KeyManagerView;
use egui::{Color32, RichText, Stroke, Ui};
use std::process::Command;

#[derive(PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    DeepLPool,
    OverlaySettings,
    SystemOcr,
    Emulator,
}

pub struct ControlWindow {
    pub active_tab: ActiveTab,
    pub key_manager_view: KeyManagerView,
    pub emulator_scenes: Vec<GameScene>,
    pub current_scene_index: usize,
}

impl Default for ControlWindow {
    fn default() -> Self {
        Self {
            active_tab: ActiveTab::Dashboard,
            key_manager_view: KeyManagerView::default(),
            emulator_scenes: get_standard_game_scenes(),
            current_scene_index: 0,
        }
    }
}

pub struct DashboardState<'a> {
    pub is_running: bool,
    pub original_text: &'a str,
    pub translated_text: &'a str,
    pub last_engine_used: &'a str,
    pub ocr_latency_ms: u64,
    pub trans_latency_ms: u64,
    pub monitors: &'a [MonitorInfo],
    pub windows: &'a [WindowInfo],
}

impl ControlWindow {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ui: &mut Ui,
        config: &mut AppConfig,
        state: &DashboardState,
        toggle_running: &mut bool,
        open_roi_selector: &mut bool,
        test_trigger: &mut Option<(usize, String)>,
        clear_subtitle_trigger: &mut bool,
    ) {
        ui.vertical(|ui| {
            // 1. Kompakt Ust Baslik ve Sekme Navigasyonu
            self.render_header_and_tabs(ui, state, toggle_running);

            ui.separator();
            ui.add_space(2.0);

            // 2. Secili Sekme Icerigi (Kaydirilabilir alan ile dar pencerelerde de tasmaz)
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.active_tab {
                    ActiveTab::Dashboard => {
                        self.render_dashboard(
                            ui,
                            config,
                            state,
                            open_roi_selector,
                            clear_subtitle_trigger,
                        );
                    }
                    ActiveTab::DeepLPool => {
                        self.key_manager_view.render(ui, config, test_trigger);
                    }
                    ActiveTab::OverlaySettings => {
                        self.render_overlay_settings(ui, config);
                    }
                    ActiveTab::SystemOcr => {
                        self.render_system_ocr(ui);
                    }
                    ActiveTab::Emulator => {
                        self.render_emulator_tab(ui);
                    }
                });
        });
    }

    fn render_header_and_tabs(
        &mut self,
        ui: &mut Ui,
        state: &DashboardState,
        toggle_running: &mut bool,
    ) {
        ui.horizontal(|ui| {
            // Baslat / Durdur Butonu
            let (btn_text, btn_color) = if state.is_running {
                ("DURDUR", COLOR_STATUS_ERR)
            } else {
                ("BASLAT", COLOR_STATUS_OK)
            };

            if ui
                .add_sized(
                    [68.0, 22.0],
                    egui::Button::new(RichText::new(btn_text).size(11.5).strong().color(btn_color)),
                )
                .clicked()
            {
                *toggle_running = true;
            }

            ui.add_space(4.0);

            // Sekmeler tek satirda yan yana kompakt
            let tabs = [
                (ActiveTab::Dashboard, "PANEL"),
                (ActiveTab::OverlaySettings, "GORUNUM"),
                (ActiveTab::DeepLPool, "DEEPL (10)"),
                (ActiveTab::Emulator, "TEST"),
                (ActiveTab::SystemOcr, "OCR"),
            ];

            for (tab, name) in tabs {
                let is_selected = self.active_tab == tab;
                let text = if is_selected {
                    RichText::new(name)
                        .size(11.5)
                        .strong()
                        .color(COLOR_ACCENT_BLUE)
                } else {
                    RichText::new(name).size(11.5).color(COLOR_TEXT_NORMAL)
                };

                if ui.selectable_label(is_selected, text).clicked() {
                    self.active_tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.is_running {
                    let total_ms = state.ocr_latency_ms + state.trans_latency_ms;
                    ui.label(
                        RichText::new(format!(
                            "{}ms (OCR:{} | Cev:{})",
                            total_ms, state.ocr_latency_ms, state.trans_latency_ms
                        ))
                        .size(10.5)
                        .color(COLOR_STATUS_OK),
                    );
                } else {
                    ui.label(
                        RichText::new("Hazir")
                            .size(10.5)
                            .color(COLOR_TEXT_MUTED),
                    );
                }
            });
        });
    }

    fn render_dashboard(
        &self,
        ui: &mut Ui,
        config: &mut AppConfig,
        state: &DashboardState,
        open_roi_selector: &mut bool,
        clear_subtitle_trigger: &mut bool,
    ) {
        ui.vertical(|ui| {
            // 1. Kompakt Ayar Karti (Kaynak, Bolge, Motor tek bir sikistirilmis kutuda)
            egui::Frame::NONE
                .fill(COLOR_BG_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER_CARD))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    // Satir 1: Kaynak secimi + Motor secimi
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Ekran:").size(11.0).color(COLOR_TEXT_MUTED));
                        let current_monitor_name = state
                            .monitors
                            .get(config.target_monitor_index)
                            .map(|m| format!("#{} {}x{}", m.index, m.width, m.height))
                            .unwrap_or_else(|| "Sec".to_string());

                        egui::ComboBox::from_id_salt("monitor_select")
                            .width(90.0)
                            .selected_text(RichText::new(current_monitor_name).size(11.0))
                            .show_ui(ui, |ui| {
                                for m in state.monitors {
                                    let label = format!("[{}] {} ({}x{})", m.index, m.name, m.width, m.height);
                                    if ui.selectable_value(&mut config.target_monitor_index, m.index, label).clicked() {
                                        config.target_window_title = None;
                                    }
                                }
                            });

                        ui.label(RichText::new("Pencere:").size(11.0).color(COLOR_TEXT_MUTED));
                        let raw_win_name = config
                            .target_window_title
                            .as_deref()
                            .unwrap_or("Tam Ekran");
                        let short_win_name = if raw_win_name.chars().count() > 16 {
                            format!("{}...", raw_win_name.chars().take(15).collect::<String>())
                        } else {
                            raw_win_name.to_string()
                        };

                        egui::ComboBox::from_id_salt("window_select")
                            .width(115.0)
                            .selected_text(RichText::new(short_win_name).size(11.0))
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(config.target_window_title.is_none(), "Tam Ekran Monitor").clicked() {
                                    config.target_window_title = None;
                                }
                                for w in state.windows {
                                    let is_sel = config.target_window_title.as_ref() == Some(&w.title);
                                    if ui.selectable_label(is_sel, &w.title).clicked() {
                                        config.target_window_title = Some(w.title.clone());
                                    }
                                }
                            });

                        ui.separator();

                        ui.label(RichText::new("Motor:").size(11.0).color(COLOR_TEXT_MUTED));
                        let engine_label = match config.preferred_engine {
                            PreferredEngine::DeepLFirst => "DeepL + Google (Onerilen)",
                            PreferredEngine::GoogleOnly => "Sadece Google Translate",
                            PreferredEngine::DeepLOnly => "Sadece DeepL",
                        };
                        egui::ComboBox::from_id_salt("engine_select")
                            .width(150.0)
                            .selected_text(RichText::new(engine_label).size(11.0).color(COLOR_TEXT_BRIGHT))
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(
                                    &mut config.preferred_engine,
                                    PreferredEngine::DeepLFirst,
                                    "DeepL + Google Yedek (Onerilen)",
                                ).clicked() {
                                    config.fallback_to_google = true;
                                    let _ = config.save();
                                }
                                if ui.selectable_value(
                                    &mut config.preferred_engine,
                                    PreferredEngine::GoogleOnly,
                                    "Sadece Google Translate (Ucretsiz)",
                                ).clicked() {
                                    let _ = config.save();
                                }
                                if ui.selectable_value(
                                    &mut config.preferred_engine,
                                    PreferredEngine::DeepLOnly,
                                    "Sadece DeepL",
                                ).clicked() {
                                    config.fallback_to_google = false;
                                    let _ = config.save();
                                }
                            });
                    });

                    ui.add_space(4.0);

                    // Satir 2: Altyazi Okuma Bolgesi Secimi
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("Altyazi Alanini Ekranda Ciz / Boyutlandir")
                                    .size(11.0)
                                    .strong()
                                    .color(COLOR_ACCENT_BLUE),
                            )
                            .clicked()
                        {
                            *open_roi_selector = true;
                        }

                        ui.label(
                            RichText::new(format!(
                                "X:{} Y:{} ({}x{})",
                                config.roi.x, config.roi.y, config.roi.width, config.roi.height
                            ))
                            .size(11.0)
                            .color(COLOR_STATUS_OK),
                        );

                        let ppp = ui.ctx().pixels_per_point().max(1.0);
                        if ui.button(RichText::new("1080p Sablon").size(10.5)).clicked() {
                            config.roi = SubtitleRoi { x: 360, y: 860, width: 1200, height: 180 };
                            config.overlay.window_x = 360.0 / ppp;
                            config.overlay.window_y = 860.0 / ppp;
                            config.overlay.window_w = 1200.0 / ppp;
                            config.overlay.window_h = (180.0 / ppp).max(90.0);
                            let _ = config.save();
                        }
                        if ui.button(RichText::new("1440p Sablon").size(10.5)).clicked() {
                            config.roi = SubtitleRoi { x: 480, y: 1150, width: 1600, height: 240 };
                            config.overlay.window_x = 480.0 / ppp;
                            config.overlay.window_y = 1150.0 / ppp;
                            config.overlay.window_w = 1600.0 / ppp;
                            config.overlay.window_h = (240.0 / ppp).max(90.0);
                            let _ = config.save();
                        }
                    });

                    ui.add_space(4.0);

                    // Satir 3: Ceviri Altyazi Katmani Hizli Kontrolu (Konum & Seffaflik)
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Ceviri Katmani:").size(11.0).color(COLOR_TEXT_MUTED));
                        let (lock_btn, lock_color) = if config.overlay.is_locked {
                            ("Konumu Tasi / Boyutlandir", COLOR_TEXT_BRIGHT)
                        } else {
                            ("Konumu Kilitle (Duzenleme Acik)", COLOR_STATUS_OK)
                        };
                        if ui
                            .button(RichText::new(lock_btn).size(10.5).color(lock_color))
                            .clicked()
                        {
                            config.overlay.is_locked = !config.overlay.is_locked;
                            let _ = config.save();
                        }

                        ui.separator();

                        ui.label(RichText::new("Arka Plan:").size(11.0).color(COLOR_TEXT_MUTED));
                        let mut ov_pct = (config.overlay.bg_opacity * 100.0) as u32;
                        ui.style_mut().spacing.slider_width = 70.0;
                        if ui
                            .add(egui::Slider::new(&mut ov_pct, 0..=100).text("%"))
                            .changed()
                        {
                            config.overlay.bg_opacity = ov_pct as f32 / 100.0;
                            let _ = config.save();
                        }
                        if ui.button(RichText::new("Tam Seffaf").size(10.5)).clicked() {
                            config.overlay.bg_opacity = 0.0;
                            let _ = config.save();
                        }

                        ui.separator();

                        ui.label(RichText::new("Yazi:").size(11.0).color(COLOR_TEXT_MUTED));
                        ui.style_mut().spacing.slider_width = 55.0;
                        if ui
                            .add(egui::Slider::new(&mut config.overlay.font_size, 14.0..=48.0).text("pt"))
                            .changed()
                        {
                            let _ = config.save();
                        }
                    });
                });

            ui.add_space(4.0);

            // 2. Kompakt Canli Metin Akis Kutusu
            egui::Frame::NONE
                .fill(COLOR_BG_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER_CARD))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    let is_text_empty = state.original_text.trim().is_empty();

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("CANLI CEVIRI")
                                .size(11.0)
                                .strong()
                                .color(COLOR_TEXT_BRIGHT),
                        );

                        if state.is_running {
                            if is_text_empty {
                                ui.label(
                                    RichText::new("[Altyazi Bekleniyor]")
                                        .size(10.5)
                                        .color(COLOR_TEXT_MUTED),
                                );
                            } else {
                                ui.label(
                                    RichText::new("[Aktif]")
                                        .size(10.5)
                                        .strong()
                                        .color(COLOR_STATUS_OK),
                                );
                            }
                        } else {
                            ui.label(
                                RichText::new("[Durduruldu]")
                                    .size(10.5)
                                    .color(COLOR_STATUS_WARN),
                            );
                        }

                        if !state.last_engine_used.is_empty() {
                            ui.label(
                                RichText::new(format!("({})", state.last_engine_used))
                                    .size(10.5)
                                    .color(COLOR_TEXT_MUTED),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(RichText::new("Temizle").size(10.0)).clicked() {
                                *clear_subtitle_trigger = true;
                            }
                        });
                    });

                    ui.add_space(2.0);

                    // Orijinal Metin (Kompakt)
                    let mut orig = if is_text_empty {
                        if state.is_running {
                            "Oyun ici altyazi algilandiginda burada gorunecek...".to_string()
                        } else {
                            "Baslatmak icin sol ustteki 'BASLAT' butonuna basin.".to_string()
                        }
                    } else {
                        state.original_text.to_string()
                    };

                    ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::TextEdit::multiline(&mut orig)
                            .font(egui::FontId::proportional(12.5))
                            .text_color(if is_text_empty {
                                COLOR_TEXT_MUTED
                            } else {
                                COLOR_TEXT_NORMAL
                            })
                            .interactive(false),
                    );

                    ui.add_space(2.0);

                    // Turkce Ceviri (Kompakt)
                    let mut trans = if is_text_empty {
                        "---".to_string()
                    } else {
                        state.translated_text.to_string()
                    };

                    ui.add_sized(
                        [ui.available_width(), 44.0],
                        egui::TextEdit::multiline(&mut trans)
                            .font(egui::FontId::proportional(13.5))
                            .text_color(if is_text_empty {
                                COLOR_TEXT_MUTED
                            } else {
                                Color32::WHITE
                            })
                            .interactive(false),
                    );
                });
        });
    }

    fn render_emulator_tab(&mut self, ui: &mut Ui) {
        let total_scenes = self.emulator_scenes.len();
        if total_scenes == 0 {
            return;
        }

        let scene = &self.emulator_scenes[self.current_scene_index];

        egui::Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER_CARD))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Sahne {}/{}: {} ({})",
                            self.current_scene_index + 1,
                            total_scenes,
                            scene.game_name,
                            scene.character
                        ))
                        .size(11.5)
                        .strong()
                        .color(COLOR_STATUS_OK),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("Canli Oyun Penceresi Ac")
                                    .size(11.0)
                                    .strong()
                                    .color(COLOR_ACCENT_BLUE),
                            )
                            .clicked()
                        {
                            if let Ok(exe_path) = std::env::current_exe() {
                                let _ = Command::new(exe_path).arg("--game-window").spawn();
                            }
                        }
                        if ui.button(RichText::new("Sonraki ->").size(11.0)).clicked() {
                            self.current_scene_index = (self.current_scene_index + 1) % total_scenes;
                        }
                        if ui.button(RichText::new("<- Onceki").size(11.0)).clicked() {
                            if self.current_scene_index == 0 {
                                self.current_scene_index = total_scenes - 1;
                            } else {
                                self.current_scene_index -= 1;
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                egui::Frame::NONE
                    .fill(Color32::from_rgb(12, 12, 14))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x3e, 0x3e, 0x42)))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let display_text =
                                format!("{}: \"{}\"", scene.character, scene.english_dialog);
                            ui.label(
                                RichText::new(display_text).size(14.0).strong().color(
                                    Color32::from_rgb(
                                        scene.text_color[0],
                                        scene.text_color[1],
                                        scene.text_color[2],
                                    ),
                                ),
                            );
                        });
                    });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Beklenen:").size(11.0).color(COLOR_TEXT_MUTED));
                    ui.label(
                        RichText::new(scene.expected_turkish)
                            .size(11.0)
                            .color(COLOR_STATUS_OK),
                    );
                });
            });
    }

    fn render_overlay_settings(&self, ui: &mut Ui, config: &mut AppConfig) {
        let mut changed = false;
        egui::Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER_CARD))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Yazi Boyutu:").size(11.5));
                    if ui
                        .add(egui::Slider::new(&mut config.overlay.font_size, 14.0..=48.0).text("pt"))
                        .changed()
                    {
                        changed = true;
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("Arka Plan Opaklik:").size(11.5));
                    let mut pct = (config.overlay.bg_opacity * 100.0) as u32;
                    if ui.add(egui::Slider::new(&mut pct, 0..=100).text("%")).changed() {
                        config.overlay.bg_opacity = pct as f32 / 100.0;
                        changed = true;
                    }
                    if ui.button(RichText::new("Seffaf").size(10.5)).clicked() {
                        config.overlay.bg_opacity = 0.0;
                        changed = true;
                    }
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut config.overlay.is_locked,
                            RichText::new("Altyaziyi Kilitle (Tiklamalar oyuna gecer)")
                                .size(11.5)
                                .color(COLOR_TEXT_NORMAL),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if config.overlay.is_locked
                        && ui
                            .button(RichText::new("Tasi / Boyutlandir").size(11.0))
                            .clicked()
                    {
                        config.overlay.is_locked = false;
                        changed = true;
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut config.auto_clear_subtitles,
                            RichText::new("Altyazi bitince otomatik sil").size(11.5),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label(RichText::new("Silme Gecikmesi:").size(11.0));
                    if ui
                        .add(egui::Slider::new(&mut config.clear_delay_ms, 50..=1500).text("ms"))
                        .changed()
                    {
                        changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Maks. Kalma:").size(11.0));
                    if ui
                        .add(egui::Slider::new(&mut config.max_hold_timeout_secs, 2..=15).text("sn"))
                        .changed()
                    {
                        changed = true;
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("Tarama Hizi:").size(11.0));
                    if ui
                        .add(egui::Slider::new(&mut config.capture_interval_ms, 50..=500).text("ms"))
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        if changed {
            let _ = config.save();
        }
    }

    fn render_system_ocr(&self, ui: &mut Ui) {
        egui::Frame::NONE
            .fill(COLOR_BG_CARD)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER_CARD))
            .inner_margin(8.0)
            .show(ui, |ui| {
                let available_langs = WinMediaOcr::get_available_languages();
                ui.label(
                    RichText::new(format!("OCR Dilleri: {}", available_langs.join(", "))).size(11.5),
                );
                let has_english = available_langs
                    .iter()
                    .any(|l| l.to_lowercase().starts_with("en"));

                if has_english {
                    ui.label(
                        RichText::new("Ingilizce OCR paketi hazir ve aktif.")
                            .size(11.5)
                            .strong()
                            .color(COLOR_STATUS_OK),
                    );
                } else {
                    ui.label(
                        RichText::new("Ingilizce (en-US) Windows OCR paketi yuklu degil!")
                            .size(11.5)
                            .strong()
                            .color(COLOR_STATUS_WARN),
                    );
                    if ui
                        .button(RichText::new("Windows Dil Ayarlarini Ac").size(11.0))
                        .clicked()
                    {
                        let _ = Command::new("cmd")
                            .args(["/c", "start", "ms-settings:regionlanguage"])
                            .spawn();
                    }
                }
            });
    }
}
