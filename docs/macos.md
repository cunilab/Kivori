# macOS support

Kivori Desktop targets Apple Silicon macOS alongside Windows. The Mac app uses native CoreAudio,
Quartz input and macOS host notifications; firmware and the wire protocol are shared. This is
native support, not Windows emulation. Intel Mac builds have not been verified.

## Features and limits

| Feature | Mac behavior |
| --- | --- |
| System volume / output mute | CoreAudio, with observed read-back; devices without writable volume report unavailable |
| Microphone mute | Per-device mute and read-back; unknown discovery or unreadable devices cannot confirm all-muted. Some microphones expose no mute control and report unavailable / unverified rather than changing input gain |
| Media keys | Sent to the now-playing app through the bundled adapter, which needs no permission; only if that fails does a key event (Accessibility) take over |
| Keyboard shortcuts | Native key events; require the Accessibility permission, asked for when one is bound (see [desktop capabilities](./desktop-capabilities.md#macos-accessibility)) |
| Current media | Bundled media observer, with Spotify/Music Automation fallback; unavailable observations stay unknown |
| Profiles / app launch / macros | Native bundle identifiers and foreground app checks; queued input retains cancellation guards |
| CPU / RAM / clock | Native system observations |
| Sleep / wake / lock / reconnect | Native handlers |
| Firmware update / BOOT recovery | Bundled product firmware (UF2, written by the app); physical recovery still requires a device |
| App volume / app mute | Unavailable; never substitutes system volume or system mute |
| App update | Manual replacement from a newer DMG; automatic updates are not enabled on macOS |

## Build and check

Use a Mac with Xcode Command Line Tools, Rust and Bun installed:

```sh
just macos-check
just bundle-macos
```

The app and DMG are written to `target/release/bundle/`. Copy the app to Applications before
allowing Accessibility in System Settings → Privacy & Security → Accessibility (Kivori asks once, in setup or in one card on the first start after an update; later in Settings). Denying that
permission keeps keyboard shortcuts unavailable; it must not affect media controls, volume or
configuration. Run `just macos-dev-identity` once first: it creates a self-signed `Kivori Dev`
identity, which `just bundle-macos` then signs with, so the approval survives rebuilds. Without it
every build is signed ad-hoc and macOS asks again each time (Kivori then offers "Reset and ask
again"). Details in [release.md](./release.md#signing-and-why-it-must-be-stable).
Automation prompts may appear for the optional Spotify/Music observation fallback.

`macos-check` runs native unit and read-only backend checks without starting the media observer.
It does not inject keys or change volume/mute. `just macos-audio-test` explicitly exercises output
volume and mute and restores their original values, including on assertion failure. Microphone mute and native shortcuts are not exercised by these commands.

## Distribution

A local build does not require an Apple account. Builds use a self-signed identity (`Kivori Dev`
locally, `Kivori Release` in CI) when available so Accessibility approvals survive updates;
otherwise they are signed ad-hoc. Releases are not notarized. Do not remove quarantine or disable
Gatekeeper as a substitute for release verification.

The website serves a Mac download only when a compatible Apple Silicon DMG is present on a
published release. Until then it reports that the Mac build has not been published. Publishing,
code signing and notarization are separate release actions.
