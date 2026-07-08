//! Enumerate and locate windows via Win32 API.

use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;

use crate::model::{WindowInfo, Region};

/// Enumerate all top-level windows visible on the desktop.
pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let mut windows = Vec::new();

    unsafe {
        let ctx = &mut windows as *mut Vec<WindowInfo> as isize;
        EnumWindows(Some(enum_window_proc), ctx)
            .ok()
            .context("EnumWindows failed")?;
    }

    // Filter to only valid windows
    windows.retain(|w| w.is_valid() && !w.title.is_empty());

    Ok(windows)
}

extern "system" fn enum_window_proc(hwnd: HWND, lparam: isize) -> BOOL {
    unsafe {
        let windows = &mut *(lparam as *mut Vec<WindowInfo>);

        // Skip windows without WS_VISIBLE
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let is_visible = (style & WS_VISIBLE.0 as i32) != 0;

        if !is_visible {
            return TRUE;
        }

        // Skip windows with no title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len == 0 {
            return TRUE;
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        // Skip small/invisible windows
        if title.trim().is_empty() {
            return TRUE;
        }

        // Get class name
        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };

        // Get window rectangle
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE;
        }

        let region = Region::new(
            rect.left, rect.top,
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        );

        if !region.is_valid() {
            return TRUE;
        }

        // Check if minimized
        let is_minimized = (style & WS_MINIMIZE.0 as i32) != 0 || IsIconic(hwnd).as_bool();

        // Get monitor for this window
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let hmonitor = monitor.0 as u64;

        // Get process ID and name
        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process_name = get_process_name(pid);

        let is_pp = class_name.eq_ignore_ascii_case("screenClass")
            || title.contains("Slide Show")
            || title.contains("PowerPoint")
            || title.contains("POWERPNT");

        let win_info = WindowInfo {
            hwnd: hwnd.0 as u64,
            title: title.trim().to_string(),
            class_name,
            region,
            monitor_hmonitor: hmonitor,
            is_visible,
            is_minimized,
            is_powerpoint: is_pp,
            process_id: pid,
            process_name,
        };

        windows.push(win_info);
        TRUE
    }
}

/// Get process name from PID on Windows.
fn get_process_name(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    // Use kernel32 to get process name
    unsafe {
        let handle = windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        );
        if let Ok(handle) = handle {
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let result = windows::Win32::System::ProcessStatus::QueryFullProcessImageNameW(
                handle,
                windows::Win32::System::ProcessStatus::PROCESS_NAME_WIN32,
                &mut buf,
                &mut size,
            );
            let _ = windows::Win32::System::Threading::CloseHandle(handle);
            if result.is_ok() {
                let path = String::from_utf16_lossy(&buf[..size as usize]);
                if let Some(name) = std::path::Path::new(&path).file_name() {
                    return name.to_string_lossy().to_uppercase();
                }
                return path;
            }
        }
    }
    String::new()
}

/// Find a specific window by HWND.
pub fn find_window(hwnd: u64) -> Result<Option<WindowInfo>> {
    let windows = enumerate_windows()?;
    Ok(windows.into_iter().find(|w| w.hwnd == hwnd))
}
