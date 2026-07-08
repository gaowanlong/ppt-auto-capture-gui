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
