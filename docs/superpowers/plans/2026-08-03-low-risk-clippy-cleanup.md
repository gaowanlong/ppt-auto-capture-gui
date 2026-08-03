# Low-risk Clippy Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the repository's mechanically fixable Rust and Clippy warnings without changing capture, PPTX, packaging, or public-interface behavior, then lower the Linux CI warning ceiling.

**Architecture:** Treat structured Clippy diagnostics as the failing contract for each lint category. Apply only local, semantics-preserving rewrites covered by the existing unit and workflow suites; leave dead code and interface-design lints for separate work.

**Tech Stack:** Rust 2021, Cargo, Clippy JSON diagnostics, Bash regression tests, Ruby workflow contracts, GitHub Actions.

## Global Constraints

- Do not delete or globally suppress `dead_code`.
- Do not change public APIs merely to satisfy a lint.
- Do not restructure functions flagged for argument count or large error types.
- Do not change capture selection, image processing, PPTX generation, or platform behavior.
- Do not broaden existing `allow` attributes.
- `.clippy-warning-baseline` may only decrease from `110`.
- Preserve the untracked `dist/` directory.

---

### Task 1: Platform and capture-worker cleanup

**Files:**
- Modify: `src/capture/capture_worker.rs`

**Interfaces:**
- Consumes: existing `WorkerCommand::TestCapture`, `PptCaptureWorker::save_frame`, and `crate::model::Region` behavior.
- Produces: identical capture behavior without unused imports, unused test-capture dimensions, or needless reference creation.

- [ ] **Step 1: Reproduce the targeted diagnostics**

Run:

```bash
cargo clippy --all-targets --all-features 2>&1 | rg 'unused import: `Region`|window_client_w|window_client_h|save_frame\(&ref_frame\)'
```

Expected: diagnostics identify the unused `Region` import, the unused variables in the `TestCapture` branch, and the needless borrow passed to `save_frame`.

- [ ] **Step 2: Apply the minimal local rewrite**

Change the import to:

```rust
use crate::model::{Frame, MonitorInfo};
```

Delete only the second `window_client_w` and `window_client_h` declarations in `WorkerCommand::TestCapture`; retain the earlier pair used by the normal capture initialization path. Change:

```rust
self.save_frame(&ref_frame)
```

to:

```rust
self.save_frame(ref_frame)
```

- [ ] **Step 3: Verify focused behavior and diagnostics**

Run:

```bash
cargo test capture::capture_worker::tests --all-features
cargo clippy --all-targets --all-features 2>&1 | rg 'unused import: `Region`|window_client_w|window_client_h|save_frame\(&ref_frame\)' && exit 1 || true
```

Expected: capture-worker tests pass and the targeted diagnostics are absent.

- [ ] **Step 4: Commit the independent cleanup**

```bash
git add src/capture/capture_worker.rs
git commit -m "refactor: remove capture worker lint noise"
```

### Task 2: Semantics-preserving production rewrites

**Files:**
- Modify: `src/config.rs`
- Modify: `src/i18n.rs`
- Modify: `src/detection/change_detector.rs`
- Modify: `src/detection/stability_detector.rs`
- Modify: `src/detection/duplicate_detector.rs`
- Modify: `src/model/frame.rs`
- Modify: `src/storage/image_store.rs`

**Interfaces:**
- Consumes: existing output-directory resolution, `Language::default`, detector downsampling, hashing, thumbnail allocation, and image storage behavior.
- Produces: identical values using idiomatic constructs recognized by Clippy.

- [ ] **Step 1: Reproduce the selected production lints**

Run:

```bash
cargo clippy --all-targets --all-features 2>&1 | rg 'needless_return|derivable_impls|manual_clamp|needless_borrows_for_generic_args|unnecessary_cast'
```

Expected: the selected lint categories are present.

- [ ] **Step 2: Replace expressions with equivalent idiomatic forms**

Apply these exact transformations:

```rust
// src/config.rs, macOS cfg block
resolve_output_dir(Path::new(configured), home.as_deref(), true)
    .to_string_lossy()
    .into_owned()

// src/i18n.rs
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Language {
    Chinese,
    #[default]
    English,
}

// both detector downsampling expressions
((total as f64 / target as f64).sqrt().ceil() as u32).clamp(1, 8)

// duplicate detector hash input
hasher.update([
    frame.data[offset],
    frame.data[offset + 1],
    frame.data[offset + 2],
]);

// frame thumbnail allocation
let mut thumb = vec![0u8; new_w * new_h * 4];

// image-store capacity
let mut rgb_data = Vec::with_capacity(
    frame.width as usize * frame.height as usize * 3,
);
```

Remove the manual `impl Default for Language` after adding the derive.

- [ ] **Step 3: Run focused behavioral tests**

Run:

```bash
cargo test config::tests --all-features
cargo test i18n::tests --all-features
cargo test detection:: --all-features
cargo test model::frame::tests --all-features
cargo test storage::image_store::tests --all-features
```

Expected: all focused tests pass.

- [ ] **Step 4: Confirm selected production lints are absent**

Run:

```bash
cargo clippy --bin ppt-auto-capture-gui --all-features 2>&1 | rg 'needless_return|derivable_impls|manual_clamp|needless_borrows_for_generic_args|unnecessary_cast' && exit 1 || true
```

Expected: no selected diagnostic remains in the non-test binary.

- [ ] **Step 5: Commit production rewrites**

```bash
git add src/config.rs src/i18n.rs src/detection/change_detector.rs src/detection/stability_detector.rs src/detection/duplicate_detector.rs src/model/frame.rs src/storage/image_store.rs
git commit -m "refactor: apply low-risk Clippy suggestions"
```

### Task 3: Test-code lint cleanup

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`
- Modify: `src/storage/recovery.rs`
- Modify: `src/detection/black_frame_detector.rs`
- Modify: `src/detection/change_detector.rs`
- Modify: `src/detection/stability_detector.rs`
- Modify: `src/detection/duplicate_detector.rs`
- Modify: `src/model/frame.rs`
- Modify: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Consumes: existing test fixtures and assertions.
- Produces: unchanged test coverage with cleaner fixture construction and module ordering.

- [ ] **Step 1: Reproduce test-only diagnostics**

Run:

```bash
cargo clippy --tests --all-features 2>&1 | rg 'unused import: `std::fs`|unused_mut|items_after_test_module|bool_assert_comparison|needless_range_loop|useless_vec|join\(&format'
```

Expected: all listed test-only patterns are reported.

- [ ] **Step 2: Apply literal test rewrites**

Make the following changes:

```rust
// src/config.rs
assert!(cfg.keep_previous);

// black-frame fixtures
for byte in data.iter_mut().skip(half) {
    *byte = 255;
}

// frame fixture
for (i, byte) in data.iter_mut().enumerate() {
    *byte = (i % 256) as u8;
}

// duplicate fixture
let f = make_frame(&[128u8; 100], 5, 5);

// PPTX fixture path
slides_dir.join(format!("slide_{:04}.png", i))
```

Remove `mut` from immutable `black_data` and `data` fixtures. Remove the unused `use std::fs;` in recovery tests. Move `platform_tests` in `src/main.rs` below `setup_cjk_fonts` so no item follows a test module.

- [ ] **Step 3: Verify all tests and test-only diagnostics**

Run:

```bash
cargo test --all-targets --all-features
cargo clippy --tests --all-features 2>&1 | rg 'unused import: `std::fs`|unused_mut|items_after_test_module|bool_assert_comparison|needless_range_loop|useless_vec|join\(&format' && exit 1 || true
```

Expected: 106 tests pass and the selected test-only patterns are absent.

- [ ] **Step 4: Commit test cleanup**

```bash
git add src/main.rs src/config.rs src/storage/recovery.rs src/detection/black_frame_detector.rs src/detection/change_detector.rs src/detection/stability_detector.rs src/detection/duplicate_detector.rs src/model/frame.rs src/pptx/pptx_writer.rs
git commit -m "test: remove mechanical Clippy warnings"
```

### Task 4: Recalibrate and verify the warning ceiling

**Files:**
- Modify: `.clippy-warning-baseline`

**Interfaces:**
- Consumes: `scripts/check-clippy-baseline.sh` structured Cargo JSON counter.
- Produces: a lower Linux warning ceiling enforced by CI and release preflight.

- [ ] **Step 1: Measure the fresh local result**

Run:

```bash
bash scripts/check-clippy-baseline.sh
```

Expected: `observed` is below `110` and the script recommends reducing the baseline.

- [ ] **Step 2: Set an initial lower baseline**

Replace `.clippy-warning-baseline` with the measured local count plus the existing Linux/macOS platform delta of five warnings. Do not use a value of `110` or higher.

- [ ] **Step 3: Run the complete local verification suite**

Run:

```bash
cargo test --all-targets --all-features
for test_script in tests/*.sh; do bash "$test_script"; done
for ruby_test in tests/test_*.rb; do ruby "$ruby_test"; done
bash scripts/check-rustfmt-changed.sh
bash scripts/check-clippy-baseline.sh
git diff --check
```

Expected: every command exits zero, DMG end-to-end validation passes on Apple Silicon, and observed Clippy warnings do not exceed the new baseline.

- [ ] **Step 4: Commit the lower ceiling**

```bash
git add .clippy-warning-baseline
git commit -m "ci: lower Clippy warning baseline"
```

- [ ] **Step 5: Push and calibrate against authoritative Linux CI**

Run:

```bash
git push origin main
CI_RUN_ID="$(gh run list --workflow CI --branch main --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run watch "$CI_RUN_ID" --interval 10 --exit-status
```

Expected: all four jobs pass. If Linux reports a lower count, reduce the baseline and rerun; if it reports a slightly higher platform-specific count below `110`, set the baseline to that observed Linux count, rerun local contracts, push, and require the replacement CI run to pass.

- [ ] **Step 6: Confirm clean synchronization without touching user artifacts**

Run:

```bash
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

Expected: local and remote `main` SHAs match; only the pre-existing untracked `dist/` directory remains.
