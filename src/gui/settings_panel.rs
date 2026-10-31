use crate::capture::CaptureConfig;
use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use egui::*;

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

    pub fn load_from_config(&mut self, cfg: &CaptureConfig) {
        let cfg = cfg.clone().normalized();
        self.sample_interval_ms = cfg.sample_interval_ms;
        self.stability_frames = cfg.stability_frames;
        self.animation_timeout_ms = cfg.animation_timeout_ms;
        self.change_threshold = cfg.change_threshold;
        self.black_threshold = cfg.black_threshold;
        self.filter_duplicates = cfg.filter_duplicates;
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language) {
        self.changed = false;

        theme::page_header(
            ui,
            i18n::t_settings_title(language),
            match language {
                Language::English => {
                    "Fine-tune how quickly slide changes are detected. Changes apply immediately."
                }
                Language::Chinese => "调整换页检测的灵敏度，修改会立即生效。",
            },
        );

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let wide = ui.available_width() >= 540.0;

            control_row(ui, wide, i18n::t_sample_interval(language), |ui| {
                let mut millis = self.sample_interval_ms as f64;
                let response = theme::slider(ui, &mut millis, 100.0..=5000.0, 10.0, 0, " ms");
                self.sample_interval_ms = millis.round() as u64;
                let response = response.on_hover_text(match language {
                    Language::English => "How often to check the display for changes. Lower values react faster but use more CPU.",
                    Language::Chinese => "多久检查一次显示器变化。数值越低越灵敏，但 CPU 占用更高。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });
            theme::divider(ui);

            control_row(ui, wide, i18n::t_stability_frames(language), |ui| {
                let unit = match language {
                    Language::English => " frames",
                    Language::Chinese => " 帧",
                };
                let mut frames = self.stability_frames as f64;
                let response = theme::slider(ui, &mut frames, 1.0..=10.0, 1.0, 0, unit);
                self.stability_frames = frames.round() as u32;
                let response = response.on_hover_text(match language {
                    Language::English => "Consecutive identical frames required before saving. Higher values reduce accidental captures.",
                    Language::Chinese => "保存前需要连续相同帧的数量。数值越高，误捕获越少。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });
            theme::divider(ui);

            control_row(ui, wide, i18n::t_anim_timeout(language), |ui| {
                let mut millis = self.animation_timeout_ms as f64;
                let response = theme::slider(ui, &mut millis, 1000.0..=30000.0, 100.0, 0, " ms");
                self.animation_timeout_ms = millis.round() as u64;
                let response = response.on_hover_text(match language {
                    Language::English => "Longest wait for an animation to settle. Capture is forced after this time.",
                    Language::Chinese => "等待动画稳定的最长时间，超过后会强制截图。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });
            theme::divider(ui);

            control_row(ui, wide, i18n::t_change_threshold(language), |ui| {
                let mut value = self.change_threshold * 100.0;
                let response = theme::slider(ui, &mut value, 0.1..=50.0, 0.1, 1, " %");
                self.change_threshold = value / 100.0;
                let response = response.on_hover_text(match language {
                    Language::English => "Share of pixels that must change to count as a new slide. Lower values notice smaller changes.",
                    Language::Chinese => "判定为新页面所需的像素变化比例。数值越低，对细微变化越敏感。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });
            theme::divider(ui);

            control_row(ui, wide, i18n::t_black_threshold(language), |ui| {
                let mut value = self.black_threshold * 100.0;
                let response = theme::slider(ui, &mut value, 50.0..=100.0, 1.0, 1, " %");
                self.black_threshold = value / 100.0;
                let response = response.on_hover_text(match language {
                    Language::English => "Threshold for treating a frame as black or protected. Lower values tolerate darker slides.",
                    Language::Chinese => "判断黑屏或受保护内容的阈值。数值越低，容忍更多暗色画面。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });
            theme::divider(ui);

            control_row(ui, wide, i18n::t_filter_duplicates(language), |ui| {
                let state = match (self.filter_duplicates, language) {
                    (true, Language::English) => "On",
                    (true, Language::Chinese) => "开启",
                    (false, Language::English) => "Off",
                    (false, Language::Chinese) => "关闭",
                };
                let response = ui.checkbox(&mut self.filter_duplicates, state);
                let response = response.on_hover_text(match language {
                    Language::English => "Ignore tiny pointer changes and learned local video motion when comparing with the last saved slide. Full-screen video cannot be reliably distinguished from slide changes; disable this if small content changes are missed.",
                    Language::Chinese => "与上一张比较时忽略微小鼠标变化和已识别的局部视频动态。全屏视频无法可靠区分换页；若细微内容变化被漏掉，可关闭此项。",
                });
                if response.changed() {
                    self.changed = true;
                }
            });

            ui.add_space(6.0);
            ui.label(theme::muted(
                match language {
                    Language::English => "Hover over a setting to see how it affects capture.",
                    Language::Chinese => "将鼠标悬停在设置项上可查看详细说明。",
                },
                11.5,
            ));
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

fn control_row(ui: &mut Ui, wide: bool, label: &str, add: impl FnOnce(&mut Ui)) {
    if wide {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(200.0, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_min_width(200.0);
                ui.add_space(6.0);
                ui.label(RichText::new(label).size(12.5).color(token::INK).strong());
            });
            add(ui);
        });
    } else {
        ui.label(RichText::new(label).size(12.5).color(token::INK).strong());
        ui.add_space(2.0);
        add(ui);
    }
}
