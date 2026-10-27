use anyhow::{Context, Result};
use log::info;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::{ManifestStore, SessionStore};
use crate::model::SlideRecord;
use crate::pptx::PptxWriter;

/// Check if a previous session exists and has uncommitted work.
pub fn detect_incomplete_session(output_dir: &Path) -> Result<Option<Vec<SlideRecord>>> {
    let mut pending = Vec::new();
    for session in sessions(output_dir)? {
        if !session.completed {
            pending.extend(session.records);
        }
    }
    if let Some(records) = legacy_incomplete_session(output_dir)? {
        pending.extend(records);
    }
    Ok(if pending.is_empty() {
        None
    } else {
        Some(pending)
    })
}

fn legacy_incomplete_session(output_dir: &Path) -> Result<Option<Vec<SlideRecord>>> {
    let manifest_path = output_dir.join("manifest.jsonl");

    if !manifest_path.exists() {
        return Ok(None);
    }
    let marker = output_dir.join("legacy-recovery.json");
    if marker.exists() {
        let recovered: LegacyRecovery = serde_json::from_slice(&std::fs::read(marker)?)
            .context("Invalid legacy recovery marker")?;
        if recovered.manifest_hash == manifest_hash(output_dir)? {
            return Ok(None);
        }
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
pub fn recover_session(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut recovered = Vec::new();
    for mut session in sessions(output_dir)? {
        if !session.completed && !session.records.is_empty() {
            session.recover()?;
            recovered.extend(session.output_paths());
        }
    }
    let Some(records) = legacy_incomplete_session(output_dir)? else {
        return Ok(recovered);
    };

    info!("Recovering session with {} slides…", records.len());

    let filename = format!("legacy-recovered-{}.pptx", uuid::Uuid::new_v4());
    let pptx_writer = PptxWriter::new(&output_dir.join(&filename), "16:9", "fit");

    pptx_writer.write_slides(&records, &output_dir.join("slides"))?;
    super::atomic_file::atomic_write(
        &output_dir.join("legacy-recovery.json"),
        &serde_json::to_vec(&LegacyRecovery {
            manifest_hash: manifest_hash(output_dir)?,
            output_filename: filename.clone(),
        })?,
    )?;

    info!("Session recovery complete: {} slides", records.len());
    recovered.push(output_dir.join(filename));
    Ok(recovered)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyRecovery {
    manifest_hash: String,
    output_filename: String,
}

fn manifest_hash(root: &Path) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(std::fs::read(root.join("manifest.jsonl"))?)
    ))
}

fn sessions(root: &Path) -> Result<Vec<SessionStore>> {
    let parent = root.join("sessions");
    if !parent.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let path = entry?.path();
        if path.join("session.json").exists() {
            result.push(SessionStore::open(&path)?);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_rebuilds_and_reports_every_deck_in_an_interrupted_long_recording() {
        let root = tempfile::tempdir().unwrap();
        let mut session =
            crate::storage::create_legacy_segmented_session(root.path(), "deck.pptx", "4:3", "fit")
                .unwrap();
        for page in 1..=51 {
            let record = SlideRecord::new(crate::model::SlideRecordInput {
                slide_number: page,
                png_filename: format!("slide_{page:04}.png"),
                png_relative_path: format!("slides/slide_{page:04}.png"),
                frame_index: page as u64,
                width: 1,
                height: 1,
                content_hash: format!("{page}"),
                source_name: String::new(),
                monitor_name: String::new(),
            });
            session
                .save(
                    &crate::model::Frame::new(
                        vec![page as u8, 0, 255, 255],
                        1,
                        1,
                        4,
                        page as u64,
                        0,
                    ),
                    record,
                )
                .unwrap();
        }
        let first = session.directory.join("deck.pptx");
        let second = session.directory.join("deck-part-002.pptx");
        std::fs::remove_file(&first).unwrap();
        std::fs::remove_file(&second).unwrap();
        assert_eq!(
            detect_incomplete_session(root.path())
                .unwrap()
                .unwrap()
                .len(),
            51
        );
        assert_eq!(
            recover_session(root.path()).unwrap(),
            vec![first.clone(), second.clone()]
        );
        let mut one = zip::ZipArchive::new(std::fs::File::open(first).unwrap()).unwrap();
        assert!(one.by_name("ppt/slides/slide50.xml").is_ok());
        assert!(one.by_name("ppt/slides/slide51.xml").is_err());
        let mut two = zip::ZipArchive::new(std::fs::File::open(second).unwrap()).unwrap();
        assert!(two.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(two.by_name("ppt/slides/slide2.xml").is_err());
        assert!(detect_incomplete_session(root.path()).unwrap().is_none());
    }

    fn captured_session(root: &Path) -> SessionStore {
        let mut session =
            crate::storage::create_legacy_segmented_session(root, "my-slides.pptx", "4:3", "fill")
                .unwrap();
        let record = SlideRecord::new(crate::model::SlideRecordInput {
            slide_number: 1,
            png_filename: "slide_0001.png".into(),
            png_relative_path: "slides/slide_0001.png".into(),
            frame_index: 1,
            width: 2,
            height: 1,
            content_hash: "test".into(),
            source_name: String::new(),
            monitor_name: String::new(),
        });
        session
            .save(
                &crate::model::Frame::new(vec![255; 8], 2, 1, 8, 1, 0),
                record,
            )
            .unwrap();
        session
    }

    #[test]
    fn recovers_multiple_sessions_with_original_names_and_settings_once() {
        let root = tempfile::tempdir().unwrap();
        let first = captured_session(root.path());
        let second = captured_session(root.path());
        let expected = std::fs::read(first.output_path()).unwrap();
        std::fs::remove_file(first.output_path()).unwrap();
        assert_eq!(
            detect_incomplete_session(root.path())
                .unwrap()
                .unwrap()
                .len(),
            2
        );
        recover_session(root.path()).unwrap();
        assert_eq!(std::fs::read(first.output_path()).unwrap(), expected);
        assert!(second.output_path().is_file());
        assert!(detect_incomplete_session(root.path()).unwrap().is_none());
        recover_session(root.path()).unwrap();
        assert_eq!(std::fs::read(first.output_path()).unwrap(), expected);
        assert!(!root.path().join("output.pptx").exists());
    }

    #[test]
    fn missing_image_never_marks_recovery_complete_or_replaces_good_deck() {
        let root = tempfile::tempdir().unwrap();
        let session = captured_session(root.path());
        let old = std::fs::read(session.output_path()).unwrap();
        std::fs::remove_file(session.directory.join("slides/slide_0001.png")).unwrap();
        assert!(recover_session(root.path()).is_err());
        assert!(!SessionStore::open(&session.directory).unwrap().completed);
        assert_eq!(std::fs::read(session.output_path()).unwrap(), old);
    }

    #[test]
    fn corrupt_journal_is_reported_not_silently_skipped() {
        let root = tempfile::tempdir().unwrap();
        let session = captured_session(root.path());
        std::fs::write(session.directory.join("session.json"), b"{truncated").unwrap();
        assert!(detect_incomplete_session(root.path()).is_err());
        assert!(recover_session(root.path()).is_err());
    }

    #[test]
    fn legacy_recovery_does_not_overwrite_original_or_repeat_for_stale_tmp() {
        let root = tempfile::tempdir().unwrap();
        let session = captured_session(root.path());
        let store = ManifestStore::new(root.path().join("manifest.jsonl"));
        store.append(&session.records[0]).unwrap();
        std::fs::create_dir(root.path().join("slides")).unwrap();
        std::fs::copy(
            session.directory.join("slides/slide_0001.png"),
            root.path().join("slides/slide_0001.png"),
        )
        .unwrap();
        std::fs::write(root.path().join("output.pptx"), b"preserve original").unwrap();
        std::fs::write(root.path().join("output.tmp.pptx"), b"stale partial").unwrap();
        let paths = recover_session(root.path()).unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(
            std::fs::read(root.path().join("output.pptx")).unwrap(),
            b"preserve original"
        );
        assert!(detect_incomplete_session(root.path()).unwrap().is_none());
        assert!(recover_session(root.path()).unwrap().is_empty());
        std::fs::write(root.path().join("manifest.jsonl"), b"{truncated\n").unwrap();
        assert!(recover_session(root.path()).is_err());
    }

    #[test]
    fn test_no_incomplete_session() {
        let dir = tempfile::tempdir().unwrap();
        let result = detect_incomplete_session(dir.path()).unwrap();
        assert!(result.is_none());
    }
}
