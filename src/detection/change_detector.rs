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
            downsample: 2,
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
                if (prev_lum as i16 - curr_lum as i16).unsigned_abs() > 15 {
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    fn make_frame(data: &[u8], w: u32, h: u32) -> Frame {
        Frame::new(data.to_vec(), w, h, w * 4, 0, 0)
    }

    fn solid_frame(r: u8, g: u8, b: u8, w: u32, h: u32) -> Frame {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w * 4 + x * 4) as usize;
                data[i] = b; data[i+1] = g; data[i+2] = r; data[i+3] = 255;
            }
        }
        Frame::new(data, w, h, w * 4, 0, 0)
    }

    #[test]
    fn test_no_change_on_identical_frames() {
        let mut detector = ChangeDetector::new(0.01);
        let f1 = solid_frame(128, 128, 128, 10, 10);
        let f2 = solid_frame(128, 128, 128, 10, 10);
        // First frame sets reference
        detector.detect_change(&f1);
        detector.update_reference(&f1);
        // Second frame should be no change
        let (changed, ratio) = detector.detect_change(&f2);
        assert!(!changed, "Identical frames should not trigger change");
        assert!(ratio < 0.01, "Diff ratio should be near 0");
    }

    #[test]
    fn test_change_on_different_frames() {
        let mut detector = ChangeDetector::new(0.01);
        let f1 = solid_frame(0, 0, 0, 10, 10);
        let f2 = solid_frame(255, 255, 255, 10, 10);
        detector.detect_change(&f1);
        detector.update_reference(&f1);
        let (changed, ratio) = detector.detect_change(&f2);
        assert!(changed, "Different frames should trigger change");
        assert!(ratio > 0.01, "Diff ratio should be significant");
    }

    #[test]
    fn test_high_threshold_ignores_small_changes() {
        let mut detector = ChangeDetector::new(0.50);
        let f1 = solid_frame(0, 0, 0, 100, 100);
        let mut data = vec![0u8; 100 * 100 * 4];
        // Only change 1000 pixels out of 10000
        for i in 0..1000 {
            let idx = (i * 4) as usize;
            if idx < data.len() {
                data[idx] = 255; data[idx+1] = 255; data[idx+2] = 255; data[idx+3] = 255;
            }
        }
        let f2 = Frame::new(data, 100, 100, 400, 0, 0);
        detector.detect_change(&f1);
        detector.update_reference(&f1);
        let (changed, _) = detector.detect_change(&f2);
        // 10% change should be below 50% threshold
        assert!(!changed, "Small change should be filtered by high threshold");
    }

    #[test]
    fn test_reset_clears_state() {
        let mut detector = ChangeDetector::new(0.01);
        let f = solid_frame(128, 128, 128, 10, 10);
        detector.detect_change(&f);
        detector.update_reference(&f);
        detector.reset();
        // After reset, first detect again sets reference
        let (changed, _) = detector.detect_change(&f);
        assert!(!changed, "After reset, first frame sets reference");
    }

    #[test]
    fn test_set_threshold() {
        let mut detector = ChangeDetector::new(0.01);
        detector.set_threshold(0.90);
        let f1 = solid_frame(0, 0, 0, 10, 10);
        let f2 = solid_frame(255, 255, 255, 10, 10);
        detector.detect_change(&f1);
        detector.update_reference(&f1);
        let (changed, _) = detector.detect_change(&f2);
        assert!(changed, "100% pixel change should exceed 90% threshold");
    }
}
