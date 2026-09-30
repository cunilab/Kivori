#!/usr/bin/env bash
# Offline dependency guard (offline-first rule; docs/architecture.md).
#
# Fails if any first-party crate declares a network-client crate as a DIRECT dependency. Kivori is
# offline-first: the only link is USB serial, so no first-party crate should pull an HTTP/socket
# client. Inspects DIRECT deps only (`cargo metadata --no-deps`) across BOTH workspaces (the root and
# the isolated firmware workspace). Transitive crates are not rejected.
set -euo pipefail

NET_BANNED=(reqwest ureq isahc surf attohttpc curl hyper hyper-util awc actix-web tonic)

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required for the offline-deps check" >&2
  exit 2
fi

fail=0

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
  done <<< "$members"
}

check_workspace "."
check_workspace "firmware/esp32-c3"

if [ "$fail" -ne 0 ]; then
  echo "Offline dependency check FAILED (see violations above)." >&2
  exit 1
fi

echo "Offline dependency check OK: no first-party crate directly depends on a network client."
echo "  (banned direct deps: ${NET_BANNED[*]})"
