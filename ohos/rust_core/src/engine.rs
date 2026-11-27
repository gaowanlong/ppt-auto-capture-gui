//! Platform-neutral capture state machine, shared in spirit with the desktop
//! `capture_worker` but free of GUI channels and capture hardware.
//!
//! ArkTS captures the screen with `screenshot.capture()` and hands each frame
//! to [`CaptureEngine::push_frame`] as BGRA bytes. Every decision that affects
//! which pages end up in the PPTX lives here, so the HarmonyOS build gets the
//! same duplicate filtering, transition skipping and deferred publishing as
//! the desktop build.

use crate::detection::{
    BlackFrameDetector, ChangeDetector, DuplicateDetector, MotionFilter, StabilityDetector,
};
use crate::model::{Frame, SlideRecord, SlideRecordInput};
use crate::storage::{PublishReport, SessionStore};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Mirrors `CaptureState` in the desktop worker, minus the GUI-only states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    Running,
    WaitingForStable,
    Stable,
    ProtectedOrBlack,
    Paused,
    Completed,
}

impl EngineState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingForStable => "waiting-for-stable",
            Self::Stable => "stable",
            Self::ProtectedOrBlack => "protected-or-black",
            Self::Paused => "paused",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EngineConfig {
    pub stability_frames: u32,
    pub animation_timeout_ms: u64,
    pub change_threshold: f64,
    pub black_threshold: f64,
    pub filter_duplicates: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            stability_frames: 2,
            animation_timeout_ms: 10_000,
            change_threshold: 0.05,
            black_threshold: 0.80,
            filter_duplicates: true,
        }
    }
}

/// What happened to one submitted frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameOutcome {
    pub state: &'static str,
    pub saved: bool,
    pub black: bool,
    pub transition: bool,
}

/// Drives one recording session.
pub struct CaptureEngine {
    session: SessionStore,
    config: EngineConfig,
    state: EngineState,
    change_detector: ChangeDetector,
    black_detector: BlackFrameDetector,
    stability_detector: StabilityDetector,
    motion_filter: MotionFilter,
    duplicate_detector: DuplicateDetector,
    last_slide_hash: Option<String>,
    animation_started: Option<Instant>,
    first_frame_saved: bool,
    frame_index: u64,
    pub saved_pages: usize,
    pub skipped_pages: usize,
}

impl CaptureEngine {
    /// `root` is a directory the app can actually write to: on HarmonyOS this
    /// is a subdirectory of `context.filesDir`, never a public path.
    pub fn new(
        root: &Path,
        filename: &str,
        ratio: &str,
        fit: &str,
        config: EngineConfig,
    ) -> Result<Self> {
        let session = SessionStore::create(root, filename, ratio, fit)?;
        let change_detector = ChangeDetector::new(config.change_threshold);
        let black_detector = BlackFrameDetector::new(config.black_threshold);
        let stability_detector = StabilityDetector::new(config.stability_frames);
        Ok(Self {
            session,
            config,
            state: EngineState::Running,
            change_detector,
            black_detector,
            stability_detector,
            motion_filter: MotionFilter::default(),
            duplicate_detector: DuplicateDetector::new(),
            last_slide_hash: None,
            animation_started: None,
            first_frame_saved: false,
            frame_index: 0,
            saved_pages: 0,
            skipped_pages: 0,
        })
    }

    /// Rebuild an unfinished session after an app restart.
    pub fn open(directory: &Path, config: EngineConfig) -> Result<Self> {
        let session = SessionStore::open(directory)?;
        let saved_pages = session.records.len();
        let last_hash = session
            .records
            .last()
            .map(|record| record.content_hash.clone());
        let last_frame = session
            .records
            .last()
            .map_or(0, |record| record.frame_index);
        let change_detector = ChangeDetector::new(config.change_threshold);
        let black_detector = BlackFrameDetector::new(config.black_threshold);
        let stability_detector = StabilityDetector::new(config.stability_frames);
        Ok(Self {
            session,
            config,
            state: EngineState::Running,
            change_detector,
            black_detector,
            stability_detector,
            motion_filter: MotionFilter::default(),
            duplicate_detector: DuplicateDetector::new(),
            last_slide_hash: last_hash,
            animation_started: None,
            first_frame_saved: saved_pages > 0,
            frame_index: last_frame,
            saved_pages,
            skipped_pages: 0,
        })
    }

    pub fn state(&self) -> EngineState {
        self.state
    }

    pub fn output_path(&self) -> PathBuf {
        self.session.output_path()
    }

    pub fn session_directory(&self) -> &Path {
        &self.session.directory
    }

    /// Submit one BGRA screenshot. `timestamp_ms` is the capture time.
    pub fn push_frame(
        &mut self,
        data: Vec<u8>,
        width: u32,
        height: u32,
        timestamp_ms: u64,
    ) -> Result<FrameOutcome> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .context("frame dimensions overflow")?;
        if width == 0 || height == 0 || data.len() < expected {
            bail!(
                "frame has {} bytes, expected at least {} for {}x{} BGRA",
                data.len(),
                expected,
                width,
                height
            );
        }

        self.frame_index += 1;
        let frame = Frame::new(
            data,
            width,
            height,
            width * 4,
            self.frame_index,
            timestamp_ms,
        );

        if self.black_detector.is_black(&frame) {
            if matches!(
                self.state,
                EngineState::Running | EngineState::WaitingForStable
            ) {
                self.state = EngineState::ProtectedOrBlack;
            }
            return Ok(FrameOutcome {
                state: self.state.as_str(),
                black: true,
                ..FrameOutcome::default()
            });
        }

        if self.state == EngineState::ProtectedOrBlack {
            // The first visible frame after a fade is usually half-drawn.
            // Let the settle path keep the final picture instead.
            self.state = EngineState::WaitingForStable;
            self.animation_started = Some(Instant::now());
            self.stability_detector.reset();
            self.change_detector.update_reference(&frame);
            self.first_frame_saved = true;
        }

        let broad_transition = self.motion_filter.observe(&frame);
        let mut saved_this_frame = false;

        if !self.first_frame_saved {
            self.change_detector.detect_change(&frame);
            self.change_detector.update_reference(&frame);
            saved_this_frame = self.save_frame(&frame)?;
        }

        let (changed, _) = self.change_detector.detect_change(&frame);
        let changed =
            changed || (self.config.filter_duplicates && !self.motion_filter.is_duplicate(&frame));

        let mut transition = false;
        if changed
            && (!self.config.filter_duplicates
                || broad_transition
                || self.stability_detector.get_pending_frame().is_some())
        {
            // Only a frame that has already been still for one sample counts as
            // a real page; a moving frame is part of the transition.
            if self.state == EngineState::WaitingForStable {
                if let Some(pending) = self.stability_detector.get_pending_frame().cloned() {
                    saved_this_frame |= self.save_frame(&pending)?;
                }
            }
            self.state = EngineState::WaitingForStable;
            self.animation_started.get_or_insert_with(Instant::now);
            self.stability_detector.reset();
            transition = true;
        }

        if self.state == EngineState::WaitingForStable {
            let stable = self.stability_detector.check_stable(&frame);
            let timed_out = self.animation_started.is_some_and(|started| {
                started.elapsed() >= Duration::from_millis(self.config.animation_timeout_ms)
            });
            if stable || timed_out {
                self.state = EngineState::Stable;
                saved_this_frame |= self.save_frame(&frame)?;
                self.state = EngineState::Running;
                self.animation_started = None;
            }
        }

        self.change_detector.update_reference(&frame);
        Ok(FrameOutcome {
            state: self.state.as_str(),
            saved: saved_this_frame,
            black: false,
            transition,
        })
    }

    /// Stop recording: flush a settled page, then publish the PPTX.
    pub fn finish(&mut self) -> Result<PublishReport> {
        if self.state == EngineState::WaitingForStable {
            if let Some(pending) = self.stability_detector.get_pending_frame().cloned() {
                self.save_frame(&pending)?;
            }
        }
        self.state = EngineState::Completed;
        self.session.finish()
    }

    /// Rebuild an unfinished recording. Call after [`CaptureEngine::open`].
    pub fn recover(&mut self) -> Result<PublishReport> {
        let report = self.session.recover()?;
        self.state = EngineState::Completed;
        Ok(report)
    }

    fn save_frame(&mut self, frame: &Frame) -> Result<bool> {
        if self.state == EngineState::Completed {
            return Ok(false);
        }
        let content_hash = self.duplicate_detector.compute_hash(frame);
        if self.config.filter_duplicates {
            if self.motion_filter.is_duplicate(frame) {
                self.skipped_pages += 1;
                return Ok(false);
            }
            if self.last_slide_hash.as_deref() == Some(content_hash.as_str()) {
                self.skipped_pages += 1;
                return Ok(false);
            }
        }

        let slide_number = self.saved_pages as u32 + 1;
        let png_filename = format!("slide_{slide_number:04}.png");
        let record = SlideRecord::new(SlideRecordInput {
            slide_number,
            png_filename: png_filename.clone(),
            png_relative_path: format!("slides/{png_filename}"),
            frame_index: frame.frame_index,
            width: frame.width,
            height: frame.height,
            content_hash: content_hash.clone(),
            source_name: "HarmonyOS screen".to_owned(),
            monitor_name: "primary".to_owned(),
        });

        self.session.save(frame, record)?;
        self.motion_filter.mark_saved(frame);
        self.duplicate_detector.update_last(content_hash.clone());
        self.last_slide_hash = Some(content_hash);
        self.saved_pages += 1;
        self.first_frame_saved = true;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(value: u8) -> Vec<u8> {
        vec![
            value, value, value, 255, value, value, value, 255, value, value, value, 255, value,
            value, value, 255,
        ]
    }

    fn engine(dir: &tempfile::TempDir) -> CaptureEngine {
        CaptureEngine::new(
            dir.path(),
            "ohos-deck.pptx",
            "16:9",
            "fit",
            EngineConfig::default(),
        )
        .unwrap()
    }

    fn submit(engine: &mut CaptureEngine, value: u8) -> FrameOutcome {
        engine.push_frame(solid(value), 2, 2, 1000).unwrap()
    }

    #[test]
    fn first_frame_is_saved_then_duplicates_are_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&dir);
        assert!(submit(&mut engine, 20).saved);
        for _ in 0..4 {
            assert!(!submit(&mut engine, 20).saved);
        }
        assert_eq!(engine.saved_pages, 1);
    }

    #[test]
    fn a_settled_page_is_saved_after_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&dir);
        submit(&mut engine, 20);
        submit(&mut engine, 200);
        for _ in 0..4 {
            submit(&mut engine, 200);
        }
        assert_eq!(engine.saved_pages, 2);
    }

    #[test]
    fn transition_frames_are_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&dir);
        submit(&mut engine, 20);
        for value in [40, 80, 120, 160] {
            let outcome = submit(&mut engine, value);
            assert!(!outcome.saved, "moving frame {value} was saved");
        }
        for _ in 0..4 {
            submit(&mut engine, 240);
        }
        assert_eq!(engine.saved_pages, 2);
    }

    #[test]
    fn finish_publishes_a_readable_pptx() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&dir);
        submit(&mut engine, 20);
        submit(&mut engine, 200);
        for _ in 0..3 {
            submit(&mut engine, 200);
        }
        let report = engine.finish().unwrap();
        assert_eq!(report.published, 2);
        assert!(engine.output_path().is_file());
    }

    #[test]
    fn too_short_a_frame_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&dir);
        assert!(engine.push_frame(vec![0; 8], 2, 2, 0).is_err());
    }
}
