use crate::capture::CaptureState;
use crate::diagnostics::DiagnosticHistory;
use crate::i18n::{self, Language};
use egui::*;

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
    pub copy_diagnostics_requested: bool,
    pub open_log_directory_requested: bool,
    pub diagnostics_status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_starts_with_diagnostic_requests_cleared() {
        let dashboard = DashboardPanel::new();

        assert!(!dashboard.copy_diagnostics_requested);
        assert!(!dashboard.open_log_directory_requested);
    }
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
            copy_diagnostics_requested: false,
            open_log_directory_requested: false,
            diagnostics_status: None,
        }
    }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        language: Language,
        diagnostic_history: &DiagnosticHistory,
        persistent_log_available: bool,
        start_btn: &mut bool,
        pause_btn: &mut bool,
        stop_btn: &mut bool,
        resume_btn: &mut bool,
    ) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::t_status(language)).strong());
                let state_color = match self.current_state {
                    CaptureState::Idle => Color32::GRAY,
                    CaptureState::Running => Color32::GREEN,
                    CaptureState::WaitingForStable => Color32::YELLOW,
                    CaptureState::Stable => Color32::from_rgb(0, 200, 0),
                    CaptureState::Saving => Color32::LIGHT_BLUE,
                    CaptureState::Paused => Color32::GOLD,
                    CaptureState::Stopped => Color32::GRAY,
                    CaptureState::ProtectedOrBlack => Color32::RED,
                    CaptureState::Error => Color32::RED,
                };
                ui.colored_label(state_color, self.current_state.label());
            });

            ui.separator();

            let info_rows: [(&str, &str); 4] = [
                (i18n::t_capture_source(language), &self.source_window_title),
                (i18n::t_display(language), &self.monitor_description),
                (i18n::t_output(language), &self.output_path),
                (
                    i18n::t_slides_saved(language),
                    &format!("{}", self.saved_slides_count),
                ),
            ];

            egui::Grid::new("dashboard_info")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .striped(true)
                .show(ui, |grid| {
                    for (label, value) in &info_rows {
                        grid.label(RichText::new(*label).strong());
                        grid.label(*value);
                        grid.end_row();
                    }
                });

            ui.separator();

            if let Some(ref err) = self.last_error {
                ui.colored_label(Color32::RED, format!("⚠ {}", err));
                ui.separator();
            }

            ui.label(RichText::new(i18n::t_diagnostics(language)).strong());
            if let Some(status) = &self.diagnostics_status {
                ui.label(status);
            }
            if !persistent_log_available {
                ui.colored_label(
                    Color32::YELLOW,
                    i18n::t_persistent_logging_unavailable(language),
                );
            }
            for entry in diagnostic_history.entries_newest_first().take(5) {
                ui.label(format!(
                    "{} [{}] {}: {}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S %:z"),
                    entry.level,
                    entry.category,
                    entry.message,
                ));
            }
            ui.horizontal(|ui| {
                if ui.button(i18n::t_copy_diagnostics(language)).clicked() {
                    self.copy_diagnostics_requested = true;
                }
                if ui
                    .add_enabled(
                        persistent_log_available,
                        Button::new(i18n::t_open_log_folder(language)),
                    )
                    .clicked()
                {
                    self.open_log_directory_requested = true;
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.current_state == CaptureState::Idle
                            || self.current_state == CaptureState::Stopped
                            || self.current_state == CaptureState::Error,
                        Button::new(RichText::new(i18n::t_start(language)).size(16.0)),
                    )
                    .clicked()
                {
                    *start_btn = true;
                }

                if ui
                    .add_enabled(
                        self.current_state == CaptureState::Running
                            || self.current_state == CaptureState::WaitingForStable
                            || self.current_state == CaptureState::Stable,
                        Button::new(RichText::new(i18n::t_pause(language)).size(16.0)),
                    )
                    .clicked()
                {
                    *pause_btn = true;
                }

                if ui
                    .add_enabled(
                        self.current_state.is_paused(),
                        Button::new(RichText::new(i18n::t_resume(language)).size(16.0)),
                    )
                    .clicked()
                {
                    *resume_btn = true;
                }

                if ui
                    .add_enabled(
                        self.current_state.is_active()
                            || self.current_state.is_paused()
                            || self.current_state == CaptureState::ProtectedOrBlack,
                        Button::new(RichText::new(i18n::t_stop(language)).size(16.0)),
                    )
                    .clicked()
                {
                    *stop_btn = true;
                }
            });

            ui.separator();

            // Preview images using egui::Image::new(egui::ImageSource::Texture(...))
            // For ColorImage, we need to go through a texture handle or use the image widget differently.
            // In egui 0.31, ColorImage can be shown via:
            //   ui.add(egui::Image::new(egui::load::SizedTexture::new(texture_id, size)));
            // But for simplicity, we can use `egui::widgets::Image::from_color_image`.
            // Actually, the simplest way: use `egui::Image::from_bytes()` or convert to texture.

            // For now, skip thumbnails in favor of getting the rest compiling.
            // The test_frame preview below handles the most important preview case.

            // --- Test frame preview ---
            if let Some(ref rgba_data) = self.test_frame_rgba {
                if self.test_frame_w > 0 && self.test_frame_h > 0 {
                    ui.separator();
                    ui.label(RichText::new(i18n::t_test_preview(language)).strong());

                    // Show as a raw image created from byte data
                    let available = ui.available_size();
                    let preview_w = (320.0_f32).min(available.x - 10.0);
                    let preview_h = (240.0_f32).min(available.y - 20.0);

                    let (cw, ch) = (self.test_frame_w as usize, self.test_frame_h as usize);
                    if cw > 0 && ch > 0 {
                        let image = egui::ColorImage::from_rgba_unmultiplied([cw, ch], rgba_data);
                        // Use texture sharing via the painter
                        let texture = ui.ctx().load_texture(
                            "test_preview",
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                            texture.id(),
                            egui::Vec2::new(preview_w, preview_h),
                        )));
                    }
                }
            }

            if !self.state_message.is_empty()
                && self.current_state == CaptureState::ProtectedOrBlack
            {
                ui.separator();
                ui.colored_label(
                    Color32::RED,
                    "⚠ Protected or black content detected. Capture paused.\n\
                     The window might be showing protected video content or is blank.\n\
                     PPT will resume automatically when content becomes visible.",
                );
            }
        });
    }
}
