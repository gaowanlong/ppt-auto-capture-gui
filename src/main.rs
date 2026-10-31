//! PPT Auto Capture GUI — Main Entry Point
//!
//! A Windows-first tool for automatic PowerPoint slide capture.
//! Uses DXGI Desktop Duplication (with GDI fallback) to monitor
//! a selected display for slide changes, then saves them as PNG
//! and populates a real-time output.pptx.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use log::info;

mod app;
mod capture;
mod config;
mod detection;
mod diagnostics;
mod gui;
mod i18n;
mod model;
mod pptx;
mod storage;

mod windows;

fn main() -> Result<(), eframe::Error> {
    // Set DPI awareness for high-resolution screenshots on Windows
    #[cfg(target_os = "windows")]
    {
        // Make process DPI aware so GetDC(NULL) returns physical pixels.
        // Calls legacy SetProcessDPIAware() which works with the windows crate 0.60.
        // High-DPI scaling correction is handled inside GdiCapturer::capture_frame().
        #[link(name = "user32")]
        extern "system" {
            fn SetProcessDPIAware() -> i32;
        }
        {
            let _ = unsafe { SetProcessDPIAware() };
        }
    }

    let logging = diagnostics::initialize_logging();
    std::panic::set_hook(Box::new(|panic| {
        log::error!(
            "Unhandled panic: {panic}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
    }));
    info!(
        "PPT Auto Capture {} starting on {}/{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 760.0])
            .with_min_inner_size([760.0, 560.0])
            .with_title("PPT Auto Capture"),
        #[cfg(target_os = "windows")]
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "PPT Auto Capture",
        options,
        Box::new(move |cc| {
            // Load CJK fonts for Chinese text support, then apply the shared
            // light workspace theme before any panel is drawn.
            setup_cjk_fonts(&cc.egui_ctx);
            gui::theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::PptAutoCaptureApp::new(logging)))
        }),
    )
}

/// Append platform CJK fonts as fallbacks so Latin text keeps egui's compact
/// interface face while Chinese glyphs still resolve on every platform.
fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "windows")]
    let font_list: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
    ];
    #[cfg(target_os = "macos")]
    let font_list: &[&str] = &[
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ];
    #[cfg(target_os = "linux")]
    let font_list: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];

    for (i, path) in font_list.iter().enumerate() {
        if let Ok(data) = std::fs::read(path) {
            let name = format!("cjk_{}", i);
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(data).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(name.clone());
            // Monospace details (paths, timestamps, window metadata) also carry
            // Chinese copy, so the same glyph fallback is needed there.
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod platform_tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_xcap_backend() {
        assert_eq!(crate::windows::backend_name(), "macos-xcap");
    }
}
