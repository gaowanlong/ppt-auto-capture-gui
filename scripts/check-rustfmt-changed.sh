#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

declare -a candidates=()

if [[ -n "${CHANGED_RUST_FILES+x}" ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && candidates+=("$file")
  done <<< "${CHANGED_RUST_FILES}"
elif [[ -n "${GITHUB_BASE_SHA:-}" && -n "${GITHUB_HEAD_SHA:-}" ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && candidates+=("$file")
  done < <(git diff --name-only "$GITHUB_BASE_SHA" "$GITHUB_HEAD_SHA" -- '*.rs')
elif [[ -n "${GITHUB_EVENT_BEFORE:-}" && -n "${GITHUB_SHA:-}" && ! "$GITHUB_EVENT_BEFORE" =~ ^0+$ ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && candidates+=("$file")
  done < <(git diff --name-only "$GITHUB_EVENT_BEFORE" "$GITHUB_SHA" -- '*.rs')
elif git rev-parse --verify HEAD^ >/dev/null 2>&1; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && candidates+=("$file")
  done < <(git diff --name-only HEAD^ HEAD -- '*.rs')
else
  while IFS= read -r file; do
    [[ -n "$file" ]] && candidates+=("$file")
  done < <(git ls-files '*.rs')
fi

declare -a rust_files=()
for file in "${candidates[@]}"; do
  if [[ "$file" == *.rs && -f "$file" ]]; then
    rust_files+=("$file")
  fi
done

if [[ "${#rust_files[@]}" -eq 0 ]]; then
  printf 'No changed Rust files to format-check\n'
  exit 0
fi

printf 'Checking Rust formatting for:\n'
printf '  %s\n' "${rust_files[@]}"
rustfmt --edition 2021 --check "${rust_files[@]}"
