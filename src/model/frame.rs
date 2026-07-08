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
