//! Safe deletion of one recording session directory.
//!
//! A session folder lives directly under `<output>/sessions/`. Deleting it can
//! fail on Windows while another program (a preview handler, an office suite,
//! an antivirus scanner, or a sync client) still holds one of its files open.
//! Deletion is retried briefly, and a failure reports which processes the
//! system identifies as using the folder so the user can close them.

use anyhow::{ensure, Context, Result};
use std::path::Path;
use std::time::Duration;

#[cfg(target_os = "windows")]
use anyhow::bail;

const RETRY_DELAYS_MS: [u64; 6] = [0, 150, 350, 750, 1200, 2000];

/// Delete a single session folder after proving it is a direct child of the
/// recordings `sessions` directory and contains a session journal.
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
    ensure!(
        directory.join("session.json").is_file(),
        "Refusing to delete a folder without a session journal: {}",
        directory.display()
    );

    #[cfg(target_os = "windows")]
    clear_read_only(&directory);

    let mut last_error = None;
    for delay in RETRY_DELAYS_MS {
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }

    #[cfg(target_os = "windows")]
    {
        let blockers = locking_processes(&directory);
        if !blockers.is_empty() {
            bail!(
                "Cannot delete {} because it is in use by {}. Close it and try again.",
                directory.display(),
                blockers.join(", ")
            );
        }
    }

    let error = last_error.expect("at least one removal attempt");
    Err(error).with_context(|| {
        format!(
            "Cannot delete {}; close any program using its files and try again",
            directory.display()
        )
    })
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
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
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
                pending.push(entry.path());
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

    #[cfg(target_os = "windows")]
    #[test]
    fn reports_the_program_holding_a_session_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempfile::tempdir().unwrap();
        let folder = session_folder(root.path(), "20260101-000000-test");
        let sessions = root.path().join("sessions");
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(folder.join("slides/slide_0001.png"))
            .unwrap();

        let error = delete_session(&folder, &sessions).unwrap_err();

        assert!(
            error.to_string().contains("in use by"),
            "unexpected error: {error:#}"
        );
        assert!(folder.exists());
        drop(locked);
        delete_session(&folder, &sessions).unwrap();
        assert!(!folder.exists());
    }
}
