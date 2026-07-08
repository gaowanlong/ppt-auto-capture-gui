use crate::model::Region;

/// Represents a detected monitor or virtual display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitorInfo {
    /// Windows HMONITOR handle value as u64.
    pub hmonitor: u64,
    /// Adapter name, e.g. "Intel(R) UHD Graphics".
    pub adapter_name: String,
    /// Output name, e.g. "\\\\.\\DISPLAY1".
    pub output_name: String,
    /// Friendly description.
    pub description: String,
    /// Screen region in virtual desktop coordinates.
    pub region: Region,
    /// Whether this is the primary monitor.
    pub is_primary: bool,
    /// Whether this is suspected to be a virtual or dummy display.
    pub is_virtual_suspect: bool,
    /// Monitor index for DXGI.
    pub output_index: u32,
    /// Adapter index for DXGI.
    pub adapter_index: u32,
}

impl MonitorInfo {
    pub fn is_valid(&self) -> bool {
        self.region.is_valid()
    }
}
