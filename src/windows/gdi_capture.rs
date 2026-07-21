use anyhow::Result;
use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC, CreateCompatibleDC, DeleteDC, CreateCompatibleBitmap,
    SelectObject, DeleteObject, BitBlt, GetDIBits, GetDeviceCaps, GET_DEVICE_CAPS_INDEX, BITMAPINFO, BITMAPINFOHEADER,
    ROP_CODE, DIB_USAGE, HDC,
};
use crate::model::{Frame, MonitorInfo, Region};

#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hWnd: isize, hDCBlt: isize, nFlags: u32) -> i32;
    fn GetWindowDC(hWnd: isize) -> isize;
}

/// Capture a specific window's content using PrintWindow (ignores overlapping windows).
/// This is the preferred capture method when a window is selected.
pub fn capture_window_content(hwnd: u64, width: u32, height: u32) -> Result<Vec<u8>> {
    anyhow::ensure!(width > 0 && height > 0, "Invalid capture dimensions for PrintWindow");
    unsafe {
        let hwnd_i = hwnd as isize;
        let wdc_val = GetWindowDC(hwnd_i);
        if wdc_val == 0 {
            return Err(anyhow::anyhow!("GetWindowDC failed for HWND 0x{:X}", hwnd));
        }
        let wdc = HDC(wdc_val as *mut std::ffi::c_void);
        
        let mdc = CreateCompatibleDC(Some(wdc));
        if mdc.is_invalid() {
            let _ = ReleaseDC(None, wdc);
            return Err(anyhow::anyhow!("CreateCompatibleDC failed"));
        }
        
        let bmp = CreateCompatibleBitmap(wdc, width as i32, height as i32);
        if bmp.is_invalid() {
            let _ = ReleaseDC(None, wdc);
            let _ = DeleteDC(mdc);
            return Err(anyhow::anyhow!("CreateCompatibleBitmap failed"));
        }
        SelectObject(mdc, bmp.into());
        
        // PrintWindow captures only the target window content
        // PW_CLIENT_ONLY = 0x1 captures only the window client area (excludes title bar)
        let pw_ret = PrintWindow(hwnd_i, mdc.0 as isize, 0x1);
        if pw_ret == 0 {
            // Try without PW_CLIENT_ONLY as fallback
            let pw_ret2 = PrintWindow(hwnd_i, mdc.0 as isize, 0);
            if pw_ret2 == 0 {
                let _ = ReleaseDC(None, wdc);
                let _ = DeleteObject(bmp.into());
                let _ = DeleteDC(mdc);
                return Err(anyhow::anyhow!("PrintWindow failed for HWND 0x{:X}", hwnd));
            }
        }
        
        // Read pixel data from the bitmap
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = width as i32;
        bi.bmiHeader.biHeight = -(height as i32);
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = 0;
        
        let mut data = vec![0u8; (width * height * 4) as usize];
        GetDIBits(mdc, bmp, 0, height, Some(data.as_mut_ptr() as *mut _), &mut bi, DIB_USAGE(0));
        
        let _ = ReleaseDC(None, wdc);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mdc);
        Ok(data)
    }
}

/// The SRCCOPY raster operation code (0x00CC0020).
const SRCCOPY: ROP_CODE = ROP_CODE(0x00CC0020u32);


pub struct GdiCapturer {
    region: Region, frame_index: u64, window_hwnd: u64,
    /// Client area dimensions (for PrintWindow-based capture, RDP compatible).
    window_width: u32, window_height: u32,
}
impl GdiCapturer {
    pub fn new() -> Self { Self { region: Region::new(0,0,0,0), frame_index: 0, window_hwnd: 0, window_width: 0, window_height: 0 } }
    pub fn initialize(&mut self, mon: &MonitorInfo) -> Result<()> { self.region = mon.region; self.frame_index = 0; Ok(()) }
    pub fn set_window_hwnd(&mut self, hwnd: u64, client_w: u32, client_h: u32) { self.window_hwnd = hwnd; self.window_width = client_w; self.window_height = client_h; }
    pub fn region(&self) -> &Region { &self.region }
    pub fn capture_frame(&mut self) -> Result<Frame> {
        // When a window HWND is set, use PrintWindow (works in RDP; BitBlt fails there).
        if self.window_hwnd != 0 && self.window_width > 0 && self.window_height > 0 {
            return self.capture_window_with_printwindow();
        }
        self.capture_screen_with_bitblt()
    }

    /// Capture a specific window via PrintWindow (RDP-compatible, ignores overlapping windows).
    fn capture_window_with_printwindow(&mut self) -> Result<Frame> {
        let w = self.window_width;
        let h = self.window_height;
        let data = capture_window_content(self.window_hwnd, w, h)?;
        self.frame_index += 1;
        Ok(Frame::new(data, w, h, w * 4, self.frame_index, now_ms()))
    }

    /// Capture the full monitor region via BitBlt from screen DC.
    /// Note: may fail in RDP or locked desktop sessions.
    fn capture_screen_with_bitblt(&mut self) -> Result<Frame> {
        let (w, h) = (self.region.width, self.region.height);
        if w == 0 || h == 0 {
            return Err(anyhow::anyhow!("Empty capture region: {}x{}", w, h));
        }
        unsafe {
            let sdc = GetDC(None);
            if sdc.is_invalid() { return Err(anyhow::anyhow!("GetDC")); }

            // DPI diagnostic: log the screen DC dimensions vs capture region
            let dpi_x = GetDeviceCaps(Some(sdc), GET_DEVICE_CAPS_INDEX(88));
            let dpi_y = GetDeviceCaps(Some(sdc), GET_DEVICE_CAPS_INDEX(90));
            if dpi_x != 96 || dpi_y != 96 {
                let dc_w = GetDeviceCaps(Some(sdc), GET_DEVICE_CAPS_INDEX(8)) as u32;
                let dc_h = GetDeviceCaps(Some(sdc), GET_DEVICE_CAPS_INDEX(10)) as u32;
                log::debug!(
                    "GDI capture: region={}x{}@{},{} DC={}x{} DPI={}x{}",
                    w, h, self.region.x, self.region.y, dc_w, dc_h, dpi_x, dpi_y
                );
            }

            let mdc = CreateCompatibleDC(Some(sdc));
            if mdc.is_invalid() { let _ = ReleaseDC(None, sdc); return Err(anyhow::anyhow!("CreateCompatibleDC")); }
            let bmp = CreateCompatibleBitmap(sdc, w as i32, h as i32);
            if bmp.is_invalid() { let _ = ReleaseDC(None, sdc); let _ = DeleteDC(mdc); return Err(anyhow::anyhow!("CreateCompatibleBitmap")); }
            SelectObject(mdc, bmp.into());
            let blt_result = BitBlt(mdc, 0, 0, w as i32, h as i32, Some(sdc), self.region.x, self.region.y, SRCCOPY);
            // BitBlt can fail in RDP or locked sessions — return a clear error instead of silent black frame
            if blt_result.is_err() {
                let _ = DeleteObject(bmp.into()); let _ = ReleaseDC(None, sdc); let _ = DeleteDC(mdc);
                return Err(anyhow::anyhow!("BitBlt failed (RDP or locked session). Try selecting a window for PrintWindow capture."));
            }
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w as i32; bi.bmiHeader.biHeight = -(h as i32);
            bi.bmiHeader.biPlanes = 1; bi.bmiHeader.biBitCount = 32; bi.bmiHeader.biCompression = 0;
            let mut data = vec![0u8; (w * h * 4) as usize];
            GetDIBits(mdc, bmp, 0, h, Some(data.as_mut_ptr() as *mut _), &mut bi, DIB_USAGE(0));
            let _ = DeleteObject(bmp.into()); let _ = ReleaseDC(None, sdc); let _ = DeleteDC(mdc);
            self.frame_index += 1;
            Ok(Frame::new(data, w, h, w*4, self.frame_index, now_ms()))
        }
    }
    pub fn is_initialized(&self) -> bool { self.region.is_valid() }
}
fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }
