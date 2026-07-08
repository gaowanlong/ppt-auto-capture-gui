use anyhow::Result;
use crate::model::{MonitorInfo, WindowInfo, Region};

pub struct DxgiCapturer;
impl DxgiCapturer {
    pub fn new() -> Self { Self }
    pub fn initialize(&mut self, _mon: &MonitorInfo) -> Result<()> { Err(anyhow::anyhow!("DXGI stub")) }
    pub fn capture_frame(&mut self, _t: u32) -> Result<Option<crate::model::Frame>> { Ok(None) }
    pub fn release(&mut self) {}
    pub fn is_initialized(&self) -> bool { false }
}
pub struct GdiCapturer;
impl GdiCapturer {
    pub fn new() -> Self { Self }
    pub fn initialize(&mut self, _mon: &MonitorInfo) -> Result<()> { Ok(()) }
    pub fn capture_frame(&mut self) -> Result<crate::model::Frame> { Err(anyhow::anyhow!("GDI stub")) }
    pub fn is_initialized(&self) -> bool { false }
}
pub struct SessionEventMonitor;
impl SessionEventMonitor {
    pub fn new() -> Self { Self }
    pub fn start(&mut self) {}
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> { let (tx,rx)=crossbeam_channel::unbounded();rx }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SessionState { Unlocked, Locked }
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    Ok(vec![MonitorInfo{
        hmonitor:1, adapter_name:"Mock".into(), output_name:"DISPLAY1".into(),
        description:"Mock".into(), region:Region::new(0,0,1920,1080),
        is_primary:true, is_virtual_suspect:false, output_index:0, adapter_index:0,
    }])
}
pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    Ok(vec![WindowInfo{
        hwnd:1, title:"Mock".into(), class_name:"screenClass".into(),
        region:Region::new(0,0,1920,1080), monitor_hmonitor:1,
        is_visible:true, is_minimized:false, is_powerpoint:true, process_id:0, process_name:"".into(),
    }])
}
pub fn move_window_to_monitor(_:u64, _:&Region) -> Result<()> { Ok(()) }
pub fn maximize_window(_:u64) -> Result<()> { Ok(()) }
