# ADR-0009: macOS now-playing observation is layered: MediaRemote adapter, then AppleScript, then unknown

- **Status:** Accepted
- **Date:** 2026-10-03
- **Related:** [ADR-0005](./0005-typed-activity-log-with-allowlist.md)

## Context

macOS has no public system-wide now-playing API: `MPNowPlayingInfoCenter` only publishes, and the private MediaRemote framework has refused unentitled callers since 15.4.

## Decision

Kivori therefore uses three layers.

1. Run the BSD-3 [ungive/mediaremote-adapter](https://github.com/ungive/mediaremote-adapter), copied into `apps/desktop/src-tauri/vendor/` at a pinned commit and compiled from source by `build.rs` with plain clang (no prebuilt binaries, no extra build tool, no network; a missing toolchain is a warning, not a failed build). Apple's own entitled `/usr/bin/perl` loads the helper and streams JSON lines, read by a `kivori-media` thread that owns the long-running child. Each start first runs the adapter's `test` command, so an empty payload, once it has lasted 1 s, is an honest system-wide Stopped.
2. While the adapter is unavailable (test fails, process dies, unexpected output) Kivori retries it with backoff (2 s doubling to 60 s) and meanwhile polls Spotify and Music through public Apple Events (`osascript` with a 3 s timeout, never launching either app, one-time Automation prompt). Because AppleScript cannot see browsers, finding nothing there is unknown, never Stopped.
3. Otherwise status and title are `None`.

Titles and artists are never logged (ADR-0005).

## Consequences

Accepted risk: a private framework reached through an Apple binary that Apple may remove or lock down; then Kivori degrades to layer 2 or unknown and never reports a guessed state. Release builds must bundle and sign the helper (roadmap M3).

## Alternatives considered

- The Rust wrapper crates: rejected, they ship prebuilt binaries, need a Swift toolchain, or are barely maintained.
- AppleScript alone: rejected, no browsers.
