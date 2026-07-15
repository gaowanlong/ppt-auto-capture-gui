//! Detects duplicate slides (same content as previously saved).
//! Uses a SHA256 hash of the pixel data.

use sha2::{Sha256, Digest};
use crate::model::Frame;

pub struct DuplicateDetector {
    /// SHA256 hash of the last saved slide
    last_hash: Option<String>,
}

impl DuplicateDetector {
    pub fn new() -> Self {
        Self { last_hash: None }
    }

    /// Compute a SHA256 hash of the frame's pixel data (downsampled for speed).
    pub fn compute_hash(&self, frame: &Frame) -> String {
        let step = 4usize;
        let mut hasher = Sha256::new();

        let mut y = 0;
        while y < frame.height {
            let mut x = 0;
            while x < frame.width {
                let offset = (y * frame.stride + x * 4) as usize;
                if offset + 3 < frame.data.len() {
                    hasher.update(&[frame.data[offset], frame.data[offset+1], frame.data[offset+2]]);
                }
                x += step as u32;
            }
            y += step as u32;
        }

        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Check if a new hash is a duplicate of the previously seen one.
    pub fn is_duplicate(&self, hash: &str) -> bool {
        if let Some(ref last) = self.last_hash {
            last == hash
        } else {
            false
        }
    }

    pub fn update_last(&mut self, hash: String) {
        self.last_hash = Some(hash);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frame;

    fn make_frame(data: &[u8], w: u32, h: u32) -> Frame {
        Frame::new(data.to_vec(), w, h, w * 4, 0, 0)
    }

    #[test]
    fn test_duplicate_detection() {
        let detector = DuplicateDetector::new();
        let f = make_frame(&vec![128u8; 100], 5, 5);
        let hash = detector.compute_hash(&f);
        assert_eq!(hash.len(), 64, "SHA256 hex should be 64 chars");
        // First check with no previous hash — not a duplicate
        assert!(!detector.is_duplicate(&hash));
    }

    #[test]
    fn test_update_last_then_duplicate() {
        let mut detector = DuplicateDetector::new();
        let f = make_frame(&[100u8; 400], 10, 10);
        let hash = detector.compute_hash(&f);
        detector.update_last(hash.clone());
        assert!(detector.is_duplicate(&hash), "Same hash should be duplicate");
    }

    #[test]
    fn test_different_hashes_not_duplicate() {
        let mut detector = DuplicateDetector::new();
        // Frame 1
        let f1 = make_frame(&[100u8; 400], 10, 10);
        let h1 = detector.compute_hash(&f1);
        detector.update_last(h1);
        // Frame 2 (different data)
        let f2 = make_frame(&[200u8; 400], 10, 10);
        let h2 = detector.compute_hash(&f2);
        assert!(!detector.is_duplicate(&h2), "Different hash should not be duplicate");
    }

    #[test]
    fn test_sha256_consistency() {
        let detector = DuplicateDetector::new();
        let f1 = make_frame(&[50u8; 400], 10, 10);
        let f2 = make_frame(&[50u8; 400], 10, 10);
        let h1 = detector.compute_hash(&f1);
        let h2 = detector.compute_hash(&f2);
        assert_eq!(h1, h2, "Identical frames should produce same hash");
    }
}
