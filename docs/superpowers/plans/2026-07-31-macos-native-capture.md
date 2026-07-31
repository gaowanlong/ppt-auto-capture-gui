# macOS Native Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Apple Silicon macOS application enumerate and capture real displays and windows while preserving the existing capture, detection, storage, recovery, and PPTX pipeline.

**Architecture:** Keep the existing `crate::windows` compatibility interface used by the capture worker, but route macOS to a new xcap-backed implementation and leave Windows and Linux unchanged. Isolate deterministic conversion and error classification so they can be tested without Screen Recording permission or a GUI session; cover packaging metadata and platform routing with shell tests.

**Tech Stack:** Rust 2021, xcap 0.9.7, image 0.25, anyhow, cargo test, Bash packaging tests, GitHub Actions, Apple Silicon macOS.

## Global Constraints

- macOS support is Apple Silicon (`aarch64-apple-darwin`) only.
- Use `xcap = "0.9.7"` as a target-specific macOS dependency.
- Do not require an Apple Developer signing certificate; keep ad-hoc signing.
- Capture output must use the existing BGRA `Frame` contract.
- A missing display or window must stop with an actionable error and must never silently switch sources.
- Screen Recording permission errors must tell the user, in English and Chinese, to enable the app in System Settings and restart it.
- Move-to-display and maximize remain unavailable on macOS; Accessibility permission support is outside this phase.
- Windows behavior and the Linux testing stub must not regress.

---

### Task 1: Route macOS to a Native Backend

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/windows/mod.rs`
- Create: `src/macos/mod.rs`

**Interfaces:**
- Consumes: the existing `crate::windows` public API.
- Produces: a macOS module exporting `DxgiCapturer`, `GdiCapturer`, `SessionEventMonitor`, `SessionState`, `enumerate_monitors`, `enumerate_windows`, `get_client_window_rect`, `get_window_rect`, `move_window_to_monitor`, and `maximize_window`.

- [ ] **Step 1: Write a failing platform-routing test**

Add a `#[cfg(target_os = "macos")]` unit test in `src/windows/mod.rs` that asserts `crate::windows::backend_name() == "macos-xcap"`.

- [ ] **Step 2: Verify the test fails**

Run: `cargo test windows::tests::macos_uses_xcap_backend`

Expected: compilation fails because `backend_name` and the macOS backend do not exist.

- [ ] **Step 3: Add the dependency and routing skeleton**

Add:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
xcap = "0.9.7"
```

Route `target_os = "macos"` to `src/macos/mod.rs`, restrict `stub.rs` to Linux/other non-Windows non-macOS targets, and implement `pub const fn backend_name() -> &'static str { "macos-xcap" }` in the macOS module.

- [ ] **Step 4: Verify routing and compilation**

Run: `cargo test windows::tests::macos_uses_xcap_backend`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/windows/mod.rs src/macos/mod.rs
git commit -m "build: route macOS to xcap capture backend"
```

### Task 2: Convert xcap Metadata and Pixels

**Files:**
- Create: `src/macos/adapter.rs`
- Modify: `src/macos/mod.rs`

**Interfaces:**
- Produces: `rgba_to_bgra(Vec<u8>) -> Vec<u8>`, `monitor_info(MonitorSnapshot, usize) -> MonitorInfo`, `window_info(WindowSnapshot) -> WindowInfo`, and snapshot structs containing primitive xcap metadata.
- Produces: `capture_error_message(kind: CaptureErrorKind, detail: &str) -> String`.

- [ ] **Step 1: Write failing pure unit tests**

Test that `[10, 20, 30, 40]` RGBA becomes `[30, 20, 10, 40]` BGRA; negative display coordinates survive mapping; PowerPoint app names set `is_powerpoint`; and permission/source-loss messages contain both English and Chinese recovery instructions.

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test macos::adapter::tests`

Expected: compilation fails because the adapter functions and types do not exist.

- [ ] **Step 3: Implement the pure adapter**

Define `MonitorSnapshot`, `WindowSnapshot`, and `CaptureErrorKind::{PermissionDenied, DisplayLost, WindowLost, Other}`. Map xcap IDs into the existing `u64` handle fields, use `width * 4` stride, preserve coordinates, classify PowerPoint by case-insensitive app name/title matching, and swap only red/blue channels.

- [ ] **Step 4: Verify the adapter tests**

Run: `cargo test macos::adapter::tests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/macos/mod.rs src/macos/adapter.rs
git commit -m "feat: add macOS capture data adapters"
```

### Task 3: Enumerate and Capture Displays and Windows

**Files:**
- Modify: `src/macos/mod.rs`
- Modify: `src/macos/adapter.rs`

**Interfaces:**
- Consumes: xcap `Monitor::all`, `Window::all`, `capture_image`, and Task 2 adapters.
- Produces: drop-in capture types with the same signatures as `src/windows/stub.rs`.

- [ ] **Step 1: Write failing lifecycle and error-classification tests**

Test that new capturers report uninitialized, release returns them to uninitialized, capture before initialize returns `"No capturer initialized"`, and representative permission-denied/source-missing error text maps to the correct bilingual error category.

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test macos::tests`

Expected: FAIL because capturer state and classification are not implemented.

- [ ] **Step 3: Implement monitor/window enumeration and lookup**

Use `xcap::Monitor::all()` and `xcap::Window::all()`, filter zero-sized and invisible windows, preserve xcap IDs, and sort output deterministically. On every initialization/capture, resolve the selected stable ID again; return `DisplayLost` or `WindowLost` if absent.

- [ ] **Step 4: Implement frame capture**

`DxgiCapturer` captures the selected monitor and returns `Ok(Some(Frame))`. `GdiCapturer` captures a selected window when `set_window_hwnd` received a nonzero ID and otherwise captures its initialized monitor. Convert xcap RGBA pixels to BGRA and maintain a monotonically increasing frame index and timestamp.

- [ ] **Step 5: Implement unsupported window operations and session monitor**

Return explicit macOS unsupported errors for move/maximize, return xcap-backed rectangles for valid stable IDs, and keep the no-op session receiver behavior because macOS session lock integration is outside this phase.

- [ ] **Step 6: Verify the backend**

Run: `cargo test macos::`

Expected: PASS without requiring Screen Recording permission because hardware calls are not made by unit tests.

- [ ] **Step 7: Commit**

```bash
git add src/macos/mod.rs src/macos/adapter.rs
git commit -m "feat: capture macOS displays and windows with xcap"
```

### Task 4: Integrate macOS with the Capture Worker and UI

**Files:**
- Modify: `src/capture/capture_worker.rs`
- Modify: `src/ui/source_panel.rs`
- Modify: `src/i18n.rs`

**Interfaces:**
- Consumes: the drop-in macOS capturers from Task 3.
- Produces: normal test capture and continuous capture on macOS; disabled move/maximize controls with localized explanation.

- [ ] **Step 1: Write failing worker-policy and localization tests**

Extract and test `capture_backend_unavailable_message()` so it returns the Windows-only testing warning only on Linux/unsupported targets, not macOS. Add i18n tests for the English and Chinese macOS explanation that window movement requires Accessibility permission and is unavailable in this phase.

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test capture_worker::tests i18n::tests`

Expected: FAIL because the policy helper and localized strings do not exist.

- [ ] **Step 3: Remove the macOS stub rejection**

Replace `cfg!(not(target_os = "windows"))` in the `"No capturer initialized"` branch with a helper that only rejects unsupported platforms. Preserve normal worker error propagation on macOS so permission/source-loss errors reach the UI.

- [ ] **Step 4: Disable unsupported macOS controls**

Use target-aware UI enablement so move/maximize stay enabled on Windows, are disabled on macOS with localized hover text, and preserve existing behavior elsewhere.

- [ ] **Step 5: Verify integration**

Run: `cargo test capture_worker::tests i18n::tests`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/capture/capture_worker.rs src/ui/source_panel.rs src/i18n.rs
git commit -m "feat: enable macOS capture workflow"
```

### Task 5: Declare and Test Screen Recording Permission Metadata

**Files:**
- Modify: `tests/test_macos_dmg_packaging.sh`
- Modify: `scripts/package-macos-dmg.sh`

**Interfaces:**
- Produces: `PPT Auto Capture.app/Contents/Info.plist` with a nonempty `NSScreenCaptureUsageDescription`.

- [ ] **Step 1: Add a failing packaging assertion**

After mounting/extracting the packaged app, assert with `/usr/libexec/PlistBuddy` that `NSScreenCaptureUsageDescription` exists and mentions screen/window capture.

- [ ] **Step 2: Verify the packaging test fails**

Run: `bash tests/test_macos_dmg_packaging.sh`

Expected: FAIL because the key is absent.

- [ ] **Step 3: Add the plist usage description**

Add:

```xml
<key>NSScreenCaptureUsageDescription</key>
<string>PPT Auto Capture needs screen recording access to capture the selected display or presentation window.</string>
```

- [ ] **Step 4: Verify package metadata and signature**

Run: `bash tests/test_macos_dmg_packaging.sh`

Expected: PASS, including ad-hoc signature verification and the new plist assertion.

- [ ] **Step 5: Commit**

```bash
git add tests/test_macos_dmg_packaging.sh scripts/package-macos-dmg.sh
git commit -m "fix: declare macOS screen recording permission"
```

### Task 6: Document, Build, and Regression-Test the macOS Release

**Files:**
- Modify: `README.md`
- Create: `docs/testing/macos-manual-test-checklist.md`

**Interfaces:**
- Produces: repeatable manual coverage for permission denial/grant, display/window capture, test capture, pause/resume/stop, source loss, PPTX opening, and DMG launch.

- [ ] **Step 1: Write the manual checklist**

Include exact expected results for first-launch permission denial, System Settings grant plus app restart, full-display capture, selected PowerPoint window capture, test capture, continuous capture, pause/resume/stop, unplugged display/closed window errors, generated PPTX opening without repair, and launching the app from the DMG.

- [ ] **Step 2: Update supported-platform documentation**

Document Apple Silicon support, unsigned/ad-hoc Gatekeeper behavior, Screen Recording permission setup, restart requirement, and the macOS move/maximize limitation.

- [ ] **Step 3: Run the complete verification matrix**

Run:

```bash
cargo fmt --check
cargo test
cargo check --target x86_64-pc-windows-msvc
bash tests/test_macos_dmg_packaging.sh
bash tests/test_release_workflow.sh
```

Expected: all commands PASS.

- [ ] **Step 4: Build the release DMG**

Run:

```bash
cargo build --release --target aarch64-apple-darwin
bash scripts/package-macos-dmg.sh target/aarch64-apple-darwin/release/ppt-auto-capture-gui dist
```

Expected: `dist/ppt-auto-capture-gui-macos-apple-silicon.dmg` exists, mounts, contains `PPT Auto Capture.app`, and the executable reports `arm64`.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/testing/macos-manual-test-checklist.md
git commit -m "docs: add macOS native capture test guide"
```

- [ ] **Step 6: Push and trigger GitHub build**

Run:

```bash
git push origin codex/macos-native-capture
gh workflow run build-release.yml --ref codex/macos-native-capture
```

Expected: branch push succeeds and GitHub returns a new workflow run for this branch.
