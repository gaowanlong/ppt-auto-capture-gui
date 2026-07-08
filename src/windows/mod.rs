#[cfg(target_os = "windows")]
mod monitor;
#[cfg(target_os = "windows")]
mod window_locator;
#[cfg(target_os = "windows")]
mod window_mover;
#[cfg(target_os = "windows")]
mod dxgi_capture;
#[cfg(target_os = "windows")]
mod gdi_capture;
#[cfg(target_os = "windows")]
mod session_events;

#[cfg(target_os = "windows")]
pub use monitor::*;
#[cfg(target_os = "windows")]
pub use window_locator::*;
#[cfg(target_os = "windows")]
pub use window_mover::*;
#[cfg(target_os = "windows")]
pub use dxgi_capture::*;
#[cfg(target_os = "windows")]
pub use gdi_capture::*;
#[cfg(target_os = "windows")]
pub use session_events::*;

// --- Stubs for non-Windows builds ---
#[cfg(not(target_os = "windows"))]
pub mod stub {
    pub struct MonitorEnumerator;
    pub struct WindowLocator;
    pub struct WindowMover;
    pub struct DxgiCapturer;
    pub struct GdiCapturer;
    pub struct SessionEventMonitor;
}
#[cfg(not(target_os = "windows"))]
pub use stub::*;
