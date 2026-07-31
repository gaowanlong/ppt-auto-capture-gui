# PPTX Office Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate PowerPoint-compatible PPTX packages that open without repair prompts and protect the package graph, view properties, theme, master, presentation defaults, slides, and media from regression.

**Architecture:** Keep the existing ZIP-based `PptxWriter` and dynamic slide/image generation. Replace invalid or incomplete static OOXML templates with PowerPoint-compatible baseline XML, extend dynamic presentation relationships for every support part, and validate the generated package through real ZIP/XML integration tests.

**Tech Stack:** Rust, `zip`, existing PPTX XML builders, `quick-xml` for test-side XML parsing if already available; otherwise the existing XML parser dependency used by the project.

## Global Constraints

- Do not change screen capture behavior, image encoding, output paths, or naming.
- Keep one captured PNG per slide.
- Preserve current page-ratio and image-fit behavior.
- Do not add a third-party PPTX generation library.
- Do not commit the user's original or repaired PPTX files.
- Complete with `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and the complete test suite.
- Merge the completed branch into the repository's main branch and push GitHub.

---

### Task 1: Complete Presentation-Level Relationships

**Files:**
- Modify: `src/pptx/slide_xml.rs:171-194`
- Test: `src/pptx/slide_xml.rs`
- Test: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Consumes: `PresentationRelsXml::new(slides: &[(u32, String)]) -> String`
- Produces: presentation relationship XML with unique IDs for the master, every slide, presentation properties, view properties, theme, and table styles.

- [ ] **Step 1: Write failing unit tests**

Add tests that parse the relationship XML and compare literal relationship types and targets:

```rust
#[test]
fn presentation_relationships_include_support_parts() {
    let xml = PresentationRelsXml::new(&[(1, "image1.png".into())]);
    for expected in [
        "relationships/presProps\" Target=\"presProps.xml\"",
        "relationships/viewProps\" Target=\"viewProps.xml\"",
        "relationships/theme\" Target=\"theme/theme1.xml\"",
        "relationships/tableStyles\" Target=\"tableStyles.xml\"",
    ] {
        assert!(xml.contains(expected), "missing {expected}");
    }
}
```

Add a table-driven test for 0, 1, 3, and 100 slides that extracts every `Id` attribute into a set and asserts the count equals the number of relationships.

Also add a package integration test through the real `PptxWriter` that requires the four support-part relationships. This test must be run before the relationship implementation so it proves that the generated package—not only the XML builder—exposes the repaired contract.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test presentation_relationships_include_support_parts
cargo test presentation_relationship_ids_are_unique
cargo test generated_pptx_has_required_presentation_relationships
```

Expected: the unit and package support-part tests fail because none of the four relationships exist. The uniqueness test must be written so the old fixed slide numbering plus the expected new support relationships cannot pass accidentally.

- [ ] **Step 3: Implement dynamic support relationships**

After generating slide relationships, allocate the next four numeric relationship IDs from:

```rust
let next_id = slides.iter().map(|(num, _)| num + 1).max().unwrap_or(1) + 1;
```

Generate the four required relationships with consecutive unique IDs. Keep `rId1` for the slide master and the existing slide relationship IDs.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
cargo test presentation_relationships
```

Expected: all relationship unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/pptx/slide_xml.rs src/pptx/pptx_writer.rs
git commit -m "fix: complete pptx presentation relationships"
```

### Task 2: Correct View Properties

**Files:**
- Modify: `src/pptx/slide_xml.rs:310-313`
- Test: `src/pptx/slide_xml.rs`

**Interfaces:**
- Consumes: `VIEW_PROPS_XML`
- Produces: schema-compatible normal-view restoration sizes.

- [ ] **Step 1: Write a failing test**

```rust
#[test]
fn view_properties_use_size_attributes() {
    assert!(VIEW_PROPS_XML.contains("<p:restoredLeft sz=\"15611\"/>"));
    assert!(VIEW_PROPS_XML.contains("<p:restoredTop sz=\"94660\"/>"));
    assert!(!VIEW_PROPS_XML.contains(" cx="));
    assert!(!VIEW_PROPS_XML.contains(" cy="));
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test view_properties_use_size_attributes
```

Expected: FAIL because the old template uses `cx` and `cy`.

- [ ] **Step 3: Apply the minimal schema fix**

Replace the two nodes with:

```xml
<p:restoredLeft sz="15611"/>
<p:restoredTop sz="94660"/>
```

- [ ] **Step 4: Run the test and verify GREEN**

Run:

```bash
cargo test view_properties_use_size_attributes
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pptx/slide_xml.rs
git commit -m "fix: emit valid pptx view properties"
```

### Task 3: Replace Incomplete OOXML Baseline Templates

**Files:**
- Modify: `src/pptx/slide_xml.rs:150-168`
- Modify: `src/pptx/slide_xml.rs:205-313`
- Test: `src/pptx/slide_xml.rs`
- Test: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Consumes: `PresentationXml::new`, `SLIDE_MASTER_XML`, `THEME_XML`
- Produces: PowerPoint-compatible presentation defaults, slide-master text styles, and complete theme elements while retaining dynamic page dimensions.

- [ ] **Step 1: Write failing compatibility tests**

Add focused tests for these observable package contracts:

```rust
#[test]
fn theme_has_complete_font_collections() {
    for font in ["majorFont", "minorFont"] {
        let section = xml_section(THEME_XML, font);
        assert!(section.contains("<a:latin"));
        assert!(section.contains("<a:ea"));
        assert!(section.contains("<a:cs"));
    }
}
```

Add tests that count at least three fill styles, three line styles, three effect styles, and three background fill styles; assert the generated presentation contains `p:defaultTextStyle`; and assert the master contains `p:bg` and all three master text-style groups.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test theme_has_complete
cargo test presentation_has_default_text_style
cargo test slide_master_has_compatible_defaults
```

Expected: FAIL because the current templates omit these structures.

- [ ] **Step 3: Replace templates with the compatible baseline**

Copy only the structural XML from the repaired sample:

- keep the existing color scheme unless PowerPoint requires normalized values;
- add `ea` and `cs` fonts to both font collections;
- use complete three-entry style matrices;
- add master background and title/body/other text styles;
- add presentation default text style;
- preserve dynamic `sldSz` dimensions;
- emit `type="screen16x9"` only for the existing 16:9 page-ratio variant.

Do not copy document identifiers, timestamps, user metadata, media, or slide content.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
cargo test theme_has_complete
cargo test presentation_has_default_text_style
cargo test slide_master_has_compatible_defaults
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pptx/slide_xml.rs src/pptx/pptx_writer.rs
git commit -m "fix: use PowerPoint-compatible pptx templates"
```

### Task 4: Validate the Complete OPC Relationship Graph

**Files:**
- Modify: `Cargo.toml` only if an XML test dependency is required
- Modify: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Consumes: bytes produced by the real `PptxWriter`
- Produces: integration-test assertions that every internal relationship target exists and every relationship ID is unique within its `.rels` part.

- [ ] **Step 1: Write the package graph test**

Create a test helper that:

1. Generates a real PPTX in a temporary directory.
2. Reads every `.rels` part from the ZIP.
3. Parses each `Relationship` element.
4. Rejects duplicate IDs within the same relationship part.
5. Resolves non-external targets relative to the owner part.
6. Rejects traversal outside the package root.
7. Asserts the normalized target exists in the ZIP entry set.

Add:

```rust
#[test]
fn generated_pptx_has_closed_internal_relationship_graph() {
    let bytes = generate_test_pptx(3);
    assert_relationship_graph_is_closed(&bytes);
}
```

- [ ] **Step 2: Run the integration test and verify RED**

Run:

```bash
cargo test generated_pptx_has_closed_internal_relationship_graph
```

Expected: PASS after Task 1. To prove the graph validator protects a real break, temporarily change one generated internal target in the in-memory test ZIP to a nonexistent path, observe the validator fail with that path, then restore the fixture. The missing-required-relationship failure was already observed before Task 1.

- [ ] **Step 3: Complete only required helper plumbing**

Use the project's existing XML dependency where possible. Keep relationship resolution in the test module unless production code also needs it.

- [ ] **Step 4: Run the test and verify GREEN**

Run:

```bash
cargo test generated_pptx_has_closed_internal_relationship_graph
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/pptx/pptx_writer.rs
git commit -m "test: validate pptx relationship graph"
```

### Task 5: Protect Slides and Embedded Media

**Files:**
- Modify: `src/pptx/pptx_writer.rs`

**Interfaces:**
- Consumes: real PNG fixtures and generated PPTX bytes
- Produces: regression coverage for slide/media correspondence and byte preservation.

- [ ] **Step 1: Add multi-page and media tests**

Generate three distinct small PNG files and assert:

- three slide parts exist;
- three slide relationship parts exist;
- three media parts exist;
- each slide points to its own media and layout;
- media bytes exactly equal the input PNG bytes;
- no relationship ID is duplicated.

- [ ] **Step 2: Demonstrate the test protects a real break**

Run the new test against current production code. If it passes because existing behavior is already correct, temporarily mutate the test fixture's expected slide-to-media mapping locally and observe failure, then restore the correct expectation before committing. This is a characterization/regression guard, not justification for changing working production behavior.

- [ ] **Step 3: Run focused and module tests**

Run:

```bash
cargo test pptx_writer
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/pptx/pptx_writer.rs
git commit -m "test: protect pptx slides and media"
```

### Task 6: Full Verification, Merge, and Push

**Files:**
- Modify only files required by failures caused by this change.

**Interfaces:**
- Consumes: completed feature branch
- Produces: verified main branch and updated GitHub remote.

- [ ] **Step 1: Format and inspect the diff**

Run:

```bash
cargo fmt --all
git diff --check
git status --short
```

- [ ] **Step 2: Run complete verification**

Run:

```bash
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

All commands must exit zero.

- [ ] **Step 3: Generate and inspect a diagnostic PPTX**

Run the existing diagnostic generation test and inspect the resulting ZIP:

```bash
cargo test test_pptx_generate_to_tmp -- --nocapture
unzip -t /tmp/pptx_test_output.pptx
```

Confirm the emitted package contains all required parts and closed relationships.

- [ ] **Step 4: Commit remaining verified changes**

```bash
git add src/pptx docs/superpowers/plans/2026-07-31-pptx-office-compatibility.md Cargo.toml Cargo.lock
git commit -m "test: prevent PowerPoint repair regressions"
```

Skip this commit if there are no uncommitted tracked changes.

- [ ] **Step 5: Push the feature branch**

```bash
git push origin codex/macos-native-capture
```

- [ ] **Step 6: Merge into the main branch safely**

Resolve the actual main branch from `origin/HEAD`, fetch it, switch to it only when the working tree has no tracked changes, merge `codex/macos-native-capture` without rewriting history, and rerun the focused PPTX tests.

- [ ] **Step 7: Push the main branch**

```bash
git push origin <resolved-main-branch>
```

Report the feature commit, merge commit or fast-forward result, verification commands, and pushed branches.
