use crate::model::Region;

#[derive(Debug, Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub region: Region,
    pub frame_index: u64,
    pub timestamp_ms: u64,
    pub is_blank: bool,
}

impl Frame {
    pub fn new(data: Vec<u8>, width: u32, height: u32, stride: u32, frame_index: u64, timestamp_ms: u64) -> Self {
        let region = Region::new(0, 0, width, height);
        Self {
            data,
            width,
            height,
            stride,
            region,
            frame_index,
            timestamp_ms,
            is_blank: false,
        }
    }

    pub fn luminance_at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let offset = (y * self.stride + x * 4) as usize;
        if offset + 3 >= self.data.len() {
            return 0;
        }
        let b = self.data[offset] as u32;
        let g = self.data[offset + 1] as u32;
        let r = self.data[offset + 2] as u32;
        ((r as f32 * 0.299) + (g as f32 * 0.587) + (b as f32 * 0.114)) as u8
    }

    /// Resize to a thumbnail for preview (simple nearest-neighbor downscale).
    pub fn thumbnail(&self, max_width: u32, max_height: u32) -> Vec<u8> {
        let scale_w = max_width as f32 / self.width as f32;
        let scale_h = max_height as f32 / self.height as f32;
        let scale = scale_w.min(scale_h).min(1.0);
        let new_w = (self.width as f32 * scale) as usize;
        let new_h = (self.height as f32 * scale) as usize;
        let new_w = new_w.max(1);
        let new_h = new_h.max(1);

        let mut thumb = vec![0u8; (new_w * new_h * 4) as usize];
        for ty in 0..new_h {
            for tx in 0..new_w {
                let sx = (tx as f32 / scale) as u32;
                let sy = (ty as f32 / scale) as u32;
                let src_offset = (sy * self.stride + sx * 4) as usize;
                let dst_offset = (ty * new_w + tx) * 4;
                if src_offset + 3 < self.data.len() && dst_offset + 3 < thumb.len() {
                    thumb[dst_offset] = self.data[src_offset];
                    thumb[dst_offset + 1] = self.data[src_offset + 1];
                    thumb[dst_offset + 2] = self.data[src_offset + 2];
                    thumb[dst_offset + 3] = 255;
                }
            }
        }
        thumb
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple colored frame
    fn make_frame(r_val: u8, g_val: u8, b_val: u8, w: u32, h: u32) -> Frame {
        let stride = w * 4;
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * stride + x * 4) as usize;
                data[idx] = b_val;     // B
                data[idx+1] = g_val;   // G
                data[idx+2] = r_val;   // R
                data[idx+3] = 255;     // A
            }
        }
        Frame::new(data, w, h, stride, 1, 1000)
    }

    #[test]
    fn test_frame_new() {
        let f = make_frame(255, 0, 0, 10, 10);
        assert_eq!(f.width, 10);
        assert_eq!(f.height, 10);
        assert_eq!(f.stride, 40);
        assert_eq!(f.data.len(), 400);
        assert_eq!(f.frame_index, 1);
        assert_eq!(f.timestamp_ms, 1000);
    }

    #[test]
    fn test_luminance_at() {
        // White frame → luminance ~255
        let white = make_frame(255, 255, 255, 5, 5);
        assert_eq!(white.luminance_at(2, 2), 255);
        // Black frame → luminance ~0
        let black = make_frame(0, 0, 0, 5, 5);
        assert_eq!(black.luminance_at(2, 2), 0);
        // Red frame → R=255, G=0, B=0 → luminance = 255*0.299 ≈ 76
        let red = make_frame(255, 0, 0, 5, 5);
        assert_eq!(red.luminance_at(2, 2), 76);
        // Green frame → R=0, G=255, B=0 → luminance = 255*0.587 ≈ 149
        let green = make_frame(0, 255, 0, 5, 5);
        assert_eq!(green.luminance_at(2, 2), 149);
    }

    #[test]
    fn test_luminance_out_of_bounds() {
        let f = make_frame(128, 128, 128, 10, 10);
        assert_eq!(f.luminance_at(100, 100), 0);
        assert_eq!(f.luminance_at(10, 10), 0);
    }

    #[test]
    fn test_thumbnail_downscale() {
        let f = make_frame(255, 0, 0, 100, 50);
        let thumb = f.thumbnail(20, 20);
        // 100x50 scaled to fit within 20x20: scale = min(20/100, 20/50) = 0.2
        // new_w = 100 * 0.2 = 20, new_h = 50 * 0.2 = 10
        // size = 20 * 10 * 4 = 800
        assert_eq!(thumb.len(), 800, "Expected 20x10x4 = 800 bytes");
    }

    #[test]
    fn test_thumbnail_upscale_not_happens() {
        let f = make_frame(0, 255, 0, 10, 10);
        let thumb = f.thumbnail(100, 100);
        // Scale is capped at 1.0, so thumb should be same size: 10x10x4 = 400
        assert_eq!(thumb.len(), 400, "Should not upscale");
    }
}
