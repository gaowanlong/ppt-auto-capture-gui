use super::ImageStore;
use crate::model::{Frame, SlideRecord};
use crate::pptx::PptxWriter;
use anyhow::{ensure, Context, Result};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Durable source of truth: images first, journal second, published deck last.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SessionStore {
    #[serde(skip)]
    pub directory: PathBuf,
    version: u32,
    filename: String,
    ratio: String,
    fit: String,
    pub completed: bool,
    pub records: Vec<SlideRecord>,
}

fn plain_filename(value: &str) -> bool {
    let mut parts = Path::new(value).components();
    matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none()
        && !value.contains(['\\', ':'])
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
            version: 1,
            filename: filename.into(),
            ratio: ratio.into(),
            fit: fit.into(),
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
        ensure!(session.version == 1, "Unsupported session version");
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
        self.directory.join(&self.filename)
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
        self.publish()
    }

    pub fn publish(&self) -> Result<()> {
        if self.records.is_empty() {
            return Ok(());
        }
        PptxWriter::new(&self.output_path(), &self.ratio, &self.fit)
            .write_slides(&self.records, &self.directory.join("slides"))
    }

    pub fn finish(&mut self) -> Result<()> {
        self.publish()?;
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
        let mut a = SessionStore::create(root.path(), "custom.pptx", "4:3", "fill").unwrap();
        a.save(&frame(), record()).unwrap();
        let old = std::fs::read(a.output_path()).unwrap();
        let mut b = SessionStore::create(root.path(), "custom.pptx", "16:9", "fit").unwrap();
        b.save(&frame(), record()).unwrap();
        assert_ne!(a.directory, b.directory);
        assert_eq!(std::fs::read(a.output_path()).unwrap(), old);
    }

    #[test]
    fn failed_publication_is_recoverable_and_retry_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let mut session = SessionStore::create(root.path(), "custom.pptx", "4:3", "fill").unwrap();
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
