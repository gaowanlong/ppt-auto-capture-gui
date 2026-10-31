use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use crate::model::WindowInfo;
use egui::*;

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
        theme::page_header(
            ui,
            i18n::t_window_source(language),
            match language {
                Language::English => {
                    "Choose the slideshow or meeting window. PowerPoint windows appear first."
                }
                Language::Chinese => "选择幻灯片放映或会议窗口，PPT 窗口会排在前面。",
            },
        );

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match language {
                        Language::English => "Available windows",
                        Language::Chinese => "可用窗口",
                    })
                    .size(13.0)
                    .color(token::INK)
                    .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(theme::secondary_button(i18n::t_refresh(language)))
                        .clicked()
                    {
                        self.refresh_requested = true;
                    }
                });
            });
            theme::divider(ui);

            egui::ScrollArea::vertical()
                .id_salt("source_windows")
                .max_height(300.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let full_screen_label = match language {
                        Language::English => "Full screen",
                        Language::Chinese => "全屏截图",
                    };
                    let full_screen_detail = match language {
                        Language::English => "Capture the entire selected display",
                        Language::Chinese => "捕获所选的整个显示器",
                    };
                    if theme::list_item(
                        ui,
                        self.selected_hwnd == 0,
                        full_screen_label,
                        full_screen_detail,
                    )
                    .clicked()
                    {
                        self.selected_hwnd = 0;
                        self.selected_title = String::new();
                        self.full_screen_selected = true;
                        self.status_text = match language {
                            Language::English => "Selected: Full screen".into(),
                            Language::Chinese => "已选择：全屏截图".into(),
                        };
                    }

                    if self.windows.is_empty() {
                        theme::empty_note(
                            ui,
                            match language {
                                Language::English => {
                                    "No windows found yet. Choose Refresh to scan again."
                                }
                                Language::Chinese => "尚未找到窗口，点击刷新重新扫描。",
                            },
                        );
                    } else {
                        let mut sorted: Vec<&WindowInfo> = self.windows.iter().collect();
                        sorted.sort_by(|a, b| {
                            let a_score = i32::from(a.is_powerpoint);
                            let b_score = i32::from(b.is_powerpoint);
                            b_score.cmp(&a_score).then_with(|| a.title.cmp(&b.title))
                        });

                        for win in sorted {
                            // Untitled helper windows (status items, invisible
                            // overlays) cannot be identified in the list, so
                            // leave them out instead of showing blank rows.
                            if win.title.trim().is_empty() {
                                continue;
                            }
                            let mut detail = format!(
                                "{}×{}  at {},{}",
                                win.region.width, win.region.height, win.region.x, win.region.y
                            );
                            if win.is_powerpoint {
                                detail.push_str(match language {
                                    Language::English => "  (PowerPoint)",
                                    Language::Chinese => "  (PPT 放映)",
                                });
                            }
                            if win.is_minimized {
                                detail.push_str(match language {
                                    Language::English => "  (minimized)",
                                    Language::Chinese => "  (已最小化)",
                                });
                            }
                            if theme::list_item(
                                ui,
                                self.selected_hwnd == win.hwnd,
                                &win.title,
                                &detail,
                            )
                            .clicked()
                            {
                                self.selected_hwnd = win.hwnd;
                                self.selected_title = win.title.clone();
                                self.full_screen_selected = false;
                                self.status_text =
                                    format!("{} {}", i18n::t_selected(language), win.title);
                            }
                        }
                    }
                });
        });

        ui.add_space(18.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(theme::muted(
                match language {
                    Language::English => "Current selection",
                    Language::Chinese => "当前选择",
                },
                11.0,
            ));
            ui.add_space(2.0);
            ui.label(
                RichText::new(if self.selected_hwnd == 0 {
                    match language {
                        Language::English => "Full screen",
                        Language::Chinese => "全屏截图",
                    }
                } else {
                    self.selected_title.as_str()
                })
                .size(16.0)
                .color(token::INK)
                .strong(),
            );
            ui.add_space(2.0);
            ui.label(theme::mono(format!(
                "HWND {}",
                if has_window_selected {
                    format!("0x{:X}", self.selected_hwnd)
                } else {
                    "—".into()
                }
            )));

            theme::divider(ui);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                let window_controls_available = cfg!(target_os = "windows");
                let move_button = ui.add_enabled(
                    window_controls_available && has_window_selected && monitor_ready,
                    theme::secondary_button(match language {
                        Language::English => "Move to display",
                        Language::Chinese => "移到显示器",
                    }),
                );
                #[cfg(target_os = "macos")]
                let move_button = move_button
                    .on_disabled_hover_text(i18n::t_macos_window_controls_unavailable(language));
                if move_button.clicked() {
                    self.move_requested = true;
                }

                let maximize_button = ui.add_enabled(
                    window_controls_available && has_window_selected,
                    theme::secondary_button(match language {
                        Language::English => "Maximize",
                        Language::Chinese => "最大化",
                    }),
                );
                #[cfg(target_os = "macos")]
                let maximize_button = maximize_button
                    .on_disabled_hover_text(i18n::t_macos_window_controls_unavailable(language));
                if maximize_button.clicked() {
                    self.maximize_requested = true;
                }

                if ui
                    .add_enabled(
                        !self.windows.is_empty() || self.selected_hwnd == 0,
                        theme::secondary_button(i18n::t_test_screenshot_short(language)),
                    )
                    .clicked()
                {
                    self.test_requested = true;
                }
            });

            if !self.status_text.is_empty() {
                ui.add_space(6.0);
                ui.label(theme::muted(self.status_text.clone(), 11.5));
            }
        });
    }
}
