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
                    if r < 20 && g < 20 && b < 20 {
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    fn all_black_frame(w: u32, h: u32) -> Frame {
        let data = vec![0u8; (w * h * 4) as usize];
        Frame::new(data, w, h, w * 4, 0, 0)
    }

    fn all_white_frame(w: u32, h: u32) -> Frame {
        let data = vec![255u8; (w * h * 4) as usize];
        Frame::new(data, w, h, w * 4, 0, 0)
    }

    fn mixed_frame() -> Frame {
        let mut data = vec![0u8; 100 * 100 * 4];
        // Make half black, half white
        let half = (100 * 100 * 4) / 2;
        for i in half..data.len() {
            data[i] = 255;
        }
        Frame::new(data, 100, 100, 400, 0, 0)
    }

    #[test]
    fn test_all_black_is_black() {
        let detector = BlackFrameDetector::new(0.95);
        let frame = all_black_frame(100, 100);
        assert!(detector.is_black(&frame), "All-black frame should be detected");
    }

    #[test]
    fn test_all_white_is_not_black() {
        let detector = BlackFrameDetector::new(0.95);
        let frame = all_white_frame(100, 100);
        assert!(!detector.is_black(&frame), "All-white frame should not be black");
    }

    #[test]
    fn test_low_threshold_detects_mixed() {
        let detector = BlackFrameDetector::new(0.40);
        let frame = mixed_frame(); // 50% black
        assert!(detector.is_black(&frame), "Mixed 50% black should be detected with 40% threshold");
    }

    #[test]
    fn test_set_threshold() {
        let mut detector = BlackFrameDetector::new(0.95);
        detector.set_threshold(0.05);
        let frame = all_white_frame(10, 10);
        assert!(!detector.is_black(&frame), "All-white frame should not be black even with 5% threshold");
    }



    /// 70% dark frame should not be flagged as black at 80% threshold.
    #[test]
    fn test_dark_frame_not_black() {
        let detector = BlackFrameDetector::new(0.80);
        let mut data = vec![0u8; 100 * 100 * 4];
        let half = (100 * 100 * 4) * 70 / 100;
        for i in half..data.len() { data[i] = 255; }
        let frame = Frame::new(data, 100, 100, 400, 0, 0);
        assert!(!detector.is_black(&frame), "70% dark should not be black at 80% threshold");
    }

    /// 1x1 pixel frames.
    #[test]
    fn test_tiny_frame_black_detection() {
        let detector = BlackFrameDetector::new(0.95);
        let black = Frame::new(vec![0u8; 4], 1, 1, 4, 0, 0);
        let white = Frame::new(vec![255u8; 4], 1, 1, 4, 0, 0);
        assert!(detector.is_black(&black));
        assert!(!detector.is_black(&white));
    }

    /// All-zero frame is black.
    #[test]
    fn test_all_zeros_is_black() {
        let detector = BlackFrameDetector::new(0.95);
        let frame = Frame::new(vec![0u8; 100 * 100 * 4], 100, 100, 400, 0, 0);
        assert!(detector.is_black(&frame));
    }
}
