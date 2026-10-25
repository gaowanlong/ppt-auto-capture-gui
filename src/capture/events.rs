//! Nonblocking, bounded UI snapshots. Recording data remains in the session journal.
use super::WorkerEvent;
use crossbeam_channel::{SendError, TryRecvError};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct Shared {
    queue: Mutex<VecDeque<WorkerEvent>>,
    senders: AtomicUsize,
    receivers: AtomicUsize,
}

pub struct WorkerEventSender(Arc<Shared>);
pub struct WorkerEventReceiver(Arc<Shared>);

pub fn worker_event_channel() -> (WorkerEventSender, WorkerEventReceiver) {
    let shared = Arc::new(Shared {
        queue: Mutex::new(VecDeque::new()),
        senders: AtomicUsize::new(1),
        receivers: AtomicUsize::new(1),
    });
    (
        WorkerEventSender(shared.clone()),
        WorkerEventReceiver(shared),
    )
}

impl WorkerEventSender {
    pub fn send(&self, event: WorkerEvent) -> Result<(), SendError<WorkerEvent>> {
        if self.0.receivers.load(Ordering::Acquire) == 0 {
            return Err(SendError(event));
        }
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(event, WorkerEvent::Error(_)) {
            if queue
                .iter()
                .filter(|e| matches!(e, WorkerEvent::Error(_)))
                .count()
                >= 20
            {
                if let Some(index) = queue
                    .iter()
                    .position(|e| matches!(e, WorkerEvent::Error(_)))
                {
                    queue.remove(index);
                }
            }
        } else {
            let kind = std::mem::discriminant(&event);
            queue.retain(|old| std::mem::discriminant(old) != kind);
        }
        // One snapshot per non-error kind plus twenty recent errors fits well below 64.
        if queue.len() >= 64 {
            queue.pop_front();
        }
        queue.push_back(event);
        Ok(())
    }

    pub fn try_send(&self, event: WorkerEvent) -> Result<(), SendError<WorkerEvent>> {
        self.send(event)
    }
    pub fn len(&self) -> usize {
        self.0.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl WorkerEventReceiver {
    pub fn is_disconnected(&self) -> bool {
        self.0.senders.load(Ordering::Acquire) == 0 && self.len() == 0
    }
    pub fn try_recv(&self) -> Result<WorkerEvent, TryRecvError> {
        let mut queue = self.0.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.pop_front().ok_or_else(|| {
            if self.0.senders.load(Ordering::Acquire) == 0 {
                TryRecvError::Disconnected
            } else {
                TryRecvError::Empty
            }
        })
    }
    pub fn try_iter(&self) -> impl Iterator<Item = WorkerEvent> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }
    pub fn len(&self) -> usize {
        self.0.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl Clone for WorkerEventSender {
    fn clone(&self) -> Self {
        self.0.senders.fetch_add(1, Ordering::Relaxed);
        Self(self.0.clone())
    }
}
impl Drop for WorkerEventSender {
    fn drop(&mut self) {
        self.0.senders.fetch_sub(1, Ordering::Release);
    }
}
impl Clone for WorkerEventReceiver {
    fn clone(&self) -> Self {
        self.0.receivers.fetch_add(1, Ordering::Relaxed);
        Self(self.0.clone())
    }
}
impl Drop for WorkerEventReceiver {
    fn drop(&mut self) {
        self.0.receivers.fetch_sub(1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_errors_are_drained_before_last_sender_disconnects() {
        let (tx, rx) = worker_event_channel();
        let other = tx.clone();
        drop(tx);
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
        other
            .send(WorkerEvent::Error("last failure".into()))
            .unwrap();
        drop(other);
        assert!(
            matches!(rx.try_recv(), Ok(WorkerEvent::Error(message)) if message == "last failure")
        );
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Disconnected)));
    }
    #[test]
    fn disconnected_receivers_do_not_keep_a_worker_alive() {
        let (tx, rx) = worker_event_channel();
        let other = rx.clone();
        drop(rx);
        tx.send(WorkerEvent::BlackFrameDetected).unwrap();
        assert!(other.try_recv().is_ok());
        drop(other);
        assert!(tx.send(WorkerEvent::BlackFrameDetected).is_err());
    }
}
