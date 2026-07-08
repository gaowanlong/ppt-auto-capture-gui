use anyhow::{Context, Result};
use log::info;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::model::Region;

pub fn move_window_to_monitor(hwnd: u64, monitor_region: &Region) -> Result<()> {
    let hwnd = HWND(hwnd as isize);
    unsafe {
        if IsIconic(hwnd).as_bool() { ShowWindow(hwnd, SW_RESTORE); }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).context("GetWindowRect")?;
        let w = (rect.right - rect.left) as i32;
        let h = (rect.bottom - rect.top) as i32;
        let x = (monitor_region.x + (monitor_region.width as i32 - w) / 2).max(monitor_region.x);
        let y = (monitor_region.y + (monitor_region.height as i32 - h) / 2).max(monitor_region.y);
        info!("Moving window to ({}, {})", x, y);
        SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW).context("SetWindowPos")?;
        SetForegroundWindow(hwnd);
    }
    Ok(())
}

pub fn maximize_window(hwnd: u64) -> Result<()> {
    unsafe { ShowWindow(HWND(hwnd as isize), SW_MAXIMIZE).ok().context("ShowWindow")?; }
    Ok(())
}
