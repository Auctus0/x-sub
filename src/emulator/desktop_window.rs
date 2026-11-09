use crate::emulator::scenes::{get_standard_game_scenes, GameScene, GameScreenRenderer};
use image::RgbaImage;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, SetDIBitsToDevice, BITMAPINFO, BITMAPINFOHEADER,
    DIB_RGB_COLORS, HBRUSH, PAINTSTRUCT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, PostQuitMessage,
    RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, SW_SHOW, WM_DESTROY,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_PAINT, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

static CURRENT_SCENE_INDEX: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let width = (rect.right - rect.left).max(800) as u32;
            let height = (rect.bottom - rect.top).max(450) as u32;

            let scenes = get_standard_game_scenes();
            let idx = CURRENT_SCENE_INDEX.load(Ordering::Relaxed) % scenes.len();
            let scene = &scenes[idx];

            let renderer = GameScreenRenderer::new();
            let frame = render_game_frame(&renderer, scene, width, height);

            let mut bgra_pixels = vec![0u8; (width * height * 4) as usize];
            #[allow(clippy::chunks_exact_to_as_chunks)]
            for (src, dst) in frame.chunks_exact(4).zip(bgra_pixels.chunks_exact_mut(4)) {
                dst[0] = src[2]; // B
                dst[1] = src[1]; // G
                dst[2] = src[0]; // R
                dst[3] = src[3]; // A
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    ..Default::default()
                },
                ..Default::default()
            };

            SetDIBitsToDevice(
                hdc,
                0,
                0,
                width,
                height,
                0,
                0,
                0,
                height,
                bgra_pixels.as_ptr().cast(),
                &bmi,
                DIB_RGB_COLORS,
            );

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            // Space veya Sag Ok ile sahneyi degistir
            let key = wparam.0 as u32;
            if key == 0x20 || key == 0x27 {
                // VK_SPACE or VK_RIGHT
                let scenes = get_standard_game_scenes();
                let next = (CURRENT_SCENE_INDEX.load(Ordering::Relaxed) + 1) % scenes.len();
                CURRENT_SCENE_INDEX.store(next, Ordering::Relaxed);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let scenes = get_standard_game_scenes();
            let next = (CURRENT_SCENE_INDEX.load(Ordering::Relaxed) + 1) % scenes.len();
            CURRENT_SCENE_INDEX.store(next, Ordering::Relaxed);
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 1280x720 oyun penceresi karesi uretir (Baslik, sinematik arkaplan, PS5 altyazi kutusu)
fn render_game_frame(
    renderer: &GameScreenRenderer,
    scene: &GameScene,
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);

    // Koyu sinematik arkaplan
    for x in 0..width {
        for y in 0..height {
            let grad = ((y as f32 / height as f32) * 20.0) as u8;
            img.put_pixel(x, y, image::Rgba([10 + grad, 10 + grad, 14 + grad, 255]));
        }
    }

    // Ust Bilgi Seridi
    if let Some(font) = renderer.font() {
        let header_text = format!(
            "PS5 OYUN SIMULATORU: {} (Sahne degistirmek icin Tiklayin veya Space'e basin)",
            scene.game_name
        );
        renderer.draw_text(
            &mut img,
            font,
            &header_text,
            15.0,
            30,
            25,
            image::Rgba([160, 160, 168, 255]),
        );
    }

    // Altyazi kutusu (pencerenin alt kisminda)
    let sub_h = 110u32;
    let sub_y = height.saturating_sub(sub_h + 30);
    let sub_pad_x = 40u32;
    let sub_w = width.saturating_sub(sub_pad_x * 2);

    for x in sub_pad_x..(sub_pad_x + sub_w) {
        for y in sub_y..(sub_y + sub_h) {
            img.put_pixel(x, y, image::Rgba([14, 14, 18, 255]));
        }
    }

    // Altyazi kutusu cercevesi
    for x in sub_pad_x..(sub_pad_x + sub_w) {
        img.put_pixel(x, sub_y, image::Rgba([50, 50, 56, 255]));
        img.put_pixel(x, sub_y + sub_h - 1, image::Rgba([50, 50, 56, 255]));
    }
    for y in sub_y..(sub_y + sub_h) {
        img.put_pixel(sub_pad_x, y, image::Rgba([50, 50, 56, 255]));
        img.put_pixel(sub_pad_x + sub_w - 1, y, image::Rgba([50, 50, 56, 255]));
    }

    // Altyazi metnini ciz
    let sub_snippet = renderer.render_scene(scene);
    // snippet 1200x180 -> icindeki yaziyi sub_y bolgesine bindir
    let snippet_w = sub_snippet.width().min(sub_w);
    let snippet_h = sub_snippet.height().min(sub_h);
    let offset_x = sub_pad_x + (sub_w.saturating_sub(snippet_w)) / 2;
    let offset_y = sub_y + (sub_h.saturating_sub(snippet_h)) / 2;

    for sx in 0..snippet_w {
        for sy in 0..snippet_h {
            let p = sub_snippet.get_pixel(sx, sy);
            // Eger siyah arka plan degilse veya parlak metinse aktar
            if p[0] > 20 || p[1] > 20 || p[2] > 20 {
                let dx = offset_x + sx;
                let dy = offset_y + sy;
                if dx < width && dy < height {
                    img.put_pixel(dx, dy, *p);
                }
            }
        }
    }

    img
}

pub fn run_game_simulator_standalone() -> eframe::Result {
    unsafe {
        let class_name = w!("PS5SimulatorWindowClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null())
                .unwrap_or_default()
                .into(),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            lpszClassName: class_name,
            ..Default::default()
        };

        let _ = RegisterClassW(&wc);

        let window_title = w!("PS5 Oyun Ekrani Simulatoru");
        let hwnd = match CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            class_name,
            window_title,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            1280,
            720,
            HWND::default(),
            windows::Win32::UI::WindowsAndMessaging::HMENU::default(),
            wc.hInstance,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[HATA] Native Win32 penceresi olusturulamadi: {}", e);
                return Ok(());
            }
        };

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
