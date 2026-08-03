#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VALIDATOR="$PROJECT_ROOT/scripts/validate-release-tag.sh"

for tag in v1.24 v1.24.1 v0.1.0; do
  "$VALIDATOR" "$tag" >/dev/null
done

for tag in "" 1.24 v1 v1.2.3.4 'v1.2;echo unsafe'; do
  if "$VALIDATOR" "$tag" >/dev/null 2>&1; then
    printf 'Expected invalid release tag to fail: %s\n' "$tag"
    exit 1
  fi
done

printf 'release tag validation tests passed\n'
