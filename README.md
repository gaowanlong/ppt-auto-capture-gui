# PPT Auto Capture GUI

A Windows desktop tool that automatically captures PowerPoint slide show screenshots and produces a real-time `output.pptx`.

## Features

- **Auto-capture**: Detects slide changes on a selected display, waits for animations to settle, then captures
- **Dual capture engine**: DXGI Desktop Duplication (primary) with GDI fallback
- **GUI-driven**: No command-line interaction — launch the `.exe` and configure visually
- **Window management**: Select a window, move it to the capture display, maximize it — all from the UI
- **Real-time PPTX**: Each screenshot is immediately appended to `output.pptx`
- **Crash recovery**: On restart, detects incomplete sessions and offers to rebuild the PPTX
- **Protected content detection**: Black/blank frames enter a safe state without trying to bypass DRM
- **Smart capture pipeline**: Change detection → stability detection → dedup → save

## Quick Start

1. Download the latest `ppt-auto-capture-gui.exe` from [Releases](https://github.com/gaowanlong/ppt-auto-capture-gui/releases)
2. Double-click to launch
3. Go to **Display** tab → click "Refresh Displays" → select the monitor/screen to capture
4. Go to **Window** tab → click "Refresh Window List" → select the window you want to track
5. (Optional) Click "Move to Display" to place the window on the capture monitor
6. Go to **Dashboard** → click **Start**
7. Switch to your PowerPoint slideshow — the tool watches for slide changes automatically

## Building from Source

### Prerequisites

- Rust 1.75+ (`rustup` recommended)
- Windows SDK (on Windows) or MinGW-w64 (cross-compilation)

### Build

```bash
git clone https://github.com/gaowanlong/ppt-auto-capture-gui.git
cd ppt-auto-capture-gui
cargo build --release
```

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
