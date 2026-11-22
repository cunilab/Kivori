#!/usr/bin/env bash
# Real GNOME/GJS/GTK Wayland regression, isolated from the logged-in Shell and consent.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ${1:-} != --session ]]; then
  probe_root=$(mktemp -d /tmp/kivori-gnome-smoke.XXXXXX)
  trap 'rm -rf "$probe_root" 2>/dev/null || true' EXIT
  export KIVORI_SMOKE_ROOT="$probe_root"
  # D-Bus activated services (notably dconf) must inherit the isolated directories too.
  export XDG_CONFIG_HOME="$probe_root/config" XDG_DATA_HOME="$probe_root/data" XDG_CACHE_HOME="$probe_root/cache"
  dbus-run-session -- bash "$0" --session
  exit
fi
export XDG_CONFIG_HOME="$KIVORI_SMOKE_ROOT/config"
export XDG_DATA_HOME="$KIVORI_SMOKE_ROOT/data"
export XDG_CACHE_HOME="$KIVORI_SMOKE_ROOT/cache"
export GDK_BACKEND=wayland
extension_dir="$XDG_DATA_HOME/gnome-shell/extensions/kivori-focus@cunilab.id"
mkdir -p "$extension_dir" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
cp "${KIVORI_SMOKE_EXTENSION_DIR:-apps/desktop/src-tauri/linux/gnome-extension/kivori-focus@cunilab.id}"/* "$extension_dir/"
# A fresh GNOME session starts in the overview, which holds keyboard focus.
# This fixture-only hook dismisses it and activates the probe. A persistent fixture
# keyboard gives the headless seat its keyboard capability before the client connects,
# matching a live desktop with a physical keyboard. Shortcut and identity code stays unchanged.
sed -i '1i import GLibSmoke from "gi://GLib";' "$extension_dir/extension.js"
sed -i '/^  enable() {/a\    this._smokeKeyboard = global.stage.context.get_backend().get_default_seat().create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE); GLibSmoke.timeout_add(GLibSmoke.PRIORITY_DEFAULT, 500, () => { Main.overview.hide(); const window = global.get_window_actors().map(actor => actor.meta_window).find(window => window.get_title() === "Kivori shortcut smoke"); if (!window) return GLibSmoke.SOURCE_CONTINUE; window.activate(global.get_current_time()); return GLibSmoke.SOURCE_REMOVE; });' "$extension_dir/extension.js"
gjs -m scripts/tests/fixtures/gnome-shortcut-smoke.js prepare "$extension_dir"
gsettings set org.gnome.shell enabled-extensions "['kivori-focus@cunilab.id']"
gsettings set org.gnome.shell welcome-dialog-last-shown-version '50.1'
gnome-shell --mode=user --headless --wayland --no-x11 --virtual-monitor 800x600 --wayland-display "kivori-smoke-$$" > "$KIVORI_SMOKE_ROOT/shell.log" 2>&1 &
shell_pid=$!
trap 'kill "$shell_pid" 2>/dev/null || true; wait "$shell_pid" 2>/dev/null || true' EXIT
export WAYLAND_DISPLAY="kivori-smoke-$$"
for _attempt in {1..100}; do
  if gdbus call --session --dest id.cunilab.KivoriFocus --object-path /id/cunilab/KivoriFocus --method org.freedesktop.DBus.Properties.Get id.cunilab.KivoriFocus Version >/dev/null 2>&1; then break; fi
  if ! kill -0 "$shell_pid" 2>/dev/null; then cat "$KIVORI_SMOKE_ROOT/shell.log"; exit 1; fi
  sleep 0.1
done
if ! timeout 20 gjs -m scripts/tests/fixtures/gnome-shortcut-smoke.js probe "$extension_dir"; then
  cat "$KIVORI_SMOKE_ROOT/shell.log"
  exit 1
fi
