//! Safe deletion of one recording session directory.
//!
//! A session folder lives directly under `<output>/sessions/`. Deleting it can
//! fail on Windows while another program (a preview handler, an office suite,
//! an antivirus scanner, or a sync client) still holds one of its files open.
//! Deletion is retried for about 18 seconds, then reports both the process that
//! holds a file and any file or folder that still refuses deletion. The extra
//! detail matters because an Explorer window holds the folder itself, which
//! the Restart Manager does not report as a file user.

use anyhow::{ensure, Context, Result};
use std::path::Path;
use std::time::Duration;

#[cfg(target_os = "windows")]
use anyhow::bail;

// Freshly written screenshots and decks are often scanned by antivirus,
// thumbnail providers, or sync clients for several seconds. Keep retrying long
// enough for those transient handles to close; the UI runs this on a worker.
#[cfg(not(test))]
const RETRY_DELAYS_MS: [u64; 12] = [
    0, 100, 200, 300, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000,
];
#[cfg(test)]
const RETRY_DELAYS_MS: [u64; 3] = [0, 10, 25];

/// List recording session folders under `<output>/sessions`, newest first.
/// Only direct children that contain a session journal are returned.
pub fn list_session_folders(output_root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let sessions = output_root.join("sessions");
    if !sessions.is_dir() {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(&sessions)
        .with_context(|| format!("Cannot inspect sessions folder {}", sessions.display()))?
    {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() && path.join("session.json").is_file() {
            folders.push(path);
        }
    }
    folders.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    Ok(folders)
}

/// Delete a single session folder after proving it is a direct child of the
/// recordings `sessions` directory and is recognizable as a session.
pub fn delete_session(session_dir: &Path, sessions_root: &Path) -> Result<()> {
    if !session_dir.exists() {
        return Ok(());
    }
    let root = sessions_root
        .canonicalize()
        .with_context(|| format!("Cannot resolve sessions folder {:?}", sessions_root))?;
    let directory = session_dir
        .canonicalize()
        .with_context(|| format!("Cannot resolve recording folder {:?}", session_dir))?;
    ensure!(
        root.file_name() == Some(std::ffi::OsStr::new("sessions")),
        "Refusing to delete from a folder that is not a sessions directory: {}",
        root.display()
    );
    ensure!(
        directory != root,
        "Refusing to delete the sessions directory itself"
    );
    ensure!(
        directory.parent() == Some(root.as_path()),
        "Refusing to delete a folder outside the sessions directory: {}",
        directory.display()
    );
    // A failed removal can delete session.json before a locked file stops it.
    // Accept the generated `<timestamp>-<uuid>` name as proof for retries.
    let has_journal = directory.join("session.json").is_file();
    let generated_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(looks_like_generated_session_name);
    ensure!(
        has_journal || generated_name,
        "Refusing to delete a folder without a session journal: {}",
        directory.display()
    );

    #[cfg(target_os = "windows")]
    clear_read_only(&directory);

    let removal_error = match remove_with_retries(&directory) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };

    #[cfg(target_os = "windows")]
    {
        // Identify what still holds the folder, then try once more: the probe
        // itself takes a moment, and transient scanners often let go in that
        // window.
        let probe = probe_delete_locks(&directory);
        let blockers = locking_processes(&directory);
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => {}
        }
        if probe.folder_locked || !probe.locked_files.is_empty() || !blockers.is_empty() {
            bail!(
                "Cannot delete {}: {}. Close the program using it and try again.",
                directory.display(),
                describe_lock(&directory, &probe, &blockers)
            );
        }
    }

    Err(anyhow::Error::new(removal_error)).with_context(|| {
        format!(
            "Cannot delete {}; close any program using its files and try again",
            directory.display()
        )
    })
}

/// Remove the folder, retrying for ~18 seconds while scanners or preview
/// handlers briefly hold files. A folder that disappears is a success.
fn remove_with_retries(directory: &Path) -> std::io::Result<()> {
    let mut last_error = None;
    for delay in RETRY_DELAYS_MS {
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        match std::fs::remove_dir_all(directory) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.expect("at least one removal attempt"))
}

#[cfg(target_os = "windows")]
struct LockProbe {
    /// The folder itself cannot be opened for deletion (Explorer and some
    /// bookmark/sync tools hold the directory, which Restart Manager misses).
    folder_locked: bool,
    /// Individual files that still refuse delete access.
    locked_files: Vec<std::path::PathBuf>,
}

/// Open each file and the folder itself with delete access and every sharing
/// flag. A sharing/lock violation means another handle forbids deletion.
#[cfg(target_os = "windows")]
fn probe_delete_locks(directory: &Path) -> LockProbe {
    const MAX_REPORTED_FILES: usize = 6;
    let folder_locked = delete_is_blocked(directory);
    let mut locked_files = Vec::new();
    for file in collect_files(directory, 512) {
        if delete_is_blocked(&file) {
            locked_files.push(file);
            if locked_files.len() >= MAX_REPORTED_FILES {
                break;
            }
        }
    }
    LockProbe {
        folder_locked,
        locked_files,
    }
}

#[cfg(target_os = "windows")]
fn delete_is_blocked(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    match unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            DELETE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    } {
        Ok(handle) => {
            let _ = unsafe { CloseHandle(handle) };
            false
        }
        Err(error) => {
            let code = error.code().0 as u32 & 0xFFFF;
            code == 32 || code == 33
        }
    }
}

#[cfg(target_os = "windows")]
fn describe_lock(directory: &Path, probe: &LockProbe, blockers: &[String]) -> String {
    let mut parts = Vec::new();
    if probe.folder_locked {
        parts.push(
            "the folder itself is in use by another program, such as an open Explorer window"
                .to_string(),
        );
    }
    for file in &probe.locked_files {
        let shown = file.strip_prefix(directory).unwrap_or(file);
        parts.push(format!("the file {} is in use", shown.display()));
    }
    if !blockers.is_empty() {
        parts.push(format!("in use by {}", blockers.join(", ")));
    }
    parts.join("; ")
}

fn looks_like_generated_session_name(name: &str) -> bool {
    let mut parts = name.splitn(3, '-');
    let (Some(date), Some(time), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    date.len() == 8
        && date.bytes().all(|byte| byte.is_ascii_digit())
        && time.len() == 6
        && time.bytes().all(|byte| byte.is_ascii_digit())
        && uuid::Uuid::parse_str(id).is_ok()
}

#[cfg(target_os = "windows")]
fn clear_read_only(path: &Path) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            clear_read_only(&entry.path());
        } else if let Ok(metadata) = entry.metadata() {
            if metadata.permissions().readonly() {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(entry.path(), permissions);
            }
        }
    }
}

/// Ask the Windows Restart Manager which processes currently use files in the
/// folder. Returns an empty list when the information is unavailable.
#[cfg(target_os = "windows")]
fn locking_processes(directory: &Path) -> Vec<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };

    let files = collect_files(directory, 128);
    if files.is_empty() {
        return Vec::new();
    }
    let wide_paths: Vec<Vec<u16>> = files
        .iter()
        .map(|path| {
            path.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        })
        .collect();
    let pointers: Vec<PCWSTR> = wide_paths
        .iter()
        .map(|path| PCWSTR(path.as_ptr()))
        .collect();

    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    if unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) } != ERROR_SUCCESS {
        return Vec::new();
    }

    let mut names = Vec::new();
    let status = unsafe { RmRegisterResources(session, Some(&pointers), None, None) };
    if status == ERROR_SUCCESS {
        let mut needed = 0u32;
        let mut count = 0u32;
        let mut reasons = 0u32;
        let status = unsafe { RmGetList(session, &mut needed, &mut count, None, &mut reasons) };
        if status == ERROR_MORE_DATA || (status == ERROR_SUCCESS && needed > 0) {
            let mut infos = vec![RM_PROCESS_INFO::default(); needed as usize];
            count = needed;
            let status = unsafe {
                RmGetList(
                    session,
                    &mut needed,
                    &mut count,
                    Some(infos.as_mut_ptr()),
                    &mut reasons,
                )
            };
            if status == ERROR_SUCCESS {
                infos.truncate(count as usize);
                for info in &infos {
                    let raw: Vec<u16> = info
                        .strAppName
                        .iter()
                        .copied()
                        .take_while(|code| *code != 0)
                        .collect();
                    let name = String::from_utf16_lossy(&raw);
                    let pid = info.Process.dwProcessId;
                    names.push(if name.is_empty() {
                        format!("PID {pid}")
                    } else {
                        format!("{name} (PID {pid})")
                    });
                }
                names.sort();
                names.dedup();
            }
        }
    }
    let _ = unsafe { RmEndSession(session) };
    names
}

#[cfg(target_os = "windows")]
fn collect_files(directory: &Path, limit: usize) -> Vec<std::path::PathBuf> {
    // Breadth first: the journal and top-level outputs are the most likely to
    // be held (and the most useful to name), so gather them before slide files.
    let mut files = Vec::new();
    let mut pending = std::collections::VecDeque::new();
    pending.push_back(directory.to_path_buf());
    while let Some(current) = pending.pop_front() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            if files.len() >= limit {
                return files;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push_back(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn session_folder(root: &Path, name: &str) -> PathBuf {
        let sessions = root.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let folder = sessions.join(name);
        std::fs::create_dir_all(folder.join("slides")).unwrap();
        std::fs::write(folder.join("session.json"), br#"{"completed":false}"#).unwrap();
        std::fs::write(folder.join("slides/slide_0001.png"), b"image").unwrap();
        folder
    }

    #[test]
    fn deletes_a_session_folder_and_its_contents() {
        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(root.path(), "20260101-000000-test");
        let sessions = root.path().join("sessions");

        delete_session(&folder, &sessions).unwrap();

        assert!(!folder.exists());
        assert!(sessions.is_dir());
    }

    #[test]
    fn refuses_a_folder_outside_the_sessions_directory() {
        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(root.path(), "20260101-000000-test");
        let other = root.path().join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("session.json"), b"{}").unwrap();

        let error = delete_session(&other, &root.path().join("sessions")).unwrap_err();

        assert!(error.to_string().contains("outside the sessions directory"));
        assert!(other.exists());
        assert!(folder.exists());
    }

    #[test]
    fn refuses_the_sessions_directory_itself() {
        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(root.path(), "20260101-000000-test");
        let sessions = root.path().join("sessions");
        std::fs::write(sessions.join("session.json"), b"{}").unwrap();

        let error = delete_session(&sessions, &sessions).unwrap_err();

        assert!(error.to_string().contains("sessions directory itself"));
        assert!(folder.exists());
    }

    #[test]
    fn refuses_a_folder_without_a_session_journal() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        let folder = sessions.join("20260101-000000-test");
        std::fs::create_dir_all(&folder).unwrap();

        let error = delete_session(&folder, &sessions).unwrap_err();

        assert!(error.to_string().contains("without a session journal"));
        assert!(folder.exists());
    }

    #[test]
    fn treats_an_already_missing_folder_as_deleted() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();

        delete_session(&sessions.join("already-gone"), &sessions).unwrap();
    }

    #[test]
    fn refuses_a_root_that_is_not_a_sessions_folder() {
        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(root.path(), "20260101-000000-test");

        let error = delete_session(&folder, root.path()).unwrap_err();

        assert!(error.to_string().contains("not a sessions directory"));
        assert!(folder.exists());
    }

    #[test]
    fn lists_only_session_folders_newest_first() {
        let root = tempfile::tempdir().unwrap();
        session_folder(root.path(), "20260101-000001-older");
        session_folder(root.path(), "20260102-000000-newer");
        std::fs::create_dir_all(root.path().join("sessions/not-a-session")).unwrap();
        std::fs::write(root.path().join("sessions/stray.txt"), b"x").unwrap();

        let folders = list_session_folders(root.path()).unwrap();

        let names: Vec<_> = folders
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["20260102-000000-newer", "20260101-000001-older"]);
    }

    #[test]
    fn lists_no_sessions_when_the_output_has_none() {
        let root = tempfile::tempdir().unwrap();

        assert!(list_session_folders(root.path()).unwrap().is_empty());
    }

    #[test]
    fn retries_a_partially_deleted_session_without_its_journal() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        let folder = sessions.join(format!("20260101-000000-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(folder.join("slides")).unwrap();
        std::fs::write(folder.join("slides/slide_0001.png"), b"image").unwrap();

        delete_session(&folder, &sessions).unwrap();

        assert!(!folder.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reports_the_program_holding_a_session_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(
            root.path(),
            &format!("20260101-000000-{}", uuid::Uuid::new_v4()),
        );
        let sessions = root.path().join("sessions");
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(folder.join("slides/slide_0001.png"))
            .unwrap();

        let error = delete_session(&folder, &sessions).unwrap_err();

        let message = format!("{error:#}");
        assert!(message.contains("in use"), "unexpected error: {message}");
        assert!(
            message.contains("slide_0001.png"),
            "locked file not named: {message}"
        );
        assert!(folder.exists());
        drop(locked);
        delete_session(&folder, &sessions).unwrap();
        assert!(!folder.exists());
    }
}
