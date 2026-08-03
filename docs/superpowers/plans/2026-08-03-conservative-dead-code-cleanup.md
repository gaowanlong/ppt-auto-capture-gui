# Conservative Dead-code Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove eight proven-redundant code groups without changing UI state, output paths, image storage, PPTX output, platform APIs, or packaging.

**Architecture:** Use structured Clippy diagnostics as the failing contract for structural removals and add consumer-visible characterization tests before deleting redundant storage. Work in small groups whose consumers live together, then verify the retained warning set and calibrate the Linux ceiling from CI.

**Tech Stack:** Rust 2021, Cargo, Clippy JSON diagnostics, existing Rust tests, Bash and Ruby contract tests, Apple Silicon DMG tooling, GitHub Actions.

## Global Constraints

- Remove only the eight items listed in the approved design.
- Do not change worker event fields or variants, translation APIs, `atomic_file`, duplicate-detection state, session-monitor APIs, or platform-specific model helpers.
- Do not add or broaden `allow(dead_code)` attributes.
- Preserve capture behavior, UI output, PPTX bytes, image paths, macOS behavior, and packaging.
- `.clippy-warning-baseline` may only decrease from `64`; Linux CI supplies the authoritative value.
- Preserve the user's untracked `dist/` directory.

## File map

- `src/app.rs`: application state and output filename flow.
- `src/gui/dashboard.rs`: dashboard state actually rendered by the UI.
- `src/gui/output_panel.rs`: output settings construction and filename ownership.
- `src/gui/mod.rs`, `src/gui/preview_panel.rs`: GUI module declarations and obsolete empty preview shell.
- `src/storage/image_store.rs`: derived slides directory and PNG persistence.
- `src/pptx/pptx_writer.rs`, `src/pptx/slide_xml.rs`: active PPTX generation code and unused duplicates.
- `src/detection/change_detector.rs`: production detector and its test-local helpers.
- `.clippy-warning-baseline`: Linux warning ceiling.

---

### Task 1: Protect filename and image-path behavior

**Files:**
- Modify: `src/gui/output_panel.rs`
- Modify: `src/storage/image_store.rs`

**Interfaces:**
- Consumes: `OutputPanel::new_with_filename(&str) -> OutputPanel`.
- Consumes: `ImageStore::new(PathBuf) -> Result<ImageStore>` and `save_png(&Frame, u32) -> Result<PathBuf>`.
- Produces: characterization tests that protect the consumers of the fields removed in Tasks 2 and 3.

- [ ] **Step 1: Add the output filename ownership test**

Append this test module to `src/gui/output_panel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_filename_is_owned_by_the_output_panel() {
        let panel = OutputPanel::new_with_filename("configured.pptx");
        assert_eq!(panel.output_filename, "configured.pptx");
    }
}
```

The mutation caught is a constructor that ignores or replaces the configured filename. This test intentionally passes before removal; the existing Clippy diagnostic is the RED contract for the redundant application field itself.

- [ ] **Step 2: Add the real image-save path test**

Add to `src/storage/image_store.rs`:

```rust
#[test]
fn saves_png_under_the_derived_slides_directory() {
    let temp = tempfile::tempdir().unwrap();
    let store = ImageStore::new(temp.path().to_path_buf()).unwrap();
    let frame = Frame::new(vec![0, 0, 255, 255], 1, 1, 4, 0, 0);

    let saved = store.save_png(&frame, 3).unwrap();

    assert_eq!(saved, temp.path().join("slides/slide_0003.png"));
    assert!(saved.is_file());
}
```

The mutations caught are deriving the wrong storage directory, constructing the wrong filename, or failing to persist the PNG.

- [ ] **Step 3: Run characterization tests**

Run:

```bash
cargo test gui::output_panel::tests::configured_filename_is_owned_by_the_output_panel --all-features
cargo test storage::image_store::tests::saves_png_under_the_derived_slides_directory --all-features
```

Expected: both pass on the existing implementation, establishing behavior before structural deletion.

- [ ] **Step 4: Capture the structural RED diagnostics**

Run:

```bash
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null \
  | rg 'output_filename.*never read|output_dir.*never read'
```

Expected: diagnostics identify `PptAutoCaptureApp.output_filename` and `ImageStore.output_dir` as unread fields.

- [ ] **Step 5: Commit the characterization tests**

```bash
git add src/gui/output_panel.rs src/storage/image_store.rs
git commit -m "test: protect output and image storage paths"
```

### Task 2: Remove redundant application and dashboard state

**Files:**
- Modify: `src/app.rs`
- Modify: `src/gui/dashboard.rs`
- Modify: `src/gui/output_panel.rs`
- Modify: `src/gui/mod.rs`
- Delete: `src/gui/preview_panel.rs`

**Interfaces:**
- Preserves: `OutputPanel::new_with_filename(&str)` as the only output-panel constructor.
- Preserves: Dashboard test preview through `test_frame_rgba`, `test_frame_w`, and `test_frame_h`.

- [ ] **Step 1: Delete the redundant application filename field**

Remove this field and its initialization from `PptAutoCaptureApp`:

```rust
output_filename: String,
```

```rust
output_filename: config.output_filename.clone(),
```

Do not change any `self.output_panel.output_filename` read or write.

- [ ] **Step 2: Delete unused dashboard fields**

Remove these declarations and their default initializers:

```rust
pub source_window_hwnd: u64,
pub preview_thumbnail: Option<egui::ColorImage>,
```

```rust
source_window_hwnd: 0,
preview_thumbnail: None,
```

- [ ] **Step 3: Delete the unused default constructor**

Remove only:

```rust
pub fn new() -> Self {
    Self::new_with_filename("output.pptx")
}
```

Keep `new_with_filename` unchanged.

- [ ] **Step 4: Delete the obsolete preview module**

Remove `pub mod preview_panel;` from `src/gui/mod.rs` and delete `src/gui/preview_panel.rs`. The active preview code in `DashboardPanel::render` remains unchanged.

- [ ] **Step 5: Verify focused behavior and diagnostics**

Run:

```bash
cargo test app::tests --all-features
cargo test gui::output_panel::tests --all-features
cargo test capture::capture_worker::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null >/tmp/ppt_capture_ui_cleanup.json
python3 - /tmp/ppt_capture_ui_cleanup.json <<'PY'
import json, sys
messages = []
for line in open(sys.argv[1]):
    record = json.loads(line)
    message = record.get("message", {})
    if message.get("level") == "warning":
        messages.append(message.get("message", ""))
joined = "\n".join(messages)
for removed in (
    "field `output_filename` is never read",
    "fields `source_window_hwnd` and `preview_thumbnail` are never read",
    "struct `PreviewPanel` is never constructed",
):
    assert removed not in joined, removed
PY
```

Expected: all focused tests and the diagnostic-removal assertions pass.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/gui/dashboard.rs src/gui/output_panel.rs src/gui/mod.rs src/gui/preview_panel.rs
git commit -m "refactor: remove redundant GUI state"
```

### Task 3: Remove unused storage and PPTX internals

**Files:**
- Modify: `src/storage/image_store.rs`
- Modify: `src/pptx/pptx_writer.rs`
- Modify: `src/pptx/slide_xml.rs`
- Modify: `src/detection/change_detector.rs`

**Interfaces:**
- Preserves: `ImageStore { slides_dir }` and its public constructor/save API.
- Preserves: active `DOC_PROPS_APP_XML_TEMPLATE` and all PPTX render APIs.

- [ ] **Step 1: Remove `ImageStore.output_dir`**

Change the type and constructor from two stored paths to one:

```rust
pub struct ImageStore {
    slides_dir: PathBuf,
}
```

```rust
Ok(Self { slides_dir })
```

- [ ] **Step 2: Remove the unused PNG dimension helper**

Delete the complete private `PptxWriter::read_png_dimensions` function. Do not modify the active dimension reads from `SlideRecord.width` and `SlideRecord.height`.

- [ ] **Step 3: Remove the duplicate document-properties constant**

Delete only `DOC_PROPS_APP_XML_TEMPLATE_TEMPLATE`. Keep the active `DOC_PROPS_APP_XML_TEMPLATE` literal and all references to it byte-for-byte unchanged.

- [ ] **Step 4: Remove the unused test helper**

Delete only the `make_frame(data: &[u8], w: u32, h: u32) -> Frame` helper in the `change_detector` test module. Do not alter the helpers or tests that are actually called.

- [ ] **Step 5: Verify focused behavior and diagnostics**

Run:

```bash
cargo test storage::image_store::tests --all-features
cargo test pptx::pptx_writer::tests --all-features
cargo test pptx::slide_xml::tests --all-features
cargo test detection::change_detector::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null >/tmp/ppt_capture_internal_cleanup.json
python3 - /tmp/ppt_capture_internal_cleanup.json <<'PY'
import json, sys
messages = []
for line in open(sys.argv[1]):
    record = json.loads(line)
    message = record.get("message", {})
    if message.get("level") == "warning":
        messages.append(message.get("message", ""))
joined = "\n".join(messages)
for removed in (
    "field `output_dir` is never read",
    "associated function `read_png_dimensions` is never used",
    "constant `DOC_PROPS_APP_XML_TEMPLATE_TEMPLATE` is never used",
    "function `make_frame` is never used",
):
    assert removed not in joined, removed
PY
```

Expected: focused suites pass, and structured diagnostics no longer mention `ImageStore.output_dir`, `read_png_dimensions`, `DOC_PROPS_APP_XML_TEMPLATE_TEMPLATE`, or the test `make_frame` helper.

- [ ] **Step 6: Commit**

```bash
git add src/storage/image_store.rs src/pptx/pptx_writer.rs src/pptx/slide_xml.rs src/detection/change_detector.rs
git commit -m "refactor: remove unused storage and PPTX internals"
```

### Task 4: Verify retained warnings and lower the ceiling

**Files:**
- Modify: `.clippy-warning-baseline`

**Interfaces:**
- Consumes: Cargo JSON warnings from `scripts/check-clippy-baseline.sh`.
- Produces: a Linux ceiling below `64` whose difference from the previous diagnostic set consists only of approved removals.

- [ ] **Step 1: Format every changed Rust file**

Run:

```bash
rustfmt --edition 2021 --config skip_children=true \
  src/app.rs \
  src/detection/change_detector.rs \
  src/gui/dashboard.rs \
  src/gui/mod.rs \
  src/gui/output_panel.rs \
  src/pptx/pptx_writer.rs \
  src/pptx/slide_xml.rs \
  src/storage/image_store.rs
```

- [ ] **Step 2: Classify the remaining local diagnostics**

Run `cargo clippy --all-targets --all-features --message-format=json >/tmp/ppt_capture_conservative_cleanup.json 2>/dev/null`, then enforce the approved diagnostic boundary:

```bash
python3 - /tmp/ppt_capture_conservative_cleanup.json <<'PY'
import json, sys
codes = []
messages = []
for line in open(sys.argv[1]):
    record = json.loads(line)
    message = record.get("message", {})
    if record.get("reason") == "compiler-message" and message.get("level") == "warning":
        codes.append((message.get("code") or {}).get("code"))
        messages.append(message.get("message", ""))
assert set(codes) <= {"dead_code"}, set(codes)
joined = "\n".join(messages)
for retained in (
    "variants `MonitorLost`, `Progress`, and `ProtectedContent` are never constructed",
    "function `t_protected_warning` is never used",
    "function `atomic_write` is never used",
    "methods `is_duplicate` and `update_last` are never used",
    "struct `SessionEventMonitor` is never constructed",
):
    assert retained in joined, retained
print(f"remaining warnings: {len(codes)}")
PY
```

Expected: every disappeared diagnostic belongs to the approved removal list; excluded worker events, translations, atomic helpers, duplicate state, session APIs, and model helpers remain represented; no non-`dead_code` warning appears.

- [ ] **Step 3: Set the provisional Linux baseline**

Set `.clippy-warning-baseline` to the local observed count plus the current Linux/macOS difference, while keeping it below `64`. GitHub CI supplies the final authoritative count.

- [ ] **Step 4: Run complete local verification**

Run:

```bash
cargo test --all-targets --all-features
for test_script in tests/*.sh; do bash "$test_script"; done
for test_script in tests/test_*.rb; do ruby "$test_script"; done
CHANGED_RUST_FILES="$(git diff --name-only origin/main..HEAD -- '*.rs')" \
  bash scripts/check-rustfmt-changed.sh
bash scripts/check-clippy-baseline.sh
cargo build --release --target aarch64-apple-darwin
bash scripts/package-macos-dmg.sh \
  target/aarch64-apple-darwin/release/ppt-auto-capture-gui \
  1.1.0 \
  artifacts/ppt-auto-capture-gui-macos-apple-silicon.dmg
git diff --check origin/main..HEAD
```

Expected: at least 109 Rust tests pass, every contract test passes, the real Apple Silicon DMG validates after mounting, and the diff has no whitespace errors.

- [ ] **Step 5: Commit the lower ceiling**

```bash
git add .clippy-warning-baseline
git commit -m "ci: lower baseline after dead-code cleanup"
```

- [ ] **Step 6: Review, merge, and verify GitHub**

Review the semantic diff and prove every excluded API is unchanged. Fast-forward the branch into `main`, rerun the complete suite on the merged tree, push `main`, and watch the CI run.

If Linux reports a different count, replace the provisional baseline with that exact value, rerun the baseline tests, commit, push, and wait for the replacement run. Complete only after all four checks report `success` with zero annotations. Remove the generated `artifacts/` directory and isolated worktree; preserve `dist/`.
