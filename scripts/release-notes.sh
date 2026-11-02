#!/usr/bin/env bash
# Print the CHANGELOG.md section for a release tag.
#
# Usage: scripts/release-notes.sh v2.8.3
#
# The release workflow publishes exactly this text as the GitHub release body, so
# a tag without a section here fails instead of shipping empty notes.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHANGELOG="${CHANGELOG_FILE:-$PROJECT_ROOT/CHANGELOG.md}"

tag="${1:?usage: release-notes.sh <tag>}"
version="${tag#v}"

if [[ ! -f "$CHANGELOG" ]]; then
  printf 'Changelog not found: %s\n' "$CHANGELOG" >&2
  exit 1
fi

notes="$(
  awk -v version="$version" '
    /^## / {
      if (inside) exit
      if (index($0, "[" version "]") > 0) inside = 1
      next
    }
    inside { lines[++count] = $0 }
    END {
      start = 1
      while (start <= count && lines[start] ~ /^[[:space:]]*$/) start++
      finish = count
      while (finish >= start && lines[finish] ~ /^[[:space:]]*$/) finish--
      for (line = start; line <= finish; line++) print lines[line]
      if (start > finish) exit 1
    }
  ' "$CHANGELOG"
)" || true

if [[ -z "${notes//[[:space:]]/}" ]]; then
  printf 'No changelog entry for %s. Add a "## [%s]" section to CHANGELOG.md first.\n' \
    "$tag" "$version" >&2
  exit 1
fi

printf '%s\n' "$notes"
