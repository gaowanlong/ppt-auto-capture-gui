//! Application configuration, persisted to disk as JSON.

use anyhow::{Context, Result};
use crate::i18n::Language;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Sample interval in milliseconds.
    pub sample_interval_ms: u64,
    /// Number of stable frames required before capture.
    pub stability_frames: u32,
    /// Maximum time to wait for animation stability (ms).
    pub animation_timeout_ms: u64,
    /// Pixel change threshold (0.0 to 1.0).
    pub change_threshold: f64,
    /// Black frame detection threshold (0.0 to 1.0).
    pub black_threshold: f64,
    /// Whether to filter duplicate slides.
    pub filter_duplicates: bool,
    /// Output directory.
    pub output_dir: String,
    /// Output PPTX filename.
    pub output_filename: String,
    /// Page aspect ratio (e.g. "16:9", "4:3").
    pub page_ratio: String,
    /// Image fit mode: "fill" or "fit".
    pub image_fit: String,
    /// Whether to preserve previous.pptx.
    pub keep_previous: bool,
    /// Last used capture source (serialized for persistence).
    pub last_window_hwnd: u64,
    pub last_window_title: String,
    pub last_monitor_hmonitor: u64,
    pub last_monitor_description: String,
    /// Whether to use DXGI (vs GDI).
    pub use_dxgi: bool,
    /// UI language
    pub language: Language,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 500,
            stability_frames: 3,
            animation_timeout_ms: 10000,
            change_threshold: 0.15,
            black_threshold: 0.95,
            filter_duplicates: true,
            output_dir: "output".to_string(),
            output_filename: format!("ppt-capture-{}.pptx", chrono::Local::now().format("%Y%m%d-%H%M%S")),
            page_ratio: "16:9".to_string(),
            image_fit: "fit".to_string(),
            keep_previous: true,
            last_window_hwnd: 0,
            last_window_title: String::new(),
            last_monitor_hmonitor: 0,
            last_monitor_description: String::new(),
            use_dxgi: true,
            language: Language::English,
        }
    }
}

impl AppConfig {
    const CONFIG_FILE: &'static str = "ppt-auto-capture-config.json";

    /// Load config from default location (next to exe / cwd).
    pub fn load() -> Self {
        let path = Path::new(Self::CONFIG_FILE);
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => log::warn!("Failed to parse config file: {}", e),
                    }
                }
                Err(e) => log::warn!("Failed to read config file: {}", e),
            }
        }
        let mut cfg = Self::default();
        // Generate a fresh timestamp-based filename on first run
        cfg.output_filename = format!("ppt-capture-{}.pptx",
            chrono::Local::now().format("%Y%m%d-%H%M%S"));
        cfg
    }

    /// Save config to default location.
    pub fn save(&self) -> Result<()> {
        let path = Path::new(Self::CONFIG_FILE);
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        std::fs::write(path, content)
            .context("Failed to write config file")?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.sample_interval_ms, 500);
        assert_eq!(cfg.stability_frames, 3);
        assert_eq!(cfg.animation_timeout_ms, 10000);
        assert_eq!(cfg.change_threshold, 0.15);
        assert_eq!(cfg.black_threshold, 0.95);
        assert!(cfg.filter_duplicates);
        assert_eq!(cfg.page_ratio, "16:9");
        assert_eq!(cfg.image_fit, "fit");
        assert_eq!(cfg.keep_previous, true);
    }

    #[test]
    fn test_config_serialize_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("Serialization failed");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(deserialized.sample_interval_ms, cfg.sample_interval_ms);
        assert_eq!(deserialized.change_threshold, cfg.change_threshold);
    }
}
