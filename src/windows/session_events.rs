use anyhow::Result;
use log::{info, warn};
use std::sync::mpsc;
use std::thread;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Unlocked,
    Locked,
    Disconnected,
    Reconnected,
}

pub struct SessionEventMonitor {
    tx: crossbeam_channel::Sender<SessionState>,
    rx: crossbeam_channel::Receiver<SessionState>,
    running: bool,
}

impl SessionEventMonitor {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx, running: false }
    }

    pub fn start(&mut self) {
        if self.running { return; }
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

fn check_session_state() -> SessionState {
    unsafe {
        let hdesk = OpenInputDesktop(
            0,
            false,
            DESKTOP_READOBJECTS,
        );

        match hdesk {
            Ok(desk) => {
                let _ = CloseHandle(desk);
                SessionState::Unlocked
            }
            Err(_) => {
                SessionState::Locked
            }
        }
    }
}
