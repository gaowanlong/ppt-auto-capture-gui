use anyhow::{Context, Result};
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;

use crate::capture::capture_source::CaptureSource;
use crate::capture::capture_state::CaptureState;
use crate::detection::{BlackFrameDetector, ChangeDetector, DuplicateDetector, StabilityDetector};
use crate::model::{Frame, MonitorInfo};
use crate::storage::SessionStore;
use crate::windows::{DxgiCapturer, GdiCapturer};

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    Start(CaptureSource),
    Stop,
    Pause,
    Resume,
    UpdateConfig(CaptureConfig),
    TestCapture(CaptureSource),
}

#[derive(Debug, Clone)]
pub enum WorkerEvent {
    SessionStarted(String),
    StateChanged(CaptureState),
    FrameCaptured {
        frame_index: u64,
        timestamp_ms: u64,
    },
    ChangeDetected {
        frame_index: u64,
        diff_ratio: f64,
    },
    FrameStable {
        frame_index: u64,
    },
    SlideSaved {
        slide_number: u32,
        png_filename: String,
    },
    MonitorLost(String),
    Error(String),
    TestFrame(Vec<u8>, u32, u32),
    Progress {
        saved_count: u32,
        current_state: CaptureState,
    },
    BlackFrameDetected,
    ProtectedContent,
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub sample_interval_ms: u64,
    pub stability_frames: u32,
    pub animation_timeout_ms: u64,
    pub change_threshold: f64,
    pub black_threshold: f64,
    pub filter_duplicates: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 500,
            stability_frames: 3,
            animation_timeout_ms: 10000,
            change_threshold: 0.01,
            black_threshold: 0.95,
            filter_duplicates: true,
        }
    }
}

pub struct CaptureWorker {
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    cmd_tx: Sender<WorkerCommand>,
    event_rx: crossbeam_channel::Receiver<WorkerEvent>,
}

impl CaptureWorker {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::Builder::new()
            .name("capture-worker".into())
            .spawn(move || {
                WorkerLoop::new(cmd_rx, event_tx).run(running_clone);
            })
            .expect("Failed to spawn capture worker thread");

        Self {
            handle: Some(handle),
            running,
            cmd_tx,
            event_rx,
        }
    }

    pub fn command_tx(&self) -> Sender<WorkerCommand> {
        self.cmd_tx.clone()
    }

    pub fn event_rx(&self) -> crossbeam_channel::Receiver<WorkerEvent> {
        self.event_rx.clone()
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerLoop {
    cmd_rx: crossbeam_channel::Receiver<WorkerCommand>,
    event_tx: crossbeam_channel::Sender<WorkerEvent>,
    dxgi_capturer: DxgiCapturer,
    gdi_capturer: GdiCapturer,
    change_detector: ChangeDetector,
    stability_detector: StabilityDetector,
    duplicate_detector: DuplicateDetector,
    black_frame_detector: BlackFrameDetector,
    state: CaptureState,
    config: CaptureConfig,
    source: Option<CaptureSource>,
    session: Option<SessionStore>,
    pending_save: Option<Frame>,
    slide_count: u32,
    last_slide_hash: Option<String>,
    animation_timer: Option<std::time::Instant>,
    first_frame_saved: bool,
}

impl WorkerLoop {
    fn new(
        cmd_rx: crossbeam_channel::Receiver<WorkerCommand>,
        event_tx: crossbeam_channel::Sender<WorkerEvent>,
    ) -> Self {
        Self {
            cmd_rx,
            event_tx,
            dxgi_capturer: DxgiCapturer::new(),
            gdi_capturer: GdiCapturer::new(),
            change_detector: ChangeDetector::new(CaptureConfig::default().change_threshold),
            stability_detector: StabilityDetector::new(CaptureConfig::default().stability_frames),
            duplicate_detector: DuplicateDetector::new(),
            black_frame_detector: BlackFrameDetector::new(CaptureConfig::default().black_threshold),
            state: CaptureState::Idle,
            config: CaptureConfig::default(),
            source: None,
            session: None,
            pending_save: None,
            slide_count: 0,
            last_slide_hash: None,
            animation_timer: None,
            first_frame_saved: false,
        }
    }

    fn run(&mut self, running: Arc<AtomicBool>) {
        info!("Capture worker started");

        while running.load(Ordering::SeqCst) {
            self.process_commands();

            if self.state == CaptureState::Running || self.state == CaptureState::WaitingForStable {
                match self.do_capture_cycle() {
                    Ok(()) => {}
                    Err(e) => {
                        let msg = format!("Capture cycle error: {}", e);
                        error!("{}", msg);
                        let _ = self.event_tx.send(WorkerEvent::Error(msg.clone()));
                        if msg.contains("DXGI access lost") {
                            warn!("DXGI lost, pausing capture");
                            self.state = CaptureState::Paused;
                            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                        } else if msg.contains("No capturer initialized")
                            && capture_backend_unavailable_message().is_some()
                        {
                            warn!("Capture not available on this platform. PPTX test mode.");
                            self.state = CaptureState::Idle;
                            let _ = self.event_tx.send(WorkerEvent::Error(
                                capture_backend_unavailable_message().unwrap().into(),
                            ));
                        } else {
                            self.state = CaptureState::Error;
                            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                        }
                    }
                }
            }

            if self.state.is_active() {
                thread::sleep(Duration::from_millis(self.config.sample_interval_ms));
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }

        self.finish_session();
        info!("Capture worker stopped");
    }

    fn process_commands(&mut self) {
        loop {
            let cmd = match self.cmd_rx.try_recv() {
                Ok(c) => c,
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    info!("Command channel disconnected");
                    return;
                }
            };

            match cmd {
                WorkerCommand::Start(source) => {
                    if self.session.is_some() {
                        self.finish_session();
                        if self.pending_save.is_some()
                            || self.session.as_ref().is_some_and(|s| !s.completed)
                        {
                            let _ = self.event_tx.send(WorkerEvent::Error("Previous session has unsaved work; resume or restart to recover it before starting another session".into()));
                            continue;
                        }
                        self.session = None;
                    }
                    info!("Starting capture with source: {}", source.display_name());
                    self.source = Some(source.clone());
                    self.state = CaptureState::Running;
                    self.slide_count = 0;
                    self.last_slide_hash = None;
                    self.animation_timer = None;
                    self.first_frame_saved = false;
                    self.pending_save = None;
                    self.change_detector.reset();
                    self.stability_detector.reset();

                    let monitor = self.create_monitor_info_for_source(&source);
                    match monitor {
                        Ok(mut mon) => {
                            // If window is selected, clip capture region to window bounds
                            let mut window_client_w = 0u32;
                            let mut window_client_h = 0u32;
                            if source.window_hwnd != 0 {
                                // Use client rect for accurate content capture (excludes title bar, borders, shadow)
                                if let Ok(window_rect) =
                                    crate::windows::get_client_window_rect(source.window_hwnd)
                                {
                                    // Validate window rect - skip if off-screen or too small
                                    if window_rect.width > 10
                                        && window_rect.height > 10
                                        && window_rect.x > -32000
                                        && window_rect.y > -32000
                                        && window_rect.x < 50000
                                        && window_rect.y < 50000
                                    {
                                        let rx = mon.region.x.max(window_rect.x);
                                        // Save client dims for PrintWindow-based capture (RDP compatible)
                                        window_client_w = window_rect.width;
                                        window_client_h = window_rect.height;
                                        let ry = mon.region.y.max(window_rect.y);
                                        let rr = (mon.region.x + mon.region.width as i32)
                                            .min(window_rect.x + window_rect.width as i32);
                                        let rb = (mon.region.y + mon.region.height as i32)
                                            .min(window_rect.y + window_rect.height as i32);
                                        if rr > rx && rb > ry {
                                            let new_w = (rr - rx) as u32;
                                            let new_h = (rb - ry) as u32;
                                            mon.region =
                                                crate::model::Region::new(rx, ry, new_w, new_h);
                                            log::info!(
                                                "Capture clipped to window: {}x{} @ ({},{})",
                                                new_w,
                                                new_h,
                                                rx,
                                                ry
                                            );
                                        } else {
                                            log::warn!("Window is outside monitor bounds, capturing full monitor");
                                        }
                                    } else {
                                        log::warn!("Window has invalid rect (minimized/off-screen), capturing full monitor");
                                    }
                                }
                            }
                            if should_use_display_capturer(&source) {
                                match self.dxgi_capturer.initialize(&mon) {
                                    Ok(()) => info!("DXGI capturer initialized"),
                                    Err(e) => {
                                        warn!("DXGI init failed ({}), falling back to GDI", e);
                                        match self.gdi_capturer.initialize(&mon) {
                                            Ok(()) => {
                                                info!("GDI capturer initialized (fallback)");
                                                self.gdi_capturer.set_window_hwnd(
                                                    source.window_hwnd,
                                                    window_client_w,
                                                    window_client_h,
                                                );
                                            }
                                            Err(e2) => {
                                                let msg = format!(
                                                    "Both DXGI and GDI failed: {} / {}",
                                                    e, e2
                                                );
                                                error!("{}", msg);
                                                self.state = CaptureState::Error;
                                                let _ = self.event_tx.send(WorkerEvent::Error(msg));
                                                continue;
                                            }
                                        }
                                    }
                                }
                            } else {
                                match self.gdi_capturer.initialize(&mon) {
                                    Ok(()) => {
                                        info!("GDI capturer initialized");
                                        self.gdi_capturer.set_window_hwnd(
                                            source.window_hwnd,
                                            window_client_w,
                                            window_client_h,
                                        );
                                    }
                                    Err(e) => {
                                        let msg = format!("GDI init failed: {}", e);
                                        error!("{}", msg);
                                        self.state = CaptureState::Error;
                                        let _ = self.event_tx.send(WorkerEvent::Error(msg));
                                        continue;
                                    }
                                }
                            }

                            let out_dir = std::path::PathBuf::from(&source.output_dir);
                            let session = match SessionStore::create(
                                &out_dir,
                                &source.output_filename,
                                &source.page_ratio,
                                &source.image_fit,
                            ) {
                                Ok(store) => store,
                                Err(e) => {
                                    let msg =
                                        format!("Failed to initialize output directory: {}", e);
                                    error!("{}", msg);
                                    self.state = CaptureState::Error;
                                    let _ = self.event_tx.send(WorkerEvent::Error(msg));
                                    continue;
                                }
                            };
                            let _ = self.event_tx.send(WorkerEvent::SessionStarted(
                                session.output_path().display().to_string(),
                            ));
                            self.session = Some(session);

                            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                        }
                        Err(e) => {
                            let msg = format!("Failed to get monitor info: {}", e);
                            error!("{}", msg);
                            self.state = CaptureState::Error;
                            let _ = self.event_tx.send(WorkerEvent::Error(msg));
                        }
                    }
                }
                WorkerCommand::Stop => {
                    info!("Stopping capture");
                    self.state = if self.finish_session() {
                        CaptureState::Stopped
                    } else {
                        CaptureState::Paused
                    };
                    self.dxgi_capturer.release();
                    let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                }
                WorkerCommand::Pause => {
                    info!("Pausing capture");
                    self.state = CaptureState::Paused;
                    self.dxgi_capturer.release();
                    let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                }
                WorkerCommand::Resume => {
                    info!("Resuming capture");
                    if let Some(ref src) = self.source.clone() {
                        let monitor = self.create_monitor_info_for_source(src);
                        if let Ok(mon) = monitor {
                            if should_use_display_capturer(src) {
                                let _ = self.dxgi_capturer.initialize(&mon);
                            } else {
                                if self.gdi_capturer.initialize(&mon).is_ok() {
                                    self.gdi_capturer.set_window_hwnd(src.window_hwnd, 0, 0);
                                }
                            }
                            self.state = CaptureState::Running;
                        } else {
                            // Monitor no longer available — try GDI fallback
                            log::warn!("Monitor unavailable on resume, trying GDI");
                            self.state = CaptureState::Running;
                        }
                    } else {
                        log::warn!("No source set on resume");
                        self.state = CaptureState::Stopped;
                    }
                    let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                }
                WorkerCommand::UpdateConfig(cfg) => {
                    info!("Updating capture config");
                    self.config = cfg;
                    self.change_detector
                        .set_threshold(self.config.change_threshold);
                    self.stability_detector
                        .set_required_stable(self.config.stability_frames);
                    self.black_frame_detector
                        .set_threshold(self.config.black_threshold);
                }
                WorkerCommand::TestCapture(source) => {
                    info!("Test capture requested");
                    let mut monitor = self.create_monitor_info_for_source(&source);
                    // Also clip to window for test capture
                    if let Ok(ref mut mon) = monitor {
                        if source.window_hwnd != 0 {
                            if let Ok(window_rect) =
                                crate::windows::get_client_window_rect(source.window_hwnd)
                            {
                                if window_rect.width > 10
                                    && window_rect.height > 10
                                    && window_rect.x > -32000
                                    && window_rect.y > -32000
                                {
                                    let rx = mon.region.x.max(window_rect.x);
                                    let ry = mon.region.y.max(window_rect.y);
                                    let rr = (mon.region.x + mon.region.width as i32)
                                        .min(window_rect.x + window_rect.width as i32);
                                    let rb = (mon.region.y + mon.region.height as i32)
                                        .min(window_rect.y + window_rect.height as i32);
                                    if rr > rx && rb > ry {
                                        mon.region = crate::model::Region::new(
                                            rx,
                                            ry,
                                            (rr - rx) as u32,
                                            (rb - ry) as u32,
                                        );
                                        log::info!(
                                            "Test capture clipped to window: {}x{}",
                                            mon.region.width,
                                            mon.region.height
                                        );
                                    }
                                }
                            }
                        }
                    }
                    match monitor {
                        Ok(mon) => {
                            if !should_use_display_capturer(&source) {
                                let mut test_gdi = GdiCapturer::new();
                                match test_gdi.initialize(&mon) {
                                    Ok(()) => {
                                        test_gdi.set_window_hwnd(source.window_hwnd, 0, 0);
                                        match test_gdi.capture_frame() {
                                            Ok(frame) => {
                                                let thumb = frame.thumbnail(320, 240);
                                                let _ = self
                                                    .event_tx
                                                    .send(WorkerEvent::TestFrame(thumb, 320, 240));
                                            }
                                            Err(e) => {
                                                let _ = self.event_tx.send(WorkerEvent::Error(
                                                    format!("Test window capture failed: {}", e),
                                                ));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = self.event_tx.send(WorkerEvent::Error(format!(
                                            "Test window capture init failed: {}",
                                            e
                                        )));
                                    }
                                }
                                continue;
                            }

                            let mut test_dxgi = DxgiCapturer::new();
                            match test_dxgi.initialize(&mon) {
                                Ok(()) => {
                                    match test_dxgi.capture_frame(2000) {
                                        Ok(Some(frame)) => {
                                            let thumb = frame.thumbnail(320, 240);
                                            let _ = self
                                                .event_tx
                                                .send(WorkerEvent::TestFrame(thumb, 320, 240));
                                        }
                                        Ok(None) => {
                                            let _ = self.event_tx.send(WorkerEvent::Error(
                                                "Test capture timed out".into(),
                                            ));
                                        }
                                        Err(e) => {
                                            info!("DXGI test failed ({}), trying GDI", e);
                                            let mut test_gdi = GdiCapturer::new();
                                            if test_gdi.initialize(&mon).is_ok() {
                                                if let Ok(frame) = test_gdi.capture_frame() {
                                                    let thumb = frame.thumbnail(320, 240);
                                                    let _ = self.event_tx.send(
                                                        WorkerEvent::TestFrame(thumb, 320, 240),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    test_dxgi.release();
                                }
                                Err(e) => {
                                    let _ = self.event_tx.send(WorkerEvent::Error(format!(
                                        "Test capture init failed: {}",
                                        e
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(WorkerEvent::Error(format!("Monitor lookup failed: {}", e)));
                        }
                    }
                }
            }
        }
    }

    fn do_capture_cycle(&mut self) -> Result<()> {
        // Never capture over a failed save. Resume retries exactly that frame first.
        if let Some(frame) = self.pending_save.clone() {
            if let Err(error) = self.save_frame(&frame) {
                let _ = self.event_tx.send(WorkerEvent::Error(format!("Retry save failed: {error:#}. Resolve the output problem and resume to retry this slide.")));
            }
            return Ok(());
        }
        let frame = if self.dxgi_capturer.is_initialized() {
            match self
                .dxgi_capturer
                .capture_frame(self.config.sample_interval_ms as u32)?
            {
                Some(f) => f,
                None => return Ok(()),
            }
        } else if self.gdi_capturer.is_initialized() {
            self.gdi_capturer.capture_frame()?
        } else {
            return Err(anyhow::anyhow!("No capturer initialized"));
        };

        if self.black_frame_detector.is_black(&frame) {
            warn!("Black/blank frame detected at index {}", frame.frame_index);
            if self.state == CaptureState::Running || self.state == CaptureState::WaitingForStable {
                self.state = CaptureState::ProtectedOrBlack;
                let _ = self.event_tx.send(WorkerEvent::BlackFrameDetected);
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
            }
            return Ok(());
        }

        if self.state == CaptureState::ProtectedOrBlack {
            self.state = CaptureState::Running;
            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));

            // Just exited a black/protected transition (e.g. PPT slide change).
            // The current frame is almost certainly the new slide content.
            // Force-save it immediately (save_frame handles duplicates internally).
            self.change_detector.detect_change(&frame);
            self.change_detector.update_reference(&frame);
            if let Err(e) = self.save_frame(&frame) {
                error!("Failed to save frame after black recovery: {}", e);
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "Black-recovery save failed: {}",
                    e
                )));
            }
            return Ok(());
        }

        // Save first frame immediately on starting capture
        if !self.first_frame_saved {
            self.change_detector.detect_change(&frame);
            self.change_detector.update_reference(&frame);
            if let Err(e) = self.save_frame(&frame) {
                error!("Failed to save first frame: {}", e);
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "First frame save failed: {}",
                    e
                )));
                return Ok(());
            }
            // Still continue with normal detection after the first save
        }

        let (changed, diff_ratio) = self.change_detector.detect_change(&frame);
        let _ = self.event_tx.send(WorkerEvent::FrameCaptured {
            frame_index: frame.frame_index,
            timestamp_ms: frame.timestamp_ms,
        });

        if changed {
            // If we were already stabilizing a previous slide, save it before resetting.
            // This prevents rapid PPT slide transitions from losing intermediate slides.
            // Save the partially-stabilized slide before a new transition resets everything.
            // Without this, rapid PPT transitions cause intermediate slides to be lost.
            if self.state == CaptureState::WaitingForStable {
                // First try: stability detector has a pending stable frame (stable_count > 0)
                let mut saved = false;
                let pending_frame = self.stability_detector.get_pending_frame().cloned();
                if let Some(ref pframe) = pending_frame {
                    log::info!("Saving pending stable frame before rapid transition");
                    if let Err(e) = self.save_frame(pframe) {
                        self.report_rapid_transition_save_failure("pending stable frame", e);
                        return Ok(());
                    } else {
                        saved = true;
                    }
                }
                // Fallback: save the change detector's reference frame (last known good frame).
                // This catches the case where stability was just reset (stable_count = 0).
                if !saved {
                    if let Some(ref ref_frame) = self.change_detector.get_reference_frame().cloned()
                    {
                        log::info!("Saving reference frame as fallback before rapid transition");
                        if let Err(e) = self.save_frame(ref_frame) {
                            self.report_rapid_transition_save_failure(
                                "fallback reference frame",
                                e,
                            );
                            return Ok(());
                        }
                    }
                }
            }
            self.state = CaptureState::WaitingForStable;
            self.animation_timer = Some(std::time::Instant::now());
            self.stability_detector.reset();
            let _ = self.event_tx.send(WorkerEvent::ChangeDetected {
                frame_index: frame.frame_index,
                diff_ratio,
            });
            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
        }

        if self.state == CaptureState::WaitingForStable {
            let is_stable = self.stability_detector.check_stable(&frame);
            let timed_out = self
                .animation_timer
                .map(|t| t.elapsed() >= Duration::from_millis(self.config.animation_timeout_ms))
                .unwrap_or(false);

            if is_stable || timed_out {
                self.state = CaptureState::Stable;
                let _ = self.event_tx.send(WorkerEvent::FrameStable {
                    frame_index: frame.frame_index,
                });
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));

                if let Err(e) = self.save_frame(&frame) {
                    error!("Failed to save frame: {}", e);
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::Error(format!("Save failed: {}", e)));
                    return Ok(());
                }

                self.state = CaptureState::Running;
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
            }
        }

        self.change_detector.update_reference(&frame);
        Ok(())
    }

    fn report_rapid_transition_save_failure(&self, frame_kind: &str, error: anyhow::Error) {
        let message = format!("Rapid-transition {frame_kind} save failed: {error:#}");
        error!("{message}");
        let _ = self.event_tx.send(WorkerEvent::Error(message));
    }

    fn save_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.pending_save.is_none() {
            self.pending_save = Some(frame.clone());
        }
        let pending = self.pending_save.as_ref().unwrap().clone();
        let result = self.commit_frame(&pending);
        if result.is_ok() {
            self.pending_save = None;
            self.first_frame_saved = true;
        } else {
            self.state = CaptureState::Paused;
            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
        }
        result
    }

    fn finish_session(&mut self) -> bool {
        if let Some(frame) = self.pending_save.clone() {
            if let Err(error) = self.save_frame(&frame) {
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "Pending slide could not be saved: {error}"
                )));
                return false;
            }
        }
        if let Some(session) = self.session.as_mut() {
            if session.completed {
                return true;
            }
            if let Err(error) = session.finish() {
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "Session finalization failed: {error}"
                )));
                return false;
            }
        }
        true
    }

    fn commit_frame(&mut self, frame: &Frame) -> Result<()> {
        self.state = CaptureState::Saving;
        let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));

        let content_hash = self.duplicate_detector.compute_hash(frame);

        if self.config.filter_duplicates {
            if let Some(ref last_hash) = self.last_slide_hash {
                if *last_hash == content_hash {
                    info!("Duplicate slide detected, skipping");
                    self.state = CaptureState::Running;
                    return Ok(());
                }
            }
        }

        let slide_number = self.slide_count + 1;
        let png_filename = format!("slide_{:04}.png", slide_number);
        let png_relative = format!("slides/{}", png_filename);

        let source_name = self
            .source
            .as_ref()
            .map(|s| s.window_title.clone())
            .unwrap_or_default();
        let monitor_name = self
            .source
            .as_ref()
            .map(|s| s.monitor_description.clone())
            .unwrap_or_default();

        let record = crate::model::SlideRecord::new(crate::model::SlideRecordInput {
            slide_number,
            png_filename: png_filename.clone(),
            png_relative_path: png_relative,
            frame_index: frame.frame_index,
            width: frame.width,
            height: frame.height,
            content_hash: content_hash.clone(),
            source_name,
            monitor_name,
        });

        self.session
            .as_mut()
            .context("Session not initialized")?
            .save(frame, record)?;
        self.slide_count = slide_number;

        self.last_slide_hash = Some(content_hash);

        let _ = self.event_tx.send(WorkerEvent::SlideSaved {
            slide_number,
            png_filename,
        });

        info!("Slide {} saved (frame {})", slide_number, frame.frame_index);
        self.state = CaptureState::Running;
        let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
        Ok(())
    }

    fn create_monitor_info_for_source(&self, source: &CaptureSource) -> Result<MonitorInfo> {
        let monitors = crate::windows::enumerate_monitors()?;
        let mon = monitors
            .into_iter()
            .find(|m| m.hmonitor == source.monitor_hmonitor)
            .ok_or_else(|| anyhow::anyhow!("Monitor {} not found", source.monitor_hmonitor))?;
        Ok(mon)
    }
}

fn should_use_display_capturer(source: &CaptureSource) -> bool {
    #[cfg(target_os = "macos")]
    {
        source.use_dxgi && !source.is_window_selected()
    }
    #[cfg(not(target_os = "macos"))]
    {
        source.use_dxgi
    }
}

fn capture_backend_unavailable_message() -> Option<&'static str> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        None
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Some("Capture only works on Windows and Apple Silicon macOS. This build is for PPTX testing only.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_first_frame_retries_same_page_without_success_event_or_gap() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (events, received) = crossbeam_channel::unbounded();
        let mut worker = WorkerLoop::new(rx, events);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        let output = worker.session.as_ref().unwrap().output_path();
        std::fs::create_dir(&output).unwrap();
        let frame = Frame::new(vec![0, 0, 255, 255], 1, 1, 4, 17, 0);
        assert!(worker.save_frame(&frame).is_err());
        assert_eq!(worker.slide_count, 0);
        assert!(!worker.first_frame_saved);
        assert!(worker.last_slide_hash.is_none());
        assert_eq!(worker.state, CaptureState::Paused);
        assert!(!received
            .try_iter()
            .any(|event| matches!(event, WorkerEvent::SlideSaved { .. })));
        worker.do_capture_cycle().unwrap();
        assert_eq!(worker.state, CaptureState::Paused);
        assert_eq!(worker.slide_count, 0);
        std::fs::remove_dir(output).unwrap();
        // Retry before touching a capturer; its source has deliberately not been initialized.
        worker.do_capture_cycle().unwrap();
        assert_eq!(worker.slide_count, 1);
        assert!(worker.first_frame_saved);
        assert!(worker.pending_save.is_none());
        let records = &worker.session.as_ref().unwrap().records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].frame_index, 17);
        assert_eq!(
            received
                .try_iter()
                .filter(|event| matches!(
                    event,
                    WorkerEvent::SlideSaved {
                        slide_number: 1,
                        ..
                    }
                ))
                .count(),
            1
        );
        let next = Frame::new(vec![255, 0, 0, 255], 1, 1, 4, 18, 0);
        worker.save_frame(&next).unwrap();
        assert_eq!(worker.slide_count, 2);
        worker.finish_session();
        assert!(worker.session.as_ref().unwrap().completed);
    }

    #[test]
    fn stop_keeps_png_and_journal_failures_retryable() {
        for fail_journal in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (tx, rx) = crossbeam_channel::unbounded();
            let (events, _received) = crossbeam_channel::unbounded();
            let mut worker = WorkerLoop::new(rx, events);
            worker.session =
                Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
            let session_dir = worker.session.as_ref().unwrap().directory.clone();
            let blocker = if fail_journal {
                let journal = session_dir.join("session.json");
                std::fs::remove_file(&journal).unwrap();
                journal
            } else {
                session_dir.join("slides/slide_0001.tmp.png")
            };
            std::fs::create_dir(&blocker).unwrap();
            assert!(worker
                .save_frame(&Frame::new(vec![255; 4], 1, 1, 4, 42, 0))
                .is_err());
            tx.send(WorkerCommand::Stop).unwrap();
            worker.process_commands();
            assert_eq!(worker.state, CaptureState::Paused);
            assert!(worker.pending_save.is_some());
            assert_eq!(worker.slide_count, 0);
            std::fs::remove_dir(blocker).unwrap();
            tx.send(WorkerCommand::Stop).unwrap();
            worker.process_commands();
            assert_eq!(worker.state, CaptureState::Stopped);
            let recovered = SessionStore::open(&session_dir).unwrap();
            assert!(recovered.completed);
            assert_eq!(recovered.records.len(), 1);
            assert_eq!(recovered.records[0].frame_index, 42);
        }
    }

    #[test]
    fn shutdown_preserves_incomplete_journal_when_publication_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (events, _received) = crossbeam_channel::unbounded();
        let mut worker = WorkerLoop::new(rx, events);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        std::fs::create_dir(worker.session.as_ref().unwrap().output_path()).unwrap();
        assert!(worker
            .save_frame(&Frame::new(vec![255; 4], 1, 1, 4, 1, 0))
            .is_err());
        worker.run(Arc::new(AtomicBool::new(false)));
        let reopened = SessionStore::open(&worker.session.as_ref().unwrap().directory).unwrap();
        assert!(!reopened.completed);
        assert_eq!(reopened.records.len(), 1);
    }

    #[test]
    fn rapid_transition_save_failure_emits_complete_worker_error() {
        let (_command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let worker = WorkerLoop::new(command_rx, event_tx);
        let error = anyhow::Error::msg("disk full").context("writing slide PNG");

        worker.report_rapid_transition_save_failure("pending stable frame", error);

        let WorkerEvent::Error(message) = event_rx.try_recv().expect("worker error event") else {
            panic!("expected a worker error event");
        };
        assert_eq!(
            message,
            "Rapid-transition pending stable frame save failed: writing slide PNG: disk full"
        );
    }

    #[test]
    fn rapid_transition_fallback_failure_emits_complete_worker_error() {
        let (_command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let worker = WorkerLoop::new(command_rx, event_tx);
        let error = anyhow::Error::msg("permission denied").context("saving fallback PNG");

        worker.report_rapid_transition_save_failure("fallback reference frame", error);

        let WorkerEvent::Error(message) = event_rx.try_recv().expect("worker error event") else {
            panic!("expected a worker error event");
        };
        assert_eq!(
            message,
            "Rapid-transition fallback reference frame save failed: saving fallback PNG: permission denied"
        );
    }

    #[test]
    fn macos_is_not_reported_as_an_unsupported_capture_backend() {
        #[cfg(target_os = "macos")]
        assert!(capture_backend_unavailable_message().is_none());
    }

    #[test]
    fn macos_window_sources_use_the_window_capturer() {
        #[cfg(target_os = "macos")]
        {
            let source = CaptureSource::new(7, "Slides".into(), 42, "Display".into());
            assert!(!should_use_display_capturer(&source));
        }
    }
}
