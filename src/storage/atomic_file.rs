//! Atomic file write: write to .tmp then rename to final path.
//! Ensures crash-safe file operations.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Atomically write data to a file: writes to a .tmp file, then renames.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp_path = path.with_extension("tmp");

    // Write to temp file
    std::fs::write(&tmp_path, data)
        .with_context(|| format!("Failed to write temp file: {:?}", tmp_path))?;

    // Atomic rename (on same filesystem)
    std::fs::rename(&tmp_path, path)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp_path, path))?;

    Ok(())
}

/// Atomically copy a file to a destination via tmp + rename.
pub fn atomic_copy(src: &Path, dst: &Path) -> Result<()> {
    let tmp_path = dst.with_extension("tmp");

    std::fs::copy(src, &tmp_path)
        .with_context(|| format!("Failed to copy {:?} to {:?}", src, tmp_path))?;

    std::fs::rename(&tmp_path, dst)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp_path, dst))?;

    Ok(())
}

/// Create a backup by copying to a .bak / .previous file.
pub fn create_backup(src: &Path) -> Result<Option<PathBuf>> {
    if !src.exists() {
        return Ok(None);
    }

    let backup_path = src.with_extension("previous.pptx");
    std::fs::copy(src, &backup_path)
        .with_context(|| format!("Failed to create backup {:?}", backup_path))?;

    Ok(Some(backup_path))
}
