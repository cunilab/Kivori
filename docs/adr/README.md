# Architecture decision records

An architecture decision record (ADR) is one short file that captures one decision: the context, what was decided, what follows from it, and the options that were rejected. Numbers are never reused or renumbered. A superseded ADR stays in place with the status "Superseded by ADR-XXXX". To add one, copy [`template.md`](./template.md), take the next number, and keep the status Proposed until the change is merged.

| ADR | Title | Status | Date |
|---|---|---|---|
| [ADR-0001](./0001-two-cargo-workspaces.md) | Two cargo workspaces | Accepted | 2026-08-04 |
| [ADR-0002](./0002-one-wire-protocol-crate.md) | One wire protocol crate | Accepted | 2026-08-04 |
| [ADR-0003](./0003-integer-millisecond-timebase.md) | Integer-millisecond timebase | Accepted | 2026-08-04 |
| [ADR-0004](./0004-layered-scenes-and-compiled-asset-blob.md) | Layered scenes and a compiled asset blob | Accepted | 2026-08-04 |
| [ADR-0005](./0005-typed-activity-log-with-allowlist.md) | Typed activity log with an allowlist | Accepted | 2026-08-04 |
| [ADR-0006](./0006-handshake-nonce-is-session-identity.md) | The handshake nonce is the session identity | Accepted | 2026-09-30 |
| [ADR-0007](./0007-interrupt-captured-input-edges.md) | Interrupt-captured input edges | Accepted | 2026-10-02 |
| [ADR-0008](./0008-desk-state-as-two-new-messages.md) | Desk state as two new messages | Accepted | 2026-10-02 |
| [ADR-0009](./0009-macos-now-playing-layered-observation.md) | macOS now-playing observation is layered: MediaRemote adapter, then AppleScript, then unknown | Accepted | 2026-10-03 |
| [ADR-0010](./0010-v1-controller-with-native-usb.md) | v1 controller with native USB | Proposed | 2026-10-11 |
| [ADR-0011](./0011-mic-mute-every-capture-device.md) | Mic mute mutes every active capture device | Accepted | 2026-10-11 |
| [ADR-0012](./0012-idle-life-runs-in-the-firmware.md) | Idle life runs in the firmware | Accepted | 2026-10-11 |
| [ADR-0013](./0013-device-internals-on-request.md) | Device internals on request | Accepted | 2026-10-11 |
| [ADR-0014](./0014-versions-product-identity-and-product-catalog.md) | Versions, product identity and the product catalog | Proposed | 2026-10-12 |
| [ADR-0015](./0015-app-updates-through-github-releases.md) | App updates through GitHub Releases | Proposed | 2026-10-12 |
| [ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md) | Locking a board to its firmware, in tiers | Proposed | 2026-10-12 |
