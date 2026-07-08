use log::info;
use std::thread;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

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
                let cur = check_session();
                if cur != last { let _ = tx.send(cur); last = cur; }
                thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> { self.rx.clone() }
}

fn check_session() -> SessionState {
    unsafe {
        let hdesk = OpenInputDesktop(0, false, DESKTOP_READOBJECTS);
        match hdesk {
            Ok(desk) => { let _ = CloseHandle(desk); SessionState::Unlocked }
            Err(_) => SessionState::Locked
        }
    }
}
