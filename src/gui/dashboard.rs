use crate::capture::CaptureState;
use crate::diagnostics::{DiagnosticHistory, DiagnosticLevel};
use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use egui::*;
use std::time::{Duration, Instant};

const DIAGNOSTICS_STATUS_DURATION: Duration = Duration::from_secs(3);

pub struct DashboardPanel {
    pub current_state: CaptureState,
    pub saved_slides_count: u32,
    pub source_window_title: String,
    pub monitor_description: String,
    pub output_path: String,
    pub test_frame_rgba: Option<Vec<u8>>,
    pub test_frame_w: u32,
    pub test_frame_h: u32,
    pub last_error: Option<String>,
    pub state_message: String,
    pub session_active: bool,
    pub stopping: bool,
    pub copy_diagnostics_requested: bool,
    pub open_log_directory_requested: bool,
    pub diagnostics_status: Option<String>,
    pub open_session_requested: bool,
    pub delete_session_requested: bool,
    pub deleting_session: bool,
    pub cleanup_status: Option<String>,
    pub keep_screenshots_requested: bool,
    /// Persisted through the app configuration; applied when a recording starts.
    pub minimize_on_start: bool,
    diagnostics_status_expires_at: Option<Instant>,
    preview_texture: Option<TextureHandle>,
    preview_dirty: bool,
}

pub struct DashboardRenderInput<'a> {
    pub diagnostic_history: &'a DiagnosticHistory,
    pub persistent_log_available: bool,
    pub session_folder_available: bool,
    pub start_requested: &'a mut bool,
    pub pause_requested: &'a mut bool,
    pub stop_requested: &'a mut bool,
    pub resume_requested: &'a mut bool,
}

impl DashboardPanel {
    pub fn new() -> Self {
        Self {
            current_state: CaptureState::Idle,
            saved_slides_count: 0,
            source_window_title: "None".into(),
            monitor_description: "None".into(),
            output_path: "output/output.pptx".into(),
            test_frame_rgba: None,
            test_frame_w: 0,
            test_frame_h: 0,
            last_error: None,
            state_message: CaptureState::Idle.label().to_string(),
            session_active: false,
            stopping: false,
            copy_diagnostics_requested: false,
            open_log_directory_requested: false,
            diagnostics_status: None,
            open_session_requested: false,
            delete_session_requested: false,
            deleting_session: false,
            cleanup_status: None,
            keep_screenshots_requested: false,
            minimize_on_start: false,
            diagnostics_status_expires_at: None,
            preview_texture: None,
            preview_dirty: false,
        }
    }

    pub fn show_diagnostics_status(&mut self, status: String) {
        self.set_diagnostics_status_until(status, Instant::now() + DIAGNOSTICS_STATUS_DURATION);
    }

    pub fn update_preview(&mut self, pixels: Vec<u8>, width: u32, height: u32) -> bool {
        if width == 0
            || height == 0
            || width > 320
            || height > 240
            || pixels.len() != width as usize * height as usize * 4
        {
            return false;
        }
        self.test_frame_rgba = Some(pixels);
        self.test_frame_w = width;
        self.test_frame_h = height;
        self.preview_dirty = true;
        true
    }

    fn set_diagnostics_status_until(&mut self, status: String, expires_at: Instant) {
        self.diagnostics_status = Some(status);
        self.diagnostics_status_expires_at = Some(expires_at);
    }

    fn clear_expired_diagnostics_status(&mut self, now: Instant) {
        if self
            .diagnostics_status_expires_at
            .is_some_and(|expires_at| now >= expires_at)
        {
            self.diagnostics_status = None;
            self.diagnostics_status_expires_at = None;
        }
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language, input: DashboardRenderInput<'_>) {
        self.clear_expired_diagnostics_status(Instant::now());

        let DashboardRenderInput {
            diagnostic_history,
            persistent_log_available,
            session_folder_available,
            start_requested,
            pause_requested,
            stop_requested,
            resume_requested,
        } = input;

        if let Some(error) = self.last_error.clone() {
            self.render_error(ui, language, error);
            ui.add_space(14.0);
        }

        self.render_hero(
            ui,
            language,
            start_requested,
            pause_requested,
            stop_requested,
            resume_requested,
        );

        ui.add_space(20.0);
        self.render_setup(ui, language, session_folder_available);

        if self.test_frame_rgba.is_some() && self.test_frame_w > 0 && self.test_frame_h > 0 {
            ui.add_space(20.0);
            self.render_preview(ui, language);
        }

        ui.add_space(20.0);
        self.render_activity(ui, language, diagnostic_history, persistent_log_available);
    }

    fn render_error(&mut self, ui: &mut Ui, language: Language, error: String) {
        theme::banner_frame(token::DANGER_SOFT, token::DANGER_LINE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let text_width = (ui.available_width() - 76.0).max(120.0);
                ui.vertical(|ui| {
                    ui.set_width(text_width);
                    ui.label(
                        RichText::new(match language {
                            Language::English => "Recording needs attention",
                            Language::Chinese => "录制需要处理",
                        })
                        .size(12.5)
                        .color(token::DANGER)
                        .strong(),
                    );
                    ui.add_space(2.0);
                    ui.label(RichText::new(error).size(12.0).color(token::DANGER));
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if ui
                        .add(theme::quiet_button(match language {
                            Language::English => "Dismiss",
                            Language::Chinese => "收起",
                        }))
                        .clicked()
                    {
                        self.last_error = None;
                    }
                });
            });
        });
    }

    fn render_hero(
        &mut self,
        ui: &mut Ui,
        language: Language,
        start_requested: &mut bool,
        pause_requested: &mut bool,
        stop_requested: &mut bool,
        resume_requested: &mut bool,
    ) {
        let state = self.current_state;
        let stopping = self.stopping;
        let accent = state_accent(state, stopping);
        let (fill, border) = if stopping {
            (token::WARN_SOFT, token::WARN_LINE)
        } else {
            match state {
                CaptureState::Running
                | CaptureState::WaitingForStable
                | CaptureState::Stable
                | CaptureState::Saving => (token::RECORD_SOFT, token::RECORD_LINE),
                CaptureState::Paused | CaptureState::WaitingForDesktop => {
                    (token::WARN_SOFT, token::WARN_LINE)
                }
                CaptureState::Error | CaptureState::ProtectedOrBlack => {
                    (token::DANGER_SOFT, token::DANGER_LINE)
                }
                _ => (token::SURFACE, token::LINE),
            }
        };

        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::symmetric(22, 20))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if ui.available_width() >= 600.0 {
                    let stage_width = 204.0;
                    let left_width = (ui.available_width() - stage_width - 24.0).max(280.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(left_width);
                            self.render_hero_text(
                                ui,
                                language,
                                state,
                                stopping,
                                accent,
                                start_requested,
                                pause_requested,
                                stop_requested,
                                resume_requested,
                            );
                        });
                        ui.add_space(4.0);
                        draw_stage(
                            ui,
                            self.session_active && !stopping,
                            self.saved_slides_count,
                            language,
                        );
                    });
                } else {
                    ui.vertical(|ui| {
                        self.render_hero_text(
                            ui,
                            language,
                            state,
                            stopping,
                            accent,
                            start_requested,
                            pause_requested,
                            stop_requested,
                            resume_requested,
                        );
                        ui.add_space(14.0);
                        draw_stage(
                            ui,
                            self.session_active && !stopping,
                            self.saved_slides_count,
                            language,
                        );
                    });
                }
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_hero_text(
        &mut self,
        ui: &mut Ui,
        language: Language,
        state: CaptureState,
        stopping: bool,
        accent: Color32,
        start_requested: &mut bool,
        pause_requested: &mut bool,
        stop_requested: &mut bool,
        resume_requested: &mut bool,
    ) {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, accent);
            ui.label(
                RichText::new(state_title(language, state, stopping))
                    .size(26.0)
                    .color(token::INK)
                    .strong(),
            );
        });
        ui.add_space(3.0);
        ui.label(
            RichText::new(state_detail(language, state, stopping))
                .size(13.0)
                .color(token::MUTED),
        );

        if self.session_active && !stopping && self.saved_slides_count > 0 {
            ui.add_space(8.0);
            ui.label(
                RichText::new(match language {
                    Language::English => {
                        format!("{} slides saved to this recording", self.saved_slides_count)
                    }
                    Language::Chinese => format!("本次录制已保存 {} 页", self.saved_slides_count),
                })
                .size(11.5)
                .color(token::MUTED),
            );
        }

        ui.add_space(16.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            if stopping {
                ui.add_enabled(
                    false,
                    theme::secondary_button(i18n::t_saving_recording(language)),
                );
            } else if self.session_active {
                if state.is_paused() {
                    if ui
                        .add(theme::primary_button(i18n::t_resume_recording(language)))
                        .clicked()
                    {
                        *resume_requested = true;
                    }
                } else if state != CaptureState::Error
                    && ui
                        .add(theme::secondary_button(i18n::t_pause_recording(language)))
                        .clicked()
                {
                    *pause_requested = true;
                }
                if ui
                    .add(theme::danger_button(i18n::t_stop_recording(language)))
                    .clicked()
                {
                    *stop_requested = true;
                }
            } else if ui
                .add(theme::primary_button(i18n::t_start_recording(language)))
                .clicked()
            {
                *start_requested = true;
            }
        });

        if self.session_active
            && !stopping
            && matches!(state, CaptureState::Paused | CaptureState::Error)
        {
            ui.add_space(10.0);
            if ui
                .add(theme::secondary_button(match language {
                    Language::Chinese => "停止并保留截图（稍后生成 PPTX）",
                    Language::English => "Stop and keep screenshots (generate PPTX later)",
                }))
                .clicked()
            {
                self.keep_screenshots_requested = true;
            }
            ui.add_space(3.0);
            ui.label(theme::muted(
                match language {
                    Language::Chinese => {
                        "截图和恢复记录会保留；待保存截图无法写入时不会结束录制。"
                    }
                    Language::English => {
                        "Screenshots remain recoverable; the recording stays open if a pending screenshot cannot be saved."
                    }
                },
                11.5,
            ));
        }

        theme::divider(ui);
        let enabled = !self.session_active && !self.stopping;
        ui.add_enabled_ui(enabled, |ui| {
            ui.checkbox(
                &mut self.minimize_on_start,
                RichText::new(i18n::t_minimize_on_start(language))
                    .size(13.0)
                    .color(token::INK),
            )
            .on_hover_text(i18n::t_minimize_on_start_hint(language));
        });
        ui.add_space(2.0);
        ui.label(theme::muted(i18n::t_minimize_on_start_hint(language), 11.5));
    }

    fn render_setup(&mut self, ui: &mut Ui, language: Language, session_folder_available: bool) {
        theme::section_title(
            ui,
            match language {
                Language::English => "Recording setup",
                Language::Chinese => "录制设置",
            },
        );
        ui.add_space(4.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let source = if self.source_window_title.trim().is_empty()
                || self.source_window_title == "None"
            {
                i18n::t_none(language)
            } else if self.source_window_title.contains("Full Screen")
                || self.source_window_title.contains("全屏")
            {
                match language {
                    Language::English => "📺 Full Screen",
                    Language::Chinese => "📺 全屏截图",
                }
            } else {
                self.source_window_title.as_str()
            };
            let display = if self.monitor_description.trim().is_empty()
                || self.monitor_description == "None"
            {
                i18n::t_none(language)
            } else {
                self.monitor_description.as_str()
            };
            let output = if self.output_path.trim().is_empty() {
                i18n::t_none(language)
            } else {
                self.output_path.as_str()
            };

            let wide = ui.available_width() >= 540.0;
            if wide {
                ui.columns(3, |columns| {
                    setup_column(
                        &mut columns[0],
                        i18n::t_capture_source(language).trim_end_matches([':', '：']),
                        source,
                    );
                    setup_column(
                        &mut columns[1],
                        i18n::t_display(language).trim_end_matches([':', '：']),
                        display,
                    );
                    setup_column(
                        &mut columns[2],
                        match language {
                            Language::English => "Output",
                            Language::Chinese => "输出",
                        },
                        output,
                    );
                });
            } else {
                setup_column(
                    ui,
                    i18n::t_capture_source(language).trim_end_matches([':', '：']),
                    source,
                );
                theme::divider(ui);
                setup_column(
                    ui,
                    i18n::t_display(language).trim_end_matches([':', '：']),
                    display,
                );
                theme::divider(ui);
                setup_column(
                    ui,
                    match language {
                        Language::English => "Output",
                        Language::Chinese => "输出",
                    },
                    output,
                );
            }

            if self.saved_slides_count > 0 || session_folder_available {
                theme::divider(ui);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    if self.saved_slides_count > 0
                        && ui
                            .add(theme::secondary_button(i18n::t_open_recording_folder(
                                language,
                            )))
                            .clicked()
                    {
                        self.open_session_requested = true;
                    }
                    if session_folder_available {
                        let can_delete =
                            !self.session_active && !self.stopping && !self.deleting_session;
                        let label = if self.deleting_session {
                            i18n::t_deleting_recording_folder(language)
                        } else {
                            i18n::t_delete_recording_folder(language)
                        };
                        if ui
                            .add_enabled(can_delete, theme::danger_button(label))
                            .clicked()
                        {
                            self.delete_session_requested = true;
                        }
                    }
                });
            }
            if let Some(status) = &self.cleanup_status {
                ui.add_space(6.0);
                ui.label(theme::muted(status.clone(), 11.5));
            }
        });
    }

    fn render_preview(&mut self, ui: &mut Ui, language: Language) {
        let title = match language {
            Language::English => "Test capture preview",
            Language::Chinese => "测试截图预览",
        };
        theme::section_title(ui, title);
        ui.add_space(4.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(ref rgba_data) = self.test_frame_rgba else {
                return;
            };
            let (cw, ch) = (self.test_frame_w as usize, self.test_frame_h as usize);
            if cw == 0
                || ch == 0
                || cw.checked_mul(ch).and_then(|n| n.checked_mul(4)) != Some(rgba_data.len())
            {
                return;
            }
            if self.preview_dirty || self.preview_texture.is_none() {
                let image = ColorImage::from_rgba_unmultiplied([cw, ch], rgba_data);
                if let Some(texture) = self.preview_texture.as_mut() {
                    texture.set(image, TextureOptions::LINEAR);
                } else {
                    self.preview_texture = Some(ui.ctx().load_texture(
                        "test_preview",
                        image,
                        TextureOptions::LINEAR,
                    ));
                }
                self.preview_dirty = false;
            }
            let Some(texture) = self.preview_texture.as_ref() else {
                return;
            };
            let max_width = ui.available_width().min(420.0);
            let max_height = 260.0;
            let scale = (max_width / cw as f32).min(max_height / ch as f32);
            ui.add(Image::new(egui::load::SizedTexture::new(
                texture.id(),
                Vec2::new(cw as f32 * scale, ch as f32 * scale),
            )));
        });
    }

    fn render_activity(
        &mut self,
        ui: &mut Ui,
        language: Language,
        history: &DiagnosticHistory,
        persistent_log_available: bool,
    ) {
        theme::section_title(
            ui,
            match language {
                Language::English => "Activity",
                Language::Chinese => "活动记录",
            },
        );
        ui.add_space(4.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(i18n::t_diagnostics(language))
                        .size(13.0)
                        .color(token::INK)
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(theme::quiet_button(i18n::t_open_log_folder(language)))
                        .clicked()
                    {
                        self.open_log_directory_requested = true;
                    }
                    if ui
                        .add(theme::quiet_button(i18n::t_copy_diagnostics(language)))
                        .clicked()
                    {
                        self.copy_diagnostics_requested = true;
                    }
                });
            });
            theme::divider(ui);

            if let Some(status) = &self.diagnostics_status {
                ui.label(RichText::new(status).size(11.5).color(token::OK));
                ui.add_space(4.0);
            }
            if !persistent_log_available {
                ui.label(
                    RichText::new(i18n::t_persistent_logging_unavailable(language))
                        .size(11.5)
                        .color(token::WARN),
                );
                ui.add_space(4.0);
            }

            let entries: Vec<_> = history.entries_newest_first().take(5).collect();
            if entries.is_empty() {
                theme::empty_note(
                    ui,
                    match language {
                        Language::English => {
                            "No activity yet. Start a recording to see events here."
                        }
                        Language::Chinese => "暂无活动。开始录制后会在这里显示记录。",
                    },
                );
            } else {
                let wide = ui.available_width() >= 540.0;
                for (index, entry) in entries.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(7.0);
                    }
                    let color = match entry.level {
                        DiagnosticLevel::Info => token::MUTED,
                        DiagnosticLevel::Warning => token::WARN,
                        DiagnosticLevel::Error => token::DANGER,
                    };
                    let message = entry.message.clone();
                    let hover = format!("{} ({})", entry.level, entry.category);
                    if wide {
                        ui.horizontal_top(|ui| {
                            ui.add_sized(
                                [104.0, 16.0],
                                Label::new(
                                    RichText::new(
                                        entry.timestamp.format("%m-%d %H:%M:%S").to_string(),
                                    )
                                    .monospace()
                                    .size(10.5)
                                    .color(token::FAINT),
                                )
                                .truncate(),
                            );
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(5.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 2.5, color);
                            ui.label(RichText::new(message).size(12.0).color(token::INK))
                                .on_hover_text(hover);
                        });
                    } else {
                        ui.label(
                            RichText::new(entry.timestamp.format("%m-%d %H:%M:%S").to_string())
                                .monospace()
                                .size(10.5)
                                .color(token::FAINT),
                        );
                        ui.horizontal_top(|ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(5.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 2.5, color);
                            ui.label(RichText::new(message).size(12.0).color(token::INK));
                        });
                    }
                }
            }
        });
    }
}

fn setup_column(ui: &mut Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(11.0).color(token::MUTED));
    ui.add_space(2.0);
    ui.add(
        Label::new(RichText::new(value).size(13.0).color(token::INK))
            .truncate()
            .halign(Align::LEFT),
    )
    .on_hover_text(value);
}

fn state_accent(state: CaptureState, stopping: bool) -> Color32 {
    if stopping {
        return token::WARN;
    }
    match state {
        CaptureState::Running
        | CaptureState::WaitingForStable
        | CaptureState::Stable
        | CaptureState::Saving => token::RECORD,
        CaptureState::Paused | CaptureState::WaitingForDesktop => token::WARN,
        CaptureState::Error | CaptureState::ProtectedOrBlack => token::DANGER,
        CaptureState::Stopped => token::OK,
        _ => token::PRIMARY,
    }
}

fn state_title(language: Language, state: CaptureState, stopping: bool) -> &'static str {
    if stopping {
        return match language {
            Language::English => "Finishing the recording",
            Language::Chinese => "正在完成录制",
        };
    }
    match (state, language) {
        (CaptureState::Idle, Language::English) => "Ready to record",
        (CaptureState::Idle, Language::Chinese) => "准备录制",
        (CaptureState::Running, Language::English) => "Recording",
        (CaptureState::Running, Language::Chinese) => "正在录制",
        (CaptureState::WaitingForStable, Language::English) => "Capturing the next slide",
        (CaptureState::WaitingForStable, Language::Chinese) => "正在捕获下一页",
        (CaptureState::Stable, Language::English) => "Slide detected",
        (CaptureState::Stable, Language::Chinese) => "检测到新页面",
        (CaptureState::Saving, Language::English) => "Saving slide",
        (CaptureState::Saving, Language::Chinese) => "正在保存截图",
        (CaptureState::Paused, Language::English) => "Paused",
        (CaptureState::Paused, Language::Chinese) => "已暂停",
        (CaptureState::WaitingForDesktop, Language::English) => "Waiting for the desktop",
        (CaptureState::WaitingForDesktop, Language::Chinese) => "等待桌面恢复",
        (CaptureState::Stopped, Language::English) => "Recording finished",
        (CaptureState::Stopped, Language::Chinese) => "录制已结束",
        (CaptureState::ProtectedOrBlack, Language::English) => "Cannot see the slide",
        (CaptureState::ProtectedOrBlack, Language::Chinese) => "无法捕获画面",
        (CaptureState::Error, Language::English) => "Recording needs attention",
        (CaptureState::Error, Language::Chinese) => "录制需要处理",
    }
}

fn state_detail(language: Language, state: CaptureState, stopping: bool) -> &'static str {
    if stopping {
        return match language {
            Language::English => "Saving the slides and building the PPTX file.",
            Language::Chinese => "正在保存截图并生成 PPTX 文件。",
        };
    }
    match (state, language) {
        (CaptureState::Idle, Language::English) => {
            "Choose a window and display; capture starts the moment you press Start."
        }
        (CaptureState::Idle, Language::Chinese) => "选择窗口和显示器，点击开始后立即截图。",
        (CaptureState::Running, Language::English) => {
            "Watching for slide changes. You can minimize this window and present."
        }
        (CaptureState::Running, Language::Chinese) => "正在等待换页，可以最小化窗口开始演示。",
        (CaptureState::WaitingForStable, Language::English) => {
            "A change was detected; waiting for the animation to settle."
        }
        (CaptureState::WaitingForStable, Language::Chinese) => "检测到变化，等待动画稳定。",
        (CaptureState::Stable, Language::English) => "The slide is stable and will be saved.",
        (CaptureState::Stable, Language::Chinese) => "画面已稳定，正在保存。",
        (CaptureState::Saving, Language::English) => "Writing the screenshot into this recording.",
        (CaptureState::Saving, Language::Chinese) => "正在写入本次录制的截图。",
        (CaptureState::Paused, Language::English) => {
            "Resume when ready. If saving failed, close the program using the output file first."
        }
        (CaptureState::Paused, Language::Chinese) => {
            "准备好后继续。若保存失败，请先关闭占用输出文件的程序。"
        }
        (CaptureState::WaitingForDesktop, Language::English) => {
            "The desktop is unavailable. Capture resumes automatically; Pause or Stop cancels it."
        }
        (CaptureState::WaitingForDesktop, Language::Chinese) => {
            "桌面暂不可用，恢复后会自动继续；暂停或停止可取消。"
        }
        (CaptureState::Stopped, Language::English) => {
            "The recording is closed. PPTX generation runs in the background."
        }
        (CaptureState::Stopped, Language::Chinese) => "录制已结束，正在后台生成 PPTX。",
        (CaptureState::ProtectedOrBlack, Language::English) => {
            "Protected or black content cannot be captured. Capture resumes when the slide returns."
        }
        (CaptureState::ProtectedOrBlack, Language::Chinese) => {
            "受保护或黑屏内容无法捕获，画面恢复后会自动继续。"
        }
        (CaptureState::Error, Language::English) => {
            "Check the window, display, and output folder, then start again."
        }
        (CaptureState::Error, Language::Chinese) => "请检查窗口、显示器和输出文件夹后重试。",
    }
}

fn draw_stage(ui: &mut Ui, recording: bool, count: u32, language: Language) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(204.0, 118.0), Sense::hover());
    let slide = Rect::from_center_size(rect.center(), Vec2::new(196.0, 110.0));
    let painter = ui.painter();
    painter.rect(
        slide,
        CornerRadius::same(7),
        token::SURFACE,
        Stroke::new(1.0, token::LINE),
        StrokeKind::Inside,
    );
    painter.text(
        slide.center() - Vec2::new(0.0, 10.0),
        Align2::CENTER_CENTER,
        count.to_string(),
        FontId::new(34.0, FontFamily::Monospace),
        token::INK,
    );
    painter.text(
        slide.center() + Vec2::new(0.0, 20.0),
        Align2::CENTER_CENTER,
        match language {
            Language::English => "slides saved",
            Language::Chinese => "已保存截图",
        },
        FontId::new(10.5, FontFamily::Proportional),
        token::MUTED,
    );
    if recording {
        painter.circle_filled(slide.left_top() + Vec2::new(12.0, 12.0), 3.2, token::RECORD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_texture_is_reused_and_only_updated_for_new_pixels() {
        fn draw(ctx: &egui::Context, panel: &mut DashboardPanel) -> egui::FullOutput {
            let history = DiagnosticHistory::new(20);
            let mut actions = [false; 4];
            let [start, pause, stop, resume] = &mut actions;
            ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    panel.render(
                        ui,
                        Language::English,
                        DashboardRenderInput {
                            diagnostic_history: &history,
                            persistent_log_available: true,
                            session_folder_available: true,
                            start_requested: start,
                            pause_requested: pause,
                            stop_requested: stop,
                            resume_requested: resume,
                        },
                    )
                });
            })
        }
        let ctx = egui::Context::default();
        let mut panel = DashboardPanel::new();
        assert!(panel.update_preview(vec![255; 16], 2, 2));
        draw(&ctx, &mut panel);
        let id = panel.preview_texture.as_ref().unwrap().id();
        let unchanged = draw(&ctx, &mut panel);
        assert!(!unchanged
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        assert!(panel.update_preview(vec![0; 16], 2, 2));
        let changed = draw(&ctx, &mut panel);
        assert_eq!(panel.preview_texture.as_ref().unwrap().id(), id);
        assert!(changed
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
    }
    use crate::diagnostics::{DiagnosticEntry, DiagnosticLevel};
    use std::time::Instant;

    #[test]
    fn dashboard_starts_with_diagnostic_requests_cleared() {
        let dashboard = DashboardPanel::new();

        assert!(!dashboard.copy_diagnostics_requested);
        assert!(!dashboard.open_log_directory_requested);
        assert!(!dashboard.minimize_on_start);
    }

    #[test]
    fn dashboard_clears_diagnostic_success_status_after_its_expiry() {
        let mut dashboard = DashboardPanel::new();
        let expiry = Instant::now();
        dashboard.last_error = Some("capture failed".into());

        dashboard.set_diagnostics_status_until("Diagnostics copied".into(), expiry);
        dashboard.clear_expired_diagnostics_status(expiry);

        assert!(dashboard.diagnostics_status.is_none());
        assert_eq!(dashboard.last_error.as_deref(), Some("capture failed"));
    }

    #[test]
    fn dashboard_render_input_keeps_history_and_action_requests_separate() {
        let mut history = DiagnosticHistory::new(20);
        history.push(DiagnosticEntry::new(
            DiagnosticLevel::Error,
            "capture",
            "Capture failed",
        ));
        let mut start_requested = false;
        let mut pause_requested = false;
        let mut stop_requested = false;
        let mut resume_requested = false;
        {
            let input = DashboardRenderInput {
                diagnostic_history: &history,
                persistent_log_available: true,
                session_folder_available: true,
                start_requested: &mut start_requested,
                pause_requested: &mut pause_requested,
                stop_requested: &mut stop_requested,
                resume_requested: &mut resume_requested,
            };

            assert_eq!(
                input
                    .diagnostic_history
                    .entries_newest_first()
                    .next()
                    .map(|entry| entry.message.as_str()),
                Some("Capture failed")
            );
            *input.start_requested = true;
        }
        assert!(start_requested);
        assert!(!pause_requested);
        assert!(!stop_requested);
        assert!(!resume_requested);
    }

    #[test]
    fn hero_status_copy_is_localized() {
        assert_eq!(
            state_title(Language::English, CaptureState::Idle, false),
            "Ready to record"
        );
        assert_eq!(
            state_title(Language::Chinese, CaptureState::Idle, false),
            "准备录制"
        );
        assert_eq!(
            state_title(Language::English, CaptureState::Running, true),
            "Finishing the recording"
        );
        assert!(!state_detail(Language::Chinese, CaptureState::Stopped, false).is_empty());
    }
}
