# macOS Apple Silicon Manual Test Checklist

Test the packaged `PPT Auto Capture.app` from the mounted DMG on an Apple Silicon Mac. Record the macOS version, app commit, PowerPoint version, and output files for each run.

## Installation and Permission

- [ ] Mount `ppt-auto-capture-gui-macos-apple-silicon.dmg`; confirm it contains `PPT Auto Capture.app` and an `Applications` shortcut.
- [ ] Drag the app to Applications and open it through Finder. If Gatekeeper blocks it, right-click the app, choose **Open**, and confirm.
- [ ] With Screen Recording disabled for the app, refresh windows and try **Test Screenshot**. Confirm the app shows an actionable English/Chinese permission error referring to **System Settings → Privacy & Security → Screen Recording** and restart.
- [ ] Enable Screen Recording for `PPT Auto Capture`, quit the app completely, and reopen it. Confirm the display and window lists refresh successfully.

## Display Capture

- [ ] Select **Full Screen**, select the intended display, and click **Test Screenshot**. Confirm the preview is that display, including correct colors and orientation.
- [ ] Start continuous capture, advance through at least three visually distinct slides, pause, wait, resume, advance once more, and stop.
- [ ] Confirm no images are saved while paused and capture resumes from the same selected display.
- [ ] Confirm `slides/*.png`, `manifest.jsonl`, and the configured PPTX are created and contain the expected slide sequence.
- [ ] With capture running on an external display, disconnect that display. Confirm capture enters an error state and does not silently capture another display.

## Window Capture

- [ ] Open a PowerPoint slide show, refresh windows, and confirm the PowerPoint window appears near the top with the presentation title.
- [ ] Select the PowerPoint window and click **Test Screenshot**. Confirm the preview contains only the selected window rather than the entire display.
- [ ] Start continuous capture and advance through at least three slides. Confirm saved PNG dimensions match the selected window and colors are correct.
- [ ] Pause, resume, and stop. Confirm state transitions work and no frames are saved while paused.
- [ ] Close the selected PowerPoint window while capture is running. Confirm capture enters an error state and does not switch to another window.
- [ ] Confirm **Move to Display** and **Maximize** are disabled and their hover text explains the macOS Accessibility limitation.

## PPTX and Recovery

- [ ] Open the generated PPTX in the installed macOS Microsoft PowerPoint. Confirm it opens without a repair prompt or deleted-content warning.
- [ ] Verify every captured slide renders, has the configured aspect ratio, and preserves its screenshot without color-channel swapping.
- [ ] Start a new capture, save at least two slides, then force-quit the app. Relaunch it, accept recovery, and confirm the rebuilt PPTX opens without repair.

## Packaging Evidence

Run:

```bash
file "/Applications/PPT Auto Capture.app/Contents/MacOS/ppt-auto-capture-gui"
codesign --verify --deep --strict --verbose=2 "/Applications/PPT Auto Capture.app"
plutil -extract NSScreenCaptureUsageDescription raw \
  "/Applications/PPT Auto Capture.app/Contents/Info.plist"
```

Expected: the executable reports `arm64`, ad-hoc signature verification succeeds, and the usage description explains screen/window capture.
