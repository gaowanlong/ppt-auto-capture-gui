//! Detects black, blank, or protected-content frames.
//! If the frame is overwhelmingly black (>= threshold), it's flagged.

use crate::model::Frame;

pub struct BlackFrameDetector {
    /// Fraction of pixels that must be "near-black" to flag as black.
    threshold: f64,
    downsample: u32,
}

impl BlackFrameDetector {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            downsample: 8,
        }
    }

    pub fn set_threshold(&mut self, threshold: f64) {
        self.threshold = threshold;
    }

    /// Returns true if the frame is black/blank.
    pub fn is_black(&self, frame: &Frame) -> bool {
        let step = self.downsample as usize;
        let mut black_pixels: u64 = 0;
        let mut total_pixels: u64 = 0;

        let mut y = 0;
        while y < frame.height {
            let mut x = 0;
            while x < frame.width {
                let offset = (y * frame.stride + x * 4) as usize;
                if offset + 3 < frame.data.len() {
                    let b = frame.data[offset] as u16;
                    let g = frame.data[offset + 1] as u16;
                    let r = frame.data[offset + 2] as u16;
                    // Near-black if all channels < 30
                    if r < 30 && g < 30 && b < 30 {
                        black_pixels += 1;
                    }
                }
                total_pixels += 1;
                x += step as u32;
            }
            y += step as u32;
        }

        if total_pixels == 0 {
            return true;
        }

        let ratio = black_pixels as f64 / total_pixels as f64;
        ratio >= self.threshold
    }
}
