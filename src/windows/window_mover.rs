use anyhow::Result;
use log::info;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::model::Region;

pub fn move_window_to_monitor(hwnd: u64, mr: &Region) -> Result<()> {
    let h = HWND(hwnd as isize);
    unsafe {
        if IsIconic(h).as_bool() { ShowWindow(h, SW_RESTORE); }
        let mut r = RECT::default();
        GetWindowRect(h, &mut r)?;
        let (ww, wh) = (r.right-r.left, r.bottom-r.top);
        let x = (mr.x + (mr.width as i32 - ww)/2).max(mr.x);
        let y = (mr.y + (mr.height as i32 - wh)/2).max(mr.y);
        SetWindowPos(h, HWND(0), x, y, ww, wh, SWP_NOZORDER | SWP_SHOWWINDOW)?;
        SetForegroundWindow(h);
    }
    Ok(())
}
pub fn maximize_window(hwnd: u64) -> Result<()> {
    unsafe { ShowWindow(HWND(hwnd as isize), SW_MAXIMIZE); }
    Ok(())
}
