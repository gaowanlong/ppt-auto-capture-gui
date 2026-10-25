use crate::model::{Frame, MonitorInfo, Region};
use anyhow::{ensure, Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    GetWindowDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP,
    HDC, HGDIOBJ, SRCCOPY,
};

#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hwnd: isize, dc: isize, flags: u32) -> i32;
}

/// Own every native resource, including on capture/decoding failure.
struct BitmapCapture {
    owner: Option<HWND>,
    source: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl BitmapCapture {
    fn new(owner: Option<HWND>, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= i32::MAX as u32 && height <= i32::MAX as u32,
            "Invalid capture dimensions"
        );
        unsafe {
            let source = if owner.is_some() {
                GetWindowDC(owner)
            } else {
                GetDC(None)
            };
            ensure!(!source.is_invalid(), "GetDC/GetWindowDC failed");
            let mut capture = Self {
                owner,
                source,
                memory: HDC::default(),
                bitmap: HBITMAP::default(),
                previous: HGDIOBJ::default(),
            };
            capture.memory = CreateCompatibleDC(Some(source));
            ensure!(!capture.memory.is_invalid(), "CreateCompatibleDC failed");
            capture.bitmap = CreateCompatibleBitmap(source, width as i32, height as i32);
            ensure!(
                !capture.bitmap.is_invalid(),
                "CreateCompatibleBitmap failed"
            );
            capture.previous = SelectObject(capture.memory, capture.bitmap.into());
            ensure!(
                !capture.previous.is_invalid() && capture.previous.0 as isize != -1,
                "SelectObject failed"
            );
            Ok(capture)
        }
    }

    fn pixels(&mut self, width: u32, height: u32) -> Result<Vec<u8>> {
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .context("Capture buffer size overflow")?;
        let mut data = Vec::new();
        data.try_reserve_exact(bytes)
            .context("Insufficient memory for screenshot")?;
        data.resize(bytes, 0);
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        };
        unsafe {
            // GetDIBits requires that the bitmap NOT be selected into any DC.
            let restored = SelectObject(self.memory, self.previous);
            ensure!(
                !restored.is_invalid() && restored.0 as isize != -1,
                "Failed to deselect capture bitmap"
            );
            self.previous = HGDIOBJ::default();
            let lines = GetDIBits(
                self.memory,
                self.bitmap,
                0,
                height,
                Some(data.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            ensure!(
                lines == height as i32,
                "GetDIBits returned incomplete pixels: {lines}/{height}"
            );
        }
        Ok(data)
    }
}

impl Drop for BitmapCapture {
    fn drop(&mut self) {
        unsafe {
            if !self.memory.is_invalid() {
                if !self.previous.is_invalid() && self.previous.0 as isize != -1 {
                    let _ = SelectObject(self.memory, self.previous);
                }
                // Delete DC before bitmap even if restoring the old object failed.
                let _ = DeleteDC(self.memory);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.source.is_invalid() {
                let _ = ReleaseDC(self.owner, self.source);
            }
        }
    }
}

pub fn capture_window_content(hwnd: u64, width: u32, height: u32) -> Result<Vec<u8>> {
    let owner = HWND(hwnd as usize as *mut std::ffi::c_void);
    let mut capture = BitmapCapture::new(Some(owner), width, height)?;
    unsafe {
        ensure!(
            PrintWindow(hwnd as isize, capture.memory.0 as isize, 1) != 0
                || PrintWindow(hwnd as isize, capture.memory.0 as isize, 0) != 0,
            "PrintWindow failed for HWND 0x{hwnd:X}"
        );
    }
    capture.pixels(width, height)
}

pub struct GdiCapturer {
    region: Region,
    frame_index: u64,
    window_hwnd: u64,
    window_width: u32,
    window_height: u32,
}

impl GdiCapturer {
    pub fn new() -> Self {
        Self {
            region: Region::new(0, 0, 0, 0),
            frame_index: 0,
            window_hwnd: 0,
            window_width: 0,
            window_height: 0,
        }
    }
    pub fn initialize(&mut self, mon: &MonitorInfo) -> Result<()> {
        self.region = mon.region;
        Ok(())
    }
    pub fn set_window_hwnd(&mut self, hwnd: u64, width: u32, height: u32) {
        self.window_hwnd = hwnd;
        self.window_width = width;
        self.window_height = height;
    }
    pub fn region(&self) -> &Region {
        &self.region
    }
    pub fn is_initialized(&self) -> bool {
        self.region.is_valid()
    }
    pub fn capture_frame(&mut self) -> Result<Frame> {
        let (width, height, data) = if self.window_hwnd != 0 {
            ensure!(
                self.window_width > 0 && self.window_height > 0,
                "Selected window has no capture dimensions"
            );
            (
                self.window_width,
                self.window_height,
                capture_window_content(self.window_hwnd, self.window_width, self.window_height)?,
            )
        } else {
            let (w, h) = (self.region.width, self.region.height);
            let mut capture = BitmapCapture::new(None, w, h)?;
            unsafe {
                BitBlt(
                    capture.memory,
                    0,
                    0,
                    w as i32,
                    h as i32,
                    Some(capture.source),
                    self.region.x,
                    self.region.y,
                    SRCCOPY,
                )
                .context("BitBlt failed; desktop may be locked or unavailable")?;
            }
            (w, h, capture.pixels(w, h)?)
        };
        self.frame_index += 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Ok(Frame::new(
            data,
            width,
            height,
            width * 4,
            self.frame_index,
            now,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};

    #[test]
    fn repeated_bitmap_success_and_early_failure_release_gdi_handles() {
        unsafe {
            let before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            for _ in 0..2000 {
                let mut capture = BitmapCapture::new(None, 32, 32).unwrap();
                capture.pixels(32, 32).unwrap();
                drop(capture);
                // Early return while the bitmap is still selected.
                drop(BitmapCapture::new(None, 32, 32).unwrap());
            }
            let after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            assert!(
                after <= before + 4,
                "Leaked GDI handles: before={before}, after={after}"
            );
        }
    }
}
