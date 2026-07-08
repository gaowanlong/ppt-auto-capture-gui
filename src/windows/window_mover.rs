//! Move windows between monitors.

use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::model::Region;

/// Move a window to a specific monitor, centering it on that display.
pub fn move_window_to_monitor(hwnd: u64, monitor_region: &Region) -> Result<()> {
    let hwnd = HWND(hwnd as isize);

    unsafe {
        // Restore if minimized
        if IsIconic(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE);
        }

        // Get current window size
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)
            .context("GetWindowRect failed")?;

        let win_w = (rect.right - rect.left) as u32;
        let win_h = (rect.bottom - rect.top) as u32;

        // Calculate centered position on target monitor
        let new_x = monitor_region.x + (monitor_region.width as i32 - win_w as i32) / 2;
        let new_y = monitor_region.y + (monitor_region.height as i32 - win_h as i32) / 2;

        // Clamp so window isn't off-screen
        let new_x = new_x.max(monitor_region.x);
        let new_y = new_y.max(monitor_region.y);

        info!("Moving window HWND={} to ({}, {}) on monitor ({},{} {}x{})",
            hwnd.0, new_x, new_y,
            monitor_region.x, monitor_region.y,
            monitor_region.width, monitor_region.height);

        // Move the window
        SetWindowPos(
            hwnd,
            HWND_TOP,
            new_x,
            new_y,
            win_w as i32,
            win_h as i32,
            SWP_NOZORDER | SWP_SHOWWINDOW,
        )
        .context("SetWindowPos failed")?;

        // Bring to foreground
        SetForegroundWindow(hwnd);
    }

    Ok(())
}

/// Maximize a window.
pub fn maximize_window(hwnd: u64) -> Result<()> {
    let hwnd = HWND(hwnd as isize);
    unsafe {
        ShowWindow(hwnd, SW_MAXIMIZE)
            .ok()
            .context("ShowWindow maximize failed")?;
        SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// Get the window rectangle.
pub fn get_window_rect(hwnd: u64) -> Result<Region> {
    let hwnd = HWND(hwnd as isize);
    unsafe {
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)
            .context("GetWindowRect failed")?;
        Ok(Region::new(
            rect.left, rect.top,
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        ))
    }
}
