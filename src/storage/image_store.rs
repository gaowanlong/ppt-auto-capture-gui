//! Saves frames as PNG files with atomic writes.

use anyhow::{Context, Result};
use log::info;
use std::path::PathBuf;

use crate::model::Frame;

/// Manages the PNG image storage directory.
pub struct ImageStore {
    output_dir: PathBuf,
    slides_dir: PathBuf,
}

impl ImageStore {
    pub fn new(output_dir: PathBuf) -> Self {
        let slides_dir = output_dir.join("slides");
        std::fs::create_dir_all(&slides_dir).unwrap_or_default();
        Self {
            output_dir,
            slides_dir,
        }
    }

    /// Save a frame as a PNG file. Returns the path to the saved file.
    pub fn save_png(&self, frame: &Frame, slide_number: u32) -> Result<PathBuf> {
        let filename = format!("slide_{:04}.png", slide_number);
        let filepath = self.slides_dir.join(&filename);

        // Convert BGRA frame data to RGB (drop alpha — some PowerPoint versions 
        // fail to render RGBA PNG and delete the image content silently).
        let mut rgb_data = Vec::with_capacity((frame.width as usize * frame.height as usize * 3) as usize);

        for y in 0..frame.height {
            for x in 0..frame.width {
                let offset = (y * frame.stride + x * 4) as usize;
                if offset + 3 < frame.data.len() {
                    let b = frame.data[offset];
                    let g = frame.data[offset + 1];
                    let r = frame.data[offset + 2];
                    rgb_data.push(r);
                    rgb_data.push(g);
                    rgb_data.push(b);
                }
            }
        }

        let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(frame.width, frame.height, rgb_data)
            .context("Failed to create image buffer")?;

        // Save as PNG atomically
        let tmp_path = filepath.with_extension("tmp.png");
        img.save(&tmp_path)
            .with_context(|| format!("Failed to save PNG to {:?}", tmp_path))?;

        std::fs::rename(&tmp_path, &filepath)
            .with_context(|| format!("Failed to rename {:?} to {:?}", tmp_path, filepath))?;

        info!("Saved PNG: {}", filepath.display());

        Ok(filepath)
    }
}
