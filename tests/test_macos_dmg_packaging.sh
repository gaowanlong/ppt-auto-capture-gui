#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGER="$PROJECT_ROOT/scripts/package-macos-dmg.sh"
TEST_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ppt-capture-dmg-test.XXXXXX")"
MOUNT_DIR="$TEST_TMP_DIR/mount"
IS_MOUNTED=0

cleanup() {
  if [[ "$IS_MOUNTED" -eq 1 ]]; then
    hdiutil detach "$MOUNT_DIR" -quiet || true
  fi
  rm -rf "$TEST_TMP_DIR"
}
trap cleanup EXIT

FAKE_EXECUTABLE="$TEST_TMP_DIR/not-a-mach-o"
printf '#!/usr/bin/env bash\nexit 0\n' > "$FAKE_EXECUTABLE"
chmod +x "$FAKE_EXECUTABLE"

expect_failure() {
  local expected_message="$1"
  shift

  local output
  if output="$("$@" 2>&1)"; then
    printf 'Expected command to fail:'
    printf ' %q' "$@"
    printf '\n'
    exit 1
  fi

  if [[ "$output" != *"$expected_message"* ]]; then
    printf 'Expected failure message containing %q, got:\n%s\n' \
      "$expected_message" "$output"
    exit 1
  fi
}

expect_failure \
  "invalid version" \
  "$PACKAGER" "$FAKE_EXECUTABLE" "release-candidate" "$TEST_TMP_DIR/out.dmg"

expect_failure \
  "executable not found" \
  "$PACKAGER" "$TEST_TMP_DIR/missing" "1.23" "$TEST_TMP_DIR/out.dmg"

expect_failure \
  "must be an arm64 Mach-O executable" \
  "$PACKAGER" "$FAKE_EXECUTABLE" "1.23" "$TEST_TMP_DIR/out.dmg"

printf 'macOS DMG validation failure tests passed\n'

if [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
  ARM64_EXECUTABLE="$PROJECT_ROOT/target/aarch64-apple-darwin/release/ppt-auto-capture-gui"
  [[ -f "$ARM64_EXECUTABLE" ]] || {
    printf 'Apple Silicon release executable not found: %s\n' "$ARM64_EXECUTABLE"
    printf 'Run: cargo build --release --target aarch64-apple-darwin\n'
    exit 1
  }

  OUTPUT_DMG="$TEST_TMP_DIR/ppt-auto-capture-gui-macos-apple-silicon.dmg"
  "$PACKAGER" "$ARM64_EXECUTABLE" "1.23" "$OUTPUT_DMG"
  test -f "$OUTPUT_DMG"

  mkdir -p "$MOUNT_DIR"
  hdiutil attach \
    -readonly \
    -nobrowse \
    -mountpoint "$MOUNT_DIR" \
    "$OUTPUT_DMG" >/dev/null
  IS_MOUNTED=1

  APP_PATH="$MOUNT_DIR/PPT Auto Capture.app"
  APP_EXECUTABLE="$APP_PATH/Contents/MacOS/ppt-auto-capture-gui"
  INFO_PLIST="$APP_PATH/Contents/Info.plist"

  test -d "$APP_PATH"
  test -x "$APP_EXECUTABLE"
  test -L "$MOUNT_DIR/Applications"
  test "$(readlink "$MOUNT_DIR/Applications")" = "/Applications"
  test "$(plutil -extract CFBundleIdentifier raw "$INFO_PLIST")" \
    = "com.gaowanlong.ppt-auto-capture"
  test "$(plutil -extract CFBundleExecutable raw "$INFO_PLIST")" \
    = "ppt-auto-capture-gui"
  test "$(plutil -extract CFBundleShortVersionString raw "$INFO_PLIST")" = "1.23"
  test "$(plutil -extract CFBundleVersion raw "$INFO_PLIST")" = "1.23"
  test "$(plutil -extract LSMinimumSystemVersion raw "$INFO_PLIST")" = "11.0"
  SCREEN_CAPTURE_USAGE="$(
    plutil -extract NSScreenCaptureUsageDescription raw "$INFO_PLIST"
  )"
  [[ "$SCREEN_CAPTURE_USAGE" == *"screen"* ]]
  [[ "$SCREEN_CAPTURE_USAGE" == *"window"* ]]
  file "$APP_EXECUTABLE" | grep -F "arm64" >/dev/null
  codesign --verify --deep --strict --verbose=2 "$APP_PATH"

  hdiutil detach "$MOUNT_DIR" -quiet
  IS_MOUNTED=0
  printf 'macOS DMG end-to-end packaging test passed\n'
fi
