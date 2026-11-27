//! Shared Rust core for the HarmonyOS PC build.
//!
//! The four `#[path]` modules below are the *same source files* used by the
//! Windows/macOS binary (`src/pptx`, `src/detection`, `src/storage`,
//! `src/model`). Nothing is copied, so a fix to slide detection or PPTX
//! packaging reaches every platform at once.
//!
//! `engine` reproduces the frame-decision state machine from the desktop
//! capture worker without depending on eframe, channels or OS capture APIs.
//! The ArkUI side only needs to deliver BGRA frames at the configured interval.

#[path = "../../../src/detection/mod.rs"]
pub mod detection;
#[path = "../../../src/model/mod.rs"]
pub mod model;
#[path = "../../../src/pptx/mod.rs"]
pub mod pptx;
#[path = "../../../src/storage/mod.rs"]
pub mod storage;

mod engine;
pub use engine::*;

#[cfg(target_env = "ohos")]
mod napi_bridge;
