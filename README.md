# PPT Auto Capture GUI

A desktop tool that captures PowerPoint slide show screenshots and produces a real-time `output.pptx`. Capture is supported on Windows and Apple Silicon macOS.

## Features

- **Auto-capture**: Detects slide changes on a selected display, waits for animations to settle, then captures
- **Native capture engines**: DXGI/GDI on Windows and xcap/CoreGraphics on macOS
- **GUI-driven**: No command-line interaction — launch the `.exe` and configure visually
- **Window management**: Select a window, move it to the capture display, maximize it — all from the UI
- **Real-time PPTX**: Each screenshot is immediately appended to `output.pptx`
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
  1.1.0 \
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
                                     │  ├─ manifest.jsonl│
                                     │  └─ output.pptx   │
                                     └──────────────────┘
```

## License

MIT
