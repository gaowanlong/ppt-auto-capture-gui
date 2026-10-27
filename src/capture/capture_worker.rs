use anyhow::{Context, Result};
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;

use super::{worker_event_channel, WorkerEventReceiver, WorkerEventSender};
use crate::capture::capture_source::CaptureSource;
use crate::capture::capture_state::CaptureState;
use crate::detection::{
    BlackFrameDetector, ChangeDetector, DuplicateDetector, MotionFilter, StabilityDetector,
};
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
    StartFailed(String),
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

impl CaptureConfig {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.sample_interval_ms = self.sample_interval_ms.clamp(100, 5000);
        self.stability_frames = self.stability_frames.clamp(1, 10);
        self.animation_timeout_ms = self.animation_timeout_ms.clamp(1000, 30000);
        self.change_threshold = if self.change_threshold.is_finite() {
            self.change_threshold.clamp(0.001, 0.5)
        } else {
            defaults.change_threshold
        };
        self.black_threshold = if self.black_threshold.is_finite() {
            self.black_threshold.clamp(0.5, 1.0)
        } else {
            defaults.black_threshold
        };
        self
    }
}

pub struct CaptureWorker {
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    cmd_tx: Sender<WorkerCommand>,
    event_rx: WorkerEventReceiver,
}

impl CaptureWorker {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = worker_event_channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::Builder::new()
            .name("capture-worker".into())
            .spawn(move || {
                let errors = event_tx.clone();
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    WorkerLoop::new(cmd_rx, event_tx).run(running_clone);
                })).is_err() {
                    error!("Capture worker panicked; saved session remains recoverable");
                    let _ = errors.send(WorkerEvent::Error("Capture worker stopped unexpectedly; saved pages can be recovered on restart. See application log.".into()));
                }
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

    pub fn event_rx(&self) -> WorkerEventReceiver {
        self.event_rx.clone()
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if self.handle.is_some() {
            let _ = self.cmd_tx.send(WorkerCommand::Stop);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

struct WorkerLoop {
    cmd_rx: crossbeam_channel::Receiver<WorkerCommand>,
    event_tx: WorkerEventSender,
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
    motion_filter: MotionFilter,
    desktop_retry_at: Option<std::time::Instant>,
    queued_command: Option<WorkerCommand>,
}

impl WorkerLoop {
    fn new(
        cmd_rx: crossbeam_channel::Receiver<WorkerCommand>,
        event_tx: WorkerEventSender,
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
            motion_filter: MotionFilter::default(),
            desktop_retry_at: None,
            queued_command: None,
        }
    }

    fn run(&mut self, running: Arc<AtomicBool>) {
        info!("Capture worker started");
        let mut health_at = std::time::Instant::now();

        while running.load(Ordering::SeqCst) {
            self.process_commands();
            if health_at.elapsed() >= Duration::from_secs(60) {
                crate::windows::log_capture_resources();
                info!(
                    "Capture health: saved_slides={}, pending_save={}, event_backlog={}",
                    self.slide_count,
                    self.pending_save.is_some(),
                    self.event_tx.len()
                );
                health_at = std::time::Instant::now();
            }

            let available = crate::windows::input_desktop_available();
            if self.check_desktop(available)
                && self
                    .desktop_retry_at
                    .is_none_or(|time| std::time::Instant::now() >= time)
            {
                match self.restore_capture_source() {
                    Ok(()) => {
                        self.prepare_resume();
                        self.desktop_retry_at = None;
                        let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                    }
                    Err(error) => {
                        warn!("Capture source still unavailable: {error:#}");
                        self.desktop_retry_at =
                            Some(std::time::Instant::now() + Duration::from_secs(2));
                    }
                }
            }

            if available && self.should_capture() {
                match self.do_capture_cycle() {
                    Ok(()) => {}
                    Err(e) => {
                        let msg = format!("Capture cycle error: {}", e);
                        error!("{}", msg);
                        let _ = self.event_tx.send(WorkerEvent::Error(msg.clone()));
                        if cfg!(target_os = "windows") {
                            self.state = CaptureState::WaitingForDesktop;
                            self.desktop_retry_at =
                                Some(std::time::Instant::now() + Duration::from_secs(2));
                            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                        } else if msg.contains("DXGI access lost") {
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

            let delay = if self.state.is_active() {
                self.config.sample_interval_ms
            } else {
                100
            };
            if !self.wait_for_work(Duration::from_millis(delay)) {
                break;
            }
        }

        self.finish_session();
        info!("Capture worker stopped");
    }

    fn should_capture(&self) -> bool {
        matches!(
            self.state,
            CaptureState::Running | CaptureState::WaitingForStable | CaptureState::ProtectedOrBlack
        )
    }

    fn prepare_resume(&mut self) {
        self.state = CaptureState::Running;
        self.first_frame_saved = false;
        self.change_detector.reset();
        self.stability_detector.reset();
        self.animation_timer = None;
    }

    /// Returns true only when an automatically suspended session may resume.
    fn check_desktop(&mut self, available: bool) -> bool {
        if !available && self.should_capture() {
            self.dxgi_capturer.release();
            self.state = CaptureState::WaitingForDesktop;
            self.desktop_retry_at = None;
            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
        }
        available && self.state == CaptureState::WaitingForDesktop
    }

    fn restore_capture_source(&mut self) -> Result<()> {
        let source = self
            .source
            .as_ref()
            .context("No capture source selected")?
            .clone();
        let mut monitor = self.create_monitor_info_for_source(&source)?;
        let (width, height) = if source.window_hwnd != 0 {
            let region = crate::windows::get_client_window_rect(source.window_hwnd)?;
            anyhow::ensure!(
                region.is_valid(),
                "Selected window is minimized or unavailable"
            );
            monitor.region = region;
            (region.width, region.height)
        } else {
            (0, 0)
        };
        self.dxgi_capturer.release();
        if !should_use_display_capturer(&source) || self.dxgi_capturer.initialize(&monitor).is_err()
        {
            self.gdi_capturer.initialize(&monitor)?;
            self.gdi_capturer
                .set_window_hwnd(source.window_hwnd, width, height);
        }
        Ok(())
    }

    fn wait_for_work(&mut self, duration: Duration) -> bool {
        match self.cmd_rx.recv_timeout(duration) {
            Ok(command) => {
                self.queued_command = Some(command);
                true
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => true,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => false,
        }
    }

    fn process_commands(&mut self) {
        loop {
            let cmd = match self
                .queued_command
                .take()
                .map(Ok)
                .unwrap_or_else(|| self.cmd_rx.try_recv())
            {
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
                    self.motion_filter = MotionFilter::default();
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
                                                let _ = self
                                                    .event_tx
                                                    .send(WorkerEvent::StartFailed(msg));
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
                                        let _ = self.event_tx.send(WorkerEvent::StartFailed(msg));
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
                                    let _ = self.event_tx.send(WorkerEvent::StartFailed(msg));
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
                            let _ = self.event_tx.send(WorkerEvent::StartFailed(msg));
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
                    match self.restore_capture_source() {
                        Ok(()) => self.prepare_resume(),
                        Err(error) => {
                            self.state = CaptureState::Paused;
                            let _ = self.event_tx.send(WorkerEvent::Error(format!(
                                "Cannot resume selected source: {error:#}"
                            )));
                        }
                    }
                    let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                }
                WorkerCommand::UpdateConfig(cfg) => {
                    info!("Updating capture config");
                    self.config = cfg.normalized();
                    self.change_detector
                        .set_threshold(self.config.change_threshold);
                    self.stability_detector
                        .set_required_stable(self.config.stability_frames);
                    self.black_frame_detector
                        .set_threshold(self.config.black_threshold);
                }
                WorkerCommand::TestCapture(source) => match self.capture_preview_frame(&source) {
                    Ok(frame) => {
                        let _ = self.event_tx.send(make_preview_event(&frame));
                    }
                    Err(error) => {
                        let _ = self.event_tx.send(WorkerEvent::Error(format!(
                            "Test screenshot failed: {error:#}"
                        )));
                    }
                },
            }
        }
    }

    fn capture_preview_frame(&self, source: &CaptureSource) -> Result<Frame> {
        anyhow::ensure!(
            crate::windows::input_desktop_available(),
            "Desktop unavailable; unlock Windows and try again"
        );
        let mut monitor = self.create_monitor_info_for_source(source)?;
        let (width, height) = if source.window_hwnd != 0 {
            let region = crate::windows::get_client_window_rect(source.window_hwnd)?;
            anyhow::ensure!(
                region.is_valid(),
                "Selected window is unavailable or minimized"
            );
            monitor.region = region;
            (region.width, region.height)
        } else {
            (0, 0)
        };
        if should_use_display_capturer(source) {
            let mut display = DxgiCapturer::new();
            let frame = display
                .initialize(&monitor)
                .and_then(|_| display.capture_frame(2000));
            display.release();
            if let Ok(Some(frame)) = frame {
                return Ok(frame);
            }
        }
        // Use GDI on both initialization and capture failure, with the exact target window.
        let mut capture = GdiCapturer::new();
        capture.initialize(&monitor)?;
        capture.set_window_hwnd(source.window_hwnd, width, height);
        capture.capture_frame()
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

        self.process_frame(&frame)
    }

    fn process_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.black_frame_detector.is_black(frame) {
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
            self.change_detector.detect_change(frame);
            self.change_detector.update_reference(frame);
            if let Err(e) = self.save_frame(frame) {
                error!("Failed to save frame after black recovery: {}", e);
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "Black-recovery save failed: {}",
                    e
                )));
            }
            return Ok(());
        }

        let broad_transition = self.motion_filter.observe(frame);

        // Save first frame immediately on starting capture
        if !self.first_frame_saved {
            self.change_detector.detect_change(frame);
            self.change_detector.update_reference(frame);
            if let Err(e) = self.save_frame(frame) {
                error!("Failed to save first frame: {}", e);
                let _ = self.event_tx.send(WorkerEvent::Error(format!(
                    "First frame save failed: {}",
                    e
                )));
                return Ok(());
            }
            // Still continue with normal detection after the first save
        }

        let (changed, diff_ratio) = self.change_detector.detect_change(frame);
        let changed =
            changed || (self.config.filter_duplicates && !self.motion_filter.is_duplicate(frame));
        self.send_telemetry(WorkerEvent::FrameCaptured {
            frame_index: frame.frame_index,
            timestamp_ms: frame.timestamp_ms,
        });

        if changed
            && (!self.config.filter_duplicates
                || broad_transition
                || self.stability_detector.get_pending_frame().is_some())
        {
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
            self.animation_timer
                .get_or_insert_with(std::time::Instant::now);
            self.stability_detector.reset();
            self.send_telemetry(WorkerEvent::ChangeDetected {
                frame_index: frame.frame_index,
                diff_ratio,
            });
            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
        }

        if changed && self.config.filter_duplicates && self.state != CaptureState::WaitingForStable
        {
            self.state = CaptureState::WaitingForStable;
            self.animation_timer = Some(std::time::Instant::now());
            self.stability_detector.reset();
        }

        if self.state == CaptureState::WaitingForStable {
            let is_stable = self.stability_detector.check_stable(frame);
            let timed_out = self
                .animation_timer
                .map(|t| t.elapsed() >= Duration::from_millis(self.config.animation_timeout_ms))
                .unwrap_or(false);

            if is_stable || timed_out {
                self.state = CaptureState::Stable;
                self.send_telemetry(WorkerEvent::FrameStable {
                    frame_index: frame.frame_index,
                });
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));

                if let Err(e) = self.save_frame(frame) {
                    error!("Failed to save frame: {}", e);
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::Error(format!("Save failed: {}", e)));
                    return Ok(());
                }

                self.state = CaptureState::Running;
                self.animation_timer = None;
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
            }
        }

        self.change_detector.update_reference(frame);
        Ok(())
    }

    fn send_telemetry(&self, event: WorkerEvent) {
        // Keep a stalled/minimized UI from accumulating sampling notifications all day.
        // SlideSaved, errors and state changes use the reliable event path instead.
        if self.event_tx.len() < 128 {
            let _ = self.event_tx.try_send(event);
        }
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
            if self.motion_filter.is_duplicate(frame) {
                self.state = CaptureState::Running;
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                return Ok(());
            }
            if let Some(ref last_hash) = self.last_slide_hash {
                if *last_hash == content_hash {
                    info!("Duplicate slide detected, skipping");
                    self.state = CaptureState::Running;
                    let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
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

        let previous_path = self
            .session
            .as_ref()
            .context("Session not initialized")?
            .output_path_at_count(self.slide_count as usize);
        self.session
            .as_mut()
            .context("Session not initialized")?
            .save(frame, record)?;
        self.slide_count = slide_number;

        let path = self.session.as_ref().unwrap().output_path();
        if path != previous_path {
            let _ = self
                .event_tx
                .send(WorkerEvent::SessionStarted(path.display().to_string()));
        }

        self.last_slide_hash = Some(content_hash);
        self.motion_filter.mark_saved(frame);

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

fn make_preview_event(frame: &Frame) -> WorkerEvent {
    let (pixels, width, height) = frame.thumbnail_with_dimensions(320, 240);
    WorkerEvent::TestFrame(pixels, width, height)
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
    fn recording_rolls_over_at_50_pages_without_gaps_or_empty_extra_decks() {
        let root = tempfile::tempdir().unwrap();
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (events, received) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        worker.config.filter_duplicates = false;
        let session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let folder = session.directory.clone();
        worker.session = Some(session);
        for index in 1..=101 {
            let pixel = [index as u8, 0, 255, 255];
            worker
                .save_frame(&Frame::new(pixel.to_vec(), 1, 1, 4, index, 0))
                .unwrap();
            if index == 50 {
                assert!(!folder.join("deck-part-002.pptx").exists());
            }
            if index == 100 {
                assert!(!folder.join("deck-part-003.pptx").exists());
            }
        }
        assert_eq!(worker.slide_count, 101);
        for (name, count) in [
            ("deck.pptx", 50),
            ("deck-part-002.pptx", 50),
            ("deck-part-003.pptx", 1),
        ] {
            let file = std::fs::File::open(folder.join(name)).unwrap();
            let mut archive = zip::ZipArchive::new(file).unwrap();
            for page in 1..=count {
                assert!(archive
                    .by_name(&format!("ppt/slides/slide{page}.xml"))
                    .is_ok());
            }
            assert!(archive
                .by_name(&format!("ppt/slides/slide{}.xml", count + 1))
                .is_err());
            use std::io::Read;
            let mut image = Vec::new();
            archive
                .by_name("ppt/media/image1.png")
                .unwrap()
                .read_to_end(&mut image)
                .unwrap();
            let expected_blue = match name {
                "deck.pptx" => 1,
                "deck-part-002.pptx" => 51,
                _ => 101,
            };
            assert_eq!(
                image::load_from_memory(&image)
                    .unwrap()
                    .to_rgb8()
                    .get_pixel(0, 0)
                    .0,
                [255, 0, expected_blue]
            );
        }
        assert!(received.try_iter().any(|event| matches!(event, WorkerEvent::SessionStarted(path) if path.ends_with("deck-part-003.pptx"))));
        assert!(worker.finish_session());
    }

    #[test]
    fn rollover_save_failure_keeps_prior_deck_and_retries_the_same_frame() {
        let root = tempfile::tempdir().unwrap();
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (events, received) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        worker.config.filter_duplicates = false;
        let session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let folder = session.directory.clone();
        worker.session = Some(session);
        for index in 1..=50 {
            worker
                .save_frame(&Frame::new(
                    vec![index as u8, 0, 255, 255],
                    1,
                    1,
                    4,
                    index,
                    0,
                ))
                .unwrap();
        }
        let prior = std::fs::read(folder.join("deck.pptx")).unwrap();
        let blocked = folder.join("deck-part-002.pptx");
        std::fs::create_dir(&blocked).unwrap();
        let frame = Frame::new(vec![51, 0, 255, 255], 1, 1, 4, 51, 0);
        assert!(worker.save_frame(&frame).is_err());
        assert_eq!(worker.slide_count, 50);
        assert_eq!(worker.pending_save.as_ref().unwrap().frame_index, 51);
        assert_eq!(std::fs::read(folder.join("deck.pptx")).unwrap(), prior);
        assert!(!received.try_iter().any(|event| matches!(event, WorkerEvent::SessionStarted(path) if path.ends_with("deck-part-002.pptx"))));
        std::fs::remove_dir(&blocked).unwrap();
        worker.save_frame(&frame).unwrap();
        assert_eq!(worker.slide_count, 51);
        assert_eq!(worker.session.as_ref().unwrap().records[50].frame_index, 51);
        assert!(blocked.is_file());
        assert!(received.try_iter().any(|event| matches!(event, WorkerEvent::SessionStarted(path) if path.ends_with("deck-part-002.pptx"))));
    }

    #[test]
    fn duplicate_at_the_split_boundary_does_not_create_an_empty_deck() {
        let root = tempfile::tempdir().unwrap();
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (events, _) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        worker.config.filter_duplicates = false;
        let session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let folder = session.directory.clone();
        worker.session = Some(session);
        let mut last = Frame::new(vec![0; 4], 1, 1, 4, 0, 0);
        for index in 1..=50 {
            last = Frame::new(vec![index as u8, 0, 255, 255], 1, 1, 4, index, 0);
            worker.save_frame(&last).unwrap();
        }
        worker.config.filter_duplicates = true;
        worker.save_frame(&last).unwrap();
        assert_eq!(worker.slide_count, 50);
        assert!(!folder.join("deck-part-002.pptx").exists());
    }

    #[test]
    fn new_deck_does_not_rebuild_the_previous_50_slides() {
        let root = tempfile::tempdir().unwrap();
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (events, _) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        worker.config.filter_duplicates = false;
        let session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let folder = session.directory.clone();
        worker.session = Some(session);
        for index in 1..=50 {
            worker
                .save_frame(&Frame::new(
                    vec![index as u8, 0, 255, 255],
                    1,
                    1,
                    4,
                    index,
                    0,
                ))
                .unwrap();
        }
        let first = std::fs::read(folder.join("deck.pptx")).unwrap();
        std::fs::remove_file(folder.join("slides/slide_0001.png")).unwrap();
        for index in 51..=52 {
            worker
                .save_frame(&Frame::new(
                    vec![index as u8, 0, 255, 255],
                    1,
                    1,
                    4,
                    index,
                    0,
                ))
                .unwrap();
        }
        assert_eq!(std::fs::read(folder.join("deck.pptx")).unwrap(), first);
        assert_eq!(worker.slide_count, 52);
        assert!(worker.finish_session());
    }

    #[test]
    fn stop_interrupts_a_long_sample_wait_and_is_processed_once() {
        let (commands, rx) = crossbeam_channel::unbounded();
        let (events, received) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        commands.send(WorkerCommand::Stop).unwrap();
        let start = std::time::Instant::now();
        assert!(worker.wait_for_work(Duration::from_secs(5)));
        assert!(start.elapsed() < Duration::from_secs(1));
        worker.process_commands();
        assert_eq!(worker.state, CaptureState::Stopped);
        assert_eq!(
            received
                .try_iter()
                .filter(|e| matches!(e, WorkerEvent::StateChanged(CaptureState::Stopped)))
                .count(),
            1
        );
        drop(commands);
        assert!(!worker.wait_for_work(Duration::from_secs(5)));
    }

    #[test]
    fn unsafe_config_does_not_create_zero_delay_capture_or_invalid_thresholds() {
        let (commands, rx) = crossbeam_channel::unbounded();
        let (events, _) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        commands
            .send(WorkerCommand::UpdateConfig(CaptureConfig {
                sample_interval_ms: 0,
                stability_frames: 0,
                animation_timeout_ms: u64::MAX,
                change_threshold: f64::NAN,
                black_threshold: f64::INFINITY,
                filter_duplicates: true,
            }))
            .unwrap();
        worker.process_commands();
        assert!(worker.config.sample_interval_ms >= 100);
        assert!(worker.config.stability_frames >= 1);
        assert!(worker.config.animation_timeout_ms <= 30000);
        assert!(worker.config.change_threshold.is_finite());
        assert!(worker.config.black_threshold.is_finite());
    }

    #[test]
    fn failed_start_reports_a_retryable_start_failure() {
        let (commands, rx) = crossbeam_channel::unbounded();
        let (events, received) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, events);
        commands
            .send(WorkerCommand::Start(CaptureSource::new(
                0,
                String::new(),
                u64::MAX,
                "Unavailable".into(),
            )))
            .unwrap();
        worker.process_commands();
        assert_eq!(worker.state, CaptureState::Error);
        assert!(received
            .try_iter()
            .any(|event| matches!(event, WorkerEvent::StartFailed(_))));
        assert!(worker.session.is_none());
    }

    #[test]
    fn preview_event_matches_actual_wide_tall_and_tiny_pixel_buffers() {
        for (width, height, expected) in [
            (1920, 1080, (320, 180)),
            (900, 1600, (135, 240)),
            (4, 4, (4, 4)),
        ] {
            let frame = Frame::new(
                vec![255; (width * height * 4) as usize],
                width,
                height,
                width * 4,
                0,
                0,
            );
            let WorkerEvent::TestFrame(pixels, w, h) = make_preview_event(&frame) else {
                panic!("preview expected");
            };
            assert_eq!((w, h), expected);
            assert_eq!(pixels.len(), (w * h * 4) as usize);
        }
    }

    #[test]
    fn duplicate_save_finishes_in_running_state_for_the_ui() {
        let dir = tempfile::tempdir().unwrap();
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (tx, events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        let frame = Frame::new(vec![255; 16], 2, 2, 8, 0, 0);
        worker.save_frame(&frame).unwrap();
        for _ in events.try_iter() {}
        worker.save_frame(&frame).unwrap();
        let states: Vec<_> = events
            .try_iter()
            .filter_map(|e| {
                if let WorkerEvent::StateChanged(s) = e {
                    Some(s)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(states.last(), Some(&CaptureState::Running));
        assert_eq!(worker.slide_count, 1);
    }

    #[test]
    fn control_event_storm_is_bounded_and_preserves_latest_stop_and_page() {
        let (_commands, rx) = crossbeam_channel::unbounded();
        let (tx, events) = worker_event_channel();
        let worker = WorkerLoop::new(rx, tx);
        for number in 1..=100_000 {
            worker
                .event_tx
                .send(WorkerEvent::StateChanged(CaptureState::Saving))
                .unwrap();
            worker
                .event_tx
                .send(WorkerEvent::SlideSaved {
                    slide_number: number,
                    png_filename: format!("slide_{number}.png"),
                })
                .unwrap();
            worker
                .event_tx
                .send(WorkerEvent::Error(format!("transient {number}")))
                .unwrap();
        }
        worker
            .event_tx
            .send(WorkerEvent::StateChanged(CaptureState::Stopped))
            .unwrap();
        assert!(
            events.len() <= 64,
            "Control event backlog grew to {}",
            events.len()
        );
        let events: Vec<_> = events.try_iter().collect();
        assert!(events.iter().any(|event| matches!(
            event,
            WorkerEvent::SlideSaved {
                slide_number: 100_000,
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|event| matches!(event, WorkerEvent::StateChanged(CaptureState::Stopped))));
    }

    #[test]
    fn settled_content_inside_a_previous_video_region_is_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        for i in 0..40 {
            let mut frame = Frame::new(vec![255; 64 * 36 * 4], 64, 36, 256, i, 0);
            let value = if i < 8 && i % 2 == 0 { 0 } else { 160 };
            for y in 9..27 {
                for x in 16..48 {
                    frame.data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 3].fill(value);
                }
            }
            worker.process_frame(&frame).unwrap();
        }
        assert_eq!(worker.slide_count, 2);
    }

    #[test]
    fn slow_ui_does_not_accumulate_per_frame_notifications() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        let frame = Frame::new(vec![255; 64 * 36 * 4], 64, 36, 256, 1, 0);
        for _ in 0..1000 {
            worker.process_frame(&frame).unwrap();
        }
        assert!(
            events.len() <= 256,
            "Per-frame UI events grew without bound: {}",
            events.len()
        );
        assert!(events.try_iter().any(|event| matches!(
            event,
            WorkerEvent::SlideSaved {
                slide_number: 1,
                ..
            }
        )));
    }

    #[test]
    fn dropping_worker_stops_its_background_thread() {
        let worker = CaptureWorker::new();
        let running = worker.running.clone();
        drop(worker);
        assert!(!running.load(Ordering::SeqCst));
    }

    #[test]
    fn first_frame_after_unlock_saves_a_new_page_and_skips_an_unchanged_page() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        let first = Frame::new(vec![255; 64 * 36 * 4], 64, 36, 256, 1, 0);
        worker.process_frame(&first).unwrap();
        worker.prepare_resume();
        worker.process_frame(&first).unwrap();
        assert_eq!(worker.slide_count, 1);
        worker.prepare_resume();
        worker
            .process_frame(&Frame::new(vec![100; 64 * 36 * 4], 64, 36, 256, 2, 0))
            .unwrap();
        assert_eq!(worker.slide_count, 2);
    }

    #[test]
    fn same_template_fast_pages_keep_intermediate_content() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        for value in [255, 128, 128, 64, 64, 64, 64, 64] {
            let mut frame = Frame::new(vec![255; 64 * 36 * 4], 64, 36, 256, 1, 0);
            frame.data[..64 * 6 * 4].fill(value);
            worker.process_frame(&frame).unwrap();
        }
        assert_eq!(worker.slide_count, 3);
    }

    #[test]
    fn broad_rapid_slide_changes_keep_intermediate_pages() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        for value in [255, 128, 64, 64, 64, 64, 64] {
            worker
                .process_frame(&Frame::new(vec![value; 64 * 36 * 4], 64, 36, 256, 1, 0))
                .unwrap();
        }
        assert_eq!(worker.slide_count, 3);
    }

    #[test]
    fn video_motion_timeout_and_new_slide_do_not_create_repeated_pages() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        worker.state = CaptureState::Running;
        for i in 0..20_000 {
            let background = if i < 10_000 { 255 } else { 100 };
            let mut frame = Frame::new(vec![background; 64 * 36 * 4], 64, 36, 256, i, i * 200);
            for y in 9..27 {
                for x in 16..48 {
                    let offset = (y * 64 + x) * 4;
                    frame.data[offset..offset + 3].fill(if i % 2 == 0 { 0 } else { 180 });
                }
            }
            if i % 25 == 0 && worker.state == CaptureState::WaitingForStable {
                worker.animation_timer = Some(std::time::Instant::now() - Duration::from_secs(11));
            }
            worker.process_frame(&frame).unwrap();
            // Exercise a consuming UI too; do not let the test itself accumulate telemetry.
            for _ in events.try_iter() {}
        }
        assert_eq!(worker.slide_count, 2);
        assert_eq!(worker.session.as_ref().unwrap().records.len(), 2);
        worker.finish_session();
    }

    #[test]
    fn lock_wait_does_not_override_user_pause_or_pending_save() {
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.state = CaptureState::Running;
        worker.pending_save = Some(Frame::new(vec![255; 4], 1, 1, 4, 42, 0));
        assert!(!worker.check_desktop(false));
        assert_eq!(worker.state, CaptureState::WaitingForDesktop);
        assert!(worker.check_desktop(true));
        assert_eq!(worker.pending_save.as_ref().unwrap().frame_index, 42);
        worker.state = CaptureState::Paused;
        assert!(!worker.check_desktop(true));
        assert_eq!(worker.state, CaptureState::Paused);
        worker.state = CaptureState::Stopped;
        assert!(!worker.check_desktop(true));
    }

    #[test]
    fn black_transition_can_return_to_capture_without_manual_resume() {
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.state = CaptureState::Running;
        worker
            .process_frame(&Frame::new(vec![0; 4], 1, 1, 4, 1, 0))
            .unwrap();
        assert_eq!(worker.state, CaptureState::ProtectedOrBlack);
        assert!(worker.should_capture());
    }

    #[test]
    fn cursor_motion_does_not_save_another_slide() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (tx, _events) = worker_event_channel();
        let mut worker = WorkerLoop::new(rx, tx);
        worker.session =
            Some(SessionStore::create(dir.path(), "deck.pptx", "16:9", "fit").unwrap());
        let first = Frame::new(vec![255; 200 * 100 * 4], 200, 100, 800, 1, 0);
        worker.save_frame(&first).unwrap();
        for i in 0..100 {
            let mut frame = first.clone();
            let offset = ((20 * 200 + i) * 4) as usize;
            frame.data[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
            worker.save_frame(&frame).unwrap();
        }
        assert_eq!(worker.slide_count, 1);
    }

    #[test]
    fn failed_first_frame_retries_same_page_without_success_event_or_gap() {
        let dir = tempfile::tempdir().unwrap();
        let (_tx, rx) = crossbeam_channel::unbounded();
        let (events, received) = worker_event_channel();
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
            let (events, _received) = worker_event_channel();
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
        let (events, _received) = worker_event_channel();
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
        let (event_tx, event_rx) = worker_event_channel();
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
        let (event_tx, event_rx) = worker_event_channel();
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
