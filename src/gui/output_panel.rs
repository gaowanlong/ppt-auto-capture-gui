use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use egui::*;

#[derive(Clone)]
pub struct OutputPanel {
    pub output_dir: String,
    pub output_filename: String,
    pub filename_is_custom: bool,
    pub page_ratio: String,
    pub image_fit: String,
    pub auto_crop: bool,
    pub keep_previous: bool,
    pub open_output_requested: bool,
    pub manage_folders_requested: bool,
    pub status_text: String,
}

impl OutputPanel {
    pub fn new_with_filename(filename: &str) -> Self {
        Self {
            output_dir: "output".into(),
            output_filename: filename.to_string(),
            filename_is_custom: false,
            page_ratio: "16:9".into(),
            image_fit: "fit".into(),
            auto_crop: false,
            keep_previous: true,
            open_output_requested: false,
            manage_folders_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language, locked: bool) {
        theme::page_header(
            ui,
            i18n::t_output_page_title(language),
            i18n::t_output_page_subtitle(language),
        );

        if locked {
            theme::banner_frame(token::WARN_SOFT, token::WARN_LINE).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(i18n::t_output_locked(language))
                        .size(12.5)
                        .color(token::WARN),
                );
            });
            ui.add_space(14.0);
        }

        if !self.status_text.is_empty() {
            theme::banner_frame(token::OK_SOFT, token::LINE).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&self.status_text).size(12.5).color(token::OK));
            });
            ui.add_space(14.0);
        }

        ui.add_enabled_ui(!locked, |ui| {
            self.where_card(ui, language);
            ui.add_space(14.0);
            self.layout_card(ui, language);
            ui.add_space(14.0);
            self.folders_card(ui, language);
        });

        ui.add_space(18.0);
        theme::section_title(ui, i18n::t_output_notes_heading(language));
        ui.add_space(5.0);
        for note in [
            i18n::t_output_notes_1(language),
            i18n::t_output_notes_2(language),
            i18n::t_output_notes_3(language),
            i18n::t_output_notes_4(language),
        ] {
            theme::note_row(ui, note);
        }
    }

    fn where_card(&mut self, ui: &mut Ui, language: Language) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_output_where(language));
            ui.add_space(4.0);
            let wide = ui.available_width() >= 560.0;

            field_row(ui, wide, i18n::t_output_dir(language), |ui| {
                let width = ui.available_width().min(430.0);
                ui.add(
                    TextEdit::singleline(&mut self.output_dir)
                        .desired_width(width)
                        .hint_text("output"),
                );
                ui.label(theme::muted(i18n::t_output_dir_hint(language), 11.5));
            });
            ui.add_space(8.0);

            field_row(ui, wide, i18n::t_pptx_filename(language), |ui| {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 112.0).clamp(140.0, 380.0);
                    if ui
                        .add(
                            TextEdit::singleline(&mut self.output_filename)
                                .desired_width(width)
                                .hint_text("deck.pptx"),
                        )
                        .changed()
                    {
                        self.filename_is_custom = true;
                    }
                    if ui
                        .add(theme::secondary_button(i18n::t_automatic_name(language)))
                        .clicked()
                    {
                        self.filename_is_custom = false;
                        self.output_filename = format!(
                            "ppt-capture-{}.pptx",
                            chrono::Local::now().format("%Y%m%d-%H%M%S")
                        );
                    }
                });
                ui.label(theme::muted(i18n::t_filename_hint(language), 11.5));
            });
        });
    }

    fn layout_card(&mut self, ui: &mut Ui, language: Language) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_output_layout(language));
            ui.add_space(6.0);
            let wide = ui.available_width() >= 560.0;

            let ratio = i18n::t_slide_aspect(language)
                .trim_end_matches([':', '：'])
                .to_string();
            let fit = i18n::t_image_fit(language)
                .trim_end_matches([':', '：'])
                .to_string();
            let fit_options = [
                ("fill", i18n::t_fit_fill(language)),
                ("fit", i18n::t_fit_fit(language)),
            ];
            if wide {
                ui.columns(2, |columns| {
                    choice(
                        &mut columns[0],
                        &ratio,
                        "page_ratio_selector",
                        &mut self.page_ratio,
                        &[
                            ("16:9", "16:9"),
                            ("4:3", "4:3"),
                            ("3:2", "3:2"),
                            ("16:10", "16:10"),
                        ],
                    );
                    choice(
                        &mut columns[1],
                        &fit,
                        "image_fit_selector",
                        &mut self.image_fit,
                        &fit_options,
                    );
                });
            } else {
                choice(
                    ui,
                    &ratio,
                    "page_ratio_selector",
                    &mut self.page_ratio,
                    &[
                        ("16:9", "16:9"),
                        ("4:3", "4:3"),
                        ("3:2", "3:2"),
                        ("16:10", "16:10"),
                    ],
                );
                ui.add_space(6.0);
                choice(
                    ui,
                    &fit,
                    "image_fit_selector",
                    &mut self.image_fit,
                    &fit_options,
                );
            }

            theme::divider(ui);
            ui.checkbox(&mut self.auto_crop, i18n::t_auto_crop_label(language));
            ui.label(theme::muted(i18n::t_auto_crop_hint(language), 11.5));
        });
    }

    fn folders_card(&mut self, ui: &mut Ui, language: Language) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_output_folders(language));
            ui.add_space(6.0);

            let root = if self.output_dir.trim().is_empty() {
                ".".to_string()
            } else {
                self.output_dir.trim().to_string()
            };
            theme::soft_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::mono(root));
            });
            ui.add_space(6.0);
            ui.label(theme::muted(i18n::t_output_folders_hint(language), 11.5));
            ui.add_space(6.0);

            let wide = ui.available_width() >= 560.0;
            if wide {
                ui.horizontal(|ui| {
                    if ui
                        .add(theme::secondary_button(i18n::t_open_recording_folder(
                            language,
                        )))
                        .clicked()
                    {
                        self.open_output_requested = true;
                    }
                    if ui
                        .add(theme::secondary_button(i18n::t_manage_recording_folders(
                            language,
                        )))
                        .clicked()
                    {
                        self.manage_folders_requested = true;
                    }
                });
            } else {
                ui.vertical(|ui| {
                    if ui
                        .add(theme::secondary_button(i18n::t_open_recording_folder(
                            language,
                        )))
                        .clicked()
                    {
                        self.open_output_requested = true;
                    }
                    if ui
                        .add(theme::secondary_button(i18n::t_manage_recording_folders(
                            language,
                        )))
                        .clicked()
                    {
                        self.manage_folders_requested = true;
                    }
                });
            }
        });
    }
}

fn field_row(ui: &mut Ui, wide: bool, label: &str, add: impl FnOnce(&mut Ui)) {
    if wide {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(190.0, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_min_width(190.0);
                ui.add_space(8.0);
                ui.label(RichText::new(label).size(12.5).color(token::INK).strong());
            });
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                add(ui);
            });
        });
    } else {
        ui.label(RichText::new(label).size(12.5).color(token::INK).strong());
        ui.add_space(2.0);
        add(ui);
    }
}

fn choice(ui: &mut Ui, label: &str, id: &str, value: &mut String, options: &[(&str, &str)]) {
    ui.label(theme::muted(label, 12.0));
    ui.add_space(2.0);
    let selected = options
        .iter()
        .find(|(key, _)| *key == value.as_str())
        .map(|(_, text)| *text)
        .unwrap_or(value.as_str());
    ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width().min(220.0))
        .show_ui(ui, |ui| {
            for (key, text) in options {
                if ui.selectable_label(value.as_str() == *key, *text).clicked() {
                    *value = (*key).to_string();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_filename_is_owned_by_the_output_panel() {
        let panel = OutputPanel::new_with_filename("configured.pptx");
        assert_eq!(panel.output_filename, "configured.pptx");
    }

    #[test]
    fn folder_actions_start_unrequested() {
        let panel = OutputPanel::new_with_filename("deck.pptx");
        assert!(!panel.manage_folders_requested);
        assert!(!panel.open_output_requested);
    }
}
