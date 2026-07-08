use anyhow::{Context, Result};
use crate::model::{Frame, MonitorInfo};
/// DXGI capturer stub — always returns an error, forcing GDI fallback.
/// The windows crate 0.60 on MSVC has type resolution issues with COM casting
/// and BOOL-dependent callback types. GDI capture is used instead.
pub struct DxgiCapturer;
impl DxgiCapturer {
    pub fn new() -> Self { Self }
    pub fn initialize(&mut self, _mon: &MonitorInfo) -> Result<()> {
        Err(anyhow::anyhow!("DXGI not available in this build"))
    }
    pub fn capture_frame(&mut self, _timeout: u32) -> Result<Option<Frame>> {
        Err(anyhow::anyhow!("DXGI not available"))
    }
    pub fn release(&mut self) {}
    pub fn is_initialized(&self) -> bool { false }
}
