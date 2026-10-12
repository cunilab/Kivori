# ADR-0015: App updates through GitHub Releases

- **Status:** Proposed
- **Date:** 2026-10-12
- **Milestone:** M2.7
- **Related:** [ADR-0014](./0014-versions-product-identity-and-product-catalog.md), [architecture.md, offline-first rule](../architecture.md#offline-first-rule), product.md (UX and timing rules)

## Context

Updates must be authenticated (invariant 56, gate 10) even while the installer is unsigned (#36), and the offline-first rule allows network access to one module only.

## Decision

The Tauri updater plugin checks `https://github.com/cunilab/Kivori/releases/latest/download/latest.json` (the latest non-draft, non-pre-release), at start and every 24 h, on by default with a setting. It sends no ids or telemetry (GitHub sees an IP like any download). Every update is verified against the updater public key compiled into the release config before it is installed; the key is separate from Windows code signing, so updates are authenticated (invariant 56, gate 10) even while the installer is unsigned (#36). CI signs the installer with `TAURI_SIGNING_PRIVATE_KEY` and its password (GitHub secrets) and attaches the signature and `latest.json` to the release; the NSIS installer runs in passive mode and the app restarts.

The updater lives in one native module, the only code allowed network access (offline-first rule); the webview gets only the result. Dev builds have no endpoint. UX and timing rules are in product.md.

The firmware offer reuses the existing comparison (`UpdateAdvice`) against the catalog entry for the device's product and board, keyed per device hash and firmware version.

## Consequences

Key custody: losing the updater key means installed apps can never auto-update again (users reinstall by hand), so it is kept offline in two places; rotation ships one update signed with the old key that carries the new public key. Channels or our own server later only change the endpoint, through one update.

**As built (step 4).** `tauri.release.conf.json` sets `bundle.createUpdaterArtifacts: true`. `release.yml` checks for the `TAURI_SIGNING_PRIVATE_KEY` secret (never read on pull requests); without it the build merges `{"bundle":{"createUpdaterArtifacts":false}}`, prints a `::warning::` that the release is not updatable and skips `latest.json`. With it the installer gets a `.sig` and `scripts/updater-manifest.mjs` writes `latest.json` (`version`, `notes`, `pub_date`, `platforms."windows-x86_64"` with `signature` and the stable `releases/download/vX.Y.Z/<installer>` URL). The release is a draft, never a pre-release, and carries the installer, `.sig`, `latest.json` and `SHA256SUMS`. The public key is not in any config yet (step 5). The installed Tauri CLI also offers `requireSignedVersion` in the updater plugin; step 5 should enable it.

**As built (step 5).** `tauri-plugin-updater` 2.13 (no `zip` feature) lives in `src/updater/`; `policy.rs` holds the pure schedule, offer rules and state machine, and `mod.rs` the worker thread, the only code that reaches the network. Deviations and choices:

- `tauri.release.conf.json` carries `plugins.updater` (the endpoint, `pubkey: ""`, `requireSignedVersion: true`, `windows.installMode: "passive"`). The app checks only when it runs on Windows, the section names exactly that endpoint and the key is not empty; otherwise the status is `disabled` (`devBuild`, `noKey`, `badEndpoint`) or `unsupported` (macOS) and the plugin is not even registered, because it requires its config at init. Until the owner sends the public key, every build, release builds included, is `disabled` (`noKey`). The key (id `B53242CE79304D17`) went into the release config on 2026-10-12, so release builds check for updates from then on; dev builds stay `devBuild`. `release.yml` fails a signed release whose release config has no key.
- "At start" is 30 s after start, so startup never waits on the network; then every 24 h while running. A failed check is retried at the next interval, not sooner.
- "Check now" ignores a snooze (the user is asking), never a skip. A scheduled check runs only while nothing is offered, so an unanswered offer is not replaced by a newer release until it is dismissed.
- An update found during a firmware flash or a knob gesture (an open gesture in `InputIngress`, published by the device thread) is offered when both are over; a downloaded update waits the same way before the installer starts.
- The dialog opens by itself once per version per window session; the tray item ("Update to vX.Y.Z…") and the Device page reopen it. Release notes are plain text.
- One command beyond the planned five: `set_auto_update_check` saves the setting. The last check time is per session, not stored.
- The firmware offer ordering ("app update first") is wired in step 6, below.

**As built (step 6).** The firmware offer is a pure decision in `src/firmware_offer.rs`, evaluated by the device thread every tick and published as `FirmwareOfferDto` on `firmware://offer` (`get_firmware_offer`, `accept_firmware_offer`, `dismiss_firmware_offer`). Choices:

- The bundled firmware "for its product and board" is the embedded image whose identity record names the device's product and board; there is one image per build, so any other product, an unknown one, or a device whose `Identity` has not arrived is offered nothing. The `wrongProduct` guard in the flash path is unchanged and still decides.
- "Idle for a few seconds" is 5 s connected with no knob gesture and no flash; a gesture restarts the wait, but does not withdraw an offer already shown.
- App first: `Machine::app_update_due` (an offer, a failed install waiting for "Try again" or "Later", a download, a pending restart, or a release held for an idle device) turns the question into `deferredForAppUpdate`. The tray then shows only "Update to vX.Y.Z…" and the dialog does not open by itself; answering the app update ("Later", "Skip") releases the firmware offer on the next tick. After "Install & restart" the new app decides afresh (validation 14.7).
- "Later" is kept in memory per run, keyed by short device hash, bundled firmware version and connection generation, so it lasts until the device next connects (an app restart is a new connect). Nothing is persisted, so nothing can skip an update for good.
- Required: an incompatible handshake with an older protocol major (or one not read), or a connected device older than the bundled firmware that lacks a capability the catalog requires. Its dialog has "Update now" and "Not now" (closes only; asked again from the tray, and by itself on the next connect). "Update now" for an incompatible device takes the existing restore path. A newer protocol major, like newer firmware, is `updateApp`.
- The dialog auto-opens once per key per window session, like the app update dialog; if an app update takes over while it asks, it closes and opens again after. "Update now" queues the ordinary flash and the dialog follows the firmware status to success or failure. Activity gains `firmwareOfferShown`, `firmwareOfferAccepted` and `firmwareOfferLater` with no metadata (no versions).

## Alternatives considered

- Our own update server now: rejected, GitHub already hosts the binaries.
- An update check in the webview: rejected, breaks the offline guard.
- A modal popup at launch: rejected, Kivori usually starts hidden at login.
