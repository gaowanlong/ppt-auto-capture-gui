use egui::*;

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
        Self {
            output_dir: "output".into(),
            output_filename: "output.pptx".into(),
            page_ratio: "16:9".into(),
            image_fit: "fill".into(),
            keep_previous: true,
            open_output_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Output Settings");

            // --- Editable fields ---
            ui.horizontal(|ui| {
                ui.label("Output Directory:");
                ui.add(TextEdit::singleline(&mut self.output_dir));
            });
            ui.horizontal(|ui| {
                ui.label("PPTX Filename:");
                ui.add(TextEdit::singleline(&mut self.output_filename));
            });

            ui.separator();

            // --- Aspect ratio ---
            ui.horizontal(|ui| {
                ui.label("Slide Aspect Ratio:");
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
                ui.label("Image Fit:");
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
            ui.checkbox(&mut self.keep_previous, "Keep Previous.pptx");

            ui.separator();

            if ui.button("📂 Open Output Directory").clicked() {
                self.open_output_requested = true;
            }

            if !self.status_text.is_empty() {
                ui.separator();
                ui.label(&self.status_text);
            }

            ui.separator();
            ui.label(RichText::new("Output notes:").strong());
            ui.label("• PNG files are saved to output/slides/ directory.");
            ui.label("• output.pptx is rebuilt with each new slide.");
            ui.label("• output.previous.pptx keeps the last version for safety.");
            ui.label("• manifest.jsonl tracks all captured slides for recovery.");
        });
    }
}
