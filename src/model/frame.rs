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
    fn test_frame_clone_preserves_pixel_data() {
        let mut data = vec![0u8; 4 * 4 * 4];  // 4x4 RGBA
        // Set each pixel to a unique value
        for i in 0..data.len() { data[i] = (i % 256) as u8; }
        let f1 = Frame::new(data, 4, 4, 16, 1, 1000);
        let f2 = f1.clone();
        assert_eq!(f1.width, f2.width);
        assert_eq!(f1.height, f2.height);
        assert_eq!(f1.stride, f2.stride);
        assert_eq!(f1.data, f2.data, "Cloned frame pixel data should match");
    }

    #[test]
    fn test_frame_thumbnail_checksum() {
        let mut data = vec![0u8; 100 * 100 * 4];
        // Red rectangle in the center
        for y in 25..75 {
            for x in 25..75 {
                let idx = (y * 400 + x * 4) as usize;
                data[idx] = 0;     // B
                data[idx+1] = 0;   // G
                data[idx+2] = 255; // R
                data[idx+3] = 255; // A
            }
        }
        let f = Frame::new(data, 100, 100, 400, 1, 1000);
        let thumb = f.thumbnail(50, 50);
        // Thumbnail should be 50x50 RGBA = 10000 bytes
        assert_eq!(thumb.len(), 10000, "Thumbnail should be 50x50x4");
        // There should be some red pixels (R > 0) in the thumbnail
        let has_red = thumb.chunks(4).any(|px| px[2] > 0);
        assert!(has_red, "Thumbnail should contain red pixels from original");
    }
    #[test]
    fn test_thumbnail_upscale_not_happens() {
        let f = make_frame(0, 255, 0, 10, 10);
        let thumb = f.thumbnail(100, 100);
        // Scale is capped at 1.0, so thumb should be same size: 10x10x4 = 400
        assert_eq!(thumb.len(), 400, "Should not upscale");
    }



    /// All-zero frame.
    #[test]
    fn test_empty_frame_all_zero() {
        let f = Frame::new(vec![0u8; 400], 10, 10, 40, 0, 0);
        assert_eq!(f.luminance_at(5, 5), 0);
        let thumb = f.thumbnail(5, 5);
        assert_eq!(thumb.len(), 5 * 5 * 4);
    }

    /// Frame with padded stride.
    #[test]
    fn test_frame_padded_stride() {
        let stride = 28; let w = 6; let h = 5;
        let mut data = vec![0u8; (stride * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let off = (y * stride + x * 4) as usize;
                data[off] = (x * 255 / w) as u8; data[off+1] = (y * 255 / h) as u8;
                data[off+2] = 128; data[off+3] = 255;
            }
        }
        let f = Frame::new(data, w, h, stride, 0, 0);
        assert!(f.luminance_at(0, 0) > 0);
        assert_eq!(f.luminance_at(100, 100), 0, "OOB should return 0");
    }

    /// Very small frame thumbnail — no upscaling.
    #[test]
    fn test_tiny_frame_no_upscale() {
        let f = Frame::new(vec![100u8; 64], 4, 4, 16, 0, 0);
        let thumb = f.thumbnail(100, 100);
        assert_eq!(thumb.len(), 64, "Tiny frame should not upscale: 4x4x4=64");
    }
}
