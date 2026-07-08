use egui::*;
use crate::capture::CaptureConfig;

pub struct SettingsPanel {
    pub sample_interval_ms: u64,
    pub stability_frames: u32,
    pub animation_timeout_ms: u64,
    pub change_threshold: f64,
    pub black_threshold: f64,
    pub filter_duplicates: bool,
    pub changed: bool,
}

impl SettingsPanel {
    pub fn new() -> Self {
        let cfg = CaptureConfig::default();
        Self {
            sample_interval_ms: cfg.sample_interval_ms,
            stability_frames: cfg.stability_frames,
            animation_timeout_ms: cfg.animation_timeout_ms,
            change_threshold: cfg.change_threshold,
            black_threshold: cfg.black_threshold,
            filter_duplicates: true,
            changed: false,
        }
    }

    pub fn render(&mut self, ui: &mut Ui) {
        self.changed = false;

        ui.vertical(|ui| {
            ui.heading("Capture Settings");

            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .striped(true)
                .show(ui, |grid| {
                    let resp = grid.add(Slider::new(&mut self.sample_interval_ms, 100..=5000).text("ms"));
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    let resp = grid.add(Slider::new(&mut self.stability_frames, 1..=10).text("frames"));
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    let resp = grid.add(Slider::new(&mut self.animation_timeout_ms, 1000..=30000).text("ms"));
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    let mut val = self.change_threshold * 100.0;
                    let resp = grid.add(Slider::new(&mut val, 0.1..=50.0).text("%"));
                    self.change_threshold = val / 100.0;
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    let mut val = self.black_threshold * 100.0;
                    let resp = grid.add(Slider::new(&mut val, 50.0..=100.0).text("%"));
                    self.black_threshold = val / 100.0;
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label("Filter Duplicates:");
                    let resp = grid.add(Checkbox::new(&mut self.filter_duplicates, ""));
                    if resp.changed() { self.changed = true; }
                    grid.end_row();
                });

            ui.separator();
            ui.label(RichText::new("Threshold tips:").strong());
            ui.label("• Change Threshold: Higher = only capture when large changes occur.");
            ui.label("• Lower = capture even small slide animation changes.");
            ui.label("• Black Threshold: How much black content triggers protected mode.");
            ui.label("• Higher Stability Frames = fewer accidental captures during animations.");
            ui.label("• Animation Timeout: Force-capture after this time regardless of stability.");
        });
    }

    pub fn get_config(&self) -> CaptureConfig {
        CaptureConfig {
            sample_interval_ms: self.sample_interval_ms,
            stability_frames: self.stability_frames,
            animation_timeout_ms: self.animation_timeout_ms,
            change_threshold: self.change_threshold,
            black_threshold: self.black_threshold,
            filter_duplicates: self.filter_duplicates,
        }
    }
}
