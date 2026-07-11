//! Appends slide records to a JSONL manifest file.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;

use crate::model::SlideRecord;

pub struct ManifestStore {
    path: PathBuf,
}

impl ManifestStore {
    pub fn new(path: PathBuf) -> Self {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_default();
        }
        Self { path }
    }

    /// Append a slide record as a JSON line.
    pub fn append(&self, record: &SlideRecord) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("Failed to open manifest: {:?}", self.path))?;

        let json = serde_json::to_string(record)
            .context("Failed to serialize slide record")?;

        writeln!(file, "{}", json)
            .with_context(|| format!("Failed to write to manifest: {:?}", self.path))?;

        file.flush()?;

        Ok(())
    }

    /// Read all records from the manifest (for recovery).
    pub fn read_all(&self) -> Result<Vec<SlideRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read manifest: {:?}", self.path))?;

        let mut records = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SlideRecord>(line) {
                Ok(r) => records.push(r),
                Err(e) => {
                    log::warn!("Skipping malformed manifest line: {} ({})", line, e);
                }
            }
        }

        Ok(records)
    }
}
