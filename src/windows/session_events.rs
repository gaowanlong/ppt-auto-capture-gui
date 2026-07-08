use log::info;
use std::thread;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState { Unlocked, Locked }

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
                // Simple check: can we get the desktop window?
                let unlocked = unsafe { GetDesktopWindow() != HWND(0) };
                let cur = if unlocked { SessionState::Unlocked } else { SessionState::Locked };
                if cur != last { let _ = tx.send(cur); last = cur; }
                thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> { self.rx.clone() }
}
