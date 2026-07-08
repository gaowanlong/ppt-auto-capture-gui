use anyhow::{Context, Result};
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, RECT, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetClassNameW, GetWindowLongW, GetWindowRect,
    IsIconic, MonitorFromWindow, MONITOR_DEFAULTTOPRIMARY,
    GetWindowThreadProcessId, GWL_STYLE, WS_VISIBLE, WS_MINIMIZE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use crate::model::{WindowInfo, Region};

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let mut windows = Vec::new();
    unsafe {
        EnumWindows(Some(ewp), &mut windows as *mut _ as isize).ok().context("EnumWindows")?;
    }
    windows.retain(|w| w.is_valid() && !w.title.is_empty());
    Ok(windows)
}

unsafe extern "system" fn ewp(hwnd: HWND, lparam: isize) -> BOOL {
    let w = &mut *(lparam as *mut Vec<WindowInfo>);
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    if style & WS_VISIBLE.0 == 0 { return BOOL(1); }
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len == 0 { return BOOL(1); }
    let title = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
    if title.is_empty() { return BOOL(1); }
    let mut cb = [0u16; 256];
    let cl = GetClassNameW(hwnd, &mut cb);
    let class = if cl > 0 { String::from_utf16_lossy(&cb[..cl as usize]) } else { String::new() };
    let mut r = RECT::default();
    if GetWindowRect(hwnd, &mut r).is_err() { return BOOL(1); }
    let region = Region::new(r.left, r.top, (r.right-r.left)as u32, (r.bottom-r.top)as u32);
    if !region.is_valid() { return BOOL(1); }
    let minim = (style & WS_MINIMIZE.0 as u32) != 0 || IsIconic(hwnd).as_bool();
    let hm = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
    let mut pid: u32 = 0;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let pname = get_process_name(pid);
    let ppt = class.eq_ignore_ascii_case("screenClass") || title.contains("Slide Show");
    w.push(WindowInfo {
        hwnd: hwnd.0 as u64, title, class_name: class, region, monitor_hmonitor: hm.0 as u64,
        is_visible: true, is_minimized: minim, is_powerpoint: ppt, process_id: pid, process_name: pname,
    });
    BOOL(1)
}

#[cfg(target_os = "windows")]
fn get_process_name(_pid: u32) -> String {
    // Simplified: return PID as string since QueryFullProcessImageNameW is not consistently
    // available across all Windows SDK versions in the windows crate 0.60
    if _pid == 0 { return String::new(); }
    format!("PID_{}", _pid)
}
