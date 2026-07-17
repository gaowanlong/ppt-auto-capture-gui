use anyhow::Result;
use crate::model::Region;

#[link(name = "user32")]
extern "system" {
    fn IsIconic(hWnd: isize) -> i32;
    fn GetWindowRect(hWnd: isize, lpRect: *mut u8) -> i32;
    fn GetClientRect(hWnd: isize, lpRect: *mut u8) -> i32;
    fn ClientToScreen(hWnd: isize, lpPoint: *mut u8) -> i32;
    fn SetWindowPos(hWnd: isize, hWndInsertAfter: isize, x: i32, y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
    fn SetForegroundWindow(hWnd: isize) -> i32;
    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
}

#[repr(C)]
struct MyRect { left: i32, top: i32, right: i32, bottom: i32 }

pub fn move_window_to_monitor(hwnd: u64, mr: &Region) -> Result<()> {
    let h = hwnd as isize;
    unsafe {
        if IsIconic(h) != 0 { ShowWindow(h, 9i32); }
        let mut r = MyRect { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(h, &mut r as *mut MyRect as *mut u8);
        let ww = r.right - r.left;
        let wh = r.bottom - r.top;
        SetWindowPos(h, 0isize, mr.x, mr.y, ww, wh, 0x0044u32);
        SetForegroundWindow(h);
    }
    Ok(())
}

pub fn maximize_window(hwnd: u64) -> Result<()> {
    unsafe { ShowWindow(hwnd as isize, 3i32); }
    Ok(())
}

/// Get the bounding rectangle of a window in screen coordinates (includes shadow/title bar).
pub fn get_window_rect(hwnd: u64) -> Result<Region> {
    unsafe {
        let mut r = MyRect { left: 0, top: 0, right: 0, bottom: 0 };
        let ret = GetWindowRect(hwnd as isize, &mut r as *mut MyRect as *mut u8);
        if ret != 0 {
            Ok(Region::new(r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32))
        } else {
            Err(anyhow::anyhow!("GetWindowRect failed for HWND 0x{:X}", hwnd))
        }
    }
}

/// Get the CLIENT area of a window in screen coordinates (excludes title bar, borders, shadow).
/// This is the actual pixel content area, accurate for BitBlt capture.
pub fn get_client_window_rect(hwnd: u64) -> Result<Region> {
    unsafe {
        let mut cr = MyRect { left: 0, top: 0, right: 0, bottom: 0 };
        let ret = GetClientRect(hwnd as isize, &mut cr as *mut MyRect as *mut u8);
        if ret == 0 {
            return Err(anyhow::anyhow!("GetClientRect failed for HWND 0x{:X}", hwnd));
        }
        // Client rect is relative to window client area, we need to convert to screen coords
        let mut pt_arr: [i32; 2] = [0, 0]; // POINT { x, y }
        let pt_ok = ClientToScreen(hwnd as isize, &mut pt_arr as *mut i32 as *mut u8);
        if pt_ok == 0 {
            return Err(anyhow::anyhow!("ClientToScreen failed for HWND 0x{:X}", hwnd));
        }
        let left = pt_arr[0];
        let top = pt_arr[1];
        let w = (cr.right - cr.left) as u32;
        let h = (cr.bottom - cr.top) as u32;
        if w == 0 || h == 0 {
            return Err(anyhow::anyhow!("Window has zero client area for HWND 0x{:X}", hwnd));
        }
        Ok(Region::new(left, top, w, h))
    }
}
