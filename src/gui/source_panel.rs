use egui::*;
use crate::i18n::{self, Language};
use crate::model::WindowInfo;

pub struct SourcePanel {
    pub windows: Vec<WindowInfo>,
    pub selected_hwnd: u64,
    pub selected_title: String,
    /// True when user explicitly opted into full-screen capture (selected_hwnd == 0).
    /// Prevents auto-selection from overriding the choice on window list refresh.
    pub full_screen_selected: bool,
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
            full_screen_selected: false,
            refresh_requested: false,
            move_requested: false,
            maximize_requested: false,
            test_requested: false,
            status_text: String::new(),
        }
    }

    pub fn render(&mut self, ui: &mut Ui, language: Language, monitor_ready: bool) {
        let has_window_selected = self.selected_hwnd != 0;
        ui.vertical(|ui| {
            ui.heading(i18n::t_window_source(language));
            ui.label(RichText::new(match language {
                Language::English => "Select the meeting or slideshow window. 📊 = PPT slideshow recommended.",
                Language::Chinese => "选择会议窗口或幻灯片放映窗口。📊 = 推荐的PPT放映窗口。",
            }).size(11.0).color(egui::Color32::GRAY));

            if ui.button(i18n::t_refresh_windows(language)).clicked() {
                self.refresh_requested = true;
            }

            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    // "Full Screen" option at the top — no window clipping
                    let full_screen_label = match language {
                        Language::English => "📺 Full Screen (capture entire monitor)",
                        Language::Chinese => "📺 全屏截图（捕获整个显示器）",
                    };
                    let is_full_screen = self.selected_hwnd == 0;
                    if ui.selectable_label(is_full_screen, full_screen_label).clicked() {
                        self.selected_hwnd = 0;
                        self.selected_title = String::new();
                        self.full_screen_selected = true;
                        self.status_text = match language {
                            Language::English => "Selected: Full Screen".into(),
                            Language::Chinese => "已选择：全屏截图".into(),
                        };
                    }

                    if self.windows.is_empty() {
                        ui.label(i18n::t_no_windows(language));
                    } else {
                        let mut sorted: Vec<&WindowInfo> = self.windows.iter().collect();
                        sorted.sort_by(|a, b| {
                            let a_score = if a.is_powerpoint { 1 } else { 0 };
                            let b_score = if b.is_powerpoint { 1 } else { 0 };
                            b_score.cmp(&a_score)
                                .then_with(|| a.title.cmp(&b.title))
                        });

                        for win in sorted {
                            let pos_label = format!("({},{})", win.region.x, win.region.y);
                            let label = format!("{} [{}x{} @{}] {} {}",
                                win.title,
                                win.region.width, win.region.height,
                                pos_label,
                                if win.is_powerpoint { "📊" } else { "" },
                                if win.is_minimized { "(minimized)" } else { "" }
                            );

                                if self.selected_hwnd == win.hwnd {
                                    if ui.selectable_label(true, &label).clicked() {
                                        self.selected_hwnd = win.hwnd;
                                        self.selected_title = win.title.clone();
                                        self.full_screen_selected = false;
                                        self.status_text = format!("Selected: {}", win.title);
                                }
                                } else {
                                    if ui.selectable_label(false, &label).clicked() {
                                        self.selected_hwnd = win.hwnd;
                                        self.selected_title = win.title.clone();
                                        self.full_screen_selected = false;
                                        self.status_text = format!("Selected: {}", win.title);
                                }
                            }
                        }
                    }
                });

            ui.separator();
            ui.label(format!("{} {}", i18n::t_selected(language),
                if self.selected_hwnd == 0 {
                    match language { Language::English => "Full Screen", Language::Chinese => "全屏", }
                } else {
                    &self.selected_title
                }
            ));
            ui.label(format!("HWND: {}", if has_window_selected { format!("0x{:X}", self.selected_hwnd) } else { "None".into() }));

            ui.separator();

            ui.horizontal(|ui| {
                if ui.add_enabled(has_window_selected && monitor_ready, Button::new(i18n::t_move_to_display(language))).clicked() {
                    self.move_requested = true;
                }
                if ui.add_enabled(has_window_selected, Button::new(i18n::t_maximize(language))).clicked() {
                    self.maximize_requested = true;
                }
            });

            // Test screenshot works for both full screen and window capture
            if ui.add_enabled(!self.windows.is_empty() || self.selected_hwnd == 0, Button::new(i18n::t_test_screenshot(language))).clicked() {
                self.test_requested = true;
            }

            if !self.status_text.is_empty() {
                ui.separator();
                ui.label(&self.status_text);
            }
        });
    }
}
