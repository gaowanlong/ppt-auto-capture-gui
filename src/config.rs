//! Application configuration, persisted to disk as JSON.

use crate::i18n::Language;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn filename_for_new_operation(filename: &str) -> String {
    let automatic = filename
        .strip_prefix("ppt-capture-")
        .and_then(|s| s.strip_suffix(".pptx"))
        .is_some_and(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d-%H%M%S").is_ok());
    if automatic {
        format!(
            "ppt-capture-{}.pptx",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        )
    } else {
        filename.to_owned()
    }
}

/// How the interface picks a palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    /// Follow the operating system's appearance.
    System,
}

impl ThemeMode {
    /// Resolve to "is dark" using the system appearance when following it.
    /// `system_dark` is `None` when the platform does not report a preference.
    pub fn is_dark(self, system_dark: Option<bool>) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_dark.unwrap_or(false),
        }
    }
}

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
    #[serde(default)]
    pub filename_is_custom: bool,
    /// Page aspect ratio (e.g. "16:9", "4:3").
    pub page_ratio: String,
    /// Image fit mode: "fill" or "fit".
    pub image_fit: String,
    /// Whether to preserve previous.pptx.
    pub keep_previous: bool,
    /// Last used capture source (serialized for persistence).
    pub last_window_hwnd: u64,
    pub last_window_title: String,
    /// Whether user last chose full-screen capture (zero window).
    pub last_full_screen: bool,
    pub last_monitor_hmonitor: u64,
    pub last_monitor_description: String,
    /// Whether to use DXGI (vs GDI).
    pub use_dxgi: bool,
    /// Minimize the main window as soon as a recording starts successfully.
    #[serde(default)]
    pub minimize_on_start: bool,
    /// UI language
    pub language: Language,
    /// Light, dark, or follow the system.
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// Optional picture drawn behind the workspace.
    #[serde(default)]
    pub background_image: String,
    /// How much the background picture is dimmed, 0..=90 percent.
    #[serde(default = "default_background_dim")]
    pub background_dim: f64,
}

pub const DEFAULT_BACKGROUND_DIM: f64 = 45.0;

fn default_background_dim() -> f64 {
    DEFAULT_BACKGROUND_DIM
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sample_interval_ms: 200,
            stability_frames: 2,
            animation_timeout_ms: 10000,
            change_threshold: 0.05,
            black_threshold: 0.80,
            filter_duplicates: true,
            output_dir: "output".to_string(),
            output_filename: format!(
                "ppt-capture-{}.pptx",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ),
            filename_is_custom: false,
            page_ratio: "16:9".to_string(),
            image_fit: "fit".to_string(),
            keep_previous: true,
            last_window_hwnd: 0,
            last_window_title: String::new(),
            last_full_screen: false,
            last_monitor_hmonitor: 0,
            last_monitor_description: String::new(),
            use_dxgi: true,
            minimize_on_start: false,
            language: Language::English,
            theme_mode: ThemeMode::Light,
            background_image: String::new(),
            background_dim: DEFAULT_BACKGROUND_DIM,
        }
    }
}

impl AppConfig {
    const CONFIG_FILE: &'static str = "ppt-auto-capture-config.json";

    /// Absolute path of the settings file for this run.
    ///
    /// Releases before 2.15 wrote the file to the working directory. That is
    /// the executable's folder when a Windows user double-clicks it, but macOS
    /// launches an app bundle with `/` as the working directory, so the write
    /// failed and every launch started from the defaults. The per-user
    /// directory below is writable on all three platforms.
    pub fn path() -> PathBuf {
        Self::path_in(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn path_in(current_dir: &Path) -> PathBuf {
        config_directory(
            current_os(),
            env_path("HOME").as_deref(),
            env_path("APPDATA").as_deref(),
            env_path("XDG_CONFIG_HOME").as_deref(),
            current_dir,
        )
        .join(Self::CONFIG_FILE)
    }

    /// Where releases before 2.15 kept the settings: the working directory.
    fn legacy_path() -> PathBuf {
        PathBuf::from(Self::CONFIG_FILE)
    }

    /// Load config from the per-user directory, migrating an older copy.
    pub fn load() -> Self {
        let path = Self::path();
        log::info!("Settings file: {}", path.display());
        if let Some(config) = Self::load_migrating(&path, &Self::legacy_path()) {
            return config;
        }
        let mut cfg = Self::default();
        // Generate a fresh timestamp-based filename on first run
        cfg.output_filename = format!(
            "ppt-capture-{}.pptx",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        cfg.output_dir = platform_output_dir(&cfg.output_dir);
        cfg
    }

    /// Reads the current settings file, falling back to the pre-2.15 location
    /// in the working directory and copying it forward.
    fn load_migrating(path: &Path, legacy: &Path) -> Option<Self> {
        if let Some(config) = Self::read(path) {
            return Some(config);
        }
        if legacy == path {
            return None;
        }
        let config = Self::read(legacy)?;
        log::info!(
            "Moving settings from {} to {}",
            legacy.display(),
            path.display()
        );
        if let Err(error) = config.save_to(path) {
            log::warn!("Could not move the settings file: {error:#}");
        }
        Some(config)
    }

    /// Save config to the per-user directory, creating it when needed.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }

    fn read(path: &Path) -> Option<Self> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                log::warn!("Failed to read config file {}: {error}", path.display());
                return None;
            }
        };
        match serde_json::from_str::<Self>(&content) {
            Ok(mut config) => {
                config.output_dir = platform_output_dir(&config.output_dir);
                Some(config)
            }
            Err(error) => {
                log::warn!("Failed to parse config file {}: {error}", path.display());
                None
            }
        }
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;
        Ok(())
    }
}

/// Which platform's convention [`config_directory`] should follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigOs {
    MacOs,
    Windows,
    Other,
}

fn current_os() -> ConfigOs {
    if cfg!(target_os = "macos") {
        ConfigOs::MacOs
    } else if cfg!(target_os = "windows") {
        ConfigOs::Windows
    } else {
        ConfigOs::Other
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// The folder that holds the settings file.
///
/// Returns the working directory when the platform gives no usable base (for
/// example a stripped environment), which keeps the old behaviour working
/// instead of failing outright.
fn config_directory(
    os: ConfigOs,
    home: Option<&Path>,
    appdata: Option<&Path>,
    xdg_config_home: Option<&Path>,
    current_dir: &Path,
) -> PathBuf {
    let directory = match os {
        ConfigOs::MacOs => {
            home.map(|home| home.join("Library/Application Support/PPT Auto Capture"))
        }
        ConfigOs::Windows => appdata.map(|appdata| appdata.join("PPT Auto Capture")),
        ConfigOs::Other => xdg_config_home
            .map(Path::to_path_buf)
            .or_else(|| home.map(|home| home.join(".config")))
            .map(|base| base.join("ppt-auto-capture")),
    };
    directory.unwrap_or_else(|| current_dir.to_path_buf())
}

fn platform_output_dir(configured: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        resolve_output_dir(Path::new(configured), home.as_deref(), true)
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(target_os = "macos"))]
    {
        configured.to_string()
    }
}

fn resolve_output_dir(path: &Path, home: Option<&Path>, is_macos: bool) -> PathBuf {
    if path.is_absolute() || !is_macos {
        return path.to_path_buf();
    }
    let Some(home) = home else {
        return path.to_path_buf();
    };
    let base = home.join("Documents").join("PPT Auto Capture");
    if path == Path::new("output") || path.as_os_str().is_empty() {
        base
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.sample_interval_ms, 200);
        assert_eq!(cfg.stability_frames, 2);
        assert_eq!(cfg.animation_timeout_ms, 10000);
        assert_eq!(cfg.change_threshold, 0.05);
        assert_eq!(cfg.black_threshold, 0.80);
        assert!(cfg.filter_duplicates);
        assert_eq!(cfg.page_ratio, "16:9");
        assert_eq!(cfg.image_fit, "fit");
        assert!(cfg.keep_previous);
        assert!(!cfg.minimize_on_start);
    }

    #[test]
    fn test_config_serialize_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("Serialization failed");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(deserialized.sample_interval_ms, cfg.sample_interval_ms);
        assert_eq!(deserialized.change_threshold, cfg.change_threshold);
    }

    #[test]
    fn config_from_an_older_version_still_loads() {
        // The capture-time crop is no longer remembered, so the key stays in
        // config files written by older versions. It must not stop the file
        // from loading.
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("auto_crop".into(), serde_json::Value::Bool(true));
        let restored: AppConfig = serde_json::from_value(value).unwrap();
        assert_eq!(restored.page_ratio, AppConfig::default().page_ratio);
    }

    #[test]
    fn old_config_without_minimize_option_keeps_the_window_visible() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        value.as_object_mut().unwrap().remove("minimize_on_start");
        let restored: AppConfig = serde_json::from_value(value).unwrap();
        assert!(!restored.minimize_on_start);
    }

    #[test]
    fn old_config_without_appearance_options_keeps_a_light_themed_app() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("theme_mode");
        object.remove("background_image");
        object.remove("background_dim");
        let restored: AppConfig = serde_json::from_value(value).unwrap();
        assert_eq!(restored.theme_mode, ThemeMode::Light);
        assert!(restored.background_image.is_empty());
        assert_eq!(restored.background_dim, DEFAULT_BACKGROUND_DIM);
    }

    #[test]
    fn theme_mode_resolves_against_the_system_appearance() {
        assert!(!ThemeMode::Light.is_dark(Some(true)));
        assert!(ThemeMode::Dark.is_dark(Some(false)));
        assert!(ThemeMode::System.is_dark(Some(true)));
        assert!(!ThemeMode::System.is_dark(Some(false)));
        // A platform that does not report a preference stays light.
        assert!(!ThemeMode::System.is_dark(None));
    }

    #[test]
    fn settings_live_in_the_user_config_directory_on_every_platform() {
        let home = Path::new("/Users/tester");
        let appdata = Path::new("/Users/tester/AppData/Roaming");
        let cwd = Path::new("/");

        assert_eq!(
            config_directory(ConfigOs::MacOs, Some(home), None, None, cwd),
            Path::new("/Users/tester/Library/Application Support/PPT Auto Capture")
        );
        assert_eq!(
            config_directory(ConfigOs::Windows, None, Some(appdata), None, cwd),
            Path::new("/Users/tester/AppData/Roaming/PPT Auto Capture")
        );
        assert_eq!(
            config_directory(ConfigOs::Other, Some(home), None, None, cwd),
            Path::new("/Users/tester/.config/ppt-auto-capture")
        );
        assert_eq!(
            config_directory(
                ConfigOs::Other,
                Some(home),
                None,
                Some(Path::new("/etc/xdg")),
                cwd
            ),
            Path::new("/etc/xdg/ppt-auto-capture")
        );
    }

    #[test]
    fn settings_fall_back_to_the_working_directory_without_a_home() {
        assert_eq!(
            config_directory(ConfigOs::MacOs, None, None, None, Path::new("/tmp/run")),
            Path::new("/tmp/run")
        );
        assert_eq!(
            config_directory(ConfigOs::Windows, None, None, None, Path::new("/tmp/run")),
            Path::new("/tmp/run")
        );
    }

    #[test]
    fn saving_creates_the_directory_and_round_trips_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("PPT Auto Capture")
            .join(AppConfig::CONFIG_FILE);
        assert!(!path.parent().unwrap().exists());

        let mut config = AppConfig::default();
        config.sample_interval_ms = 500;
        config.theme_mode = ThemeMode::Dark;
        config.save_to(&path).unwrap();

        let restored = AppConfig::read(&path).expect("saved settings should load");
        assert_eq!(restored.sample_interval_ms, 500);
        assert_eq!(restored.theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn a_missing_settings_file_loads_nothing_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(AppConfig::read(&dir.path().join("absent.json")).is_none());
    }

    #[test]
    fn the_settings_path_is_absolute_and_never_the_working_directory() {
        let path = AppConfig::path();
        assert!(path.is_absolute(), "{} is not absolute", path.display());
        assert_eq!(path.file_name().unwrap(), AppConfig::CONFIG_FILE);
        if cfg!(target_os = "macos") && std::env::var_os("HOME").is_some() {
            assert!(
                path.starts_with("Library/Application Support/PPT Auto Capture")
                    || path
                        .to_string_lossy()
                        .contains("/Library/Application Support/"),
                "macOS settings should live in the user library, got {}",
                path.display()
            );
        }
    }

    #[test]
    fn settings_from_the_old_location_are_copied_forward() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("ppt-auto-capture-config.json");
        let current = dir
            .path()
            .join("config")
            .join("ppt-auto-capture-config.json");

        let mut old = AppConfig::default();
        old.stability_frames = 5;
        old.save_to(&legacy).unwrap();
        assert!(!current.exists());

        let loaded = AppConfig::load_migrating(&current, &legacy).expect("legacy settings");
        assert_eq!(loaded.stability_frames, 5);
        assert!(current.exists(), "the old file should be copied forward");
        assert_eq!(
            AppConfig::read(&current).unwrap().stability_frames,
            5,
            "the copied file should be readable next time"
        );
    }

    #[test]
    fn the_current_location_wins_over_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("old.json");
        let current = dir.path().join("new.json");

        let mut old = AppConfig::default();
        old.stability_frames = 5;
        old.save_to(&legacy).unwrap();
        let mut new = AppConfig::default();
        new.stability_frames = 3;
        new.save_to(&current).unwrap();

        let loaded = AppConfig::load_migrating(&current, &legacy).expect("current settings");
        assert_eq!(loaded.stability_frames, 3);
    }

    #[test]
    fn macos_default_output_is_in_the_users_documents_directory() {
        let resolved =
            resolve_output_dir(Path::new("output"), Some(Path::new("/Users/tester")), true);
        assert_eq!(
            resolved,
            Path::new("/Users/tester/Documents/PPT Auto Capture")
        );
    }

    #[test]
    fn absolute_output_directory_is_preserved() {
        let resolved = resolve_output_dir(
            Path::new("/Volumes/Captures"),
            Some(Path::new("/Users/tester")),
            true,
        );
        assert_eq!(resolved, Path::new("/Volumes/Captures"));
    }
}
