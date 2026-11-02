//! Small platform helpers shared by the GUI layers.

use std::ffi::OsStr;
use std::process::Command;

/// A [`Command`] for a console helper (PowerShell, `zenity`, `osascript`).
///
/// The application is built as a Windows GUI subsystem binary, so it owns no
/// console. Starting a console program from it would make Windows allocate a
/// fresh console — the black window that flashes for a moment before the file
/// dialog appears. `CREATE_NO_WINDOW` asks for a child process without one; the
/// dialog the helper opens is still shown, only its console is suppressed.
pub fn quiet_command(program: impl AsRef<OsStr>) -> Command {
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW` from the process creation flags.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quiet_command_keeps_the_program_it_was_given() {
        let command = quiet_command("powershell.exe");
        assert_eq!(command.get_program(), OsStr::new("powershell.exe"));
    }

    #[test]
    fn a_quiet_command_still_captures_output() {
        #[cfg(target_os = "windows")]
        let output = quiet_command("cmd.exe")
            .args(["/C", "echo quiet"])
            .output()
            .unwrap();
        #[cfg(not(target_os = "windows"))]
        let output = quiet_command("echo").arg("quiet").output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("quiet"));
    }
}
