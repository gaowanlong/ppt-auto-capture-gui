//! Platform-independent checks used by the native Windows stress harness.
use anyhow::Result;
use std::time::Duration;

#[derive(Default)]
pub(crate) struct FreshnessWatchdog {
    previous: Option<[u8; 32]>,
    last_change: Duration,
}

impl FreshnessWatchdog {
    pub(crate) fn observe(&mut self, fingerprint: [u8; 32], elapsed: Duration) -> Result<bool> {
        let changed = self
            .previous
            .is_some_and(|previous| previous != fingerprint);
        if changed {
            self.last_change = elapsed;
        }
        self.previous = Some(fingerprint);
        anyhow::ensure!(
            elapsed.saturating_sub(self.last_change) <= Duration::from_secs(10),
            "Native capture froze for more than ten seconds despite changing fixture content"
        );
        Ok(changed)
    }
}

#[test]
fn freshness_watchdog_rejects_a_capture_that_changes_once_then_freezes() {
    let mut watchdog = FreshnessWatchdog::default();
    assert!(!watchdog.observe([0; 32], Duration::ZERO).unwrap());
    assert!(watchdog.observe([1; 32], Duration::from_secs(1)).unwrap());
    assert!(!watchdog.observe([1; 32], Duration::from_secs(5)).unwrap());
    assert!(watchdog.observe([1; 32], Duration::from_secs(12)).is_err());
}

#[test]
fn freshness_watchdog_accepts_continuously_changing_capture() {
    let mut watchdog = FreshnessWatchdog::default();
    for seconds in 0..86400 {
        watchdog
            .observe([seconds as u8; 32], Duration::from_secs(seconds))
            .unwrap();
    }
}

#[test]
fn freshness_watchdog_rejects_a_capture_that_never_changes() {
    let mut watchdog = FreshnessWatchdog::default();
    assert!(!watchdog.observe([0; 32], Duration::ZERO).unwrap());
    assert!(watchdog.observe([0; 32], Duration::from_secs(11)).is_err());
}
