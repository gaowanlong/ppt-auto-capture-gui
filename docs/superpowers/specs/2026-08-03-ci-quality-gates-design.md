# CI Quality Gates Design

## Goal

Protect the `main` branch from test, platform-build, PPTX, packaging, and release-workflow regressions while keeping ordinary feature-branch pushes inexpensive.

## Scope

This change adds continuous-integration checks and strengthens release preflight validation. It does not change application runtime behavior, capture logic, PPTX output, signing policy, or supported platforms.

The CI workflow runs for:

- every pull request targeting `main`;
- every push to `main`.

Ordinary pushes to other branches do not run the full cross-platform matrix. Tag pushes and manually dispatched releases continue to use the existing release workflow.

## Chosen Architecture

Add a dedicated `.github/workflows/ci.yml` instead of mixing pull-request CI conditions into `build-release.yml`.

The CI workflow has two responsibilities:

1. A fast Linux quality job runs tests and repository-level validation.
2. A platform matrix proves that Windows x86_64, Linux x86_64, and macOS Apple Silicon compile successfully.

The existing release workflow remains responsible for producing and publishing artifacts, but every build job performs a local test/preflight step before packaging. GitHub Actions cannot directly require a workflow run from another event, so the release workflow repeats the small critical checks rather than assuming a previous CI run exists.

## CI Trigger Contract

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

The workflow also supports `workflow_dispatch` for diagnostics. It does not use path filters because changes to documentation, scripts, Cargo metadata, or workflows can affect packaging and releases.

Concurrency groups cancel superseded runs for the same PR or branch while never cancelling a different branch's run.

## Fast Quality Job

The Ubuntu quality job performs:

1. Repository checkout with a clean worktree.
2. Stable Rust installation.
3. Linux GUI dependency installation.
4. `cargo test --all-targets --all-features`.
5. `bash tests/test_macos_dmg_packaging.sh`.
6. `bash tests/test_release_workflow.sh`.
7. CI workflow contract validation through a new shell test.
8. Targeted Rust formatting validation for files changed by the PR or push, falling back to all tracked Rust files when no base revision is available.
9. Clippy warning-baseline validation.

## Clippy Warning Baseline

The repository already contains warnings, so enabling `-D warnings` immediately would block all development for unrelated historical debt.

Add a script that:

- runs `cargo clippy --all-targets --all-features --message-format=short`;
- counts warning records in stable machine-readable output;
- fails when the count exceeds a committed maximum;
- succeeds when the count is equal to or lower than the maximum;
- prints the current count and baseline;
- accepts an injected command/output fixture in tests so its pass/fail behavior is testable without compiling the project repeatedly.

The committed baseline may only decrease. Any intentional increase requires an explicit review-visible baseline edit.

## Changed-File Formatting Gate

Full `cargo fmt --check` currently fails because of historical formatting differences outside this task. CI therefore determines changed tracked `.rs` files relative to:

- the PR base SHA for pull requests;
- the previous commit SHA for pushes;
- all tracked Rust files for manual runs or missing base SHAs.

It invokes `rustfmt --edition 2021 --check` only for that set. If no Rust file changed, the check succeeds without invoking rustfmt.

This gate prevents new formatting debt without creating a repository-wide formatting-only change.

## Platform Build Matrix

The matrix contains three explicit jobs/configurations:

| Runner | Target | Verification |
| --- | --- | --- |
| `ubuntu-latest` | native x86_64 Linux | `cargo check --all-targets --all-features` |
| `windows-latest` | `x86_64-pc-windows-msvc` | `cargo check --all-targets --target x86_64-pc-windows-msvc` |
| `macos-latest` | `aarch64-apple-darwin` | `cargo check --all-targets --target aarch64-apple-darwin` |

Linux installs the same system dependencies as the release build. macOS uses Apple Silicon as the supported architecture. The CI matrix does not generate release artifacts.

## macOS Packaging Check

The fast job runs the packaging script's existing fixture-based test. The macOS matrix job additionally creates a release binary and unsigned DMG only if the cost remains acceptable after implementation measurement. At minimum, it must verify the Apple Silicon release binary's architecture and the packaging script contract.

No certificate, notarization credential, or secret is required. Signing and notarization remain separate future work.

## Release Workflow Hardening

The existing release workflow gains:

- validation that the effective tag matches `v<major>.<minor>` or `v<major>.<minor>.<patch>`;
- tests before release builds;
- existing workflow/package shell tests before artifact publication;
- a release job dependency on every build job;
- no change to current artifact names.

Manual dispatch uses the supplied tag. Tag-triggered runs use `github.ref_name`. Invalid or empty values fail before compilation and artifact publication.

## Workflow Contract Tests

Add `tests/test_ci_workflow.sh`. It operates on repository files and fails with actionable messages when:

- `ci.yml` does not trigger on pushes and pull requests to `main`;
- the quality job omits the full Rust test command;
- any supported platform is absent from the matrix;
- changed-file formatting or warning-baseline checks are missing;
- packaging and release workflow tests are not invoked;
- the release workflow omits tag validation or test preflight.

The test asserts workflow behavior-relevant tokens and script exit codes, not YAML whitespace or job ordering.

Add focused tests for the warning-baseline and changed-file-formatting scripts using temporary fixture directories and injected command output.

## Failure Behavior

- Test failures stop quality validation.
- Build failures identify the affected runner and target.
- A warning-count increase prints the observed and allowed counts.
- Invalid release tags fail before any artifact is published.
- Superseded runs for the same PR/branch are cancelled to save Actions minutes.

## Success Criteria

- CI triggers only for `main` pushes, PRs targeting `main`, and manual dispatch.
- All 106 or more Rust tests run in CI.
- Windows x86_64, Linux x86_64, and macOS Apple Silicon compile checks exist.
- PPTX regression tests run as part of the Rust suite.
- DMG, release workflow, and CI workflow shell tests pass.
- New Rust formatting debt and Clippy warning increases are blocked.
- Release jobs validate tags and run tests before packaging.
- Local validation and GitHub Actions complete successfully.
- The finished change is merged into `main` and pushed to GitHub.
