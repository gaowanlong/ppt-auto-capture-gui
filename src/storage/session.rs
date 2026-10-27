use super::ImageStore;
use crate::model::{Frame, SlideRecord};
use crate::pptx::PptxWriter;
use anyhow::{ensure, Context, Result};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub(super) const SLIDES_PER_DECK: usize = 50;

/// Durable source of truth: images first, journal second, published deck last.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SessionStore {
    #[serde(skip)]
    pub directory: PathBuf,
    version: u32,
    filename: String,
    ratio: String,
    fit: String,
    // Older v1 journals have no limit and must recover into their original single deck.
    #[serde(default)]
    max_slides_per_deck: Option<usize>,
    #[serde(default)]
    auto_crop: bool,
    pub completed: bool,
    pub records: Vec<SlideRecord>,
}

pub(super) fn plain_filename(value: &str) -> bool {
    let mut parts = Path::new(value).components();
    matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none()
        && !value.contains(['\\', ':'])
}

pub(super) fn part_filename(filename: &str, part: usize) -> String {
    if part == 0 {
        return filename.to_string();
    }
    let stem = Path::new(filename)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    format!("{stem}-part-{:03}.pptx", part + 1)
}

impl SessionStore {
    pub fn create(root: &Path, filename: &str, ratio: &str, fit: &str) -> Result<Self> {
        ensure!(
            plain_filename(filename) && filename.to_lowercase().ends_with(".pptx"),
            "Invalid PPTX filename"
        );
        let parent = root.join("sessions");
        std::fs::create_dir_all(&parent)?;
        let directory = parent.join(format!(
            "{}-{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S"),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&directory)?;
        ImageStore::new(directory.clone())?;
        let session = Self {
            directory,
            version: 3,
            filename: filename.into(),
            ratio: ratio.into(),
            fit: fit.into(),
            max_slides_per_deck: Some(SLIDES_PER_DECK),
            auto_crop: false,
            completed: false,
            records: Vec::new(),
        };
        session.persist()?;
        Ok(session)
    }

    pub fn open(directory: &Path) -> Result<Self> {
        let mut session: Self =
            serde_json::from_slice(&std::fs::read(directory.join("session.json"))?)
                .context("Invalid session journal; refusing partial recovery")?;
        ensure!(
            matches!(session.version, 1..=3),
            "Unsupported session version"
        );
        ensure!(
            session.version == 1 || session.max_slides_per_deck.is_some(),
            "Segmented session is missing its deck size limit"
        );
        ensure!(
            session.version != 1 || session.max_slides_per_deck.is_none(),
            "Legacy session has an unexpected deck size limit"
        );
        ensure!(
            session.max_slides_per_deck != Some(0),
            "Invalid deck size limit"
        );
        ensure!(
            plain_filename(&session.filename),
            "Invalid session output filename"
        );
        for (index, record) in session.records.iter().enumerate() {
            ensure!(
                record.slide_number as usize == index + 1 && plain_filename(&record.png_filename),
                "Invalid session slide sequence or path"
            );
        }
        session.directory = directory.to_path_buf();
        Ok(session)
    }

    pub fn output_path(&self) -> PathBuf {
        self.output_path_at_count(self.records.len())
    }

    pub fn set_auto_crop(&mut self, enabled: bool) -> Result<()> {
        ensure!(
            self.records.is_empty(),
            "Cannot change crop settings after capture starts"
        );
        let previous = self.auto_crop;
        self.auto_crop = enabled;
        if let Err(error) = self.persist() {
            self.auto_crop = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn output_path_at_count(&self, count: usize) -> PathBuf {
        let part = self
            .max_slides_per_deck
            .map_or(0, |limit| count.saturating_sub(1) / limit);
        self.output_path_for_part(part)
    }

    pub fn output_paths(&self) -> Vec<PathBuf> {
        let limit = self
            .max_slides_per_deck
            .unwrap_or(self.records.len().max(1));
        self.records
            .chunks(limit)
            .enumerate()
            .map(|(part, _)| self.output_path_for_part(part))
            .collect()
    }

    fn output_path_for_part(&self, part: usize) -> PathBuf {
        self.directory.join(part_filename(&self.filename, part))
    }

    fn persist(&self) -> Result<()> {
        let path = self
            .directory
            .join(format!("session-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(&serde_json::to_vec(self)?)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&path, self.directory.join("session.json"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&path);
        }
        result
    }

    pub fn save(&mut self, frame: &Frame, record: SlideRecord) -> Result<()> {
        ensure!(!self.completed, "Session already completed");
        ensure!(record.slide_number > 0, "Invalid slide number");
        let index = record.slide_number as usize - 1;
        ensure!(
            record.png_filename == format!("slide_{:04}.png", record.slide_number),
            "Invalid slide filename"
        );
        if record.slide_number as usize <= self.records.len() {
            let previous = &self.records[record.slide_number.saturating_sub(1) as usize];
            ensure!(
                previous.slide_number == record.slide_number
                    && previous.content_hash == record.content_hash,
                "Conflicting retry; pending slide must be saved first"
            );
        } else {
            ensure!(
                record.slide_number as usize == self.records.len() + 1,
                "Nonsequential slide"
            );
            ImageStore::new(self.directory.clone())?.save_png(frame, record.slide_number)?;
            self.records.push(record);
            if let Err(error) = self.persist() {
                self.records.pop();
                return Err(error);
            }
        }
        if self.version >= 3 {
            Ok(())
        } else {
            self.publish_at(index)
        }
    }

    pub fn publish(&self) -> Result<()> {
        let limit = self
            .max_slides_per_deck
            .unwrap_or(self.records.len().max(1));
        for (part, records) in self.records.chunks(limit).enumerate() {
            self.publish_part(part, records)?;
        }
        Ok(())
    }

    fn publish_at(&self, index: usize) -> Result<()> {
        let limit = self.max_slides_per_deck.unwrap_or(self.records.len());
        let part = index / limit;
        let start = part * limit;
        let end = start.saturating_add(limit).min(self.records.len());
        self.publish_part(part, &self.records[start..end])
    }

    fn publish_part(&self, part: usize, records: &[SlideRecord]) -> Result<()> {
        let records: Vec<_> = records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                let mut local = record.clone();
                local.slide_number = (index + 1) as u32;
                local
            })
            .collect();
        PptxWriter::new(&self.output_path_for_part(part), &self.ratio, &self.fit)
            .with_auto_crop(self.auto_crop)
            .write_slides(&records, &self.directory.join("slides"))
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.version >= 3 {
            self.publish()?;
        } else if !self.records.is_empty() {
            self.publish_at(self.records.len() - 1)?;
        }
        self.mark_completed()
    }

    pub fn recover(&mut self) -> Result<()> {
        self.publish()?;
        self.mark_completed()
    }

    fn mark_completed(&mut self) -> Result<()> {
        self.completed = true;
        if let Err(error) = self.persist() {
            self.completed = false;
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshots_are_saved_immediately_but_decks_are_built_only_on_finish() {
        let root = tempfile::tempdir().unwrap();
        let mut session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        for page in 1..=51 {
            let mut entry = record();
            entry.slide_number = page;
            entry.png_filename = format!("slide_{page:04}.png");
            entry.png_relative_path = format!("slides/slide_{page:04}.png");
            entry.content_hash = format!("{page}");
            session.save(&frame(), entry).unwrap();
        }
        assert!(session.directory.join("slides/slide_0051.png").is_file());
        assert!(!session.directory.join("deck.pptx").exists());
        assert!(!session.directory.join("deck-part-002.pptx").exists());
        session.finish().unwrap();
        for (filename, count) in [("deck.pptx", 50), ("deck-part-002.pptx", 1)] {
            let mut zip = zip::ZipArchive::new(
                std::fs::File::open(session.directory.join(filename)).unwrap(),
            )
            .unwrap();
            assert!(zip.by_name(&format!("ppt/slides/slide{count}.xml")).is_ok());
            assert!(zip
                .by_name(&format!("ppt/slides/slide{}.xml", count + 1))
                .is_err());
        }
    }

    #[test]
    fn recovery_keeps_crop_setting_and_never_alters_the_original_screenshot() {
        use std::io::Read;
        let root = tempfile::tempdir().unwrap();
        let mut session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        session.set_auto_crop(true).unwrap();
        let mut pixels = vec![35u8; 400 * 300 * 4];
        for y in 60..240 {
            for x in 40..360 {
                let offset = (y * 400 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[235, 235, 235, 255]);
            }
        }
        let image = Frame::new(pixels, 400, 300, 1600, 1, 0);
        let mut entry = record();
        entry.width = 400;
        entry.height = 300;
        session.save(&image, entry).unwrap();
        let original_path = session.directory.join("slides/slide_0001.png");
        let original = std::fs::read(&original_path).unwrap();
        let mut reopened = SessionStore::open(&session.directory).unwrap();
        reopened.recover().unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(reopened.output_path()).unwrap()).unwrap();
        let mut embedded = Vec::new();
        zip.by_name("ppt/media/image1.png")
            .unwrap()
            .read_to_end(&mut embedded)
            .unwrap();
        let image = image::load_from_memory(&embedded).unwrap();
        assert!((image.width() as i32 - 320).abs() <= 5);
        assert!((image.height() as i32 - 180).abs() <= 5);
        assert_eq!(std::fs::read(original_path).unwrap(), original);
    }

    #[test]
    fn segmented_sessions_use_a_new_journal_version_and_reject_missing_limit() {
        let root = tempfile::tempdir().unwrap();
        let session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let journal = session.directory.join("session.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&journal).unwrap()).unwrap();
        // Older applications reject versions other than 1, preventing accidental
        // recovery of a split recording as a single incompatible deck.
        assert_eq!(json["version"], 3);
        json.as_object_mut().unwrap().remove("max_slides_per_deck");
        std::fs::write(&journal, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(SessionStore::open(&session.directory).is_err());
    }

    #[test]
    fn old_journal_without_segmentation_recovers_as_one_original_deck() {
        let root = tempfile::tempdir().unwrap();
        let session = SessionStore::create(root.path(), "old.pptx", "16:9", "fit").unwrap();
        let journal = session.directory.join("session.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&journal).unwrap()).unwrap();
        json["version"] = 1.into();
        json.as_object_mut().unwrap().remove("max_slides_per_deck");
        std::fs::write(&journal, serde_json::to_vec(&json).unwrap()).unwrap();
        let mut old = SessionStore::open(&session.directory).unwrap();
        for page in 1..=51 {
            let mut entry = record();
            entry.slide_number = page;
            entry.png_filename = format!("slide_{page:04}.png");
            entry.png_relative_path = format!("slides/slide_{page:04}.png");
            entry.content_hash = format!("{page}");
            old.save(&frame(), entry).unwrap();
        }
        assert_eq!(old.output_path(), session.directory.join("old.pptx"));
        assert!(!session.directory.join("old-part-002.pptx").exists());
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(old.output_path()).unwrap()).unwrap();
        assert!(archive.by_name("ppt/slides/slide51.xml").is_ok());
    }

    #[test]
    fn retry_of_an_earlier_saved_page_republishes_its_own_deck() {
        let root = tempfile::tempdir().unwrap();
        let mut session = crate::storage::create_legacy_segmented_session(
            root.path(),
            "deck.pptx",
            "16:9",
            "fit",
        )
        .unwrap();
        let first_record = record();
        for page in 1..=51 {
            let mut entry = first_record.clone();
            entry.slide_number = page;
            entry.png_filename = format!("slide_{page:04}.png");
            entry.png_relative_path = format!("slides/slide_{page:04}.png");
            if page != 1 {
                entry.content_hash = format!("{page}");
            }
            session.save(&frame(), entry).unwrap();
        }
        let first = session.directory.join("deck.pptx");
        let second = session.directory.join("deck-part-002.pptx");
        let old_second = std::fs::read(&second).unwrap();
        std::fs::remove_file(&first).unwrap();
        session.save(&frame(), first_record).unwrap();
        assert!(first.is_file());
        assert_eq!(std::fs::read(second).unwrap(), old_second);
        assert_eq!(session.records.len(), 51);
    }

    #[test]
    fn repeated_sessions_survive_png_and_journal_faults_without_page_gaps() {
        let root = tempfile::tempdir().unwrap();
        for round in 0..64 {
            let mut session = crate::storage::create_legacy_segmented_session(
                root.path(),
                "same-name.pptx",
                "16:9",
                "fit",
            )
            .unwrap();
            session.save(&frame(), record()).unwrap();
            let original = std::fs::read(session.output_path()).unwrap();
            let mut second = record();
            second.slide_number = 2;
            second.png_filename = "slide_0002.png".into();
            second.png_relative_path = "slides/slide_0002.png".into();
            second.content_hash = format!("round-{round}");
            let blocker = session.directory.join("slides/slide_0002.tmp.png");
            std::fs::create_dir(&blocker).unwrap();
            assert!(session.save(&frame(), second.clone()).is_err());
            assert_eq!(session.records.len(), 1);
            assert_eq!(std::fs::read(session.output_path()).unwrap(), original);
            std::fs::remove_dir(blocker).unwrap();
            let journal = session.directory.join("session.json");
            let saved_journal = std::fs::read(&journal).unwrap();
            std::fs::remove_file(&journal).unwrap();
            std::fs::create_dir(&journal).unwrap();
            assert!(session.save(&frame(), second.clone()).is_err());
            assert_eq!(session.records.len(), 1);
            std::fs::remove_dir(&journal).unwrap();
            std::fs::write(&journal, saved_journal).unwrap();
            session.save(&frame(), second).unwrap();
            let mut reopened = SessionStore::open(&session.directory).unwrap();
            reopened.finish().unwrap();
            assert_eq!(reopened.records.len(), 2);
            let mut zip =
                zip::ZipArchive::new(std::fs::File::open(session.output_path()).unwrap()).unwrap();
            assert!(zip.by_name("ppt/slides/slide1.xml").is_ok());
            assert!(zip.by_name("ppt/slides/slide2.xml").is_ok());
            assert!(zip.by_name("ppt/slides/slide3.xml").is_err());
        }
        assert!(super::super::detect_incomplete_session(root.path())
            .unwrap()
            .is_none());
    }

    fn record() -> SlideRecord {
        SlideRecord::new(crate::model::SlideRecordInput {
            slide_number: 1,
            png_filename: "slide_0001.png".into(),
            png_relative_path: "slides/slide_0001.png".into(),
            frame_index: 1,
            width: 1,
            height: 1,
            content_hash: "red".into(),
            source_name: String::new(),
            monitor_name: String::new(),
        })
    }
    fn frame() -> Frame {
        Frame::new(vec![0, 0, 255, 255], 1, 1, 4, 1, 0)
    }

    #[test]
    fn successive_sessions_never_share_files() {
        let root = tempfile::tempdir().unwrap();
        let mut a = crate::storage::create_legacy_segmented_session(
            root.path(),
            "custom.pptx",
            "4:3",
            "fill",
        )
        .unwrap();
        a.save(&frame(), record()).unwrap();
        let old = std::fs::read(a.output_path()).unwrap();
        let mut b = crate::storage::create_legacy_segmented_session(
            root.path(),
            "custom.pptx",
            "16:9",
            "fit",
        )
        .unwrap();
        b.save(&frame(), record()).unwrap();
        assert_ne!(a.directory, b.directory);
        assert_eq!(std::fs::read(a.output_path()).unwrap(), old);
    }

    #[test]
    fn failed_publication_is_recoverable_and_retry_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let mut session = crate::storage::create_legacy_segmented_session(
            root.path(),
            "custom.pptx",
            "4:3",
            "fill",
        )
        .unwrap();
        std::fs::create_dir(session.output_path()).unwrap();
        let rec = record();
        assert!(session.save(&frame(), rec.clone()).is_err());
        let mut reopened = SessionStore::open(&session.directory).unwrap();
        assert_eq!(reopened.records.len(), 1);
        assert!(!reopened.completed);
        std::fs::remove_dir(session.output_path()).unwrap();
        reopened.save(&frame(), rec).unwrap();
        assert_eq!(reopened.records.len(), 1);
        reopened.finish().unwrap();
        assert!(SessionStore::open(&session.directory).unwrap().completed);
        let file = std::fs::File::open(session.output_path()).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        use std::io::Read;
        let mut xml = String::new();
        zip.by_name("ppt/presentation.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("9144000"));
    }

    #[test]
    fn failed_png_or_journal_does_not_advance_records() {
        let root = tempfile::tempdir().unwrap();
        let mut session = SessionStore::create(root.path(), "deck.pptx", "16:9", "fit").unwrap();
        let blocker = session.directory.join("slides/slide_0001.tmp.png");
        std::fs::create_dir(&blocker).unwrap();
        assert!(session.save(&frame(), record()).is_err());
        assert!(session.records.is_empty());
        std::fs::remove_dir(blocker).unwrap();
        let journal = session.directory.join("session.json");
        std::fs::remove_file(&journal).unwrap();
        std::fs::create_dir(&journal).unwrap();
        assert!(session.save(&frame(), record()).is_err());
        assert!(session.records.is_empty());
        std::fs::remove_dir(journal).unwrap();
        session.save(&frame(), record()).unwrap();
        assert_eq!(session.records[0].slide_number, 1);
    }

    #[test]
    fn refuses_path_traversal() {
        let root = tempfile::tempdir().unwrap();
        assert!(SessionStore::create(root.path(), "../other.pptx", "16:9", "fit").is_err());
    }
}
