# Clippy API-design cleanup

## Goal

Remove the remaining non-`dead_code` Clippy warnings without changing capture behavior, macOS behavior, generated PPTX bytes, user-facing errors, or packaging. This batch is limited to API-expression improvements that make invalid or confusing call patterns harder to write.

## Scope

The batch covers these warning classes:

- `clippy::new_ret_no_self` in PPTX XML generators;
- `clippy::inherent_to_string` in the content-types XML generator;
- `clippy::too_many_arguments` in `SlideRecord` construction;
- `clippy::result_large_err` in capture worker-command dispatch.

It does not remove or suppress `dead_code`, change generated XML, alter slide detection, modify capture defaults, or add macOS features. Unused macOS scaffolding remains available for a separate evidence-based audit.

## API changes

### XML generators

Functions in `src/pptx/slide_xml.rs` that return XML text but are named `new` will be renamed to `render`. All internal call sites will use the new names. Their inputs, templates, escaping behavior, relationship identifiers, and output text remain unchanged.

### Content types

`ContentTypesXml` will implement `std::fmt::Display` instead of defining an inherent `to_string` method. Existing consumers may continue to call the standard `.to_string()` supplied by `Display`. A regression test will assert exact output equality so the trait conversion cannot alter whitespace, ordering, or escaping.

### Slide records

The nine positional arguments accepted by `SlideRecord::new` will be replaced by a `SlideRecordInput` parameter object containing the same named values. This preserves the current constructor workflow while preventing argument-order mistakes. No stored field, default, timestamp, geometry value, filename, or serialization representation changes.

### Worker command dispatch

The capture-start path currently uses `Option::map` only for the side effect of sending `WorkerCommand::Start`. That expression temporarily carries `SendError<WorkerCommand>`, whose `WorkerCommand::Start` variant contains a large capture source. It will be replaced by an explicit conditional send whose result is discarded at the same point, matching the existing behavior while preventing the large error value from becoming the closure's result.

## Data and compatibility invariants

- Generated PPTX XML must remain byte-for-byte identical for the same input.
- `SlideRecord` fields must receive exactly the same values as before.
- Worker-command delivery behavior must remain unchanged: send when a command channel exists and ignore a disconnected-channel result.
- Capture, slide-change detection, output paths, and macOS permission handling must not change.
- No new `allow` attributes may hide these warnings.
- The existing `dead_code` warnings are outside this batch and may not be deleted opportunistically.

## Test strategy

Implementation follows red-green-refactor for each warning class:

1. Add or strengthen exact-output tests for XML generators and content types.
2. Add a named-field mapping test for `SlideRecord` construction.
3. Protect worker-command dispatch with the existing application/capture tests and structured Clippy verification; the rewrite changes no observable result because the current send error is already discarded.
4. Confirm each focused test fails for the intended missing API or invariant before changing production code.
5. Apply one minimal API change at a time and rerun the focused and full suites.

Final verification includes:

- `cargo test --all-targets --all-features`;
- every repository shell and Ruby contract test;
- changed-file rustfmt validation;
- structured Clippy counting, with all non-`dead_code` warnings removed;
- Apple Silicon DMG creation, mounting, and application-bundle validation;
- GitHub CI builds for Apple Silicon macOS, Linux, and Windows.

The Linux warning count is authoritative for the committed ceiling. The baseline may be reduced only after CI supplies the exact platform count.

## Delivery

Changes will be made on an isolated `codex/` branch, reviewed as a behavior-preserving diff, merged into `main`, and pushed. The task is complete only after the resulting GitHub Actions run is green with no check annotations. The user's untracked `dist/` directory remains untouched.

## Success criteria

- No `new_ret_no_self`, `inherent_to_string`, `too_many_arguments`, or `result_large_err` diagnostics remain.
- PPTX output and worker-command behavior are unchanged.
- All local validation and Apple Silicon DMG checks pass.
- The Linux Clippy ceiling is lower than 76.
- GitHub CI passes on every supported target.
