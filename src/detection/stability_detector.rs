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
            downsample: 4,
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

    pub fn reset(&mut self) {
        self.stable_count = 0;
        self.previous_frame = None;
    }
}
