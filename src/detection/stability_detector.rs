//! Detects when a sequence of frames has become stable
//! (i.e., no significant changes for N consecutive frames).

use crate::model::Frame;

pub struct StabilityDetector {
    /// How many consecutive similar frames are required for stability.
    required_stable: u32,
    /// Current count of consecutive stable frames.
    stable_count: u32,
    previous_frame: Option<Frame>,
    downsample: u32,
}

impl StabilityDetector {
    pub fn new(required_stable: u32) -> Self {
        Self {
            required_stable,
            stable_count: 0,
            previous_frame: None,
            downsample: 2,
        }
    }

    pub fn set_required_stable(&mut self, n: u32) {
        self.required_stable = n;
    }

    /// Check if the current frame is stable. Returns true when stable.
    pub fn check_stable(&mut self, frame: &Frame) -> bool {
        let prev = match self.previous_frame.as_ref() {
            Some(p) => p,
            None => {
                self.previous_frame = Some(frame.clone());
                self.stable_count = 0;
                return false;
            }
        };

        if prev.width != frame.width || prev.height != frame.height {
            self.stable_count = 0;
            self.previous_frame = Some(frame.clone());
            return false;
        }

        // Auto-adjust downsample by resolution (same formula as ChangeDetector)
        let target: u64 = 500_000;
        let total = frame.width as u64 * frame.height as u64;
        let ds = ((total as f64 / target as f64).sqrt().ceil() as u32).max(1).min(8);
        self.downsample = ds;

        let step = self.downsample as usize;
        let mut diff_pixels: u64 = 0;
        let mut total_pixels: u64 = 0;

        let mut y = 0;
        while y < frame.height {
            let mut x = 0;
            while x < frame.width {
                let prev_lum = prev.luminance_at(x, y);
                let curr_lum = frame.luminance_at(x, y);

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

        // If minimal change, count as stable
        if diff_ratio < 0.005 {
            self.stable_count += 1;
        } else {
            self.stable_count = 0;
        }

        self.previous_frame = Some(frame.clone());

        self.stable_count >= self.required_stable
    }

    /// Returns the last frame being stabilized, if any.
    /// Used by the capture loop to save intermediate slides during rapid transitions.
    pub fn get_pending_frame(&self) -> Option<&Frame> {
        if self.stable_count > 0 {
            self.previous_frame.as_ref()
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.stable_count = 0;
        self.previous_frame = None;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    fn solid_frame(r: u8, g: u8, b: u8) -> Frame {
        let mut data = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA
        for y in 0..4 {
            for x in 0..4 {
                let i = (y * 16 + x * 4) as usize;
                data[i] = b; data[i+1] = g; data[i+2] = r; data[i+3] = 255;
            }
        }
        Frame::new(data, 4, 4, 16, 0, 0)
    }

    fn different_frame() -> Frame {
        let mut data = vec![128u8; 4 * 4 * 4];
        Frame::new(data, 4, 4, 16, 0, 0)
    }

    #[test]
    fn test_not_stable_until_enough_frames() {
        let mut detector = StabilityDetector::new(5);
        let f = solid_frame(100, 100, 100);
        // First call sets reference, doesn't increment counter
        assert!(!detector.check_stable(&f));
        // Need 5 more identical frames to reach stable_count >= 5
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        // 6th frame → stable_count = 5
        assert!(detector.check_stable(&f), "Should be stable after 5 identical frames");
    }

    #[test]
    fn test_different_frame_resets_counter() {
        let mut detector = StabilityDetector::new(3);
        let f1 = solid_frame(100, 100, 100);
        assert!(!detector.check_stable(&f1));  // sets ref
        assert!(!detector.check_stable(&f1));  // stable=1
        // Different frame resets counter
        let f2 = different_frame();
        assert!(!detector.check_stable(&f2));  // f2 != ref, counter=0
        // After reset, first f1 call sets new ref, then 3 more for stable_count >= 3
        assert!(!detector.check_stable(&f1));  // sets ref to f1, no compare
        assert!(!detector.check_stable(&f1));  // stable=1
        assert!(!detector.check_stable(&f1));  // stable=2
        assert!(detector.check_stable(&f1));   // stable=3
    }

    #[test]
    fn test_set_required_stable() {
        let mut detector = StabilityDetector::new(10);
        detector.set_required_stable(2);
        let f = solid_frame(50, 100, 150);
        assert!(!detector.check_stable(&f));  // sets ref
        assert!(!detector.check_stable(&f));  // stable=1
        assert!(detector.check_stable(&f),   // stable=2
            "Should be stable after 3 calls (1st sets ref, 2 more = stable=2)");
    }

    #[test]
    fn test_reset() {
        let mut detector = StabilityDetector::new(3);
        let f = solid_frame(100, 100, 100);
        detector.check_stable(&f);  // sets ref
        detector.check_stable(&f);  // stable=1
        detector.reset();
        assert!(!detector.check_stable(&f), "Reset clears ref, first call sets new ref");
        assert!(!detector.check_stable(&f));  // stable=1
        assert!(!detector.check_stable(&f));  // stable=2
        assert!(detector.check_stable(&f));   // stable=3
    }



    #[test]
    fn test_stability_resets_on_dimension_change() {
        let mut detector = StabilityDetector::new(2);
        let f1 = solid_frame(100, 100, 100);
        assert!(!detector.check_stable(&f1));
        assert!(!detector.check_stable(&f1));
        // Frame with different dimensions triggers reset
        let data = vec![100u8; 8 * 8 * 4];
        let f2 = Frame::new(data, 8, 8, 32, 0, 0);
        assert!(!detector.check_stable(&f2), "Dimension change resets stable_count");
    }

    #[test]
    fn test_many_identical_frames_stable() {
        let mut detector = StabilityDetector::new(5);
        let f = solid_frame(100, 100, 100);
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(!detector.check_stable(&f));
        assert!(detector.check_stable(&f), "Stable after 5 identical frames");
    }
}
