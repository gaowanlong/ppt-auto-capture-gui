use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{Frame, MonitorInfo, Region, WindowInfo};
use anyhow::{Context, Result};
use xcap::{Monitor, Window};

mod adapter;
use adapter::{
    capture_error_message, classify_capture_error, monitor_info, rgba_to_bgra, window_info,
    CaptureErrorKind, MonitorSnapshot, WindowSnapshot,
};

pub const fn backend_name() -> &'static str {
    "macos-xcap"
}

pub struct DxgiCapturer {
    selected_monitor_id: Option<u32>,
    frame_index: u64,
}

impl DxgiCapturer {
    pub fn new() -> Self {
        Self {
            selected_monitor_id: None,
            frame_index: 0,
        }
    }

    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        let monitor_id = u32::try_from(monitor.hmonitor)
            .context("macOS display ID is outside the supported range")?;
        find_monitor(monitor_id)?;
        self.selected_monitor_id = Some(monitor_id);
        self.frame_index = 0;
        Ok(())
    }

    pub fn capture_frame(&mut self, _timeout_ms: u32) -> Result<Option<Frame>> {
        let monitor_id = self
            .selected_monitor_id
            .ok_or_else(|| anyhow::anyhow!("No capturer initialized"))?;
        let monitor = find_monitor(monitor_id)?;
        let image = monitor
            .capture_image()
            .map_err(|error| capture_error(error.to_string()))?;
        self.frame_index += 1;
        Ok(Some(frame_from_image(image, self.frame_index)))
    }

    pub fn release(&mut self) {
        self.selected_monitor_id = None;
        self.frame_index = 0;
    }

    pub fn is_initialized(&self) -> bool {
        self.selected_monitor_id.is_some()
    }
}

pub struct GdiCapturer {
    selected_monitor_id: Option<u32>,
    selected_window_id: Option<u32>,
    frame_index: u64,
}

impl GdiCapturer {
    pub fn new() -> Self {
        Self {
            selected_monitor_id: None,
            selected_window_id: None,
            frame_index: 0,
        }
    }

    pub fn initialize(&mut self, monitor: &MonitorInfo) -> Result<()> {
        let monitor_id = u32::try_from(monitor.hmonitor)
            .context("macOS display ID is outside the supported range")?;
        find_monitor(monitor_id)?;
        self.selected_monitor_id = Some(monitor_id);
        self.frame_index = 0;
        Ok(())
    }

    pub fn capture_frame(&mut self) -> Result<Frame> {
        let image = if let Some(window_id) = self.selected_window_id {
            find_window(window_id)?
                .capture_image()
                .map_err(|error| capture_error(error.to_string()))?
        } else {
            let monitor_id = self
                .selected_monitor_id
                .ok_or_else(|| anyhow::anyhow!("No capturer initialized"))?;
            find_monitor(monitor_id)?
                .capture_image()
                .map_err(|error| capture_error(error.to_string()))?
        };
        self.frame_index += 1;
        Ok(frame_from_image(image, self.frame_index))
    }

    pub fn is_initialized(&self) -> bool {
        self.selected_monitor_id.is_some()
    }

    pub fn set_window_hwnd(&mut self, window_id: u64, _client_w: u32, _client_h: u32) {
        self.selected_window_id = u32::try_from(window_id).ok().filter(|id| *id != 0);
    }
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

pub fn get_client_window_rect(window_id: u64) -> Result<Region> {
    get_window_rect(window_id)
}

pub fn get_window_rect(window_id: u64) -> Result<Region> {
    let window_id =
        u32::try_from(window_id).context("macOS window ID is outside the supported range")?;
    let window = find_window(window_id)?;
    Ok(Region::new(
        window
            .x()
            .map_err(|error| capture_error(error.to_string()))?,
        window
            .y()
            .map_err(|error| capture_error(error.to_string()))?,
        window
            .width()
            .map_err(|error| capture_error(error.to_string()))?,
        window
            .height()
            .map_err(|error| capture_error(error.to_string()))?,
    ))
}

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let monitors = Monitor::all().map_err(|error| capture_error(error.to_string()))?;
    let mut result = monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| monitor_snapshot(monitor).map(|item| monitor_info(item, index)))
        .collect::<Result<Vec<_>>>()?;
    result.retain(|monitor| monitor.region.is_valid());
    result.sort_by_key(|monitor| (!monitor.is_primary, monitor.region.x, monitor.region.y));
    Ok(result)
}

pub fn enumerate_windows() -> Result<Vec<WindowInfo>> {
    let windows = Window::all().map_err(|error| capture_error(error.to_string()))?;
    let mut result = windows
        .iter()
        .map(window_snapshot)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(window_info)
        .filter(|window| window.is_visible && !window.is_minimized && window.region.is_valid())
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .is_powerpoint
            .cmp(&left.is_powerpoint)
            .then_with(|| left.process_name.cmp(&right.process_name))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.hwnd.cmp(&right.hwnd))
    });
    Ok(result)
}

pub fn move_window_to_monitor(_window_id: u64, _region: &Region) -> Result<()> {
    Err(anyhow::anyhow!("Moving windows is not supported on macOS"))
}

pub fn maximize_window(_window_id: u64) -> Result<()> {
    Err(anyhow::anyhow!(
        "Maximizing windows is not supported on macOS"
    ))
}

fn monitor_snapshot(monitor: &Monitor) -> Result<MonitorSnapshot> {
    Ok(MonitorSnapshot {
        id: monitor
            .id()
            .map_err(|error| capture_error(error.to_string()))?,
        name: monitor
            .name()
            .map_err(|error| capture_error(error.to_string()))?,
        friendly_name: monitor
            .friendly_name()
            .map_err(|error| capture_error(error.to_string()))?,
        x: monitor
            .x()
            .map_err(|error| capture_error(error.to_string()))?,
        y: monitor
            .y()
            .map_err(|error| capture_error(error.to_string()))?,
        width: monitor
            .width()
            .map_err(|error| capture_error(error.to_string()))?,
        height: monitor
            .height()
            .map_err(|error| capture_error(error.to_string()))?,
        is_primary: monitor
            .is_primary()
            .map_err(|error| capture_error(error.to_string()))?,
    })
}

fn window_snapshot(window: &Window) -> Result<WindowSnapshot> {
    let monitor_id = window
        .current_monitor()
        .and_then(|monitor| monitor.id())
        .unwrap_or_default();
    Ok(WindowSnapshot {
        id: window
            .id()
            .map_err(|error| capture_error(error.to_string()))?,
        pid: window
            .pid()
            .map_err(|error| capture_error(error.to_string()))?,
        app_name: window
            .app_name()
            .map_err(|error| capture_error(error.to_string()))?,
        title: window
            .title()
            .map_err(|error| capture_error(error.to_string()))?,
        monitor_id,
        x: window
            .x()
            .map_err(|error| capture_error(error.to_string()))?,
        y: window
            .y()
            .map_err(|error| capture_error(error.to_string()))?,
        width: window
            .width()
            .map_err(|error| capture_error(error.to_string()))?,
        height: window
            .height()
            .map_err(|error| capture_error(error.to_string()))?,
        is_minimized: window
            .is_minimized()
            .map_err(|error| capture_error(error.to_string()))?,
    })
}

fn find_monitor(monitor_id: u32) -> Result<Monitor> {
    Monitor::all()
        .map_err(|error| capture_error(error.to_string()))?
        .into_iter()
        .find(|monitor| monitor.id().ok() == Some(monitor_id))
        .ok_or_else(|| {
            anyhow::anyhow!(capture_error_message(
                CaptureErrorKind::DisplayLost,
                &monitor_id.to_string()
            ))
        })
}

fn find_window(window_id: u32) -> Result<Window> {
    Window::all()
        .map_err(|error| capture_error(error.to_string()))?
        .into_iter()
        .find(|window| window.id().ok() == Some(window_id))
        .ok_or_else(|| {
            anyhow::anyhow!(capture_error_message(
                CaptureErrorKind::WindowLost,
                &window_id.to_string()
            ))
        })
}

fn capture_error(detail: String) -> anyhow::Error {
    anyhow::anyhow!(capture_error_message(
        classify_capture_error(&detail),
        &detail
    ))
}

fn frame_from_image(image: image::RgbaImage, frame_index: u64) -> Frame {
    let width = image.width();
    let height = image.height();
    Frame::new(
        rgba_to_bgra(image.into_raw()),
        width,
        height,
        width * 4,
        frame_index,
        now_ms(),
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capturers_start_uninitialized() {
        assert!(!DxgiCapturer::new().is_initialized());
        assert!(!GdiCapturer::new().is_initialized());
    }

    #[test]
    fn display_capture_before_initialize_is_an_error() {
        let error = DxgiCapturer::new().capture_frame(0).unwrap_err();
        assert!(error.to_string().contains("No capturer initialized"));
    }

    #[test]
    fn window_capture_before_initialize_is_an_error() {
        let error = GdiCapturer::new().capture_frame().unwrap_err();
        assert!(error.to_string().contains("No capturer initialized"));
    }

    #[test]
    fn release_clears_initialized_state() {
        let mut capturer = DxgiCapturer::new();
        capturer.selected_monitor_id = Some(42);
        capturer.release();
        assert!(!capturer.is_initialized());
    }
}
