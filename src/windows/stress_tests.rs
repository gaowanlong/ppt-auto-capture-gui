//! Real Win32 capture pressure tests. Reports contain metrics, never captured images.
use super::*;
use crate::capture::stress_support::FreshnessWatchdog;
use crate::capture::CaptureWorker;
use crate::windows::session_events::{capture_resources, CaptureResources};
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, SetWindowTextW, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    WS_VISIBLE,
};

struct Fixture(HWND);
impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

#[derive(serde::Serialize)]
struct Sample {
    elapsed_seconds: f64,
    resources: CaptureResources,
}

#[derive(serde::Serialize)]
struct Report {
    schema_version: u32,
    outcome: String,
    requested_seconds: u64,
    elapsed_seconds: f64,
    frames: u64,
    changed_frames: u64,
    reinitializations: u64,
    early_error_checks: u64,
    worker_restarts: u64,
    before: CaptureResources,
    after: CaptureResources,
    samples: Vec<Sample>,
    error: Option<String>,
}

fn run_native_stress(
    seconds: u64,
    width: u32,
    height: u32,
    interval_ms: u64,
    report_path: Option<&std::path::Path>,
) -> Result<()> {
    let _serial = NATIVE_CAPTURE_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    ensure!(
        (1..=86400).contains(&seconds),
        "Stress duration must be 1..86400 seconds"
    );
    ensure!(
        (64..=3840).contains(&width) && (64..=2160).contains(&height),
        "Stress resolution outside supported range"
    );
    let fixture = Fixture(unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("PPT Capture Windows stress fixture"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            width as i32,
            height as i32,
            None,
            None,
            None,
            None,
        )?
    });
    let region = crate::windows::get_client_window_rect(fixture.0 .0 as u64)?;
    let monitor = MonitorInfo {
        hmonitor: 0,
        adapter_name: "controlled stress fixture".into(),
        output_name: "fixture".into(),
        description: "fixture".into(),
        region,
        is_primary: true,
        is_virtual_suspect: false,
        output_index: 0,
        adapter_index: 0,
    };
    let mut capture = GdiCapturer::new();
    capture.initialize(&monitor)?;
    capture.set_window_hwnd(fixture.0 .0 as u64, region.width, region.height);
    // Warm the native APIs and allocator before establishing the resource baseline.
    for _ in 0..20 {
        capture.capture_frame()?;
    }
    let before = capture_resources()?;
    let start = Instant::now();
    let mut report = Report {
        schema_version: 1,
        outcome: "running".into(),
        requested_seconds: seconds,
        elapsed_seconds: 0.0,
        frames: 0,
        changed_frames: 0,
        reinitializations: 0,
        early_error_checks: 0,
        worker_restarts: 0,
        before: before.clone(),
        after: before,
        samples: Vec::new(),
        error: None,
    };
    let result = (|| -> Result<()> {
        use sha2::{Digest, Sha256};
        let mut freshness = FreshnessWatchdog::default();
        let mut next_sample = Duration::ZERO;
        while start.elapsed() < Duration::from_secs(seconds) {
            unsafe {
                SetWindowTextW(
                    fixture.0,
                    if report.frames % 2 == 0 {
                        w!("Stress frame A")
                    } else {
                        w!("Stress frame B")
                    },
                )?;
            }
            let frame = capture.capture_frame()?;
            ensure!(
                frame.width == region.width
                    && frame.height == region.height
                    && frame.data.len() == region.width as usize * region.height as usize * 4,
                "Incomplete native window frame"
            );
            report.frames += 1;
            let fingerprint = Sha256::digest(&frame.data).into();
            if freshness.observe(fingerprint, start.elapsed())? {
                report.changed_frames += 1;
            }
            drop(frame);
            if report.frames % 25 == 0 {
                // Both early-return and selected-bitmap cleanup paths must remain flat.
                ensure!(
                    capture_window_content(u64::MAX, 32, 32).is_err(),
                    "Invalid window capture unexpectedly succeeded"
                );
                drop(BitmapCapture::new(None, 32, 32)?);
                report.early_error_checks += 1;
                capture.initialize(&monitor)?;
                capture.set_window_hwnd(fixture.0 .0 as u64, region.width, region.height);
                report.reinitializations += 1;
            }
            if report.frames % 100 == 0 {
                drop(CaptureWorker::new());
                report.worker_restarts += 1;
            }
            if start.elapsed() >= next_sample {
                report.samples.push(Sample {
                    elapsed_seconds: start.elapsed().as_secs_f64(),
                    resources: capture_resources()?,
                });
                next_sample += Duration::from_secs(60);
            }
            if interval_ms > 0 {
                std::thread::sleep(Duration::from_millis(interval_ms.min(1000)));
            }
        }
        report.after = capture_resources()?;
        ensure!(report.frames >= 10, "Stress test captured too few frames");
        ensure!(
            report.changed_frames > 0,
            "Native capture returned only frozen/blank frames despite changing fixture content"
        );
        ensure!(
            report.after.gdi_objects <= report.before.gdi_objects + 8,
            "GDI handle growth: {} -> {}",
            report.before.gdi_objects,
            report.after.gdi_objects
        );
        ensure!(
            report.after.user_objects <= report.before.user_objects + 8,
            "USER handle growth"
        );
        ensure!(
            report.after.process_handles <= report.before.process_handles + 32,
            "Process handle growth"
        );
        ensure!(
            report.after.working_set_bytes
                <= report
                    .before
                    .working_set_bytes
                    .saturating_add(256 * 1024 * 1024),
            "Working set grew by more than 256 MiB after warmup"
        );
        Ok(())
    })();
    if let Ok(resources) = capture_resources() {
        report.after = resources;
    }
    report.elapsed_seconds = start.elapsed().as_secs_f64();
    report.outcome = if result.is_ok() { "passed" } else { "failed" }.into();
    report.error = result.as_ref().err().map(|error| format!("{error:#}"));
    if let Some(path) = report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("Windows native stress: outcome={}, elapsed={:.2}s, frames={}, restarts={}, GDI {} -> {}, handles {} -> {}", report.outcome, report.elapsed_seconds, report.frames, report.worker_restarts, report.before.gdi_objects, report.after.gdi_objects, report.before.process_handles, report.after.process_handles);
    result
}

#[test]
fn windows_native_capture_stress_quick() {
    run_native_stress(3, 640, 360, 0, None).unwrap();
}

#[test]
#[ignore = "Use scripts/run-windows-stress.ps1 for a timed native capture soak and JSON report"]
fn windows_native_capture_stress_soak() {
    fn value(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .map(|s| s.parse().expect("Invalid stress numeric setting"))
            .unwrap_or(default)
    }
    let path = std::env::var_os("PPT_CAPTURE_STRESS_REPORT").map(std::path::PathBuf::from);
    run_native_stress(
        value("PPT_CAPTURE_STRESS_SECONDS", 60),
        value("PPT_CAPTURE_STRESS_WIDTH", 1920).try_into().unwrap(),
        value("PPT_CAPTURE_STRESS_HEIGHT", 1080).try_into().unwrap(),
        value("PPT_CAPTURE_STRESS_INTERVAL_MS", 10),
        path.as_deref(),
    )
    .unwrap();
}
