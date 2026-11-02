use crate::gui::output_panel::OutputPanel;
use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use crate::pptx::{crop_deck_images, CropPlacement, DeckCropSummary};
use egui::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Which input the page converts: a folder of screenshots, or a deck that
/// already contains them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportMode {
    Images,
    Deck,
}

enum ExportMessage {
    Progress(usize, usize),
    Finished(Result<ExportOutcome, String>),
}

struct ExportOutcome {
    paths: Vec<PathBuf>,
    deck: Option<DeckCropSummary>,
}

pub struct ExportPanel {
    pub settings: OutputPanel,
    pub mode: ExportMode,
    pub source_dir: String,
    pub source_pptx: String,
    pub placement: CropPlacement,
    pub status: String,
    pub output_to_open: Option<PathBuf>,
    open_requested: bool,
    export: Option<Receiver<ExportMessage>>,
    picker: Option<Receiver<Result<Option<PathBuf>, String>>>,
    pub recording_root: PathBuf,
    pub latest_slides: Option<PathBuf>,
}

impl ExportPanel {
    pub fn is_busy(&self) -> bool {
        self.export.is_some()
    }

    pub fn notify_close_blocked(&mut self, language: Language) {
        self.status = if language == Language::Chinese {
            "正在生成 PPTX，请等待完成后关闭。"
        } else {
            "PPTX generation is running. Wait for it to finish before closing."
        }
        .into();
    }

    pub fn new(settings: &OutputPanel) -> Self {
        Self {
            settings: settings.clone(),
            mode: ExportMode::Images,
            source_dir: String::new(),
            source_pptx: String::new(),
            placement: CropPlacement::FillArea,
            status: String::new(),
            output_to_open: None,
            open_requested: false,
            export: None,
            picker: None,
            recording_root: PathBuf::from(&settings.output_dir),
            latest_slides: None,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, language: Language) {
        self.poll(language);

        theme::page_header(
            ui,
            i18n::t_export_page_title(language),
            i18n::t_export_page_subtitle(language),
        );

        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_export_mode_card(language));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (mode, label) in [
                    (ExportMode::Images, i18n::t_export_mode_images(language)),
                    (ExportMode::Deck, i18n::t_export_mode_deck(language)),
                ] {
                    let button = theme::segment_button(label, self.mode == mode);
                    if ui.add_enabled(self.export.is_none(), button).clicked() {
                        self.switch_mode(mode);
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(theme::muted(i18n::t_export_mode_hint(language), 11.5));
        });

        ui.add_space(14.0);

        match self.mode {
            ExportMode::Images => {
                self.images_source_card(ui, language);
                ui.add_space(14.0);
                self.deck_settings_card(ui, language, false);
            }
            ExportMode::Deck => {
                self.pptx_source_card(ui, language);
                ui.add_space(14.0);
                self.deck_settings_card(ui, language, true);
            }
        }

        ui.add_space(16.0);
        self.actions(ui, language);
        self.status_banner(ui, language);
    }

    fn switch_mode(&mut self, mode: ExportMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.status.clear();
        self.output_to_open = None;
    }

    fn images_source_card(&mut self, ui: &mut egui::Ui, language: Language) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_export_source_card(language));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 108.0).max(160.0);
                ui.add(
                    TextEdit::singleline(&mut self.source_dir)
                        .desired_width(width)
                        .hint_text(match language {
                            Language::Chinese => "选择包含截图的文件夹",
                            Language::English => "Choose a folder of slides",
                        }),
                );
                if ui
                    .add_enabled(
                        self.picker.is_none(),
                        theme::secondary_button(i18n::t_export_browse(language)),
                    )
                    .clicked()
                {
                    self.choose_path(false);
                }
            });
            ui.label(theme::muted(i18n::t_export_source_hint(language), 11.5));
        });
    }

    fn pptx_source_card(&mut self, ui: &mut egui::Ui, language: Language) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_export_pptx_source_card(language));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 130.0).max(160.0);
                ui.add(
                    TextEdit::singleline(&mut self.source_pptx)
                        .desired_width(width)
                        .hint_text(match language {
                            Language::Chinese => "选择一个 PPTX 文件",
                            Language::English => "Choose a PPTX file",
                        }),
                );
                if ui
                    .add_enabled(
                        self.picker.is_none(),
                        theme::secondary_button(i18n::t_export_choose_pptx(language)),
                    )
                    .clicked()
                {
                    self.choose_path(true);
                }
            });
            ui.label(theme::muted(
                i18n::t_export_pptx_source_hint(language),
                11.5,
            ));
        });
    }

    fn deck_settings_card(&mut self, ui: &mut egui::Ui, language: Language, cropped: bool) {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            theme::section_title(ui, i18n::t_export_deck_card(language));
            ui.add_space(4.0);
            let wide = ui.available_width() >= 560.0;

            field_row(ui, wide, i18n::t_export_output_folder(language), |ui| {
                let width = ui.available_width().min(430.0);
                ui.add(TextEdit::singleline(&mut self.settings.output_dir).desired_width(width));
            });
            ui.add_space(6.0);

            field_row(ui, wide, i18n::t_pptx_filename(language), |ui| {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 112.0).clamp(140.0, 380.0);
                    if ui
                        .add(
                            TextEdit::singleline(&mut self.settings.output_filename)
                                .desired_width(width),
                        )
                        .changed()
                    {
                        self.settings.filename_is_custom = true;
                    }
                    if ui
                        .add(theme::secondary_button(i18n::t_automatic_name(language)))
                        .clicked()
                    {
                        self.settings.filename_is_custom = false;
                        self.settings.output_filename = if cropped {
                            self.cropped_filename()
                        } else {
                            format!(
                                "ppt-capture-{}.pptx",
                                chrono::Local::now().format("%Y%m%d-%H%M%S")
                            )
                        };
                    }
                });
            });
            ui.add_space(8.0);

            if cropped {
                theme::divider(ui);
                let wide = ui.available_width() >= 560.0;
                field_row(ui, wide, i18n::t_export_placement(language), |ui| {
                    ui.vertical(|ui| {
                        placement_option(
                            ui,
                            &mut self.placement,
                            CropPlacement::FillArea,
                            i18n::t_placement_fill(language),
                            i18n::t_placement_fill_hint(language),
                        );
                        ui.add_space(4.0);
                        placement_option(
                            ui,
                            &mut self.placement,
                            CropPlacement::KeepScale,
                            i18n::t_placement_keep(language),
                            i18n::t_placement_keep_hint(language),
                        );
                    });
                });
                ui.add_space(6.0);
                ui.label(theme::muted(i18n::t_export_pptx_crop_hint(language), 11.5));
                return;
            }

            let ratio = i18n::t_slide_aspect(language)
                .trim_end_matches([':', '：'])
                .to_string();
            let fit = i18n::t_image_fit(language)
                .trim_end_matches([':', '：'])
                .to_string();
            let fit_options = [
                ("fit", i18n::t_fit_fit(language)),
                ("fill", i18n::t_fit_fill(language)),
            ];
            if wide {
                ui.columns(2, |columns| {
                    combo(
                        &mut columns[0],
                        &ratio,
                        "export_ratio",
                        &mut self.settings.page_ratio,
                        &[
                            ("16:9", "16:9"),
                            ("4:3", "4:3"),
                            ("3:2", "3:2"),
                            ("16:10", "16:10"),
                        ],
                    );
                    combo(
                        &mut columns[1],
                        &fit,
                        "export_fit",
                        &mut self.settings.image_fit,
                        &fit_options,
                    );
                });
            } else {
                combo(
                    ui,
                    &ratio,
                    "export_ratio",
                    &mut self.settings.page_ratio,
                    &[
                        ("16:9", "16:9"),
                        ("4:3", "4:3"),
                        ("3:2", "3:2"),
                        ("16:10", "16:10"),
                    ],
                );
                ui.add_space(6.0);
                combo(
                    ui,
                    &fit,
                    "export_fit",
                    &mut self.settings.image_fit,
                    &fit_options,
                );
            }

            theme::divider(ui);
            ui.checkbox(
                &mut self.settings.auto_crop,
                i18n::t_export_crop_label(language),
            );
            ui.label(theme::muted(i18n::t_export_crop_hint(language), 11.5));
        });
    }

    fn actions(&mut self, ui: &mut egui::Ui, language: Language) {
        ui.horizontal(|ui| {
            let ready = self.export.is_none()
                && match self.mode {
                    ExportMode::Images => !self.source_dir.trim().is_empty(),
                    ExportMode::Deck => {
                        !self.source_pptx.trim().is_empty()
                            && Path::new(self.source_pptx.trim()).is_file()
                    }
                };
            let label = match self.mode {
                ExportMode::Images => i18n::t_export_generate(language),
                ExportMode::Deck => i18n::t_export_crop_generate(language),
            };
            if ui
                .add_enabled(ready, theme::primary_button(label))
                .clicked()
            {
                self.start(language);
            }
            if self.export.is_none() {
                let missing = match self.mode {
                    ExportMode::Images => self.source_dir.trim().is_empty(),
                    ExportMode::Deck => self.source_pptx.trim().is_empty(),
                };
                if missing {
                    ui.label(theme::muted(
                        match self.mode {
                            ExportMode::Images => i18n::t_export_ready_hint(language),
                            ExportMode::Deck => i18n::t_export_pptx_ready_hint(language),
                        },
                        12.0,
                    ));
                } else if self.mode == ExportMode::Deck
                    && !Path::new(self.source_pptx.trim()).is_file()
                {
                    ui.label(theme::muted(i18n::t_export_pptx_missing(language), 12.0));
                }
            }
            if self.export.is_some() {
                ui.label(theme::muted(
                    match language {
                        Language::Chinese => "生成在后台进行，可以继续切换页面。",
                        Language::English => {
                            "Generation runs in the background; you can keep browsing."
                        }
                    },
                    12.0,
                ));
            }
        });
    }

    fn status_banner(&mut self, ui: &mut egui::Ui, language: Language) {
        if self.status.is_empty() {
            return;
        }
        ui.add_space(12.0);
        let error = status_is_error(&self.status);
        let success = !error && self.export.is_none() && self.output_to_open.is_some();
        let (fill, stroke, text) = if error {
            (token::WARN_SOFT, token::WARN_LINE, token::WARN)
        } else if success {
            (token::OK_SOFT, token::LINE, token::OK)
        } else {
            (token::PRIMARY_SOFT, token::LINE, token::PRIMARY)
        };
        theme::banner_frame(fill, stroke).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).size(12.5).color(text));
                if self.output_to_open.is_some()
                    && ui
                        .with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(theme::quiet_button(i18n::t_export_open_folder(language)))
                        })
                        .inner
                        .clicked()
                {
                    // The application consumes this by calling take_output_to_open after rendering.
                    self.open_requested = true;
                }
            });
        });
    }

    fn cropped_filename(&self) -> String {
        let stem = Path::new(self.source_pptx.trim())
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        if stem.is_empty() {
            return "pptx-cropped.pptx".into();
        }
        format!("{stem}-cropped.pptx")
    }

    fn start(&mut self, language: Language) {
        if self.export.is_some() {
            return;
        }
        match self.mode {
            ExportMode::Images => self.start_images(language),
            ExportMode::Deck => self.start_deck(language),
        }
    }

    fn start_deck(&mut self, language: Language) {
        let source = PathBuf::from(self.source_pptx.trim());
        if !source.is_file() {
            self.status = i18n::t_export_pptx_missing(language).to_string();
            return;
        }
        let destination = PathBuf::from(self.settings.output_dir.trim());
        let filename = if self.settings.filename_is_custom {
            self.settings.output_filename.clone()
        } else {
            self.cropped_filename()
        };
        self.settings.output_filename = filename.clone();
        let placement = self.placement;
        let (tx, rx) = mpsc::sync_channel(32);
        let spawned = std::thread::Builder::new()
            .name("pptx-crop".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crop_deck_images(
                        &source,
                        &destination,
                        &filename,
                        placement,
                        |done, total| {
                            if done == total {
                                let _ = tx.send(ExportMessage::Progress(done, total));
                            } else {
                                let _ = tx.try_send(ExportMessage::Progress(done, total));
                            }
                        },
                    )
                }))
                .map_err(|_| "PPTX cropping stopped unexpectedly".to_string())
                .and_then(|result| result.map_err(|error| format!("{error:#}")))
                .map(|summary| ExportOutcome {
                    paths: vec![summary.output.clone()],
                    deck: Some(summary),
                });
                let _ = tx.send(ExportMessage::Finished(result));
            });
        match spawned {
            Ok(_) => {
                self.export = Some(rx);
                self.output_to_open = None;
                self.status = if language == Language::Chinese {
                    "正在裁剪截图边缘…"
                } else {
                    "Trimming screenshot edges…"
                }
                .into();
            }
            Err(error) => self.status = format!("Failed to start cropping: {error}"),
        }
    }

    fn start_images(&mut self, language: Language) {
        let source = PathBuf::from(self.source_dir.trim());
        let destination = PathBuf::from(self.settings.output_dir.trim());
        let filename = if self.settings.filename_is_custom {
            self.settings.output_filename.clone()
        } else {
            crate::config::filename_for_new_operation(&self.settings.output_filename)
        };
        self.settings.output_filename = filename.clone();
        let ratio = self.settings.page_ratio.clone();
        let fit = self.settings.image_fit.clone();
        let auto_crop = self.settings.auto_crop;
        let (tx, rx) = mpsc::sync_channel(32);
        let spawned = std::thread::Builder::new()
            .name("pptx-import".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::storage::import_directory(
                        &source,
                        &destination,
                        &filename,
                        &ratio,
                        &fit,
                        auto_crop,
                        |done, total| {
                            if done == total {
                                let _ = tx.send(ExportMessage::Progress(done, total));
                            } else {
                                let _ = tx.try_send(ExportMessage::Progress(done, total));
                            }
                        },
                    )
                }))
                .map_err(|_| "PPTX generation stopped unexpectedly".to_string())
                .and_then(|result| result.map_err(|error| format!("{error:#}")))
                .map(|paths| ExportOutcome { paths, deck: None });
                let _ = tx.send(ExportMessage::Finished(result));
            });
        match spawned {
            Ok(_) => {
                self.export = Some(rx);
                self.output_to_open = None;
                self.status = if language == Language::Chinese {
                    "正在处理图片…"
                } else {
                    "Processing images…"
                }
                .into();
            }
            Err(error) => self.status = format!("Failed to start export: {error}"),
        }
    }

    pub fn take_open_request(&mut self) -> Option<PathBuf> {
        if std::mem::take(&mut self.open_requested) {
            self.output_to_open.clone()
        } else {
            None
        }
    }

    pub fn poll(&mut self, language: Language) {
        if let Some(rx) = &self.picker {
            match rx.try_recv() {
                Ok(Ok(Some(path))) => {
                    if self.mode == ExportMode::Deck {
                        self.source_pptx = path.to_string_lossy().into_owned();
                        if !self.settings.filename_is_custom {
                            self.settings.output_filename = self.cropped_filename();
                        }
                    } else {
                        self.source_dir = path.to_string_lossy().into_owned();
                    }
                    self.picker = None;
                }
                Ok(Ok(None)) => self.picker = None,
                Ok(Err(error)) => {
                    self.status = error;
                    self.picker = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.status = "Folder selector stopped unexpectedly".into();
                    self.picker = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.export {
            loop {
                let message = match rx.try_recv() {
                    Ok(message) => message,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.export = None;
                        self.status = if language == Language::Chinese {
                            "生成任务意外中断，请重试。"
                        } else {
                            "PPTX generation stopped unexpectedly; please retry."
                        }
                        .into();
                        break;
                    }
                };
                match message {
                    ExportMessage::Progress(done, total) => {
                        let trimming = self.mode == ExportMode::Deck;
                        self.status = if language == Language::Chinese {
                            if done == total {
                                if trimming {
                                    format!("已分析 {done}/{total} 张幻灯片，正在写出新 PPTX…")
                                } else {
                                    format!("已处理 {done}/{total}，正在生成 PPTX…")
                                }
                            } else if trimming {
                                format!("正在分析幻灯片 {done}/{total}…")
                            } else {
                                format!("正在处理图片 {done}/{total}…")
                            }
                        } else if done == total {
                            if trimming {
                                format!("Analysed {done}/{total} slides; writing the new PPTX…")
                            } else {
                                format!("Processed {done}/{total}; generating PPTX…")
                            }
                        } else if trimming {
                            format!("Analysing slides {done}/{total}…")
                        } else {
                            format!("Processing images {done}/{total}…")
                        };
                    }
                    ExportMessage::Finished(result) => {
                        match result {
                            Ok(outcome) => {
                                self.output_to_open = outcome
                                    .paths
                                    .first()
                                    .and_then(|path| path.parent())
                                    .map(PathBuf::from);
                                self.status = describe_outcome(&outcome, language);
                            }
                            Err(error) => {
                                self.status = if language == Language::Chinese {
                                    format!("生成失败：{error}")
                                } else {
                                    format!("Export failed: {error}")
                                }
                            }
                        }
                        self.export = None;
                        break;
                    }
                }
            }
        }
    }

    fn choose_path(&mut self, file: bool) {
        let initial = if file {
            Path::new(self.source_pptx.trim())
                .parent()
                .map(Path::to_path_buf)
        } else {
            self.initial_folder()
        }
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(&path))
                    .unwrap_or(path)
            }
        });
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("folder-picker".into())
            .spawn(move || {
                let _ = tx.send(if file {
                    pick_file(initial.as_deref())
                } else {
                    pick_folder(initial.as_deref())
                });
            });
        match spawned {
            Ok(_) => self.picker = Some(rx),
            Err(error) => {
                self.status = if file {
                    format!("Failed to open file picker: {error}")
                } else {
                    format!("Failed to open folder picker: {error}")
                }
            }
        }
    }

    fn initial_folder(&self) -> Option<PathBuf> {
        let selected = PathBuf::from(self.source_dir.trim());
        if !self.source_dir.trim().is_empty() && selected.is_dir() {
            return Some(selected);
        }
        if let Some(path) = &self.latest_slides {
            if path.starts_with(self.recording_root.join("sessions")) && path.is_dir() {
                return Some(path.clone());
            }
        }
        let mut sessions: Vec<_> = std::fs::read_dir(self.recording_root.join("sessions"))
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join("slides").is_dir())
            .collect();
        sessions.sort_by_cached_key(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let timestamp = name.chars().take(15).collect::<String>();
            let created = path
                .metadata()
                .ok()
                .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()).ok());
            (timestamp, created, path.clone())
        });
        sessions.pop().map(|path| path.join("slides"))
    }
}

fn describe_outcome(outcome: &ExportOutcome, language: Language) -> String {
    match &outcome.deck {
        Some(summary) => {
            if language == Language::Chinese {
                if summary.cropped == 0 {
                    format!(
                        "已生成新文件：{} 张幻灯片都不需要裁剪，内容与原件相同。",
                        summary.slides
                    )
                } else {
                    format!(
                        "裁剪完成：共 {} 张幻灯片，裁剪了 {} 张截图。",
                        summary.slides, summary.cropped
                    )
                }
            } else if summary.cropped == 0 {
                format!(
                    "Generated the deck: none of the {} slides needed trimming, so it matches the original.",
                    summary.slides
                )
            } else {
                format!("Trimmed {} of {} slides.", summary.cropped, summary.slides)
            }
        }
        None => {
            if language == Language::Chinese {
                format!("生成完成：{} 个 PPTX", outcome.paths.len())
            } else {
                format!("Generated {} PPTX file(s)", outcome.paths.len())
            }
        }
    }
}

fn placement_option(
    ui: &mut Ui,
    current: &mut CropPlacement,
    value: CropPlacement,
    label: &str,
    hint: &str,
) {
    ui.radio_value(
        current,
        value,
        RichText::new(label).size(12.5).color(token::INK).strong(),
    );
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        ui.label(theme::muted(hint, 11.5));
    });
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

fn combo(ui: &mut Ui, label: &str, id: &str, value: &mut String, options: &[(&str, &str)]) {
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

fn status_is_error(status: &str) -> bool {
    let lower = status.to_lowercase();
    lower.contains("failed")
        || lower.contains("cannot")
        || lower.contains("unexpectedly")
        || lower.contains("stopped")
        || status.contains("失败")
        || status.contains("中断")
}

/// The Windows choosers print the chosen path as the hex of its UTF-8 bytes.
///
/// Plain text would travel through the console code page, which turns any path
/// that is not ASCII into question marks, and the usual workaround — setting
/// `[Console]::OutputEncoding` — is exactly what a helper without a console
/// cannot do. Hex is ASCII whatever the code page is.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_FOLDER_CHOOSER: &str = "Add-Type -AssemblyName System.Windows.Forms; \
$d = New-Object System.Windows.Forms.FolderBrowserDialog; \
$d.SelectedPath = $env:PPT_CAPTURE_INITIAL_FOLDER; \
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
$b = [System.Text.Encoding]::UTF8.GetBytes($d.SelectedPath); \
([System.BitConverter]::ToString($b)).Replace('-', '') }";

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_FILE_CHOOSER: &str = "Add-Type -AssemblyName System.Windows.Forms; \
$d = New-Object System.Windows.Forms.OpenFileDialog; \
$d.Filter = 'PowerPoint presentation (*.pptx)|*.pptx'; \
$d.InitialDirectory = $env:PPT_CAPTURE_INITIAL_FOLDER; \
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
$b = [System.Text.Encoding]::UTF8.GetBytes($d.FileName); \
([System.BitConverter]::ToString($b)).Replace('-', '') }";

/// What the platform chooser printed, as a path. An empty answer means the user
/// cancelled the dialog.
fn chooser_path(stdout: &[u8]) -> Result<Option<PathBuf>, String> {
    let raw = std::str::from_utf8(stdout)
        .map_err(|_| "The chooser returned text that is not UTF-8".to_string())?;
    let text = decode_chooser_output(raw.trim())?;
    let text = text
        .trim_start_matches('\u{feff}')
        .trim_end_matches(['\r', '\n']);
    Ok(if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    })
}

#[cfg(target_os = "windows")]
fn decode_chooser_output(raw: &str) -> Result<String, String> {
    decode_hex_text(raw)
}

#[cfg(not(target_os = "windows"))]
fn decode_chooser_output(raw: &str) -> Result<String, String> {
    Ok(raw.to_string())
}

/// Decode the hex of a UTF-8 string. Platform-neutral so the decoding is covered
/// by the tests that run on every platform.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn decode_hex_text(raw: &str) -> Result<String, String> {
    if !raw.len().is_multiple_of(2) {
        return Err("The chooser returned an odd number of hex digits".into());
    }
    let mut bytes = Vec::with_capacity(raw.len() / 2);
    for pair in raw.as_bytes().chunks(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| "The chooser returned text that is not hex".to_string())?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| "The chooser returned text that is not hex".to_string())?;
        bytes.push((high * 16 + low) as u8);
    }
    String::from_utf8(bytes).map_err(|_| "The chosen path is not UTF-8".to_string())
}

fn pick_folder(initial: Option<&std::path::Path>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = crate::platform::quiet_command("powershell.exe")
        .env(
            "PPT_CAPTURE_INITIAL_FOLDER",
            initial.unwrap_or_else(|| std::path::Path::new("")),
        )
        .args(["-NoProfile", "-STA", "-Command", WINDOWS_FOLDER_CHOOSER])
        .output();
    #[cfg(target_os = "macos")]
    let result = crate::platform::quiet_command("osascript")
        .args([
            "-e",
            "on run argv\nif (item 1 of argv) is not \"\" then\nreturn POSIX path of (choose folder with prompt \"Select an image folder\" default location (POSIX file (item 1 of argv)))\nelse\nreturn POSIX path of (choose folder with prompt \"Select an image folder\")\nend if\nend run",
            "--",
        ])
        .arg(initial.unwrap_or_else(|| std::path::Path::new("")))
        .output();
    #[cfg(target_os = "linux")]
    let result = {
        let mut command = crate::platform::quiet_command("zenity");
        command.args(["--file-selection", "--directory"]);
        if let Some(path) = initial {
            command.arg(format!("--filename={}/", path.display()));
        }
        command.output()
    };
    let result = result.map_err(|error| {
        format!("Cannot open folder chooser ({error}); enter the folder path manually")
    })?;
    if !result.status.success() {
        return Ok(None);
    }
    chooser_path(&result.stdout)
}

fn pick_file(initial: Option<&std::path::Path>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = crate::platform::quiet_command("powershell.exe")
        .env(
            "PPT_CAPTURE_INITIAL_FOLDER",
            initial.unwrap_or_else(|| std::path::Path::new("")),
        )
        .args(["-NoProfile", "-STA", "-Command", WINDOWS_FILE_CHOOSER])
        .output();
    #[cfg(target_os = "macos")]
    let result = crate::platform::quiet_command("osascript")
        .args([
            "-e",
            "on run argv\nif (item 1 of argv) is not \"\" then\nreturn POSIX path of (choose file with prompt \"Select a PPTX\" default location (POSIX file (item 1 of argv)) of type {\"org.openxmlformats.presentationml.presentation\"})\nelse\nreturn POSIX path of (choose file with prompt \"Select a PPTX\" of type {\"org.openxmlformats.presentationml.presentation\"})\nend if\nend run",
            "--",
        ])
        .arg(initial.unwrap_or_else(|| std::path::Path::new("")))
        .output();
    #[cfg(target_os = "linux")]
    let result = {
        let mut command = crate::platform::quiet_command("zenity");
        command.args([
            "--file-selection",
            "--file-filter=PowerPoint (*.pptx) | *.pptx",
        ]);
        if let Some(path) = initial {
            command.arg(format!("--filename={}/", path.display()));
        }
        command.output()
    };
    let result = result
        .map_err(|error| format!("Cannot open file chooser ({error}); enter the path manually"))?;
    if !result.status.success() {
        return Ok(None);
    }
    chooser_path(&result.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_export_unblocks_the_panel() {
        let mut panel = ExportPanel::new(&OutputPanel::new_with_filename("slides.pptx"));
        let (tx, rx) = mpsc::channel();
        panel.export = Some(rx);
        drop(tx);
        panel.poll(Language::English);
        assert!(!panel.is_busy());
        assert!(panel.status.contains("unexpectedly"));
    }

    #[test]
    fn picker_defaults_to_latest_session_and_respects_explicit_folder() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("sessions/20260101-100000-old/slides");
        let latest = root.path().join("sessions/20260201-100000-new/slides");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&latest).unwrap();
        let mut settings = OutputPanel::new_with_filename("deck.pptx");
        settings.output_dir = root.path().display().to_string();
        let mut panel = ExportPanel::new(&settings);
        assert_eq!(panel.initial_folder(), Some(latest));
        panel.source_dir = old.display().to_string();
        assert_eq!(panel.initial_folder(), Some(old));
    }

    #[test]
    fn changing_recording_root_does_not_use_a_cached_session_from_old_root() {
        let old_root = tempfile::tempdir().unwrap();
        let new_root = tempfile::tempdir().unwrap();
        let old = old_root.path().join("sessions/20260101-100000-old/slides");
        let new = new_root.path().join("sessions/20260201-100000-new/slides");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        let mut panel = ExportPanel::new(&OutputPanel::new_with_filename("deck.pptx"));
        panel.latest_slides = Some(old);
        panel.recording_root = new_root.path().into();
        assert_eq!(panel.initial_folder(), Some(new));
    }

    #[test]
    fn picker_selects_newest_session_when_multiple_start_in_same_second() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("sessions/20260101-100000-zzzz/slides");
        let new = root.path().join("sessions/20260101-100000-aaaa/slides");
        std::fs::create_dir_all(&old).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(&new).unwrap();
        let mut settings = OutputPanel::new_with_filename("deck.pptx");
        settings.output_dir = root.path().display().to_string();
        let panel = ExportPanel::new(&settings);
        assert_eq!(panel.initial_folder(), Some(new));
    }

    #[test]
    fn directory_export_runs_in_background_and_reports_its_output_folder() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0]))
            .save(source.path().join("1.png"))
            .unwrap();
        let mut settings = OutputPanel::new_with_filename("slides.pptx");
        settings.output_dir = output.path().to_string_lossy().into_owned();
        let mut panel = ExportPanel::new(&settings);
        panel.source_dir = source.path().to_string_lossy().into_owned();
        panel.start(Language::English);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while panel.export.is_some() && std::time::Instant::now() < deadline {
            panel.poll(Language::English);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(panel.export.is_none(), "background export did not finish");
        assert!(
            panel.status.contains("Generated 1 PPTX"),
            "{}",
            panel.status
        );
        assert!(panel.output_to_open.unwrap().join("slides.pptx").is_file());
    }

    #[test]
    fn export_refreshes_automatic_filename_when_started() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0]))
            .save(source.path().join("1.png"))
            .unwrap();
        let mut settings = OutputPanel::new_with_filename("ppt-capture-20000101-000000.pptx");
        settings.output_dir = output.path().display().to_string();
        let mut panel = ExportPanel::new(&settings);
        panel.source_dir = source.path().display().to_string();
        panel.start(Language::English);
        assert_ne!(
            panel.settings.output_filename,
            "ppt-capture-20000101-000000.pptx"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while panel.is_busy() && std::time::Instant::now() < deadline {
            panel.poll(Language::English);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!panel.is_busy());
        assert!(panel
            .output_to_open
            .unwrap()
            .join(panel.settings.output_filename)
            .is_file());
    }

    #[test]
    fn the_windows_chooser_scripts_are_single_line_commands() {
        for script in [WINDOWS_FOLDER_CHOOSER, WINDOWS_FILE_CHOOSER] {
            // The literal continues over several source lines; what PowerShell
            // receives has to be one command with every statement separated.
            assert!(!script.contains('\n'), "{script}");
            assert!(!script.contains('\\'), "{script}");
            assert!(
                script.contains("[System.Windows.Forms.DialogResult]::OK"),
                "{script}"
            );
            assert!(script.contains("Replace('-', '')"), "{script}");
            assert!(
                script.contains("$env:PPT_CAPTURE_INITIAL_FOLDER"),
                "{script}"
            );
        }
    }

    #[test]
    fn a_chinese_path_survives_the_hex_the_windows_chooser_prints() {
        // What [BitConverter]::ToString produces for the UTF-8 bytes of
        // "C:\数据\幻灯片.pptx", minus the dashes.
        let hex = "433A5CE695B0E68DAE5CE5B9BBE781AFE789872E70707478";
        assert_eq!(decode_hex_text(hex).unwrap(), r"C:\数据\幻灯片.pptx");
    }

    #[test]
    fn malformed_hex_is_rejected_instead_of_becoming_a_path() {
        assert!(decode_hex_text("ABC").is_err(), "odd number of digits");
        assert!(decode_hex_text("ZZ").is_err(), "not hex");
        assert!(decode_hex_text("FFFE").is_err(), "not UTF-8");
        assert_eq!(decode_hex_text("").unwrap(), "");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_plain_path_from_the_native_chooser_is_taken_as_is() {
        assert_eq!(
            chooser_path(b"/Users/allen/\xe8\xb5\x84\xe6\x96\x99/\n").unwrap(),
            Some(PathBuf::from("/Users/allen/资料"))
        );
    }

    #[test]
    fn a_cancelled_chooser_returns_no_path() {
        assert_eq!(chooser_path(b"").unwrap(), None);
        assert_eq!(chooser_path(b"\r\n").unwrap(), None);
    }

    #[test]
    fn status_banner_classifies_errors() {
        assert!(status_is_error("Export failed: disk full"));
        assert!(status_is_error("生成失败：磁盘已满"));
        assert!(!status_is_error("Generated 2 PPTX file(s)"));
    }

    #[test]
    fn cropped_filename_follows_the_source_deck() {
        let mut panel = ExportPanel::new(&OutputPanel::new_with_filename("deck.pptx"));
        panel.source_pptx = "/tmp/harmony-review.pptx".into();
        assert_eq!(panel.cropped_filename(), "harmony-review-cropped.pptx");
        panel.source_pptx = String::new();
        assert_eq!(panel.cropped_filename(), "pptx-cropped.pptx");
    }

    #[test]
    fn deck_mode_requires_an_existing_file_before_starting() {
        let output = tempfile::tempdir().unwrap();
        let mut settings = OutputPanel::new_with_filename("deck.pptx");
        settings.output_dir = output.path().display().to_string();
        let mut panel = ExportPanel::new(&settings);
        panel.mode = ExportMode::Deck;
        panel.source_pptx = output.path().join("missing.pptx").display().to_string();
        panel.start(Language::English);
        assert!(panel.export.is_none());
        assert!(panel.status.contains("not found"), "{}", panel.status);
    }

    #[test]
    fn switching_modes_clears_a_stale_result_banner() {
        let mut panel = ExportPanel::new(&OutputPanel::new_with_filename("deck.pptx"));
        panel.status = "Generated 1 PPTX file(s)".into();
        panel.output_to_open = Some(PathBuf::from("/tmp"));
        panel.switch_mode(ExportMode::Deck);
        assert!(panel.status.is_empty());
        assert!(panel.output_to_open.is_none());
    }
}
