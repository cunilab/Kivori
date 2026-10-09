#!/usr/bin/env bash
# Fetches the pinned espflash release and places it where Tauri's `externalBin` expects it:
#   apps/desktop/src-tauri/binaries/espflash-<target-triple>[.exe]
# (M3 S5, ADR: release builds flash only with this bundled copy, never an espflash found on PATH.)
#
# Usage: scripts/fetch-espflash.sh [target-triple]   (default: this machine's rustc host triple)
# Windows runners without bash can use scripts/fetch-espflash.ps1 instead.
#
# The SHA-256 of each release asset is pinned below and checked before anything is extracted. The
# values were taken from the official release page and confirmed by downloading the assets.
set -euo pipefail

ESPFLASH_VERSION="4.6.0"
BASE_URL="https://github.com/esp-rs/espflash/releases/download/v${ESPFLASH_VERSION}"

# asset name for a target triple, and its pinned SHA-256 (keep in sync with fetch-espflash.ps1).
asset_for() {
  case "$1" in
    x86_64-pc-windows-msvc) echo "espflash-x86_64-pc-windows-msvc.zip b2cb4656b067716fe2b3794cf0604b461b2476d06dee7f481704cb6c8495b11c" ;;
    aarch64-apple-darwin) echo "espflash-aarch64-apple-darwin.zip f39bff252a181a6e345991f603d7606cf9762550e557073c1282eada46d8c757" ;;
    x86_64-apple-darwin) echo "espflash-x86_64-apple-darwin.zip e945685fe62e45a120487b79ccecc5ac3586bbf5f4e7d78f95cc09e2227bf32d" ;;
    *) return 1 ;;
  esac
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest_dir="$root/apps/desktop/src-tauri/binaries"
triple="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

if ! entry="$(asset_for "$triple")"; then
  echo "error: no pinned espflash asset for target '$triple' (supported: x86_64-pc-windows-msvc, aarch64-apple-darwin, x86_64-apple-darwin)" >&2
  exit 2
fi
asset="${entry%% *}"
expected="${entry##* }"

exe_suffix=""
bin_name="espflash"
case "$triple" in *windows*) exe_suffix=".exe" ;; esac
dest="$dest_dir/espflash-${triple}${exe_suffix}"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "Fetching espflash ${ESPFLASH_VERSION} (${asset})…"
curl --fail --silent --show-error --location --retry 3 -o "$work/$asset" "$BASE_URL/$asset"

actual="$(sha256_of "$work/$asset")"
if [ "$actual" != "$expected" ]; then
  echo "error: SHA-256 mismatch for $asset" >&2
  echo "  expected $expected" >&2
  echo "  actual   $actual" >&2
  exit 1
fi
echo "SHA-256 verified."

mkdir -p "$work/x" "$dest_dir"
unzip -q -o "$work/$asset" -d "$work/x"
[ -f "$work/x/${bin_name}${exe_suffix}" ] || { echo "error: ${bin_name}${exe_suffix} not found in $asset" >&2; exit 1; }
cp "$work/x/${bin_name}${exe_suffix}" "$dest"
chmod +x "$dest"
echo "Wrote $dest"
