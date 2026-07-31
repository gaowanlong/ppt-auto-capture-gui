use anyhow::Result;

use crate::model::{Frame, MonitorInfo, Region, WindowInfo};

mod adapter;

pub const fn backend_name() -> &'static str {
    "macos-xcap"
}

pub struct DxgiCapturer;

impl DxgiCapturer {
    pub fn new() -> Self {
        Self
    }

    pub fn initialize(&mut self, _monitor: &MonitorInfo) -> Result<()> {
        Err(anyhow::anyhow!("macOS display capture is not initialized"))
    }

    pub fn capture_frame(&mut self, _timeout_ms: u32) -> Result<Option<Frame>> {
        Ok(None)
    }

    pub fn release(&mut self) {}

    pub fn is_initialized(&self) -> bool {
        false
    }
}

pub struct GdiCapturer;

impl GdiCapturer {
    pub fn new() -> Self {
        Self
    }

    pub fn initialize(&mut self, _monitor: &MonitorInfo) -> Result<()> {
        Ok(())
    }

    pub fn capture_frame(&mut self) -> Result<Frame> {
        Err(anyhow::anyhow!("macOS window capture is not initialized"))
    }

    pub fn is_initialized(&self) -> bool {
        false
    }

    pub fn set_window_hwnd(&mut self, _window_id: u64, _client_w: u32, _client_h: u32) {}
}

pub struct SessionEventMonitor;

impl SessionEventMonitor {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&mut self) {}

    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> {
        let (_sender, receiver) = crossbeam_channel::unbounded();
        receiver
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Unlocked,
    Locked,
}

pub fn get_client_window_rect(_window_id: u64) -> Result<Region> {
    Err(anyhow::anyhow!(
        "get_client_window_rect is not implemented on macOS"
    ))
}

pub fn get_window_rect(_window_id: u64) -> Result<Region> {
    Err(anyhow::anyhow!("get_window_rect is not implemented on macOS"))
}

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    Ok(Vec::new())
}

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    Ok(Vec::new())
}

pub fn move_window_to_monitor(_window_id: u64, _region: &Region) -> Result<()> {
    Err(anyhow::anyhow!(
        "Moving windows is not supported on macOS"
    ))
}

pub fn maximize_window(_window_id: u64) -> Result<()> {
    Err(anyhow::anyhow!(
        "Maximizing windows is not supported on macOS"
    ))
}
