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
            ui.text_edit_singleline(&mut self.settings.output_filename);
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
        let source = PathBuf::from(self.source_dir.trim());
        let destination = PathBuf::from(self.settings.output_dir.trim());
        let filename = self.settings.output_filename.clone();
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
            while let Ok(message) = rx.try_recv() {
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
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("folder-picker".into())
            .spawn(move || {
                let _ = tx.send(pick_folder());
            });
        match spawned {
            Ok(_) => self.picker = Some(rx),
            Err(error) => self.status = format!("Failed to open folder picker: {error}"),
        }
    }
}

fn pick_folder() -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("powershell.exe").args(["-NoProfile", "-STA", "-Command", "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::Write($d.SelectedPath) }"]).output();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Select an image folder\")",
        ])
        .output();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("zenity")
        .args(["--file-selection", "--directory"])
        .output();
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
}
