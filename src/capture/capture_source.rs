/// Describes the capture source — which window and monitor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaptureSource {
    /// HWND of the target window (0 if capturing full monitor).
    pub window_hwnd: u64,
    /// Window title at time of selection.
    pub window_title: String,
    /// HMONITOR to capture.
    pub monitor_hmonitor: u64,
    /// Monitor description.
    pub monitor_description: String,
    /// Whether to use DXGI (preferred) or GDI (fallback).
    pub use_dxgi: bool,
    /// Output directory for PNG/PPTX files.
    pub output_dir: String,
    /// Output PPTX filename.
    pub output_filename: String,
    /// Slide aspect ratio ("16:9" or "4:3").
    pub page_ratio: String,
    /// Image fit mode: "fill" or "fit".
    pub image_fit: String,
}

impl CaptureSource {
    pub fn new(
        window_hwnd: u64,
        window_title: String,
        monitor_hmonitor: u64,
        monitor_description: String,
    ) -> Self {
        Self {
            window_hwnd,
            window_title,
            monitor_hmonitor,
            monitor_description,
            use_dxgi: true,
            output_dir: String::from("output"),
            output_filename: String::from("output.pptx"),
            page_ratio: String::from("16:9"),
            image_fit: String::from("fit"),
        }
    }

    pub fn is_window_selected(&self) -> bool {
        self.window_hwnd != 0
    }

    pub fn display_name(&self) -> String {
        if self.is_window_selected() {
            format!("{} → {}", self.window_title, self.monitor_description)
        } else {
            format!("Full monitor: {}", self.monitor_description)
        }
    }
}
