# macOS Apple Silicon DMG Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the raw macOS release binary with an ad-hoc-signed Apple Silicon `.app` distributed in a validated DMG.

**Architecture:** A standalone Bash packager will assemble, sign, package, mount, and validate the application so the same release behavior can be exercised locally and in GitHub Actions. Shell integration tests will drive the packager through failure and success paths; the release workflow will call the tested script and publish its DMG.

**Tech Stack:** Bash 3.2-compatible shell, Rust `aarch64-apple-darwin`, macOS `codesign`, `hdiutil`, `plutil`, `ditto`, GitHub Actions.

## Global Constraints

- Support Apple Silicon (`arm64`) only.
- Do not require or claim Apple Developer ID signing or notarization.
- Sign the complete application bundle ad hoc with `codesign --sign -`.
- Publish `ppt-auto-capture-gui-macos-apple-silicon.dmg`.
- Keep Windows and Linux release behavior unchanged.
- The mounted DMG must contain `PPT Auto Capture.app` and an `Applications` symlink to `/Applications`.
- Gatekeeper first-launch instructions must be documented accurately.

---

### Task 1: Test and Implement the macOS Bundle/DMG Packager

**Files:**
- Create: `scripts/package-macos-dmg.sh`
- Create: `tests/test_macos_dmg_packaging.sh`

**Interfaces:**
- Consumes: `scripts/package-macos-dmg.sh <executable> <version> <output-dmg>`
- Produces: a compressed read-only DMG containing an arm64, ad-hoc-signed `PPT Auto Capture.app`.

- [ ] **Step 1: Write failure-path tests**

Create `tests/test_macos_dmg_packaging.sh` with a temporary-directory cleanup trap and assertions that these commands fail:

```bash
"$PACKAGER" "$FAKE_EXECUTABLE" "release-candidate" "$TMP_DIR/out.dmg"
"$PACKAGER" "$TMP_DIR/missing" "1.23" "$TMP_DIR/out.dmg"
"$PACKAGER" /bin/echo "1.23" "$TMP_DIR/out.dmg"
```

Each failure must assert a stable diagnostic substring: `invalid version`, `executable not found`, and `must be an arm64 Mach-O executable`.

- [ ] **Step 2: Run the tests to verify RED**

Run:

```bash
bash tests/test_macos_dmg_packaging.sh
```

Expected: FAIL because `scripts/package-macos-dmg.sh` does not exist.

- [ ] **Step 3: Implement validation and bundle assembly**

Create `scripts/package-macos-dmg.sh` using `set -euo pipefail`. It must:

```bash
VERSION_PATTERN='^[0-9]+([.][0-9]+)*$'
APP_NAME='PPT Auto Capture.app'
BUNDLE_ID='com.gaowanlong.ppt-auto-capture'
EXECUTABLE_NAME='ppt-auto-capture-gui'
```

Validate all three arguments, the version pattern, executable existence, and `file "$EXECUTABLE"` containing both `Mach-O` and `arm64`. Generate `Contents/Info.plist` through a literal plist template containing every key from the approved design, copy the executable with mode `755`, create `Contents/Resources`, and sign/verify the bundle.

- [ ] **Step 4: Extend the test with the success path**

When the host is macOS arm64 and `target/aarch64-apple-darwin/release/ppt-auto-capture-gui` exists, invoke the packager and assert:

```bash
test -f "$OUTPUT_DMG"
hdiutil attach -readonly -nobrowse -mountpoint "$MOUNT_DIR" "$OUTPUT_DMG"
test -d "$MOUNT_DIR/PPT Auto Capture.app"
test "$(readlink "$MOUNT_DIR/Applications")" = "/Applications"
plutil -extract CFBundleIdentifier raw "$MOUNT_DIR/PPT Auto Capture.app/Contents/Info.plist"
file "$MOUNT_DIR/PPT Auto Capture.app/Contents/MacOS/ppt-auto-capture-gui"
codesign --verify --deep --strict "$MOUNT_DIR/PPT Auto Capture.app"
```

The test must detach the mounted image through its cleanup trap.

- [ ] **Step 5: Complete DMG creation and internal validation**

The packager must stage the signed application, create the `/Applications` symlink, run:

```bash
hdiutil create -volname "PPT Auto Capture" \
  -srcfolder "$STAGING_DIR" -ov -format UDZO "$OUTPUT_DMG"
```

It must then mount the new image at a temporary mount point, independently repeat the structure, metadata, architecture, and signature checks, and detach it before returning success.

- [ ] **Step 6: Build the Apple Silicon binary and verify GREEN**

Run:

```bash
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
bash tests/test_macos_dmg_packaging.sh
```

Expected: all negative cases and the complete mountable-DMG success case PASS.

- [ ] **Step 7: Commit**

```bash
git add scripts/package-macos-dmg.sh tests/test_macos_dmg_packaging.sh
git commit -m "build: package Apple Silicon app as DMG"
```

### Task 2: Test and Update the GitHub Release Workflow

**Files:**
- Create: `tests/test_release_workflow.sh`
- Modify: `.github/workflows/build-release.yml`

**Interfaces:**
- Consumes: `scripts/package-macos-dmg.sh` and a tag or `workflow_dispatch` version.
- Produces: GitHub artifact `ppt-auto-capture-gui-macos-apple-silicon` containing the DMG and README; release asset `ppt-auto-capture-gui-macos-apple-silicon.dmg`.

- [ ] **Step 1: Write the failing workflow contract test**

Create `tests/test_release_workflow.sh` to assert:

```bash
grep -F 'Build macOS (Apple Silicon)' .github/workflows/build-release.yml
grep -F 'aarch64-apple-darwin' .github/workflows/build-release.yml
grep -F 'scripts/package-macos-dmg.sh' .github/workflows/build-release.yml
grep -F 'ppt-auto-capture-gui-macos-apple-silicon.dmg' .github/workflows/build-release.yml
! grep -F 'ppt-auto-capture-gui-macos-x86_64' .github/workflows/build-release.yml
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```bash
bash tests/test_release_workflow.sh
```

Expected: FAIL because the workflow still builds a native raw binary and publishes the old x86_64 tarball.

- [ ] **Step 3: Update the macOS build job**

Change the job to install `aarch64-apple-darwin`, build the explicit target, derive `RELEASE_VERSION` from `${{ inputs.tag }}` for dispatch or `${{ github.ref_name }}` for tag pushes, strip the leading `v`, and call:

```bash
bash scripts/package-macos-dmg.sh \
  target/aarch64-apple-darwin/release/ppt-auto-capture-gui \
  "$RELEASE_VERSION" \
  artifacts/ppt-auto-capture-gui-macos-apple-silicon.dmg
```

Upload the artifact as `ppt-auto-capture-gui-macos-apple-silicon`.

- [ ] **Step 4: Update release assembly**

Copy:

```bash
downloaded-artifacts/ppt-auto-capture-gui-macos-apple-silicon/ppt-auto-capture-gui-macos-apple-silicon.dmg
```

into `release-files`, and replace the old macOS tarball entry in `softprops/action-gh-release` with the DMG.

- [ ] **Step 5: Run the workflow contract test to verify GREEN**

Run:

```bash
bash tests/test_release_workflow.sh
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/build-release.yml tests/test_release_workflow.sh
git commit -m "ci: publish Apple Silicon DMG"
```

### Task 3: Document macOS Installation and Verify the Release Change

**Files:**
- Modify: `README.md`
- Modify: `tests/test_release_workflow.sh`

**Interfaces:**
- Consumes: the DMG name and unsigned/ad-hoc distribution constraints.
- Produces: user-facing installation steps that match the release artifact and Gatekeeper behavior.

- [ ] **Step 1: Extend the failing documentation contract**

Add checks to `tests/test_release_workflow.sh` requiring README to contain:

```text
ppt-auto-capture-gui-macos-apple-silicon.dmg
PPT Auto Capture.app
right-click
Screen Recording
Apple Silicon
not notarized
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```bash
bash tests/test_release_workflow.sh
```

Expected: FAIL because README only describes the Windows executable.

- [ ] **Step 3: Add platform-specific Quick Start instructions**

Keep the existing Windows steps, then add a macOS Apple Silicon section explaining DMG download, drag-to-Applications installation, first-launch right-click/Open fallback, Screen Recording permission, restart after permission changes, and the explicit not-notarized limitation.

- [ ] **Step 4: Run focused tests to verify GREEN**

Run:

```bash
bash tests/test_release_workflow.sh
bash tests/test_macos_dmg_packaging.sh
```

Expected: PASS.

- [ ] **Step 5: Run complete verification**

Run:

```bash
cargo test
cargo build --release --target aarch64-apple-darwin
git diff --check
```

Then mount the final test DMG once more and verify `arm64`, `Info.plist`, signature, application directory, and `/Applications` symlink.

- [ ] **Step 6: Commit**

```bash
git add README.md tests/test_release_workflow.sh
git commit -m "docs: add Apple Silicon DMG installation"
```
