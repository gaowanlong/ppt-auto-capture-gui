#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKFLOW="$PROJECT_ROOT/.github/workflows/build-release.yml"
README="$PROJECT_ROOT/README.md"

assert_contains() {
  local expected="$1"
  local file="$2"
  grep -F "$expected" "$file" >/dev/null || {
    printf 'Expected %s to contain: %s\n' "$file" "$expected"
    exit 1
  }
}

assert_not_contains() {
  local unexpected="$1"
  local file="$2"
  if grep -F "$unexpected" "$file" >/dev/null; then
    printf 'Expected %s not to contain: %s\n' "$file" "$unexpected"
    exit 1
  fi
}

assert_contains "Build macOS (Apple Silicon)" "$WORKFLOW"
assert_contains "aarch64-apple-darwin" "$WORKFLOW"
assert_contains "scripts/package-macos-dmg.sh" "$WORKFLOW"
assert_contains "ppt-auto-capture-gui-macos-apple-silicon.dmg" "$WORKFLOW"
assert_not_contains "ppt-auto-capture-gui-macos-x86_64" "$WORKFLOW"

assert_contains "ppt-auto-capture-gui-macos-apple-silicon.dmg" "$README"
assert_contains "PPT Auto Capture.app" "$README"
assert_contains "right-click" "$README"
assert_contains "Screen Recording" "$README"
assert_contains "Apple Silicon" "$README"
assert_contains "not notarized" "$README"

printf 'release workflow contract tests passed\n'
