use anyhow::Result;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SetForegroundWindow, ShowWindow, IsIconic, GetWindowRect,
    SWP_NOZORDER, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_RESTORE,
};
use crate::model::Region;
pub fn move_window_to_monitor(hwnd: u64, mr: &Region) -> Result<()> {
    let h = HWND(hwnd as isize);
    unsafe {
        if IsIconic(h).0 != 0 { ShowWindow(h, SW_RESTORE); }
        let mut r = RECT::default();
        let _ = GetWindowRect(h, &mut r);
        let (ww, wh) = (r.right-r.left, r.bottom-r.top);
        SetWindowPos(h, HWND(0), mr.x, mr.y, ww, wh, SWP_NOZORDER | SWP_SHOWWINDOW);
        SetForegroundWindow(h);
    }
    Ok(())
}
pub fn maximize_window(hwnd: u64) -> Result<()> {
    unsafe { ShowWindow(HWND(hwnd as isize), SW_MAXIMIZE); }
    Ok(())
}
