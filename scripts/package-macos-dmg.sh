#!/usr/bin/env bash
set -euo pipefail

APP_NAME="PPT Auto Capture.app"
VOLUME_NAME="PPT Auto Capture"
BUNDLE_ID="com.gaowanlong.ppt-auto-capture"
EXECUTABLE_NAME="ppt-auto-capture-gui"
VERSION_PATTERN='^[0-9]+([.][0-9]+)*$'

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

if [[ "$#" -ne 3 ]]; then
  fail "usage: $0 <arm64-executable> <version> <output-dmg>"
fi

EXECUTABLE="$1"
VERSION="$2"
OUTPUT_DMG="$3"

[[ "$VERSION" =~ $VERSION_PATTERN ]] || fail "invalid version: $VERSION"
[[ -f "$EXECUTABLE" ]] || fail "executable not found: $EXECUTABLE"

FILE_DESCRIPTION="$(file "$EXECUTABLE")"
if [[ "$FILE_DESCRIPTION" != *"Mach-O"* || "$FILE_DESCRIPTION" != *"arm64"* ]]; then
  fail "executable must be an arm64 Mach-O executable: $FILE_DESCRIPTION"
fi

BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ppt-capture-dmg.XXXXXX")"
APP_PATH="$BUILD_DIR/$APP_NAME"
STAGING_DIR="$BUILD_DIR/staging"
VERIFY_MOUNT="$BUILD_DIR/mount"
IS_MOUNTED=0

cleanup() {
  if [[ "$IS_MOUNTED" -eq 1 ]]; then
    hdiutil detach "$VERIFY_MOUNT" -quiet || true
  fi
  rm -rf "$BUILD_DIR"
}
trap cleanup EXIT

mkdir -p \
  "$APP_PATH/Contents/MacOS" \
  "$APP_PATH/Contents/Resources" \
  "$STAGING_DIR" \
  "$VERIFY_MOUNT"

cp "$EXECUTABLE" "$APP_PATH/Contents/MacOS/$EXECUTABLE_NAME"
chmod 755 "$APP_PATH/Contents/MacOS/$EXECUTABLE_NAME"

cat > "$APP_PATH/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>PPT Auto Capture</string>
  <key>CFBundleDisplayName</key>
  <string>PPT Auto Capture</string>
  <key>CFBundleExecutable</key>
  <string>$EXECUTABLE_NAME</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSArchitecturePriority</key>
  <array>
    <string>arm64</string>
  </array>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSScreenCaptureUsageDescription</key>
  <string>PPT Auto Capture needs screen recording access to capture the selected display or presentation window.</string>
</dict>
</plist>
EOF

plutil -lint "$APP_PATH/Contents/Info.plist" >/dev/null
codesign --force --deep --sign - "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

ditto "$APP_PATH" "$STAGING_DIR/$APP_NAME"
ln -s /Applications "$STAGING_DIR/Applications"

mkdir -p "$(dirname "$OUTPUT_DMG")"
rm -f "$OUTPUT_DMG"
hdiutil create \
  -volname "$VOLUME_NAME" \
  -srcfolder "$STAGING_DIR" \
  -ov \
  -format UDZO \
  "$OUTPUT_DMG"

hdiutil attach \
  -readonly \
  -nobrowse \
  -mountpoint "$VERIFY_MOUNT" \
  "$OUTPUT_DMG" >/dev/null
IS_MOUNTED=1

MOUNTED_APP="$VERIFY_MOUNT/$APP_NAME"
MOUNTED_EXECUTABLE="$MOUNTED_APP/Contents/MacOS/$EXECUTABLE_NAME"

[[ -d "$MOUNTED_APP" ]] || fail "mounted DMG is missing $APP_NAME"
[[ -L "$VERIFY_MOUNT/Applications" ]] || fail "mounted DMG is missing Applications symlink"
[[ "$(readlink "$VERIFY_MOUNT/Applications")" == "/Applications" ]] \
  || fail "Applications symlink does not target /Applications"
[[ "$(plutil -extract CFBundleIdentifier raw "$MOUNTED_APP/Contents/Info.plist")" == "$BUNDLE_ID" ]] \
  || fail "mounted app has an invalid bundle identifier"
[[ "$(plutil -extract CFBundleVersion raw "$MOUNTED_APP/Contents/Info.plist")" == "$VERSION" ]] \
  || fail "mounted app has an invalid bundle version"

MOUNTED_FILE_DESCRIPTION="$(file "$MOUNTED_EXECUTABLE")"
if [[ "$MOUNTED_FILE_DESCRIPTION" != *"Mach-O"* || "$MOUNTED_FILE_DESCRIPTION" != *"arm64"* ]]; then
  fail "mounted app executable is not arm64: $MOUNTED_FILE_DESCRIPTION"
fi

codesign --verify --deep --strict --verbose=2 "$MOUNTED_APP"
hdiutil detach "$VERIFY_MOUNT" -quiet
IS_MOUNTED=0

printf 'Created validated Apple Silicon DMG: %s\n' "$OUTPUT_DMG"
