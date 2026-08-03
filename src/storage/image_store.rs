//! Saves frames as PNG files with atomic writes.

use anyhow::{Context, Result};
use log::info;
use std::path::PathBuf;

use crate::model::Frame;

/// Manages the PNG image storage directory.
pub struct ImageStore {
    slides_dir: PathBuf,
}

impl ImageStore {
    pub fn new(output_dir: PathBuf) -> Result<Self> {
        let slides_dir = output_dir.join("slides");
        std::fs::create_dir_all(&slides_dir)
            .with_context(|| format!("Failed to create slides directory {:?}", slides_dir))?;
        Ok(Self { slides_dir })
    }

    /// Save a frame as a PNG file. Returns the path to the saved file.
    pub fn save_png(&self, frame: &Frame, slide_number: u32) -> Result<PathBuf> {
        let filename = format!("slide_{:04}.png", slide_number);
        let filepath = self.slides_dir.join(&filename);

        // Convert BGRA frame data to RGB (drop alpha — some PowerPoint versions
        // fail to render RGBA PNG and delete the image content silently).
        let mut rgb_data = Vec::with_capacity(frame.width as usize * frame.height as usize * 3);

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

        let img =
            image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(frame.width, frame.height, rgb_data)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_png_under_the_derived_slides_directory() {
        let temp = tempfile::tempdir().unwrap();
        let store = ImageStore::new(temp.path().to_path_buf()).unwrap();
        let frame = Frame::new(vec![0, 0, 255, 255], 1, 1, 4, 0, 0);

        let saved = store.save_png(&frame, 3).unwrap();

        assert_eq!(saved, temp.path().join("slides/slide_0003.png"));
        assert!(saved.is_file());
    }

    #[test]
    fn constructor_reports_when_slides_directory_cannot_be_created() {
        let temp = tempfile::tempdir().unwrap();
        let blocking_file = temp.path().join("not-a-directory");
        std::fs::write(&blocking_file, b"file").unwrap();

        let error = match ImageStore::new(blocking_file) {
            Ok(_) => panic!("constructor unexpectedly succeeded"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("slides"));
    }
}
