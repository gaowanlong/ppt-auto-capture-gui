//! Display Panel — monitor enumeration and selection.

use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use crate::model::MonitorInfo;
use egui::*;

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

    pub fn render(&mut self, ui: &mut Ui, language: Language) {
        theme::page_header(
            ui,
            i18n::t_capture_display(language),
            match language {
                Language::English => {
                    "Pick the screen that will show the presentation, then test the capture."
                }
                Language::Chinese => "选择用于放映幻灯片的显示器，并先测试截图效果。",
            },
        );

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match language {
                        Language::English => "Displays",
                        Language::Chinese => "显示器",
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
                .id_salt("display_list")
                .max_height(280.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.monitors.is_empty() {
                        theme::empty_note(
                            ui,
                            match language {
                                Language::English => "No displays found. Choose Refresh to scan.",
                                Language::Chinese => "未找到显示器，点击刷新重新扫描。",
                            },
                        );
                    } else {
                        for mon in &self.monitors {
                            let mut detail = format!("{}×{}", mon.region.width, mon.region.height);
                            if mon.is_primary {
                                detail.push_str(match language {
                                    Language::English => "  Primary",
                                    Language::Chinese => "  主显示器",
                                });
                            }
                            detail.push_str(if mon.is_virtual_suspect {
                                match language {
                                    Language::English => "  Suspected virtual",
                                    Language::Chinese => "  疑似虚拟",
                                }
                            } else {
                                match language {
                                    Language::English => "  Physical",
                                    Language::Chinese => "  物理显示器",
                                }
                            });

                            if theme::list_item(
                                ui,
                                self.selected_hmonitor == mon.hmonitor,
                                mon.output_name.trim(),
                                &detail,
                            )
                            .clicked()
                            {
                                self.selected_hmonitor = mon.hmonitor;
                                self.selected_description = format!(
                                    "{} ({}x{})",
                                    mon.output_name.trim(),
                                    mon.region.width,
                                    mon.region.height
                                );
                                self.status_text = format!(
                                    "{} {}",
                                    i18n::t_selected(language),
                                    mon.output_name.trim()
                                );
                            }
                        }
                    }
                });
        });

        ui.add_space(18.0);
        let selected_monitor = self
            .monitors
            .iter()
            .find(|mon| mon.hmonitor == self.selected_hmonitor);

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(theme::muted(
                match language {
                    Language::English => "Selected display",
                    Language::Chinese => "当前显示器",
                },
                11.0,
            ));
            ui.add_space(2.0);
            ui.label(
                RichText::new(&self.selected_description)
                    .size(16.0)
                    .color(token::INK)
                    .strong(),
            );

            if let Some(mon) = selected_monitor {
                ui.add_space(6.0);
                ui.label(theme::muted(
                    format!("{} {}", i18n::t_adapter(language), mon.adapter_name),
                    11.5,
                ));
                ui.add_space(2.0);
                if mon.is_virtual_suspect {
                    ui.label(
                        RichText::new(i18n::t_virtual_suspect(language))
                            .size(11.5)
                            .color(token::WARN),
                    );
                } else {
                    ui.label(
                        RichText::new(i18n::t_physical_display(language))
                            .size(11.5)
                            .color(token::OK),
                    );
                }
                if mon.is_primary {
                    ui.label(
                        RichText::new(i18n::t_primary_monitor(language))
                            .size(11.5)
                            .color(token::OK),
                    );
                }
            }

            theme::divider(ui);
            if ui
                .add_enabled(
                    self.selected_hmonitor != 0,
                    theme::secondary_button(i18n::t_test_capture_short(language)),
                )
                .clicked()
            {
                self.test_capture_requested = true;
            }

            if !self.status_text.is_empty() {
                ui.add_space(6.0);
                ui.label(theme::muted(self.status_text.clone(), 11.5));
            }
        });
    }
}
