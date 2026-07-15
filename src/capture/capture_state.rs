/// State machine for the capture process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CaptureState {
    /// Initial idle state.
    Idle,
    /// Capture is running and looking for changes.
    Running,
    /// A change was detected; waiting for animation to stabilize.
    WaitingForStable,
    /// Frame is stable; about to save.
    Stable,
    /// Saving the frame to disk and PPTX.
    Saving,
    /// Capture is paused (user or auto-pause).
    Paused,
    /// Stopped by user.
    Stopped,
    /// Underlying content is protected or black; cannot capture.
    ProtectedOrBlack,
    /// Error state.
    Error,
}

impl CaptureState {
    pub fn label(&self) -> &'static str {
        match self {
            CaptureState::Idle => "Idle (Ready)",
            CaptureState::Running => "Running — Watching for changes…",
            CaptureState::WaitingForStable => "Waiting for animation to stabilize…",
            CaptureState::Stable => "Stable — Ready to capture",
            CaptureState::Saving => "Saving screenshot…",
            CaptureState::Paused => "Paused",
            CaptureState::Stopped => "Stopped",
            CaptureState::ProtectedOrBlack => "Protected/Black Content — Cannot capture",
            CaptureState::Error => "Error",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, CaptureState::Running | CaptureState::WaitingForStable | CaptureState::Stable | CaptureState::Saving)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, CaptureState::Paused | CaptureState::ProtectedOrBlack)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_active() {
        assert!(!CaptureState::Idle.is_active());
        assert!(CaptureState::Running.is_active());
        assert!(CaptureState::WaitingForStable.is_active());
        assert!(CaptureState::Stable.is_active());
        assert!(CaptureState::Saving.is_active());
        assert!(!CaptureState::Paused.is_active());
        assert!(!CaptureState::Stopped.is_active());
        assert!(!CaptureState::ProtectedOrBlack.is_active());
        assert!(!CaptureState::Error.is_active());
    }

    #[test]
    fn test_is_paused() {
        assert!(!CaptureState::Idle.is_paused());
        assert!(CaptureState::Paused.is_paused());
        assert!(CaptureState::ProtectedOrBlack.is_paused());
        assert!(!CaptureState::Running.is_paused());
    }

    #[test]
    fn test_label_not_empty() {
        for state in &[
            CaptureState::Idle,
            CaptureState::Running,
            CaptureState::WaitingForStable,
            CaptureState::Stable,
            CaptureState::Saving,
            CaptureState::Paused,
            CaptureState::Stopped,
            CaptureState::ProtectedOrBlack,
            CaptureState::Error,
        ] {
            assert!(!state.label().is_empty(), "Label should not be empty for {:?}", state);
        }
    }
}
