//! Detects meaningful changes between consecutive frames.
//! Uses pixel-difference analysis on a downsampled version for performance.

use crate::model::Frame;

pub struct ChangeDetector {
    threshold: f64,
    previous_frame: Option<Frame>,
    /// Downsample factor for faster comparison (4 = every 4th pixel).
    downsample: u32,
}

impl ChangeDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            previous_frame: None,
            downsample: 4,
        }
    }

    pub fn set_threshold(&mut self, threshold: f64) {
        self.threshold = threshold;
    }

    /// Compares the new frame to the stored reference.
    /// Returns (changed: bool, diff_ratio: f64).
    pub fn detect_change(&mut self, frame: &Frame) -> (bool, f64) {
        let prev = match self.previous_frame.as_ref() {
            Some(p) => p,
            None => {
                return (false, 0.0);
            }
        };

        // Ensure same dimensions
        if prev.width != frame.width || prev.height != frame.height {
            return (true, 1.0);
        }

        let step = self.downsample as usize;
        let mut diff_pixels: u64 = 0;
        let mut total_pixels: u64 = 0;

        // Compare downsampled pixels using luminance
        let mut y = 0;
        while y < frame.height {
            let mut x = 0;
            while x < frame.width {
                let prev_lum = prev.luminance_at(x, y);
                let curr_lum = frame.luminance_at(x, y);

                // Consider pixel changed if luminance differs by more than 20
                if (prev_lum as i16 - curr_lum as i16).unsigned_abs() > 20 {
                    diff_pixels += 1;
                }
                total_pixels += 1;

                x += step as u32;
            }
            y += step as u32;
        }

        let diff_ratio = if total_pixels > 0 {
            diff_pixels as f64 / total_pixels as f64
        } else {
            0.0
        };

        let changed = diff_ratio >= self.threshold;

        (changed, diff_ratio)
    }

    /// Update the reference frame after comparison.
    pub fn update_reference(&mut self, frame: &Frame) {
        self.previous_frame = Some(frame.clone());
    }

    pub fn reset(&mut self) {
        self.previous_frame = None;
    }
}
