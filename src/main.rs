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
        Box::new(|_cc| {
            Ok(Box::new(app::PptAutoCaptureApp::new()))
        }),
    )
}
