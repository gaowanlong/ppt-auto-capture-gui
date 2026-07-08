use anyhow::{Context, Result};
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};

use crate::capture::capture_source::CaptureSource;
use crate::capture::capture_state::CaptureState;
use crate::detection::{ChangeDetector, StabilityDetector, DuplicateDetector, BlackFrameDetector};
use crate::model::{Frame, MonitorInfo};
use crate::pptx::PptxWriter;
use crate::storage::{ImageStore, ManifestStore};
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
    StateChanged(CaptureState),
    FrameCaptured { frame_index: u64, timestamp_ms: u64 },
    ChangeDetected { frame_index: u64, diff_ratio: f64 },
    FrameStable { frame_index: u64 },
    SlideSaved { slide_number: u32, png_filename: String },
    MonitorLost(String),
    Error(String),
    TestFrame(Vec<u8>, u32, u32),
    Progress { saved_count: u32, current_state: CaptureState },
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
    image_store: Option<ImageStore>,
    manifest_store: Option<ManifestStore>,
    pptx_writer: Option<PptxWriter>,
    slide_count: u32,
    last_slide_hash: Option<String>,
    animation_timer: Option<std::time::Instant>,
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
            change_detector: ChangeDetector::new(0.01),
            stability_detector: StabilityDetector::new(3),
            duplicate_detector: DuplicateDetector::new(),
            black_frame_detector: BlackFrameDetector::new(0.95),
            state: CaptureState::Idle,
            config: CaptureConfig::default(),
            source: None,
            image_store: None,
            manifest_store: None,
            pptx_writer: None,
            slide_count: 0,
            last_slide_hash: None,
            animation_timer: None,
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
                    info!("Starting capture with source: {}", source.display_name());
                    self.source = Some(source.clone());
                    self.state = CaptureState::Running;
                    self.slide_count = 0;
                    self.last_slide_hash = None;
                    self.animation_timer = None;

                    let monitor = self.create_monitor_info_for_source(&source);
                    match monitor {
                        Ok(mon) => {
                            if source.use_dxgi {
                                match self.dxgi_capturer.initialize(&mon) {
                                    Ok(()) => info!("DXGI capturer initialized"),
                                    Err(e) => {
                                        warn!("DXGI init failed ({}), falling back to GDI", e);
                                        match self.gdi_capturer.initialize(&mon) {
                                            Ok(()) => info!("GDI capturer initialized (fallback)"),
                                            Err(e2) => {
                                                let msg = format!("Both DXGI and GDI failed: {} / {}", e, e2);
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
                                    Ok(()) => info!("GDI capturer initialized"),
                                    Err(e) => {
                                        let msg = format!("GDI init failed: {}", e);
                                        error!("{}", msg);
                                        self.state = CaptureState::Error;
                                        let _ = self.event_tx.send(WorkerEvent::Error(msg));
                                        continue;
                                    }
                                }
                            }

                            let out_dir = std::path::PathBuf::from("output");
                            let _ = std::fs::create_dir_all(&out_dir);
                            self.image_store = Some(ImageStore::new(out_dir.clone()));
                            self.manifest_store = Some(ManifestStore::new(out_dir.join("manifest.jsonl")));
                            self.pptx_writer = Some(PptxWriter::new(&out_dir.join("output.pptx")));

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
                    self.state = CaptureState::Stopped;
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
                            if src.use_dxgi {
                                let _ = self.dxgi_capturer.initialize(&mon);
                            } else {
                                let _ = self.gdi_capturer.initialize(&mon);
                            }
                            self.state = CaptureState::Running;
                            let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
                        }
                    }
                }
                WorkerCommand::UpdateConfig(cfg) => {
                    info!("Updating capture config");
                    self.config = cfg;
                    self.change_detector.set_threshold(self.config.change_threshold);
                    self.stability_detector.set_required_stable(self.config.stability_frames);
                    self.black_frame_detector.set_threshold(self.config.black_threshold);
                }
                WorkerCommand::TestCapture(source) => {
                    info!("Test capture requested");
                    let monitor = self.create_monitor_info_for_source(&source);
                    match monitor {
                        Ok(mon) => {
                            let mut test_dxgi = DxgiCapturer::new();
                            match test_dxgi.initialize(&mon) {
                                Ok(()) => {
                                    match test_dxgi.capture_frame(2000) {
                                        Ok(Some(frame)) => {
                                            let thumb = frame.thumbnail(320, 240);
                                            let _ = self.event_tx.send(WorkerEvent::TestFrame(thumb, 320, 240));
                                        }
                                        Ok(None) => {
                                            let _ = self.event_tx.send(WorkerEvent::Error("Test capture timed out".into()));
                                        }
                                        Err(e) => {
                                            info!("DXGI test failed ({}), trying GDI", e);
                                            let mut test_gdi = GdiCapturer::new();
                                            if test_gdi.initialize(&mon).is_ok() {
                                                if let Ok(frame) = test_gdi.capture_frame() {
                                                    let thumb = frame.thumbnail(320, 240);
                                                    let _ = self.event_tx.send(WorkerEvent::TestFrame(thumb, 320, 240));
                                                }
                                            }
                                        }
                                    }
                                    test_dxgi.release();
                                }
                                Err(e) => {
                                    let _ = self.event_tx.send(WorkerEvent::Error(format!("Test capture init failed: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = self.event_tx.send(WorkerEvent::Error(format!("Monitor lookup failed: {}", e)));
                        }
                    }
                }
            }
        }
    }

    fn do_capture_cycle(&mut self) -> Result<()> {
        let frame = if self.dxgi_capturer.is_initialized() {
            match self.dxgi_capturer.capture_frame(self.config.sample_interval_ms as u32)? {
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
        }

        let (changed, diff_ratio) = self.change_detector.detect_change(&frame);
        let _ = self.event_tx.send(WorkerEvent::FrameCaptured {
            frame_index: frame.frame_index,
            timestamp_ms: frame.timestamp_ms,
        });

        if changed {
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
            let timed_out = self.animation_timer
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
                    let _ = self.event_tx.send(WorkerEvent::Error(format!("Save failed: {}", e)));
                }

                self.state = CaptureState::Running;
                let _ = self.event_tx.send(WorkerEvent::StateChanged(self.state));
            }
        }

        self.change_detector.update_reference(&frame);
        Ok(())
    }

    fn save_frame(&mut self, frame: &Frame) -> Result<()> {
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

        self.slide_count += 1;
        let slide_number = self.slide_count;
        let png_filename = format!("slide_{:04}.png", slide_number);
        let png_relative = format!("slides/{}", png_filename);

        let source_name = self.source.as_ref()
            .map(|s| s.window_title.clone())
            .unwrap_or_default();
        let monitor_name = self.source.as_ref()
            .map(|s| s.monitor_description.clone())
            .unwrap_or_default();

        let record = crate::model::SlideRecord::new(
            slide_number,
            png_filename.clone(),
            png_relative,
            frame.frame_index,
            frame.width,
            frame.height,
            content_hash.clone(),
            source_name,
            monitor_name,
        );

        let image_store = self.image_store.as_ref().context("Image store not initialized")?;
        let png_path = image_store.save_png(frame, slide_number)?;

        if let Some(ref manifest_store) = self.manifest_store {
            manifest_store.append(&record)?;
        }

        if let Some(ref pptx_writer) = self.pptx_writer {
            pptx_writer.add_slide(&record, &png_path)?;
        }

        self.last_slide_hash = Some(content_hash);

        let _ = self.event_tx.send(WorkerEvent::SlideSaved {
            slide_number,
            png_filename,
        });

        info!("Slide {} saved (frame {})", slide_number, frame.frame_index);
        Ok(())
    }

    fn create_monitor_info_for_source(&self, source: &CaptureSource) -> Result<MonitorInfo> {
        let monitors = crate::windows::enumerate_monitors()?;
        let mon = monitors.into_iter()
            .find(|m| m.hmonitor == source.monitor_hmonitor)
            .ok_or_else(|| anyhow::anyhow!("Monitor {} not found", source.monitor_hmonitor))?;
        Ok(mon)
    }
}
