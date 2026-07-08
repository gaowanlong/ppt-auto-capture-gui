use crate::model::Region;

/// Represents a detected window that could be captured.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    /// Windows HWND value as u64.
    pub hwnd: u64,
    /// Window title text.
    pub title: String,
    /// Window class name.
    pub class_name: String,
    /// Current window region on screen.
    pub region: Region,
    /// Which monitor this window is mostly on (HMONITOR as u64).
    pub monitor_hmonitor: u64,
    /// Whether the window is visible.
    pub is_visible: bool,
    /// Whether the window is minimized.
    pub is_minimized: bool,
    /// Whether this is likely a PowerPoint window (class = "screenClass" or similar).
    pub is_powerpoint: bool,
    /// Process ID.
    pub process_id: u32,
    /// Process name (e.g. "POWERPNT.EXE", "chrome.exe", "msedge.exe").
    pub process_name: String,
}

impl WindowInfo {
    /// Check if this window is likely a slideshow viewer (not the editable deck).
    pub fn is_slideshow(&self) -> bool {
        self.class_name.eq_ignore_ascii_case("screenClass")
            || self.title.contains("Slide Show")
            || self.title.contains("放映")
            || self.title.contains("Präsentation")
    }

    pub fn is_valid(&self) -> bool {
        self.hwnd != 0 && self.region.is_valid()
    }
}
