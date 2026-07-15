use eframe::egui;
use log::{error, info};

use crossbeam_channel::{Receiver, Sender};

use crate::capture::{CaptureState, CaptureWorker, WorkerCommand, WorkerEvent, CaptureSource};
use crate::i18n::{self, Language};
use crate::gui::dashboard::DashboardPanel;
use crate::gui::source_panel::SourcePanel;
use crate::gui::display_panel::DisplayPanel;
use crate::gui::settings_panel::SettingsPanel;
use crate::gui::output_panel::OutputPanel;

use crate::config::AppConfig;
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
    worker: Option<CaptureWorker>,
    cmd_tx: Option<Sender<WorkerCommand>>,
    event_rx: Option<Receiver<WorkerEvent>>,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    active_tab: Tab,
    pending_start: bool,
    pending_pause: bool,
    pending_stop: bool,
    pending_resume: bool,
    output_dir: String,
    output_filename: String,
    recovery_available: bool,
    recovery_slides: u32,
    recovery_accepted: bool,
    recovery_declined: bool,
    language: Language,
}

#[derive(PartialEq)]
enum Tab { Dashboard, Source, Display, Settings, Output }

impl PptAutoCaptureApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        let mut app = Self {
            config: config.clone(),
            dashboard: DashboardPanel::new(),
            source_panel: SourcePanel::new(),
            display_panel: DisplayPanel::new(),
            settings_panel: SettingsPanel::new(),
            output_panel: OutputPanel::new_with_filename(&config.output_filename),
            worker: None, cmd_tx: None, event_rx: None,
            monitors: Vec::new(), windows: Vec::new(),
            active_tab: Tab::Dashboard,
            pending_start: false, pending_pause: false, pending_stop: false, pending_resume: false,
            output_dir: config.output_dir.clone(),
            output_filename: config.output_filename.clone(),
            recovery_available: false, recovery_slides: 0,
            recovery_accepted: false, recovery_declined: false,
            language: config.language,
        };
        app.display_panel.selected_hmonitor = config.last_monitor_hmonitor;
        app.display_panel.selected_description = config.last_monitor_description.clone();
        app.source_panel.selected_hwnd = config.last_window_hwnd;
        app.source_panel.selected_title = config.last_window_title.clone();
        app.check_recovery();
        app
    }

    fn check_recovery(&mut self) {
        if let Ok(Some(records)) = detect_incomplete_session(std::path::Path::new(&self.output_dir)) {
            self.recovery_available = true;
            self.recovery_slides = records.len() as u32;
            info!("Recovery: {} slides", self.recovery_slides);
        }
    }

    fn start_capture(&mut self) {
        info!("Starting capture");
        let worker = CaptureWorker::new();
        self.event_rx = Some(worker.event_rx());
        self.cmd_tx = Some(worker.command_tx());
        let source = CaptureSource::new(
            self.source_panel.selected_hwnd, self.source_panel.selected_title.clone(),
            self.display_panel.selected_hmonitor, self.display_panel.selected_description.clone(),
        );
        let _ = self.cmd_tx.as_ref().map(|tx| tx.send(WorkerCommand::Start(source)));
        self.worker = Some(worker);
        self.dashboard.session_active = true;
        self.dashboard.output_path = format!("{}/{}",
                        self.output_panel.output_dir.trim_end_matches('/').trim_end_matches('\\'),
                        self.output_panel.output_filename);
        self.dashboard.source_window_title = self.source_panel.selected_title.clone();
        self.dashboard.monitor_description = self.display_panel.selected_description.clone();
    }

    fn pause_capture(&mut self) { if let Some(ref tx) = self.cmd_tx { let _ = tx.send(WorkerCommand::Pause); } }
    fn resume_capture(&mut self) { if let Some(ref tx) = self.cmd_tx { let _ = tx.send(WorkerCommand::Resume); } }
    fn stop_capture(&mut self) {
        if let Some(ref tx) = self.cmd_tx { let _ = tx.send(WorkerCommand::Stop); }
        if let Some(mut worker) = self.worker.take() { worker.stop(); }
        self.cmd_tx = None;
        self.event_rx = None;
        self.dashboard.current_state = CaptureState::Stopped;
        self.dashboard.state_message = CaptureState::Stopped.label().to_string();
        self.dashboard.session_active = false;
    }

    fn process_events(&mut self) {
        let rx = match self.event_rx.as_ref() { Some(r) => r, None => return };
        loop {
            let event = match rx.try_recv() {
                Ok(e) => e,
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    error!("Worker disconnected");
                    if let Some(mut worker) = self.worker.take() { worker.stop(); }
                    break;
                }
            };
            match event {
                WorkerEvent::StateChanged(s) => { self.dashboard.current_state = s; self.dashboard.state_message = s.label().to_string(); }
                WorkerEvent::SlideSaved { slide_number, .. } => self.dashboard.saved_slides_count = slide_number,
                WorkerEvent::Error(msg) => { self.dashboard.last_error = Some(msg); }
                WorkerEvent::TestFrame(d, w, h) => { self.dashboard.test_frame_rgba = Some(d); self.dashboard.test_frame_w = w; self.dashboard.test_frame_h = h; }
                WorkerEvent::BlackFrameDetected => { self.dashboard.state_message = "Black frame".into(); }
                _ => {}
            }
        }
    }

    fn refresh_windows(&mut self) {
        self.source_panel.refresh_requested = false;
        if let Ok(w) = enumerate_windows() { 
            self.windows = w.clone(); 
            self.source_panel.windows = w.clone();
            
            // Auto-select the best window: prefer slideshow, then PPT window
            if self.source_panel.selected_hwnd == 0 {
                // Sort by priority: slideshow first, then PPT
                let mut candidates: Vec<&WindowInfo> = w.iter().collect();
                candidates.sort_by(|a, b| {
                    let a_score = if a.is_powerpoint { 2 } else if a.title.to_lowercase().contains("powerpoint") || a.title.to_lowercase().contains("ppt") { 1 } else { 0 };
                    let b_score = if b.is_powerpoint { 2 } else if b.title.to_lowercase().contains("powerpoint") || b.title.to_lowercase().contains("ppt") { 1 } else { 0 };
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
        if let Ok(m) = enumerate_monitors() { self.monitors = m.clone(); self.display_panel.monitors = m; }
    }
}

impl eframe::App for PptAutoCaptureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();

        if self.recovery_available && !self.recovery_accepted && !self.recovery_declined {
            let mut open = true;
            egui::Window::new("Session Recovery").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).show(ctx, |ui| {
                ui.heading(i18n::t_session_recovery_title(self.language));
                ui.label(i18n::t_recovery_msg(self.language, self.recovery_slides));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t_recover(self.language)).clicked() {
                        if let Ok(()) = recover_session(std::path::Path::new(&self.output_dir)) {
                            self.dashboard.saved_slides_count = self.recovery_slides;
                            self.recovery_accepted = true;
                        }
                    }
                    if ui.button(i18n::t_skip(self.language)).clicked() { self.recovery_declined = true; }
                });
            });
            if !open { self.recovery_declined = true; }
            ctx.request_repaint();
            return;
        }

        if self.pending_start {
            self.pending_start = false;
            if self.display_panel.selected_hmonitor == 0 { self.dashboard.last_error = Some(i18n::t_no_display_selected(self.language).to_string()); }
            else { self.dashboard.last_error = None; self.start_capture(); }
        }
        if self.pending_pause { self.pending_pause = false; self.pause_capture(); }
        if self.pending_resume { self.pending_resume = false; self.resume_capture(); }
        if self.pending_stop { self.pending_stop = false; self.stop_capture(); }

        if self.source_panel.refresh_requested { self.refresh_windows(); }
        if self.source_panel.move_requested && self.display_panel.selected_hmonitor != 0 {
            self.source_panel.move_requested = false;
            if let Some(mon) = self.monitors.iter().find(|m| m.hmonitor == self.display_panel.selected_hmonitor) {
                let _ = crate::windows::move_window_to_monitor(self.source_panel.selected_hwnd, &mon.region);
            }
        }
        if self.source_panel.maximize_requested { self.source_panel.maximize_requested = false; let _ = crate::windows::maximize_window(self.source_panel.selected_hwnd); }
        if self.display_panel.refresh_requested { self.refresh_displays(); }

        egui::TopBottomPanel::top("bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Dashboard, i18n::t_tab_dashboard(self.language));
                ui.selectable_value(&mut self.active_tab, Tab::Source, i18n::t_tab_source(self.language));
                ui.selectable_value(&mut self.active_tab, Tab::Display, i18n::t_tab_display(self.language));
                ui.selectable_value(&mut self.active_tab, Tab::Settings, i18n::t_tab_settings(self.language));
                ui.selectable_value(&mut self.active_tab, Tab::Output, i18n::t_tab_output(self.language));
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
                    self.dashboard.output_path = format!("{}/{}",
                        self.output_panel.output_dir.trim_end_matches('/').trim_end_matches('\\'),
                        self.output_panel.output_filename);
                    self.dashboard.render(ui, self.language, &mut self.pending_start, &mut self.pending_pause, &mut self.pending_stop, &mut self.pending_resume);
                }
                Tab::Source => { self.source_panel.render(ui, self.language, self.display_panel.selected_hmonitor != 0); if self.windows.is_empty() { self.refresh_windows(); } }
                Tab::Display => { self.display_panel.render(ui, self.language); if self.monitors.is_empty() { self.refresh_displays(); } }
                Tab::Settings => self.settings_panel.render(ui, self.language),
                Tab::Output => {
                    self.output_panel.render(ui, self.language);
                    // Sync output filename to dashboard
                    self.dashboard.output_path = format!("{}/{}",
                        self.output_panel.output_dir.trim_end_matches('/').trim_end_matches('\\'),
                        self.output_panel.output_filename);
                }
            }
        });

        ctx.request_repaint();
    }
}

impl Drop for PptAutoCaptureApp {
    fn drop(&mut self) {
        self.config.output_dir = self.output_dir.clone();
        self.config.output_filename = self.output_panel.output_filename.clone();
        self.config.last_window_hwnd = self.source_panel.selected_hwnd;
        self.config.last_window_title = self.source_panel.selected_title.clone();
        self.config.last_monitor_hmonitor = self.display_panel.selected_hmonitor;
        self.config.last_monitor_description = self.display_panel.selected_description.clone();
        self.config.language = self.language;
        let _ = self.config.save();
        if let Some(mut worker) = self.worker.take() { worker.stop(); }
    }
}
