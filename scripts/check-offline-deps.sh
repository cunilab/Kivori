#!/usr/bin/env bash
# Offline dependency guard (offline-first rule; docs/architecture.md).
#
# Kivori is offline-first: the only link is USB serial. The one exception is app update discovery
# (ADR-0015): the desktop app's `src/updater/` module may reach the release endpoint through
# `tauri-plugin-updater`, and nothing else may reach the network. This guard allows exactly that:
#
#   1. Direct dependencies, root and every catalog-selected firmware workspace
#      (`cargo metadata --no-deps`): no first-party crate depends on a network client, and no crate
#      depends on a network-capable Tauri plugin except the pairs in NET_ALLOWED.
#   2. Transitive dependencies, root workspace, every desktop target (normal and build edges): with
#      the allowed plugin's own tree pruned, no network client is left. So the updater plugin's
#      dependency tree is the only way a network crate gets into the app.
#   3. Sources: no first-party shipped Rust source outside ALLOWED_DIR names the updater plugin, a
#      network client or a socket API. So the updater module is the only code that uses them.
set -euo pipefail

NET_BANNED=(reqwest ureq isahc surf attohttpc curl hyper hyper-util awc actix-web tonic)
# Tauri plugins that give network access (direct dependencies are refused unless allowed below).
NET_PLUGINS=(tauri-plugin-updater tauri-plugin-http tauri-plugin-websocket tauri-plugin-upload)
# "<crate> <plugin>": the only allowed pairs (ADR-0015).
NET_ALLOWED=("kivori-desktop tauri-plugin-updater")
# Pruned from the transitive check: the allowed plugins themselves.
NET_PRUNE=(tauri-plugin-updater)
# Targets the desktop app ships to (and Linux CI, where the guard runs).
TARGETS=(x86_64-pc-windows-msvc aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu)
# The one module allowed to use the network, and the first-party sources that ship.
ALLOWED_DIR="apps/desktop/src-tauri/src/updater/"
SOURCE_ROOTS=(apps/desktop/src-tauri/src apps/desktop/src-tauri/build.rs apps/desktop/src-tauri/examples
  crates tools)
NET_SOURCE='tauri_plugin_updater|UpdaterExt|reqwest|ureq|hyper::|std::net|TcpStream|TcpListener|UdpSocket'

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required for the offline-deps check" >&2
  exit 2
fi

fail=0

allowed() {
  local pair="$1 $2" ok
  for ok in "${NET_ALLOWED[@]}"; do
    [ "$ok" = "$pair" ] && return 0
  done
  return 1
}

check_workspace() {
  local dir="$1"
  local meta members deps
  meta="$(cd "$dir" && cargo metadata --no-deps --format-version 1)"
  members="$(printf '%s' "$meta" | jq -r '.packages[].name')"
  while IFS= read -r crate; do
    [ -z "$crate" ] && continue
    deps="$(printf '%s' "$meta" | jq -r --arg n "$crate" \
      '.packages[] | select(.name==$n) | .dependencies[].name')"
    for banned in "${NET_BANNED[@]}"; do
      if printf '%s\n' "$deps" | grep -qx "$banned"; then
        echo "OFFLINE VIOLATION: '$crate' directly depends on network-client crate '$banned'"
        fail=1
      fi
    done
    for plugin in "${NET_PLUGINS[@]}"; do
      if printf '%s\n' "$deps" | grep -qx "$plugin" && ! allowed "$crate" "$plugin"; then
        echo "OFFLINE VIOLATION: '$crate' directly depends on network plugin '$plugin' (not allowed by ADR-0015)"
        fail=1
      fi
    done
  done <<< "$members"
}

check_workspace "."
# Use packaging's TOML resolver so new firmware workspaces cannot escape the dependency/source checks.
firmware_paths="$(mktemp)"
trap 'rm -f "$firmware_paths"' EXIT
bun --eval '
  import { listFirmwarePlans } from "./scripts/build-firmware.mjs";
  import { realpathSync } from "node:fs";
  import { relative, sep } from "node:path";
  const root = realpathSync(process.cwd());
  const crates = new Set(listFirmwarePlans(root).map((plan) =>
    relative(root, realpathSync(plan.cwd)).split(sep).join("/")));
  for (const crate of crates) process.stdout.write(crate + "\0");
' > "$firmware_paths"
while IFS= read -r -d '' firmware_crate; do
  check_workspace "$firmware_crate"
  SOURCE_ROOTS+=("$firmware_crate/src")
done < "$firmware_paths"

prune=()
for p in "${NET_PRUNE[@]}"; do prune+=(--prune "$p"); done
for target in "${TARGETS[@]}"; do
  tree="$(cargo tree --workspace -e normal,build --target "$target" "${prune[@]}" --prefix none \
    --format '{p}' | awk '{print $1}' | sort -u)"
  for banned in "${NET_BANNED[@]}" "${NET_PLUGINS[@]}"; do
    if printf '%s\n' "$tree" | grep -qx "$banned"; then
      echo "OFFLINE VIOLATION: '$banned' reaches the root workspace ($target) other than through ${NET_PRUNE[*]}"
      fail=1
    fi
  done
done

existing=()
for root in "${SOURCE_ROOTS[@]}"; do [ -e "$root" ] && existing+=("$root"); done
hits="$(grep -rnE --include='*.rs' "$NET_SOURCE" "${existing[@]}" | grep -v "^${ALLOWED_DIR}" || true)"
if [ -n "$hits" ]; then
  echo "OFFLINE VIOLATION: network APIs used outside ${ALLOWED_DIR}:"
  printf '%s\n' "$hits" | sed 's/^/  /'
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "Offline dependency check FAILED (see violations above)." >&2
  exit 1
fi

echo "Offline dependency check OK: no first-party crate depends on a network client; the only network"
echo "path is ${NET_ALLOWED[*]} (ADR-0015), used only from ${ALLOWED_DIR}."
echo "  (banned: ${NET_BANNED[*]}; network plugins: ${NET_PLUGINS[*]}; targets: ${TARGETS[*]})"
