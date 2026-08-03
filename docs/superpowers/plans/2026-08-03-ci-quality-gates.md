# CI Quality Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tested GitHub CI gates for `main`, protect the current Clippy warning ceiling and changed-file formatting, compile all supported platforms, and prevent invalid or untested releases.

**Architecture:** Keep continuous integration in a new `.github/workflows/ci.yml` and artifact publishing in the existing release workflow. Put nontrivial warning and formatting logic in locally testable shell scripts, then protect both workflows with repository contract tests.

**Tech Stack:** GitHub Actions YAML, Bash, Git, Rust/Cargo, Python 3 for parsing Cargo JSON diagnostics, Ruby/Psych for structural YAML contract tests.

## Global Constraints

- CI runs on pushes to `main`, pull requests targeting `main`, and manual dispatch only.
- Supported build targets remain Windows x86_64 MSVC, Linux x86_64, and macOS Apple Silicon.
- Existing release artifact names remain unchanged.
- No signing certificate or notarization secret is required.
- Existing warnings may remain, but their committed ceiling may not increase.
- Existing untracked `dist/` content must not be modified or committed.
- Finish by merging into `main` and pushing GitHub.

---

### Task 1: Changed-Rust-File Formatting Gate

**Files:**
- Create: `scripts/check-rustfmt-changed.sh`
- Create: `tests/test_changed_rustfmt.sh`

**Interfaces:**
- Consumes: optional newline-separated `CHANGED_RUST_FILES`; otherwise GitHub event SHAs or Git history.
- Produces: exit 0 when changed Rust files are formatted or no Rust files changed; nonzero with file names when rustfmt detects differences.

- [ ] **Step 1: Write the failing script test**

The test creates one formatted and one intentionally unformatted temporary `.rs` file, invokes the missing checker with `CHANGED_RUST_FILES`, and asserts formatted input passes while unformatted input fails.

- [ ] **Step 2: Verify RED**

Run:

```bash
bash tests/test_changed_rustfmt.sh
```

Expected: FAIL because `scripts/check-rustfmt-changed.sh` does not exist.

- [ ] **Step 3: Implement the checker**

The script must:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Resolve files from `CHANGED_RUST_FILES`, PR base/head SHAs, push before/after SHAs, or `git ls-files '*.rs'`. Filter nonexistent and non-`.rs` paths, print the selected files, and execute:

```bash
rustfmt --edition 2021 --check -- "${files[@]}"
```

- [ ] **Step 4: Verify GREEN**

Run `bash tests/test_changed_rustfmt.sh`; expected PASS.

- [ ] **Step 5: Commit**

```bash
git add scripts/check-rustfmt-changed.sh tests/test_changed_rustfmt.sh
git commit -m "ci: check formatting of changed Rust files"
```

### Task 2: Clippy Warning Ceiling

**Files:**
- Create: `scripts/check-clippy-baseline.sh`
- Create: `tests/test_clippy_baseline.sh`
- Create: `.clippy-warning-baseline`

**Interfaces:**
- Consumes: `.clippy-warning-baseline`; optional `CLIPPY_OUTPUT_FILE` containing Cargo JSON lines.
- Produces: exit 0 at or below the ceiling and nonzero above it, with observed and allowed counts.

- [ ] **Step 1: Write failing fixture tests**

Create JSON-line fixtures with zero, equal-to-baseline, and above-baseline compiler warning messages. Assert zero/equal pass, above fails, malformed or missing baseline fails, and lower observed counts print a baseline-reduction hint.

- [ ] **Step 2: Verify RED**

Run `bash tests/test_clippy_baseline.sh`; expected FAIL because the checker is absent.

- [ ] **Step 3: Implement JSON warning counting**

When `CLIPPY_OUTPUT_FILE` is absent, run:

```bash
cargo clippy --all-targets --all-features --message-format=json > "$output"
```

Use Python 3 to count only Cargo `compiler-message` records whose `message.level` is `warning`. Compare that integer to the numeric baseline.

- [ ] **Step 4: Establish the real baseline**

Run the checker once against the repository, record the observed count in `.clippy-warning-baseline`, and rerun. The baseline is a ceiling and may be reduced later.

- [ ] **Step 5: Verify GREEN and commit**

```bash
bash tests/test_clippy_baseline.sh
bash scripts/check-clippy-baseline.sh
git add .clippy-warning-baseline scripts/check-clippy-baseline.sh tests/test_clippy_baseline.sh
git commit -m "ci: prevent new Clippy warnings"
```

### Task 3: Main and Pull-Request CI Workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `tests/test_ci_workflow.rb`

**Interfaces:**
- Consumes: pushes to `main`, PRs targeting `main`, and manual dispatch.
- Produces: Linux quality result and Windows/Linux/macOS compile matrix results.

- [ ] **Step 1: Write the failing workflow contract test**

Parse the workflow with Ruby's standard Psych YAML parser. Assert its actual mapping structure contains main push/PR triggers, concurrency cancellation, the full Cargo test command, both local script-test commands, changed-rustfmt and Clippy checkers, and the three exact runner/target combinations. Normalize Psych's YAML 1.1 boolean interpretation of the `on` key before assertions.

- [ ] **Step 2: Verify RED**

Run `ruby tests/test_ci_workflow.rb`; expected FAIL because `ci.yml` is absent.

- [ ] **Step 3: Implement `ci.yml`**

Add:

- `quality` on Ubuntu with Linux dependencies, tests, shell tests, rustfmt checker, and Clippy baseline checker;
- a three-entry explicit matrix with runner, target, and Linux dependency flag;
- stable Rust plus each target;
- `cargo check --all-targets` using the target value;
- concurrency keyed by workflow and PR number/ref.

- [ ] **Step 4: Verify GREEN and commit**

```bash
ruby tests/test_ci_workflow.rb
git add .github/workflows/ci.yml tests/test_ci_workflow.rb
git commit -m "ci: validate main across supported platforms"
```

### Task 4: Release Preflight and Tag Validation

**Files:**
- Modify: `.github/workflows/build-release.yml`
- Modify: `tests/test_release_workflow.sh`
- Create: `scripts/validate-release-tag.sh`
- Create: `tests/test_release_tag.sh`

**Interfaces:**
- Consumes: an effective release tag from tag push or workflow-dispatch input.
- Produces: early failure unless it matches `v<major>.<minor>` or `v<major>.<minor>.<patch>`; tested build jobs before artifact publishing.

- [ ] **Step 1: Write failing release-tag tests**

Assert `v1.24`, `v1.24.1`, and `v0.1.0` pass; empty, `1.24`, `v1`, `v1.2.3.4`, and strings containing shell/YAML metacharacters fail.

- [ ] **Step 2: Extend the workflow contract test before implementation**

Require a preflight job, tag-validation script invocation, full Rust tests, CI/release/package shell tests, and all build jobs depending on preflight.

- [ ] **Step 3: Verify RED**

Run both shell tests; expected FAIL for the missing script and missing preflight workflow structure.

- [ ] **Step 4: Implement tag validation and preflight**

The tag script uses a Bash regex anchored to the complete input. Add one Ubuntu preflight job to the release workflow and make every build job depend on it. Keep `create-release` dependent on all three builds.

- [ ] **Step 5: Verify GREEN and commit**

```bash
bash tests/test_release_tag.sh
bash tests/test_release_workflow.sh
ruby tests/test_ci_workflow.rb
git add .github/workflows/build-release.yml scripts/validate-release-tag.sh tests/test_release_tag.sh tests/test_release_workflow.sh tests/test_ci_workflow.rb
git commit -m "ci: add release preflight validation"
```

### Task 5: Full Verification and Integration

**Files:**
- Modify only files required by failures caused by this change.

**Interfaces:**
- Consumes: completed feature branch.
- Produces: green local validation, pushed feature branch, and merged/pushed `main`.

- [ ] **Step 1: Run all shell tests**

```bash
for test_script in tests/*.sh; do bash "$test_script"; done
```

- [ ] **Step 2: Run Rust and CI checkers**

```bash
cargo test --all-targets --all-features
bash scripts/check-rustfmt-changed.sh
bash scripts/check-clippy-baseline.sh
git diff --check
```

- [ ] **Step 3: Validate workflow YAML**

Parse both workflow files with an available YAML parser and confirm their `on`, `jobs`, `needs`, matrix, and command values match the contract tests.

- [ ] **Step 4: Push and inspect GitHub Actions**

Push `codex/ci-quality-gates`. Because branch pushes intentionally do not trigger CI, merge into `main`, push `main`, then use GitHub CLI or the GitHub web interface to confirm the new main CI run starts and completes.

- [ ] **Step 5: Report integration**

Report commits, local test counts, warning baseline, workflow run URL/status, and the final `main` SHA. Preserve the user's untracked `dist/` directory.
