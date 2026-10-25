# Windows stability testing

The stress kit runs actual Win32 window capture against a controlled test window. It records resource metrics and logs, never desktop screenshots. Keep the test window open and Windows unlocked during this test. This is separate from lock/unlock functional testing in the application.

## Running without Rust

Download `windows-stress-kit` from the **Windows native stress** GitHub Actions run and extract all files together. The kit contains the test executable and `run-windows-stress.ps1`; it does not install a driver or modify Windows lock/power settings. In PowerShell, from the extracted directory:

For a simple start, double-click `run-windows-stress-10min.cmd` or `run-windows-stress-24h.cmd`. The console stays open after the run, and uniquely named report directories appear under `stress-results` next to the launcher. The following commands are for choosing custom settings:

```powershell
./run-windows-stress.ps1 -Minutes 10 -Resolution 1080p -OutputDirectory ./stress-results
```

For a real 24-hour local test at the usual 200 ms sampling rate:

```powershell
./run-windows-stress.ps1 -Minutes 1440 -Resolution 1080p -IntervalMilliseconds 200 -OutputDirectory ./stress-results
```

The computer must remain awake for the requested duration. Run scripts only in an environment where your organization's execution policy permits them; the kit does not change that policy.

## Running from source

The same script uses Cargo automatically when the bundled executable is absent. A Rust toolchain with Windows build tools is required. GitHub's manually dispatched **Windows native stress** workflow runs 1–60 minutes and uploads reports and a reusable kit. The ordinary Windows CI suite additionally runs a short native stress test on every change.

## What is exercised

- Real `GdiCapturer` / `PrintWindow` frame acquisition, complete buffers and repeated reinitialization.
- Invalid-window and early-return cleanup, and repeated creation/drop of capture worker threads.
- GDI objects, USER objects, process handles, working-set memory and elapsed wall-clock time.
- Resources sampled after warmup and once per minute. Reports have bounded size for a 24-hour test.

The native test fails for fewer than ten frames, frame errors, GDI/USER growth exceeding eight objects, process-handle growth exceeding 32 handles, or working-set growth exceeding 256 MiB after warmup. Memory allowances accommodate allocator caching and are not a promise of zero memory growth. Review the time series for ongoing trends even when the final thresholds pass.

Each run gets a unique directory containing `native-report.json`, `runner-result.json`, and `stress.log`. An early build/setup failure can produce only the runner result and log. `requested_seconds` is the requested test duration; `elapsed_seconds` measures actual native-loop time and excludes compilation and warmup. Synthetic frame-count tests are not counted as elapsed native runtime.

## Application acceptance

Use a test presentation to check: test screenshot before recording starts; wide/portrait previews; window and full-screen sources; start/pause/resume/stop repeatedly; opening the generated PPTX; output locked by another process and successful retry; window closing; lock/unlock recovery; video and pointer changes; and restart recovery. The automated mailbox storm, preview, failed-start, configuration, save-retry and storage recovery tests cover their noninteractive boundaries. A passing controlled-window stress run does not by itself prove a particular meeting application's 24-hour stability.
