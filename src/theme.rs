use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Sistemdeki Segoe UI Semibold / Bold fontunu yukleyerek Turkce (ğ, ü, ş, ı, ö, ç, İ)
/// ve tum Latin karakterlerin şeffaf katman uzerinde de net, keskin ve bozulmadan cizilmesini garanti eder.
pub fn configure_turkish_fonts(ctx: &egui::Context) {
    // Ana arayuz icin normal Segoe UI, Altyazi icin kalin/okunakli Segoe UI Semibold (seguisb.ttf)
    let regular_bytes = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf")
        .or_else(|_| std::fs::read(r"C:\Windows\Fonts\arial.ttf"));
    let bold_bytes = std::fs::read(r"C:\Windows\Fonts\seguisb.ttf")
        .or_else(|_| std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf"))
        .or_else(|_| std::fs::read(r"C:\Windows\Fonts\arialbd.ttf"));

    let mut fonts = egui::FontDefinitions::default();

    if let Ok(bytes) = regular_bytes {
        fonts.font_data.insert(
            "segoe_ui_tr".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, "segoe_ui_tr".to_owned());
        }
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            family.push("segoe_ui_tr".to_owned());
        }
    }

    if let Ok(bytes) = bold_bytes {
        fonts.font_data.insert(
            "subtitle_bold_tr".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts.families.insert(
            egui::FontFamily::Name("subtitle_bold".into()),
            vec!["subtitle_bold_tr".to_owned(), "segoe_ui_tr".to_owned()],
        );
    }

    ctx.set_fonts(fonts);
}

/// Profesyonel, goz yormayan mat koyu tema (JetBrains / VS Code tarzi).
/// Sifir neon, sifir parilti, sifir emoji/dekoratif susleme.
pub fn apply_minimal_dark_theme(ctx: &egui::Context) {
    configure_turkish_fonts(ctx);
    let mut visuals = Visuals::dark();

    // Ana arka plan ve panel renkleri (Mat koyu grafit)
    visuals.panel_fill = Color32::from_rgb(0x1e, 0x1e, 0x1e);
    visuals.window_fill = Color32::from_rgb(0x25, 0x25, 0x26);
    visuals.faint_bg_color = Color32::from_rgb(0x2a, 0x2d, 0x2e);
    visuals.extreme_bg_color = Color32::from_rgb(0x18, 0x18, 0x18);

    // Kenarliklar ve ayirici cizgiler
    visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x3e, 0x3e, 0x42));
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x33, 0x37));

    // Normal butonlar / girdiler
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x2d, 0x2d, 0x30);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x3e, 0x3e, 0x42));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xd4, 0xd4, 0xd4));

    // Uzerine gelindiginde (Hover)
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x3e, 0x3e, 0x42);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x00, 0x7a, 0xcc));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xff, 0xff, 0xff));

    // Tiklandiginda / Aktif (Active)
    visuals.widgets.active.bg_fill = Color32::from_rgb(0x00, 0x7a, 0xcc);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x00, 0x98, 0xff));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(0xff, 0xff, 0xff));

    // Secili alanlar
    visuals.selection.bg_fill = Color32::from_rgb(0x26, 0x4f, 0x78);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x00, 0x7a, 0xcc));

    // Metin renkleri
    visuals.override_text_color = Some(Color32::from_rgb(0xd4, 0xd4, 0xd4));

    // Keskin, hafif yuvarlatilmis kose yapisi (abartisiz)
    visuals.window_corner_radius = CornerRadius::same(4);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(2);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(2);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(2);
    visuals.widgets.active.corner_radius = CornerRadius::same(2);

    ctx.set_visuals(visuals);

    // Kompakt pencere duzeni icin sikistirilmis bosluklar
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(6.0, 2.5);
    style.spacing.window_margin = egui::Margin::same(6);
    ctx.set_style(style);
}

// Renk sabitleri
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(0x85, 0x85, 0x85);
pub const COLOR_TEXT_NORMAL: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
pub const COLOR_TEXT_BRIGHT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);

pub const COLOR_STATUS_OK: Color32 = Color32::from_rgb(0x4e, 0xc9, 0xb0); // Mat turkuaz/yesil
pub const COLOR_STATUS_WARN: Color32 = Color32::from_rgb(0xce, 0x91, 0x78); // Mat kehribar/turuncu
pub const COLOR_STATUS_ERR: Color32 = Color32::from_rgb(0xf4, 0x47, 0x47); // Mat kirmizi
pub const COLOR_ACCENT_BLUE: Color32 = Color32::from_rgb(0x00, 0x7a, 0xcc); // Mat celik mavisi
pub const COLOR_BG_CARD: Color32 = Color32::from_rgb(0x25, 0x25, 0x26);
pub const COLOR_BORDER_CARD: Color32 = Color32::from_rgb(0x3e, 0x3e, 0x42);
