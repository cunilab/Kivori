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

## Alternatives considered

- Our own update server now: rejected, GitHub already hosts the binaries.
- An update check in the webview: rejected, breaks the offline guard.
- A modal popup at launch: rejected, Kivori usually starts hidden at login.
