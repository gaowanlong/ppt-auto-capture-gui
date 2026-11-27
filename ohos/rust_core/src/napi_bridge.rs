//! N-API surface consumed by the ArkUI layer.
//!
//! Exports return JSON strings so the TypeScript side only needs a small
//! declared interface; no generated `.d.ts` has to be kept in sync with nested
//! Rust structs. A Rust error becomes an exception with a readable message.

use crate::pptx::{crop_deck_images, CropPlacement};
use crate::{CaptureEngine, EngineConfig};
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Uint8Array;
use napi_ohos::{Error as NapiError, Result as NapiResult};
use std::path::Path;

fn fail(error: impl std::fmt::Display) -> NapiError {
    NapiError::from_reason(error.to_string())
}

fn parse_config(json: &str) -> NapiResult<EngineConfig> {
    if json.trim().is_empty() {
        return Ok(EngineConfig::default());
    }
    serde_json::from_str(json).map_err(fail)
}

/// One recording session. ArkTS owns the instance and feeds it BGRA frames.
#[napi]
pub struct OhosSession {
    engine: Option<CaptureEngine>,
    init_error: String,
}

#[napi]
impl OhosSession {
    #[napi(constructor)]
    pub fn new(
        root: String,
        filename: String,
        ratio: String,
        fit: String,
        config_json: String,
    ) -> Self {
        match CaptureEngine::new(
            Path::new(&root),
            &filename,
            &ratio,
            &fit,
            parse_config(&config_json).unwrap_or_default(),
        ) {
            Ok(engine) => Self {
                engine: Some(engine),
                init_error: String::new(),
            },
            Err(error) => Self {
                engine: None,
                init_error: error.to_string(),
            },
        }
    }

    /// Empty when construction succeeded; otherwise the reason to show in UI.
    #[napi(js_name = "initError")]
    pub fn init_error(&self) -> String {
        self.init_error.clone()
    }

    /// Feed one BGRA screenshot. Returns a JSON status string.
    #[napi(js_name = "pushFrame")]
    pub fn push_frame(
        &mut self,
        data: Uint8Array,
        width: u32,
        height: u32,
        timestamp_ms: f64,
    ) -> NapiResult<String> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| fail("Session was not created"))?;
        let outcome = engine
            .push_frame(data.as_ref().to_vec(), width, height, timestamp_ms as u64)
            .map_err(fail)?;
        Ok(serde_json::json!({
            "state": outcome.state,
            "saved": outcome.saved,
            "black": outcome.black,
            "transition": outcome.transition,
            "savedPages": engine.saved_pages,
            "skippedPages": engine.skipped_pages,
        })
        .to_string())
    }

    /// Stop and build the PPTX. Returns the publish report as JSON.
    #[napi]
    pub fn finish(&mut self) -> NapiResult<String> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| fail("Session was not created"))?;
        let report = engine.finish().map_err(fail)?;
        Ok(serde_json::json!({
            "published": report.published,
            "skipped": report.skipped,
            "decks": report.decks,
            "output": engine.output_path(),
        })
        .to_string())
    }

    #[napi]
    pub fn state(&self) -> String {
        self.engine
            .as_ref()
            .map_or("uninitialized", |engine| engine.state().as_str())
            .to_owned()
    }

    #[napi(js_name = "sessionDirectory")]
    pub fn session_directory(&self) -> NapiResult<String> {
        let engine = self
            .engine
            .as_ref()
            .ok_or_else(|| fail("Session was not created"))?;
        Ok(engine.session_directory().to_string_lossy().into_owned())
    }
}

/// Resume an unfinished session; used on startup after a crash or a forced stop.
#[napi(js_name = "openSession")]
pub fn open_session(directory: String, config_json: String) -> NapiResult<OhosSession> {
    let engine =
        CaptureEngine::open(Path::new(&directory), parse_config(&config_json)?).map_err(fail)?;
    Ok(OhosSession {
        engine: Some(engine),
        init_error: String::new(),
    })
}

/// Rebuild an unfinished recording without keeping a live session.
#[napi(js_name = "recoverSession")]
pub fn recover_session(directory: String, config_json: String) -> NapiResult<String> {
    let mut engine =
        CaptureEngine::open(Path::new(&directory), parse_config(&config_json)?).map_err(fail)?;
    let report = engine.recover().map_err(fail)?;
    Ok(serde_json::json!({
        "published": report.published,
        "skipped": report.skipped,
        "decks": report.decks,
    })
    .to_string())
}

/// Trim a finished PPTX. `placement` is `fill` or `keep`.
#[napi(js_name = "trimDeck")]
pub fn trim_deck(source: String, output_dir: String, placement: String) -> NapiResult<String> {
    let mode = match placement.as_str() {
        "keep" => CropPlacement::KeepScale,
        _ => CropPlacement::FillArea,
    };
    let filename = Path::new(&source)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| fail("The source has no file name"))?
        .to_owned();
    let summary = crop_deck_images(
        Path::new(&source),
        Path::new(&output_dir),
        &filename,
        mode,
        |_, _| {},
    )
    .map_err(fail)?;
    Ok(serde_json::json!({
        "output": summary.output,
        "slides": summary.slides,
        "cropped": summary.cropped,
    })
    .to_string())
}
