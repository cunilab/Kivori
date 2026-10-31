#!/usr/bin/env bash
# Signs a Kivori.app with a STABLE identity, so the macOS Accessibility approval survives updates.
#
#   scripts/sign-macos-bundle.sh path/to/Kivori.app
#
# macOS ties an approval (TCC) to the app's designated requirement: bundle identifier plus signing
# certificate. A linker ad-hoc signature changes with every build, so every build or update would
# lose the approval. Signing every build with the same identity keeps it.
#
# Identity: KIVORI_MACOS_SIGNING_IDENTITY (a keychain identity name, or `-` for ad-hoc). Unset: "Kivori Dev" when it is
# in the keychain (`just macos-dev-identity` creates it), else ad-hoc (`-`) with a warning. A named
# identity that is set but missing, or any signing/verification failure, is an error: it never
# silently falls back to ad-hoc.
# KIVORI_MACOS_EXPECT_LEAF (optional, certificate SHA-1): the designated requirement must pin that
# leaf, so a swapped key is caught.
#
# No hardened runtime: the app sends Apple Events (now-playing fallback) and has no entitlements to
# go with it. When a real Developer ID (APPLE_SIGNING_IDENTITY) is configured, scripts/bundle-macos.sh
# signs with that instead and never calls this.
set -euo pipefail

app="${1:?usage: sign-macos-bundle.sh path/to/Kivori.app}"
[ -d "$app" ] || { echo "error: $app is not a directory" >&2; exit 1; }
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bundle_id="$(plutil -extract identifier raw -o - "$root/apps/desktop/src-tauri/tauri.conf.json")"
[ -n "$bundle_id" ] || { echo "error: no bundle identifier in tauri.conf.json" >&2; exit 1; }

warn() {
  if [ "${GITHUB_ACTIONS:-}" = true ]; then echo "::warning::$1"; else echo "warning: $1" >&2; fi
}

has_identity() {
  local found
  found="$(security find-identity -p codesigning)"
  grep -Fq "\"$1\"" <<<"$found"
}

identity="${KIVORI_MACOS_SIGNING_IDENTITY:-}"
if [ -n "$identity" ]; then
  [ "$identity" = - ] || has_identity "$identity" || { echo "error: signing identity '$identity' is not in the keychain" >&2; exit 1; }
elif has_identity "Kivori Dev"; then
  identity="Kivori Dev"
else
  identity="-"
  warn "signing ad-hoc: this build's Accessibility approval will not survive the next build or update. Run 'just macos-dev-identity' for a stable local identity."
fi

echo "Signing ${app##*/} as '$bundle_id' with identity '$identity'..."
# Nested code first (the adapter helpers are Mach-O files under Resources, outside the places
# --deep walks), then the app that contains it.
adapter="$app/Contents/Resources/mediaremote-adapter"
for helper in \
  "$adapter/MediaRemoteAdapter.framework/MediaRemoteAdapter" \
  "$adapter/MediaRemoteAdapterTestClient"; do
  if [ -e "$helper" ]; then codesign --force --timestamp=none --sign "$identity" "$helper"; fi
done
codesign --force --deep --timestamp=none --sign "$identity" --identifier "$bundle_id" "$app"
codesign --verify --deep --strict --verbose=2 "$app"

details="$(codesign -dv "$app" 2>&1)"
printf '%s\n' "$details" | grep -Fxq "Identifier=$bundle_id" ||
  { echo "error: the signature does not carry the identifier $bundle_id" >&2; exit 1; }
requirement="$(codesign -d -r- "$app" 2>&1)"
if [ -n "${KIVORI_MACOS_EXPECT_LEAF:-}" ]; then
  expected="certificate leaf = H\"$(printf '%s' "$KIVORI_MACOS_EXPECT_LEAF" | tr 'A-F' 'a-f')\""
  case "$requirement" in
    *"$expected"*) echo "Signing certificate matches the pinned leaf." ;;
    *) printf "error: the signing certificate is not the pinned one (%s); got:\n%s\n" "$expected" "$requirement" >&2; exit 1 ;;
  esac
fi
