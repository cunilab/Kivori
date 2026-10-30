# macOS support

Kivori Desktop targets Apple Silicon macOS alongside Windows. The Mac app uses native CoreAudio,
Quartz input and macOS host notifications; firmware and the wire protocol are shared. This is
native support, not Windows emulation. Intel Mac builds have not been verified.

## Features and limits

| Feature | Mac behavior |
| --- | --- |
| System volume / output mute | CoreAudio, with observed read-back; devices without writable volume report unavailable |
| Microphone mute | Per-device mute and read-back; unknown discovery or unreadable devices cannot confirm all-muted. Some microphones expose no mute control and report unavailable / unverified rather than changing input gain |
| Media keys / keyboard shortcuts | Native input; requires Accessibility permission for the installed app |
| Current media | Bundled media observer, with Spotify/Music Automation fallback; unavailable observations stay unknown |
| Profiles / app launch / macros | Native bundle identifiers and foreground app checks; queued input retains cancellation guards |
| CPU / RAM / clock | Native system observations |
| Sleep / wake / lock / reconnect | Native handlers exist; physical acceptance is recorded separately below |
| Firmware update / BOOT recovery | Bundled espflash and product firmware; physical recovery still requires a device |
| App volume / app mute | Unavailable; never substitutes system volume or system mute |
| App update | Manual replacement from a newer DMG; automatic updates are not enabled on macOS |

## Build and check

Use a Mac with Xcode Command Line Tools, Rust and Bun installed:

```sh
just macos-check
just bundle-macos
```

The app and DMG are written to `target/release/bundle/`. Copy the app to Applications before
allowing Accessibility in System Settings → Privacy & Security → Accessibility. Denying that
permission keeps keyboard actions unavailable; it must not affect volume or configuration.
Automation prompts may appear for the optional Spotify/Music observation fallback.

`macos-check` runs native unit and read-only backend checks without starting the media observer.
It does not inject keys or change volume/mute. `just macos-audio-test` explicitly exercises output
volume and mute and restores their original values, including on assertion failure. Microphone
mute and native shortcuts still need a deliberate hands-on session.

## Distribution

A local build does not require an Apple account. `APPLE_SIGNING_IDENTITY` enables Developer ID
signing in the existing bundle script. Notarization is not yet implemented; locally built unsigned
or ad-hoc signed bundles are not evidence that Gatekeeper accepts a public release. Do not remove
quarantine or disable Gatekeeper as a substitute for release verification.

The website serves a Mac download only when a compatible Apple Silicon DMG is present on a
published release. Until then it reports that the Mac build has not been published. Publishing,
code signing and notarization are separate release actions.

## Acceptance record

Mac acceptance is separate from the existing Windows M2.9 20/24 count. Neither shared host tests
nor the virtual device close either platform's physical gates.

- [ ] Packaged Apple Silicon app starts, displays setup and quits cleanly.
- [ ] Accessibility denial, grant and revocation behave honestly for the installed copy.
- [ ] Real volume/mute read-back and external changes; unsupported output devices are handled.
- [ ] Multiple microphones, unreadable / unsettable devices and hotplug never falsely confirm mute.
- [ ] Media controls, shortcuts to the intended app, app launch and macro cancellation work.
- [ ] Device discovery, buffered inputs, display output, USB reconnect, sleep/wake and lock/unlock work.
- [ ] Bundled firmware update and interrupted-flash BOOT recovery work on a physical unit.
- [ ] Login startup and a manual app replacement preserve configuration.
- [ ] Signed and notarized installation works on a clean Mac before public distribution.
- [ ] Actual idle soak, sustained use and unfamiliar-user observation are recorded on Mac.

## Local validation — 2026-10-30

On Andress-MacBook-Air, arm64 macOS 27.0.1 (26A434), with the working changes based on
`8d9bb4aa` (not a published release):

- `just macos-check`: 19 native unit tests, five read-only native backend tests and eight volume
  contract tests passed. Three unit tests and the output-write integration test remain opt-in;
  the media-observer integration test is deliberately excluded from this command.
- `just macos-audio-test`: passed against the actual output endpoint, including independent
  external change observation and asserted restoration of the original volume and mute. Two
  earlier runs exposed test-isolation assumptions about coalesced changes and the external
  command changing mute; the test now settles the actually observed baseline. Production echo
  classification remains conservative and unchanged. AppleScript restoration is only a fallback
  if exact native restoration fails, avoiding unnecessary rounding after a successful restore.
- Six microphone fault-injection regressions passed; three first failed against the old logic.
  Discovery/capability failures and failed notification rescans cannot retain all-muted certainty.
  Missing device/system subscriptions retry during existing rescans.
- Website: all 123 tests, typecheck, scoped ESLint and formatting passed. Downloads are selected
  independently per OS, reject ambiguous / incompatible DMGs, and remain unavailable until the
  matching artifact is published. Server rendering does not initially link Mac visitors to EXEs.
- Strict native Clippy, Rust formatting and diff whitespace checks passed.
- `just bundle-macos` built `target/release/bundle/macos/Kivori.app` (34.80 MiB) and
  `target/release/bundle/dmg/Kivori_0.2.1_aarch64.dmg` (13.30 MiB). The executable is arm64;
  bundled firmware, espflash, media helper and notices are present. The release-artifact guard
  passed (development controls absent, firmware marker present), and `hdiutil verify` passed.
  DMG SHA-256: `42e76b981e8ad587c3ba4a1d3ea12022253ff009b408ff244951bfb71d7b0844`.
- Claude Opus 5.5 at medium effort approved the microphone handling and the final Mac changes
  after the web availability wording was corrected. Its optional restoration improvement was
  applied and the live output-audio test passed again.

The bundle is not Developer ID signed or notarized. Native GUI inspection was attempted but the
Computer Use service refused with `sky requires node_repl; configure NODE_REPL_TRUSTED_SERVICES`;
no startup, setup-screen or permission-grant result is claimed. Real microphone writes, native
shortcuts, USB, firmware flashing, sleep/wake, login and long-use acceptance remain open. No
website deployment, artifact publication or paid CI was performed. All checklist rows above remain
open until their complete observations are available; the single-output audio result does not
establish multiple-device or hotplug coverage.


Follow-up, 2026-10-30: GUI access was retried and remains blocked by the Computer Use trusted-service
configuration. Read-only USB inspection found no compatible Pip (`303a:1001`) connected, and the
available keychains have no Developer ID Application signing identity. GUI setup/Accessibility, physical
controls/reconnect/power transitions and signed distribution therefore remain NOT RUN.

Follow-up, 2026-10-30 (packaged-app smoke from the terminal): screen capture and the window server
work from Claude Code, so the packaged app was driven without Computer Use; Accessibility cannot see
the Tauri window, so windows are found by CoreGraphics window ID. With a temporarily removed config
(restored byte-identical afterwards), the app started and showed setup step 1 of 6 ("Welcome to
Kivori"). Quitting through the Quit Apple event then **failed** to quit cleanly: the app exited but
the now-playing helper (`mediaremote-adapter.pl … stream`) stayed running, reparented to launchd.
Cause: a macOS terminate (Quit Apple event, Cmd+Q, logout) reaches Tauri only as `RunEvent::Exit`,
which did not run `AppState::shutdown()`, so the observer that reaps the helper was never dropped.
SIGTERM, Force Quit and crashes skip shutdown entirely and orphaned it too.

Fixed: `RunEvent::Exit` now runs the idempotent shutdown, and the stream helper runs behind a forked
watcher holding a stdin pipe from Kivori; when Kivori dies by any route the pipe closes and the watcher
terminates the helper. Two unit tests cover the watcher (each fails when its mechanism is removed),
the live MediaRemote observer test still streams and stops promptly, and `just macos-check`, strict
Clippy and formatting pass. On the rebuilt bundle, quitting by Apple event, SIGTERM and SIGKILL each
left no Kivori process. The setup screen was not re-captured on the rebuilt bundle because the
display locked mid-run; the first row stays open until that screenshot is taken.
