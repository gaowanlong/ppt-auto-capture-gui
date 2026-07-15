use egui::*;
use crate::i18n::{self, Language};
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

    pub fn render(&mut self, ui: &mut Ui, language: Language) {
        self.changed = false;

        ui.vertical(|ui| {
            ui.heading(i18n::t_settings_title(language));

            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .striped(true)
                .show(ui, |grid| {
                    grid.label(i18n::t_sample_interval(language));
                    let resp = grid.add(Slider::new(&mut self.sample_interval_ms, 100..=5000).text("ms"));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "How often to check the display for changes. Lower = more responsive but higher CPU usage.",
                        Language::Chinese => "多久检查一次显示器变化。数值越低越灵敏，但CPU占用更高。",
                    });
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label(i18n::t_stability_frames(language));
                    let resp = grid.add(Slider::new(&mut self.stability_frames, 1..=10).text("frames"));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "Number of consecutive identical frames required before saving. Higher = fewer accidental captures.",
                        Language::Chinese => "保存前需要连续相同帧的数量。数值越高，误捕获越少。",
                    });
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label(i18n::t_anim_timeout(language));
                    let resp = grid.add(Slider::new(&mut self.animation_timeout_ms, 1000..=30000).text("ms"));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "Maximum time to wait for animation to stabilize. Force-captures after this duration.",
                        Language::Chinese => "等待动画稳定的最长时间。超过此时间将强制截图。",
                    });
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label(i18n::t_change_threshold(language));
                    let mut val = self.change_threshold * 100.0;
                    let resp = grid.add(Slider::new(&mut val, 0.1..=50.0).text("%"));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "Fraction of pixels that must change to detect a new slide. Lower = more sensitive to small animation changes.",
                        Language::Chinese => "需要改变的像素比例。数值越低，对微小动画变化越敏感。",
                    });
                    self.change_threshold = val / 100.0;
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label(i18n::t_black_threshold(language));
                    let mut val = self.black_threshold * 100.0;
                    let resp = grid.add(Slider::new(&mut val, 50.0..=100.0).text("%"));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "Threshold at which a frame is considered black/protected. Lower = tolerates more dark content.",
                        Language::Chinese => "判断黑屏/受保护内容的阈值。数值越低，容忍更多暗色内容。",
                    });
                    self.black_threshold = val / 100.0;
                    if resp.changed() { self.changed = true; }
                    grid.end_row();

                    grid.label(i18n::t_filter_duplicates(language));
                    let resp = grid.add(Checkbox::new(&mut self.filter_duplicates, ""));
                    let resp = resp.on_hover_text(match language {
                        Language::English => "Skip saving slides that are identical to the previous one. Prevents duplicates in output.",
                        Language::Chinese => "跳过与上一张完全相同的幻灯片，避免输出中出现重复内容。",
                    });
                    if resp.changed() { self.changed = true; }
                    grid.end_row();
                });

            ui.separator();
            ui.label(RichText::new(match language {
                Language::English => "Tip: Hover over each setting for details.",
                Language::Chinese => "提示：将鼠标悬停在每个设置项上查看详细说明。",
            }).size(11.0).color(egui::Color32::GRAY));
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
