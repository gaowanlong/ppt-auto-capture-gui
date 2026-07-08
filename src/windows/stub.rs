//! Stub implementation for non-Windows platforms.
//! Provides empty implementations and mock data for compilation.

use anyhow::Result;

use crate::model::{MonitorInfo, WindowInfo, Region};

pub struct DxgiCapturer;
impl DxgiCapturer {
    pub fn new() -> Self { Self }
    pub fn initialize(&mut self, _monitor: &MonitorInfo) -> Result<()> { Ok(()) }
    pub fn capture_frame(&mut self, _timeout_ms: u32) -> Result<Option<crate::model::Frame>> { Ok(None) }
    pub fn release(&mut self) {}
    pub fn is_initialized(&self) -> bool { false }
}

pub struct GdiCapturer;
impl GdiCapturer {
    pub fn new() -> Self { Self }
    pub fn initialize(&mut self, _monitor: &MonitorInfo) -> Result<()> { Ok(()) }
    pub fn capture_frame(&mut self) -> Result<crate::model::Frame> {
        Err(anyhow::anyhow!("GDI not available on this platform"))
    }
    pub fn is_initialized(&self) -> bool { false }
}

pub struct SessionEventMonitor;
impl SessionEventMonitor {
    pub fn new() -> Self { Self }
    pub fn start(&mut self) {}
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> {
        let (tx, rx) = crossbeam_channel::unbounded();
        std::mem::drop(tx);
        rx
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Unlocked,
    Locked,
    Disconnected,
    Reconnected,
}

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    Ok(vec![
        MonitorInfo {
            hmonitor: 1,
            adapter_name: "Mock Adapter".into(),
            output_name: "\\\\.\\DISPLAY1".into(),
            description: "Mock Display (non-Windows build)".into(),
            region: Region::new(0, 0, 1920, 1080),
            is_primary: true,
            is_virtual_suspect: false,
            output_index: 0,
            adapter_index: 0,
        }
    ])
}

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    Ok(vec![
        WindowInfo {
            hwnd: 0xDEAD,
            title: "Mock PowerPoint Slide Show".into(),
            class_name: "screenClass".into(),
            region: Region::new(0, 0, 1920, 1080),
            monitor_hmonitor: 1,
            is_visible: true,
            is_minimized: false,
            is_powerpoint: true,
            process_id: 1234,
            process_name: "POWERPNT.EXE".into(),
        }
    ])
}

pub fn move_window_to_monitor(_hwnd: u64, _monitor_region: &Region) -> Result<()> {
    Ok(())
}

pub fn maximize_window(_hwnd: u64) -> Result<()> {
    Ok(())
}
