# Conservative dead-code cleanup design

## Goal

Remove only code that current data-flow and cross-platform reference analysis prove redundant. The cleanup must reduce the committed Clippy warning ceiling without changing capture behavior, UI output, PPTX generation, macOS behavior, packaging, or public platform contracts.

## Confirmed removals

This batch removes the following isolated items:

- `PptAutoCaptureApp.output_filename`, because all reads and writes use `OutputPanel.output_filename` and configuration is saved from the output panel;
- `DashboardPanel.source_window_hwnd` and `DashboardPanel.preview_thumbnail`, because neither field has a producer or consumer and test previews use `test_frame_rgba` instead;
- `OutputPanel::new`, because every construction uses `new_with_filename`;
- the empty `PreviewPanel` module, because preview rendering is implemented inside `DashboardPanel` and the module is never constructed;
- `PptxWriter::read_png_dimensions`, because PPTX image dimensions come from each `SlideRecord` and the helper has no caller;
- `DOC_PROPS_APP_XML_TEMPLATE_TEMPLATE`, because it is an unreferenced duplicate of the active document-properties template;
- `ImageStore.output_dir`, because construction derives and stores `slides_dir`, which is the only path used when saving images;
- the unused `make_frame` helper inside the `change_detector` test module.

No replacement behavior is introduced for these items. Constructors and outputs keep the same effective values.

## Explicit exclusions

The following warnings remain outside this batch:

- worker event fields and event variants, which require a separate decision between UI integration and protocol simplification;
- macOS, Windows, and unsupported-platform session-monitor APIs;
- unused translation functions that may belong to planned UI states;
- `atomic_file` crash-safety helpers;
- the duplicate-detection state split between `DuplicateDetector` and `WorkerLoop::last_slide_hash`;
- model validation and geometry helpers that are platform-specific or test-supported;
- any code whose only apparent redundancy depends on compiling for one host platform.

No `allow(dead_code)` attributes will be added or broadened.

## Data-flow invariants

- The output filename displayed, passed to the capture source, written to the dashboard, and persisted to configuration remains sourced from `OutputPanel.output_filename`.
- Dashboard status, test preview rendering, saved-slide counts, and error display remain unchanged.
- `ImageStore::new(output_dir)` still creates `output_dir/slides`; `save_png` writes the same RGB PNG path and performs the same temporary-file rename.
- PPTX XML, media paths, image dimensions, relationship identifiers, and ZIP contents remain unchanged.
- Platform module exports and capture backend APIs remain unchanged.

## Test strategy

Implementation follows red-green-refactor where a consumer-visible invariant exists:

1. Add an `OutputPanel` characterization test asserting that `new_with_filename` retains the supplied filename; the existing `Drop` implementation remains the configuration-persistence consumer of that field.
2. Strengthen `ImageStore` tests to assert that constructing a store creates the `slides` directory and that saving a frame returns the expected slide path.
3. Remove one isolated group at a time and rerun its focused tests.
4. Use structured Clippy JSON as the failing and passing contract for purely structural removals that have no observable behavior.

Final verification includes:

- `cargo test --all-targets --all-features`;
- all repository shell and Ruby contract tests;
- rustfmt validation for every changed Rust file;
- structured Clippy classification to prove only approved warnings disappeared and no new warning class appeared;
- compilation for Apple Silicon macOS, Linux, and Windows in GitHub CI;
- creation, mounting, architecture inspection, Info.plist inspection, and ad-hoc signature verification of a real Apple Silicon DMG.

The Linux warning count reported by GitHub CI is authoritative. `.clippy-warning-baseline` may only decrease from `64`.

## Delivery

Work will be performed on an isolated `codex/` branch, reviewed as a semantic diff, fast-forwarded into `main`, and pushed. Completion requires a green GitHub Actions run with zero annotations. The user's untracked `dist/` directory remains untouched.

## Success criteria

- Every confirmed-removal item is absent and has no remaining reference.
- Excluded APIs and warning categories are unchanged.
- Application output-path behavior, image storage, PPTX output, and macOS packaging remain covered and passing.
- The Linux Clippy ceiling is lower than `64`.
- All supported CI targets and quality gates pass with zero annotations.
