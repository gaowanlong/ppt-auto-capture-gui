use crate::model::{MonitorInfo, Region, WindowInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MonitorSnapshot {
    pub id: u32,
    pub name: String,
    pub friendly_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowSnapshot {
    pub id: u32,
    pub pid: u32,
    pub app_name: String,
    pub title: String,
    pub monitor_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureErrorKind {
    PermissionDenied,
    DisplayLost,
    WindowLost,
    Other,
}

pub(crate) fn rgba_to_bgra(mut pixels: Vec<u8>) -> Vec<u8> {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    pixels
}

pub(crate) fn monitor_info(snapshot: MonitorSnapshot, output_index: usize) -> MonitorInfo {
    MonitorInfo {
        hmonitor: u64::from(snapshot.id),
        adapter_name: "macOS".into(),
        output_name: snapshot.name,
        description: snapshot.friendly_name,
        region: Region::new(snapshot.x, snapshot.y, snapshot.width, snapshot.height),
        is_primary: snapshot.is_primary,
        is_virtual_suspect: false,
        output_index: output_index as u32,
        adapter_index: 0,
    }
}

pub(crate) fn window_info(snapshot: WindowSnapshot) -> WindowInfo {
    let identity = format!("{} {}", snapshot.app_name, snapshot.title).to_ascii_lowercase();
    let is_powerpoint = identity.contains("powerpoint") || identity.contains("powerpnt");

    WindowInfo {
        hwnd: u64::from(snapshot.id),
        title: snapshot.title,
        class_name: snapshot.app_name.clone(),
        region: Region::new(snapshot.x, snapshot.y, snapshot.width, snapshot.height),
        monitor_hmonitor: u64::from(snapshot.monitor_id),
        is_visible: snapshot.width > 0 && snapshot.height > 0,
        is_minimized: snapshot.is_minimized,
        is_powerpoint,
        process_id: snapshot.pid,
        process_name: snapshot.app_name,
    }
}

pub(crate) fn capture_error_message(kind: CaptureErrorKind, detail: &str) -> String {
    match kind {
        CaptureErrorKind::PermissionDenied => format!(
            "Screen Recording permission is required. Open System Settings → Privacy & Security → Screen Recording, enable PPT Auto Capture, then restart the app. \
             需要“屏幕录制”权限。请打开“系统设置”→“隐私与安全性”→“屏幕录制”，启用 PPT Auto Capture，然后重新启动应用。 ({detail})"
        ),
        CaptureErrorKind::DisplayLost => format!(
            "The selected display ({detail}) is no longer available. Refresh displays and select it again. \
             所选显示器（{detail}）已不可用。请刷新显示器列表后重新选择。"
        ),
        CaptureErrorKind::WindowLost => format!(
            "The selected window ({detail}) is no longer available. Refresh windows and select it again. \
             所选窗口（{detail}）已不可用。请刷新窗口列表后重新选择。"
        ),
        CaptureErrorKind::Other => format!("macOS capture failed / macOS 截图失败: {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgba_pixels_to_bgra() {
        assert_eq!(rgba_to_bgra(vec![10, 20, 30, 40]), vec![30, 20, 10, 40]);
    }

    #[test]
    fn monitor_mapping_preserves_negative_coordinates() {
        let info = monitor_info(
            MonitorSnapshot {
                id: 42,
                name: "Display 42".into(),
                friendly_name: "Studio Display".into(),
                x: -1920,
                y: -120,
                width: 1920,
                height: 1080,
                is_primary: false,
            },
            3,
        );

        assert_eq!(info.hmonitor, 42);
        assert_eq!(info.region.x, -1920);
        assert_eq!(info.region.y, -120);
        assert_eq!(info.output_index, 3);
    }

    #[test]
    fn window_mapping_recognizes_powerpoint() {
        let info = window_info(WindowSnapshot {
            id: 7,
            pid: 99,
            app_name: "Microsoft PowerPoint".into(),
            title: "Quarterly Review".into(),
            monitor_id: 42,
            x: 10,
            y: 20,
            width: 1280,
            height: 720,
            is_minimized: false,
        });

        assert!(info.is_powerpoint);
        assert_eq!(info.process_name, "Microsoft PowerPoint");
        assert_eq!(info.monitor_hmonitor, 42);
    }

    #[test]
    fn permission_error_is_bilingual_and_actionable() {
        let message = capture_error_message(CaptureErrorKind::PermissionDenied, "denied");
        assert!(message.contains("System Settings"));
        assert!(message.contains("restart"));
        assert!(message.contains("系统设置"));
        assert!(message.contains("重新启动"));
    }

    #[test]
    fn source_loss_errors_identify_the_selected_source() {
        let display = capture_error_message(CaptureErrorKind::DisplayLost, "42");
        let window = capture_error_message(CaptureErrorKind::WindowLost, "7");
        assert!(display.contains("display"));
        assert!(display.contains("显示器"));
        assert!(window.contains("window"));
        assert!(window.contains("窗口"));
    }
}
