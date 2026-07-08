use chrono::{DateTime, Utc};

/// Represents a single captured slide in the session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideRecord {
    /// Unique slide ID (UUIDv4).
    pub slide_id: String,
    /// Sequential slide number in this session.
    pub slide_number: u32,
    /// Filename of the saved PNG (e.g. "slide_0003.png").
    pub png_filename: String,
    /// Path to the PNG file relative to output directory.
    pub png_relative_path: String,
    /// Timestamp when the slide was captured.
    pub captured_at: DateTime<Utc>,
    /// Frame index at capture time.
    pub frame_index: u64,
    /// Width of the captured image in pixels.
    pub width: u32,
    /// Height of the captured image in pixels.
    pub height: u32,
    /// SHA256 hash of the pixel content, for duplicate detection.
    pub content_hash: String,
    /// The capture source name at time of capture.
    pub source_name: String,
    /// The monitor name at time of capture.
    pub monitor_name: String,
}

impl SlideRecord {
    /// Create a new slide record.
    pub fn new(
        slide_number: u32,
        png_filename: String,
        png_relative_path: String,
        frame_index: u64,
        width: u32,
        height: u32,
        content_hash: String,
        source_name: String,
        monitor_name: String,
    ) -> Self {
        Self {
            slide_id: uuid::Uuid::new_v4().to_string(),
            slide_number,
            png_filename,
            png_relative_path,
            captured_at: Utc::now(),
            frame_index,
            width,
            height,
            content_hash,
            source_name,
            monitor_name,
        }
    }
}
