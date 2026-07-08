use std::thread;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState { Unlocked, Locked }
pub struct SessionEventMonitor {
    tx: crossbeam_channel::Sender<SessionState>,
    rx: crossbeam_channel::Receiver<SessionState>,
    running: bool,
}
impl SessionEventMonitor {
    pub fn new() -> Self { let (tx, rx) = crossbeam_channel::unbounded(); Self { tx, rx, running: false } }
    pub fn start(&mut self) {
        if self.running { return; }
        self.running = true; let tx = self.tx.clone();
        thread::spawn(move || loop { let _ = tx.send(SessionState::Unlocked); thread::sleep(std::time::Duration::from_secs(30)); });
    }
    pub fn get_receiver(&self) -> crossbeam_channel::Receiver<SessionState> { self.rx.clone() }
}
