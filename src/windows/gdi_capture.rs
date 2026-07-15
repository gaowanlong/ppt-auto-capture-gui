use anyhow::Result;
use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC, CreateCompatibleDC, DeleteDC, CreateCompatibleBitmap,
    SelectObject, DeleteObject, BitBlt, GetDIBits, BITMAPINFO, BITMAPINFOHEADER,
    ROP_CODE, DIB_USAGE,
};
use crate::model::{Frame, MonitorInfo, Region};

/// The SRCCOPY raster operation code (0x00CC0020).
const SRCCOPY: ROP_CODE = ROP_CODE(0x00CC0020u32);

pub struct GdiCapturer {
    region: Region, frame_index: u64,
}
impl GdiCapturer {
    pub fn new() -> Self { Self { region: Region::new(0,0,0,0), frame_index: 0 } }
    pub fn initialize(&mut self, mon: &MonitorInfo) -> Result<()> { self.region = mon.region; self.frame_index = 0; Ok(()) }
    pub fn capture_frame(&mut self) -> Result<Frame> {
        let (w, h) = (self.region.width, self.region.height);
        unsafe {
            let sdc = GetDC(None);
            if sdc.is_invalid() { return Err(anyhow::anyhow!("GetDC")); }
            let mdc = CreateCompatibleDC(Some(sdc));
            if mdc.is_invalid() { let _ = ReleaseDC(None, sdc); return Err(anyhow::anyhow!("CreateCompatibleDC")); }
            let bmp = CreateCompatibleBitmap(sdc, w as i32, h as i32);
            if bmp.is_invalid() { let _ = ReleaseDC(None, sdc); let _ = DeleteDC(mdc); return Err(anyhow::anyhow!("CreateCompatibleBitmap")); }
            SelectObject(mdc, bmp.into());
            let _ = BitBlt(mdc, 0, 0, w as i32, h as i32, Some(sdc), self.region.x, self.region.y, SRCCOPY);
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
