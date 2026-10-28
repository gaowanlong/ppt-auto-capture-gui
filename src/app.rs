use eframe::egui;
use log::{error, info, warn};

use crossbeam_channel::Sender;

use crate::capture::WorkerEventReceiver;
use crate::capture::{CaptureSource, CaptureState, CaptureWorker, WorkerCommand, WorkerEvent};
use crate::gui::dashboard::{DashboardPanel, DashboardRenderInput};
use crate::gui::display_panel::DisplayPanel;
use crate::gui::export_panel::ExportPanel;
use crate::gui::output_panel::OutputPanel;
use crate::gui::settings_panel::SettingsPanel;
use crate::gui::source_panel::SourcePanel;
use crate::i18n::{self, Language};

use crate::config::AppConfig;
use crate::diagnostics::{
    render_diagnostic_report, DiagnosticEntry, DiagnosticHistory, DiagnosticLevel,
    DiagnosticReportContext, LoggingSetup,
};
use crate::model::{MonitorInfo, WindowInfo};
use crate::storage::{detect_incomplete_session, recover_session};
use crate::windows::{enumerate_monitors, enumerate_windows};

pub struct PptAutoCaptureApp {
    config: AppConfig,
    dashboard: DashboardPanel,
    source_panel: SourcePanel,
    display_panel: DisplayPanel,
    settings_panel: SettingsPanel,
    output_panel: OutputPanel,
    export_panel: ExportPanel,
    worker: Option<CaptureWorker>,
    cmd_tx: Option<Sender<WorkerCommand>>,
    event_rx: Option<WorkerEventReceiver>,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    active_tab: Tab,
    pending_start: bool,
    pending_pause: bool,
    pending_stop: bool,
    pending_resume: bool,
    output_dir: String,
    recovery_available: bool,
    recovery_slides: u32,
    recovery_accepted: bool,
    recovery_declined: bool,
    recovery_rx: Option<std::sync::mpsc::Receiver<Result<Vec<std::path::PathBuf>, String>>>,
    recovery_error: Option<String>,
    language: Language,
    diagnostic_history: DiagnosticHistory,
    log_directory: Option<std::path::PathBuf>,
    log_file: Option<std::path::PathBuf>,
    test_capture_pending: bool,
    close_after_stop: bool,
}

#[derive(PartialEq)]
enum Tab {
    Dashboard,
    Source,
    Display,
    Settings,
    Output,
    Export,
}

impl PptAutoCaptureApp {
    pub fn new(logging: LoggingSetup) -> Self {
        let config = AppConfig::load();
        let mut app = Self {
            config: config.clone(),
            dashboard: DashboardPanel::new(),
            source_panel: SourcePanel::new(),
            display_panel: DisplayPanel::new(),
            settings_panel: SettingsPanel::new(),
            output_panel: OutputPanel::new_with_filename(&config.output_filename),
            export_panel: ExportPanel::new(&OutputPanel::new_with_filename(
                &config.output_filename,
            )),
            worker: None,
            cmd_tx: None,
            event_rx: None,
            monitors: Vec::new(),
            windows: Vec::new(),
            active_tab: Tab::Dashboard,
            pending_start: false,
            pending_pause: false,
            pending_stop: false,
            pending_resume: false,
            output_dir: config.output_dir.clone(),
            recovery_available: false,
            recovery_slides: 0,
            recovery_accepted: false,
            recovery_declined: false,
            recovery_rx: None,
            recovery_error: None,
            language: config.language,
            diagnostic_history: DiagnosticHistory::new(20),
            log_directory: logging.log_directory,
            log_file: logging.log_file,
            test_capture_pending: false,
            close_after_stop: false,
        };
        if let Some(warning) = logging.warning {
            warn!("{warning}");
            app.diagnostic_history.push(DiagnosticEntry::new(
                DiagnosticLevel::Warning,
                "logging",
                warning,
            ));
        }
        if let Some(log_directory) = app.log_directory.as_ref() {
            info!("Diagnostic log directory: {}", log_directory.display());
        }
        if let Some(log_file) = app.log_file.as_ref() {
            info!("Diagnostic log file: {}", log_file.display());
        }
        // Load saved settings into settings panel
        {
            let cfg = crate::capture::CaptureConfig {
                sample_interval_ms: config.sample_interval_ms,
                stability_frames: config.stability_frames,
                animation_timeout_ms: config.animation_timeout_ms,
                change_threshold: config.change_threshold,
                black_threshold: config.black_threshold,
                filter_duplicates: config.filter_duplicates,
            };
            app.settings_panel.load_from_config(&cfg);
        }
        app.display_panel.selected_hmonitor = config.last_monitor_hmonitor;
        app.display_panel.selected_description = config.last_monitor_description.clone();
        app.source_panel.selected_hwnd = config.last_window_hwnd;
        app.source_panel.selected_title = config.last_window_title.clone();
        // Restore full-screen mode flag if previous session was full-screen
        if config.last_full_screen {
            app.source_panel.full_screen_selected = true;
        }
        #[cfg(target_os = "macos")]
        apply_macos_source_default(&mut app.source_panel);
        app.output_panel.output_dir = config.output_dir.clone();
        app.output_panel.page_ratio = config.page_ratio.clone();
        app.output_panel.image_fit = config.image_fit.clone();
        app.output_panel.auto_crop = config.auto_crop;
        app.output_panel.keep_previous = config.keep_previous;
        app.export_panel.settings = app.output_panel.clone();
        app.dashboard.output_path = output_path(&app.output_panel);
        app.refresh_displays();
        app.check_recovery();
        app
    }

    fn record_error(&mut self, category: &str, message: String) {
        error!("{category}: {message}");
        self.dashboard.last_error = Some(message.clone());
        self.diagnostic_history.push(DiagnosticEntry::new(
            DiagnosticLevel::Error,
            category,
            message,
        ));
    }

    fn copy_diagnostics(&mut self, ctx: &egui::Context) {
        let source = if self.source_panel.full_screen_selected {
            "Full Screen"
        } else {
            self.source_panel.selected_title.as_str()
        };
        let display = self.display_panel.selected_description.as_str();
        let output_path = self.dashboard.output_path.clone();
        let report = render_diagnostic_report(
            DiagnosticReportContext {
                version: env!("CARGO_PKG_VERSION"),
                os: std::env::consts::OS,
                architecture: std::env::consts::ARCH,
                capture_state: self.dashboard.current_state.label(),
                source,
                display,
                output_path: &output_path,
                log_directory: self.log_directory.as_deref().and_then(|path| path.to_str()),
            },
            &self.diagnostic_history,
        );
        ctx.copy_text(report);
    }

    fn open_log_directory(&mut self) {
        let Some(directory) = self.log_directory.clone() else {
            self.record_error("diagnostics", "Log directory is unavailable".into());
            return;
        };
        self.open_directory(&directory);
    }

    fn open_directory(&mut self, directory: &std::path::Path) {
        #[cfg(target_os = "windows")]
        let command = "explorer";
        #[cfg(target_os = "macos")]
        let command = "open";
        #[cfg(target_os = "linux")]
        let command = "xdg-open";

        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        if let Err(error) = std::process::Command::new(command).arg(directory).spawn() {
            self.record_error(
                "diagnostics",
                format!("Unable to open log directory: {error}"),
            );
        }
    }

    fn check_recovery(&mut self) {
        match detect_incomplete_session(std::path::Path::new(&self.output_dir)) {
            Ok(Some(records)) => {
                self.recovery_available = true;
                self.recovery_slides = records.len() as u32;
                info!("Recovery: {} slides", self.recovery_slides);
            }
            Ok(None) => {}
            Err(error) => self.record_error(
                "recovery",
                format!("Cannot inspect saved sessions: {error:#}"),
            ),
        }
    }

    fn start_capture(&mut self) {
        if self.dashboard.session_active {
            return;
        }
        // Preserve the selected recovery root even if the application exits unexpectedly.
        apply_output_to_config(&self.output_panel, &mut self.config);
        self.output_dir = self.output_panel.output_dir.clone();
        if let Err(error) = self.config.save() {
            self.record_error(
                "config",
                format!("Unable to save recording settings for recovery: {error:#}"),
            );
        }
        let source = if self.source_panel.full_screen_selected {
            "Full Screen"
        } else {
            self.source_panel.selected_title.as_str()
        };
        info!(
            "Starting capture with source: {source}; display: {}",
            self.display_panel.selected_description
        );
        self.ensure_worker();
        let mut source = CaptureSource::new(
            self.source_panel.selected_hwnd,
            self.source_panel.selected_title.clone(),
            self.display_panel.selected_hmonitor,
            self.display_panel.selected_description.clone(),
        );
        apply_output_to_source(&self.output_panel, &mut source);
        if let Some(tx) = self.cmd_tx.as_ref() {
            let _ = tx.send(WorkerCommand::UpdateConfig(
                self.settings_panel.get_config(),
            ));
            let _ = tx.send(WorkerCommand::Start(source));
        }
        self.dashboard.session_active = true;
        self.dashboard.saved_slides_count = 0;
        self.dashboard.output_path = output_path(&self.output_panel);

        if self.source_panel.full_screen_selected {
            self.dashboard.source_window_title = "📺 Full Screen".to_string();
        } else {
            self.dashboard.source_window_title = self.source_panel.selected_title.clone();
        }
        self.dashboard.monitor_description = self.display_panel.selected_description.clone();
    }

    fn pause_capture(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(WorkerCommand::Pause);
        }
    }
    fn resume_capture(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(WorkerCommand::Resume);
        }
    }
    fn stop_capture(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            if tx.send(WorkerCommand::Stop).is_ok() {
                self.dashboard.stopping = true;
            } else {
                self.handle_worker_disconnected();
            }
        }
        // Wait for successful finalization. On error the worker stays alive and retryable.
    }

    fn stop_and_keep_screenshots(&mut self) {
        if let Some(tx) = &self.cmd_tx {
            if tx.send(WorkerCommand::StopAndKeepScreenshots).is_ok() {
                self.dashboard.stopping = true;
            } else {
                self.handle_worker_disconnected();
            }
        }
    }

    fn request_safe_close(&mut self) -> bool {
        if self.export_panel.is_busy() || self.recovery_rx.is_some() {
            if self.export_panel.is_busy() {
                self.active_tab = Tab::Export;
                self.export_panel.notify_close_blocked(self.language);
            }
            return true;
        }
        if self.dashboard.session_active {
            self.close_after_stop = true;
            self.active_tab = Tab::Dashboard;
            if !self.dashboard.stopping {
                self.stop_capture();
            }
            return true;
        }
        false
    }

    fn ensure_worker(&mut self) {
        if self.cmd_tx.is_none() {
            let worker = CaptureWorker::new();
            self.event_rx = Some(worker.event_rx());
            self.cmd_tx = Some(worker.command_tx());
            self.worker = Some(worker);
        }
    }

    fn send_test_capture(&mut self) {
        if self.test_capture_pending || self.dashboard.stopping {
            return;
        }
        if self.display_panel.selected_hmonitor == 0 {
            self.record_error(
                "capture",
                i18n::t_no_display_selected(self.language).to_string(),
            );
            return;
        }
        self.ensure_worker();
        let mut source = CaptureSource::new(
            self.source_panel.selected_hwnd,
            self.source_panel.selected_title.clone(),
            self.display_panel.selected_hmonitor,
            self.display_panel.selected_description.clone(),
        );
        apply_output_to_source(&self.output_panel, &mut source);

        if let Some(ref tx) = self.cmd_tx {
            match tx.send(WorkerCommand::TestCapture(source)) {
                Ok(()) => {
                    self.test_capture_pending = true;
                    info!("Test capture command sent");
                }
                Err(_) => self.handle_worker_disconnected(),
            }
        }
    }

    fn process_events(&mut self) {
        self.drain_events(true);
    }

    fn start_recovery(&mut self) {
        if self.recovery_rx.is_some() {
            return;
        }
        self.recovery_error = None;
        let root = std::path::PathBuf::from(&self.output_dir);
        let (tx, rx) = std::sync::mpsc::channel();
        match std::thread::Builder::new()
            .name("pptx-recovery".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| recover_session(&root))
                    .map_err(|_| "Recovery stopped unexpectedly".to_string())
                    .and_then(|result| result.map_err(|error| format!("{error:#}")));
                let _ = tx.send(result);
            }) {
            Ok(_) => self.recovery_rx = Some(rx),
            Err(error) => {
                let message = format!("Cannot start recovery: {error}");
                self.recovery_error = Some(message.clone());
                self.record_error("recovery", message);
            }
        }
    }

    fn poll_recovery(&mut self) {
        let Some(receiver) = self.recovery_rx.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(paths)) => {
                self.recovery_rx = None;
                self.dashboard.saved_slides_count = self.recovery_slides;
                self.recovery_accepted = true;
                if let Some(path) = paths.last() {
                    self.dashboard.output_path = path.display().to_string();
                }
            }
            Ok(Err(error)) => {
                self.recovery_rx = None;
                let message = format!("Recovery failed: {error}");
                self.recovery_error = Some(message.clone());
                self.record_error("recovery", message);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.recovery_rx = None;
                let message = "Recovery worker disconnected".to_string();
                self.recovery_error = Some(message.clone());
                self.record_error("recovery", message);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    fn drain_events(&mut self, unexpected_disconnect: bool) {
        // Drain at most one bounded batch; a fast producer cannot monopolize a UI frame.
        let Some(receiver) = self.event_rx.as_ref() else {
            return;
        };
        let mut pending = Vec::with_capacity(receiver.len().min(64));
        pending.extend(receiver.try_iter().take(64));
        for event in pending {
            match event {
                WorkerEvent::StartFailed(message) => {
                    self.close_after_stop = false;
                    self.dashboard.stopping = false;
                    self.dashboard.session_active = false;
                    self.dashboard.current_state = CaptureState::Error;
                    self.record_error("capture", message);
                }
                WorkerEvent::SessionStarted(path) => self.dashboard.output_path = path,
                WorkerEvent::ScreenshotsPreserved(path) => {
                    let message = match self.language {
                        Language::Chinese => format!("截图和恢复记录已保留在 {path}；PPTX 尚未生成，可重启后恢复或在图片转 PPTX 页面选择其中的 slides 文件夹。"),
                        Language::English => format!("Screenshots and the recovery journal are saved in {path}; PPTX was not generated. Recover on restart or import the slides folder."),
                    };
                    warn!("{message}");
                    self.dashboard.last_error = Some(message.clone());
                    self.diagnostic_history.push(DiagnosticEntry::new(
                        DiagnosticLevel::Warning,
                        "recovery",
                        message,
                    ));
                }
                WorkerEvent::StateChanged(s) => {
                    if matches!(
                        s,
                        CaptureState::Paused | CaptureState::Stopped | CaptureState::Error
                    ) {
                        self.dashboard.stopping = false;
                    }
                    if matches!(s, CaptureState::Paused | CaptureState::Error) {
                        // A failed finalization must leave the window open for retry.
                        self.close_after_stop = false;
                    }
                    self.dashboard.current_state = s;
                    self.dashboard.state_message = s.label().to_string();
                    info!("Capture state changed: {s:?}");
                    if s == CaptureState::Stopped {
                        self.dashboard.session_active = false;
                        if let Some(mut worker) = self.worker.take() {
                            worker.stop();
                        }
                        self.cmd_tx = None;
                    }
                }
                WorkerEvent::SlideSaved { slide_number, .. } => {
                    self.dashboard.saved_slides_count = slide_number
                }
                WorkerEvent::Error(msg) => {
                    self.test_capture_pending = false;
                    self.record_error("worker", msg);
                }
                WorkerEvent::TestFrame(d, w, h) => {
                    self.test_capture_pending = false;
                    if self.dashboard.update_preview(d, w, h) {
                        if !self.dashboard.session_active {
                            self.dashboard.last_error = None;
                        }
                    } else {
                        self.record_error("preview", "Invalid screenshot preview; refresh the selected window/display and try again".into());
                    }
                }
                WorkerEvent::BlackFrameDetected => {
                    self.dashboard.state_message = "Black frame".into();
                }
                _ => {}
            }
        }
        if self
            .event_rx
            .as_ref()
            .is_some_and(|rx| rx.is_disconnected())
        {
            if unexpected_disconnect && self.cmd_tx.is_some() {
                self.handle_worker_disconnected();
            } else {
                self.event_rx = None;
            }
        }
    }

    fn handle_worker_disconnected(&mut self) {
        self.close_after_stop = false;
        self.dashboard.stopping = false;
        self.test_capture_pending = false;
        self.event_rx = None;
        self.cmd_tx = None;
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        self.dashboard.current_state = CaptureState::Error;
        self.dashboard.state_message = CaptureState::Error.label().to_string();
        self.dashboard.session_active = false;
        self.record_error("worker", "Capture worker disconnected".into());
    }

    fn refresh_windows(&mut self) {
        self.source_panel.refresh_requested = false;
        if let Ok(w) = enumerate_windows() {
            self.windows = w.clone();
            self.source_panel.windows = w.clone();

            // Auto-select the best window only when user hasn't intentionally
            // chosen full-screen capture AND no window is currently selected.
            if self.source_panel.selected_hwnd == 0 && !self.source_panel.full_screen_selected {
                // Sort by priority: slideshow first, then PPT
                let mut candidates: Vec<&WindowInfo> = w.iter().collect();
                candidates.sort_by(|a, b| {
                    let a_score = if a.is_powerpoint {
                        2
                    } else if a.title.to_lowercase().contains("powerpoint")
                        || a.title.to_lowercase().contains("ppt")
                    {
                        1
                    } else {
                        0
                    };
                    let b_score = if b.is_powerpoint {
                        2
                    } else if b.title.to_lowercase().contains("powerpoint")
                        || b.title.to_lowercase().contains("ppt")
                    {
                        1
                    } else {
                        0
                    };
                    b_score.cmp(&a_score)
                });
                if let Some(best) = candidates.first() {
                    self.source_panel.selected_hwnd = best.hwnd;
                    self.source_panel.selected_title = best.title.clone();
                    self.source_panel.status_text = format!("Auto-selected: {}", best.title);
                    info!("Auto-selected window: {} (0x{:X})", best.title, best.hwnd);
                }
            }
        }
    }

    fn refresh_displays(&mut self) {
        self.display_panel.refresh_requested = false;
        if let Ok(m) = enumerate_monitors() {
            #[cfg(target_os = "macos")]
            if !m
                .iter()
                .any(|monitor| monitor.hmonitor == self.display_panel.selected_hmonitor)
            {
                if let Some(monitor) = default_monitor(&m) {
                    self.display_panel.selected_hmonitor = monitor.hmonitor;
                    self.display_panel.selected_description = format!(
                        "{} ({}x{})",
                        monitor.output_name.trim(),
                        monitor.region.width,
                        monitor.region.height
                    );
                    self.display_panel.status_text =
                        format!("Selected: {}", monitor.output_name.trim());
                }
            }
            self.monitors = m.clone();
            self.display_panel.monitors = m;
        }
    }
}

fn default_monitor(monitors: &[MonitorInfo]) -> Option<&MonitorInfo> {
    monitors
        .iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors.first())
}

fn apply_macos_source_default(panel: &mut SourcePanel) {
    panel.selected_hwnd = 0;
    panel.selected_title.clear();
    panel.full_screen_selected = true;
}

fn apply_output_to_source(panel: &OutputPanel, source: &mut CaptureSource) {
    source.output_dir = panel.output_dir.clone();
    source.output_filename = panel.output_filename.clone();
    source.page_ratio = panel.page_ratio.clone();
    source.image_fit = panel.image_fit.clone();
    source.auto_crop = panel.auto_crop;
}

fn output_path(panel: &OutputPanel) -> String {
    format!(
        "{}/{}",
        panel
            .output_dir
            .trim_end_matches('/')
            .trim_end_matches('\\'),
        panel.output_filename
    )
}

fn apply_output_to_config(panel: &OutputPanel, config: &mut AppConfig) {
    config.output_dir = panel.output_dir.clone();
    config.output_filename = panel.output_filename.clone();
    config.page_ratio = panel.page_ratio.clone();
    config.image_fit = panel.image_fit.clone();
    config.auto_crop = panel.auto_crop;
    config.keep_previous = panel.keep_previous;
}

impl eframe::App for PptAutoCaptureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();
        self.export_panel.poll(self.language);
        self.poll_recovery();

        if self.close_after_stop
            && !self.dashboard.session_active
            && !self.dashboard.stopping
            && self.dashboard.current_state == CaptureState::Stopped
        {
            self.close_after_stop = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ctx.input(|input| input.viewport().close_requested()) && self.request_safe_close() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        if self.dashboard.open_session_requested {
            self.dashboard.open_session_requested = false;
            let path = std::path::PathBuf::from(&self.dashboard.output_path);
            if let Some(directory) = path.parent() {
                self.open_directory(directory);
            }
        }

        if self.recovery_available && !self.recovery_accepted && !self.recovery_declined {
            let mut open = true;
            egui::Window::new("Session Recovery")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading(i18n::t_session_recovery_title(self.language));
                    ui.label(i18n::t_recovery_msg(self.language, self.recovery_slides));
                    if self.recovery_rx.is_some() {
                        ui.label(match self.language {
                            Language::Chinese => "正在恢复截图并生成 PPTX，请稍候…",
                            Language::English => "Recovering screenshots and generating PPTX…",
                        });
                    }
                    if let Some(error) = &self.recovery_error {
                        ui.colored_label(egui::Color32::RED, error);
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                self.recovery_rx.is_none(),
                                egui::Button::new(i18n::t_recover(self.language)),
                            )
                            .clicked()
                        {
                            self.start_recovery();
                        }
                        if ui
                            .add_enabled(
                                self.recovery_rx.is_none(),
                                egui::Button::new(i18n::t_skip(self.language)),
                            )
                            .clicked()
                        {
                            self.recovery_declined = true;
                        }
                    });
                });
            if self.recovery_rx.is_some() {
                // Keep the recovery dialog open while the background writer runs.
                open = true;
            }
            if !open {
                self.recovery_declined = true;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }

        if self.pending_start {
            self.pending_start = false;
            if self.display_panel.selected_hmonitor == 0 {
                self.record_error(
                    "capture",
                    i18n::t_no_display_selected(self.language).to_string(),
                );
            } else {
                self.dashboard.last_error = None;
                self.start_capture();
            }
        }
        if self.pending_pause {
            self.pending_pause = false;
            self.pause_capture();
        }
        if self.pending_resume {
            self.pending_resume = false;
            self.resume_capture();
        }
        if self.pending_stop {
            self.pending_stop = false;
            self.stop_capture();
        }

        if self.source_panel.refresh_requested {
            self.refresh_windows();
        }
        if self.source_panel.move_requested && self.display_panel.selected_hmonitor != 0 {
            self.source_panel.move_requested = false;
            if let Some(mon) = self
                .monitors
                .iter()
                .find(|m| m.hmonitor == self.display_panel.selected_hmonitor)
            {
                let _ = crate::windows::move_window_to_monitor(
                    self.source_panel.selected_hwnd,
                    &mon.region,
                );
            }
        }
        if self.source_panel.maximize_requested {
            self.source_panel.maximize_requested = false;
            let _ = crate::windows::maximize_window(self.source_panel.selected_hwnd);
        }
        if self.display_panel.refresh_requested {
            self.refresh_displays();
        }

        egui::TopBottomPanel::top("bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Dashboard,
                    i18n::t_tab_dashboard(self.language),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Source,
                    i18n::t_tab_source(self.language),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Display,
                    i18n::t_tab_display(self.language),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Settings,
                    i18n::t_tab_settings(self.language),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Output,
                    i18n::t_tab_output(self.language),
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::Export,
                    match self.language {
                        Language::Chinese => "图片转 PPTX",
                        Language::English => "Create PPTX",
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(i18n::t_language_switch(self.language)).clicked() {
                        self.language = self.language.next();
                        let _ = self.config.save();
                    }
                    ui.label(format!("  {}", self.language.label()));
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Dashboard => {
                    // Sync output info to dashboard
                    if self.dashboard.output_path.is_empty() {
                        self.dashboard.output_path = output_path(&self.output_panel);
                    }
                    self.dashboard.render(
                        ui,
                        self.language,
                        DashboardRenderInput {
                            diagnostic_history: &self.diagnostic_history,
                            persistent_log_available: self.log_directory.is_some(),
                            start_requested: &mut self.pending_start,
                            pause_requested: &mut self.pending_pause,
                            stop_requested: &mut self.pending_stop,
                            resume_requested: &mut self.pending_resume,
                        },
                    );
                    if std::mem::take(&mut self.dashboard.keep_screenshots_requested) {
                        self.stop_and_keep_screenshots();
                    }
                    if self.dashboard.copy_diagnostics_requested {
                        self.dashboard.copy_diagnostics_requested = false;
                        self.copy_diagnostics(ctx);
                        self.dashboard.show_diagnostics_status(
                            i18n::t_diagnostics_copied(self.language).to_string(),
                        );
                    }
                    if self.dashboard.open_log_directory_requested {
                        self.dashboard.open_log_directory_requested = false;
                        self.open_log_directory();
                    }
                }
                Tab::Source => {
                    self.source_panel.render(
                        ui,
                        self.language,
                        self.display_panel.selected_hmonitor != 0,
                    );
                    if self.source_panel.test_requested {
                        self.source_panel.test_requested = false;
                        self.send_test_capture();
                    }
                    if self.windows.is_empty() {
                        self.refresh_windows();
                    }
                }
                Tab::Display => {
                    self.display_panel.render(ui, self.language);
                    if self.display_panel.test_capture_requested {
                        self.display_panel.test_capture_requested = false;
                        self.send_test_capture();
                    }
                    if self.monitors.is_empty() {
                        self.refresh_displays();
                    }
                }
                Tab::Settings => {
                    self.settings_panel.render(ui, self.language);
                    // Immediately push settings changes to the worker
                    if self.settings_panel.changed {
                        if let Some(ref tx) = self.cmd_tx {
                            let cfg = self.settings_panel.get_config();
                            let _ = tx.send(WorkerCommand::UpdateConfig(cfg));
                        }
                    }
                    self.settings_panel.changed = false;
                }
                Tab::Output => {
                    if self.dashboard.session_active {
                        ui.label(match self.language {
                            Language::Chinese => "录制期间输出设置已锁定，停止后可修改。",
                            Language::English => "Output settings are locked during recording.",
                        });
                    }
                    ui.add_enabled_ui(!self.dashboard.session_active, |ui| {
                        self.output_panel.render(ui, self.language)
                    });
                    // Handle open output directory button
                    if self.output_panel.open_output_requested {
                        self.output_panel.open_output_requested = false;
                        let dir = self.output_panel.output_dir.trim();
                        let dir = if dir.is_empty() { "." } else { dir };
                        log::info!("Opening output directory: {}", dir);
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer").arg(dir).spawn();
                        }
                        #[cfg(target_os = "macos")]
                        {
                            let _ = std::process::Command::new("open").arg(dir).spawn();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                        }
                    }
                    // Sync output filename to dashboard
                    if self.dashboard.output_path.is_empty() {
                        self.dashboard.output_path = output_path(&self.output_panel);
                    }
                }
                Tab::Export => {
                    self.export_panel.render(ui, self.language);
                    if let Some(directory) = self.export_panel.take_open_request() {
                        self.open_directory(&directory);
                    }
                }
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

impl Drop for PptAutoCaptureApp {
    fn drop(&mut self) {
        if !self.dashboard.session_active {
            apply_output_to_config(&self.output_panel, &mut self.config);
        }
        // Save settings panel values
        let scfg = self.settings_panel.get_config();
        self.config.sample_interval_ms = scfg.sample_interval_ms;
        self.config.stability_frames = scfg.stability_frames;
        self.config.animation_timeout_ms = scfg.animation_timeout_ms;
        self.config.change_threshold = scfg.change_threshold;
        self.config.black_threshold = scfg.black_threshold;
        self.config.filter_duplicates = scfg.filter_duplicates;
        self.config.last_window_hwnd = self.source_panel.selected_hwnd;
        self.config.last_window_title = self.source_panel.selected_title.clone();
        self.config.last_full_screen = self.source_panel.full_screen_selected;
        self.config.last_monitor_hmonitor = self.display_panel.selected_hmonitor;
        self.config.last_monitor_description = self.display_panel.selected_description.clone();
        self.config.language = self.language;
        let _ = self.config.save();
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::LoggingSetup;
    use crate::model::Region;

    #[test]
    fn export_settings_do_not_change_the_active_recording_output_or_crop() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        app.output_panel.output_dir = "recording-root".into();
        app.output_panel.auto_crop = false;
        app.export_panel.settings.output_dir = "export-root".into();
        app.export_panel.settings.auto_crop = true;
        let mut source = CaptureSource::new(0, String::new(), 1, String::new());
        apply_output_to_source(&app.output_panel, &mut source);
        assert_eq!(source.output_dir, "recording-root");
        assert!(!source.auto_crop);
    }

    #[test]
    fn closing_active_recording_requests_async_stop_and_keeps_window_open_on_failure() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (commands, received) = crossbeam_channel::unbounded();
        let (events, rx) = crate::capture::worker_event_channel();
        app.cmd_tx = Some(commands);
        app.event_rx = Some(rx);
        app.dashboard.session_active = true;
        assert!(app.request_safe_close());
        assert!(app.close_after_stop);
        assert!(app.dashboard.stopping);
        assert!(matches!(received.try_recv(), Ok(WorkerCommand::Stop)));
        events
            .send(WorkerEvent::StateChanged(CaptureState::Paused))
            .unwrap();
        app.process_events();
        assert!(!app.close_after_stop);
        assert!(!app.dashboard.stopping);
        assert!(app.dashboard.session_active);
    }

    #[test]
    fn failed_start_or_worker_disconnect_cancels_deferred_close() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (events, rx) = crate::capture::worker_event_channel();
        app.event_rx = Some(rx);
        app.dashboard.session_active = true;
        app.close_after_stop = true;
        events
            .send(WorkerEvent::StartFailed("failed".into()))
            .unwrap();
        app.process_events();
        assert!(!app.close_after_stop);
        assert_eq!(app.dashboard.current_state, CaptureState::Error);
        app.close_after_stop = true;
        app.dashboard.session_active = true;
        app.handle_worker_disconnected();
        assert!(!app.close_after_stop);
        assert_eq!(app.dashboard.current_state, CaptureState::Error);
    }

    #[test]
    fn preserve_screenshots_stops_the_session_and_allows_window_to_close() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (commands, received) = crossbeam_channel::unbounded();
        let (events, rx) = crate::capture::worker_event_channel();
        app.cmd_tx = Some(commands);
        app.event_rx = Some(rx);
        app.dashboard.session_active = true;
        app.dashboard.current_state = CaptureState::Paused;
        app.stop_and_keep_screenshots();
        assert!(app.dashboard.stopping);
        assert!(matches!(
            received.try_recv(),
            Ok(WorkerCommand::StopAndKeepScreenshots)
        ));
        events
            .send(WorkerEvent::ScreenshotsPreserved("saved/session".into()))
            .unwrap();
        events
            .send(WorkerEvent::StateChanged(CaptureState::Stopped))
            .unwrap();
        app.process_events();
        assert!(!app.dashboard.session_active);
        assert!(!app.request_safe_close());
        assert!(app
            .dashboard
            .last_error
            .as_deref()
            .unwrap()
            .contains("saved/session"));
    }

    #[test]
    fn unfinished_screenshot_session_recovers_in_a_background_worker() {
        let root = tempfile::tempdir().unwrap();
        let mut session =
            crate::storage::SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let frame = crate::model::Frame::new(vec![255; 4], 1, 1, 4, 1, 0);
        let record = crate::model::SlideRecord::new(crate::model::SlideRecordInput {
            slide_number: 1,
            png_filename: "slide_0001.png".into(),
            png_relative_path: "slides/slide_0001.png".into(),
            frame_index: 1,
            width: 1,
            height: 1,
            content_hash: "image".into(),
            source_name: String::new(),
            monitor_name: String::new(),
        });
        session.save(&frame, record).unwrap();
        assert!(!session.output_path().exists());
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        app.output_dir = root.path().to_string_lossy().into_owned();
        app.recovery_slides = 1;
        app.start_recovery();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.recovery_rx.is_some() && std::time::Instant::now() < deadline {
            app.poll_recovery();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.recovery_rx.is_none());
        assert!(app.recovery_accepted);
        assert!(session.output_path().is_file());
    }

    fn monitor(id: u64, primary: bool) -> MonitorInfo {
        MonitorInfo {
            hmonitor: id,
            adapter_name: "macOS".into(),
            output_name: format!("Display {id}"),
            description: format!("Display {id}"),
            region: Region::new(0, 0, 1920, 1080),
            is_primary: primary,
            is_virtual_suspect: false,
            output_index: 0,
            adapter_index: 0,
        }
    }

    #[test]
    fn default_monitor_prefers_the_primary_display() {
        let monitors = vec![monitor(1, false), monitor(2, true)];
        assert_eq!(default_monitor(&monitors).unwrap().hmonitor, 2);
    }

    #[test]
    fn macos_source_defaults_to_full_screen() {
        let mut panel = SourcePanel::new();
        panel.selected_hwnd = 99;
        panel.selected_title = "Old Window".into();
        apply_macos_source_default(&mut panel);
        assert_eq!(panel.selected_hwnd, 0);
        assert!(panel.full_screen_selected);
    }

    #[test]
    fn output_panel_is_the_single_source_for_capture_dashboard_and_persistence() {
        let mut panel = OutputPanel::new_with_filename("configured.pptx");
        panel.output_dir = "/tmp/captures/".into();
        panel.page_ratio = "4:3".into();
        panel.image_fit = "fill".into();
        panel.keep_previous = false;
        panel.auto_crop = true;

        let mut source = CaptureSource::new(1, "Slides".into(), 2, "Display".into());
        apply_output_to_source(&panel, &mut source);
        assert_eq!(source.output_dir, "/tmp/captures/");
        assert_eq!(source.output_filename, "configured.pptx");
        assert_eq!(source.page_ratio, "4:3");
        assert_eq!(source.image_fit, "fill");
        assert!(source.auto_crop);
        assert_eq!(output_path(&panel), "/tmp/captures/configured.pptx");

        let mut config = AppConfig::default();
        apply_output_to_config(&panel, &mut config);
        assert_eq!(config.output_dir, "/tmp/captures/");
        assert_eq!(config.output_filename, "configured.pptx");
        assert_eq!(config.page_ratio, "4:3");
        assert_eq!(config.image_fit, "fill");
        assert!(!config.keep_previous);
        assert!(config.auto_crop);
    }

    #[test]
    fn worker_errors_keep_history_and_latest_dashboard_error() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));

        app.record_error("worker", "first error".into());
        app.record_error("worker", "second error".into());

        assert_eq!(app.dashboard.last_error.as_deref(), Some("second error"));
        let messages: Vec<_> = app
            .diagnostic_history
            .entries_newest_first()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages, ["second error", "first error"]);
    }

    #[test]
    fn test_screenshot_initializes_worker_without_starting_recording() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        app.display_panel.selected_hmonitor = u64::MAX;
        app.send_test_capture();
        assert!(app.worker.is_some());
        assert!(app.test_capture_pending);
        assert!(!app.dashboard.session_active);
        app.worker.take().unwrap().stop();
    }

    #[test]
    fn idle_worker_disconnect_clears_pending_preview_and_allows_restart() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (tx, rx) = crate::capture::worker_event_channel();
        let (cmd, commands) = crossbeam_channel::unbounded();
        app.event_rx = Some(rx);
        app.cmd_tx = Some(cmd);
        app.test_capture_pending = true;
        drop(tx);
        drop(commands);
        app.process_events();
        assert!(!app.test_capture_pending);
        assert!(app.cmd_tx.is_none());
        app.ensure_worker();
        assert!(app.worker.is_some());
        app.worker.take().unwrap().stop();
    }

    #[test]
    fn start_failure_and_bad_preview_do_not_leave_ui_active_or_crash() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (tx, rx) = crate::capture::worker_event_channel();
        app.event_rx = Some(rx);
        app.dashboard.session_active = true;
        tx.send(WorkerEvent::StartFailed("missing display".into()))
            .unwrap();
        app.process_events();
        assert!(!app.dashboard.session_active);
        assert_eq!(app.dashboard.current_state, CaptureState::Error);
        tx.send(WorkerEvent::TestFrame(vec![0; 4], 320, 240))
            .unwrap();
        app.process_events();
        assert!(app.dashboard.test_frame_rgba.is_none());
        assert!(app
            .dashboard
            .last_error
            .as_deref()
            .unwrap()
            .contains("preview"));
        tx.send(WorkerEvent::TestFrame(vec![255; 320 * 180 * 4], 320, 180))
            .unwrap();
        app.process_events();
        assert_eq!(app.dashboard.test_frame_h, 180);
        assert!(app.dashboard.last_error.is_none());
    }

    #[test]
    fn stop_request_waits_for_worker_finalization() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (tx, rx) = crossbeam_channel::unbounded();
        app.cmd_tx = Some(tx);
        app.dashboard.session_active = true;
        app.stop_capture();
        assert!(matches!(rx.try_recv().unwrap(), WorkerCommand::Stop));
        assert!(app.cmd_tx.is_some());
        assert!(app.dashboard.session_active);
    }

    #[test]
    fn intentional_stop_drains_final_errors_without_false_disconnect() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (tx, rx) = crate::capture::worker_event_channel();
        app.event_rx = Some(rx);
        tx.send(WorkerEvent::SessionStarted(
            "sessions/unique/custom.pptx".into(),
        ))
        .unwrap();
        tx.send(WorkerEvent::SlideSaved {
            slide_number: 3,
            png_filename: "slide_0003.png".into(),
        })
        .unwrap();
        tx.send(WorkerEvent::Error("final save failed".into()))
            .unwrap();
        drop(tx);
        app.drain_events(false);
        assert_eq!(app.dashboard.output_path, "sessions/unique/custom.pptx");
        assert_eq!(app.dashboard.saved_slides_count, 3);
        assert_eq!(
            app.dashboard.last_error.as_deref(),
            Some("final save failed")
        );
    }

    #[test]
    fn disconnected_worker_is_recorded_once_and_clears_capture_channels() {
        let mut app = std::mem::ManuallyDrop::new(PptAutoCaptureApp::new(LoggingSetup {
            log_directory: None,
            log_file: None,
            warning: None,
        }));
        let (event_tx, event_rx) = crate::capture::worker_event_channel();
        drop(event_tx);
        let (command_tx, _command_rx) = crossbeam_channel::unbounded();
        app.event_rx = Some(event_rx);
        app.cmd_tx = Some(command_tx);
        app.dashboard.session_active = true;
        app.dashboard.current_state = CaptureState::Running;

        app.process_events();
        app.process_events();

        assert!(app.event_rx.is_none());
        assert!(app.cmd_tx.is_none());
        assert_eq!(app.dashboard.current_state, CaptureState::Error);
        assert!(!app.dashboard.session_active);
        assert_eq!(
            app.dashboard.last_error.as_deref(),
            Some("Capture worker disconnected")
        );
        let messages: Vec<_> = app
            .diagnostic_history
            .entries_newest_first()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages, ["Capture worker disconnected"]);
    }
}
