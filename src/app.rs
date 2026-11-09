use crate::capture::{MonitorInfo, ScreenCapture, WindowInfo};
use crate::config::{AppConfig, KeyStatus};
use crate::ocr::{
    calculate_image_diff, calculate_text_similarity, clean_ocr_text, compute_dhash,
    hamming_distance, isolate_subtitle_for_ocr, AntiGarbageFilter, WinMediaOcr,
};
use crate::theme::apply_minimal_dark_theme;
use crate::translation::TranslationPool;
use crate::ui::{ControlWindow, DashboardState, RoiSelectorView, SubtitleOverlay};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use image::RgbaImage;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub enum WorkerCommand {
    UpdateConfig(AppConfig),
    SetRunning(bool),
    ClearSubtitle,
    TestKey(usize, String),
}

pub enum WorkerEvent {
    SubtitleUpdate {
        original: String,
        translated: String,
        engine: String,
        ocr_ms: u64,
        trans_ms: u64,
    },
    SubtitleCleared,
    KeyStatusesUpdated {
        keys: Vec<(usize, KeyStatus, usize)>, // (index, status, characters_used)
        active_index: usize,
    },
    TestResult {
        slot: usize,
        result: Result<String, String>,
    },
}

pub struct XSubApp {
    config: AppConfig,
    control_window: ControlWindow,
    roi_selector: RoiSelectorView,
    is_roi_selector_open: bool,

    is_running: bool,
    original_text: String,
    translated_text: String,
    last_engine_used: String,
    ocr_latency_ms: u64,
    trans_latency_ms: u64,

    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    last_window_refresh: Instant,

    cmd_sender: Sender<WorkerCommand>,
    event_receiver: Receiver<WorkerEvent>,
}

impl XSubApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        apply_minimal_dark_theme(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Focus);

        let config = AppConfig::load();
        let monitors = ScreenCapture::list_monitors();
        let windows = ScreenCapture::list_windows();

        let (cmd_tx, cmd_rx) = unbounded::<WorkerCommand>();
        let (evt_tx, evt_rx) = unbounded::<WorkerEvent>();

        // Arka plan isci is parcacigini (Worker Thread) baslat
        let worker_config = config.clone();
        thread::spawn(move || {
            run_worker_thread(worker_config, cmd_rx, evt_tx);
        });

        Self {
            config,
            control_window: ControlWindow::default(),
            roi_selector: RoiSelectorView::default(),
            is_roi_selector_open: false,

            is_running: false,
            original_text: String::new(),
            translated_text: String::new(),
            last_engine_used: String::new(),
            ocr_latency_ms: 0,
            trans_latency_ms: 0,

            monitors,
            windows,
            last_window_refresh: Instant::now(),

            cmd_sender: cmd_tx,
            event_receiver: evt_rx,
        }
    }
}

impl eframe::App for XSubApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Arka plandan gelen olaylari tuket
        let mut subtitle_updated = false;
        while let Ok(evt) = self.event_receiver.try_recv() {
            match evt {
                WorkerEvent::SubtitleUpdate {
                    original,
                    translated,
                    engine,
                    ocr_ms,
                    trans_ms,
                } => {
                    self.original_text = original;
                    self.translated_text = translated;
                    self.last_engine_used = engine;
                    self.ocr_latency_ms = ocr_ms;
                    self.trans_latency_ms = trans_ms;
                    subtitle_updated = true;
                }
                WorkerEvent::SubtitleCleared => {
                    self.original_text.clear();
                    self.translated_text.clear();
                    subtitle_updated = true;
                }
                WorkerEvent::KeyStatusesUpdated { keys, active_index } => {
                    self.config.active_key_index = active_index;
                    for (slot, status, chars) in keys {
                        if slot < self.config.deepl_keys.len() {
                            self.config.deepl_keys[slot].status = status;
                            self.config.deepl_keys[slot].characters_used = chars;
                        }
                    }
                    let _ = self.config.save();
                }
                WorkerEvent::TestResult { slot, result } => {
                    self.control_window.key_manager_view.test_result_message = Some((slot, result));
                }
            }
        }

        // Pencereleri 3 saniyede bir otomatik yenile
        if self.last_window_refresh.elapsed() > Duration::from_secs(3) {
            self.windows = ScreenCapture::list_windows();
            self.last_window_refresh = Instant::now();
        }

        let mut toggle_running = false;
        let mut open_roi_selector = false;
        let mut clear_subtitle = false;
        let mut test_trigger: Option<(usize, String)> = None;
        let prev_engine = self.config.preferred_engine;
        let prev_monitor = self.config.target_monitor_index;
        let prev_window = self.config.target_window_title.clone();
        let prev_auto_clear = self.config.auto_clear_subtitles;
        let prev_clear_delay = self.config.clear_delay_ms;
        let prev_max_hold = self.config.max_hold_timeout_secs;
        let prev_interval = self.config.capture_interval_ms;
        let prev_keys: Vec<String> = self.config.deepl_keys.iter().map(|k| k.key.clone()).collect();
        let prev_roi = (
            self.config.roi.x,
            self.config.roi.y,
            self.config.roi.width,
            self.config.roi.height,
        );

        // 2. Ana Kontrol Penceresi Arayuzu
        egui::CentralPanel::default().show(ctx, |ui| {
            let state = DashboardState {
                is_running: self.is_running,
                original_text: &self.original_text,
                translated_text: &self.translated_text,
                last_engine_used: &self.last_engine_used,
                ocr_latency_ms: self.ocr_latency_ms,
                trans_latency_ms: self.trans_latency_ms,
                monitors: &self.monitors,
                windows: &self.windows,
            };

            self.control_window.render(
                ui,
                &mut self.config,
                &state,
                &mut toggle_running,
                &mut open_roi_selector,
                &mut test_trigger,
                &mut clear_subtitle,
            );
        });

        // Buton etkilesimleri
        if clear_subtitle {
            self.original_text.clear();
            self.translated_text.clear();
            let _ = self.cmd_sender.send(WorkerCommand::ClearSubtitle);
        }

        // Buton etkilesimleri
        if toggle_running {
            self.is_running = !self.is_running;
            let _ = self
                .cmd_sender
                .send(WorkerCommand::SetRunning(self.is_running));
            let _ = self
                .cmd_sender
                .send(WorkerCommand::UpdateConfig(self.config.clone()));
        }

        if open_roi_selector {
            let ppp = ctx.pixels_per_point().max(1.0);
            self.roi_selector.open_with_config(&self.config, ppp);
            self.is_roi_selector_open = true;
        }

        if let Some((slot, key)) = test_trigger {
            let _ = self.cmd_sender.send(WorkerCommand::TestKey(slot, key));
        }

        // 3. Altyazi Overlay Penceresi (Secondary Viewport)
        // Calisirken veya duzenleme modundayken overlay penceresi aciktir
        let show_overlay = self.is_running || !self.config.overlay.is_locked;
        if show_overlay {
            let overlay_id = egui::ViewportId::from_hash_of("xsub_subtitle_overlay");
            let mut overlay_builder = egui::ViewportBuilder::default()
                .with_title("x-sub Altyazi Katmani")
                .with_inner_size([self.config.overlay.window_w, self.config.overlay.window_h])
                .with_position([self.config.overlay.window_x, self.config.overlay.window_y])
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top();

            // Sadece ekranda hicbir metin yokken ve kilitliyken tiklamalari arkaya gecir;
            // metin varken kullanici dogrudan metinden tutup surukleyebilsin!
            if self.translated_text.trim().is_empty() && self.config.overlay.is_locked {
                overlay_builder = overlay_builder.with_mouse_passthrough(true);
            }

            let current_sub = self.translated_text.clone();
            ctx.show_viewport_immediate(overlay_id, overlay_builder, |ctx, _class| {
                // Windows'ta bu pencerenin DWM cam seffafligini aktiflestir (siyah arka plani onler)
                // ve kendi ekran yakalamamizda gorunmemesi icin WDA_EXCLUDEFROMCAPTURE uygula
                prepare_transparent_overlay_window("x-sub Altyazi Katmani");

                if subtitle_updated {
                    ctx.request_repaint();
                }

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                    .show(ctx, |ui| {
                        SubtitleOverlay::render(ui, ctx, &mut self.config, &current_sub);
                    });
            });
        }

        // 4. ROI Secici Penceresi (Transparent Overlay Viewport - Windows'ta siyah ekran olmamasi icin fullscreen yerine monitor boyutu kullanilir)
        if self.is_roi_selector_open {
            let roi_id = egui::ViewportId::from_hash_of("xsub_roi_selector");
            let ppp = ctx.pixels_per_point().max(1.0);
            let (mon_w, mon_h) = self
                .monitors
                .get(self.config.target_monitor_index)
                .or_else(|| self.monitors.first())
                .map(|m| {
                    (
                        (m.width as f32 / ppp).max(600.0),
                        (m.height as f32 / ppp - 1.0).max(400.0),
                    )
                })
                .unwrap_or((1920.0 / ppp, 1079.0 / ppp));

            let roi_builder = egui::ViewportBuilder::default()
                .with_title("x-sub Bolge Secici")
                .with_position([0.0, 0.0])
                .with_inner_size([mon_w, mon_h])
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false);

            ctx.show_viewport_immediate(roi_id, roi_builder, |ctx, _class| {
                prepare_transparent_overlay_window("x-sub Bolge Secici");

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                    .show(ctx, |ui| {
                        self.roi_selector.render(
                            ui,
                            ctx,
                            &mut self.config,
                            &mut self.is_roi_selector_open,
                        );
                    });
            });
        }

        // 5. Motor secimi, anahtarlar, ekran/pencere veya ROI bolgesi degistiyse (ROI secici icindeki degisiklikler dahil!)
        //    aninda diske kaydet ve arka plandaki isciye gonder.
        let curr_roi = (
            self.config.roi.x,
            self.config.roi.y,
            self.config.roi.width,
            self.config.roi.height,
        );
        let curr_keys: Vec<String> = self.config.deepl_keys.iter().map(|k| k.key.clone()).collect();
        if self.config.preferred_engine != prev_engine
            || self.config.target_monitor_index != prev_monitor
            || self.config.target_window_title != prev_window
            || self.config.auto_clear_subtitles != prev_auto_clear
            || self.config.clear_delay_ms != prev_clear_delay
            || self.config.max_hold_timeout_secs != prev_max_hold
            || self.config.capture_interval_ms != prev_interval
            || curr_keys != prev_keys
            || curr_roi != prev_roi
        {
            let _ = self.config.save();
            let _ = self
                .cmd_sender
                .send(WorkerCommand::UpdateConfig(self.config.clone()));
        }

        // Surekli akici guncelleme icin istek (Worker'dan gelen mesajlarin aninda islenmesi icin)
        let repaint_ms = if self.is_running {
            self.config.capture_interval_ms.min(80)
        } else {
            250
        };
        ctx.request_repaint_after(Duration::from_millis(repaint_ms));
    }
}

/// Windows API ile yardimci overlay pencerelerinin:
/// 1. DWM cam seffafligini (DwmExtendFrameIntoClientArea + DwmEnableBlurBehindWindow) zorunlu aktiflestirir,
///    boylece OpenGL alpha=0 oldugunda Windows'un siyah arka plan gostermesini %100 engeller.
/// 2. Ekran yakalamadan (BitBlt / DXGI) haric tutar (WDA_EXCLUDEFROMCAPTURE).
fn prepare_transparent_overlay_window(title: &str) {
    use std::collections::HashSet;
    use std::mem::size_of;
    use std::sync::Mutex;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
        DWMWINDOWATTRIBUTE, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, HGDIOBJ};
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowDisplayAffinity, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    };

    static INITIALIZED_HWNDS: Mutex<Option<HashSet<isize>>> = Mutex::new(None);

    unsafe {
        let htitle = HSTRING::from(title);
        if let Ok(hwnd) = FindWindowW(None, &htitle) {
            if !hwnd.0.is_null() {
                let key = hwnd.0 as isize;
                let mut guard = INITIALIZED_HWNDS.lock().unwrap();
                let set = guard.get_or_insert_with(HashSet::new);

                if !set.contains(&key) {
                    // 0. Windows 11 DWM 1px pencere cercevesini (border) ve golgesini tamamen kapat
                    let ncrp_disabled: u32 = 1; // DWMNCRP_DISABLED
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWINDOWATTRIBUTE(2), // DWMWA_NCRENDERING_POLICY
                        (&ncrp_disabled as *const u32).cast(),
                        size_of::<u32>() as u32,
                    );
                    let corner_none: u32 = 1; // DWMWCP_DONOTROUND
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWINDOWATTRIBUTE(33), // DWMWA_WINDOW_CORNER_PREFERENCE
                        (&corner_none as *const u32).cast(),
                        size_of::<u32>() as u32,
                    );
                    let border_none: u32 = 0xFFFFFFFE; // DWMWA_COLOR_NONE
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWINDOWATTRIBUTE(34), // DWMWA_BORDER_COLOR
                        (&border_none as *const u32).cast(),
                        size_of::<u32>() as u32,
                    );

                    // 1. DWM Frame'i tum istemci alanina genislet (Alpha kanali masaustuyle harmanlansin)
                    let margins = MARGINS {
                        cxLeftWidth: -1,
                        cxRightWidth: -1,
                        cyTopHeight: -1,
                        cyBottomHeight: -1,
                    };
                    let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

                    // 2. Bos (1px) bolge ile DwmEnableBlurBehindWindow: OpenGL per-pixel alpha'yi
                    //    her ekran kartinda (NVIDIA/AMD/Intel) bulaniklik olmadan cam gibi seffaf yapar!
                    let hrgn = CreateRectRgn(0, 0, -1, -1);
                    let bb = DWM_BLURBEHIND {
                        dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                        fEnable: BOOL(1),
                        hRgnBlur: hrgn,
                        fTransitionOnMaximized: BOOL(0),
                    };
                    let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
                    if !hrgn.is_invalid() {
                        let _ = DeleteObject(HGDIOBJ(hrgn.0));
                    }

                    set.insert(key);
                }

                // 3. Ekran yakalamadan haric tutmayi her zaman dogrula (pencere yeniden acilsa bile)
                let mut current = 0u32;
                if GetWindowDisplayAffinity(hwnd, &mut current).is_ok()
                    && current != WDA_EXCLUDEFROMCAPTURE.0
                {
                    let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
                }
            }
        }
    }
}

/// Arka planda calisan asenkron / senkron isci fonksiyonu
fn run_worker_thread(
    mut config: AppConfig,
    cmd_rx: Receiver<WorkerCommand>,
    evt_tx: Sender<WorkerEvent>,
) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let pool = Arc::new(TranslationPool::new());
    let ocr = match WinMediaOcr::new(&config.source_lang) {
        Ok(engine) => Some(engine),
        Err(err) => {
            eprintln!("Windows Media OCR baslatilamadi: {}", err);
            None
        }
    };

    let mut is_running = false;
    let mut prev_frame: Option<(RgbaImage, u64)> = None;
    let mut last_text = String::new();
    let mut last_spoken_text = String::new(); // Ekrandan silinse bile son cevrilen repligi hatirlar
    let mut last_valid_seen: Option<Instant> = None;
    let mut empty_since: Option<Instant> = None;
    let mut empty_frame_count: usize = 0;

    loop {
        // 1. Gelen komutlari isle
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                WorkerCommand::UpdateConfig(new_cfg) => {
                    let roi_changed = new_cfg.roi.x != config.roi.x
                        || new_cfg.roi.y != config.roi.y
                        || new_cfg.roi.width != config.roi.width
                        || new_cfg.roi.height != config.roi.height
                        || new_cfg.target_window_title != config.target_window_title
                        || new_cfg.target_monitor_index != config.target_monitor_index;
                    config = new_cfg;
                    prev_frame = None;
                    if roi_changed {
                        last_text.clear();
                        last_spoken_text.clear();
                        last_valid_seen = None;
                        empty_since = None;
                        empty_frame_count = 0;
                        let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                    }
                }
                WorkerCommand::ClearSubtitle => {
                    last_text.clear();
                    last_spoken_text.clear();
                    last_valid_seen = None;
                    empty_since = None;
                    empty_frame_count = 0;
                    let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                }
                WorkerCommand::SetRunning(running) => {
                    is_running = running;
                    if !running {
                        prev_frame = None;
                        last_text.clear();
                        last_spoken_text.clear();
                        last_valid_seen = None;
                        empty_since = None;
                        empty_frame_count = 0;
                        let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                    }
                }
                WorkerCommand::TestKey(slot, key) => {
                    let pool_clone = pool.clone();
                    let evt_tx_clone = evt_tx.clone();
                    rt.spawn(async move {
                        let res = pool_clone.test_key(&key).await;
                        let _ = evt_tx_clone.send(WorkerEvent::TestResult { slot, result: res });
                    });
                }
            }
        }

        // 2. Calisma dongusu
        if is_running {
            let capture_start = Instant::now();

            // Ekran altyazi bolgesini yakala
            match ScreenCapture::capture_roi(
                config.target_monitor_index,
                config.target_window_title.as_deref(),
                &config.roi,
            ) {
                Ok(current_image) => {
                    let current_dhash = compute_dhash(&current_image);

                    let should_process = if let Some((prev_img, prev_hash)) = &prev_frame {
                        let ham_dist = hamming_distance(*prev_hash, current_dhash);
                        if ham_dist <= 1 {
                            let (changed, _ratio) = calculate_image_diff(
                                prev_img,
                                &current_image,
                                config.pixel_diff_threshold.max(0.015),
                            );
                            changed
                        } else {
                            true
                        }
                    } else {
                        true
                    };

                    if should_process {
                        if let Some(ocr_engine) = &ocr {
                            let ocr_start = Instant::now();
                            let raw_text = ocr_engine
                                .recognize_rgba(
                                    current_image.as_raw(),
                                    current_image.width(),
                                    current_image.height(),
                                )
                                .unwrap_or_default();

                            let mut cleaned = clean_ocr_text(&raw_text);
                            let (mut is_valid, _reason) =
                                AntiGarbageFilter::is_valid_subtitle(&cleaned);

                            // Eger ham goruntu karisik oyun arka plani yuzunden okunamadiysa veya
                            // cop/gurultu urettiyse, parlak altyazi izole maskesiyle (binarize) tekrar dene!
                            if !is_valid {
                                let isolated = isolate_subtitle_for_ocr(&current_image);
                                let iso_raw = ocr_engine
                                    .recognize_rgba(
                                        isolated.as_raw(),
                                        isolated.width(),
                                        isolated.height(),
                                    )
                                    .unwrap_or_default();
                                let iso_cleaned = clean_ocr_text(&iso_raw);
                                let (iso_valid, _) =
                                    AntiGarbageFilter::is_valid_subtitle(&iso_cleaned);
                                if iso_valid {
                                    cleaned = iso_cleaned;
                                    is_valid = true;
                                }
                            }

                            let ocr_ms = ocr_start.elapsed().as_millis() as u64;

                            if is_valid {
                                // Gecerli bir oyun altyazisi bulundu: bosluk sayaclarini sifirla
                                empty_frame_count = 0;
                                empty_since = None;

                                // Fuzzy Similarity kontrolu: Eger su an ekranda olan metinle %84+ ayniysa
                                // ufak bir OCR titremesidir, hicbir islem yapma!
                                let is_active_duplicate = !last_text.is_empty()
                                    && calculate_text_similarity(&cleaned, &last_text) >= 0.84;

                                // Eger altyazi max_hold_timeout yuzunden ekrandan gizlendiyse ama oyun
                                // sahnesinde ayni replik hala duruyorsa (last_spoken_text ile %84+ ayniysa),
                                // tekrar tekrar cevirip ekrana getirme!
                                let is_already_spoken_and_timed_out = last_text.is_empty()
                                    && !last_spoken_text.is_empty()
                                    && calculate_text_similarity(&cleaned, &last_spoken_text) >= 0.84;

                                if is_active_duplicate {
                                    // Watchdog: Eger max_hold_timeout dolduysa ve ayni metin ekranda asili kaldiysa temizle
                                    if config.auto_clear_subtitles && config.max_hold_timeout_secs > 0 {
                                        if let Some(seen_time) = last_valid_seen {
                                            if seen_time.elapsed()
                                                >= Duration::from_secs(config.max_hold_timeout_secs)
                                            {
                                                last_text.clear();
                                                last_valid_seen = None;
                                                let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                                            }
                                        }
                                    }
                                } else if !is_already_spoken_and_timed_out {
                                    last_text = cleaned.clone();
                                    last_spoken_text = cleaned.clone();
                                    last_valid_seen = Some(Instant::now());

                                    // Ceviri islemi (TranslationCache de ayrica %86 fuzzy koruma saglar)
                                    let trans_start = Instant::now();
                                    let pool_clone = pool.clone();
                                    let evt_tx_clone = evt_tx.clone();
                                    let cleaned_clone = cleaned.clone();

                                    rt.block_on(async {
                                        match pool_clone
                                            .translate(&cleaned_clone, &mut config)
                                            .await
                                        {
                                            Ok(res) => {
                                                let trans_ms =
                                                    trans_start.elapsed().as_millis() as u64;
                                                let _ = evt_tx_clone.send(
                                                    WorkerEvent::SubtitleUpdate {
                                                        original: cleaned_clone,
                                                        translated: res.translated_text,
                                                        engine: res.used_engine,
                                                        ocr_ms,
                                                        trans_ms,
                                                    },
                                                );

                                                // Anahtar durumlari degismisse UI'a gonder
                                                let statuses: Vec<_> = config
                                                    .deepl_keys
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(i, k)| {
                                                        (i, k.status.clone(), k.characters_used)
                                                    })
                                                    .collect();

                                                let _ = evt_tx_clone.send(
                                                    WorkerEvent::KeyStatusesUpdated {
                                                        keys: statuses,
                                                        active_index: config.active_key_index,
                                                    },
                                                );
                                            }
                                            Err(err) => {
                                                let _ = evt_tx_clone.send(
                                                    WorkerEvent::SubtitleUpdate {
                                                        original: cleaned_clone,
                                                        translated: format!("[Hata: {}]", err),
                                                        engine: "Hata".to_string(),
                                                        ocr_ms,
                                                        trans_ms: 0,
                                                    },
                                                );
                                            }
                                        }
                                    });
                                }
                            } else {
                                // Ekranda altyazi yok veya silindi!
                                if !last_text.is_empty() && config.auto_clear_subtitles {
                                    empty_frame_count += 1;
                                    let elapsed_empty =
                                        empty_since.get_or_insert_with(Instant::now).elapsed();
                                    let delay = Duration::from_millis(config.clear_delay_ms);

                                    // Belirlenen gecikme suresi dolduysa veya arka arkaya 2 bos kare yakalandiysa:
                                    if elapsed_empty >= delay || empty_frame_count >= 2 {
                                        last_text.clear();
                                        last_spoken_text.clear(); // Gercekten bos ekran goruldugunde sifirla
                                        last_valid_seen = None;
                                        empty_since = None;
                                        empty_frame_count = 0;
                                        let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                                    }
                                }
                            }
                        }
                    } else {
                        // Goruntu degismedi (statik kare).
                        // Watchdog Zaman Asimi Kontrolu: Eger altyazi ekranda cok uzun sure (max_hold) takildiysa temizle
                        if !last_text.is_empty()
                            && config.auto_clear_subtitles
                            && config.max_hold_timeout_secs > 0
                        {
                            if let Some(seen_time) = last_valid_seen {
                                if seen_time.elapsed()
                                    >= Duration::from_secs(config.max_hold_timeout_secs)
                                {
                                    last_text.clear();
                                    last_valid_seen = None;
                                    empty_since = None;
                                    empty_frame_count = 0;
                                    let _ = evt_tx.send(WorkerEvent::SubtitleCleared);
                                }
                            }
                        }
                    }

                    prev_frame = Some((current_image, current_dhash));
                }
                Err(_err) => {
                    // Ekran yakalama hatasi (minimized vb.)
                }
            }

            let elapsed = capture_start.elapsed();
            let target_interval = Duration::from_millis(config.capture_interval_ms);
            if elapsed < target_interval {
                thread::sleep(target_interval - elapsed);
            }
        } else {
            thread::sleep(Duration::from_millis(100));
        }
    }
}
