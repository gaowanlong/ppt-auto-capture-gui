#!/usr/bin/env bash
# Contract tests for scripts/release-notes.sh: the release body is only as good
# as the section this script pulls out of CHANGELOG.md.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$PROJECT_ROOT/scripts/release-notes.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail() {
  printf 'release notes contract failed: %s\n' "$1" >&2
  exit 1
}

cat > "$TMP/CHANGELOG.md" <<'EOF'
# Changelog

Intro text that belongs to no release.

## [Unreleased]

- work in progress

## [1.2.3] - 2026-01-02

### Added

- a thing

### Fixed

- another thing

## [1.2.2] - 2026-01-01

- older thing

## [1.2.1]
EOF

notes="$(CHANGELOG_FILE="$TMP/CHANGELOG.md" bash "$SCRIPT" v1.2.3)"
expected="### Added

- a thing

### Fixed

- another thing"
[[ "$notes" == "$expected" ]] || fail "the 1.2.3 section must be the trimmed body only: got [$notes]"

notes_without_prefix="$(CHANGELOG_FILE="$TMP/CHANGELOG.md" bash "$SCRIPT" 1.2.3)"
[[ "$notes_without_prefix" == "$expected" ]] || fail "a tag without the v prefix must work too"

older="$(CHANGELOG_FILE="$TMP/CHANGELOG.md" bash "$SCRIPT" v1.2.2)"
[[ "$older" == "- older thing" ]] || fail "each version must stop at the next heading: got [$older]"

if CHANGELOG_FILE="$TMP/CHANGELOG.md" bash "$SCRIPT" v9.9.9 >/dev/null 2>&1; then
  fail "a version without a section must fail instead of releasing empty notes"
fi

if CHANGELOG_FILE="$TMP/CHANGELOG.md" bash "$SCRIPT" v1.2.1 >/dev/null 2>&1; then
  fail "a heading with no content must fail"
fi

if CHANGELOG_FILE="$TMP/missing.md" bash "$SCRIPT" v1.2.3 >/dev/null 2>&1; then
  fail "a missing changelog must fail"
fi

# The real changelog has to cover the tag being released, otherwise the release
# workflow stops at preflight one tag later than anyone wants to find out.
latest="$(sed -n 's/^## \[\([0-9][^]]*\)\].*/\1/p' "$PROJECT_ROOT/CHANGELOG.md" | head -1)"
[[ -n "$latest" ]] || fail "CHANGELOG.md has no version section"
bash "$SCRIPT" "v$latest" >/dev/null || fail "the newest changelog section must be usable as release notes"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$PROJECT_ROOT/Cargo.toml" | head -1)"
bash "$SCRIPT" "v$version" >/dev/null \
  || fail "Cargo.toml version $version has no changelog section; write one before tagging"

printf 'release notes contract tests passed\n'
