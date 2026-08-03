#!/usr/bin/env bash
set -euo pipefail

tag="${1:-}"

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
  printf 'Invalid release tag: %s (expected vMAJOR.MINOR or vMAJOR.MINOR.PATCH)\n' "$tag" >&2
  exit 1
fi

printf 'Validated release tag: %s\n' "$tag"
