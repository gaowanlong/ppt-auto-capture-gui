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

    pub fn next(self) -> Self {
        match self {
            Language::Chinese => Language::English,
            Language::English => Language::Chinese,
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

pub fn t_status(lang: Language) -> &'static str {
    match lang {
        Language::English => "Status:",
        Language::Chinese => "状态：",
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
pub fn t_output(lang: Language) -> &'static str {
    match lang {
        Language::English => "PPTX output (after stop):",
        Language::Chinese => "PPTX 输出（停止后生成）：",
    }
}
pub fn t_slides_saved(lang: Language) -> &'static str {
    match lang {
        Language::English => "Slides Saved:",
        Language::Chinese => "已保存页：",
    }
}
pub fn t_start(lang: Language) -> &'static str {
    match lang {
        Language::English => "▶ Start",
        Language::Chinese => "▶ 开始",
    }
}
pub fn t_pause(lang: Language) -> &'static str {
    match lang {
        Language::English => "⏸ Pause",
        Language::Chinese => "⏸ 暂停",
    }
}
pub fn t_resume(lang: Language) -> &'static str {
    match lang {
        Language::English => "▶ Resume",
        Language::Chinese => "▶ 恢复",
    }
}
pub fn t_stop(lang: Language) -> &'static str {
    match lang {
        Language::English => "⏹ Stop",
        Language::Chinese => "⏹ 停止",
    }
}
pub fn t_test_preview(lang: Language) -> &'static str {
    match lang {
        Language::English => "Test Capture Preview:",
        Language::Chinese => "测试截图预览：",
    }
}
pub fn t_protected_warning(lang: Language) -> &'static str {
    match lang {
        Language::English => {
            "⚠ Protected or black content detected. Capture paused.\n\
             The window might be showing protected video content or is blank.\n\
             PPT will resume automatically when content becomes visible."
        }
        Language::Chinese => {
            "⚠ 检测到受保护或黑屏内容。捕获已暂停。\n\
             窗口可能正在播放受保护的视频内容或处于黑屏状态。\n\
             当内容变得可见时，程序将自动恢复。"
        }
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
pub fn t_refresh_windows(lang: Language) -> &'static str {
    match lang {
        Language::English => "🔄 Refresh Window List",
        Language::Chinese => "🔄 刷新窗口列表",
    }
}
pub fn t_no_windows(lang: Language) -> &'static str {
    match lang {
        Language::English => "No windows found. Click Refresh.",
        Language::Chinese => "未找到窗口。点击刷新。",
    }
}
pub fn t_selected(lang: Language) -> &'static str {
    match lang {
        Language::English => "Selected:",
        Language::Chinese => "已选择：",
    }
}
pub fn t_move_to_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "↗ Move to Display",
        Language::Chinese => "↗ 移动到显示器",
    }
}
pub fn t_maximize(lang: Language) -> &'static str {
    match lang {
        Language::English => "⬜ Maximize",
        Language::Chinese => "⬜ 最大化",
    }
}
pub fn t_macos_window_controls_unavailable(lang: Language) -> &'static str {
    match lang {
        Language::English => "Moving or maximizing another app requires Accessibility permission and is not available in this macOS release.",
        Language::Chinese => "移动或最大化其他应用需要“辅助功能”权限，此 macOS 版本暂不提供该功能。",
    }
}
pub fn t_test_screenshot(lang: Language) -> &'static str {
    match lang {
        Language::English => "📷 Test Screenshot",
        Language::Chinese => "📷 测试截图",
    }
}

// --- Display Panel ---
pub fn t_capture_display(lang: Language) -> &'static str {
    match lang {
        Language::English => "Capture Display",
        Language::Chinese => "捕获显示器",
    }
}
pub fn t_refresh_displays(lang: Language) -> &'static str {
    match lang {
        Language::English => "🔄 Refresh Displays",
        Language::Chinese => "🔄 刷新显示器",
    }
}
pub fn t_no_displays(lang: Language) -> &'static str {
    match lang {
        Language::English => "No displays found. Click Refresh.",
        Language::Chinese => "未找到显示器。点击刷新。",
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
pub fn t_test_capture(lang: Language) -> &'static str {
    match lang {
        Language::English => "📷 Test Capture",
        Language::Chinese => "📷 测试采集",
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
pub fn t_output_settings(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output Settings",
        Language::Chinese => "输出设置",
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
pub fn t_open_output(lang: Language) -> &'static str {
    match lang {
        Language::English => "📂 Open Output Directory",
        Language::Chinese => "📂 打开输出目录",
    }
}
pub fn t_output_notes(lang: Language) -> &'static str {
    match lang {
        Language::English => "Output notes:",
        Language::Chinese => "输出说明：",
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

// --- Monitor errors ---
pub fn t_no_display_selected(lang: Language) -> &'static str {
    match lang {
        Language::English => "Select a capture display",
        Language::Chinese => "请选择捕获显示器",
    }
}
pub fn t_black_frame(lang: Language) -> &'static str {
    match lang {
        Language::English => "Black frame",
        Language::Chinese => "黑屏帧",
    }
}
pub fn t_none(lang: Language) -> &'static str {
    match lang {
        Language::English => "None",
        Language::Chinese => "无",
    }
}
pub fn t_language_switch(lang: Language) -> &'static str {
    match lang {
        Language::English => "🌐 Language",
        Language::Chinese => "🌐 语言",
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
    fn test_language_next() {
        assert_eq!(Language::English.next(), Language::Chinese);
        assert_eq!(Language::Chinese.next(), Language::English);
    }

    #[test]
    fn test_all_translations_non_empty() {
        let langs = [Language::English, Language::Chinese];
        // Spot-check several translations
        for lang in &langs {
            assert!(!t_main_title(*lang).is_empty());
            assert!(!t_status(*lang).is_empty());
            assert!(!t_start(*lang).is_empty());
            assert!(!t_stop(*lang).is_empty());
            assert!(!t_window_source(*lang).is_empty());
            assert!(!t_capture_display(*lang).is_empty());
            assert!(!t_settings_title(*lang).is_empty());
            assert!(!t_output_settings(*lang).is_empty());
            assert!(!t_diagnostics(*lang).is_empty());
            assert!(!t_copy_diagnostics(*lang).is_empty());
            assert!(!t_open_log_folder(*lang).is_empty());
            assert!(!t_diagnostics_copied(*lang).is_empty());
            assert!(!t_persistent_logging_unavailable(*lang).is_empty());
        }
    }

    #[test]
    fn test_chinese_and_english_different() {
        assert_ne!(t_status(Language::English), t_status(Language::Chinese));
        assert_ne!(t_start(Language::English), t_start(Language::Chinese));
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
