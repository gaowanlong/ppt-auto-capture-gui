use anyhow::Result;
use crate::model::Region;

#[link(name = "user32")]
extern "system" {
    fn IsIconic(hWnd: isize) -> i32;
    fn GetWindowRect(hWnd: isize, lpRect: *mut u8) -> i32;
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
