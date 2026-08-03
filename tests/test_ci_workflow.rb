#!/usr/bin/env ruby
# frozen_string_literal: true

require "psych"

root = File.expand_path("..", __dir__)
path = File.join(root, ".github/workflows/ci.yml")
abort "Missing CI workflow: #{path}" unless File.file?(path)

workflow = Psych.safe_load(File.read(path), aliases: true)
triggers = workflow["on"] || workflow[true]

def assert(condition, message)
  abort "CI workflow contract failed: #{message}" unless condition
end

assert(triggers.is_a?(Hash), "on must be a mapping")
assert(Array(triggers.dig("push", "branches")).include?("main"), "push must target main")
assert(Array(triggers.dig("pull_request", "branches")).include?("main"), "pull requests must target main")
assert(triggers.key?("workflow_dispatch"), "workflow_dispatch must be supported")
assert(workflow.dig("concurrency", "cancel-in-progress") == true, "stale runs must be cancelled")

jobs = workflow.fetch("jobs")
quality = jobs.fetch("quality")
assert(quality["runs-on"] == "ubuntu-latest", "quality job must run on Ubuntu")

quality_commands = Array(quality["steps"]).map { |step| step["run"] }.compact.join("\n")
[
  "cargo test --all-targets --all-features",
  "bash tests/test_macos_dmg_packaging.sh",
  "bash tests/test_release_workflow.sh",
  "ruby tests/test_ci_workflow.rb",
  "bash tests/test_changed_rustfmt.sh",
  "bash tests/test_clippy_baseline.sh",
  "bash scripts/check-rustfmt-changed.sh",
  "bash scripts/check-clippy-baseline.sh"
].each do |command|
  assert(quality_commands.include?(command), "quality job must run #{command}")
end

matrix = jobs.fetch("build").dig("strategy", "matrix", "include")
expected = [
  ["ubuntu-latest", "x86_64-unknown-linux-gnu"],
  ["windows-latest", "x86_64-pc-windows-msvc"],
  ["macos-latest", "aarch64-apple-darwin"]
]
actual = Array(matrix).map { |entry| [entry["runner"], entry["target"]] }
assert(actual.sort == expected.sort, "build matrix must cover Linux, Windows, and Apple Silicon macOS")

puts "CI workflow contract tests passed"
