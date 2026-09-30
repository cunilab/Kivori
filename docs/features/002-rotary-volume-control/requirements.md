# Feature Specification: Rotary Volume Control (Slice 002)

**Feature Branch**: `docs/product-prd-us-contract` (Slice 002 of the running product build; no dedicated
Spec Kit feature branch was created for this slice)

**Created**: 2026-09-17

**Status**: Software complete; physical validation outstanding (see [`validation-checklist.md`](./validation-checklist.md))

**Design authority**: [`../../superpowers/specs/2026-09-17-rotary-volume-control-loop-design.md`](../../superpowers/specs/2026-09-17-rotary-volume-control-loop-design.md)
(binding; this document restates its scope for the feature record rather than re-deciding it)

**Product contract**: [`../../product/prd.md`](../../product/prd.md),
[`../../product/user-story-contract.md`](../../product/user-story-contract.md)

**Foundation**: [`../001-device-connection-foundation/`](../001-device-connection-foundation/)

## Goal

Deliver the smallest real physical end-to-end proof of the Kivori product loop:

```
HW-040 A/B  →  validated logical detents  →  protocol  →  Windows master volume
            →  observed Core Audio state  →  StateConfirmed  →  Kivori volume presentation
```

Turning the physical knob changes Windows default-output master volume, and the device displays the
volume Windows actually reports. Feature 001 proved the desktop and device can find each other and
trust each other; Slice 002 is the first slice that proves the product's core thesis — *control the
desktop physically, understand the desktop visually* — rather than only establishing a link.

## Scope

### In scope

- HW-040 A/B decoding to validated logical detents in firmware, with a legal electrical transition
  distinguished from a completed mechanical detent (contract invariant 46).
- Rotary gesture formation and the 250 ms gesture-end boundary in firmware.
- Two capability-gated protocol variants: device→desktop `InputEvent` (tag 13), desktop→device
  `Presentation` (tag 14).
- The first two allocated protocol capability bits: `PHYSICAL_INPUT_V1`, `PRESENTATION_V1`.
- Connection-scoped session identity on both new messages, reusing the handshake nonce, enforcing
  no-stale-replay (contract invariant 5) and scoping presentation revisions to the session.
- Desktop input ingress with session and gesture staleness rejection.
- A single Global binding: `Rotate → Master Volume`, fixed 1× step (no acceleration in this slice).
- Minimum capability / availability / confirmation types — extensible, not generic.
- Explicit `Preview` / `Confirmed` / `Unverified` value confidence on the wire and in the rendered
  output (contract invariant 3 — acknowledgement is not confirmation).
- A narrow Windows Core Audio backend behind a `VolumeBackend` trait, with a deterministic fake used
  by every host-testable suite.
- Default render-endpoint rebinding via `IMMNotificationClient`, and a Kivori event-context GUID so
  Kivori's own writes are distinguishable from external changes.
- Preview ownership during a gesture and reconciliation to confirmed state at gesture end (contract
  invariants 1 and 30 — desktop truth wins; continuous gestures reconcile at gesture boundaries).
- A deterministic shared volume overlay in `kivori-renderer`, golden-frame covered per confidence.
- The physical HW-040 adapter, with CLK/DT/SW pin mapping isolated in the profile layer
  (`firmware/esp32-c3/src/profile.rs`).

### Out of scope

Acceleration and the 5× ceiling; master mute; the push-switch gesture FSM, Hold semantics, and
recovery ownership/arbitration; heartbeat rewiring; profiles/ContextEngine; DeviceRegistry and
multi-device; ExecutionTracker; ConfigStore; UpdateCoordinator; SessionOwnershipGate; macros; any
second OS backend; display-idle/ambient policy; per-application audio sessions; the
`eCommunications` device role.

### Deferred-scope consequences, recorded honestly

- With a fixed 1× step there is no acceleration multiplier state, so contract invariants 15, 16, and
  34 (reversal resets acceleration, acceleration accumulates only across same-direction detents, 5×
  ceiling) are **vacuously satisfied** in this slice — there is nothing for them to constrain yet.
  They become live behavior only when a later slice introduces a multiplier. `InputEvent` already
  carries per-detent `device_ms` so that slice needs no protocol change.
- Contract invariants 33, 40, 44, and 52 (recovery arbitration, release-qualified short press,
  release-qualified Hold, device-wide single-gesture ownership) are **not proven** by this slice
  because it has only one control (the rotary knob; the push-switch is wired but unread). They belong
  to the push-switch/recovery slice.
- The contract's boundary rule (US1 *Bounded values*) is implemented for clamping and
  repeat-suppression; the optional subtle boundary reaction is represented in the wire type
  (`at_boundary`) and rendered minimally (a solid track outline), not as a distinct animation.

## Contract rules this slice implements

Restated from [`user-story-contract.md`](../../product/user-story-contract.md) §3 (Global Invariants)
and US1, as the specific rules Slice 002 is accountable for:

| Rule | Statement | Where enforced |
|---|---|---|
| Invariant 1 | Desktop truth wins over local prediction | `GestureValue::on_input` reconciles to backend read-back at `GestureEnded`; `presentation/mod.rs` |
| Invariant 2 | Observable truth only — no invented state | `VolumeBackend::set` returns the OS read-back, never the request; `RuntimeUnavailable` when no endpoint exists |
| Invariant 3 | Acknowledgement is not confirmation | `ValueConfidence::{Preview, Confirmed, Unverified}` on the wire and in the renderer |
| Invariant 5 | No stale replay | Connection-scoped session nonce on `InputEvent`/`Presentation`; `InputIngress::accept` |
| Invariant 30 | Continuous gestures reconcile at gesture boundaries | `GestureValue` ignores external changes while a gesture is open; applies them at `GestureEnded` |
| Invariant 42 | Only observed hardware input is actionable | `QuadratureDecoder` never reconstructs unobserved detents |
| Invariant 46 | Input conditioning precedes gesture semantics | `QuadratureDecoder` (electrical) below `RotaryGesture`/gesture semantics (logical) |
| Invariant 54 | A normal rotary binding owns both directions | `resolve_binding` maps `ControlId::Rotary` to one `ActionId::MasterVolume` for both `Cw`/`Ccw` |
| US1 *Bounded values* | Clamp immediately; first reverse tick takes effect immediately; no repeat feedback into a boundary | `action::volume::apply_step` |
| §5 timing reference | 250 ms gesture-end inactivity; 800 ms value-transient display | `RotaryGesture::new(250)`; `VALUE_TRANSIENT_MS = 800` |

Invariants 15, 16, 33, 34, 40, 44, and 52 are explicitly **not** exercised by this slice; see
"Deferred-scope consequences" above.

## Independent test

With the physical ESP32-C3 + HW-040 wired and Kivori Desktop running on Windows: turning the knob one
detent changes Windows master volume by the fixed step in the corresponding direction, and the
device's volume overlay reflects the value Windows reports — not merely the value Kivori requested.
Unplugging mid-gesture and replugging must not execute the interrupted gesture. This is recorded in
[`validation-checklist.md`](./validation-checklist.md); it has **not** been executed as of this
writing.

## Spec deviations

Recorded here so review can reject them rather than discover them (carried from the task brief):

1. **`InputTuning` struct not created.** The design gathered the tuning parameters into one struct.
   With only a handful of values spread across two crates and a firmware module, a struct would add
   indirection without changing behavior. The parameters are instead named constants at their point
   of use plus the single documented table in [`architecture.md`](./architecture.md#input-tuning-parameters),
   which preserves the intent — these are measured parameters, not settled constants — at lower cost.
   Revisit when a third consumer needs them together.
2. **`ValueConfidence::Preview` is reported even for a write the backend confirmed.** During an open
   gesture the value is still a gesture-local target that the final read-back may revise, so
   promoting it to `Confirmed` mid-gesture would be the exact over-claim the field exists to prevent.
   Promotion happens once, at gesture end (`GestureValue::on_input`, the `GestureEnded` arm).

## Out of scope for this record

This document does not restate Feature 001's requirements (device discovery, handshake, companion
states, Device Studio) — see [`../001-device-connection-foundation/requirements.md`](../001-device-connection-foundation/requirements.md).
It also does not claim physical hardware validation; see the status line above and
[`validation-checklist.md`](./validation-checklist.md).
