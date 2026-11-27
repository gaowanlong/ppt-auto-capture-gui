#!/usr/bin/env bash
# Build the HarmonyOS PC (2in1) debug HAP.
#
# Requires:
#   - Rust 1.88+ with the aarch64-unknown-linux-ohos target
#   - HarmonyOS SDK with the native NDK (OHOS_SDK points to its root)
#   - DevEco Studio / hvigorw for the HAP step (no certificate needed for a
#     locally debuggable build; DevEco can auto-generate a debug signature)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SDK_ROOT="${OHOS_SDK:-$HOME/ohos-sdk}"
case "$(uname -s)" in
  Darwin) HOST_DIR=darwin ;;
  Linux) HOST_DIR=linux ;;
  *) echo "Unsupported build host: $(uname -s)" >&2; exit 1 ;;
esac

NATIVE="$SDK_ROOT/$HOST_DIR/native"
LINKER="$NATIVE/llvm/bin/aarch64-unknown-linux-ohos-clang"
SYSROOT="$NATIVE/sysroot"
TARGET=aarch64-unknown-linux-ohos

if [[ ! -x "$LINKER" ]]; then
  echo "HarmonyOS NDK linker not found: $LINKER" >&2
  echo "Set OHOS_SDK to the SDK root that contains $HOST_DIR/native." >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "Installing Rust target $TARGET" >&2
  rustup target add "$TARGET"
fi

export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$LINKER"
export RUSTFLAGS="-C link-arg=--sysroot=$SYSROOT"

echo "==> Building the shared Rust core for $TARGET"
cargo build --manifest-path "$ROOT/ohos/rust_core/Cargo.toml" --target "$TARGET" --release

LIB_DIR="$ROOT/ohos/entry/src/main/libs/arm64-v8a"
mkdir -p "$LIB_DIR"
cp "$ROOT/ohos/rust_core/target/$TARGET/release/libppt_capture_ohos_core.so" "$LIB_DIR/"
echo "==> NAPI module copied to $LIB_DIR"

if command -v hvigorw >/dev/null 2>&1; then
  echo "==> Building debug HAP"
  (cd "$ROOT/ohos" && hvigorw assembleHap --mode module -p product=default -p buildMode=debug --no-daemon)
  echo "==> HAP: $ROOT/ohos/entry/build/default/outputs/default/entry-default-signed.hap"
else
  echo "hvigorw is not on PATH. Open $ROOT/ohos in DevEco Studio and run 'Build > Build HAP(s)'." >&2
  echo "Then install with: hdc install entry/build/default/outputs/default/entry-default-signed.hap" >&2
fi
