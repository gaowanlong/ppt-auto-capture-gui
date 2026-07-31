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

#[cfg(target_os = "macos")]
#[path = "../macos/mod.rs"]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[path = "stub.rs"]
mod stub;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use stub::*;
