use crate::gui::output_panel::OutputPanel;
use crate::i18n::Language;
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
        let zh = language == Language::Chinese;
        ui.heading(if zh {
            "图片生成 PPTX"
        } else {
            "Create PPTX from images"
        });
        ui.label(if zh {
            "选择图片文件夹：按文件名中的数字顺序导入 PNG/JPEG，每 50 张生成一个 PPTX。"
        } else {
            "Choose a folder: PNG/JPEG files are imported in natural filename order, 50 per PPTX."
        });
        ui.horizontal(|ui| {
            ui.label(if zh {
                "图片文件夹"
            } else {
                "Image folder"
            });
            ui.text_edit_singleline(&mut self.source_dir);
            if ui
                .add_enabled(
                    self.picker.is_none(),
                    egui::Button::new(if zh { "选择…" } else { "Browse…" }),
                )
                .clicked()
            {
                self.choose_folder();
            }
        });
        ui.horizontal(|ui| {
            ui.label(if zh {
                "输出文件夹"
            } else {
                "Output folder"
            });
            ui.text_edit_singleline(&mut self.settings.output_dir);
        });
        ui.horizontal(|ui| {
            ui.label(if zh {
                "PPTX 文件名"
            } else {
                "PPTX filename"
            });
            if ui
                .text_edit_singleline(&mut self.settings.output_filename)
                .changed()
            {
                self.settings.filename_is_custom = true;
            }
            if ui
                .button(if zh { "自动命名" } else { "Automatic" })
                .clicked()
            {
                self.settings.filename_is_custom = false;
                self.settings.output_filename = format!(
                    "ppt-capture-{}.pptx",
                    chrono::Local::now().format("%Y%m%d-%H%M%S")
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(if zh { "页面比例" } else { "Slide ratio" });
            egui::ComboBox::from_id_salt("export_ratio")
                .selected_text(&self.settings.page_ratio)
                .show_ui(ui, |ui| {
                    for ratio in ["16:9", "4:3", "3:2", "16:10"] {
                        ui.selectable_value(
                            &mut self.settings.page_ratio,
                            ratio.to_string(),
                            ratio,
                        );
                    }
                });
            ui.label(if zh { "图片适配" } else { "Image fit" });
            egui::ComboBox::from_id_salt("export_fit")
                .selected_text(&self.settings.image_fit)
                .show_ui(ui, |ui| {
                    for fit in ["fit", "fill"] {
                        ui.selectable_value(&mut self.settings.image_fit, fit.to_string(), fit);
                    }
                });
        });
        ui.checkbox(
            &mut self.settings.auto_crop,
            if zh {
                "自动识别并裁剪演示区域"
            } else {
                "Automatically detect and crop presentation area"
            },
        );
        ui.label(if zh { "识别不确定时保留完整图片；原始截图不会被修改。请检查生成结果。" } else { "If detection is uncertain, the full image is kept. Originals are unchanged; check the result." });
        if ui
            .add_enabled(
                self.export.is_none() && !self.source_dir.trim().is_empty(),
                egui::Button::new(if zh { "开始生成" } else { "Generate PPTX" }),
            )
            .clicked()
        {
            self.start(language);
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        if self.output_to_open.is_some()
            && ui
                .button(if zh {
                    "打开生成文件夹"
                } else {
                    "Open generated folder"
                })
                .clicked()
        {
            // The application consumes this by calling take_output_to_open after rendering.
            self.open_requested = true;
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
}
