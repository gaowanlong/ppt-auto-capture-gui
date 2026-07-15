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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_record_new() {
        let r = SlideRecord::new(
            1, "slide_0001.png".into(), "slides/slide_0001.png".into(),
            42, 1920, 1080, "abc123".into(),
            "TestWindow".into(), "Monitor1".into(),
        );
        assert_eq!(r.slide_number, 1);
        assert_eq!(r.png_filename, "slide_0001.png");
        assert_eq!(r.png_relative_path, "slides/slide_0001.png");
        assert_eq!(r.frame_index, 42);
        assert_eq!(r.width, 1920);
        assert_eq!(r.height, 1080);
        assert_eq!(r.content_hash, "abc123");
        assert_eq!(r.source_name, "TestWindow");
        assert_eq!(r.monitor_name, "Monitor1");
        assert!(!r.slide_id.is_empty());
    }
}
