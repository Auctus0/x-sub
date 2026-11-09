mod app;
mod benchmark;
mod capture;
mod config;
mod emulator;
mod ocr;
mod theme;
mod translation;
mod ui;

use app::XSubApp;
use eframe::egui;
use std::env;

#[tokio::main]
async fn main() -> eframe::Result {
    let args: Vec<String> = env::args().collect();

    // 1. Gercek Windows Masaustu Oyun Penceresi Baslatici
    if args.iter().any(|a| a == "--game-window") {
        return emulator::run_game_simulator_standalone();
    }

    // 2. Canli Masaustu Saha Testi (Gercek OS penceresi acip piksellerini xcap ile yakalar)
    if args.iter().any(|a| a == "--live-field-test" || a == "-l") {
        emulator::run_live_desktop_field_test().await;
        return Ok(());
    }

    // 3. Otomatik Oyun Emulatoru Test Modu
    if args.iter().any(|a| a == "--test-emulator" || a == "-t") {
        if !emulator::run_automated_emulator_test().await {
            std::process::exit(1);
        }
        return Ok(());
    }

    // 4. Performans Benchmark Modu
    if args
        .iter()
        .any(|a| a == "--benchmark" || a == "-b" || a == "--test-perf")
    {
        benchmark::run_comprehensive_benchmark();
        return Ok(());
    }

    // 5. Normal GUI Arayuzu (Kompakt Boyut)
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("x-sub")
            .with_inner_size([640.0, 380.0])
            .with_min_inner_size([520.0, 310.0])
            .with_resizable(true)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "x-sub",
        native_options,
        Box::new(|cc| Ok(Box::new(XSubApp::new(cc)))),
    )
}
