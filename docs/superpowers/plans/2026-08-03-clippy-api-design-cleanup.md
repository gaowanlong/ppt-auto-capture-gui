# Clippy API-design Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every non-`dead_code` Clippy diagnostic while preserving PPTX XML, slide metadata, capture command behavior, macOS behavior, and packaging.

**Architecture:** Treat each Clippy diagnostic class as a separate red-green unit. Introduce clearer APIs at the existing module boundaries, prove consumer-visible output with literal expectations, and use structured Clippy output as the regression contract for type-only warnings.

**Tech Stack:** Rust 2021, Cargo, Clippy JSON diagnostics, existing Rust unit/integration tests, Bash and Ruby contract tests, Apple Silicon DMG tooling, GitHub Actions.

## Global Constraints

- Generated PPTX XML must remain byte-for-byte identical for the same input.
- `SlideRecord` fields must receive exactly the same values as before.
- Worker-command delivery must still send when a channel exists and ignore a disconnected-channel result.
- Capture, slide detection, output paths, macOS permissions, and packaging must not change.
- Do not remove or suppress `dead_code` warnings in this batch.
- Do not add Clippy `allow` attributes.
- Preserve the user's untracked `dist/` directory.
- The Linux CI warning count is authoritative; `.clippy-warning-baseline` must finish below `76`.

## File map

- `src/pptx/slide_xml.rs`: render slide, presentation, and relationship XML.
- `src/pptx/content_types.rs`: render `[Content_Types].xml` through `Display`.
- `src/pptx/pptx_writer.rs`: consume the XML APIs and construct slide records in PPTX tests.
- `src/model/slide_record.rs`: define `SlideRecordInput` and map it into `SlideRecord`.
- `src/capture/capture_worker.rs`: construct production slide records.
- `src/app.rs`: dispatch the capture-start worker command without retaining a large send error.
- `.clippy-warning-baseline`: enforce the final Linux warning ceiling.

---

### Task 1: Output-oriented XML generator names

**Files:**
- Modify: `src/pptx/slide_xml.rs`
- Modify: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Produces: `SlideXml::render(...) -> (String, String)`
- Produces: `PresentationXml::render(&[(u32, String)], &str) -> String`
- Produces: `PresentationRelsXml::render(&[(u32, String)]) -> String`

- [ ] **Step 1: Add compile-time consumer tests for the new names**

Change the existing tests in `src/pptx/slide_xml.rs` to call `render` while retaining their literal XML assertions. Do not rename production methods yet. Representative changes:

```rust
let (xml, rels) = SlideXml::render(1, "image1", 1920, 1080, "fit", "16:9");
let presentation = PresentationXml::render(&[(1, "image1.png".into())], "16:9");
let rels = PresentationRelsXml::render(&[(1, "image1.png".into())]);
```

The production mutation caught is an XML generator that does not expose the approved output-oriented API. Existing literal assertions continue to catch changes to relationship IDs, dimensions, and required Office XML.

- [ ] **Step 2: Verify RED**

Run: `cargo test pptx::slide_xml::tests --all-features`

Expected: compilation fails because the three `render` associated functions do not exist.

- [ ] **Step 3: Rename production methods and call sites**

Rename the three methods from `new` to `render` without changing their bodies. Update all call sites in `src/pptx/pptx_writer.rs` and the remaining tests in `src/pptx/slide_xml.rs`.

- [ ] **Step 4: Verify GREEN and lint removal**

Run:

```bash
cargo test pptx::slide_xml::tests --all-features
cargo test pptx::pptx_writer::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null | rg 'new_ret_no_self'
```

Expected: both test commands pass; the final command exits `1` with no matches, proving all `new_ret_no_self` diagnostics are gone.

- [ ] **Step 5: Commit**

```bash
git add src/pptx/slide_xml.rs src/pptx/pptx_writer.rs
git commit -m "refactor: clarify PPTX XML render APIs"
```

### Task 2: Standard string rendering for content types

**Files:**
- Modify: `src/pptx/content_types.rs`

**Interfaces:**
- Produces: `impl std::fmt::Display for ContentTypesXml`
- Preserves: standard `ContentTypesXml::new(&slides).to_string() -> String`

- [ ] **Step 1: Add an exact `Display` output test**

Add this test using a hand-written expected string, independent of the production formatter:

```rust
#[test]
fn display_renders_exact_slide_override() {
    let rendered = format!("{}", ContentTypesXml::new(&[(7, "ignored.png".into())]));
    assert!(rendered.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types "));
    assert!(rendered.contains(
        "  <Override PartName=\"/ppt/slides/slide7.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n"
    ));
    assert!(rendered.ends_with("\n</Types>"));
}
```

The production mutation caught is a missing or incorrectly wired standard formatting boundary; the existing tests continue to cover multiple and empty slide lists.

- [ ] **Step 2: Verify RED**

Run: `cargo test pptx::content_types::tests::display_renders_exact_slide_override --all-features`

Expected: compilation fails because `ContentTypesXml` does not implement `Display`.

- [ ] **Step 3: Move rendering behind `Display::fmt`**

Import `std::fmt`, rename the current inherent `to_string` body to a private `render`, and expose it through the standard formatter:

```rust
fn render(&self) -> String {
    let mut entries = String::new();
    for (num, _) in &self.slides {
        entries.push_str("  <Override PartName=\"/ppt/slides/slide");
        entries.push_str(&num.to_string());
        entries.push_str(".xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
  <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
  <Default Extension=\"xml\" ContentType=\"application/xml\"/>\n\
  <Default Extension=\"png\" ContentType=\"image/png\"/>\n\
  <Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/>\n\
  <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/>\n\
  <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>\n\
  <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\n\
  <Override PartName=\"/ppt/presProps.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presProps+xml\"/>\n\
  <Override PartName=\"/ppt/tableStyles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml\"/>\n\
  <Override PartName=\"/ppt/viewProps.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml\"/>\n\
  <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\n\
  <Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\n\
{}\n\
</Types>",
        entries
    )
}

impl fmt::Display for ContentTypesXml {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}
```

Copy the existing method body exactly into `render`; do not change template bytes or entry ordering. Remove the old inherent `to_string`, allowing the standard `ToString` implementation supplied by `Display` to serve existing callers.

- [ ] **Step 4: Verify GREEN and lint removal**

Run:

```bash
cargo test pptx::content_types::tests --all-features
cargo test pptx::pptx_writer::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null | rg 'inherent_to_string'
```

Expected: tests pass and the lint search has no matches.

- [ ] **Step 5: Commit**

```bash
git add src/pptx/content_types.rs
git commit -m "refactor: use Display for content types XML"
```

### Task 3: Named slide-record construction

**Files:**
- Modify: `src/model/slide_record.rs`
- Modify: `src/capture/capture_worker.rs`
- Modify: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Produces: `pub struct SlideRecordInput` with the nine current caller-supplied fields.
- Produces: `pub fn SlideRecord::new(input: SlideRecordInput) -> SlideRecord`.

- [ ] **Step 1: Rewrite the model test against named input**

Change `test_slide_record_new` to construct the approved input type:

```rust
let r = SlideRecord::new(SlideRecordInput {
    slide_number: 1,
    png_filename: "slide_0001.png".into(),
    png_relative_path: "slides/slide_0001.png".into(),
    frame_index: 42,
    width: 1920,
    height: 1080,
    content_hash: "abc123".into(),
    source_name: "TestWindow".into(),
    monitor_name: "Monitor1".into(),
});
```

Keep the literal assertion for every field. The production mutations caught are omitted fields, swapped geometry, swapped source/monitor values, and wrong metadata mapping.

- [ ] **Step 2: Verify RED**

Run: `cargo test model::slide_record::tests::test_slide_record_new --all-features`

Expected: compilation fails because `SlideRecordInput` does not exist and `SlideRecord::new` still expects nine arguments.

- [ ] **Step 3: Introduce and map `SlideRecordInput`**

Add immediately before the `impl SlideRecord` block:

```rust
pub struct SlideRecordInput {
    pub slide_number: u32,
    pub png_filename: String,
    pub png_relative_path: String,
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub content_hash: String,
    pub source_name: String,
    pub monitor_name: String,
}
```

Change the constructor to `pub fn new(input: SlideRecordInput) -> Self` and copy each named input field into the matching record field. Continue generating `slide_id` with UUID v4 and `captured_at` with `Utc::now()`.

- [ ] **Step 4: Update all real call sites**

Import `SlideRecordInput` next to `SlideRecord` and convert every `SlideRecord::new(...)` in `src/capture/capture_worker.rs` and `src/pptx/pptx_writer.rs` to named construction. Use `rg -n 'SlideRecord::new' src` to prove no positional calls remain.

- [ ] **Step 5: Verify GREEN and lint removal**

Run:

```bash
cargo test model::slide_record::tests --all-features
cargo test capture::capture_worker::tests --all-features
cargo test pptx::pptx_writer::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null | rg 'too_many_arguments'
```

Expected: focused tests pass and the lint search has no matches.

- [ ] **Step 6: Commit**

```bash
git add src/model/slide_record.rs src/capture/capture_worker.rs src/pptx/pptx_writer.rs
git commit -m "refactor: construct slide records with named input"
```

### Task 4: Side-effect-only worker command dispatch

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `Option<Sender<WorkerCommand>>` already stored in `self.cmd_tx`.
- Preserves: one attempted `WorkerCommand::Start(source)` send when the channel exists; ignored send result.

- [ ] **Step 1: Capture the existing structured lint failure**

Run:

```bash
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null | rg 'result_large_err'
```

Expected: at least one `result_large_err` diagnostic associated with the `Option::map` closure in `start_capture`. This lint is the failing type-level contract; worker behavior is already covered by the application and capture-worker test modules.

- [ ] **Step 2: Replace the side-effect map with explicit dispatch**

Replace:

```rust
let _ = self.cmd_tx.as_ref().map(|tx| tx.send(WorkerCommand::Start(source)));
```

with:

```rust
if let Some(tx) = self.cmd_tx.as_ref() {
    let _ = tx.send(WorkerCommand::Start(source));
}
```

Do not change source construction, dashboard state, or error handling.

- [ ] **Step 3: Verify GREEN and lint removal**

Run:

```bash
cargo test app::tests --all-features
cargo test capture::capture_worker::tests --all-features
cargo clippy --all-targets --all-features --message-format=json 2>/dev/null | rg 'result_large_err'
```

Expected: focused tests pass and no `result_large_err` diagnostics remain.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "refactor: simplify worker command dispatch"
```

### Task 5: Recalibrate and fully verify the warning ceiling

**Files:**
- Modify: `.clippy-warning-baseline`

**Interfaces:**
- Consumes: structured Cargo JSON from `scripts/check-clippy-baseline.sh`.
- Produces: a Linux warning ceiling below `76`, containing only the retained `dead_code` diagnostics.

- [ ] **Step 1: Verify diagnostic composition locally**

Run `cargo clippy --all-targets --all-features --message-format=json >/tmp/ppt_capture_api_cleanup_clippy.json 2>/dev/null`, then parse warning codes:

```bash
python3 - /tmp/ppt_capture_api_cleanup_clippy.json <<'PY'
import collections, json, sys
codes = []
for line in open(sys.argv[1]):
    record = json.loads(line)
    message = record.get("message", {})
    if record.get("reason") == "compiler-message" and message.get("level") == "warning":
        codes.append((message.get("code") or {}).get("code"))
print(collections.Counter(codes))
assert set(codes) <= {"dead_code"}
PY
```

Expected: the assertion passes; locally observed diagnostics contain only `dead_code`.

- [ ] **Step 2: Set a provisional lower baseline**

Write the local observed warning count plus the currently measured Linux/macOS platform difference of seven to `.clippy-warning-baseline`, but never write `76` or higher. GitHub CI will provide the authoritative final count.

- [ ] **Step 3: Run complete local verification**

Run:

```bash
cargo test --all-targets --all-features
for test_script in tests/*.sh; do bash "$test_script"; done
for test_script in tests/test_*.rb; do ruby "$test_script"; done
bash scripts/check-rustfmt-changed.sh origin/main
bash scripts/check-clippy-baseline.sh
bash scripts/build-macos-dmg.sh
git diff --check origin/main..HEAD
```

Expected: 106 or more Rust tests pass, every contract test passes, only retained `dead_code` warnings are counted, the Apple Silicon DMG builds and validates, and the diff has no whitespace errors.

- [ ] **Step 4: Commit the lower ceiling**

```bash
git add .clippy-warning-baseline
git commit -m "ci: lower Clippy baseline after API cleanup"
```

- [ ] **Step 5: Review and deliver**

Review `git diff --stat origin/main..HEAD`, `git diff --check origin/main..HEAD`, and the semantic diff for every modified file. Merge the isolated branch into `main`, push `main`, and watch the new CI run with `gh run watch --exit-status`.

If Linux reports a different retained `dead_code` count, set the baseline to that exact count (still below `76`), rerun the baseline tests locally, commit, push, and wait for the replacement run. Confirm every check run concludes `success` with zero annotations before cleaning the worktree.
