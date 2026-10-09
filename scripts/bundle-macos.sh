#!/usr/bin/env bash
# Builds the macOS Kivori.app + .dmg (M3 S5, ADR-0009). Run on a Mac with Xcode Command Line Tools.
#
#   scripts/bundle-macos.sh
#
# Steps: pinned espflash sidecar, frontend, product firmware, then `tauri build` with the release
# configs (the Tauri build script compiles the vendored mediaremote-adapter and the config bundles it
# into Contents/Resources/mediaremote-adapter, where `adapter_dir()` looks for it).
#
# Signing is a placeholder that runs only when APPLE_SIGNING_IDENTITY is set (a "Developer ID
# Application: ..." identity from the owner's certificate). Without it the bundle is unsigned and the
# helpers keep the ad-hoc signature build.rs gave them. Notarization is not done here.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

bash scripts/fetch-espflash.sh

bun install --frozen-lockfile
bun --filter kivori-desktop-ui build

(cd firmware/esp32-c3 && cargo build --locked --release --target riscv32imc-unknown-none-elf \
  --no-default-features --features physical-st7789)
export KIVORI_FIRMWARE_PATH="$root/firmware/esp32-c3/target/riscv32imc-unknown-none-elf/release/kivori-firmware"

(cd apps/desktop && bunx tauri build \
  --config src-tauri/tauri.release.conf.json \
  --config src-tauri/tauri.release.macos.conf.json \
  -- --no-default-features)

bundle_dir="$root/target/release/bundle"
app="$bundle_dir/macos/Kivori.app"
[ -d "$app" ] || { echo "error: $app was not produced" >&2; exit 1; }

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "Signing the adapter helpers and the app with '${APPLE_SIGNING_IDENTITY}' (hardened runtime)…"
  adapter="$app/Contents/Resources/mediaremote-adapter"
  # Nested code first, then the app that contains it.
  for helper in \
    "$adapter/MediaRemoteAdapter.framework/MediaRemoteAdapter" \
    "$adapter/MediaRemoteAdapterTestClient"; do
    codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$helper"
  done
  codesign --force --options runtime --timestamp --deep \
    --entitlements apps/desktop/src-tauri/Entitlements.plist \
    --sign "$APPLE_SIGNING_IDENTITY" "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
  # The .dmg Tauri wrote holds the pre-signing app, so rebuild it from the signed one.
  # TODO(#36): notarize and staple once the owner's Apple Developer ID exists.
  dmg="$(ls "$bundle_dir"/dmg/*.dmg | head -n1)"
  rm -f "$dmg"
  hdiutil create -volname Kivori -srcfolder "$app" -ov -format UDZO "$dmg"
else
  echo "APPLE_SIGNING_IDENTITY is not set: leaving the bundle unsigned."
fi

echo "Done:"
ls -1 "$bundle_dir"/macos "$bundle_dir"/dmg
