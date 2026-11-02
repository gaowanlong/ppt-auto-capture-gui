//! The appearance page: light/dark palette and an optional picture behind the
//! workspace. Every control here changes what the user sees right away; the
//! application persists the values and repaints from [`crate::app`].

use egui::*;
use std::path::PathBuf;

use crate::config::{AppConfig, ThemeMode, DEFAULT_BACKGROUND_DIM};
use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};

pub struct AppearancePanel {
    pub mode: ThemeMode,
    /// The chosen picture, if any. Only the path is stored; the texture lives in
    /// the application, which owns the egui context.
    pub background: Option<PathBuf>,
    /// How much the picture is faded behind the interface, in percent.
    pub dim: f64,
    /// The picture could not be decoded; the workspace stays a solid colour.
    pub error: Option<String>,
    /// A control changed this frame and the application has to persist it.
    pub changed: bool,
    pub pick_requested: bool,
    pub clear_requested: bool,
}

impl AppearancePanel {
    pub fn new() -> Self {
        Self {
            mode: ThemeMode::Light,
            background: None,
            dim: DEFAULT_BACKGROUND_DIM,
            error: None,
            changed: false,
            pick_requested: false,
            clear_requested: false,
        }
    }

    pub fn load_from_config(&mut self, config: &AppConfig) {
        self.mode = config.theme_mode;
        self.background = if config.background_image.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(config.background_image.trim()))
        };
        self.dim = config.background_dim.clamp(0.0, MAX_DIM);
    }

    /// The path to store in the configuration file.
    pub fn background_setting(&self) -> String {
        self.background
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language, preview: Option<&TextureHandle>) {
        theme::page_header(
            ui,
            i18n::t_appearance_title(language),
            i18n::t_appearance_subtitle(language),
        );

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_appearance_theme(language));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for (mode, label) in [
                    (ThemeMode::Light, i18n::t_theme_light(language)),
                    (ThemeMode::Dark, i18n::t_theme_dark(language)),
                    (ThemeMode::System, i18n::t_theme_system(language)),
                ] {
                    let selected = self.mode == mode;
                    if ui.add(theme::segment_button(label, selected)).clicked() && !selected {
                        self.mode = mode;
                        self.changed = true;
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(theme::muted(i18n::t_theme_hint(language), 11.5));
        });

        ui.add_space(14.0);

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_appearance_background(language));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(theme::secondary_button(i18n::t_background_choose(language)))
                    .clicked()
                {
                    self.pick_requested = true;
                }
                if ui
                    .add_enabled(
                        self.background.is_some(),
                        theme::secondary_button(i18n::t_background_remove(language)),
                    )
                    .clicked()
                {
                    self.clear_requested = true;
                }
            });
            ui.add_space(8.0);
            match &self.background {
                Some(path) => {
                    theme::soft_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(path.display().to_string())
                                .monospace()
                                .size(11.0)
                                .color(token::muted()),
                        );
                    });
                }
                None => {
                    ui.label(theme::muted(i18n::t_background_none(language), 12.0));
                }
            }
            if let Some(error) = &self.error {
                ui.add_space(6.0);
                ui.label(RichText::new(error).size(12.0).color(token::danger()));
            }

            if let Some(texture) = preview {
                ui.add_space(12.0);
                self.preview(ui, texture, language);
            }

            ui.add_space(12.0);
            ui.label(
                RichText::new(i18n::t_background_dim(language))
                    .size(12.5)
                    .color(token::ink()),
            );
            ui.add_space(2.0);
            let response = theme::slider(ui, &mut self.dim, 0.0..=MAX_DIM, 5.0, 0, " %");
            if response.changed() {
                self.changed = true;
            }
            ui.label(theme::muted(i18n::t_background_dim_hint(language), 11.5));
            ui.add_space(4.0);
            ui.label(theme::muted(i18n::t_background_hint(language), 11.5));
        });
    }
}

impl AppearancePanel {
    /// Show the picture exactly as the workspace will show it: the same scrim
    /// the background painter applies sits on top of the thumbnail.
    fn preview(&self, ui: &mut Ui, texture: &TextureHandle, language: Language) {
        theme::section_title(ui, i18n::t_background_preview(language));
        ui.add_space(6.0);
        let source = texture.size_vec2();
        let ratio = if source.x > 0.0 {
            (source.y / source.x).clamp(0.35, 0.75)
        } else {
            0.5625
        };
        let width = ui.available_width().min(300.0);
        let height = (width * ratio).min(180.0);
        let image = ui.add(
            Image::new(egui::load::SizedTexture::from_handle(texture))
                .fit_to_exact_size(vec2(width, height))
                .corner_radius(8),
        );
        let alpha = ((self.dim.clamp(0.0, MAX_DIM) / 100.0) * 255.0).round() as u8;
        if alpha > 0 {
            let bg = token::bg();
            ui.painter().rect_filled(
                image.rect,
                8,
                Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), alpha),
            );
        }
    }
}

impl Default for AppearancePanel {
    fn default() -> Self {
        Self::new()
    }
}

/// The dimming slider stops here: a fully hidden picture is the same as no
/// picture, and 90% still leaves a hint of it behind the cards.
pub const MAX_DIM: f64 = 90.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_reads_the_saved_appearance() {
        let config = AppConfig {
            theme_mode: ThemeMode::Dark,
            background_image: "  /Users/allen/wall paper.jpg  ".into(),
            background_dim: 30.0,
            ..AppConfig::default()
        };
        let mut panel = AppearancePanel::new();
        panel.load_from_config(&config);
        assert_eq!(panel.mode, ThemeMode::Dark);
        assert_eq!(
            panel.background.as_deref(),
            Some(std::path::Path::new("/Users/allen/wall paper.jpg"))
        );
        assert_eq!(panel.dim, 30.0);
        assert_eq!(panel.background_setting(), "/Users/allen/wall paper.jpg");
    }

    #[test]
    fn an_empty_setting_means_no_background() {
        let mut panel = AppearancePanel::new();
        panel.load_from_config(&AppConfig::default());
        assert!(panel.background.is_none());
        assert_eq!(panel.background_setting(), "");
    }

    #[test]
    fn an_out_of_range_dimming_value_is_clamped() {
        let config = AppConfig {
            background_dim: 1000.0,
            ..AppConfig::default()
        };
        let mut panel = AppearancePanel::new();
        panel.load_from_config(&config);
        assert_eq!(panel.dim, MAX_DIM);
    }
}
