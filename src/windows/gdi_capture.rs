use anyhow::{Context, Result};
use log::info;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::model::{Frame, MonitorInfo, Region};

pub struct GdiCapturer {
    region: Region,
    frame_index: u64,
}

impl GdiCapturer {
    pub fn new() -> Self { Self { region: Region::new(0,0,0,0), frame_index: 0 } }
    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        self.region = monitor.region;
        self.frame_index = 0;
        Ok(())
    }
    pub fn capture_frame(&mut self) -> Result<Frame> {
        let w = self.region.width; let h = self.region.height;
        unsafe {
            let hdc_screen = GetDC(None).context("GetDC")?;
            let hdc_mem = CreateCompatibleDC(hdc_screen).context("CreateCompatibleDC")?;
            let hbmp = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32).context("CreateCompatibleBitmap")?;
            let _old = SelectObject(hdc_mem, hbmp);
            if BitBlt(hdc_mem, 0, 0, w as i32, h as i32, hdc_screen, self.region.x, self.region.y, SRCCOPY.0).is_err() {
                let _ = DeleteObject(hbmp); let _ = ReleaseDC(None, hdc_screen); let _ = DeleteDC(hdc_mem);
                return Err(anyhow::anyhow!("BitBlt failed"));
            }
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w as i32;
            bi.bmiHeader.biHeight = -(h as i32);
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB;
            let mut data = vec![0u8; (w * h * 4) as usize];
            GetDIBits(hdc_mem, hbmp, 0, h, Some(data.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            let _ = DeleteObject(hbmp); let _ = ReleaseDC(None, hdc_screen); let _ = DeleteDC(hdc_mem);
            self.frame_index += 1;
            Ok(Frame::new(data, w, h, w*4, self.frame_index, now_ms()))
        }
    }
    pub fn is_initialized(&self) -> bool { self.region.is_valid() }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
