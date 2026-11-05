# PPT Auto Capture GUI

A desktop tool that captures presentation screenshots and creates PPTX files after recording stops. Capture is supported on Windows and Apple Silicon macOS.

## Features

- **Auto-capture**: Detects slide changes on a selected display or window, waits for the animation to settle, then captures
- **Transition skipping**: Morph, push, wipe, and fade frames never reach the deck — only the picture that has come to rest is saved
- **Native capture engines**: DXGI/GDI on Windows and xcap/CoreGraphics on macOS
- **GUI-driven**: No command-line interaction — launch the `.exe` and configure visually
- **Focused workspace UI**: Sidebar navigation, card-based settings, a pine palette in light and dark, and reviewed English/Chinese layouts
- **Appearance page**: Switch between light, dark, and matching the system, and put your own picture behind the workspace
- **Minimize on Start**: Record with the main window minimized; restore it from the taskbar or Dock when you want to stop
- **Window management**: Select a window, move it to the capture display, maximize it — all from the UI
- **Deferred PPTX**: Screenshots are saved during capture; PPTX files are generated on Stop, with up to 50 slides each
- **Recording folders and diagnostics**: Each run gets its own folder you can open, list, or delete from the UI, and a diagnostics report can be copied for support
- **Trim an existing deck**: Point the Create PPTX page at a finished `.pptx` and get a new copy whose embedded screenshots have their taskbar, meeting chrome, and letterbox bars cropped away
- **Crash recovery**: On restart, detects incomplete sessions and offers to rebuild the PPTX
- **Protected content detection**: Black/blank frames enter a safe state without trying to bypass DRM
- **Smart capture pipeline**: Change detection → stability detection → dedup → save

## Quick Start — Windows

1. Download the latest `ppt-auto-capture-gui.exe` from [Releases](https://github.com/gaowanlong/ppt-auto-capture-gui/releases)
2. Double-click to launch
3. Go to the **Display** page → click "Refresh Displays" → select the monitor/screen to capture
4. Go to the **Window** page → click "Refresh Window List" → select the window you want to track
5. (Optional) Click "Move to Display" to place the window on the capture monitor
6. Go to **Dashboard** → optionally enable **Start minimized** → click **Start**
7. Switch to your PowerPoint slideshow — the tool watches for slide changes automatically
8. Restore the window from the taskbar, then click **Stop** and wait while the PPTX files are generated

## Install — macOS Apple Silicon

The macOS release supports Apple Silicon Macs only.

1. Download `ppt-auto-capture-gui-macos-apple-silicon.dmg` from [Releases](https://github.com/gaowanlong/ppt-auto-capture-gui/releases).
2. Open the DMG and drag `PPT Auto Capture.app` into the Applications folder.
3. Because this build is ad-hoc signed and not notarized, macOS may block the first launch. If that happens, right-click `PPT Auto Capture.app`, choose **Open**, then confirm **Open**.
4. If macOS requests Screen Recording permission, enable it in **System Settings → Privacy & Security → Screen Recording**.
5. Restart `PPT Auto Capture.app` after changing Screen Recording permission.

On macOS, refresh and select either a display for full-screen capture or a specific presentation window. **Test Screenshot**, continuous capture, pause, resume, stop, PNG output, recovery, and PPTX generation use the same pipeline as Windows.

Moving or maximizing another application's window is disabled on macOS because it requires Accessibility permission; arrange the presentation window manually before capture. If a selected display is disconnected or a selected window closes, capture stops with an error instead of switching to another source.

The macOS app starts with the primary display and full-screen capture selected. Its default writable output directory is `~/Documents/PPT Auto Capture`; it can be changed on the Output page.

## Recording files and recovery

Each recording has its own `sessions/<timestamp>-<unique-id>/` directory under the selected output folder. It contains the chosen PPTX filename, a `slides/` folder, and `session.json`. The dashboard shows the actual PPTX path. Starting another recording, even with the same filename, does not overwrite earlier recordings.

During recording only PNG screenshots and the session journal are saved. After Stop, the application generates PPTX files in the background, each containing at most 50 slides. The first file retains the chosen name; subsequent files in the same recording folder are named, for example, `output-part-002.pptx`, `output-part-003.pptx`, and so on. No empty extra file is created at the 50-slide boundary. The dashboard displays the target file and total saved screenshots. Screenshots remain numbered in order in the shared `slides/` folder. Older unfinished sessions remain recoverable in their original format. New recordings use a newer journal format and must be recovered with this version or a later compatible version, not an older application.

If saving a screenshot fails, capture pauses and retains the pending frame in memory. Resolve the storage problem, then choose **Resume** to retry that same screenshot. The saved-page count advances after PNG and journal writes succeed. If PPTX generation fails after Stop (for example, a file is locked or the disk is full), the session remains incomplete and **Stop** retries generation. Closing or force-quitting can lose frames not yet committed to the journal; resolve an error before closing.

When PPTX generation remains blocked, **Stop and keep screenshots (generate PPTX later)** ends capture without retrying the PPTX and keeps its PNGs and incomplete journal for recovery on restart. If a pending screenshot cannot be durably written, the recording stays paused instead of discarding it. The diagnostic message includes the underlying system error and any completed temporary PPTX path. On Windows the application briefly retries transient sharing or access errors; a persistently locked file still needs attention.

Screenshots can also be deleted from `slides/` while recording — for example to drop a duplicate page. Stop then builds the deck from the pages that are still there and reports how many were skipped in the activity log; a missing, empty, or truncated PNG never blocks Stop or the recovery offer. Deleting a page is final: it is not re-captured, and the remaining pages keep their order.

On restart, recovery rebuilds unfinished recordings from their journal and PNGs, preserving the original filename, slide size, and image-fit setting. Keep each session folder together. A screenshot deleted by hand is left out of the rebuilt deck and counted in the log; a corrupt journal still produces an error instead of a silently incomplete presentation, and existing PPTX files remain intact. Legacy `manifest.jsonl` recordings lack layout metadata and therefore retain the old 16:9/fit recovery default. Recovery cannot infer pages already overwritten by older versions.

The dashboard also offers **Delete recording folder…**. It asks for confirmation and then removes only the current session directory (never the shared `sessions` folder or unrelated files). The Output page's **Manage recording folders…** dialog lists every session under the selected output directory, including folders left by earlier runs, and uses the same safe deletion. On Windows a directory cannot be removed while any program holds one of its files **or the folder itself** open. Deletion therefore retries for about 18 seconds, clears read-only attributes, and then probes every remaining file and the folder itself with delete access to name exactly what still blocks removal; it also asks the Windows Restart Manager for process names and PIDs. The most common folder-level holder is an Explorer window opened inside the session folder, so the dashboard's recording-folder button reveals the folder in its parent instead of opening it. If deletion still fails, the dashboard keeps a **Retry delete** action and the full list of locked files and processes appears in the error banner and in **Copy diagnostics**. Other typical holders are WPS or Office preview handlers, antivirus scans, Explorer preview panes, and cloud-sync clients; close the reported program and retry. If no holder is named, send the diagnostics report for support.

The **Create PPTX** page imports PNG and JPEG images directly from a selected folder, in natural filename order, into a separate export folder. It also uses the 50-slide limit and leaves the source files unchanged. It never trims: every screenshot is embedded exactly as captured, and trimming is a separate pass over a finished deck (see below). Cropping a deck changes only the image embedded in the PPTX, never the original screenshots.

### Trimming a deck that already exists

The same page has a second mode: **An existing PPTX**. Choose a finished `.pptx` (yours, or one this tool produced earlier) and the application writes a new deck whose embedded screenshots have their edges trimmed. The source file is only read. The new deck lands in `<output folder>/exports/<timestamp>-<unique-id>/` under the name shown in the filename field (default: `<source name>-cropped.pptx`), so an earlier export is never overwritten.

The trim only runs when **Trim the screenshot edges in this PPTX** is ticked. That option appears on this page alone, because it is the only place where a PPTX is both the input and the output; the primary button stays disabled until it is ticked, so the page can never produce an unchanged copy by accident.

The trim reads each picture instead of assuming a fixed aspect ratio. It searches from the centre of the picture outward and from the outer edges inward: it finds the light presentation canvas in the middle, follows colored title and footer bands, photos, and other content that touches the canvas, and snaps each final boundary at full resolution to the strongest visible transition. Dark letterbox bars and floating meeting-window controls beside the slide are trimmed, and a dark neutral toolbar counts as chrome even though its icons look like detail. The deck's screenshots are also compared with each other, so a band that stays identical across slides — a participant strip, a meeting toolbar, a taskbar — is removed while any changing slide content is kept; a colored band or a flat bright slide line stops that trim, and a trim that would leave no changing content is reverted. A dark deck inside dark chrome is followed out to its slide boundary on the strength of that comparison, which is what keeps a meeting window from leaving a large black area around a navy slide. A result under 15% of the picture, or with an aspect ratio outside 0.4–4.0 (an extreme strip), is rejected, so a toolbar row of an application window is not mistaken for a slide.

Only pictures that cover at least 35% of a slide are treated as screenshots, so logos, icons, and decorative images stay untouched; a picture that already carries a crop rectangle, or is rotated or flipped, is also left alone. When detection is uncertain the original picture is kept. A deck where nothing qualifies is reported as such instead of writing an identical copy.

Two placements are available for the trimmed result:

- **Fill the original area** (default) — the trimmed screenshot is enlarged to the space it occupied. A picture that was full-bleed is grown to the whole page, which removes the margin the letterbox bars used to occupy.
- **Keep the original scale** — only the edges are shaved off. No pixel is resampled, so the slide gains a margin where the bars were.

Page size, slide order, notes, and every other package part are preserved byte for byte; only the cropped media and the picture offsets and extents of the slides that were trimmed change. The slides that could not be trimmed keep their original picture, and the final status line reports how many slides were trimmed out of the total.

## Capture settings

The **Settings** page decides what counts as a new slide. The defaults suit an ordinary PowerPoint show; hovering a control shows the same explanation in the app.

- **Sample interval** (200 ms) — how often the display is read. A lower value reacts faster and costs more CPU.
- **Stable frames** (2) — consecutive identical samples required before a page is saved. A transition animation keeps this counter at zero, so only the settled picture is captured.
- **Animation timeout** (10 s) — the longest wait for a picture to come to rest. After it, a frame is captured anyway, so a looping video on a slide cannot block it forever.
- **Change threshold** (5%) — share of pixels that must differ before the frame counts as a new slide.
- **Black threshold** (80%) — how dark a frame must be to be treated as black or protected content.
- **Filter duplicates** (on) — ignores pointer movement and local video motion when comparing with the last saved page.

## Appearance

The **Appearance** page sets how the window looks; every change applies as you click and is remembered in `ppt-auto-capture-config.json`, which the app reads and writes in its working directory (the folder it was launched from).

- **Light / Dark / Match system** — the palette covers the workspace, sidebar, cards, banners, and controls, so recorded screenshots are the only thing that stays untouched. *Match system* re-checks the operating system's appearance while the window is open.
- **Background picture** — choose a PNG, JPEG, BMP, or WebP file and it is scaled to fill the workspace behind the cards. The cards themselves stay opaque, so text keeps its contrast. The picture is downscaled to at most 2560 px on its longest side before it reaches the GPU, and the file is only read: moving it later just brings back the solid background with a note on the page.
- **Dimming** — fades the picture behind the interface. Raise it when the picture is busy or bright; the preview above the slider shows the result, including the fade.

Both palettes are checked by a unit test that measures WCAG contrast for every text role the panels combine, so a colour change that makes text hard to read fails before release.

## macOS troubleshooting and diagnostics

Application logs are stored at `~/Library/Logs/PPT Auto Capture`; on Windows they are written to `logs/` in the application's working directory. Either way the app retains the newest ten application logs, and **Open log folder** takes you to them. On the Dashboard, choose **Copy diagnostics** to put a concise report on the clipboard. The report includes application, OS, architecture, capture-state, approved selected source/display descriptions, and local output/log paths so a support report has useful context.

Diagnostics exclude screenshots and slide contents: they do not include screenshot pixels, window or slide contents, existing clipboard data, arbitrary environment variables, or serialized configuration. If persistent logging cannot be initialized, capture remains available and diagnostics continue through the in-app history and standard error output.

## Windows long recordings, videos, and locked sessions

Windows capture releases its screen/window DC, memory DC, and bitmap on every success and failure path. It deselects the bitmap before reading pixels and checks the full scan-line count. A resource-allocation or pixel-read failure reports an error rather than passing incomplete pixels to the slide detector. The application logs GDI object counts and working-set memory once a minute, along with saved-page and pending-save state. Rust panic information is written to the launch log.

Replacing or dropping a capture worker also stops its background thread. Per-frame UI sampling notifications are limited when the UI is slow; saved-slide, error, and state notifications remain reliable.

With duplicate filtering enabled, small pointer changes are ignored. A bounded spatial signature learns repeatedly moving local areas (up to 40% of the picture), allowing a video rectangle to change without repeatedly saving the same page. Changes elsewhere still trigger a new page. Motion exclusions expire after 15 quiet samples so later static content in the same area remains eligible; this can also save a video's settled final frame. Continuous local motion never forces a save on the rapid-transition path: there too, only a frame that has come to rest is written. Broad rapid slide changes and intermediate pages held for more than one sample keep their capture path. This is an image heuristic: full-screen video, very briefly displayed same-template pages, and pages differing only by tiny details cannot be reliably classified as slide identities. Turn off duplicate filtering when every small animation step must be recorded.

Transition animations are skipped. A frame reaches the deck only after the picture has been still for the stability window, so morph, push, wipe, and fade frames are never saved; a page that is replaced before it settles is treated as part of the transition instead of being captured. A fade that passes through black works the same way: the first visible frame after black is a half-drawn slide, so it waits for the settle window like any other change. Stopping inside that window still keeps the page already on screen. A transition longer than the animation timeout is the one exception, because the timeout forces a capture.

When Windows locks, switches to a secure desktop, or disconnects an RDP session, capture waits without saving that desktop's images. When the original desktop becomes available, it retries the selected source automatically. It preserves pending saved work and respects a manual pause or stop. A selected window stays a window source after resume; it is not silently replaced with a monitor screenshot. Ordinary desktop screenshot APIs cannot reliably capture the original PowerPoint display while it is locked. If uninterrupted capture during a host lock is mandatory, use a separate presentation/capture session that remains unlocked; changing the screenshot API alone does not remove this Windows restriction.

For release acceptance on Windows, run one capture for at least 24 hours at the normal sample interval, including static pages, local video, pointer movement, repeated lock/unlock, and RDP disconnect/reconnect if used. Inspect the launch log for growing GDI counts or memory, verify each intended slide in the output, and verify Stop still completes. Automated tests push long frame sequences through the worker and check that per-frame UI notifications stay bounded, and a Windows-native test repeats the capture allocation and failure paths while measuring GDI handle counts; these do not substitute for that hardware-specific soak test.

Windows CI also pressure-tests real window capture, cleanup and reinitialization. The manually dispatched **Windows native stress** workflow produces JSON resource reports and a standalone stress kit with ten-minute and 24-hour launchers. See [Windows stress testing](docs/testing/windows-stress.md) for running the kit without Rust, report thresholds and the distinction between controlled-window stress and real meeting-app acceptance. The UI event mailbox is bounded during state/error storms; screenshot previews work before recording, retain their actual aspect ratio and reuse their GPU texture. Stop interrupts the sample wait and shows a saving status; the Dashboard can open the current recording's folder directly.

The folder and file choosers are PowerShell helpers, and the application itself owns no console as a GUI binary. Windows would therefore allocate one for every helper and flash a black window over the desktop, so the helpers are started with `CREATE_NO_WINDOW`. The chosen path comes back as the hex of its UTF-8 bytes rather than as text: hex is ASCII under any console code page, which keeps paths such as `C:\资料\幻灯片.pptx` intact without the helper having to touch console APIs it has no console for.

## Building from Source

### Prerequisites

- Rust 1.81 or newer (`rustup` recommended; CI builds with current stable)
- Windows SDK (on Windows)
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
  2.14.1 \
  dist/ppt-auto-capture-gui-macos-apple-silicon.dmg
```

The third argument is the version stamped into the DMG, so pass the version you built.

The app is ad-hoc signed, not Developer ID signed or notarized.

### Check the Windows build from macOS or Linux

The released Windows binary is built natively on `windows-latest` for `x86_64-pc-windows-msvc`; cross-linking a full `.exe` from another operating system is not supported. To type-check the Windows-only code paths (DXGI/GDI, window messages, Restart Manager) from another platform:

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc
```

`cargo check` does not link, so it needs no Windows toolchain. It is worth running: macOS and Linux skip those branches entirely, so a mistake there would otherwise surface only in CI.

### Releasing

Every release needs a written changelog. GitHub's generated notes are not used: this repository commits straight to `main`, so they would carry nothing but a comparison link.

1. Bump `version` in `Cargo.toml` (and let `cargo build` update `Cargo.lock`).
2. Add a `## [x.y.z] - YYYY-MM-DD` section to [CHANGELOG.md](CHANGELOG.md), under `### Added`, `### Changed`, and `### Fixed`. This repository writes those entries in Chinese; only the heading format is fixed, because `scripts/release-notes.sh` extracts the section by it.
3. Commit, push `main`, then `git tag -a vX.Y.Z` and push the tag.

The release workflow turns that section into the GitHub release body, so the notes on the releases page state what actually changed. A tag with no section stops at preflight instead of shipping empty notes; `bash scripts/release-notes.sh vX.Y.Z` prints what would be published.

CI gates every push to `main` and every tag: the full test suite, a rustfmt check limited to changed files, and a clippy warning baseline, on Linux. The baseline is the reason a warning the local macOS build never shows can still fail the build.

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

On macOS the capture step is `xcap` (CoreGraphics) instead of DXGI/GDI; everything after it — detection, storage, PPTX writing — is the same code. The **Create PPTX** and **Appearance** pages sit beside the panels in the GUI box.

## License

MIT
