use anyhow::Result;
use log::{info, warn};
use std::path::Path;

use crate::model::SlideRecord;
use crate::pptx::PptxWriter;
use super::ManifestStore;

/// Check if a previous session exists and has uncommitted work.
pub fn detect_incomplete_session(output_dir: &Path) -> Result<Option<Vec<SlideRecord>>> {
    let manifest_path = output_dir.join("manifest.jsonl");

    if !manifest_path.exists() {
        return Ok(None);
    }

    let has_tmp_pptx = output_dir.join("output.tmp.pptx").exists();
    let has_final_pptx = output_dir.join("output.pptx").exists();
    let has_previous_pptx = output_dir.join("output.previous.pptx").exists();

    if has_tmp_pptx || (!has_final_pptx && !has_previous_pptx) {
        let manifest_store = ManifestStore::new(manifest_path);
        let records = manifest_store.read_all()?;

        if records.is_empty() {
            return Ok(None);
        }

        info!("Found incomplete session with {} slides", records.len());
        return Ok(Some(records));
    }

    Ok(None)
}

/// Recover slides from a previous session by rebuilding the PPTX from saved PNGs.
pub fn recover_session(output_dir: &Path) -> Result<()> {
    let manifest_path = output_dir.join("manifest.jsonl");
    let manifest_store = ManifestStore::new(manifest_path);
    let records = manifest_store.read_all()?;

    if records.is_empty() {
        return Ok(());
    }

    info!("Recovering session with {} slides…", records.len());

    let pptx_writer = PptxWriter::new(&output_dir.join("output.pptx"), "16:9", "fit");

    let slides_dir = output_dir.join("slides");
    for record in &records {
        let png_path = slides_dir.join(&record.png_filename);
        if png_path.exists() {
            pptx_writer.add_slide(record, &png_path)?;
            info!("Recovered slide {}", record.slide_number);
        } else {
            warn!("Missing PNG for slide {}: {:?}", record.slide_number, png_path);
        }
    }

    info!("Session recovery complete: {} slides", records.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_no_incomplete_session() {
        let dir = tempfile::tempdir().unwrap();
        let result = detect_incomplete_session(dir.path()).unwrap();
        assert!(result.is_none());
    }
}
