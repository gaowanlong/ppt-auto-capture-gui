use egui::*;
use crate::model::WindowInfo;

pub struct SourcePanel {
    pub windows: Vec<WindowInfo>,
    pub selected_hwnd: u64,
    pub selected_title: String,
    pub refresh_requested: bool,
    pub move_requested: bool,
    pub maximize_requested: bool,
    pub test_requested: bool,
    pub status_text: String,
}

impl SourcePanel {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            selected_hwnd: 0,
            selected_title: "None".into(),
            refresh_requested: false,
            move_requested: false,
            maximize_requested: false,
            test_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui, monitor_ready: bool) {
        let has_selection = self.selected_hwnd != 0;
        ui.vertical(|ui| {
            ui.heading("Window Source");

            if ui.button("🔄 Refresh Window List").clicked() {
                self.refresh_requested = true;
            }

            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    if self.windows.is_empty() {
                        ui.label("No windows found. Click Refresh.");
                    } else {
                        let mut sorted: Vec<&WindowInfo> = self.windows.iter().collect();
                        sorted.sort_by(|a, b| {
                            let a_score = if a.is_powerpoint { 1 } else { 0 };
                            let b_score = if b.is_powerpoint { 1 } else { 0 };
                            b_score.cmp(&a_score)
                                .then_with(|| a.title.cmp(&b.title))
                        });

                        for win in sorted {
                            let label = format!("{} ({}) [{}x{}] {}",
                                win.title,
                                win.process_name,
                                win.region.width, win.region.height,
                                if win.is_powerpoint { "📊" } else { "" }
                            );

                            if self.selected_hwnd == win.hwnd {
                                if ui.selectable_label(true, &label).clicked() {
                                    self.selected_hwnd = win.hwnd;
                                    self.selected_title = win.title.clone();
                                    self.status_text = format!("Selected: {}", win.title);
                                }
                            } else {
                                if ui.selectable_label(false, &label).clicked() {
                                    self.selected_hwnd = win.hwnd;
                                    self.selected_title = win.title.clone();
                                    self.status_text = format!("Selected: {}", win.title);
                                }
                            }
                        }
                    }
                });

            ui.separator();
            ui.label(format!("Selected: {}", self.selected_title));
            ui.label(format!("HWND: {}", if has_selection { format!("0x{:X}", self.selected_hwnd) } else { "None".into() }));

            ui.separator();

            ui.horizontal(|ui| {
                if ui.add_enabled(has_selection && monitor_ready, Button::new("↗ Move to Display")).clicked() {
                    self.move_requested = true;
                }
                if ui.add_enabled(has_selection, Button::new("⬜ Maximize")).clicked() {
                    self.maximize_requested = true;
                }
            });

            if ui.add_enabled(has_selection, Button::new("📷 Test Screenshot")).clicked() {
                self.test_requested = true;
            }

            if !self.status_text.is_empty() {
                ui.separator();
                ui.label(&self.status_text);
            }
        });
    }
}
