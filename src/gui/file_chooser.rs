//! Native "open file" and "pick a folder" dialogs.
//!
//! Each platform is driven through its own small helper: PowerShell + WinForms
//! on Windows, AppleScript on macOS, and zenity on Linux. The helpers are
//! started without a console window, and they answer with the chosen path only
//! — see [`decode_hex_text`] for why Windows sends hex.

use std::path::{Path, PathBuf};

/// What the file dialog should let the user pick.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChooserKind {
    /// A `.pptx` deck.
    Presentation,
    /// A picture that can be used as the workspace background.
    Image,
}

impl ChooserKind {
    /// The zenity `--file-filter` value.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn zenity_filter(self) -> &'static str {
        match self {
            Self::Presentation => "--file-filter=PowerPoint (*.pptx) | *.pptx",
            Self::Image => "--file-filter=Image (*.png;*.jpg;*.jpeg;*.bmp;*.webp) | *.png;*.jpg;*.jpeg;*.bmp;*.webp",
        }
    }

    /// The AppleScript `of type` clause, empty for "any file".
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn apple_types(self) -> &'static str {
        match self {
            Self::Presentation => " of type {\"org.openxmlformats.presentationml.presentation\"}",
            Self::Image => " of type {\"public.image\"}",
        }
    }

    /// The dialog title.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn prompt(self) -> &'static str {
        match self {
            Self::Presentation => "Select a PPTX",
            Self::Image => "Select a background picture",
        }
    }
}

/// The Windows choosers print the chosen path as the hex of its UTF-8 bytes.
///
/// Plain text would travel through the console code page, which turns any path
/// that is not ASCII into question marks, and the usual workaround — setting
/// `[Console]::OutputEncoding` — is exactly what a helper without a console
/// cannot do. Hex is ASCII whatever the code page is.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_FOLDER_CHOOSER: &str = "Add-Type -AssemblyName System.Windows.Forms; \
$d = New-Object System.Windows.Forms.FolderBrowserDialog; \
$d.SelectedPath = $env:PPT_CAPTURE_INITIAL_FOLDER; \
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
$b = [System.Text.Encoding]::UTF8.GetBytes($d.SelectedPath); \
([System.BitConverter]::ToString($b)).Replace('-', '') }";

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_PRESENTATION_CHOOSER: &str = "Add-Type -AssemblyName System.Windows.Forms; \
$d = New-Object System.Windows.Forms.OpenFileDialog; \
$d.Filter = 'PowerPoint presentation (*.pptx)|*.pptx'; \
$d.InitialDirectory = $env:PPT_CAPTURE_INITIAL_FOLDER; \
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
$b = [System.Text.Encoding]::UTF8.GetBytes($d.FileName); \
([System.BitConverter]::ToString($b)).Replace('-', '') }";

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const WINDOWS_IMAGE_CHOOSER: &str = "Add-Type -AssemblyName System.Windows.Forms; \
$d = New-Object System.Windows.Forms.OpenFileDialog; \
$d.Filter = 'Image (*.png;*.jpg;*.jpeg;*.bmp;*.webp)|*.png;*.jpg;*.jpeg;*.bmp;*.webp'; \
$d.InitialDirectory = $env:PPT_CAPTURE_INITIAL_FOLDER; \
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { \
$b = [System.Text.Encoding]::UTF8.GetBytes($d.FileName); \
([System.BitConverter]::ToString($b)).Replace('-', '') }";

/// What the platform chooser printed, as a path. An empty answer means the user
/// cancelled the dialog.
fn chooser_path(stdout: &[u8]) -> Result<Option<PathBuf>, String> {
    let raw = std::str::from_utf8(stdout)
        .map_err(|_| "The chooser returned text that is not UTF-8".to_string())?;
    let text = decode_chooser_output(raw.trim())?;
    let text = text
        .trim_start_matches('\u{feff}')
        .trim_end_matches(['\r', '\n']);
    Ok(if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    })
}

#[cfg(target_os = "windows")]
fn decode_chooser_output(raw: &str) -> Result<String, String> {
    decode_hex_text(raw)
}

#[cfg(not(target_os = "windows"))]
fn decode_chooser_output(raw: &str) -> Result<String, String> {
    Ok(raw.to_string())
}

/// Decode the hex of a UTF-8 string. Platform-neutral so the decoding is covered
/// by the tests that run on every platform.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn decode_hex_text(raw: &str) -> Result<String, String> {
    if !raw.len().is_multiple_of(2) {
        return Err("The chooser returned an odd number of hex digits".into());
    }
    let mut bytes = Vec::with_capacity(raw.len() / 2);
    for pair in raw.as_bytes().chunks(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| "The chooser returned text that is not hex".to_string())?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| "The chooser returned text that is not hex".to_string())?;
        bytes.push((high * 16 + low) as u8);
    }
    String::from_utf8(bytes).map_err(|_| "The chosen path is not UTF-8".to_string())
}

fn initial_directory(initial: Option<&Path>) -> &Path {
    initial.unwrap_or_else(|| Path::new(""))
}

pub fn pick_folder(initial: Option<&Path>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = crate::platform::quiet_command("powershell.exe")
        .env("PPT_CAPTURE_INITIAL_FOLDER", initial_directory(initial))
        .args(["-NoProfile", "-STA", "-Command", WINDOWS_FOLDER_CHOOSER])
        .output();
    #[cfg(target_os = "macos")]
    let result = crate::platform::quiet_command("osascript")
        .args([
            "-e",
            "on run argv\nif (item 1 of argv) is not \"\" then\nreturn POSIX path of (choose folder with prompt \"Select an image folder\" default location (POSIX file (item 1 of argv)))\nelse\nreturn POSIX path of (choose folder with prompt \"Select an image folder\")\nend if\nend run",
            "--",
        ])
        .arg(initial_directory(initial))
        .output();
    #[cfg(target_os = "linux")]
    let result = {
        let mut command = crate::platform::quiet_command("zenity");
        command.args(["--file-selection", "--directory"]);
        let start = initial_directory(initial);
        if !start.as_os_str().is_empty() {
            command.arg(format!("--filename={}/", start.display()));
        }
        command.output()
    };
    let result = result.map_err(|error| {
        format!("Cannot open folder chooser ({error}); enter the folder path manually")
    })?;
    if !result.status.success() {
        return Ok(None);
    }
    chooser_path(&result.stdout)
}

pub fn pick_file(kind: ChooserKind, initial: Option<&Path>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    let result = {
        let script = match kind {
            ChooserKind::Presentation => WINDOWS_PRESENTATION_CHOOSER,
            ChooserKind::Image => WINDOWS_IMAGE_CHOOSER,
        };
        crate::platform::quiet_command("powershell.exe")
            .env("PPT_CAPTURE_INITIAL_FOLDER", initial_directory(initial))
            .args(["-NoProfile", "-STA", "-Command", script])
            .output()
    };
    #[cfg(target_os = "macos")]
    let result = {
        let prompt = kind.prompt();
        let types = kind.apple_types();
        let script = format!(
            "on run argv\nif (item 1 of argv) is not \"\" then\nreturn POSIX path of (choose file with prompt \"{prompt}\" default location (POSIX file (item 1 of argv)){types})\nelse\nreturn POSIX path of (choose file with prompt \"{prompt}\"{types})\nend if\nend run"
        );
        crate::platform::quiet_command("osascript")
            .args(["-e", script.as_str(), "--"])
            .arg(initial_directory(initial))
            .output()
    };
    #[cfg(target_os = "linux")]
    let result = {
        let mut command = crate::platform::quiet_command("zenity");
        command.args(["--file-selection", kind.zenity_filter()]);
        let start = initial_directory(initial);
        if !start.as_os_str().is_empty() {
            command.arg(format!("--filename={}/", start.display()));
        }
        command.output()
    };
    let result = result
        .map_err(|error| format!("Cannot open file chooser ({error}); enter the path manually"))?;
    if !result.status.success() {
        return Ok(None);
    }
    chooser_path(&result.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_chooser_scripts_are_single_line_commands() {
        for script in [
            WINDOWS_FOLDER_CHOOSER,
            WINDOWS_PRESENTATION_CHOOSER,
            WINDOWS_IMAGE_CHOOSER,
        ] {
            // The literal continues over several source lines; what PowerShell
            // receives has to be one command with every statement separated.
            assert!(!script.contains('\n'), "{script}");
            assert!(!script.contains('\\'), "{script}");
            assert!(
                script.contains("[System.Windows.Forms.DialogResult]::OK"),
                "{script}"
            );
            assert!(script.contains("Replace('-', '')"), "{script}");
            assert!(
                script.contains("$env:PPT_CAPTURE_INITIAL_FOLDER"),
                "{script}"
            );
        }
    }

    #[test]
    fn the_windows_image_chooser_filters_pictures_the_background_can_use() {
        assert!(WINDOWS_IMAGE_CHOOSER.contains("*.png;*.jpg;*.jpeg;*.bmp;*.webp"));
        assert!(!WINDOWS_IMAGE_CHOOSER.contains("*.pptx"));
        assert!(WINDOWS_PRESENTATION_CHOOSER.contains("*.pptx"));
    }

    #[test]
    fn a_chinese_path_survives_the_hex_the_windows_chooser_prints() {
        // What [BitConverter]::ToString produces for the UTF-8 bytes of
        // "C:\数据\幻灯片.pptx", minus the dashes.
        let hex = "433A5CE695B0E68DAE5CE5B9BBE781AFE789872E70707478";
        assert_eq!(decode_hex_text(hex).unwrap(), r"C:\数据\幻灯片.pptx");
    }

    #[test]
    fn malformed_hex_is_rejected_instead_of_becoming_a_path() {
        assert!(decode_hex_text("ABC").is_err(), "odd number of digits");
        assert!(decode_hex_text("ZZ").is_err(), "not hex");
        assert!(decode_hex_text("FFFE").is_err(), "not UTF-8");
        assert_eq!(decode_hex_text("").unwrap(), "");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_plain_path_from_the_native_chooser_is_taken_as_is() {
        assert_eq!(
            chooser_path(b"/Users/allen/\xe8\xb5\x84\xe6\x96\x99/\n").unwrap(),
            Some(PathBuf::from("/Users/allen/资料"))
        );
    }

    #[test]
    fn a_cancelled_chooser_returns_no_path() {
        assert_eq!(chooser_path(b"").unwrap(), None);
        assert_eq!(chooser_path(b"\r\n").unwrap(), None);
    }
}
