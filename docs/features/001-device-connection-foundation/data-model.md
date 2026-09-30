# Data Model: Device Connection Foundation

**Feature:** `001-device-connection-foundation`  
**Original model date:** 2026-07-17  
**Reconciled:** 2026-09-16

This document records the durable Feature 001 domain/wire concepts. It is intentionally scoped to the implemented Device Connection Foundation and must not be treated as the complete future Kivori product-state model. Current product-wide research recommends additional orthogonal state domains as new user stories are implemented.

Wire encoding lives in [`contracts/protocol.md`](./contracts/protocol.md); native/webview IPC shapes live in [`contracts/ipc.md`](./contracts/ipc.md); the built-system overview is [`architecture.md`](./architecture.md).

## 1. Companion state

**Canonical crate:** `kivori-model`

```rust
enum CompanionState {
    Booting,
    Idle,
    Happy,
    Busy,
    Sleeping,
    Offline,
}

enum SendableState {
    Idle,
    Happy,
    Busy,
    Sleeping,
}
```

`SendableState` is the desktop-commandable subset. `Booting` and `Offline` are device-originated lifecycle presentation states.

Feature 001 invariants:

- Desktop state commands carry only `SendableState`.
- Device Studio may preview all six `CompanionState` values.
- Firmware owns transition into `Booting`/`Offline`.
- The current product architecture may later add separate takeover, execution, assignment, permission, display-power, and update domains rather than extending this enum indefinitely.

## 2. Connection lifecycle

**Canonical ownership:** `kivori-model` for public state; Desktop native core for transition logic.

```rust
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Incompatible,
    Error,
}
```

This is distinct from companion presentation state. A transport/session condition must not be conflated with what the device is visually rendering.

Typical transition inputs include discovery, handshake success/failure, I/O loss, heartbeat/session timeout, removal, and reconnect backoff. The implementation lives in `apps/desktop/src-tauri/src/device/` and its tests.

## 3. Protocol version and capabilities

**Canonical crates:** `kivori-model` + `kivori-protocol`

```rust
struct ProtocolVersion {
    major: u16,
    minor: u16,
}

struct Capabilities(u32);
```

Feature 001 compatibility rules:

- unsupported major version → incompatible;
- compatible peers negotiate a usable minor level;
- capabilities use intersection semantics;
- optional behavior must be capability-gated;
- current postcard message-enum ordering is wire-significant, so compatible evolution is append-only unless a new protocol major deliberately changes semantics.

See [ADR-0002](../../adr/0002-wire-protocol.md).

## 4. Desktop session state

The Desktop runtime keeps connection/session state in memory rather than persisting the active link across process restarts.

Conceptually:

```rust
struct ConnectionSession {
    connection: ConnectionState,
    desired: SendableState,
    reported: Option<CompanionState>,
    device: Option<DeviceIdentity>,
    negotiated_version: Option<ProtocolVersion>,
    negotiated_caps: Capabilities,
    retry_count: u32,
}
```

Important invariants:

- `desired` defaults to `Idle` on a fresh Desktop process;
- reconnect within the same running session may re-send current desired state;
- `reported` is observed device state and does not silently overwrite desktop intent;
- connection, desired, and reported state remain distinct in IPC;
- ordinary Feature 001 session state is not a durable user configuration database.

## 5. Device identity

Feature 001 currently carries an opaque 16-byte device identifier in the handshake:

```rust
struct DeviceIdentity {
    opaque_id: [u8; 16],
    firmware_version: SemVerLite,
    protocol_version: ProtocolVersion,
}
```

The physical Feature 001 firmware currently uses a fixed identifier, which is sufficient only for the single-device foundation. Product-wide technical research requires a real stable per-device `DeviceId` plus an immutable recovery-oriented `HardwareId` before multi-device assignment/update targeting is implemented.

Raw identity must not be emitted in ordinary diagnostics; Feature 001 uses a short derived value only for safe correlation.

## 6. Rendering timeline

**Canonical crate:** `kivori-model`

Canonical renderer input is integer elapsed milliseconds:

```rust
type ElapsedMs = u32;
```

Device Studio uses an integer inspection step and derives elapsed time rather than cumulatively adding a floating-point delta. This keeps deterministic replay and host/device parity testable.

See [ADR-0003](../../adr/0003-timing-model.md).

## 7. Scene and asset model

**Canonical crates:** `kivori-model`, `kivori-assets`, `kivori-renderer`

A scene is a semantic visual definition associated with companion state and compiled assets. Rendering consumes integer geometry/color/timeline data and RGB565 runtime assets; source SVG/PNG files are not runtime inputs.

Conceptually:

```rust
struct Scene {
    id: CompanionState,
    fps: FrameRate,
    frame_count: u16,
    layers: &'static [Layer],
    background: Rgb565,
}

struct FrameRate {
    num: u16,
    den: u16,
}
```

The exact Rust structures evolve with the renderer, but these Feature 001 invariants remain:

- host preview and firmware consume the same semantic scene/runtime asset model;
- identical declared inputs produce deterministic logical RGB565 output;
- scene coordinates are device-profile-relative integers;
- asset references are resolved and validated against the compiled blob.

## 8. Device profile and framebuffer

**Canonical crates:** `kivori-model` + `kivori-framebuffer`

The verified physical profile is 240x240 RGB565 on ST7789. The profile also describes controller/display geometry and tile behavior used by the renderer/display adapter.

Firmware renders into caller-owned tile/band storage rather than requiring a permanent full-frame framebuffer. Tile signatures/content hashes allow unchanged output regions to avoid unnecessary physical writes.

Physical controller/pin/geometry facts are recorded in [`validation-checklist.md`](./validation-checklist.md).

## 9. Compiled asset manifest

**Canonical crate:** `kivori-assets`; producer: `tools/asset-compiler`.

Runtime assets include a versioned manifest plus deterministic RGB565 bitmap/font/scene payloads. Bounds and references must be validated when reading the blob; CI/golden evidence detects unintended visual/toolchain changes.

See [ADR-0004](../../adr/0004-asset-format.md).

## 10. Protocol messages

**Canonical crate:** `kivori-protocol`

The Feature 001 message enum currently includes:

```text
Hello
HelloAck
Ready
Bye
SetState
StateReport
Ping
Pong
Health
Diagnostic
Error
PlayMascotAction    -- tag 11, Feature 003, capability MASCOT_INTERACTION
MascotActionApplied -- tag 12, Feature 003, capability MASCOT_INTERACTION
InputEvent      -- tag 13, Slice 002, capability PHYSICAL_INPUT_V1
Presentation    -- tag 14, Slice 002, capability PRESENTATION_V1
```

Inbound frames are validated for framing, payload length, CRC and sequence classification before semantic dispatch. Malformed input must be rejected without panicking. Future messages are added only under the protocol/version/capability rules described above.

### Slice 002 `kivori-model` types

Added to `kivori-model` (`src/input.rs`, `src/presentation.rs`) and nested inside `InputEvent`/
`Presentation`. Wire-significant like every other type nested in a `kivori-protocol` message: variant
index is part of the postcard encoding, so these are append-only from this point forward.

```rust
// kivori_model::input
enum Direction { Cw, Ccw }
struct InputLevels { a: bool, b: bool, sw: bool }   // not wire-carried; firmware-internal sampling only

// kivori_model::presentation
enum PrimaryState    { Idle, Active, Error, Unknown }
enum ValueKind        { Volume }
enum ValueConfidence  { Preview, Confirmed, Unverified }
struct ValueDisplay {
    kind: ValueKind,
    current_percent: u8,
    confidence: ValueConfidence,
    at_boundary: bool,
}
```

`ValueConfidence` exists so an optimistic local preview can never be rendered as observed desktop
truth (Feature 001's data model has no equivalent — Feature 001 never carried a continuous, externally
observable value). See [Feature 002's architecture record](../002-rotary-volume-control/architecture.md#3-protocol--two-appended-variants)
for the full rationale.

**Presentation revisions are session-scoped, not device-lifetime-scoped.** `Presentation.revision`
(a `u32`, carried alongside these types but not itself a `kivori-model` type) is strictly increasing
only *within* the session identified by `Presentation.session` (see §3's capability bits and
[`contracts/protocol.md` §4.1](./contracts/protocol.md#41-nonce-as-connection-scoped-session-identity)
for what "session" means here) and resets to 1 whenever a new session is accepted. A restarted
desktop process is therefore never mistaken for a stale sender merely because its revision counter
started low again — the comparison is always against the current session's own high-water mark, not
against any prior session's.

## 11. Safe diagnostics

**Canonical ownership:** Desktop diagnostics module plus compact firmware diagnostic messages.

Safe diagnostics carry only allowlisted metadata such as state/category/message kind/length/sequence/retry timing and a short identity correlation value. They do not carry raw payloads, raw device identity, secrets, or unnecessary user/machine paths.

See [`activity-log.md`](../../activity-log.md) and [ADR-0005](../../adr/0005-logging-policy.md).

## Scope boundary

This model documents what Feature 001 established. It does **not** imply that future product concepts such as execution jobs, permissions, OS session ownership, multi-device assignment, display power, update phases, or presentation priority should be folded into `ConnectionState` or `CompanionState`. Those are separate product domains described in [`../../research/technical-research.md`](../../research/technical-research.md).
