# macOS Native Capture Phase 1 Design

## Goal

Make the Apple Silicon macOS application perform the core capture workflow that has previously only been tested on Windows:

- enumerate real displays and visible windows;
- capture an entire display or one selected window;
- run test capture and continuous slide-change capture;
- reuse the existing stability, duplicate, image, manifest, recovery, and PPTX pipeline;
- report Screen Recording permission and source-loss errors clearly.

This phase does not attempt to reproduce Windows-only window management.

## Dependency and Branching

This work builds on the Apple Silicon DMG application bundle defined in:

`docs/superpowers/specs/2026-07-31-macos-apple-silicon-dmg-design.md`

The DMG PR remains an independently reviewable packaging change. This native-capture branch is stacked on top of it because the application bundle must include the macOS screen-capture usage description.

## Supported Platform

- Apple Silicon (`aarch64-apple-darwin`) only.
- Minimum macOS version remains 11.0 for application launch, subject to the selected capture library's actual API floor.
- Screen capture requires user-granted Screen Recording permission.
- After first granting permission, the application instructs the user to restart it.

Intel macOS is out of scope.

## Capture Backend Choice

Use `xcap` as the macOS capture backend.

Reasons:

- It exposes display and window enumeration plus still-image capture.
- Its macOS implementation uses native capture facilities rather than spawning shell commands.
- It avoids maintaining a custom Objective-C/Swift asynchronous bridge in this Rust application.
- It leaves the existing Windows DXGI/GDI implementation unchanged.

The dependency must be target-specific in `Cargo.toml`:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
xcap = "0.9.7"
```

If dependency inspection shows a higher minimum macOS version than 11.0, the application bundle and README must use the real supported minimum rather than claiming compatibility.

## Module Boundary

Replace the current non-Windows catch-all stub routing with explicit platform routing:

```text
src/windows/mod.rs
├── Windows → existing DXGI/GDI modules
├── macOS   → src/macos/mod.rs
└── Linux   → existing stub
```

The new macOS module owns:

- `enumerate_monitors()`
- `enumerate_windows()`
- `find_monitor()`
- `DxgiCapturer` compatibility adapter
- `GdiCapturer` compatibility adapter
- screen-capture permission status and error classification
- macOS no-op or unsupported window-management functions

The existing `CaptureWorker` should not import xcap directly. It continues using the same interfaces exported by `crate::windows`, minimizing platform branches in the capture pipeline.

The compatibility type names may remain `DxgiCapturer` and `GdiCapturer` for this phase to avoid a broad worker refactor:

- `DxgiCapturer` maps to macOS display capture.
- `GdiCapturer` maps to macOS display or selected-window capture.

The misleading names stay internal and can be replaced by a platform-neutral capture trait in a later refactor.

## Stable Source Identity

The current model uses numeric Windows handles:

- `MonitorInfo.hmonitor: u64`
- `WindowInfo.hwnd: u64`

xcap identifiers must be mapped without pointer casting or process-local collection indexes.

- Monitor identity uses xcap's stable numeric monitor identifier when available.
- Window identity uses xcap's numeric window identifier.
- Enumeration and later capture re-resolve the current xcap object by that identifier.
- A disappeared display or window returns a source-lost error instead of silently capturing a different source.

No fake monitor or window entries are returned on macOS.

## Display Enumeration Mapping

Each xcap monitor maps to `MonitorInfo`:

- `hmonitor`: xcap monitor ID
- `adapter_name`: `Mac`
- `output_name`: monitor name or `Display <id>`
- `description`: physical/pixel dimensions and position
- `region`: x/y, width, and height
- `is_primary`: xcap primary flag
- `is_virtual_suspect`: `false`
- `output_index`: enumeration index
- `adapter_index`: `0`

Enumeration errors must propagate through `anyhow::Result` and appear in the UI rather than returning a mock display.

## Window Enumeration Mapping

Visible capturable xcap windows map to `WindowInfo`.

Filtering rules:

- require a non-empty trimmed title;
- require a positive width and height;
- exclude minimized windows when xcap identifies them as minimized;
- retain on-screen PowerPoint slide-show and editor windows;
- retain other windows so the existing general window selector continues working;
- mark `is_powerpoint` when application name, title, or bundle identity identifies Microsoft PowerPoint or a slide show.

Fields:

- `hwnd`: xcap window ID
- `title`: xcap title
- `class_name`: macOS application name
- `region`: x/y, width, height
- `monitor_hmonitor`: containing monitor ID when determinable, otherwise `0`
- `is_visible`: `true`
- `is_minimized`: xcap minimized state
- `process_id`: process ID when exposed, otherwise `0`
- `process_name`: application name

## Frame Conversion

xcap returns an `image::RgbaImage`. The capture adapter converts it into the project's `Frame` contract:

- output channel order: BGRA;
- four bytes per pixel;
- `stride = width * 4`;
- alpha preserved from the source;
- dimensions unchanged;
- frame index and timestamp supplied by the adapter;
- region describes the captured source at origin `(0, 0)`.

The conversion is a pure function and must be exhaustively unit-tested with literal 1×1 and 2×2 pixel fixtures so channel swaps and stride regressions are detectable.

The existing `ImageStore` continues converting BGRA to RGB PNG. No macOS-specific PNG writer is introduced.

## Display Capture

The display adapter stores only the selected monitor ID.

For each requested frame:

1. Re-enumerate or re-resolve the monitor by ID.
2. Capture its current image.
3. Convert RGBA to the existing BGRA `Frame`.
4. Assign a monotonically increasing frame index.
5. Return a classified error if permission is missing or the display disappeared.

Still-image capture is sufficient for the existing polling worker. Streaming ScreenCaptureKit integration is out of scope for Phase 1.

## Window Capture

When `set_window_hwnd()` receives a nonzero ID, the macOS GDI compatibility adapter records the selected window ID.

For each capture:

- nonzero window ID: re-resolve and capture that exact window;
- zero window ID: capture the selected monitor;
- disappeared window: return a source-lost error;
- permission denial: return a permission-required error.

The `client_w` and `client_h` arguments are ignored on macOS because xcap captures the window content directly.

No additional worker-side clipping is performed for macOS window capture.

## Permission Experience

The application bundle adds:

```xml
<key>NSScreenCaptureUsageDescription</key>
<string>PPT Auto Capture needs screen access to detect and save presentation slides.</string>
```

Behavior:

- display/window refresh or test capture triggers the native permission path through xcap;
- permission-denied errors are classified and translated into a clear bilingual message;
- the message tells the user to open System Settings → Privacy & Security → Screen Recording, enable PPT Auto Capture, then restart the app;
- the application must not loop on permission dialogs or report a generic DXGI/GDI failure.

The application does not attempt to modify privacy settings programmatically.

## Window Management UI

Phase 1 does not request Accessibility permission.

On macOS:

- `move_window_to_monitor()` returns a specific unsupported-operation error.
- `maximize_window()` returns a specific unsupported-operation error.
- UI buttons for Move to Display and Maximize are disabled.
- Nearby text explains that macOS window movement is planned for a later Accessibility-enabled phase.

Windows behavior is unchanged. Linux continues using the stub.

## Pause, Resume, Stop, and Source Loss

The adapter must support the worker lifecycle:

- `initialize`: validate and store the selected source.
- `capture_frame`: capture current content.
- `release`: clear active source state.
- `is_initialized`: reflect whether a valid source was initialized.

On resume, the worker reinitializes the selected display/window using its stable ID.

If a monitor or window disappears:

- do not fall back to another display or window;
- transition to a recoverable error state;
- tell the user to refresh and select a source again.

Sleep/wake-specific native notifications are not required in Phase 1. Re-resolution during polling and resume provides the recovery boundary.

## Testing Strategy

### Pure Unit Tests

Run on macOS without Screen Recording permission:

- RGBA → BGRA conversion for one pixel.
- Conversion for multiple pixels and rows.
- Stride and dimensions.
- Monitor mapping from a complete adapter fixture.
- Window mapping and PowerPoint classification.
- Empty-title, zero-size, and minimized-window filtering.
- Stable-ID lookup and source-lost errors.
- Permission error classification.
- Lifecycle state: initialize, release, resume-compatible reinitialize.

Where xcap types cannot be directly constructed, introduce small internal metadata structs populated from xcap and construct those structs in tests. Do not mock xcap globally.

### Package Tests

Extend the DMG test to require `NSScreenCaptureUsageDescription` and confirm its value is non-empty.

### Manual Apple Silicon Integration Test

Provide a deterministic checklist or ignored test executable that the user can run after granting Screen Recording:

1. Refresh displays and verify real names/resolutions.
2. Refresh windows and verify PowerPoint appears.
3. Test-capture one display and one PowerPoint window.
4. Confirm preview dimensions and colors.
5. Start capture, advance through at least five slides including an animated slide.
6. Pause and resume.
7. Close the selected window and verify source-lost handling.
8. Confirm saved PNG files decode.
9. Open the generated PPTX in PowerPoint and confirm all slides.
10. Restart the application and verify recovery.

The CI environment runs compilation, unit tests, and package tests but must not claim successful real desktop capture because GitHub-hosted runners do not provide interactive Screen Recording permission.

## Documentation

README will clearly state:

- macOS native display/window capture is Phase 1 support;
- Apple Silicon only;
- Screen Recording permission and restart are required;
- window move/maximize remain unavailable;
- a manual verification checklist is provided for the first macOS test cycle.

## Success Criteria

- macOS UI lists real displays and real visible windows, not mock entries.
- Test Capture returns a correctly colored real screenshot.
- Full-display and selected-window continuous capture both feed the existing slide pipeline.
- PNG, manifest, recovery, and PPTX behavior remain shared with Windows.
- Permission denial gives actionable bilingual instructions.
- Lost sources never silently switch to another source.
- Move/maximize controls are disabled on macOS.
- DMG metadata contains the screen-capture usage description.
- Windows tests and Windows release compilation remain unaffected.
- The user can complete the documented Apple Silicon manual test cycle.
