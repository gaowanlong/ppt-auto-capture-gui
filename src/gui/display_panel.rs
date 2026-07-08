//! Display Panel — monitor enumeration and selection.

use egui::*;
use crate::model::MonitorInfo;

pub struct DisplayPanel {
    pub monitors: Vec<MonitorInfo>,
    pub selected_hmonitor: u64,
    pub selected_description: String,
    pub refresh_requested: bool,
    pub test_capture_requested: bool,
    pub status_text: String,
}

impl DisplayPanel {
    pub fn new() -> Self {
        Self {
            monitors: Vec::new(),
            selected_hmonitor: 0,
            selected_description: "None".into(),
            refresh_requested: false,
            test_capture_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Capture Display");

            if ui.button("🔄 Refresh Displays").clicked() {
                self.refresh_requested = true;
            }

            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    if self.monitors.is_empty() {
                        ui.label("No displays found. Click Refresh.");
                    } else {
                        for mon in &self.monitors {
                            let virtual_label = if mon.is_virtual_suspect { "🖥️" } else { "" };
                            let label = format!("{} {} ({}x{}) {}",
                                mon.output_name.trim(),
                                if mon.is_primary { "★" } else { "" },
                                mon.region.width,
                                mon.region.height,
                                virtual_label
                            );

                            let selected = self.selected_hmonitor == mon.hmonitor;
                            if ui.selectable_label(selected, &label).clicked() {
                                self.selected_hmonitor = mon.hmonitor;
                                self.selected_description = format!("{} ({}x{})",
                                    mon.output_name.trim(), mon.region.width, mon.region.height);
                                self.status_text = format!("Selected: {}", mon.output_name.trim());
                            }

                            // Show details
                            if selected {
                                ui.indent("mon_details", |ui| {
                                    ui.label(format!("Adapter: {}", mon.adapter_name));
                                    ui.colored_label(
                                        if mon.is_virtual_suspect { Color32::YELLOW } else { Color32::GREEN },
                                        if mon.is_virtual_suspect { "⚠ Suspected virtual/dummy display" } else { "✅ Physical display" }
                                    );
                                    if mon.is_primary {
                                        ui.colored_label(Color32::GREEN, "★ Primary monitor");
                                    }
                                });
                            }
                        }
                    }
                });

            ui.separator();

            ui.label(format!("Selected: {}", self.selected_description));

            ui.separator();

            if ui.add_enabled(
                self.selected_hmonitor != 0,
                Button::new("📷 Test Capture")
            ).clicked() {
                self.test_capture_requested = true;
            }

            // Status
            if !self.status_text.is_empty() {
                ui.separator();
                ui.label(&self.status_text);
            }
        });
    }
}
