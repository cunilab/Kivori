# Slice 002 architecture: the rotary volume control loop

A built-system overview of Slice 002, layered on the [Feature 001 foundation](../001-device-connection-foundation/architecture.md).
Design rationale and the candidates evaluated below live in the binding design spec:
[`docs/superpowers/specs/2026-09-17-rotary-volume-control-loop-design.md`](../../superpowers/specs/2026-09-17-rotary-volume-control-loop-design.md).
Requirements and the contract rules this slice implements are in [`requirements.md`](./requirements.md).

This document describes what was **built**, not what remains to be physically proven — see
[`validation-checklist.md`](./validation-checklist.md) for the outstanding physical evidence.

## The chain

```
HW-040 A/B/SW pins
  → InputSource port (firmware, hardware-neutral)              firmware/esp32-c3/src/ports.rs
  → QuadratureDecoder (electrical → validated logical detent)  firmware/esp32-c3/src/input/quadrature.rs
  → RotaryGesture (detent → gesture identity + 250 ms boundary) firmware/esp32-c3/src/input/gesture.rs
  → InputEvent (capability-gated, session-stamped)              crates/kivori-protocol/src/message.rs (tag 13)
  → wire (COBS/CRC/postcard, ADR-0002)
  → Session::pump decode + InputIngress (session/gesture freshness) apps/desktop/src-tauri/src/{device/session.rs, input/mod.rs}
  → resolve_binding (single Global binding: Rotary → MasterVolume) apps/desktop/src-tauri/src/action/mod.rs
  → apply_step (fixed 1× step, clamp, boundary flag)            apps/desktop/src-tauri/src/action/volume.rs
  → GestureValue (preview ownership + reconciliation)           apps/desktop/src-tauri/src/action/gesture_value.rs
  → VolumeBackend (Windows Core Audio; deterministic fake)      apps/desktop/src-tauri/src/platform/{windows/mod.rs, mod.rs}
  → PresentationResolver (ProductSnapshot → Presentation)       apps/desktop/src-tauri/src/presentation/mod.rs
  → Presentation (capability-gated, session-stamped, revisioned) crates/kivori-protocol/src/message.rs (tag 14)
  → wire
  → firmware dispatch: expire `value` after transient_ms, fall back to `primary`  firmware/esp32-c3/src/proto.rs, runtime.rs
  → render_volume_overlay (shared, deterministic, confidence-distinct)  crates/kivori-renderer/src/overlay.rs
  → physical panel / Device Studio preview (same renderer, same output)
```

Every stage above the firmware `InputSource` port and below the desktop `VolumeBackend` trait is a
pure function or a pure state machine, host-testable without hardware — the genuine hardware
unknowns (bounce duration, sample rate, wiring continuity) live entirely in the two adapters at the
ends of the chain: `PhysicalRotary` (firmware) and the Windows Core Audio backend (desktop).

### 1. Firmware input, behind a port

A fourth hardware-neutral port joins `Clock`, `Transport`, and `DisplaySink` (`firmware/esp32-c3/src/ports.rs`):

```rust
pub trait InputSource {
    fn sample(&mut self) -> InputLevels;   // { a, b, sw }
}
```

`PhysicalRotary` (`firmware/esp32-c3/src/physical_rotary.rs`) does nothing but read and invert the
three HW-040 pins every call — no debounce, no edge detection, no state. `sw` is sampled and carried
but unused in this slice; the push-switch gesture machine is a later slice.

### 2. Quadrature decoding — a legal transition is not a detent

`QuadratureDecoder` (`firmware/esp32-c3/src/input/quadrature.rs`) is a pure Gray-code state machine.
It accumulates legal quarter-step transitions and emits a `Direction` only once
`QUARTER_STEPS_PER_DETENT` (4) quarter-steps have completed in the same rotational sense — bounce
that walks between adjacent Gray states and back without completing a detent produces nothing.
Electrically impossible transitions (both bits changing at once) are counted as `invalid_transitions`
and reset the accumulator; they are a diagnostic, never a source of motion (contract invariant 46).

`RotaryGesture` (`firmware/esp32-c3/src/input/gesture.rs`) consumes validated detents plus the clock.
A gesture opens on the first detent (`gesture_id`, session-unique, `wrapping_add`, never 0) and
closes when `poll` observes `gesture_end_ms` (250 ms) of inactivity — emitted exactly once per
gesture as `GestureEnded`. A direction reversal never splits a gesture.

### 3. Protocol — two appended variants

Tags 0–12 are untouched (11/12 belong to Feature 003's mascot messages); Slice 002 takes capability
bits 1 and 2 after Feature 003's `MASCOT_INTERACTION` (bit 0). See the updated
[`protocol.md`](../001-device-connection-foundation/contracts/protocol.md) §3/§5 for the normative
table. In brief:

- Tag 13, `InputEvent` (device→desktop): `{ session, gesture_id, control, kind, device_ms }`,
  gated by `PHYSICAL_INPUT_V1`.
- Tag 14, `Presentation` (desktop→device): `{ session, revision, primary, value, transient_ms }`,
  gated by `PRESENTATION_V1`.

`ValueConfidence` (`Preview` / `Confirmed` / `Unverified`) exists so an optimistic local preview can
never be rendered as confirmed Windows truth (contract invariant 3). `value` is a transient overlay
that firmware expires locally after `transient_ms` and falls back to rendering `primary` from the
same message — this is why the message carries both, and it implements invariant 50 (a transient
restores current underlying truth, not a hardcoded Idle) without a host timer or a second round trip.

`revision` is strictly increasing **within a session** and resets when a new session is accepted
(`PresentationResolver::begin_session`) — see §4.1 below for why session-scoping, not
device-lifetime-scoping, is the correct boundary.

### 4. Connection-scoped session identity

The handshake nonce (already present in `Hello`/`HelloAck`) is reused as connection-scoped session
identity, stamped on every `InputEvent` and every `Presentation`:

- The desktop mints a session nonce and sends it in `Hello`, as it already did.
- Firmware stores the accepted nonce and stamps every `InputEvent` with it; it clears input state
  and the accepted-revision high-water mark on a new `Hello`, and on `Bye`/link loss.
- The desktop (`InputIngress::accept`, `apps/desktop/src-tauri/src/input/mod.rs`) rejects any
  `InputEvent` whose `session` does not match the current session (`RejectReason::StaleSession`),
  and separately requires an observed `GestureStarted` in this session before accepting a `Detent`
  or `GestureEnded` (`RejectReason::UnknownGesture`) — retained as defence in depth, no longer
  load-bearing on its own now that session matching is the primary guard.
- Symmetrically the desktop stamps every `Presentation` with the session nonce, and firmware rejects
  any `Presentation` that does not match its currently accepted session, or whose `revision` is not
  newer within that session.

This closes a hole in an earlier design revision that relied only on "the desktop only accepts events
for a gesture whose `GestureStarted` it observed in this connection" — a *complete* stale pair
(`GestureStarted` + `Detent`, both buffered from before a disconnect) would satisfy that rule exactly
as a live pair would. See the design spec §4.1 for the full argument and the adversarial test list.

**Required change to nonce generation**, recorded in the updated
[`protocol.md`](../001-device-connection-foundation/contracts/protocol.md#nonce-as-connection-scoped-session-identity):
the nonce can no longer be a per-process counter starting at 1 — a restarted desktop process would
mint colliding session identities across restarts. `OsNonceSource`
(`apps/desktop/src-tauri/src/device/nonce.rs`) replaces the counter outright with a fresh `u32` drawn
directly from OS randomness (`getrandom::getrandom()`) on every handshake attempt — no counter
component is retained. It remains a **freshness token, not a security credential**.

### 5. Desktop modules

Four focused modules inside `apps/desktop/src-tauri/src`, no new crates:

```
input/          InputEvent -> InputIngress (session/gesture freshness) -> LogicalInput
action/         resolve_binding (the one Global binding) · volume::apply_step (step, clamp, boundary)
                · gesture_value::GestureValue (preview ownership + reconciliation) · Outcome
platform/       trait VolumeBackend + FakeVolumeBackend
  windows/      #[cfg(windows)] Core Audio (IAudioEndpointVolume, IMMNotificationClient)
  unimplemented.rs   other targets -> NotImplementedYet { target }
presentation/   PresentationResolver: ProductSnapshot -> Presentation   (pure)
```

Binding is deliberately not its own module: this slice has exactly one binding
(`Global: Rotate → MasterVolume`, `action::resolve_binding`), so it is a match arm inside `action/`.
A binding module is introduced when a slice has more than one scope to resolve between.

### 6. Capability, availability, and confirmation

Three orthogonal concepts, none collapsed into another (`apps/desktop/src-tauri/src/platform/mod.rs`):

```rust
enum BackendAvailability { Available, NotImplemented { target }, RuntimeUnavailable { reason } }
enum ConfirmationClass   { StateConfirmed, ExecutionConfirmed, TriggeredUnverified }
enum ActionAvailability  { Available { confirmation }, NotImplementedYet { target }, RuntimeUnavailable { reason }, Unknown }
enum Outcome { Running, StateConfirmed { volume_percent }, ExecutionConfirmed, TriggeredUnverified, Failed { reason } }
```

Only `BackendAvailability` and `ConfirmationClass` (plus the derived `ActionAvailability`) are
implemented. `PlatformCapability` — a fact about what the OS/session genuinely permits, distinct from
what Kivori has or hasn't implemented — is **deliberately absent**: every action in this slice is one
the OS can perform, so such a type would have no producer and no consumer. See ADR decision 1 below.

### 7. Windows Core Audio backend

`VolumeBackend` (`apps/desktop/src-tauri/src/platform/mod.rs`) is a three-method trait —
`availability`, `read`, `set` — implemented by `FakeVolumeBackend` (used by every host-testable
suite) and by the real Windows backend (`apps/desktop/src-tauri/src/platform/windows/mod.rs`) using
`IAudioEndpointVolume` on the default render endpoint. `set` returns the OS read-back value rather
than `()`, which is what makes `StateConfirmed` structurally honest.

A dedicated `kivori-audio` OS thread owns `CoInitializeEx`, the endpoint, and both callbacks
(`IAudioEndpointVolumeCallback`, `IMMNotificationClient`). Callbacks are ingress-only: they post to a
channel and never perform COM work inline; the owning thread does all rebinding. Every Kivori-issued
write carries `KIVORI_EVENT_CONTEXT`, a Kivori-specific GUID, so the callback can classify its own
echo (`ChangeOrigin::Kivori`, ignored) versus a genuinely external change
(`ChangeOrigin::External`, applied) versus an endpoint switch (`ChangeOrigin::EndpointRebind`,
applied and published as `Confirmed`).

The device role is `eRender` + `eConsole` (a documented constant); `eCommunications` is deliberately
not followed, and whether `eMultimedia` resolves to the same endpoint on real Windows is a physical
validation item (checklist row 9), not an assumption baked into the code.

### 8. Preview ownership and reconciliation

`GestureValue` (`apps/desktop/src-tauri/src/action/gesture_value.rs`) is the piece that only the
rotary path exercises:

- While a gesture is open, `GestureValue` owns the displayed value: each detent is applied locally
  and reported as `ValueConfidence::Preview`, even when the backend `set` call itself succeeded —
  promotion to `Confirmed` happens exactly once, at `GestureEnded`, after a fresh read-back (see
  "Spec deviations" in `requirements.md`).
- An external change or endpoint rebind that arrives while a gesture is open is recorded but not
  rendered over the active preview (`on_external_change` returns `None` while `active.is_some()`);
  an endpoint rebind additionally marks the gesture `abandoned` so its remaining target is discarded
  rather than retargeted onto a different endpoint.
- On `GestureEnded`, the desktop reads back the confirmed value and reconciles the display to it as
  `Confirmed` — desktop truth wins (contract invariants 1, 30).

### 9. Deterministic volume display

`render_volume_overlay` (`crates/kivori-renderer/src/overlay.rs`) is a pure, integer-only compositor
step consumed by both the host preview and firmware — the same canonical-rendering guarantee Feature
001 established for companion states. It draws a solid-rect bar (no numerals, no glyphs, no asset
changes) whose fill treatment differs by confidence: `Confirmed` fills solid, `Preview` fills hollow
(top/bottom rows only), and the track outline turns white at a boundary. Golden frames cover each
confidence at sampled percents including both boundaries, asserting the three treatments are not
byte-identical.

## Input tuning parameters

These are **tuning parameters, not settled constants** — each has a provisional value chosen without
physical hardware, and each is paired with the checklist item that must measure it before it can be
called validated. The rotary-input measurement items are
[`validation-checklist.md`](./validation-checklist.md) **rows 1–4** (the "Hardware — rotary input
fidelity" section); "Measured by" below cites those row numbers. An earlier revision of this table
cited an "HW-validation" numbered list that exists nowhere in `docs/` and, read as checklist rows,
pointed at the Windows-flyout checks (rows 5 and 6) instead.

| Parameter | Where | Provisional | Measured by |
|---|---|---|---|
| A/B sample interval | `Runtime::step` cadence | every runtime tick | checklist rows 2, 3 (no lost detents when slow, no false reversals when fast) |
| detent qualification depth | `QUARTER_STEPS_PER_DETENT` | 4 (full step) | checklist rows 1, 4 (one detent = one step; bounce produces no phantom detents) |
| invalid-transition threshold | `QuadratureDecoder::invalid_transitions` | diagnostic only, no threshold action | checklist row 4 (counter stays low) |
| gesture-end inactivity | `RotaryGesture::new` (`GESTURE_END_MS`) | 250 ms | contract §5 |
| base step | `BASE_STEP_PERCENT` | 2 points/detent | UX tuning |
| value transient | `VALUE_TRANSIENT_MS` | 800 ms | contract §5 |

The switch debounce interval is deliberately absent from this table: it arrives with the push-switch
slice, since `sw` is unread in Slice 002.

## The HW-040 pin map — specification, not measured evidence

Recorded in `firmware/esp32-c3/src/profile.rs` (`physical_st7789::ROTARY`):

| Line | GPIO | Notes |
|---|---|---|
| CLK (quadrature A) | GPIO4 | |
| DT (quadrature B) | GPIO5 | |
| SW (push-switch) | GPIO10 | unread in Slice 002 |
| HW-040 COM | GND | |
| HW-040 VCC | 3V3 | **not** 5V — ESP32-C3 GPIOs are not 5V tolerant |

All three lines are wired active-low with internal pull-ups; `PhysicalRotary::sample` inverts to
logical levels.

**GPIO9 was deliberately rejected for `sw`.** GPIO9 is the devkit BOOT button; holding it low at
reset enters ROM download mode. The encoder's push-switch is Kivori's recovery control (contract
invariant 24 — hardware recovery is out-of-band), so a user power-cycling while holding it for
recovery would land in the downloader instead of booting Kivori. GPIO2/GPIO8 are already taken
(D/C, backlight) and are themselves strapping pins; GPIO0/GPIO1 are the XTAL_32K pair and unusable
if a 32.768 kHz crystal is fitted; GPIO20/GPIO21 are left free so the UART0 boot console stays
available.

**This pin map is a specification the maintainer is wiring to, not an observation of an existing
board.** It is categorically different from the SPI/display pin facts beside it in
[Feature 001's validation checklist](../001-device-connection-foundation/validation-checklist.md#verified-physical-kivori-profile)
(SCK GPIO6, MOSI GPIO7, D/C GPIO2, RST GPIO3, backlight GPIO8), which are **dated physical evidence
from 2026-08-11** — measured against a real board and confirmed working. The rotary pin map has no
such date and no such confirmation yet; [`validation-checklist.md`](./validation-checklist.md) row 15
exists specifically to close that gap, and until it has a dated result this pin map must be read as
"decided," not "verified."

## ADR evaluation

The design spec listed three ADR candidates and pre-authorized none of them. Now that Slice 002 is
built, each is decided below.

### 1. Capability / availability / confirmation as three orthogonal concepts — **no ADR**

The split survived implementation exactly as designed (`platform/mod.rs`: `BackendAvailability`,
`ConfirmationClass`, the derived `ActionAvailability`), and a flat enum would genuinely have been
worse — collapsing "the OS cannot do this" with "Kivori hasn't written it yet" would put a falsehood
into the eventual configuration UI (a macOS build would report volume control as
platform-unsupported rather than merely unimplemented). That said, only two of the three concepts
the spec named are actually instantiated: `BackendAvailability` and `ConfirmationClass`.
`PlatformCapability` — the third orthogonal concept, a fact about what the machine genuinely permits
— was **deliberately deferred**, because every action in this slice is one the OS can perform, so it
would have no producer and no consumer. Recording an ADR for a three-way split when only two of the
three types exist yet would overstate what has actually been settled by evidence; the design
rationale is already fully captured in the design spec §6 and restated in this file's §6 above,
which is where a reader needs it. Revisit an ADR when `PlatformCapability` gets its first real
producer (the first OS-restricted or permission-gated action) and the three-way shape can be
evaluated as built, not as designed.

### 2. Input-semantics ownership split (firmware forms detents, desktop owns action meaning) — **no ADR**

This is already settled by the product contract, not merely implied by it. The design spec's
citation of "invariant 24" for this point does not hold up under inspection — invariant 24 in the
current contract text is "Hardware recovery is out-of-band," which is unrelated. The actual settling
text is contract **invariant 46** ("Input conditioning precedes gesture semantics: ... only validated
logical detents participate in direction-reversal and acceleration rules") together with the US1
"Validated logical detents" section (`docs/product/user-story-contract.md`, around line 209): "Firmware/input
processing MAY debounce, decode quadrature transitions, reject electrically invalid transition
sequences, and otherwise convert raw encoder activity into validated logical detents before gesture
semantics are evaluated." That is a product-level normative statement of exactly the split this slice
implements (`QuadratureDecoder`/`RotaryGesture` in firmware; `action::volume`/`GestureValue` on the
desktop). An ADR would be re-documenting a decision the contract already made; the implementation
correctly follows it rather than settling it.

### 3. The handshake nonce doubling as connection-scoped session identity — **ADR-0006 (this slice adds it)**

This is the one candidate that genuinely widens an existing protocol element and imposes a new
cross-peer requirement (nonce uniqueness across desktop process restarts) that did not exist before.
It is not a temporary implementation choice — it is now load-bearing for the no-stale-replay
guarantee (contract invariant 5) on every future capability-gated message, not just this slice's two.
Future engineers extending the protocol need the reasoning for why a dedicated `SessionEpoch`
message was rejected in favor of reusing the nonce. See
[`docs/adr/0006-handshake-nonce-as-session-identity.md`](../../adr/0006-handshake-nonce-as-session-identity.md).

## Verification

| Layer | Runs on | Covers |
|---|---|---|
| Pure unit | every host, including macOS | quadrature decode against bounce/invalid vectors, gesture boundary, clamping/boundary suppression, outcome classification, availability derivation, presentation resolution (transient→primary fallback, within-session revision monotonicity, confidence never upgrading without a read-back) |
| Backend contract suite | every host, via the fake | `tests/volume_backend_contract.rs` — one suite the fake and the real Windows backend both satisfy |
| Firmware host-sim | CI | `firmware/esp32-c3/tests/rotary_input.rs` — scripted A/B level sequences produce the expected `InputEvent` stream |
| Desktop integration | CI | `apps/desktop/src-tauri/tests/rotary_loop.rs` — `InputEvent` bytes in → `Presentation` bytes out against the fake backend, plus the §4.1 adversarial stale-session-pair test (the stale-revision-`Presentation` adversarial test lives on the firmware side — see `rotary_input.rs` above) |
| Golden frames | CI, all OSes | `kivori-golden-frames` — volume overlay determinism per `ValueConfidence`, asserting the three treatments are not byte-identical |
| Windows CI | `windows-latest` | compiles the Core Audio backend; runs the fake-backend contract suite; anything requiring a real endpoint is not exercised here |
| **Physical Windows** | maintainer hardware | not yet executed — see [`validation-checklist.md`](./validation-checklist.md) |
| **Physical device** | maintainer hardware | not yet executed — see [`validation-checklist.md`](./validation-checklist.md) |

No existing assertion was weakened or deleted to build this slice.

## Non-goals (carried from the design spec)

- Proving every future input invariant. Recovery arbitration, release-qualified press semantics, and
  device-wide single-gesture ownership belong to the push-switch slice.
- Building a generic action, binding, or capability framework ahead of a second consumer.
- Feature parity across operating systems. macOS and Linux honestly report `NotImplementedYet`.
- Merging PR #2.
