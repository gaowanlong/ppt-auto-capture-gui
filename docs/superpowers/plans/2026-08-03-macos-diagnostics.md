# macOS Capture Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add bounded persistent logs and an in-app diagnostic history/report so macOS capture and PPTX failures can be reported without Terminal.

**Architecture:** Create a focused `diagnostics` module for event history, deterministic report rendering, directory resolution, retention, and a dual stderr/file writer. Initialize it before the GUI, pass startup logging state into the application, translate worker errors into history entries, and let the Dashboard request copy/open actions while the application performs those side effects.

**Tech Stack:** Rust 2021, `log`, `env_logger`, `chrono`, egui/eframe, existing platform open commands, Cargo tests, Bash/Ruby CI contracts, Apple Silicon DMG tooling.

## Global Constraints

- Keep at most 20 in-memory diagnostic entries and 10 matching persistent log files.
- macOS logs live in `~/Library/Logs/PPT Auto Capture`; Windows and other platforms use a local `logs` directory.
- Persistent logging failure must never prevent GUI startup or capture.
- Do not log screenshot pixels, clipboard contents, window contents, PPT slide text, arbitrary environment variables, or serialized configuration.
- Preserve existing capture states, error text, macOS permission guidance, PPTX bytes, and output paths.
- Do not add telemetry, automatic upload, crash collection, or ZIP export.
- Preserve the user's untracked `dist/` directory.

---

## File map

- Create `src/diagnostics.rs`: diagnostic event/history types, report rendering, log directory resolution, retention, tee writer, and logger startup result.
- Modify `src/main.rs`: register the module, initialize logging before eframe, log startup metadata, and pass logging state to the app.
- Modify `src/app.rs`: own diagnostic history, translate runtime failures, build/copy reports, and open the log directory.
- Modify `src/gui/dashboard.rs`: render recent diagnostics and expose copy/open button requests.
- Modify `src/i18n.rs`: English and Chinese labels/status text for diagnostics.
- Modify `.clippy-warning-baseline` only if authoritative Linux CI changes the count.

### Task 1: Bounded diagnostic history and deterministic report

**Files:**
- Create: `src/diagnostics.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: `DiagnosticLevel::{Info, Warning, Error}`.
- Produces: `DiagnosticEntry::new(level, category, message) -> DiagnosticEntry`.
- Produces: `DiagnosticHistory::new(capacity)`, `push`, and `entries_newest_first`.
- Produces: `DiagnosticReportContext<'a>` and `render_diagnostic_report(context, history) -> String`.

- [ ] **Step 1: Add failing bounded-history and report tests**

Create the module and tests that refer to the not-yet-implemented production types:

```rust
#[test]
fn history_evicts_the_oldest_entry_at_capacity() {
    let mut history = DiagnosticHistory::new(20);
    for index in 0..21 {
        history.push(DiagnosticEntry::new(
            DiagnosticLevel::Error,
            "capture",
            format!("failure-{index}"),
        ));
    }
    let messages: Vec<_> = history
        .entries_newest_first()
        .map(|entry| entry.message.as_str())
        .collect();
    assert_eq!(messages.len(), 20);
    assert_eq!(messages.first(), Some(&"failure-20"));
    assert!(!messages.contains(&"failure-0"));
}

#[test]
fn report_contains_runtime_context_without_private_payloads() {
    let mut history = DiagnosticHistory::new(20);
    history.push(DiagnosticEntry::new(
        DiagnosticLevel::Error,
        "pptx",
        "First frame save failed",
    ));
    let report = render_diagnostic_report(
        DiagnosticReportContext {
            version: "1.1.0",
            os: "macos",
            architecture: "aarch64",
            capture_state: "Error",
            source: "Full Screen",
            display: "Built-in Display",
            output_path: "/tmp/capture.pptx",
            log_directory: Some("/Users/test/Library/Logs/PPT Auto Capture"),
        },
        &history,
    );
    for expected in [
        "Version: 1.1.0",
        "Platform: macos/aarch64",
        "State: Error",
        "Source: Full Screen",
        "Display: Built-in Display",
        "Output: /tmp/capture.pptx",
        "First frame save failed",
    ] {
        assert!(report.contains(expected), "missing {expected}");
    }
    for forbidden in ["pixel bytes", "clipboard contents", "ppt-auto-capture-config.json"] {
        assert!(!report.contains(forbidden));
    }
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test diagnostics::tests --all-features`

Expected: compilation fails because the diagnostic types and rendering functions do not exist.

- [ ] **Step 3: Implement the minimal model and report renderer**

Use a `VecDeque<DiagnosticEntry>`, clamp capacity to at least one, timestamp entries with `chrono::Local::now()`, pop the front before pushing when full, and render a fixed-order plain-text report. Expose entry fields needed by the Dashboard as read-only public fields. Register `mod diagnostics;` in `src/main.rs`.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test diagnostics::tests --all-features`

Expected: both tests pass and the newest-first iterator returns exactly 20 entries.

- [ ] **Step 5: Commit**

```bash
git add src/diagnostics.rs src/main.rs
git commit -m "feat: add bounded diagnostic history"
```

### Task 2: Persistent dual-output logging and bounded retention

**Files:**
- Modify: `src/diagnostics.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: `LoggingSetup { pub log_directory: Option<PathBuf>, pub log_file: Option<PathBuf>, pub warning: Option<String> }`.
- Produces: `resolve_log_directory(home: Option<&Path>, is_macos: bool, current_dir: &Path) -> PathBuf`.
- Produces: `retain_recent_logs(directory: &Path, keep: usize) -> io::Result<()>`.
- Produces: `initialize_logging() -> LoggingSetup`.

- [ ] **Step 1: Add failing path, retention, and writer-degradation tests**

Use unique directories below `std::env::temp_dir()` and delete them through a small test guard on drop. Tests must assert:

```rust
assert_eq!(
    resolve_log_directory(Some(Path::new("/Users/test")), true, Path::new("/work")),
    Path::new("/Users/test/Library/Logs/PPT Auto Capture")
);
```

Create 12 files named `ppt-auto-capture-20260803-1200NN.log`, one `notes.txt`, and one child directory; set deterministic modification times by creating them in filename order and make retention sort by filename when metadata times tie. After `retain_recent_logs(dir, 10)`, assert the newest ten matching names remain and both unrelated entries remain.

Add a writer test using a deliberately invalid log target whose parent is a regular file. Assert the setup factory returns a warning and `None` file output rather than an error.

- [ ] **Step 2: Verify RED**

Run: `cargo test diagnostics::tests --all-features`

Expected: compilation fails for the missing path, retention, and logging setup APIs.

- [ ] **Step 3: Implement resolution and safe retention**

Only match regular files whose names start with `ppt-auto-capture-` and end with `.log`. Sort newest first by modification time and then filename; remove only matches beyond `keep`. Treat unreadable individual directory entries as skipped, never as permission to delete a broader path.

- [ ] **Step 4: Implement the tee writer and startup setup**

Implement `std::io::Write` for a writer that always writes to `stderr` and optionally writes to a `BufWriter<File>`, flushing the file after complete formatted records. Configure `env_logger::Builder` with `Target::Pipe(Box::new(writer))`, millisecond timestamps, and the existing default filter `info`. Use `try_init`; record setup failures in `LoggingSetup.warning` instead of panicking.

In `main`, replace the old logger builder with:

```rust
let logging = diagnostics::initialize_logging();
info!(
    "PPT Auto Capture {} starting on {}/{}",
    env!("CARGO_PKG_VERSION"),
    std::env::consts::OS,
    std::env::consts::ARCH
);
```

- [ ] **Step 5: Verify GREEN and startup fallback**

Run:

```bash
cargo test diagnostics::tests --all-features
cargo test platform_tests --all-features
```

Expected: retention/path/fallback tests pass; platform tests still pass; tests do not contend for the global logger because they exercise the setup factory/writer rather than calling `try_init` repeatedly.

- [ ] **Step 6: Commit**

```bash
git add src/diagnostics.rs src/main.rs
git commit -m "feat: persist bounded application logs"
```

### Task 3: Runtime error history and diagnostic report actions

**Files:**
- Modify: `src/app.rs`
- Modify: `src/diagnostics.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `LoggingSetup`, `DiagnosticHistory`, and `render_diagnostic_report` from Tasks 1-2.
- Produces: `PptAutoCaptureApp::new(logging: LoggingSetup) -> Self`.
- Produces: a private `record_error(&mut self, category: &str, message: String)` helper.
- Produces: private `copy_diagnostics(&mut self, ctx: &egui::Context)` and `open_log_directory(&mut self)` helpers.

- [ ] **Step 1: Add a failing application-level characterization test**

Extract worker-error handling into a helper that can be tested without launching a window. Add a test that initializes an empty Dashboard and history, applies two worker error strings, and asserts:

```rust
assert_eq!(dashboard.last_error.as_deref(), Some("second error"));
let messages: Vec<_> = history
    .entries_newest_first()
    .map(|entry| entry.message.as_str())
    .collect();
assert_eq!(messages, ["second error", "first error"]);
```

This protects both the existing prominent-last-error behavior and the new non-destructive history.

- [ ] **Step 2: Verify RED**

Run: `cargo test app::tests::worker_errors_keep_history_and_latest_dashboard_error --all-features`

Expected: compilation fails because the worker-error helper and diagnostic fields are absent.

- [ ] **Step 3: Add application diagnostic state and event translation**

Pass `LoggingSetup` from the eframe creation closure into `PptAutoCaptureApp::new(logging)`. Add `diagnostic_history`, `log_directory`, and `log_file` fields. Convert `logging.warning` into an initial warning entry. Route every `WorkerEvent::Error(msg)` through the tested helper, while retaining `dashboard.last_error = Some(msg)` semantics.

Log capture state transitions and selected source/display descriptions through `log::info!`; use paths rather than image data for successful PNG/PPTX messages.

- [ ] **Step 4: Implement deterministic copy and folder-open actions**

Build `DiagnosticReportContext` from application state and call `ctx.copy_text(report)`. Open the directory with `open` on macOS, `explorer` on Windows, and `xdg-open` on Linux. If process launch fails, call `record_error("diagnostics", ...)`; never alter capture state.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test app::tests --all-features
cargo test diagnostics::tests --all-features
cargo test capture::capture_worker::tests --all-features
```

Expected: all focused suites pass and existing worker behavior remains unchanged.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/diagnostics.rs src/main.rs
git commit -m "feat: retain runtime diagnostic errors"
```

### Task 4: Dashboard diagnostics section and localization

**Files:**
- Modify: `src/gui/dashboard.rs`
- Modify: `src/i18n.rs`
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `DiagnosticHistory` from Task 1.
- Produces: `DashboardPanel.copy_diagnostics_requested: bool`.
- Produces: `DashboardPanel.open_log_directory_requested: bool`.
- Produces: localized labels `t_diagnostics`, `t_copy_diagnostics`, `t_open_log_folder`, `t_diagnostics_copied`, and `t_persistent_logging_unavailable`.

- [ ] **Step 1: Add failing localization and request-state tests**

Extend the i18n non-empty coverage and add exact assertions that English and Chinese diagnostics labels differ. Add a Dashboard constructor test asserting both request flags start false. Keep UI button behavior thin: clicking only sets a request flag; the application performs side effects.

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test i18n::tests --all-features
cargo test gui::dashboard::tests --all-features
```

Expected: compilation fails because the labels and request flags do not exist.

- [ ] **Step 3: Implement the compact diagnostics UI**

Update `DashboardPanel::render` to accept `&DiagnosticHistory` and `persistent_log_available: bool`. Render at most the five newest entries to avoid crowding the Dashboard, while the history still retains 20. Show severity, timestamp, category, and message. Add copy and open buttons; disable the open button when no directory exists.

- [ ] **Step 4: Wire requests in the application**

Immediately after Dashboard rendering, consume and clear each flag. Copy through the egui context and set a localized transient status string. Open the folder through the Task 3 helper. Failed actions append diagnostic entries and remain visible on the next frame.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test i18n::tests --all-features
cargo test gui::dashboard::tests --all-features
cargo test app::tests --all-features
```

Expected: all focused tests pass; no test launches Finder, Explorer, or `xdg-open` because side-effect helpers are not invoked by constructor/request-state tests.

- [ ] **Step 6: Commit**

```bash
git add src/gui/dashboard.rs src/i18n.rs src/app.rs
git commit -m "feat: expose diagnostics in dashboard"
```

### Task 5: Privacy boundaries, documentation, and full delivery verification

**Files:**
- Modify: `README.md`
- Modify: `.clippy-warning-baseline` only if Linux CI supplies a different authoritative count.

**Interfaces:**
- Preserves: all capture/PPTX/macOS behavior and packaging contracts.
- Documents: log location, retention, report contents, and privacy exclusions.

- [ ] **Step 1: Add a failing README contract test**

Extend `tests/test_release_workflow.sh` to require the macOS log path, ten-file retention statement, `Copy diagnostics`, and an explicit statement that screenshots and slide contents are not logged.

- [ ] **Step 2: Verify RED**

Run: `bash tests/test_release_workflow.sh`

Expected: failure because the README lacks the diagnostics section.

- [ ] **Step 3: Document tester workflow and privacy boundary**

Add a concise macOS troubleshooting section explaining where logs live, how to copy the report, what is retained, what is excluded, and that failure to create the log directory does not block capture.

- [ ] **Step 4: Verify GREEN and run local quality gates**

Run:

```bash
cargo test --all-targets --all-features
for test_script in tests/*.sh; do bash "$test_script"; done
for ruby_test in tests/test_*.rb; do ruby "$ruby_test"; done
CHANGED_RUST_FILES="$(git diff --name-only main -- '*.rs')" bash scripts/check-rustfmt-changed.sh
cargo clippy --all-targets --all-features --message-format=json > /tmp/ppt-capture-diagnostics-clippy.json
CLIPPY_OUTPUT_FILE=/tmp/ppt-capture-diagnostics-clippy.json bash scripts/check-clippy-baseline.sh
git diff --check
```

Expected: all tests pass, changed Rust files are formatted, every Clippy diagnostic is `dead_code`, and the observed count does not exceed the committed baseline.

- [ ] **Step 5: Build and validate the Apple Silicon DMG**

Run:

```bash
cargo build --release --target aarch64-apple-darwin
mkdir -p artifacts
bash scripts/package-macos-dmg.sh \
  target/aarch64-apple-darwin/release/ppt-auto-capture-gui \
  1.1.0 \
  artifacts/ppt-auto-capture-gui-macos-apple-silicon.dmg
file target/aarch64-apple-darwin/release/ppt-auto-capture-gui
```

Expected: the executable is an arm64 Mach-O; DMG creation, ad-hoc signature validation, mounting, and mounted-app validation all pass.

- [ ] **Step 6: Review, commit, merge, and push**

Review the complete diff against the design, then:

```bash
git add README.md tests/test_release_workflow.sh .clippy-warning-baseline
git commit -m "docs: document macOS diagnostics"
```

After merged-result verification, fast-forward `main`, push it, and monitor the triggered GitHub Actions run until Quality Gates and all three platform builds pass. Inspect the Quality Gates log for the authoritative Linux warning count and check annotations. If Linux is below the current baseline, lower it in a follow-up commit and monitor the replacement run. Remove only the task-created `artifacts/` directory and isolated worktree; leave `dist/` untouched.
