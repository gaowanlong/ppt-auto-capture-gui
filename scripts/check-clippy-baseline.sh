#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASELINE_FILE="${CLIPPY_BASELINE_FILE:-$PROJECT_ROOT/.clippy-warning-baseline}"
GENERATED_OUTPUT=''

cleanup() {
  if [[ -n "$GENERATED_OUTPUT" && -f "$GENERATED_OUTPUT" ]]; then
    rm -f "$GENERATED_OUTPUT"
  fi
}
trap cleanup EXIT

if [[ ! -f "$BASELINE_FILE" ]]; then
  printf 'Clippy warning baseline file not found: %s\n' "$BASELINE_FILE" >&2
  exit 1
fi

baseline="$(tr -d '[:space:]' < "$BASELINE_FILE")"
if [[ ! "$baseline" =~ ^[0-9]+$ ]]; then
  printf 'Clippy warning baseline must be a non-negative integer: %s\n' "$BASELINE_FILE" >&2
  exit 1
fi

if [[ -n "${CLIPPY_OUTPUT_FILE:-}" ]]; then
  output_file="$CLIPPY_OUTPUT_FILE"
else
  GENERATED_OUTPUT="$(mktemp "${TMPDIR:-/tmp}/ppt-capture-clippy.XXXXXX")"
  output_file="$GENERATED_OUTPUT"
  (
    cd "$PROJECT_ROOT"
    cargo clippy --all-targets --all-features --message-format=json > "$output_file"
  )
fi

if [[ ! -f "$output_file" ]]; then
  printf 'Clippy output file not found: %s\n' "$output_file" >&2
  exit 1
fi

observed="$(python3 - "$output_file" <<'PY'
import json
import sys

count = 0
with open(sys.argv[1], encoding="utf-8") as stream:
    for line_number, line in enumerate(stream, 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise SystemExit(f"invalid Cargo JSON on line {line_number}: {error}")
        if (
            record.get("reason") == "compiler-message"
            and record.get("message", {}).get("level") == "warning"
        ):
            count += 1
print(count)
PY
)"

printf 'Clippy warning count: observed=%s baseline=%s\n' "$observed" "$baseline"

if (( observed > baseline )); then
  printf 'Clippy warning ceiling exceeded: observed=%s baseline=%s\n' "$observed" "$baseline" >&2
  exit 1
fi

if (( observed < baseline )); then
  printf 'Clippy warnings improved; reduce the baseline from %s to %s\n' "$baseline" "$observed"
fi
