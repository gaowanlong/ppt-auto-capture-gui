#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHECKER="$PROJECT_ROOT/scripts/check-rustfmt-changed.sh"
TEST_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ppt-capture-rustfmt-test.XXXXXX")"
trap 'rm -rf "$TEST_TMP_DIR"' EXIT

FORMATTED="$TEST_TMP_DIR/formatted.rs"
UNFORMATTED="$TEST_TMP_DIR/unformatted.rs"

printf 'fn main() {}\n' > "$FORMATTED"
printf 'fn main(){println!("not formatted");}\n' > "$UNFORMATTED"

CHANGED_RUST_FILES="$FORMATTED" bash "$CHECKER"

if CHANGED_RUST_FILES="$UNFORMATTED" bash "$CHECKER" >/dev/null 2>&1; then
  printf 'Expected unformatted Rust input to fail\n'
  exit 1
fi

CHANGED_RUST_FILES='' bash "$CHECKER"

printf 'changed Rust formatting checks passed\n'
