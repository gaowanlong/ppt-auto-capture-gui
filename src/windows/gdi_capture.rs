//! GDI-based fallback capture using BitBlt.
//! Used when DXGI is unavailable or fails (e.g. on older Windows or RDP sessions).

use anyhow::{Context, Result};
use log::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::model::{Frame, Region, MonitorInfo};

/// GDI-based screen capturer (fallback).
pub struct GdiCapturer {
    hmonitor: u64,
    region: Region,
    frame_index: u64,
}

impl GdiCapturer {
    pub fn new() -> Self {
        Self {
            hmonitor: 0,
            region: Region::new(0, 0, 0, 0),
            frame_index: 0,
        }
    }

    /// Initialize for a specific monitor.
    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        self.hmonitor = monitor.hmonitor;
        self.region = monitor.region;
        self.frame_index = 0;
        info!("GDI capturer initialized for monitor {} ({}x{})",
            monitor.output_name.trim(), monitor.region.width, monitor.region.height);
        Ok(())
    }

    /// Capture the current screen content using BitBlt.
    pub fn capture_frame(&mut self) -> Result<Frame> {
        let w = self.region.width;
        let h = self.region.height;

        unsafe {
            // Get screen DC
            let hdc_screen = GetDC(None)
                .context("Failed to get screen DC")?;

            // Create compatible DC
            let hdc_mem = CreateCompatibleDC(hdc_screen)
                .context("Failed to create compatible DC")?;

            // Create compatible bitmap
            let hbmp = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32)
                .context("Failed to create compatible bitmap")?;

            // Select bitmap into memory DC
            let _old = SelectObject(hdc_mem, hbmp);

            // BitBlt from screen to memory DC
            if BitBlt(
                hdc_mem,
                0, 0,
                w as i32, h as i32,
                hdc_screen,
                self.region.x, self.region.y,
                SRCCOPY,
            ).is_err() {
                // Cleanup
                let _ = SelectObject(hdc_mem, _old);
                let _ = DeleteObject(hbmp);
                let _ = ReleaseDC(None, hdc_screen);
                let _ = DeleteDC(hdc_mem);
                return Err(anyhow::anyhow!("BitBlt failed for monitor capture"));
            }

            // Get bitmap info
            let mut bmp_info = BITMAPINFO::default();
            bmp_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmp_info.bmiHeader.biWidth = w as i32;
            bmp_info.bmiHeader.biHeight = -(h as i32); // Top-down bitmap
            bmp_info.bmiHeader.biPlanes = 1;
            bmp_info.bmiHeader.biBitCount = 32;
            bmp_info.bmiHeader.biCompression = BI_RGB;

            let mut data = vec![0u8; (w * h * 4) as usize];

            let result = GetDIBits(
                hdc_mem,
                hbmp,
                0,
                h,
                Some(data.as_mut_ptr() as *mut _),
                &mut bmp_info,
                DIB_RGB_COLORS,
            );

            // Cleanup
            let _ = SelectObject(hdc_mem, _old);
            let _ = DeleteObject(hbmp);
            let _ = ReleaseDC(None, hdc_screen);
            let _ = DeleteDC(hdc_mem);

            if result == 0 {
                return Err(anyhow::anyhow!("GetDIBits failed"));
            }

            self.frame_index += 1;

            // GDI returns BGR (24-bit) or BGRA (32-bit) data. We convert stride.
            let stride = w * 4;

            let frame = Frame::new(
                data,
                w, h,
                stride,
                self.frame_index,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );

            Ok(frame)
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.hmonitor != 0
    }
}
