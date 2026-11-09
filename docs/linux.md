# Linux support

Kivori Desktop targets x86_64 Ubuntu (24.04 / 26.04) and compatible Debian desktops.
Linux hardware acceptance is pending. Firmware and the wire protocol are shared with Windows
and macOS; host features depend on the desktop session and available services.

## Install

Download the `.deb` or `.AppImage` from the download page when a Linux build is published.
For Ubuntu / Debian, prefer the Debian package: it installs the USB access rule automatically.

```sh
sudo apt install ./Kivori_*_amd64.deb
```

AppImage users can choose **Grant access** in Kivori and approve the prompt to install the USB
rule using the system authorization prompt (`pkexec`). Make the AppImage executable and open it. It also needs WebKitGTK 4.1,
libayatana-appindicator, libpulse and libudev on the host.

```sh
chmod +x Kivori_*_amd64.AppImage
./Kivori_*_amd64.AppImage
```

If FUSE is unavailable, install `libfuse2t64` on Ubuntu 24.04+ (`libfuse2` on 22.04), or run
`./Kivori_*_amd64.AppImage --appimage-extract-and-run`. Signed AppImages support in-app updates.
In-app updates are unsupported for `.deb` installs: download and install the new `.deb` from the
website, or use your package manager if it offers Kivori in future.

## USB access

The Debian package installs the USB rule automatically; AppImage users can use **Grant access**.
Reconnect the device after installing the rule. It normally grants access to the logged-in user.
The BOOTSEL drive uses ordinary removable-drive permissions.

## Build and run

Install Rust, Bun, just and jq as described in the README. `just linux-deps` prints this command
without running it:

```sh
sudo apt-get install -y build-essential pkg-config libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev libssl-dev libpulse-dev patchelf file
```

Then:

```sh
bun install --frozen-lockfile
just linux-udev
just desktop
# Native unit tests, without intentional audio writes or input injection:
just linux-check
# Production deb + AppImage, with catalog firmware embedded:
just bundle-linux
```

Packages are written under `target/release/bundle/`. Without `TAURI_SIGNING_PRIVATE_KEY`, local
bundles omit updater signatures. With a key, the usual Tauri key/password validation applies.

## Features and session limits

| Feature | X11 | Wayland |
| --- | --- | --- |
| Master volume / output mute | libpulse, confirmed by read-back | Same (PipeWire's PulseAudio compatibility service) |
| Per-app volume / mute | PulseAudio streams | Same |
| Microphone mute | Every capture device, confirmed by read-back | Same |
| Media keys / now playing | MPRIS over session D-Bus | Same |
| Keyboard shortcuts | enigo X11 input | Keyboard access through the desktop permission prompt |
| Automatic foreground-app profiles | X11 foreground window | Works with the Kivori GNOME extension (GNOME 46–50); one-time logout after enabling |
| CPU / RAM | `/proc` | Same |
| Sleep / wake / lock / unlock | systemd-logind D-Bus | Same |
| App launch | XDG `.desktop` entries | Same |
| Launch at login | `~/.config/autostart` | Same |
| Tray | libayatana-appindicator | Same; GNOME requires the AppIndicator extension |

On GNOME Wayland, Controls offers **Enable** for Kivori App Profiles. It installs a small user extension that tells Kivori which app is in front. Kivori must first ask (`Subscribe` over the local session D-Bus); changes are then sent only to Kivori, not broadcast, and the extension never shares window titles or contents or uses the network. The session bus is not a security boundary between programs you run yourself, so another program running as you could ask for the same app name: enable it only on a computer you control. While the screen is locked or GNOME Shell shows a modal prompt (a password or authentication dialog, a menu, the overview), the extension reports Protected and Kivori sends no shortcuts or macros. Log out and back in once after enabling so GNOME loads it. Kivori detects unsupported GNOME versions, disabled extensions, a missing `gnome-extensions` tool, and an older installed copy: Controls then offers **Update**, which needs one more log-out. Chrome and Brave app windows (for example the Teams web app) follow their own app's profile, not the browser's.

On Wayland, open **Controls** or **Settings**, select **Enable keyboard shortcuts**, then approve keyboard control in the system dialog. Kivori requests keyboard access only; permission lasts until the app or session ends, so enable it again after a restart. If access is denied or disconnected, use the same button to retry. Shortcuts are suspended in protected contexts.

Ubuntu normally enables the AppIndicator extension. On other GNOME installations enable it so
closing the window leaves a reachable tray icon. If profiles remain on General after the one-time logout, open the Extensions app and check that **Kivori App Profiles** is enabled. The Controls card can enable it again.

With no MPRIS player registered, media status is **nothing playing**. Play/pause, next and previous
fall back to media keys on X11 (sent without confirmation) and are unavailable on Wayland. If
players are present but unreadable, status is **unknown**. A missing PulseAudio compatibility
service reports unavailable. Tests do not prove that a real microphone, player or device worked.

## Firmware and recovery

Hold the board's **BOOT** button while connecting USB. The desktop should automount `RPI-RP2`
under `/media/$USER/`; the app copies the matching bundled UF2 onto that drive. A working device
can also enter BOOTSEL through the app's 1200-baud serial touch. If flashing cannot find the drive,
open it in Files and check that your user can write to it. Do not use a raw-disk copy command.

## Troubleshooting

- **Device not found / permission denied:** reconnect after installing the rule. Without logind,
  add your user to `plugdev` with `sudo usermod -aG plugdev "$USER"`, then log out and back in.
  If no Kivori rule is installed, the system's `dialout` group may grant serial access instead.
  For further diagnosis, inspect `ls -l /dev/ttyACM0`, `id` and
  `getfacl /dev/ttyACM0`. Reconnect after installing the rule; log out/in after group changes.
  `udevadm info /dev/ttyACM0` shows the device identity. Check `journalctl -b` for USB/udev errors.
- **No volume or mic state:** `pactl info`, `pactl list short sources` and `wpctl status` check
  PulseAudio / PipeWire. Inspect `journalctl --user -b -u pipewire -u pipewire-pulse -u wireplumber`.
- **Profiles stay on General on GNOME Wayland:** enable Kivori App Profiles from Controls, then log out and back in once. In Extensions, check that it is enabled. Other Wayland compositors are not supported.
- **Shortcuts unavailable:** inspect `echo "$XDG_SESSION_TYPE"`. On Wayland, enable keyboard shortcuts in Settings and approve the system prompt. Check that the desktop portal supports keyboard access.
- **No media info:** start an MPRIS-capable player on the same user session D-Bus.
- **No tray:** enable GNOME's AppIndicator extension, then restart the app.

## Acceptance record

- [ ] Clean Ubuntu installation of `.deb` and AppImage, USB permissions and tray reachability.
- [ ] Real audio read-back, multiple microphones, hotplug and external state changes.
- [ ] X11 shortcuts and profiles; on GNOME Wayland, approve keyboard access and verify shortcuts and profiles.
- [ ] Real media, login startup, lock/unlock, sleep/wake and USB reconnect.
- [ ] Physical UF2 update, interrupted-flash recovery, and signed AppImage self-update.

These checks remain separate from host tests and Windows/macOS acceptance records.
