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
                if ui.add(TextEdit::singleline(&mut self.output_filename)).changed() {
                    self.filename_is_custom = true;
                }
                if ui.button(match language { Language::Chinese => "自动命名", Language::English => "Automatic" }).clicked() {
                    self.filename_is_custom = false;
                    self.output_filename = format!("ppt-capture-{}.pptx", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                }
            });
            ui.label(match language {
                Language::Chinese => "默认文件名在每次开始录制时自动更新；自定义文件名保持不变。",
                Language::English => "The default filename updates when recording starts; custom filenames are kept.",
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
            ui.checkbox(&mut self.auto_crop, match language {
                Language::Chinese => "生成 PPTX 时自动裁剪演示区域（无法识别时保留原图）",
                Language::English => "Automatically crop the presentation when exporting (keep original if uncertain)",
            });
            ui.label(match language {
                Language::Chinese => "录制使用开始时的设置；请在开始录制前选择。",
                Language::English => "Recording uses the setting selected when capture starts.",
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_filename_is_owned_by_the_output_panel() {
        let panel = OutputPanel::new_with_filename("configured.pptx");
        assert_eq!(panel.output_filename, "configured.pptx");
    }
}
