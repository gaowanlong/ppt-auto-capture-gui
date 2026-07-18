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
mod config;
mod i18n;
mod capture;
mod detection;
mod gui;
mod model;
mod pptx;
mod storage;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
#[path = "windows/stub.rs"]
mod windows;

fn main() -> Result<(), eframe::Error> {
    // Set DPI awareness for high-resolution screenshots on Windows
    #[cfg(target_os = "windows")]
    {
        // Use modern per-monitor DPI awareness so GetDC(NULL) and
        // GetClientRect/ClientToScreen all return consistent physical pixels.
        // Legacy SetProcessDPIAware() only sets system DPI awareness,
        // which miscounts pixels on modern Windows with >100% DPI scaling.
        #[link(name = "shcore")]
        extern "system" {
            fn SetProcessDpiAwareness(value: u32) -> i32;
        }
        // PROCESS_PER_MONITOR_DPI_AWARE = 2 — all coordinates in device pixels
        { let _ = unsafe { SetProcessDpiAwareness(2) }; }
        // Fallback: legacy system DPI awareness
        #[link(name = "user32")]
        extern "system" {
           fn SetProcessDPIAware() -> i32;
       }
        { let _ = unsafe { SetProcessDPIAware() }; }
    }

    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .format_timestamp_millis()
    .init();

    info!("PPT Auto Capture GUI starting...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 500.0])
            .with_title("PPT Auto Capture"),
        #[cfg(target_os = "windows")]
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "PPT Auto Capture",
        options,
        Box::new(|cc| {
            // Load CJK fonts for Chinese text support
            setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(app::PptAutoCaptureApp::new()))
        }),
    )
}

/// Add CJK font support by loading system fonts on Windows.
/// Add CJK font support by loading system fonts on Windows.
fn setup_cjk_fonts(ctx: &egui::Context) {
    #[cfg(target_os = "windows")]
    {
        let mut fonts = egui::FontDefinitions::default();
        let font_list = [
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\simsun.ttc",
            r"C:\Windows\Fonts\simhei.ttf",
        ];
        for (i, path) in font_list.iter().enumerate() {
            if let Ok(data) = std::fs::read(path) {
                let name = format!("cjk_{}", i);
                fonts.font_data.insert(name.clone(), egui::FontData::from_owned(data).into());
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name);
            }
        }
        ctx.set_fonts(fonts);
    }
}
