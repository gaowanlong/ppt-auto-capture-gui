use anyhow::{Context, Result};
use log::info;
use windows::Win32::Foundation::{BOOL, HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SetForegroundWindow, ShowWindow,
    GetWindowRect, IsIconic,
    SWP_NOZORDER, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_RESTORE,
};
use crate::model::Region;

pub fn move_window_to_monitor(hwnd: u64, monitor_region: &Region) -> Result<()> {
    let h = HWND(hwnd as isize);
    unsafe {
        if IsIconic(h).as_bool() { ShowWindow(h, SW_RESTORE); }
        let mut r = RECT::default();
        GetWindowRect(h, &mut r).context("GetWindowRect")?;
        let w = r.right - r.left; let hh = r.bottom - r.top;
        let x = (monitor_region.x + (monitor_region.width as i32 - w) / 2).max(monitor_region.x);
        let y = (monitor_region.y + (monitor_region.height as i32 - hh) / 2).max(monitor_region.y);
        info!("Move window to ({},{})", x, y);
        SetWindowPos(h, HWND(0), x, y, w, hh, SWP_NOZORDER | SWP_SHOWWINDOW).context("SetWindowPos")?;
        SetForegroundWindow(h);
    }
    Ok(())
}

pub fn maximize_window(hwnd: u64) -> Result<()> {
    unsafe { ShowWindow(HWND(hwnd as isize), SW_MAXIMIZE); }
    Ok(())
}
