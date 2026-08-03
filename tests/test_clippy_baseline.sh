#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHECKER="$PROJECT_ROOT/scripts/check-clippy-baseline.sh"
TEST_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ppt-capture-clippy-test.XXXXXX")"
trap 'rm -rf "$TEST_TMP_DIR"' EXIT

FIXTURE="$TEST_TMP_DIR/clippy.jsonl"
BASELINE="$TEST_TMP_DIR/baseline"

cat > "$FIXTURE" <<'JSONL'
{"reason":"compiler-message","message":{"level":"warning","message":"first warning"}}
{"reason":"compiler-message","message":{"level":"note","message":"not a warning"}}
{"reason":"build-finished","success":true}
JSONL

printf '1\n' > "$BASELINE"
CLIPPY_OUTPUT_FILE="$FIXTURE" CLIPPY_BASELINE_FILE="$BASELINE" bash "$CHECKER"

printf '0\n' > "$BASELINE"
if output="$(CLIPPY_OUTPUT_FILE="$FIXTURE" CLIPPY_BASELINE_FILE="$BASELINE" bash "$CHECKER" 2>&1)"; then
  printf 'Expected warning count above baseline to fail\n'
  exit 1
fi
[[ "$output" == *"observed=1 baseline=0"* ]]

printf '2\n' > "$BASELINE"
output="$(CLIPPY_OUTPUT_FILE="$FIXTURE" CLIPPY_BASELINE_FILE="$BASELINE" bash "$CHECKER")"
[[ "$output" == *"reduce the baseline from 2 to 1"* ]]

printf 'not-a-number\n' > "$BASELINE"
if CLIPPY_OUTPUT_FILE="$FIXTURE" CLIPPY_BASELINE_FILE="$BASELINE" bash "$CHECKER" >/dev/null 2>&1; then
  printf 'Expected malformed warning baseline to fail\n'
  exit 1
fi

printf 'Clippy warning baseline checks passed\n'
