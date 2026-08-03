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

ruby -rpsych - "$WORKFLOW" <<'RUBY'
path = ARGV.fetch(0)
workflow = Psych.safe_load(File.read(path), aliases: true)
jobs = workflow.fetch("jobs")

def assert(condition, message)
  abort "Release workflow contract failed: #{message}" unless condition
end

preflight = jobs["preflight"]
assert(preflight, "preflight job is required")
commands = Array(preflight["steps"]).map { |step| step["run"] }.compact.join("\n")
[
  "bash scripts/validate-release-tag.sh",
  "cargo test --all-targets --all-features",
  "bash tests/test_macos_dmg_packaging.sh",
  "bash tests/test_release_tag.sh",
  "ruby tests/test_ci_workflow.rb",
  "bash scripts/check-rustfmt-changed.sh",
  "bash scripts/check-clippy-baseline.sh"
].each do |command|
  assert(commands.include?(command), "preflight must run #{command}")
end

%w[build-windows build-linux build-macos].each do |job_name|
  assert(Array(jobs.fetch(job_name)["needs"]).include?("preflight"), "#{job_name} must need preflight")
end

release_needs = Array(jobs.fetch("create-release")["needs"])
%w[build-windows build-linux build-macos].each do |job_name|
  assert(release_needs.include?(job_name), "create-release must need #{job_name}")
end
RUBY

assert_contains "ppt-auto-capture-gui-macos-apple-silicon.dmg" "$README"
assert_contains "PPT Auto Capture.app" "$README"
assert_contains "right-click" "$README"
assert_contains "Screen Recording" "$README"
assert_contains "Apple Silicon" "$README"
assert_contains "not notarized" "$README"

printf 'release workflow contract tests passed\n'
