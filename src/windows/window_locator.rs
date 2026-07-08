use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::{BOOL, CloseHandle, TRUE, FALSE, HWND, RECT, LPARAM, HINSTANCE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::ProcessStatus::{QueryFullProcessImageNameW, PROCESS_NAME_WIN32};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use crate::model::{WindowInfo, Region};

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let mut windows = Vec::new();

    unsafe {
        let ctx = &mut windows as *mut Vec<WindowInfo> as isize;
        EnumWindows(Some(enum_window_proc), ctx)
            .ok()
            .context("EnumWindows failed")?;
    }

    windows.retain(|w| w.is_valid() && !w.title.is_empty());
    Ok(windows)
}

extern "system" fn enum_window_proc(hwnd: HWND, lparam: isize) -> BOOL {
    unsafe {
        let windows = &mut *(lparam as *mut Vec<WindowInfo>);

        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let is_visible = (style & WS_VISIBLE.0 as i32) != 0;
        if !is_visible { return TRUE; }

        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len == 0 { return TRUE; }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);
        if title.trim().is_empty() { return TRUE; }

        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else { String::new() };

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() { return TRUE; }

        let region = Region::new(
            rect.left, rect.top,
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        );
        if !region.is_valid() { return TRUE; }

        let is_minimized = (style & WS_MINIMIZE.0 as i32) != 0 || IsIconic(hwnd).as_bool();

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let hmonitor = monitor.0 as u64;

        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process_name = get_process_name(pid);

        let is_pp = class_name.eq_ignore_ascii_case("screenClass")
            || title.contains("Slide Show")
            || title.contains("PowerPoint")
            || title.contains("POWERPNT");

        windows.push(WindowInfo {
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
        });

        TRUE
    }
}

fn get_process_name(pid: u32) -> String {
    if pid == 0 { return String::new(); }
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 260];
            let mut size = buf.len() as u32;
            let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, &mut buf, &mut size);
            let _ = CloseHandle(handle);
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

pub fn find_window(hwnd: u64) -> Result<Option<WindowInfo>> {
    let windows = enumerate_windows()?;
    Ok(windows.into_iter().find(|w| w.hwnd == hwnd))
}
