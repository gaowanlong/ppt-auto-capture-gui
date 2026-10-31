//! Internationalization (i18n) — Chinese and English translations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Language {
    Chinese,
    #[default]
    English,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Chinese => "中文",
            Language::English => "English",
        }
    }
}

// --- Translation helpers ---

pub fn t_main_title(lang: Language) -> &'static str {
    match lang {
        Language::English => "PPT Auto Capture",
        Language::Chinese => "PPT 自动截图工具",
    }
}

pub fn t_tab_dashboard(lang: Language) -> &'static str {
    match lang {
        Language::English => "Dashboard",
        Language::Chinese => "主面板",
    }
}
pub fn t_tab_source(lang: Language) -> &'static str {
    match lang {
        Language::English => "Window",
        Language::Chinese => "窗口",
    }
}
pub fn t_tab_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "Display",
        Language::Chinese => "显示器",
    }
}
pub fn t_tab_settings(lang: Language) -> &'static str {
    match lang {
        Language::English => "Settings",
        Language::Chinese => "设置",
    }
}
pub fn t_tab_output(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output",
        Language::Chinese => "输出",
    }
}
pub fn t_tab_export(lang: Language) -> &'static str {
    match lang {
        Language::English => "Create PPTX",
        Language::Chinese => "图片转 PPTX",
    }
}

pub fn t_start_recording(lang: Language) -> &'static str {
    match lang {
        Language::English => "Start recording",
        Language::Chinese => "开始录制",
    }
}
pub fn t_pause_recording(lang: Language) -> &'static str {
    match lang {
        Language::English => "Pause",
        Language::Chinese => "暂停",
    }
}
pub fn t_resume_recording(lang: Language) -> &'static str {
    match lang {
        Language::English => "Resume capture",
        Language::Chinese => "继续录制",
    }
}
pub fn t_stop_recording(lang: Language) -> &'static str {
    match lang {
        Language::English => "Stop",
        Language::Chinese => "停止",
    }
}
pub fn t_saving_recording(lang: Language) -> &'static str {
    match lang {
        Language::English => "Saving and building PPTX…",
        Language::Chinese => "正在保存并生成 PPTX…",
    }
}
pub fn t_minimize_on_start(lang: Language) -> &'static str {
    match lang {
        Language::English => "Start minimized",
        Language::Chinese => "开始录制时最小化窗口",
    }
}
pub fn t_minimize_on_start_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => "Capture begins immediately; the main window minimizes right away.",
        Language::Chinese => "截图会立即开始，主窗口会同时最小化。",
    }
}
pub fn t_refresh(lang: Language) -> &'static str {
    match lang {
        Language::English => "Refresh",
        Language::Chinese => "刷新",
    }
}
pub fn t_test_screenshot_short(lang: Language) -> &'static str {
    match lang {
        Language::English => "Test screenshot",
        Language::Chinese => "测试截图",
    }
}
pub fn t_test_capture_short(lang: Language) -> &'static str {
    match lang {
        Language::English => "Test capture",
        Language::Chinese => "测试采集",
    }
}
pub fn t_open_recording_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Open recording folder",
        Language::Chinese => "打开录制文件夹",
    }
}
pub fn t_delete_recording_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Delete recording folder…",
        Language::Chinese => "删除录制文件夹…",
    }
}
pub fn t_deleting_recording_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Deleting…",
        Language::Chinese => "正在删除…",
    }
}
pub fn t_retry_delete(lang: Language) -> &'static str {
    match lang {
        Language::English => "Retry delete",
        Language::Chinese => "重试删除",
    }
}
pub fn t_delete_folder_failed(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "The recording folder is still in use. Close the program using it, then retry."
        }
        Language::Chinese => "录制文件夹仍被占用，请关闭占用它的程序后重试。",
    }
}
pub fn t_manage_recording_folders(lang: Language) -> &'static str {
    match lang {
        Language::English => "Manage recording folders…",
        Language::Chinese => "管理录制文件夹…",
    }
}

// --- Dashboard ---
pub fn t_diagnostics(lang: Language) -> &'static str {
    match lang {
        Language::English => "Diagnostics",
        Language::Chinese => "诊断",
    }
}
pub fn t_copy_diagnostics(lang: Language) -> &'static str {
    match lang {
        Language::English => "Copy diagnostics",
        Language::Chinese => "复制诊断信息",
    }
}
pub fn t_open_log_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Open log folder",
        Language::Chinese => "打开日志文件夹",
    }
}
pub fn t_diagnostics_copied(lang: Language) -> &'static str {
    match lang {
        Language::English => "Diagnostics copied",
        Language::Chinese => "诊断信息已复制",
    }
}
pub fn t_persistent_logging_unavailable(lang: Language) -> &'static str {
    match lang {
        Language::English => "Persistent logging unavailable",
        Language::Chinese => "持久日志不可用",
    }
}

pub fn t_capture_source(lang: Language) -> &'static str {
    match lang {
        Language::English => "Capture Source:",
        Language::Chinese => "捕获源：",
    }
}
pub fn t_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "Display:",
        Language::Chinese => "显示器：",
    }
}
pub fn t_session_recovery_title(lang: Language) -> &'static str {
    match lang {
        Language::English => "Session Recovery",
        Language::Chinese => "会话恢复",
    }
}
pub fn t_recovery_msg(lang: Language, slides: u32) -> String {
    match lang {
        Language::English => format!("{} slides not completed. Recover?", slides),
        Language::Chinese => format!("{} 张幻灯片未完成。是否恢复？", slides),
    }
}
pub fn t_recover(lang: Language) -> &'static str {
    match lang {
        Language::English => "Recover",
        Language::Chinese => "恢复",
    }
}
pub fn t_skip(lang: Language) -> &'static str {
    match lang {
        Language::English => "Skip",
        Language::Chinese => "跳过",
    }
}

// --- Source Panel ---
pub fn t_window_source(lang: Language) -> &'static str {
    match lang {
        Language::English => "Window Source",
        Language::Chinese => "窗口源",
    }
}
pub fn t_selected(lang: Language) -> &'static str {
    match lang {
        Language::English => "Selected:",
        Language::Chinese => "已选择：",
    }
}
pub fn t_macos_window_controls_unavailable(lang: Language) -> &'static str {
    match lang {
        Language::English => "Moving or maximizing another app requires Accessibility permission and is not available in this macOS release.",
        Language::Chinese => "移动或最大化其他应用需要“辅助功能”权限，此 macOS 版本暂不提供该功能。",
    }
}
// --- Display Panel ---
pub fn t_capture_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "Capture Display",
        Language::Chinese => "捕获显示器",
    }
}
pub fn t_adapter(lang: Language) -> &'static str {
    match lang {
        Language::English => "Adapter:",
        Language::Chinese => "适配器：",
    }
}
pub fn t_virtual_suspect(lang: Language) -> &'static str {
    match lang {
        Language::English => "⚠ Suspected virtual/dummy display",
        Language::Chinese => "⚠ 疑似虚拟/模拟显示器",
    }
}
pub fn t_physical_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "✅ Physical display",
        Language::Chinese => "✅ 物理显示器",
    }
}
pub fn t_primary_monitor(lang: Language) -> &'static str {
    match lang {
        Language::English => "★ Primary monitor",
        Language::Chinese => "★ 主显示器",
    }
}
// --- Settings Panel ---
pub fn t_settings_title(lang: Language) -> &'static str {
    match lang {
        Language::English => "Capture Settings",
        Language::Chinese => "捕获设置",
    }
}
pub fn t_sample_interval(lang: Language) -> &'static str {
    match lang {
        Language::English => "Sample Interval (ms):",
        Language::Chinese => "采样间隔(毫秒)：",
    }
}
pub fn t_stability_frames(lang: Language) -> &'static str {
    match lang {
        Language::English => "Stability Frames:",
        Language::Chinese => "稳定帧数：",
    }
}
pub fn t_anim_timeout(lang: Language) -> &'static str {
    match lang {
        Language::English => "Animation Timeout (ms):",
        Language::Chinese => "动画超时(毫秒)：",
    }
}
pub fn t_change_threshold(lang: Language) -> &'static str {
    match lang {
        Language::English => "Change Threshold:",
        Language::Chinese => "变化阈值：",
    }
}
pub fn t_black_threshold(lang: Language) -> &'static str {
    match lang {
        Language::English => "Black Threshold:",
        Language::Chinese => "黑屏阈值：",
    }
}
pub fn t_filter_duplicates(lang: Language) -> &'static str {
    match lang {
        Language::English => "Filter Duplicate Slides",
        Language::Chinese => "过滤重复幻灯片",
    }
}

// --- Output Panel ---
pub fn t_output_page_title(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output & folders",
        Language::Chinese => "输出与文件夹",
    }
}
pub fn t_output_page_subtitle(lang: Language) -> &'static str {
    match lang {
        Language::English => "Choose where recordings live and how slides are laid out.",
        Language::Chinese => "选择录制文件的存放位置和幻灯片版式。",
    }
}
pub fn t_output_locked(lang: Language) -> &'static str {
    match lang {
        Language::English => "Recording in progress. Changes here apply to the next recording.",
        Language::Chinese => "正在录制。这里的修改将在下次录制时生效。",
    }
}
pub fn t_output_where(lang: Language) -> &'static str {
    match lang {
        Language::English => "Where files go",
        Language::Chinese => "文件去向",
    }
}
pub fn t_output_layout(lang: Language) -> &'static str {
    match lang {
        Language::English => "Slide layout",
        Language::Chinese => "幻灯片版式",
    }
}
pub fn t_output_folders(lang: Language) -> &'static str {
    match lang {
        Language::English => "Recording folders",
        Language::Chinese => "录制文件夹",
    }
}
pub fn t_output_dir_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => "Relative paths are created under the app's working folder.",
        Language::Chinese => "相对路径会创建在应用的工作目录下。",
    }
}
pub fn t_filename_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "The automatic name refreshes on every start; a custom name stays unchanged."
        }
        Language::Chinese => "自动文件名会在每次开始时刷新；自定义文件名保持不变。",
    }
}
pub fn t_automatic_name(lang: Language) -> &'static str {
    match lang {
        Language::English => "Automatic",
        Language::Chinese => "自动命名",
    }
}
pub fn t_auto_crop_label(lang: Language) -> &'static str {
    match lang {
        Language::English => "Auto-crop the slide area when building the PPTX",
        Language::Chinese => "生成 PPTX 时自动裁剪幻灯片区域",
    }
}
pub fn t_auto_crop_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => "If the slide cannot be identified, the full frame is kept.",
        Language::Chinese => "如果无法识别幻灯片，则保留完整画面。",
    }
}
pub fn t_output_notes_heading(lang: Language) -> &'static str {
    match lang {
        Language::English => "What each recording saves",
        Language::Chinese => "每次录制会保存什么",
    }
}
pub fn t_output_folders_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "Every recording gets its own folder; earlier sessions are never overwritten."
        }
        Language::Chinese => "每次录制都有独立文件夹，之前的会话不会被覆盖。",
    }
}
pub fn t_output_dir(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output Directory:",
        Language::Chinese => "输出目录：",
    }
}
pub fn t_pptx_filename(lang: Language) -> &'static str {
    match lang {
        Language::English => "PPTX Filename:",
        Language::Chinese => "PPTX 文件名：",
    }
}
pub fn t_slide_aspect(lang: Language) -> &'static str {
    match lang {
        Language::English => "Slide Aspect Ratio:",
        Language::Chinese => "幻灯片比例：",
    }
}
pub fn t_image_fit(lang: Language) -> &'static str {
    match lang {
        Language::English => "Image Fit:",
        Language::Chinese => "图片适配：",
    }
}
pub fn t_fit_fill(lang: Language) -> &'static str {
    match lang {
        Language::English => "Fill",
        Language::Chinese => "填充",
    }
}
pub fn t_fit_fit(lang: Language) -> &'static str {
    match lang {
        Language::English => "Fit",
        Language::Chinese => "适应",
    }
}
pub fn t_output_notes_1(lang: Language) -> &'static str {
    match lang {
        Language::English => "• Screenshots are saved in the recording's slides folder.",
        Language::Chinese => "• 截图保存在本次录制的 slides 文件夹中。",
    }
}
pub fn t_output_notes_2(lang: Language) -> &'static str {
    match lang {
        Language::English => "• PPTX files are generated after Stop, with at most 50 images each.",
        Language::Chinese => "• 停止后生成 PPTX，每个文件最多 50 张截图。",
    }
}
pub fn t_output_notes_3(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "• Each recording uses its own folder and will not overwrite earlier recordings."
        }
        Language::Chinese => "• 每次录制使用独立文件夹，不覆盖之前的录制。",
    }
}
pub fn t_output_notes_4(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "• session.json and original PNG files allow recovery after interruption."
        }
        Language::Chinese => "• session.json 和原始 PNG 可用于中断后恢复。",
    }
}

// --- Export Panel ---
pub fn t_export_page_title(lang: Language) -> &'static str {
    match lang {
        Language::English => "Create PPTX from images",
        Language::Chinese => "图片生成 PPTX",
    }
}
pub fn t_export_page_subtitle(lang: Language) -> &'static str {
    match lang {
        Language::English => "Turn a folder of saved slides into editable presentations.",
        Language::Chinese => "把已保存的截图整理成可编辑的幻灯片。",
    }
}
pub fn t_export_source_card(lang: Language) -> &'static str {
    match lang {
        Language::English => "Source images",
        Language::Chinese => "源图片",
    }
}
pub fn t_export_source_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => "PNG/JPEG files are imported in natural filename order, 50 per PPTX.",
        Language::Chinese => "按文件名中的数字顺序导入 PNG/JPEG，每 50 张生成一个 PPTX。",
    }
}
pub fn t_export_deck_card(lang: Language) -> &'static str {
    match lang {
        Language::English => "Deck settings",
        Language::Chinese => "幻灯片设置",
    }
}
pub fn t_export_output_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output folder:",
        Language::Chinese => "输出文件夹：",
    }
}
pub fn t_export_browse(lang: Language) -> &'static str {
    match lang {
        Language::English => "Browse…",
        Language::Chinese => "选择…",
    }
}
pub fn t_export_generate(lang: Language) -> &'static str {
    match lang {
        Language::English => "Generate PPTX",
        Language::Chinese => "开始生成",
    }
}
pub fn t_export_open_folder(lang: Language) -> &'static str {
    match lang {
        Language::English => "Open generated folder",
        Language::Chinese => "打开生成文件夹",
    }
}
pub fn t_export_crop_label(lang: Language) -> &'static str {
    match lang {
        Language::English => "Detect and crop the presentation area",
        Language::Chinese => "识别并裁剪演示区域",
    }
}
pub fn t_export_crop_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "If detection is uncertain the full image is kept. Originals are unchanged; check the result."
        }
        Language::Chinese => "识别不确定时保留完整图片；原始截图不会被修改。请检查生成结果。",
    }
}
pub fn t_export_ready_hint(lang: Language) -> &'static str {
    match lang {
        Language::English => "Pick a folder of slides to begin.",
        Language::Chinese => "选择一个包含截图的文件夹开始。",
    }
}

// --- Monitor errors ---
pub fn t_no_display_selected(lang: Language) -> &'static str {
    match lang {
        Language::English => "Select a capture display",
        Language::Chinese => "请选择捕获显示器",
    }
}
pub fn t_none(lang: Language) -> &'static str {
    match lang {
        Language::English => "None",
        Language::Chinese => "无",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_label() {
        assert_eq!(Language::English.label(), "English");
        assert_eq!(Language::Chinese.label(), "中文");
    }

    #[test]
    fn test_all_translations_non_empty() {
        let langs = [Language::English, Language::Chinese];
        // Spot-check several translations
        for lang in &langs {
            assert!(!t_main_title(*lang).is_empty());
            assert!(!t_start_recording(*lang).is_empty());
            assert!(!t_stop_recording(*lang).is_empty());
            assert!(!t_window_source(*lang).is_empty());
            assert!(!t_capture_display(*lang).is_empty());
            assert!(!t_settings_title(*lang).is_empty());
            assert!(!t_output_page_title(*lang).is_empty());
            assert!(!t_diagnostics(*lang).is_empty());
            assert!(!t_copy_diagnostics(*lang).is_empty());
            assert!(!t_open_log_folder(*lang).is_empty());
            assert!(!t_diagnostics_copied(*lang).is_empty());
            assert!(!t_persistent_logging_unavailable(*lang).is_empty());
            assert!(!t_start_recording(*lang).is_empty());
            assert!(!t_minimize_on_start(*lang).is_empty());
            assert!(!t_minimize_on_start_hint(*lang).is_empty());
            assert!(!t_retry_delete(*lang).is_empty());
            assert!(!t_delete_folder_failed(*lang).is_empty());
            assert!(!t_tab_export(*lang).is_empty());
        }
    }

    #[test]
    fn test_chinese_and_english_different() {
        assert_ne!(
            t_start_recording(Language::English),
            t_start_recording(Language::Chinese)
        );
        assert_ne!(
            t_diagnostics(Language::English),
            t_diagnostics(Language::Chinese)
        );
        assert_ne!(
            t_copy_diagnostics(Language::English),
            t_copy_diagnostics(Language::Chinese)
        );
        assert_ne!(
            t_open_log_folder(Language::English),
            t_open_log_folder(Language::Chinese)
        );
        assert_ne!(
            t_diagnostics_copied(Language::English),
            t_diagnostics_copied(Language::Chinese)
        );
        assert_ne!(
            t_persistent_logging_unavailable(Language::English),
            t_persistent_logging_unavailable(Language::Chinese)
        );
    }

    #[test]
    fn test_recovery_msg_formatting() {
        let msg_en = t_recovery_msg(Language::English, 5);
        assert!(msg_en.contains("5"));
        let msg_cn = t_recovery_msg(Language::Chinese, 3);
        assert!(msg_cn.contains("3"));
    }

    #[test]
    fn macos_window_control_explanation_is_localized() {
        assert!(t_macos_window_controls_unavailable(Language::English).contains("Accessibility"));
        assert!(t_macos_window_controls_unavailable(Language::Chinese).contains("辅助功能"));
    }
}
