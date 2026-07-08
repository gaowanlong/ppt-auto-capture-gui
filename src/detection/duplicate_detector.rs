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
