use anyhow::{Context, Result};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::*;
use crate::model::{WindowInfo, Region};

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let mut w = Vec::new();
    unsafe {
        EnumWindows(Some(epw), &mut w as *mut _ as isize).ok().context("EnumWindows")?;
    }
    w.retain(|w| w.is_valid() && !w.title.is_empty());
    Ok(w)
}

unsafe extern "system" fn epw(hwnd: HWND, lp: isize) -> BOOL {
    let w = &mut *(lp as *mut Vec<WindowInfo>);
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    if style & 0x10000000 == 0 { return BOOL(1); } // WS_VISIBLE
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
    let reg = Region::new(r.left,r.top,(r.right-r.left)as u32,(r.bottom-r.top)as u32);
    if !reg.is_valid() { return BOOL(1); }
    let hm = MonitorFromWindow(hwnd, 2);
    let mut pid: u32 = 0;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    w.push(WindowInfo {
        hwnd: hwnd.0 as u64, title, class_name: class, region:reg,
        monitor_hmonitor:hm.0 as u64, is_visible:true, is_minimized:false,
        is_powerpoint: class.eq_ignore_ascii_case("screenClass"),
        process_id:pid, process_name: format!("{}", pid),
    });
    BOOL(1)
}
