//! Monitor session events (lock/unlock, workstation state changes).
//! Used to pause capture when the workstation is locked.

use anyhow::Result;
use log::{info, warn};
use std::sync::mpsc;
use std::thread;

/// Session state observed by the monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Workstation is unlocked and active.
    Unlocked,
    /// Workstation is locked.
    Locked,
    /// Session disconnected (RDP).
    Disconnected,
    /// Session reconnected (RDP).
    Reconnected,
}

/// Simple session event monitor using a polling approach
/// (since listening for WTS session messages requires a Windows message pump).
pub struct SessionEventMonitor {
    tx: crossbeam_channel::Sender<SessionState>,
    rx: crossbeam_channel::Receiver<SessionState>,
    running: bool,
}

impl SessionEventMonitor {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            running: false,
        }
    }

    /// Start monitoring session events in a background thread.
    pub fn start(&mut self) {
        if self.running {
            return;
        }
        self.running = true;
        let tx = self.tx.clone();

        thread::spawn(move || {
            info!("Session event monitor started");
            let mut last_state = SessionState::Unlocked;

            loop {
                let current_state = check_session_state();
                if current_state != last_state {
                    info!("Session state changed: {:?} -> {:?}", last_state, current_state);
                    let _ = tx.send(current_state);
                    last_state = current_state;
                }
                thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }

    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> {
        self.rx.clone()
    }
}

/// Check if the workstation is locked by testing OpenInputDesktop.
fn check_session_state() -> SessionState {
    unsafe {
        // On Windows, when the workstation is locked, OpenInputDesktop fails
        // with ERROR_ACCESS_DENIED.
        let hdesk = windows::Win32::UI::WindowsAndMessaging::OpenInputDesktop(
            0,
            false,
            windows::Win32::UI::WindowsAndMessaging::DESKTOP_READOBJECTS,
        );

        match hdesk {
            Ok(desk) => {
                let _ = windows::Win32::Foundation::CloseHandle(desk);
                SessionState::Unlocked
            }
            Err(e) => {
                if e.code() == windows::Win32::Foundation::ERROR_ACCESS_DENIED.into() {
                    SessionState::Locked
                } else {
                    SessionState::Unlocked
                }
            }
        }
    }
}
