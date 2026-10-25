//! Bounded spatial signatures. Repeated local motion is excluded from slide identity.
use crate::model::Frame;

const COLS: usize = 64;
const ROWS: usize = 36;
const SAMPLES: usize = COLS * ROWS;
type Signature = Vec<[u8; 3]>;

#[derive(Default)]
pub struct MotionFilter {
    previous: Option<Signature>,
    saved: Option<Signature>,
    dimensions: (u32, u32),
    motion: Vec<u8>,
    quiet: Vec<u8>,
}

impl MotionFilter {
    fn signature(frame: &Frame) -> Signature {
        let mut result = Vec::with_capacity(SAMPLES);
        for y in 0..ROWS {
            for x in 0..COLS {
                let px = ((x * 2 + 1) as u64 * frame.width as u64 / (COLS * 2) as u64) as usize;
                let py = ((y * 2 + 1) as u64 * frame.height as u64 / (ROWS * 2) as u64) as usize;
                let offset = py * frame.stride as usize + px * 4;
                let rgb = frame.data.get(offset..offset + 3).unwrap_or(&[0, 0, 0]);
                result.push([rgb[0], rgb[1], rgb[2]]);
            }
        }
        result
    }

    fn differs(a: &[u8; 3], b: &[u8; 3]) -> bool {
        a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 20)
    }

    pub fn observe(&mut self, frame: &Frame) -> bool {
        if self.dimensions != (frame.width, frame.height) {
            *self = Self {
                dimensions: (frame.width, frame.height),
                motion: vec![0; SAMPLES],
                quiet: vec![0; SAMPLES],
                ..Self::default()
            };
        }
        let current = Self::signature(frame);
        let mut broad_transition = false;
        if let Some(previous) = &self.previous {
            let changes: Vec<bool> = previous
                .iter()
                .zip(&current)
                .map(|(a, b)| Self::differs(a, b))
                .collect();
            // A broad change is a likely slide transition, not a local video rectangle.
            if changes.iter().filter(|&&v| v).count() > SAMPLES * 2 / 5 {
                broad_transition = true;
            } else {
                for ((score, quiet), changed) in
                    self.motion.iter_mut().zip(&mut self.quiet).zip(changes)
                {
                    if changed {
                        *score = score.saturating_add(1).min(8);
                        *quiet = 0;
                    } else {
                        *quiet = quiet.saturating_add(1);
                        // A settled image must become eligible again: never hide future
                        // static content just because a video once played in this area.
                        if *quiet >= 15 {
                            *score = 0;
                        }
                    }
                }
                // Never mask most of the page, even after long presentations.
                if self.motion.iter().filter(|&&score| score >= 3).count() > SAMPLES * 2 / 5 {
                    self.motion.fill(0);
                }
            }
        }
        self.previous = Some(current);
        broad_transition
    }

    pub fn is_duplicate(&self, frame: &Frame) -> bool {
        if self.dimensions != (frame.width, frame.height) {
            return false;
        }
        let Some(saved) = &self.saved else {
            return false;
        };
        let current = Self::signature(frame);
        let changed = saved
            .iter()
            .zip(&current)
            .enumerate()
            .filter(|(i, (a, b))| {
                self.motion.get(*i).copied().unwrap_or(0) < 3 && Self::differs(a, b)
            })
            .count();
        // Tolerate a small cursor/laser pointer, not a large content edit.
        changed <= SAMPLES / 200
    }

    pub fn mark_saved(&mut self, frame: &Frame) {
        self.saved = Some(Self::signature(frame));
        self.dimensions = (frame.width, frame.height);
        if self.motion.len() != SAMPLES {
            self.motion = vec![0; SAMPLES];
            self.quiet = vec![0; SAMPLES];
        }
        self.motion.fill(0);
        self.quiet.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(background: u8, video: u8) -> Frame {
        let mut frame = Frame::new(vec![background; 320 * 180 * 4], 320, 180, 1280, 0, 0);
        for y in 45..135 {
            for x in 80..240 {
                let offset = (y * 320 + x) * 4;
                frame.data[offset..offset + 3].fill(video);
            }
        }
        frame
    }
    #[test]
    fn local_video_is_ignored_but_new_static_content_is_saved() {
        let mut filter = MotionFilter::default();
        filter.observe(&frame(255, 0));
        filter.mark_saved(&frame(255, 0));
        for value in [80, 160, 0, 80, 160, 0] {
            filter.observe(&frame(255, value));
        }
        assert!(filter.is_duplicate(&frame(255, 160)));
        assert!(!filter.is_duplicate(&frame(80, 160)));
    }
    #[test]
    fn previously_moving_region_becomes_eligible_when_static() {
        let mut filter = MotionFilter::default();
        filter.observe(&frame(255, 0));
        filter.mark_saved(&frame(255, 0));
        for value in [80, 160, 0, 80, 160, 0] {
            filter.observe(&frame(255, value));
        }
        assert!(filter.is_duplicate(&frame(255, 160)));
        for _ in 0..20 {
            filter.observe(&frame(255, 160));
        }
        assert!(!filter.is_duplicate(&frame(255, 160)));
    }

    #[test]
    fn broad_changes_do_not_become_a_permanent_motion_mask() {
        let mut filter = MotionFilter::default();
        filter.observe(&frame(255, 255));
        filter.mark_saved(&frame(255, 255));
        for value in [0, 255, 0, 255, 0] {
            filter.observe(&frame(value, value));
        }
        assert!(!filter.is_duplicate(&frame(0, 0)));
        assert!(filter.is_duplicate(&frame(255, 255)));
    }
}
