# Contract: Kivori Device Wire Protocol (USB Serial)

**Feature**: `001-device-connection-foundation` | **Date**: 2026-07-17 | **Crate**: `kivori-protocol`

Authoritative contract for the desktop-core ↔ ESP32-C3 link over USB serial. Both ends depend on the
**same** `kivori-protocol` crate so the schema cannot diverge. Recorded as **ADR-0002**.

> Scope: framing, versioning, handshake, message set, negotiation, integrity, and error handling for
> this slice. Wi-Fi/BT/OTA and integration payloads are out of scope for Feature 001.

## 1. Physical layer

- Transport: USB Serial/JTAG CDC-ACM (VID:PID `0x303A:0x1001`), treated as a byte-stream COM port. Baud
  is ignored by the device (runs at USB speed).
- Constraints: 64-byte endpoint FIFO per direction; device TX may stall if the host is not draining.
  → All frames are small (semantic, never pixels); writes are bounded and non-blocking.

## 2. Framing

Byte-stream framing uses **COBS** with a `0x00` delimiter. One wire packet = `COBS(frame) || 0x00`.

Decoded **frame** layout (all multi-byte fields **little-endian**):

| Offset | Field | Type | Notes |
|---:|---|---|---|
| 0 | `magic` | `u16` | `0x4B56` ("KV") |
| 2 | `ver_major` | `u16` | sender protocol major |
| 4 | `ver_minor` | `u16` | sender protocol minor |
| 6 | `seq` | `u16` | sequence number (wraps) |
| 8 | `payload_len` | `u16` | payload length; MUST be ≤ `MAX_PAYLOAD` (512) |
| 10 | `payload` | `[u8; payload_len]` | postcard-encoded `Message` |
| 10+len | `crc32` | `u32` | CRC-32 (IEEE) over header + payload |

`MAX_PAYLOAD = 512` bounds firmware buffers. Firmware reassembles frames across the USB FIFO using a bounded COBS accumulator.

## 3. Message set

`Message` is an **append-only** enum because postcard variant index is the wire tag. New compatible variants are appended within a major version and optional behavior is capability-gated. See [Feature 001 research R-6](../research.md#r-6-protocol-payloads-serde--postcard-in-one-shared-protocol-crate).

| Tag | Message | Dir | Payload (logical) |
|---:|---|---|---|
| 0 | `Hello` | D→V | `{ desktop_version, desktop_caps, nonce }` |
| 1 | `HelloAck` | V→D | `{ device_caps, device_id, firmware_version, nonce_echo }` |
| 2 | `Ready` | D→V | `{ negotiated_minor, negotiated_caps }` |
| 3 | `Bye` | both | `{ reason }` |
| 4 | `SetState` | D→V | `{ desired: SendableState, at_ms }` |
| 5 | `StateReport` | V→D | `{ reported: CompanionState, elapsed_ms }` |
| 6 | `Ping` | D→V | `{ t_ms }` |
| 7 | `Pong` | V→D | `{ t_ms_echo, uptime_ms }` |
| 8 | `Health` | V→D | safe health fields |
| 9 | `Diagnostic` | V→D | `{ category, code }` |
| 10 | `Error` | both | `{ category, code }` |
| 11 | `PlayMascotAction` | D→V | `{ action, personality, seed }` — capability `MASCOT_INTERACTION` (Feature 003) |
| 12 | `MascotActionApplied` | V→D | `{ action, personality, seed, applied_at_ms }` — capability `MASCOT_INTERACTION` (Feature 003) |
| 13 | `InputEvent` | V→D | `{ session, gesture_id, control, kind, device_ms }` — capability `PHYSICAL_INPUT_V1` (Slice 002) |
| 14 | `Presentation` | D→V | `{ session, revision, primary, value, transient_ms }` — capability `PRESENTATION_V1` (Slice 002) |

Frame-version fields are outside the postcard payload so major compatibility can be checked before decoding an incompatible payload.

Tags 11 and 12 were appended by Feature 003 (mascot, PR #3); tags 13 and 14 by Slice 002
(`docs/features/002-rotary-volume-control/`); tags 0–10 are unchanged. Slice 002's tags are capability-gated (§5) and both carry `session`, the connection-scoped session
identity described in [§4.1](#41-nonce-as-connection-scoped-session-identity) below — an unnegotiated
peer never sends or acts on either message. `InputEvent.control` is `ControlId::Rotary` (extensible);
`InputEvent.kind` is one of `GestureStarted`, `Detent(Direction)`, `GestureEnded`. `Presentation.value`
is an `Option<ValueDisplay>` carrying `{ kind, current_percent, confidence, at_boundary }`, where
`confidence` is one of `Preview` (Kivori's local target, not yet observed from the OS), `Confirmed`
(the OS reported this value back), or `Unverified` (the set was dispatched but read-back failed) —
see [Feature 002's architecture record](../../002-rotary-volume-control/architecture.md#3-protocol--two-appended-variants)
for the full type definitions.

## 4. Handshake

```mermaid
sequenceDiagram
    participant D as Desktop
    participant V as Device
    Note over D: port opened → Connecting
    D->>V: Hello { desktop_version, desktop_caps, nonce }
    V-->>D: HelloAck { device_caps, device_id, firmware_version, nonce_echo }
    alt nonce mismatch OR unsupported major
        D-->>V: Bye
        Note over D: Incompatible or Error
    else compatible
        D->>V: Ready { negotiated_minor, negotiated_caps }
        Note over D: Connected
        D->>V: SetState { desired: Idle }
        V-->>D: StateReport { reported: Idle, elapsed_ms }
    end
```

- Identity is established only after a well-formed `HelloAck` matching the handshake nonce. VID/PID filtering alone is not Kivori identity verification.
- If `HelloAck` does not arrive before `HANDSHAKE_TIMEOUT`, the attempt fails, backs off, and returns to discovery/retry behavior.

### 4.1 Nonce as connection-scoped session identity

**Widened role, recorded by Slice 002.** The handshake `nonce` in `Hello`/`HelloAck` was originally
only a liveness check: proof that the device answered *this* `Hello`, generated by the desktop as a
counter starting at 1 and incrementing per attempt, reinitialized on every process start. Slice 002
widens its role: the same nonce is now **connection-scoped session identity**, reused (not replaced
by a new message or field) to give `InputEvent` and `Presentation` a way to reject stale traffic
across a disconnect/reconnect.

The mechanics:

1. The desktop mints a session nonce and sends it in `Hello`, exactly as it already did for liveness.
2. Firmware stores the nonce of the `Hello` it accepted and stamps **every** `InputEvent` with it.
3. The desktop rejects any `InputEvent` whose `session` does not match the nonce it sent in the
   current connection's `Hello`.
4. Symmetrically, the desktop stamps every `Presentation` with the same nonce, and firmware rejects
   any `Presentation` that does not match its currently accepted session.
5. Firmware clears input state and the accepted-revision high-water mark on accepting a new `Hello`,
   and on `Bye`/link loss. The desktop clears its open-gesture tracking on every exit from
   `Connected`.
6. `Presentation.revision` is strictly increasing **within a session** and resets when a new session
   is accepted — session-scoped, not device-lifetime-scoped. A freshly restarted desktop begins at
   revision 1 again and is not mistaken for stale traffic against a previous process's much higher
   revision number.

**Required consequence: the nonce MUST be unique across desktop process restarts, not only within a
process.** A pure per-process counter starting at 1 is no longer sufficient — two different desktop
processes could otherwise mint the same session identity by coincidence, and a stale event stamped
with an old process's `1` could match a new process's `1`. The implemented fix replaces the counter
outright: `OsNonceSource` (`apps/desktop/src-tauri/src/device/nonce.rs`) draws a fresh `u32` directly
from OS randomness via `getrandom::getrandom()` on every handshake attempt. No counter is retained or
combined with it. This did add one new direct dependency, `getrandom` (`apps/desktop/src-tauri/Cargo.toml`)
— acceptable because it was already present transitively in the lockfile and is not a network client,
so it does not trip `scripts/check-offline-deps.sh`'s banned-crate list.

**This is a freshness token, not a security credential.** The nonce distinguishes "traffic belonging
to this connection" from "traffic belonging to a previous one" well enough to satisfy no-stale-replay
(user-story-contract invariant 5). It does not authenticate the desktop, is not secret, and must not
be relied on for any access-control purpose.

This does not change §6's sequence policy (duplicate/gap/wrap classification remains a
per-frame concern) or §7's heartbeat constants; both are orthogonal to session identity. See
[ADR-0006](../../adr/0006-handshake-nonce-as-session-identity.md) for the full decision record and
[Feature 002's architecture](../../002-rotary-volume-control/architecture.md#4-connection-scoped-session-identity)
for the implementation.

## 5. Version and capability negotiation

- Compatible iff the device major is supported by Desktop.
- Unsupported major → `Incompatible`; normal state commands are not sent.
- `negotiated_minor = min(desktop_minor, device_minor)`.
- `negotiated_caps = desktop_caps & device_caps`.
- Unknown/unnegotiated capabilities must not activate behavior.

| Desktop | Device | Result |
|---|---|---|
| same supported major | same major | compatible; negotiate minor + caps |
| unsupported major | other major | incompatible |
| minor differs | same major | compatible additive behavior only |
| cap set on one peer only | unset on other | feature disabled |

### Allocated capability bits

`Capabilities` is a `u32` bitset (`crates/kivori-model/src/capabilities.rs`). Bits are allocated
centrally and never reused once retired.

| Bit | Capability | Meaning |
|---:|---|---|
| 0 | `MASCOT_INTERACTION` | the device accepts `PlayMascotAction` and acknowledges with `MascotActionApplied` (tags 11/12, Feature 003) |
| 1 | `PHYSICAL_INPUT_V1` | the device may emit `InputEvent` (tag 13, Slice 002) |
| 2 | `PRESENTATION_V1` | the device renders `Presentation` (tag 14, Slice 002) |

All bits follow the same negotiation rule as any other capability: `negotiated_caps = desktop_caps &
device_caps`, and a bit set on only one peer leaves the corresponding behavior off rather than
activating it unilaterally. Feature 001 shipped with zero capability bits allocated; Feature 003 took bit 0 and Slice 002 bits 1–2.

## 6. Sequence numbers and integrity

Each outbound frame carries an incrementing wrapping `u16` sequence number. USB CDC already provides ordered transport, so sequence tracking is for duplicate/gap diagnostics and side-effect suppression, not generic retransmission.

- Duplicate sequence: do not re-apply side effects; record diagnostic evidence.
- Gap: accept the newer valid frame and record the gap; do not invent/retransmit missing ordinary input/state.
- Wraparound `0xFFFF → 0x0000`: valid continuation.
- CRC failure: drop/count, never dispatch.
- `Pong.t_ms_echo` correlates the matching ping.

This sequence policy is **not** sufficient for a future firmware-image transfer protocol; update transfer requires its own explicit transaction/offset/ack semantics if application-level flashing is selected.

## 7. Heartbeat and liveness

Feature 001 defines ping/pong support and heartbeat policy constants. The production Desktop wiring must be verified before relying on heartbeat timeout as a complete product-health mechanism; current product-wide technical research tracks that as an implementation evidence item.

Initial constants:

| Name | Value |
|---|---:|
| `HANDSHAKE_TIMEOUT` | 1000 ms |
| `PING_INTERVAL` | 1000 ms |
| `HEARTBEAT_MISSES` | 3 |

## 8. Malformed-frame and resync handling

The decoder must safely reject malformed input without panic/hang/mis-dispatch:

- COBS failure or overlong accumulation;
- wrong magic;
- unexpected/unsupported header version;
- payload length over bound;
- truncated frame;
- CRC mismatch;
- postcard decode failure/unknown variant;
- message invalid for the current session phase.

The decoder must continue making forward progress and resynchronize at future frame boundaries.

## 9. Initial protocol constants

| Name | Value | Meaning |
|---|---|---|
| `PROTOCOL_MAJOR` | 1 | current major |
| `PROTOCOL_MINOR` | 2 | current minor (1 = Feature 003 mascot, 2 = Slice 002) |
| `MAGIC` | `0x4B56` | frame magic |
| `MAX_PAYLOAD` | 512 | max postcard payload bytes |
| `HANDSHAKE_TIMEOUT` | 1000 ms | handshake deadline |
| `PING_INTERVAL` | 1000 ms | heartbeat period |
| `HEARTBEAT_MISSES` | 3 | configured miss threshold |

## 10. Verification

Feature 001 maintains automated tests for:

- round-trip encode/decode of message variants;
- header/framing/COBS/CRC/payload-length validation;
- version/capability negotiation;
- duplicate/gap/wrap sequence classification;
- malformed/random byte handling with no panic and forward progress;
- firmware/host-simulation consumption of the shared contract.

See `crates/kivori-protocol/tests/` and `tests/e2e-host-sim/` for current executable evidence.
