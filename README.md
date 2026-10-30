# PPT Auto Capture GUI

A desktop tool that captures presentation screenshots and creates PPTX files after recording stops. Capture is supported on Windows and Apple Silicon macOS.

## Features

- **Auto-capture**: Detects slide changes on a selected display, waits for animations to settle, then captures
- **Native capture engines**: DXGI/GDI on Windows and xcap/CoreGraphics on macOS
- **GUI-driven**: No command-line interaction — launch the `.exe` and configure visually
- **Window management**: Select a window, move it to the capture display, maximize it — all from the UI
- **Deferred PPTX**: Screenshots are saved during capture; PPTX files are generated on Stop, with up to 50 slides each
- **Crash recovery**: On restart, detects incomplete sessions and offers to rebuild the PPTX
- **Protected content detection**: Black/blank frames enter a safe state without trying to bypass DRM
- **Smart capture pipeline**: Change detection → stability detection → dedup → save

## Quick Start — Windows

1. Download the latest `ppt-auto-capture-gui.exe` from [Releases](https://github.com/gaowanlong/ppt-auto-capture-gui/releases)
2. Double-click to launch
3. Go to **Display** tab → click "Refresh Displays" → select the monitor/screen to capture
4. Go to **Window** tab → click "Refresh Window List" → select the window you want to track
5. (Optional) Click "Move to Display" to place the window on the capture monitor
6. Go to **Dashboard** → click **Start**
7. Switch to your PowerPoint slideshow — the tool watches for slide changes automatically
8. Click **Stop** and wait while the PPTX files are generated

## Install — macOS Apple Silicon

The macOS release supports Apple Silicon Macs only.

1. Download `ppt-auto-capture-gui-macos-apple-silicon.dmg` from [Releases](https://github.com/gaowanlong/ppt-auto-capture-gui/releases).
2. Open the DMG and drag `PPT Auto Capture.app` into the Applications folder.
3. Because this build is ad-hoc signed and not notarized, macOS may block the first launch. If that happens, right-click `PPT Auto Capture.app`, choose **Open**, then confirm **Open**.
4. If macOS requests Screen Recording permission, enable it in **System Settings → Privacy & Security → Screen Recording**.
5. Restart `PPT Auto Capture.app` after changing Screen Recording permission.

On macOS, refresh and select either a display for full-screen capture or a specific presentation window. **Test Screenshot**, continuous capture, pause, resume, stop, PNG output, recovery, and PPTX generation use the same pipeline as Windows.

Moving or maximizing another application's window is disabled on macOS because it requires Accessibility permission; arrange the presentation window manually before capture. If a selected display is disconnected or a selected window closes, capture stops with an error instead of switching to another source.

The macOS app starts with the primary display and full-screen capture selected. Its default writable output directory is `~/Documents/PPT Auto Capture`; it can be changed in the Output tab.

### Recording files and recovery

Each recording now has its own `sessions/<timestamp>-<unique-id>/` directory under the selected output folder. It contains the chosen PPTX filename, a `slides/` folder, and `session.json`. The dashboard shows the actual PPTX path. Starting another recording, even with the same filename, does not overwrite earlier recordings.

During recording only PNG screenshots and the session journal are saved. After Stop, the application generates PPTX files in the background, each containing at most 50 slides. The first file retains the chosen name; subsequent files in the same recording folder are named, for example, `output-part-002.pptx`, `output-part-003.pptx`, and so on. No empty extra file is created at the 50-slide boundary. The dashboard displays the target file and total saved screenshots. Screenshots remain numbered in order in the shared `slides/` folder. Older unfinished sessions remain recoverable in their original format. New recordings use a newer journal format and must be recovered with this version or a later compatible version, not an older application.

If saving a screenshot fails, capture pauses and retains the pending frame in memory. Resolve the storage problem, then choose **Resume** to retry that same screenshot. The saved-page count advances after PNG and journal writes succeed. If PPTX generation fails after Stop (for example, a file is locked or the disk is full), the session remains incomplete and **Stop** retries generation. Closing or force-quitting can lose frames not yet committed to the journal; resolve an error before closing.

When PPTX generation remains blocked, **Stop and keep screenshots (generate PPTX later)** ends capture without retrying the PPTX and keeps its PNGs and incomplete journal for recovery on restart. If a pending screenshot cannot be durably written, the recording stays paused instead of discarding it. The diagnostic message includes the underlying system error and any completed temporary PPTX path. On Windows the application briefly retries transient sharing or access errors; a persistently locked file still needs attention.

The dashboard also offers **Delete this recording's folder**. It asks for confirmation and then removes only the current session directory (never the shared `sessions` folder or unrelated files). The Output tab's **Manage recording folders…** dialog lists every session under the selected output directory, including folders left by earlier runs, and uses the same safe deletion. On Windows a single open file handle prevents its whole directory from being deleted, even after the application exits. Deletion therefore retries for several seconds, clears read-only attributes, and, if it still fails, asks the Windows Restart Manager which processes are using files in the folder and reports their names and PIDs. Typical holders are WPS or Office preview handlers, antivirus scans, and cloud-sync clients; close the reported program and choose Delete again. If no process is named, the dashboard's **Copy diagnostics** report records the failure for support.

The **Create PPTX** tab imports PNG and JPEG images directly from a selected folder, in natural filename order, into a separate export folder. It also uses the 50-slide limit and leaves the source files unchanged. The optional automatic crop scans inward from the outer edges of a desktop screenshot, detects the large light presentation canvas, and trims the taskbar, meeting-window chrome, and letterbox bars around it. It keeps colored title and footer bands as well as photos that touch the slide edge, snaps the final boundary at full resolution, and crops to the actual canvas aspect ratio instead of forcing 16:9 or 4:3. A conservative dark-background fallback applies only when a clear rectangle with standard ratio is found; otherwise the full image is used rather than risk black bars or cutting into the slide. Cropping changes only the image embedded in PPTX, never the original screenshots. Detection is heuristic, so inspect generated files before relying on the result.

On restart, recovery rebuilds unfinished recordings from their journal and PNGs, preserving the original filename, slide size, and image-fit setting. Keep each session folder together. Missing/corrupt images or journals produce an error instead of a silently incomplete presentation; existing PPTX files remain intact. Legacy `manifest.jsonl` recordings lack layout metadata and therefore retain the old 16:9/fit recovery default. Recovery cannot infer pages already overwritten by older versions.

### macOS troubleshooting and diagnostics

Application logs are stored at `~/Library/Logs/PPT Auto Capture`; the app retains the newest ten application logs. On the Dashboard, choose **Copy diagnostics** to put a concise report on the clipboard. The report includes application, OS, architecture, capture-state, approved selected source/display descriptions, and local output/log paths so a support report has useful context.

Diagnostics exclude screenshots and slide contents: they do not include screenshot pixels, window or slide contents, existing clipboard data, arbitrary environment variables, or serialized configuration. If persistent logging cannot be initialized, capture remains available and diagnostics continue through the in-app history and standard error output.

### Windows long recordings, videos, and locked sessions

Windows capture releases its screen/window DC, memory DC, and bitmap on every success and failure path. It deselects the bitmap before reading pixels and checks the full scan-line count. A resource-allocation or pixel-read failure reports an error rather than passing incomplete pixels to the slide detector. The application logs GDI object counts and working-set memory once a minute, along with saved-page and pending-save state. Rust panic information is written to the launch log.

Replacing or dropping a capture worker also stops its background thread. Per-frame UI sampling notifications are limited when the UI is slow; saved-slide, error, and state notifications remain reliable.

With duplicate filtering enabled, small pointer changes are ignored. A bounded spatial signature learns repeatedly moving local areas (up to 40% of the picture), allowing a video rectangle to change without repeatedly saving the same page. Changes elsewhere still trigger a new page. Motion exclusions expire after 15 quiet samples so later static content in the same area remains eligible; this can also save a video's settled final frame. Continuous local motion no longer forces the rapid-transition path to save every frame, and animation timeout is measured from the beginning of the pending change. Broad rapid slide changes and intermediate pages held for multiple samples retain their capture path. This is an image heuristic: full-screen video, very briefly displayed same-template pages, and pages differing only by tiny details cannot be reliably classified as slide identities. Turn off duplicate filtering when every small animation step must be recorded.

When Windows locks, switches to a secure desktop, or disconnects an RDP session, capture waits without saving that desktop's images. When the original desktop becomes available, it retries the selected source automatically. It preserves pending saved work and respects a manual pause or stop. A selected window stays a window source after resume; it is not silently replaced with a monitor screenshot. Ordinary desktop screenshot APIs cannot reliably capture the original PowerPoint display while it is locked. If uninterrupted capture during a host lock is mandatory, use a separate presentation/capture session that remains unlocked; changing the screenshot API alone does not remove this Windows restriction.

For release acceptance on Windows, run one capture for at least 24 hours at the normal sample interval, including static pages, local video, pointer movement, repeated lock/unlock, and RDP disconnect/reconnect if used. Inspect the launch log for growing GDI counts or memory, verify each intended slide in the output, and verify Stop still completes. Automated tests include a 20,000-frame video/change sequence and a Windows-native 4,000-allocation GDI cleanup test; these do not substitute for that hardware-specific soak test.

Windows CI now also pressure-tests real window capture, cleanup and reinitialization. The manually dispatched **Windows native stress** workflow produces JSON resource reports and a standalone stress kit with ten-minute and 24-hour launchers. See [Windows stress testing](docs/testing/windows-stress.md) for running the kit without Rust, report thresholds and the distinction between controlled-window stress and real meeting-app acceptance. The UI event mailbox is bounded during state/error storms; screenshot previews work before recording, retain their actual aspect ratio and reuse their GPU texture. Stop interrupts the sample wait and shows a saving status; the Dashboard can open the current recording's folder directly.

## Building from Source

### Prerequisites

- Rust 1.75+ (`rustup` recommended)
- Windows SDK (on Windows) or MinGW-w64 (cross-compilation)
- Xcode Command Line Tools (for the Apple Silicon macOS build and DMG)

### Build

```bash
git clone https://github.com/gaowanlong/ppt-auto-capture-gui.git
cd ppt-auto-capture-gui
cargo build --release
```

### Build the Apple Silicon DMG

```bash
cargo build --release --target aarch64-apple-darwin
bash scripts/package-macos-dmg.sh \
  target/aarch64-apple-darwin/release/ppt-auto-capture-gui \
  2.6.2 \
  dist/ppt-auto-capture-gui-macos-apple-silicon.dmg
```

The app is ad-hoc signed, not Developer ID signed or notarized.

### Cross-compile for Windows (from macOS/Linux)

```bash
rustup target add x86_64-pc-windows-gnu
cargo zigbuild --target x86_64-pc-windows-gnu --release
```

## Architecture

```
┌─────────────────┐     channels     ┌──────────────────┐
│   GUI (eframe)  │◄────────────────►│  CaptureWorker    │
│  Dashboard      │   commands/events│  ├─ DXGI (primary)│
│  SourcePanel    │                  │  └─ GDI (fallback)│
│  DisplayPanel   │                  └────────┬─────────┘
│  SettingsPanel  │                           │
│  OutputPanel    │                  ┌────────▼─────────┐
└─────────────────┘                  │  DetectionWorker  │
                                     │  ├─ ChangeDetect  │
                                     │  ├─ StabilityDet  │
                                     │  ├─ DupDetect     │
                                     │  └─ BlackDetect   │
                                     └────────┬─────────┘
                                              │
                                     ┌────────▼─────────┐
                                     │   StorageWorker   │
                                     │  ├─ PNG (atomic)  │
                                     │  ├─ session.json  │
                                     │  └─ output.pptx   │
                                     └──────────────────┘
```

## License

MIT
