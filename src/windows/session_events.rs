use log::info;
use std::thread;
use windows::Win32::Foundation::{BOOL, CloseHandle};
use windows::Win32::System::StationsAndDesktops::{OpenInputDesktop, DESKTOP_READOBJECTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState { Unlocked, Locked, Disconnected, Reconnected }

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
            let mut last = SessionState::Unlocked;
            loop {
                let cur = if unsafe { OpenInputDesktop(0, false, DESKTOP_READOBJECTS).is_ok() } {
                    SessionState::Unlocked
                } else {
                    SessionState::Locked
                };
                if cur != last { let _ = tx.send(cur); last = cur; }
                thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> { self.rx.clone() }
}
