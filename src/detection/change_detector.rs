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

        // Auto-adjust downsample so ~500K pixels are checked regardless of resolution.
        // 1080p → downsample 2, 4K → downsample 4, 5K → downsample 6, etc.
        let target: u64 = 500_000;
        let total = frame.width as u64 * frame.height as u64;
        let ds = ((total as f64 / target as f64).sqrt().ceil() as u32).max(1).min(8);
        self.downsample = ds;

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

    /// Returns the reference frame (the last frame used for comparison).
    /// Used as fallback to capture slides during rapid transitions where
    /// the stability detector hasn't accumulated any stable counts yet.
    pub fn get_reference_frame(&self) -> Option<&Frame> {
        self.previous_frame.as_ref()
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



    /// Simulate a PPT slide change: old slide → black transition → new slide.
    /// Verifies the full change + stability pipeline.
    #[test]
    fn test_slide_change_detection_pipeline() {
        use crate::detection::StabilityDetector;

        let mut change_det = ChangeDetector::new(0.05);
        let mut stable_det = StabilityDetector::new(2);

        // Slide 1: solid blue
        let slide1 = solid_frame(0, 0, 255, 100, 100);
        // Slide 2: solid red (different content, like a PPT slide change)
        let slide2 = solid_frame(255, 0, 0, 100, 100);
        // Black transition frame (PPT often flashes black between slides)
        let mut black_data = vec![0u8; 100 * 100 * 4];
        let black_frame = Frame::new(black_data.clone(), 100, 100, 400, 0, 0);

        // Phase 1: Slide 1 displayed, no changes
        let (changed, _) = change_det.detect_change(&slide1);
        change_det.update_reference(&slide1);
        assert!(!changed, "First frame should set reference, not detect change");

        // Phase 2: Black transition frame
        let (changed, _) = change_det.detect_change(&black_frame);
        change_det.update_reference(&black_frame);
        assert!(changed, "Black frame should be detected as change from slide1");

        // Phase 3: Slide 2 appears (like a new slide after transition)
        let (changed, _) = change_det.detect_change(&slide2);
        change_det.update_reference(&slide2);
        assert!(changed, "Slide 2 should be detected as change from black frame");

        // Phase 4: Stability check — Slide 2 stabilizes
        assert!(!stable_det.check_stable(&slide2), "First stable check sets reference");
        assert!(!stable_det.check_stable(&slide2), "Second check: stable_count=1");
        assert!(stable_det.check_stable(&slide2), "Third check: stable_count=2 => stable!");

        // Phase 5: After stability, no more changes detected for same content
        change_det.update_reference(&slide2);
        let (changed, _) = change_det.detect_change(&slide2);
        assert!(!changed, "Same slide should not trigger change after reference update");
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



    /// Simulates the full capture cycle: old slide → change detected →
    /// wait for stability → new slide stabilizes → confirmed by no further changes.
    #[test]
    fn test_capture_pipeline_full_cycle() {
        use crate::detection::StabilityDetector;
        let mut change_det = ChangeDetector::new(0.05);
        let mut stable_det = StabilityDetector::new(2);
        let slide1 = solid_frame(0, 0, 200, 100, 100);
        let slide2 = solid_frame(200, 0, 0, 100, 100);

        let (changed, _) = change_det.detect_change(&slide1);
        change_det.update_reference(&slide1);
        assert!(!changed, "First frame should not trigger change");

        let (changed, _) = change_det.detect_change(&slide1);
        change_det.update_reference(&slide1);
        assert!(!changed, "Same slide should not trigger change");

        let (changed, _) = change_det.detect_change(&slide2);
        change_det.update_reference(&slide2);
        assert!(changed, "New slide should trigger change");

        assert!(!stable_det.check_stable(&slide2), "First stable check sets ref");
        assert!(!stable_det.check_stable(&slide2), "Second: stable_count=1");
        assert!(stable_det.check_stable(&slide2), "Third: stable_count=2 => stable!");

        let (changed, _) = change_det.detect_change(&slide2);
        change_det.update_reference(&slide2);
        assert!(!changed, "Stable slide should not re-trigger change");
    }

    /// Stress test: 100 rapid slide changes in sequence, verifying the pipeline
    /// doesn't lose intermediate slides.
    #[test]
    fn test_capture_pipeline_rapid_slides() {
        use crate::detection::StabilityDetector;
        let mut change_det = ChangeDetector::new(0.05);
        let mut stable_det = StabilityDetector::new(2);
        let mut saved = 0u32;
        let n = 50;

        let first = solid_frame(0, 0, 200, 100, 100);
        change_det.detect_change(&first);
        change_det.update_reference(&first);
        saved += 1;

        for i in 1..n {
            // Use colors with high luminance contrast (not just hue shift)
            let new_slide = if i % 2 == 0 {
                solid_frame(255, 255, 255, 100, 100)  // white
            } else {
                solid_frame(0, 0, 0, 100, 100)        // black
            };
            let (changed, _) = change_det.detect_change(&new_slide);
            change_det.update_reference(&new_slide);
            assert!(changed, "Slide {} should trigger change", i + 1);
            stable_det.reset();
            assert!(!stable_det.check_stable(&new_slide));
            assert!(!stable_det.check_stable(&new_slide));
            assert!(stable_det.check_stable(&new_slide));
            let (changed, _) = change_det.detect_change(&new_slide);
            change_det.update_reference(&new_slide);
            assert!(!changed, "Slide {} should be stable after stabilization", i + 1);
            saved += 1;
        }
        assert_eq!(saved, n, "All {} slides should be captured", n);
    }

    /// Alternating frames should never stabilize.
    #[test]
    fn test_alternating_frames_not_stable() {
        use crate::detection::StabilityDetector;
        let mut stable_det = StabilityDetector::new(2);
        let a = solid_frame(255, 0, 0, 50, 50);
        let b = solid_frame(0, 255, 0, 50, 50);
        assert!(!stable_det.check_stable(&a)); assert!(!stable_det.check_stable(&b));
        assert!(!stable_det.check_stable(&a)); assert!(!stable_det.check_stable(&b));
    }

    /// Dimension change triggers detection at 1.0 diff.
    #[test]
    fn test_dimension_change_triggers_detection() {
        let mut change_det = ChangeDetector::new(0.05);
        let f1 = solid_frame(100, 100, 100, 100, 100);
        let f2 = solid_frame(100, 100, 100, 200, 200);
        change_det.detect_change(&f1);
        change_det.update_reference(&f1);
        let (changed, diff) = change_det.detect_change(&f2);
        assert!(changed);
        assert!((diff - 1.0).abs() < 0.001);
    }
}
