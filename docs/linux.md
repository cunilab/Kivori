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
| Keyboard shortcuts | enigo X11 input | Kivori GNOME extension on GNOME 46–50; desktop keyboard portal on other compositors |
| Automatic foreground-app profiles | X11 foreground window | Works with the Kivori GNOME extension (GNOME 46–50); one-time logout after enabling |
| CPU / RAM | `/proc` | Same |
| Sleep / wake / lock / unlock | systemd-logind D-Bus | Same |
| App launch | XDG `.desktop` entries | Same |
| Launch at login | `~/.config/autostart` | Same |
| Tray | libayatana-appindicator | Same; GNOME requires the AppIndicator extension |

On GNOME Wayland, Controls offers **Enable** or **Update** for Kivori App Profiles. The app installs a small user extension and records the canonical Kivori executable path in `trusted.json`. Reinstall/update refreshes that path and always requires one log-out/in, even if the extension version is unchanged. Log out and back in once to load extension v5, then on first shortcut use approve **Allow Kivori to press keyboard shortcuts?** in GNOME’s own modal dialog, which shows the full executable path and its keyboard capability. Approval is stored privately, bound to the executable path, device/inode, modification time and size; it survives app and session restarts, and a changed executable needs approval again. If the executable moves, use Update in Controls to register its new path. Rejecting the dialog leaves shortcuts unavailable; restart Kivori to request approval again. Only the installed app should run the installer: a diagnostic executable would register its own path.

Focus subscription and keyboard authorization are separate. The extension obtains each caller’s PID from the bus daemon, checks `/proc/<pid>/exe` against the installed path and its executable fingerprint, and rejects sandbox proxies whose actual identity cannot be verified. Unauthenticated callers cannot occupy focus subscription slots; only one approved unique bus peer may inject. Disconnect, executable replacement and disable revoke active injection. The extension shares no window titles or contents and uses no network. Trust and approval files are read once at extension enable and retained in memory for this Shell session; background requests never re-read them or open a consent dialog. Changes to those files during the session cannot silently grant keyboard control. A trusted-file monitor revokes current consent when `trusted.json` changes, and a changed executable fingerprint requires a fresh Shell dialog. A new executable path or on-disk approval takes effect only after re-login; only approval granted by the Shell dialog can be added in memory during this session. The cached path is retained across extension disable/re-enable. This protects against other apps silently pressing keys in this session. Like any GNOME extension, it relies on files in your home folder and does not defend against malware already running as you. The PID/executable checks are not an atomic snapshot, and the executable fingerprint uses metadata rather than a content hash.

Shortcut policy on GNOME Wayland: use a chord containing Ctrl, Alt or Super, or a **lone** F1–F12, Escape, Tab, arrow, Home, End, PageUp, PageDown, Delete, or explicit media/volume keysym. Plain printable characters, Shift-only chords, lone Enter/Return and modifier-only keys (including Super/overview) are refused. Also always blocked, even with extra modifiers: **Alt+F2**, **Ctrl+Alt+F1–F12**, **Ctrl+Alt+Delete/Backspace**, **Super+L**, and clipboard paste **Ctrl+V / Ctrl+Shift+V / Super+V**. The action picker displays a “not allowed” reason for blocked chords. There is no text or raw key-stream API. Media actions still use MPRIS.

The extension fails closed unless screen shield, session lock, modal count and stage-grab state can all be validated. Lock screens, menus, overview, authentication prompts and other Shell modal grabs refuse injection. Every send includes a Shell monotonic deadline (at most 100 ms) and focus generation; delayed or changed-focus requests are discarded without retries. Chords are rate-limited to 50 per second and key contents are never logged. Releases are attempted in reverse order, on errors, disconnect and disable; the virtual device is explicitly disposed/reset. Any unconfirmed release remains tracked and suspends further injection, including across extension disable/re-enable; restart the GNOME session if cleanup remains uncertain. Successful shortcuts are **triggered, unverified**, not confirmation that the target app performed an action.

Chrome and Brave app windows (for example the Teams web app) follow their own app’s profile, not the browser’s. Other Wayland compositors do not support automatic foreground-app profiles through this GNOME extension.

On other Wayland desktops, open **Controls** or **Settings**, select **Enable keyboard shortcuts**, then approve keyboard control in the system dialog. Kivori requests keyboard access only; permission lasts until the app or session ends, so enable it again after a restart. If access is denied or disconnected, use the same button to retry. Shortcuts are suspended in protected contexts.

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
- **Shortcuts unavailable:** inspect `echo "$XDG_SESSION_TYPE"`. On GNOME Wayland, enable or update Kivori App Profiles in Controls, then log out and back in once to load version 5 or newer and approve the GNOME keyboard dialog. Other Wayland desktops need keyboard access through the desktop portal.
- **No media info:** start an MPRIS-capable player on the same user session D-Bus.
- **No tray:** enable GNOME's AppIndicator extension, then restart the app.

## Acceptance record

- [ ] Clean Ubuntu installation of `.deb` and AppImage, USB permissions and tray reachability.
- [ ] Real audio read-back, multiple microphones, hotplug and external state changes.
- [ ] X11 shortcuts and profiles; on GNOME Wayland, update the extension, log out and back in, and verify shortcuts and profiles (including lock/modal refusal).
- [ ] Real media, login startup, lock/unlock, sleep/wake and USB reconnect.
- [ ] Physical UF2 update, interrupted-flash recovery, and signed AppImage self-update.

These checks remain separate from host tests and Windows/macOS acceptance records.
