# macOS capture diagnostics

## Goal

Make the next macOS manual-test cycle self-diagnosing. A tester should be able to see recent failures, find the persistent log, and copy a concise diagnostic report without using Terminal. The feature must remain useful on Windows and Linux, but macOS capture and PPTX-generation failures are the primary target.

## Scope

This batch adds a lightweight diagnostic center:

- persistent per-launch application logs with bounded retention;
- an in-memory history of recent diagnostic events;
- Dashboard actions to copy a diagnostic report and open the log directory;
- richer capture and save context in logs while retaining the existing user-facing error messages.

It does not add ZIP export, automatic upload, telemetry, crash reporting, screenshot capture in logs, or collection of PPT content.

## Architecture

### Diagnostic paths and retention

A small diagnostics module resolves a writable application diagnostics directory. On macOS it uses `~/Library/Logs/PPT Auto Capture`; on Windows it uses a `logs` directory beside the current configuration location; on other platforms it uses `logs` under the current working directory. If the preferred directory cannot be created, logging continues on stderr and the UI reports that persistent logging is unavailable.

Each launch creates `ppt-auto-capture-YYYYMMDD-HHMMSS.log`. Startup removes only matching application log files beyond the newest ten. Unrelated files and directories are never removed.

### Persistent logging

The existing `log` calls remain the common logging API. Logger initialization writes the same formatted records to stderr and, when available, to the current launch file. Each line includes a millisecond timestamp, level, target, and message. The file is flushed frequently enough that a capture failure remains available after an abnormal exit.

The logs include application version, OS, architecture, capture source description, selected display description, state changes, PNG save paths, PPTX generation milestones, and complete error chains. They never include image bytes, clipboard data, window contents, or PPT slide text.

### Recent diagnostic events

The application owns a bounded `DiagnosticHistory` containing at most 20 entries. Each entry has a timestamp, severity, category, and user-readable message. Worker errors, capture state errors, persistent-logger initialization failures, and important save failures enter the history. Oldest entries are evicted first.

The current `DashboardPanel.last_error` behavior remains for prominent display and start-button recovery. The history supplements it, so a later error no longer erases the evidence needed for testing.

### Dashboard controls

The Dashboard adds a compact Diagnostics section below the current status:

- recent events, newest first;
- `Copy diagnostics`, which copies a plain-text report to the clipboard;
- `Open log folder`, enabled only when a persistent log directory is available.

The report contains application version, OS, architecture, current capture state, selected source and display descriptions, output path, log path availability, and recent diagnostic events. It does not embed the saved configuration or enumerate arbitrary environment variables.

Labels and status messages are localized in English and Chinese through the existing i18n module.

## Data flow

1. Application startup resolves the diagnostics directory, applies retention, and initializes stderr plus file logging.
2. Startup metadata is written to the persistent logger.
3. Capture worker operations continue to emit `log` records and `WorkerEvent` values.
4. The application translates relevant worker events into bounded diagnostic-history entries while preserving existing state and error handling.
5. Dashboard rendering reads the history and current application metadata.
6. Copying diagnostics builds a deterministic plain-text report and places it on the egui clipboard.
7. Opening the log folder uses the existing platform-specific `open`, `explorer`, or `xdg-open` approach and records launch failures in the history.

## Error handling

- Failure to create or open a log file never prevents the GUI from starting.
- Retention ignores individual unreadable entries and logs the failure when a logger is already available.
- Clipboard and folder-opening failures produce visible diagnostic entries without changing capture state.
- macOS Screen Recording errors retain the existing bilingual, actionable permission guidance and also enter the persistent log and recent history.
- Save errors preserve their complete causal chain in the log; the Dashboard continues showing a concise actionable message.

## Testing

Implementation follows red-green-refactor and adds platform-independent tests for:

- retention keeps the ten newest matching logs and preserves unrelated files;
- bounded history evicts the oldest entry after 20 items;
- diagnostic reports contain version, platform, architecture, state, source, display, output path, log availability, and recent errors;
- diagnostic reports do not contain image bytes, clipboard contents, arbitrary environment variables, or configuration serialization;
- logger-directory failure degrades to stderr without preventing initialization;
- Worker errors enter history while preserving `DashboardPanel.last_error` behavior;
- macOS permission errors remain actionable.

Final verification includes all Rust tests, repository Shell and Ruby contract tests, changed-file rustfmt validation, structured Clippy verification, Apple Silicon release compilation and DMG mount/signature checks, and GitHub CI builds for macOS Apple Silicon, Linux, and Windows.

## Delivery and success criteria

Work is performed on an isolated `codex/` branch and merged into `main` after review and verification. The user's untracked `dist/` directory remains untouched.

The feature is complete when:

- macOS testers can locate persistent logs without Terminal;
- the Dashboard retains and displays the last 20 diagnostic events;
- copied diagnostics contain enough runtime context to reproduce a report without screenshots;
- logging failure cannot block application startup or capture;
- no screenshot pixels or PPT contents are written to diagnostics;
- all local and GitHub checks pass on every supported target.
