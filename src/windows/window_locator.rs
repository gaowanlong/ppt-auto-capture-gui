use anyhow::{Context, Result};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    WNDENUMPROC, EnumWindows, GetWindowTextW, GetClassNameW,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
    GWL_STYLE, WS_VISIBLE,
};
use crate::model::{WindowInfo, Region};

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let mut w = Vec::new();
    unsafe {
        unsafe extern "system" fn ep(hwnd: HWND, lp: isize) -> i32 {
            let w = &mut *(lp as *mut Vec<WindowInfo>);
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            if style & 0x10000000 == 0 { return 1i32; }
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len == 0 { return 1i32; }
            let title = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
            if title.is_empty() { return 1i32; }
            let mut cb = [0u16; 256];
            let cl = GetClassNameW(hwnd, &mut cb);
            let class = if cl > 0 { String::from_utf16_lossy(&cb[..cl as usize]) } else { String::new() };
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r);
            let reg = Region::new(r.left,r.top,(r.right-r.left)as u32,(r.bottom-r.top)as u32);
            if !reg.is_valid() { return 1i32; }
            let mut pid: u32 = 0;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let is_ppt = class.eq_ignore_ascii_case("screenClass")||title.contains("Slide Show");
            w.push(WindowInfo{
                hwnd: hwnd.0 as u64, title, class_name:class, region:reg,
                monitor_hmonitor:0, is_visible:true, is_minimized:(style & 0x20000000)!=0,
                is_powerpoint:is_ppt, process_id:pid, process_name:format!("{}",pid),
            });
            1i32
        }
        let ptr: usize = ep as extern "system" fn(_,_) -> i32 as usize;
        let cbf: WNDENUMPROC = std::mem::transmute(ptr);
        EnumWindows(cbf, &mut w as *mut _ as isize).ok().context("EnumWindows")?;
    }
    w.retain(|w| w.is_valid() && !w.title.is_empty());
    Ok(w)
}
