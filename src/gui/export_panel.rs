use crate::gui::output_panel::OutputPanel;
use crate::gui::theme::{self, token};
use crate::i18n::{self, Language};
use egui::*;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum ExportMessage {
    Progress(usize, usize),
    Finished(Result<Vec<PathBuf>, String>),
}

pub struct ExportPanel {
    pub settings: OutputPanel,
    pub source_dir: String,
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
            source_dir: String::new(),
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
                    self.choose_folder();
                }
            });
            ui.label(theme::muted(i18n::t_export_source_hint(language), 11.5));
        });

        ui.add_space(14.0);

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
                        self.settings.output_filename = format!(
                            "ppt-capture-{}.pptx",
                            chrono::Local::now().format("%Y%m%d-%H%M%S")
                        );
                    }
                });
            });
            ui.add_space(8.0);

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

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            let ready = self.export.is_none() && !self.source_dir.trim().is_empty();
            if ui
                .add_enabled(
                    ready,
                    theme::primary_button(i18n::t_export_generate(language)),
                )
                .clicked()
            {
                self.start(language);
            }
            if self.export.is_none() && self.source_dir.trim().is_empty() {
                ui.label(theme::muted(i18n::t_export_ready_hint(language), 12.0));
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

        if !self.status.is_empty() {
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
    }

    fn start(&mut self, language: Language) {
        if self.export.is_some() {
            return;
        }
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
                .and_then(|result| result.map_err(|error| format!("{error:#}")));
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
                    self.source_dir = path.to_string_lossy().into_owned();
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
                        self.status = if language == Language::Chinese {
                            if done == total {
                                format!("已处理 {done}/{total}，正在生成 PPTX…")
                            } else {
                                format!("正在处理图片 {done}/{total}…")
                            }
                        } else if done == total {
                            format!("Processed {done}/{total}; generating PPTX…")
                        } else {
                            format!("Processing images {done}/{total}…")
                        };
                    }
                    ExportMessage::Finished(result) => {
                        match result {
                            Ok(paths) => {
                                self.output_to_open = paths
                                    .first()
                                    .and_then(|path| path.parent())
                                    .map(PathBuf::from);
                                self.status = if language == Language::Chinese {
                                    format!("生成完成：{} 个 PPTX", paths.len())
                                } else {
                                    format!("Generated {} PPTX file(s)", paths.len())
                                };
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

    fn choose_folder(&mut self) {
        let initial = self.initial_folder().map(|path| {
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
                let _ = tx.send(pick_folder(initial.as_deref()));
            });
        match spawned {
            Ok(_) => self.picker = Some(rx),
            Err(error) => self.status = format!("Failed to open folder picker: {error}"),
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
        || status.contains("失败")
        || status.contains("中断")
}

fn pick_folder(initial: Option<&std::path::Path>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("powershell.exe").env("PPT_CAPTURE_INITIAL_FOLDER", initial.unwrap_or_else(|| std::path::Path::new(""))).args(["-NoProfile", "-STA", "-Command", "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.SelectedPath = $env:PPT_CAPTURE_INITIAL_FOLDER; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::Write($d.SelectedPath) }"]).output();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("osascript")
        .args([
            "-e",
            "on run argv\nif (item 1 of argv) is not \"\" then\nreturn POSIX path of (choose folder with prompt \"Select an image folder\" default location (POSIX file (item 1 of argv)))\nelse\nreturn POSIX path of (choose folder with prompt \"Select an image folder\")\nend if\nend run",
            "--",
        ])
        .arg(initial.unwrap_or_else(|| std::path::Path::new("")))
        .output();
    #[cfg(target_os = "linux")]
    let result = {
        let mut command = std::process::Command::new("zenity");
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
    let path =
        String::from_utf8(result.stdout).map_err(|_| "Folder path is not UTF-8".to_string())?;
    let path = path
        .trim_start_matches('\u{feff}')
        .trim_end_matches(['\r', '\n']);
    Ok(if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    })
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
    fn status_banner_classifies_errors() {
        assert!(status_is_error("Export failed: disk full"));
        assert!(status_is_error("生成失败：磁盘已满"));
        assert!(!status_is_error("Generated 2 PPTX file(s)"));
    }
}
