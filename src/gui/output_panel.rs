use egui::*;
use crate::i18n::{self, Language};

pub struct OutputPanel {
    pub output_dir: String,
    pub output_filename: String,
    pub page_ratio: String,
    pub image_fit: String,
    pub keep_previous: bool,
    pub open_output_requested: bool,
    pub status_text: String,
}

impl OutputPanel {
    pub fn new() -> Self {
        Self::new_with_filename("output.pptx")
    }

    pub fn new_with_filename(filename: &str) -> Self {
        Self {
            output_dir: "output".into(),
            output_filename: filename.to_string(),
            page_ratio: "16:9".into(),
            image_fit: "fit".into(),
            keep_previous: true,
            open_output_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language) {
        ui.vertical(|ui| {
            ui.heading(i18n::t_output_settings(language));

            // --- Editable fields ---
            ui.horizontal(|ui| {
                ui.label(i18n::t_output_dir(language));
                ui.add(TextEdit::singleline(&mut self.output_dir));
            });
            ui.horizontal(|ui| {
                ui.label(i18n::t_pptx_filename(language));
                ui.add(TextEdit::singleline(&mut self.output_filename));
            });

            ui.separator();

            // --- Aspect ratio ---
            ui.horizontal(|ui| {
                ui.label(i18n::t_slide_aspect(language));
                let ratios = ["16:9", "4:3", "3:2", "16:10"];
                egui::ComboBox::from_id_salt("page_ratio_selector")
                    .selected_text(&self.page_ratio)
                    .show_ui(ui, |ui| {
                        for ratio in &ratios {
                            ui.selectable_value(&mut self.page_ratio, ratio.to_string(), *ratio);
                        }
                    });
            });

            // --- Image fit ---
            ui.horizontal(|ui| {
                ui.label(i18n::t_image_fit(language));
                let fits = ["fill", "fit"];
                egui::ComboBox::from_id_salt("image_fit_selector")
                    .selected_text(&self.image_fit)
                    .show_ui(ui, |ui| {
                        for fit in &fits {
                            ui.selectable_value(&mut self.image_fit, fit.to_string(), *fit);
                        }
                    });
            });

            // --- Keep previous checkbox ---
            ui.checkbox(&mut self.keep_previous, i18n::t_keep_previous(language));

            ui.separator();

            if ui.button(i18n::t_open_output(language)).clicked() {
                self.open_output_requested = true;
            }

            if !self.status_text.is_empty() {
                ui.separator();
                ui.label(&self.status_text);
            }

            ui.separator();
            ui.label(RichText::new(i18n::t_output_notes(language)).strong());
            ui.label(i18n::t_output_notes_1(language));
            ui.label(i18n::t_output_notes_2(language));
            ui.label(i18n::t_output_notes_3(language));
            ui.label(i18n::t_output_notes_4(language));
        });
    }
}
