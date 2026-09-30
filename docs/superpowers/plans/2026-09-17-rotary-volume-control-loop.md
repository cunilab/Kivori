# Slice 002 — Rotary Volume Control Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turning the HW-040 knob changes Windows default-output master volume, and the Kivori device displays the volume Windows actually reports.

**Architecture:** Firmware decodes A/B levels into *validated logical detents* through a pure state machine behind a new `InputSource` port, forms rotary gestures, and emits semantic `InputEvent` messages. The desktop validates session/gesture freshness, applies a fixed 1× step through a single Global binding, executes against a narrow `VolumeBackend` (Windows Core Audio, with a deterministic fake everywhere else), and returns a semantic `Presentation` carrying an explicit `Preview`/`Confirmed`/`Unverified` value confidence. Tasks are ordered so the full vertical loop runs against the fake backend at Task 9, before any Windows or hardware code exists.

**Tech Stack:** Rust (two Cargo workspaces per ADR-0001) · `no_std` firmware on ESP32-C3 (`riscv32imc-unknown-none-elf`, esp-hal) · COBS + CRC-32 + postcard wire protocol · Tauri v2 desktop native core · Windows Core Audio (`windows` crate) · `heapless` · golden-frame determinism harness.

**Spec:** [`../specs/2026-09-17-rotary-volume-control-loop-design.md`](../specs/2026-09-17-rotary-volume-control-loop-design.md)

## Global Constraints

- Product behavior authority order: `docs/product/prd.md` → `docs/product/user-story-contract.md` → accepted ADRs → `docs/engineering-principles.md` → `docs/research/technical-research.md` (non-normative).
- `Message` is an **append-only** enum; the postcard variant index is the wire tag. Append variants only — **never** add a field to an existing variant.
- New behavior is **capability-gated**. An unnegotiated capability bit MUST leave the behavior off.
- Shared `crates/kivori-*` stay `no_std`, no-alloc, and must not gain `tauri`, `tokio`, `serialport`, `tokio-serial`, `axum`, `tower`, `sqlx`, or OS-specific desktop crates as direct dependencies (`scripts/check-crate-boundaries.sh`).
- No first-party crate may directly depend on a network-client crate (`scripts/check-offline-deps.sh`, banned list: `reqwest ureq isahc surf attohttpc curl hyper hyper-util awc actix-web tonic`).
- Rendering is deterministic: integer-only, no wall-clock, no floating point, no unseeded randomness. Intentional pixel changes update golden evidence in the same change.
- Diagnostics carry only the ADR-0005 allowlist. Never raw payload bytes, raw `DeviceId`, paths, usernames, or tokens.
- Contract timing targets used here: local acknowledgement **< 50 ms** · rotary gesture-end inactivity **250 ms**.
- Fixed step: **`BASE_STEP_PERCENT = 2`**. No acceleration in this slice.
- **Deferred, do not implement:** acceleration and the 5× ceiling · master mute · push-switch gesture FSM, Hold semantics, recovery ownership/arbitration · heartbeat rewiring · profiles/ContextEngine · DeviceRegistry · ExecutionTracker · ConfigStore · UpdateCoordinator · SessionOwnershipGate · macros · any second OS backend · per-application audio · the `eCommunications` device role.
- No unrelated refactors. Do not weaken or delete an existing assertion.
- Do not merge PR #2. Work stays on branch `docs/product-prd-us-contract`.

## File Structure

**Shared model — `crates/kivori-model/`**
- `src/input.rs` *(create)* — `Direction`, `InputLevels`. Leaf input value types shared by firmware, protocol, desktop.
- `src/presentation.rs` *(create)* — `PrimaryState`, `ValueKind`, `ValueConfidence`, `ValueDisplay`. Placed in `kivori-model` (not `kivori-protocol`) so `kivori-renderer` can draw from them without depending on the protocol crate.
- `src/capabilities.rs` *(modify)* — allocate the first two capability bits.
- `src/lib.rs` *(modify)* — declare and re-export the new modules.

**Protocol — `crates/kivori-protocol/`**
- `src/message.rs` *(modify)* — `ControlId`, `InputKind`, `InputEvent`, `Presentation`; append `Message::InputEvent` (tag 11) and `Message::Presentation` (tag 12).
- `tests/input_presentation.rs` *(create)* — round-trip, tag stability, capability gating.

**Firmware — `firmware/esp32-c3/`**
- `src/input/mod.rs` *(create)* — module root, re-exports.
- `src/input/quadrature.rs` *(create)* — pure Gray-code decoder; emits a detent only on a completed full step.
- `src/input/gesture.rs` *(create)* — gesture identity and the 250 ms gesture-end boundary.
- `src/ports.rs` *(modify)* — add the `InputSource` port.
- `src/sim/mod.rs` *(modify)* — scripted host-sim `InputSource`.
- `src/runtime.rs` *(modify)* — sample input each tick, emit `InputEvent`, accept/gate `Presentation`, expire transients.
- `src/proto.rs` *(modify)* — store the accepted session nonce, encode `InputEvent`, decode `Presentation`.
- `src/profile.rs` *(modify)* — `RotaryProfile { clk, dt, sw }` for the physical pin map.
- `src/physical_rotary.rs` *(create)* — the physical `InputSource` adapter (Task 13).
- `src/physical_st7789.rs` *(modify)* — construct and pass the physical `InputSource`.
- `tests/rotary_input.rs` *(create)* — host-sim end-to-end: scripted levels → expected `InputEvent` stream.

**Renderer — `crates/kivori-renderer/`**
- `src/overlay.rs` *(create)* — `render_volume_overlay`, the one canonical volume bar shared by preview and firmware.
- `src/lib.rs` *(modify)* — declare/re-export.

**Golden frames — `tests/golden-frames/`**
- `tests/overlay_golden.rs` *(create)* — per-confidence, per-percent hashes, plus a distinguishability assertion.
- `manifest.toml` *(modify)* — committed hashes.

**Desktop — `apps/desktop/src-tauri/`**
- `src/device/nonce.rs` *(create)* — `NonceSource`, `OsNonceSource`, `FixedNonceSource`.
- `src/device/session.rs` *(modify)* — use `NonceSource`; retain the accepted session nonce; route input/presentation.
- `src/input/mod.rs` *(create)* — ingress: session validation, open-gesture tracking, `LogicalInput`.
- `src/action/mod.rs` *(create)* — `ActionId`, binding table, `Outcome`.
- `src/action/volume.rs` *(create)* — fixed-step value model, clamp, boundary, gesture value state.
- `src/platform/mod.rs` *(create)* — `VolumeBackend` trait, availability types, `FakeVolumeBackend`.
- `src/platform/unimplemented.rs` *(create)* — non-Windows targets.
- `src/platform/windows/mod.rs` *(create)* — Core Audio thread (Task 12).
- `src/presentation/mod.rs` *(create)* — pure `PresentationResolver`.
- `src/runtime/device_task.rs` *(modify)* — wire ingress → action → presentation egress.
- `tests/rotary_loop.rs` *(create)* — desktop integration incl. both adversarial staleness tests.
- `tests/volume_backend_contract.rs` *(create)* — the suite fake and real backends both satisfy.

**Docs**
- `docs/features/002-rotary-volume-control/{requirements.md,architecture.md,validation-checklist.md}` *(create)*.
- `docs/features/001-device-connection-foundation/contracts/protocol.md`, `data-model.md` *(modify)*.

---

### Task 1: Pure quadrature decoder

A legal electrical transition must **not** automatically equal a completed detent (contract invariant 46, research R-82). The decoder tracks 2-bit Gray phase, accumulates quarter-steps, and emits a `Direction` only when a full detent has been traversed. Invalid transitions (both bits changing at once) are counted, never converted into motion.

**Files:**
- Create: `crates/kivori-model/src/input.rs`
- Modify: `crates/kivori-model/src/lib.rs`
- Create: `firmware/esp32-c3/src/input/mod.rs`
- Create: `firmware/esp32-c3/src/input/quadrature.rs`
- Modify: `firmware/esp32-c3/src/lib.rs`
- Test: `firmware/esp32-c3/tests/rotary_input.rs`

**Interfaces:**
- Produces: `kivori_model::input::{Direction, InputLevels}`; `kivori_firmware::input::quadrature::QuadratureDecoder` with `const fn new()`, `fn update(&mut self, a: bool, b: bool) -> Option<Direction>`, `fn invalid_transitions(&self) -> u32`, `fn reset(&mut self)`.

- [ ] **Step 1: Write the failing test**

Create `firmware/esp32-c3/tests/rotary_input.rs`:

```rust
#![cfg(feature = "host-sim")]

use kivori_firmware::input::quadrature::QuadratureDecoder;
use kivori_model::input::Direction;

/// Drive the decoder through a sequence of (a, b) levels, collecting emitted detents.
fn drive(seq: &[(bool, bool)]) -> Vec<Direction> {
    let mut d = QuadratureDecoder::new();
    let mut out = Vec::new();
    for &(a, b) in seq {
        if let Some(dir) = d.update(a, b) {
            out.push(dir);
        }
    }
    out
}

/// One full clockwise detent is the four-phase cycle 00 -> 01 -> 11 -> 10 -> 00.
const CW_CYCLE: [(bool, bool); 5] = [
    (false, false),
    (false, true),
    (true, true),
    (true, false),
    (false, false),
];

/// Counter-clockwise is the same cycle traversed in reverse.
const CCW_CYCLE: [(bool, bool); 5] = [
    (false, false),
    (true, false),
    (true, true),
    (false, true),
    (false, false),
];

#[test]
fn full_clockwise_cycle_emits_exactly_one_cw_detent() {
    assert_eq!(drive(&CW_CYCLE), vec![Direction::Cw]);
}

#[test]
fn full_counter_clockwise_cycle_emits_exactly_one_ccw_detent() {
    assert_eq!(drive(&CCW_CYCLE), vec![Direction::Ccw]);
}

#[test]
fn partial_motion_that_returns_emits_no_detent() {
    // Bounce walks to an adjacent Gray state and comes back without completing a detent.
    let seq = [
        (false, false),
        (false, true),
        (false, false),
        (false, true),
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn repeated_identical_samples_emit_nothing() {
    let seq = [(false, false); 8];
    assert_eq!(drive(&seq), vec![]);
}

#[test]
fn illegal_double_bit_transition_is_counted_and_emits_no_detent() {
    let mut d = QuadratureDecoder::new();
    assert_eq!(d.update(false, false), None);
    // 00 -> 11 changes both bits at once: electrically impossible for a real detent.
    assert_eq!(d.update(true, true), None);
    assert_eq!(d.invalid_transitions(), 1);
}

#[test]
fn three_consecutive_cw_cycles_emit_three_cw_detents() {
    let mut seq = Vec::new();
    for _ in 0..3 {
        seq.extend_from_slice(&CW_CYCLE[1..]);
    }
    let mut full = vec![(false, false)];
    full.extend(seq);
    assert_eq!(
        drive(&full),
        vec![Direction::Cw, Direction::Cw, Direction::Cw]
    );
}

#[test]
fn reversal_mid_cycle_does_not_emit_a_detent() {
    // Advance two quarter-steps clockwise, then retreat to the start. No detent completed.
    let seq = [
        (false, false),
        (false, true),
        (true, true),
        (false, true),
        (false, false),
    ];
    assert_eq!(drive(&seq), vec![]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') --test rotary_input`
Expected: FAIL — `unresolved import kivori_firmware::input` / `could not find input in kivori_model`.

- [ ] **Step 3: Write the shared input value types**

Create `crates/kivori-model/src/input.rs`:

```rust
//! Leaf input value types shared by firmware, the wire protocol, and the desktop core.

use serde::{Deserialize, Serialize};

/// Direction of one completed, validated logical detent.
///
/// Electrical quarter-step transitions are NOT directions; only a fully traversed
/// detent produces one (user-story-contract invariant 46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Cw,
    Ccw,
}

/// Instantaneous encoder levels sampled from hardware.
///
/// `sw` is carried but unused in Slice 002; the push-switch gesture machine is a later slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputLevels {
    pub a: bool,
    pub b: bool,
    pub sw: bool,
}
```

Add to `crates/kivori-model/src/lib.rs` alongside the existing module declarations:

```rust
pub mod input;
```

- [ ] **Step 4: Write the decoder**

Create `firmware/esp32-c3/src/input/mod.rs`:

```rust
//! Physical input: pure, host-testable decoding and gesture formation.
//!
//! Everything here sits ABOVE the `InputSource` port, so it is provable without hardware.

pub mod gesture;
pub mod quadrature;
```

Create `firmware/esp32-c3/src/input/quadrature.rs`:

```rust
//! Detent-qualified quadrature decoding.
//!
//! Phase is the 2-bit Gray code `(a << 1) | b`. Each legal transition moves one
//! quarter-step. A `Direction` is emitted only when four quarter-steps have
//! accumulated in the same rotational sense, i.e. a complete mechanical detent.
//! Partial motion that reverses before completing simply unwinds the accumulator.

use kivori_model::input::Direction;

/// Quarter-steps in one full HW-040 detent.
const QUARTER_STEPS_PER_DETENT: i8 = 4;

#[derive(Debug)]
pub struct QuadratureDecoder {
    /// Last observed Gray phase, or `None` before the first sample.
    phase: Option<u8>,
    /// Signed quarter-step accumulator; positive is clockwise.
    accumulator: i8,
    /// Count of electrically impossible transitions (both bits changed at once).
    invalid: u32,
}

impl Default for QuadratureDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl QuadratureDecoder {
    pub const fn new() -> Self {
        Self {
            phase: None,
            accumulator: 0,
            invalid: 0,
        }
    }

    /// Feed one sample. Returns a direction only when a full detent completed.
    pub fn update(&mut self, a: bool, b: bool) -> Option<Direction> {
        let next = ((a as u8) << 1) | (b as u8);

        let Some(prev) = self.phase else {
            // First sample establishes the reference phase without producing motion.
            self.phase = Some(next);
            return None;
        };

        if next == prev {
            return None;
        }

        let step = match quarter_step(prev, next) {
            Some(step) => step,
            None => {
                // Both bits changed: impossible for a real contact. Never invent motion.
                self.invalid = self.invalid.saturating_add(1);
                self.phase = Some(next);
                return None;
            }
        };

        self.phase = Some(next);
        self.accumulator += step;

        if self.accumulator >= QUARTER_STEPS_PER_DETENT {
            self.accumulator = 0;
            return Some(Direction::Cw);
        }
        if self.accumulator <= -QUARTER_STEPS_PER_DETENT {
            self.accumulator = 0;
            return Some(Direction::Ccw);
        }
        None
    }

    pub const fn invalid_transitions(&self) -> u32 {
        self.invalid
    }

    /// Drop all motion state. Used when a session ends so partial motion cannot
    /// leak across a reconnect.
    pub fn reset(&mut self) {
        self.phase = None;
        self.accumulator = 0;
    }
}

/// Gray-code adjacency: `+1` clockwise, `-1` counter-clockwise, `None` if illegal.
///
/// Clockwise phase order is 00 -> 01 -> 11 -> 10 -> 00.
const fn quarter_step(prev: u8, next: u8) -> Option<i8> {
    match (prev, next) {
        (0b00, 0b01) | (0b01, 0b11) | (0b11, 0b10) | (0b10, 0b00) => Some(1),
        (0b00, 0b10) | (0b10, 0b11) | (0b11, 0b01) | (0b01, 0b00) => Some(-1),
        _ => None,
    }
}
```

Add to `firmware/esp32-c3/src/lib.rs`, ungated so host-sim tests reach it:

```rust
pub mod input;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') --test rotary_input`
Expected: PASS, 7 tests.

- [ ] **Step 6: Verify the shared crate still builds for RISC-V**

Run: `just fw-check && just check-boundaries`
Expected: both succeed (`kivori-model` gained only `serde`, which it already uses).

- [ ] **Step 7: Commit**

```bash
git add crates/kivori-model/src/input.rs crates/kivori-model/src/lib.rs \
        firmware/esp32-c3/src/input/ firmware/esp32-c3/src/lib.rs \
        firmware/esp32-c3/tests/rotary_input.rs
git commit -m "feat(firmware): add detent-qualified quadrature decoder"
```

---

### Task 2: Rotary gesture formation

Gesture identity plus the 250 ms gesture-end boundary (contract §5). `gesture_id` is session-unique; §4.1 of the spec removed the need for it to survive across sessions.

**Files:**
- Create: `firmware/esp32-c3/src/input/gesture.rs`
- Test: `firmware/esp32-c3/tests/rotary_input.rs` (append)

**Interfaces:**
- Consumes: `kivori_model::input::Direction`.
- Produces: `kivori_firmware::input::gesture::{RotaryGesture, RotaryEvent}`; `RotaryGesture::new(gesture_end_ms: u32)`, `fn on_detent(&mut self, direction: Direction, now_ms: u32) -> (Option<RotaryEvent>, RotaryEvent)`, `fn poll(&mut self, now_ms: u32) -> Option<RotaryEvent>`, `fn reset(&mut self)`.

- [ ] **Step 1: Write the failing test**

Append to `firmware/esp32-c3/tests/rotary_input.rs`:

```rust
use kivori_firmware::input::gesture::{RotaryEvent, RotaryGesture};

const GESTURE_END_MS: u32 = 250;

#[test]
fn first_detent_opens_a_gesture_and_reports_the_detent() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    let (started, detent) = g.on_detent(Direction::Cw, 1_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 1 }));
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }
    );
}

#[test]
fn detents_inside_the_window_stay_in_one_gesture() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    let (started, _) = g.on_detent(Direction::Cw, 1_000);
    assert!(started.is_some());

    // 249 ms later: still the same gesture, so no new GestureStarted.
    let (started, detent) = g.on_detent(Direction::Cw, 1_249);
    assert_eq!(started, None);
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }
    );
}

#[test]
fn gesture_ends_after_the_inactivity_window() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);

    assert_eq!(g.poll(1_249), None, "must not end before the window elapses");
    assert_eq!(
        g.poll(1_250),
        Some(RotaryEvent::GestureEnded { gesture_id: 1 })
    );
    assert_eq!(g.poll(1_500), None, "GestureEnded is emitted exactly once");
}

#[test]
fn a_detent_after_the_window_opens_a_new_gesture_id() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    assert_eq!(
        g.poll(1_250),
        Some(RotaryEvent::GestureEnded { gesture_id: 1 })
    );

    let (started, detent) = g.on_detent(Direction::Ccw, 2_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 2 }));
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 2,
            direction: Direction::Ccw
        }
    );
}

#[test]
fn reversal_within_a_gesture_does_not_split_the_gesture() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    let (started, detent) = g.on_detent(Direction::Ccw, 1_100);
    assert_eq!(started, None, "reversal is not a new gesture");
    assert_eq!(
        detent,
        RotaryEvent::Detent {
            gesture_id: 1,
            direction: Direction::Ccw
        }
    );
}

#[test]
fn reset_closes_the_gesture_silently_and_restarts_numbering() {
    let mut g = RotaryGesture::new(GESTURE_END_MS);
    g.on_detent(Direction::Cw, 1_000);
    g.reset();
    assert_eq!(g.poll(5_000), None, "a reset gesture emits no GestureEnded");

    let (started, _) = g.on_detent(Direction::Cw, 6_000);
    assert_eq!(started, Some(RotaryEvent::GestureStarted { gesture_id: 1 }));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') --test rotary_input`
Expected: FAIL — `unresolved import kivori_firmware::input::gesture`.

- [ ] **Step 3: Write the implementation**

Create `firmware/esp32-c3/src/input/gesture.rs`:

```rust
//! Rotary gesture formation: identity plus the inactivity boundary.
//!
//! A gesture opens on the first validated detent and closes after
//! `gesture_end_ms` with no new detent (user-story-contract section 5).

use kivori_model::input::Direction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotaryEvent {
    GestureStarted { gesture_id: u16 },
    Detent { gesture_id: u16, direction: Direction },
    GestureEnded { gesture_id: u16 },
}

#[derive(Debug)]
pub struct RotaryGesture {
    gesture_end_ms: u32,
    /// Identifier of the most recently opened gesture. Session-unique.
    last_id: u16,
    /// `Some((id, last_detent_ms))` while a gesture is open.
    open: Option<(u16, u32)>,
}

impl RotaryGesture {
    pub const fn new(gesture_end_ms: u32) -> Self {
        Self {
            gesture_end_ms,
            last_id: 0,
            open: None,
        }
    }

    /// Record one validated detent.
    ///
    /// Returns `(Some(GestureStarted), Detent)` when this detent opened a new
    /// gesture, and `(None, Detent)` when it continued the current one.
    pub fn on_detent(&mut self, direction: Direction, now_ms: u32) -> (Option<RotaryEvent>, RotaryEvent) {
        let mut started = None;

        let gesture_id = match self.open {
            Some((id, _)) => id,
            None => {
                self.last_id = self.last_id.wrapping_add(1);
                if self.last_id == 0 {
                    // Never hand out id 0; it reads as "no gesture".
                    self.last_id = 1;
                }
                started = Some(RotaryEvent::GestureStarted {
                    gesture_id: self.last_id,
                });
                self.last_id
            }
        };

        self.open = Some((gesture_id, now_ms));
        (
            started,
            RotaryEvent::Detent {
                gesture_id,
                direction,
            },
        )
    }

    /// Close the gesture once the inactivity window has elapsed. Emits at most once.
    pub fn poll(&mut self, now_ms: u32) -> Option<RotaryEvent> {
        let (id, last_ms) = self.open?;
        if now_ms.wrapping_sub(last_ms) >= self.gesture_end_ms {
            self.open = None;
            return Some(RotaryEvent::GestureEnded { gesture_id: id });
        }
        None
    }

    /// Drop gesture state without emitting anything, and restart numbering.
    ///
    /// Used when a host session ends: a gesture from the old session must not be
    /// completable in the new one.
    pub fn reset(&mut self) {
        self.open = None;
        self.last_id = 0;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') --test rotary_input`
Expected: PASS, 13 tests.

- [ ] **Step 5: Commit**

```bash
git add firmware/esp32-c3/src/input/gesture.rs firmware/esp32-c3/tests/rotary_input.rs
git commit -m "feat(firmware): add rotary gesture formation with 250ms boundary"
```

---

### Task 3: Capability bits and protocol variants

**Files:**
- Modify: `crates/kivori-model/src/capabilities.rs`
- Create: `crates/kivori-model/src/presentation.rs`
- Modify: `crates/kivori-model/src/lib.rs`
- Modify: `crates/kivori-protocol/src/message.rs`
- Test: `crates/kivori-protocol/tests/input_presentation.rs`

**Interfaces:**
- Consumes: `kivori_model::input::Direction`.
- Produces:
  - `Capabilities::PHYSICAL_INPUT_V1` (bit 0), `Capabilities::PRESENTATION_V1` (bit 1).
  - `kivori_model::presentation::{PrimaryState, ValueKind, ValueConfidence, ValueDisplay}`.
  - `kivori_protocol::message::{ControlId, InputKind, InputEvent, Presentation}`; `Message::InputEvent` = tag 11, `Message::Presentation` = tag 12.

- [ ] **Step 1: Write the failing test**

Create `crates/kivori-protocol/tests/input_presentation.rs`:

```rust
use heapless::Vec;
use kivori_model::input::Direction;
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_frame, decode_message, encode_message, ControlId, InputEvent, InputKind, Message,
    Presentation, MAX_FRAME, MAX_WIRE,
};

/// Mirrors the helper in `crates/kivori-protocol/tests/roundtrip.rs`.
/// `decode_message` takes the packet WITHOUT the trailing 0x00 delimiter.
fn roundtrip(msg: &Message) -> Message {
    let v = ProtocolVersion::new(1, 0);
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, v, 7, &mut wire).unwrap();
    let packet = &wire[..wire.len() - 1];
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (_, decoded) = decode_message(packet, &mut scratch, &[1]).unwrap();
    decoded
}

/// The first payload byte is the postcard variant index, i.e. the wire tag.
fn payload_tag(msg: &Message) -> u8 {
    let v = ProtocolVersion::new(1, 0);
    let mut wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(msg, v, 0, &mut wire).unwrap();
    let packet = &wire[..wire.len() - 1];
    let mut scratch: Vec<u8, MAX_FRAME> = Vec::new();
    let (_, payload) = decode_frame(packet, &mut scratch).unwrap();
    payload[0]
}

#[test]
fn input_event_roundtrips() {
    let msg = Message::InputEvent(InputEvent {
        session: 0xDEAD_BEEF,
        gesture_id: 42,
        control: ControlId::Rotary,
        kind: InputKind::Detent(Direction::Ccw),
        device_ms: 123_456,
    });
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn every_input_kind_roundtrips() {
    for kind in [
        InputKind::GestureStarted,
        InputKind::Detent(Direction::Cw),
        InputKind::Detent(Direction::Ccw),
        InputKind::GestureEnded,
    ] {
        let msg = Message::InputEvent(InputEvent {
            session: 1,
            gesture_id: 1,
            control: ControlId::Rotary,
            kind,
            device_ms: 0,
        });
        assert_eq!(roundtrip(&msg), msg);
    }
}

#[test]
fn presentation_roundtrips_with_and_without_a_value() {
    let with_value = Message::Presentation(Presentation {
        session: 9,
        revision: 12,
        primary: PrimaryState::Active,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: 64,
            confidence: ValueConfidence::Preview,
            at_boundary: false,
        }),
        transient_ms: 800,
    });
    assert_eq!(roundtrip(&with_value), with_value);

    let without_value = Message::Presentation(Presentation {
        session: 9,
        revision: 13,
        primary: PrimaryState::Idle,
        value: None,
        transient_ms: 0,
    });
    assert_eq!(roundtrip(&without_value), without_value);
}

#[test]
fn every_value_confidence_roundtrips() {
    for confidence in [
        ValueConfidence::Preview,
        ValueConfidence::Confirmed,
        ValueConfidence::Unverified,
    ] {
        let msg = Message::Presentation(Presentation {
            session: 1,
            revision: 1,
            primary: PrimaryState::Active,
            value: Some(ValueDisplay {
                kind: ValueKind::Volume,
                current_percent: 50,
                confidence,
                at_boundary: false,
            }),
            transient_ms: 800,
        });
        assert_eq!(roundtrip(&msg), msg);
    }
}

/// The postcard variant index IS the wire tag. Existing tags 0..=10 must not move.
#[test]
fn new_variants_are_appended_at_tags_11_and_12() {
    assert_eq!(
        payload_tag(&Message::InputEvent(InputEvent {
            session: 0,
            gesture_id: 0,
            control: ControlId::Rotary,
            kind: InputKind::GestureEnded,
            device_ms: 0,
        })),
        11,
        "InputEvent must be wire tag 11"
    );

    assert_eq!(
        payload_tag(&Message::Presentation(Presentation {
            session: 0,
            revision: 0,
            primary: PrimaryState::Idle,
            value: None,
            transient_ms: 0,
        })),
        12,
        "Presentation must be wire tag 12"
    );
}

#[test]
fn the_two_new_capability_bits_are_distinct_and_stable() {
    assert_eq!(Capabilities::PHYSICAL_INPUT_V1.bits(), 1 << 0);
    assert_eq!(Capabilities::PRESENTATION_V1.bits(), 1 << 1);
}

#[test]
fn a_peer_without_the_bit_leaves_the_capability_unnegotiated() {
    let desktop = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
    let old_device = Capabilities::NONE;
    let negotiated = desktop.intersection(old_device);

    assert!(!negotiated.contains(Capabilities::PHYSICAL_INPUT_V1));
    assert!(!negotiated.contains(Capabilities::PRESENTATION_V1));
}

#[test]
fn a_peer_with_only_one_bit_negotiates_only_that_bit() {
    let desktop = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
    let device = Capabilities::PHYSICAL_INPUT_V1;
    let negotiated = desktop.intersection(device);

    assert!(negotiated.contains(Capabilities::PHYSICAL_INPUT_V1));
    assert!(!negotiated.contains(Capabilities::PRESENTATION_V1));
}
```

> `Capabilities` **already** provides `bits()`, `contains`, `intersection`, and `union` (`crates/kivori-model/src/capabilities.rs`). Do not re-add them, and do not change `NONE` or `crates/kivori-protocol/src/negotiate.rs` — this task only allocates two constants.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-protocol --test input_presentation`
Expected: FAIL — `no variant named InputEvent`, `could not find presentation in kivori_model`.

- [ ] **Step 3: Allocate the capability bits**

In `crates/kivori-model/src/capabilities.rs`, add **only** these two constants inside the existing `impl Capabilities`, directly beneath `NONE`:

```rust
    // CAPABILITY BIT REGISTRY — allocate centrally, never reuse a retired bit.
    //   bit 0  PHYSICAL_INPUT_V1  Slice 002
    //   bit 1  PRESENTATION_V1    Slice 002

    /// Bit 0 — the device may emit `InputEvent` (Slice 002, rotary input).
    pub const PHYSICAL_INPUT_V1: Capabilities = Capabilities(1 << 0);

    /// Bit 1 — the device renders semantic `Presentation` (Slice 002).
    pub const PRESENTATION_V1: Capabilities = Capabilities(1 << 1);
```

Also update the module doc comment at the top of that file: it currently says "No concrete capability flags are defined yet", which is no longer true.

- [ ] **Step 4: Write the shared presentation value types**

Create `crates/kivori-model/src/presentation.rs`:

```rust
//! Semantic presentation value types.
//!
//! These live in `kivori-model` rather than `kivori-protocol` so the shared
//! renderer can draw from them without depending on the wire crate.

use serde::{Deserialize, Serialize};

/// The underlying state that remains true beneath any transient overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimaryState {
    Idle,
    Active,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueKind {
    Volume,
}

/// How well the displayed value is known.
///
/// An optimistic local preview MUST NOT be rendered as observed desktop truth
/// (user-story-contract invariant 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueConfidence {
    /// Kivori's local target during an open gesture; not yet observed from the OS.
    Preview,
    /// The value the OS reported back.
    Confirmed,
    /// Dispatched, but read-back was unavailable.
    Unverified,
}

/// A transient value overlay. Expires after `Presentation::transient_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueDisplay {
    pub kind: ValueKind,
    /// 0..=100.
    pub current_percent: u8,
    pub confidence: ValueConfidence,
    pub at_boundary: bool,
}
```

Add to `crates/kivori-model/src/lib.rs`:

```rust
pub mod presentation;
```

- [ ] **Step 5: Append the protocol variants**

In `crates/kivori-protocol/src/message.rs`, add the payload types:

```rust
use kivori_model::input::Direction;
use kivori_model::presentation::{PrimaryState, ValueDisplay};

/// Which physical control produced an event. Extensible; only `Rotary` in Slice 002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlId {
    Rotary,
}

/// Semantic input. Raw electrical edges never reach the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    GestureStarted,
    Detent(Direction),
    GestureEnded,
}

/// Device -> desktop physical input.
///
/// `session` is the handshake nonce of the session that produced this event; the
/// desktop rejects any event that does not match its current session.
/// `device_ms` is carried for the deferred acceleration slice and is unused here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    pub session: Nonce,
    pub gesture_id: u16,
    pub control: ControlId,
    pub kind: InputKind,
    pub device_ms: u32,
}

/// Desktop -> device semantic presentation.
///
/// `primary` is the underlying truth; `value` is a transient overlay that the
/// device expires locally after `transient_ms` (0 = persistent), falling back to
/// `primary`. `revision` is strictly increasing WITHIN a session and resets with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub session: Nonce,
    pub revision: u32,
    pub primary: PrimaryState,
    pub value: Option<ValueDisplay>,
    pub transient_ms: u16,
}
```

Append to the `Message` enum — **after `Error`, never before**:

```rust
    /// Tag 11 — device -> desktop physical input (capability `PHYSICAL_INPUT_V1`).
    InputEvent(InputEvent),
    /// Tag 12 — desktop -> device semantic presentation (capability `PRESENTATION_V1`).
    Presentation(Presentation),
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p kivori-protocol && cargo test -p kivori-model`
Expected: PASS, including the pre-existing round-trip, vectors, and negotiation suites.

- [ ] **Step 7: Verify no_std and boundary guards still hold**

Run: `just fw-check && just check-boundaries && just golden`
Expected: all pass. `just golden` must be unchanged — this task touches no pixels.

- [ ] **Step 8: Commit**

```bash
git add crates/kivori-model/src/capabilities.rs crates/kivori-model/src/presentation.rs \
        crates/kivori-model/src/lib.rs crates/kivori-protocol/src/message.rs \
        crates/kivori-protocol/tests/input_presentation.rs
git commit -m "feat(protocol): append InputEvent/Presentation and allocate capability bits"
```

---

### Task 4: `InputSource` port, host-sim adapter, firmware emit path

First point at which the device produces a real `InputEvent` stream.

**Files:**
- Modify: `firmware/esp32-c3/src/ports.rs`
- Modify: `firmware/esp32-c3/src/sim/mod.rs`
- Modify: `firmware/esp32-c3/src/proto.rs`
- Modify: `firmware/esp32-c3/src/runtime.rs`
- Test: `firmware/esp32-c3/tests/rotary_input.rs` (append)

**Interfaces:**
- Consumes: `QuadratureDecoder`, `RotaryGesture`, `RotaryEvent`, `Message::InputEvent`, `Capabilities::PHYSICAL_INPUT_V1`.
- Produces: `kivori_firmware::ports::InputSource` with `fn sample(&mut self) -> InputLevels`; `kivori_firmware::sim::ScriptedInput` with `ScriptedInput::new(steps: Vec<InputLevels>)`; `Dispatcher::accepted_session(&self) -> Option<Nonce>`; `Runtime::step` gains an `input: &mut impl InputSource` parameter.

- [ ] **Step 1: Write the failing test**

Append to `firmware/esp32-c3/tests/rotary_input.rs`:

```rust
use kivori_firmware::sim::ScriptedInput;
use kivori_model::input::InputLevels;

fn lv(a: bool, b: bool) -> InputLevels {
    InputLevels { a, b, sw: false }
}

#[test]
fn scripted_input_source_replays_levels_then_holds_the_last() {
    use kivori_firmware::ports::InputSource;

    let mut src = ScriptedInput::new(vec![lv(false, false), lv(false, true)]);
    assert_eq!(src.sample(), lv(false, false));
    assert_eq!(src.sample(), lv(false, true));
    // Exhausted scripts hold the final level rather than wrapping or panicking.
    assert_eq!(src.sample(), lv(false, true));
}

#[test]
fn a_full_cw_cycle_through_the_port_produces_started_detent_ended() {
    use kivori_firmware::sim::{drive_rotary, SeenInput};

    // Host-sim end-to-end: scripted levels -> validated detent -> emitted event stream.
    let levels = vec![
        lv(false, false),
        lv(false, true),
        lv(true, true),
        lv(true, false),
        lv(false, false),
    ];

    assert_eq!(
        drive_rotary(levels),
        vec![
            SeenInput::GestureStarted { gesture_id: 1 },
            SeenInput::Detent {
                gesture_id: 1,
                direction: Direction::Cw
            },
            SeenInput::GestureEnded { gesture_id: 1 },
        ]
    );
}

#[test]
fn a_reversal_stays_in_one_gesture_and_reports_both_directions() {
    use kivori_firmware::sim::{drive_rotary, SeenInput};

    let mut levels = vec![
        // one CW detent
        lv(false, false),
        lv(false, true),
        lv(true, true),
        lv(true, false),
        lv(false, false),
    ];
    // then one CCW detent, back the way it came
    levels.extend_from_slice(&[lv(true, false), lv(true, true), lv(false, true), lv(false, false)]);

    assert_eq!(
        drive_rotary(levels),
        vec![
            SeenInput::GestureStarted { gesture_id: 1 },
            SeenInput::Detent { gesture_id: 1, direction: Direction::Cw },
            SeenInput::Detent { gesture_id: 1, direction: Direction::Ccw },
            SeenInput::GestureEnded { gesture_id: 1 },
        ]
    );
}

#[test]
fn the_dispatcher_records_the_accepted_session_nonce() {
    // Uses the existing host-sim handshake helper pattern from
    // `firmware/esp32-c3/tests/host_sim.rs`: drive Hello -> HelloAck -> Ready, then assert
    // the dispatcher retained the nonce it accepted.
    let mut d = kivori_firmware::sim::handshaken_dispatcher(0x1234_5678);
    assert_eq!(d.accepted_session(), Some(0x1234_5678));
}
```

Add `handshaken_dispatcher(nonce: u32) -> Dispatcher` to `firmware/esp32-c3/src/sim/mod.rs` in Step 4, built from the same in-memory transport the existing `host_sim.rs` tests already use — do not introduce a second handshake path.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') --test rotary_input`
Expected: FAIL — `no InputSource in ports`, `no ScriptedInput in sim`.

- [ ] **Step 3: Add the port**

Append to `firmware/esp32-c3/src/ports.rs`:

```rust
use kivori_model::input::InputLevels;

/// Instantaneous physical input levels.
///
/// Sampling and pin mapping live in the adapter; ALL semantics (debounce,
/// detent qualification, gesture formation) live above this port so they are
/// provable without hardware.
pub trait InputSource {
    /// Read the current levels. MUST NOT block.
    fn sample(&mut self) -> InputLevels;
}
```

- [ ] **Step 4: Add the host-sim adapter and scenario helpers**

Append to `firmware/esp32-c3/src/sim/mod.rs`:

```rust
use crate::input::gesture::{RotaryEvent, RotaryGesture};
use crate::input::quadrature::QuadratureDecoder;
use crate::ports::InputSource;
use kivori_model::input::{Direction, InputLevels};

/// Replays a fixed level sequence, then holds the final level forever.
pub struct ScriptedInput {
    steps: Vec<InputLevels>,
    index: usize,
}

impl ScriptedInput {
    pub fn new(steps: Vec<InputLevels>) -> Self {
        assert!(!steps.is_empty(), "ScriptedInput needs at least one level");
        Self { steps, index: 0 }
    }
}

impl InputSource for ScriptedInput {
    fn sample(&mut self) -> InputLevels {
        let level = self.steps[self.index];
        if self.index + 1 < self.steps.len() {
            self.index += 1;
        }
        level
    }
}

/// Test-only mirror of the emitted stream, in the order the runtime produced it.
#[derive(Debug, PartialEq, Eq)]
pub enum SeenInput {
    GestureStarted { gesture_id: u16 },
    Detent { gesture_id: u16, direction: Direction },
    GestureEnded { gesture_id: u16 },
}
```

Also add, in the same file, a helper that drives decoder + gesture over a script and returns the emitted stream. Gesture end is forced by advancing the virtual clock past the window:

```rust
/// Drive decoder + gesture over a level script and collect the emitted events.
///
/// Each level consumes 1 ms; the clock then advances past the gesture window so a
/// trailing `GestureEnded` is produced deterministically.
pub fn drive_rotary(levels: Vec<InputLevels>) -> Vec<SeenInput> {
    const GESTURE_END_MS: u32 = 250;
    let mut decoder = QuadratureDecoder::new();
    let mut gesture = RotaryGesture::new(GESTURE_END_MS);
    let mut src = ScriptedInput::new(levels.clone());
    let mut out = Vec::new();

    for tick in 0..levels.len() as u32 {
        let l = src.sample();
        if let Some(direction) = decoder.update(l.a, l.b) {
            let (started, detent) = gesture.on_detent(direction, tick);
            if let Some(RotaryEvent::GestureStarted { gesture_id }) = started {
                out.push(SeenInput::GestureStarted { gesture_id });
            }
            if let RotaryEvent::Detent { gesture_id, direction } = detent {
                out.push(SeenInput::Detent { gesture_id, direction });
            }
        }
    }

    let end_at = levels.len() as u32 + GESTURE_END_MS;
    if let Some(RotaryEvent::GestureEnded { gesture_id }) = gesture.poll(end_at) {
        out.push(SeenInput::GestureEnded { gesture_id });
    }
    out
}
```

Also add `handshaken_dispatcher(nonce: u32) -> Dispatcher` here: construct the in-memory transport the existing `firmware/esp32-c3/tests/host_sim.rs` already uses, drive `Hello` → `HelloAck` → `Ready` through it with the given nonce, and return the resulting `Dispatcher`. Reuse that file's existing handshake helper rather than writing a second one.

- [ ] **Step 5: Store the accepted session and emit input in the dispatcher**

In `firmware/esp32-c3/src/proto.rs`:

1. Add a field `accepted_session: Option<Nonce>` to `Dispatcher`, defaulting to `None`.
2. Where `Hello` is handled and `HelloAck` is produced, record `self.accepted_session = Some(hello.nonce)`.
3. Where `Bye` is handled (which already resets `SequenceTracker`), also set `self.accepted_session = None`.
4. Add:

```rust
    pub const fn accepted_session(&self) -> Option<Nonce> {
        self.accepted_session
    }

    /// Encode one input event. Returns `false` when the capability is not negotiated
    /// or no session is accepted — an unnegotiated capability MUST stay inert.
    pub fn send_input_event(
        &mut self,
        transport: &mut impl Transport,
        gesture_id: u16,
        kind: InputKind,
        device_ms: u32,
    ) -> bool {
        if !self.negotiated_caps.contains(Capabilities::PHYSICAL_INPUT_V1) {
            return false;
        }
        let Some(session) = self.accepted_session else {
            return false;
        };
        let msg = Message::InputEvent(InputEvent {
            session,
            gesture_id,
            control: ControlId::Rotary,
            kind,
            device_ms,
        });
        self.send(transport, &msg).is_ok()
    }
```

Use whatever the existing private send helper is named in `proto.rs` (it already encodes with `tx_seq`); do not introduce a second encode path.

- [ ] **Step 6: Sample input in the runtime tick**

In `firmware/esp32-c3/src/runtime.rs`, add `decoder: QuadratureDecoder` and `gesture: RotaryGesture` to `Runtime`, add an `input: &mut impl InputSource` parameter to `step` and `run`, and insert **after** protocol dispatch and **before** the render gate:

```rust
        // --- physical input ---------------------------------------------------
        let levels = input.sample();
        if let Some(direction) = self.decoder.update(levels.a, levels.b) {
            let (started, detent) = self.gesture.on_detent(direction, now_ms);
            if let Some(RotaryEvent::GestureStarted { gesture_id }) = started {
                self.dispatcher
                    .send_input_event(transport, gesture_id, InputKind::GestureStarted, now_ms);
            }
            if let RotaryEvent::Detent { gesture_id, direction } = detent {
                self.dispatcher.send_input_event(
                    transport,
                    gesture_id,
                    InputKind::Detent(direction),
                    now_ms,
                );
            }
        }
        if let Some(RotaryEvent::GestureEnded { gesture_id }) = self.gesture.poll(now_ms) {
            self.dispatcher
                .send_input_event(transport, gesture_id, InputKind::GestureEnded, now_ms);
        }
```

Where the dispatcher clears session state (`Bye`/link loss), also call `self.decoder.reset()` and `self.gesture.reset()` so partial motion and open gestures cannot survive a session boundary.

Update every existing caller of `Runtime::step`/`run` — `physical_st7789.rs`, `wokwi_runtime.rs`, and the host-sim tests — to pass an `InputSource`. For `physical_st7789.rs` and `wokwi_runtime.rs` in this task, pass a constant-level stub that always returns `InputLevels { a: false, b: false, sw: false }`; Task 13 replaces the physical one.

- [ ] **Step 7: Run tests to verify they pass**

Run: `just fw-test`
Expected: PASS, including the pre-existing `host_sim.rs`, `production_runtime.rs`, `render_parity.rs`, and `display_spi.rs` suites.

- [ ] **Step 8: Verify the firmware still builds for the device**

Run: `just fw-check && cd firmware/esp32-c3 && cargo clippy --features host-sim --target $(rustc -vV | sed -n 's/^host: //p') -- -D warnings`
Expected: both succeed.

- [ ] **Step 9: Commit**

```bash
git add firmware/esp32-c3/src/ports.rs firmware/esp32-c3/src/sim/mod.rs \
        firmware/esp32-c3/src/proto.rs firmware/esp32-c3/src/runtime.rs \
        firmware/esp32-c3/src/physical_st7789.rs firmware/esp32-c3/src/wokwi_runtime.rs \
        firmware/esp32-c3/tests/
git commit -m "feat(firmware): emit semantic InputEvent from a sampled InputSource port"
```

---

### Task 5: Connection-scoped nonce freshness

The current nonce is a counter initialised to `1` at `Session::new` and incremented per attempt (`apps/desktop/src-tauri/src/device/session.rs:65,80,100-101`). That is adequate for handshake liveness but **not** for session identity: a restarted desktop begins at `1` again, so a stale `InputEvent` stamped `1` would match a fresh session.

**Freshness source decision.** Use **`getrandom`** as an explicit direct dependency of `kivori-desktop`:

- It states exactly the property we want — OS-provided fresh bytes — rather than relying on a side effect of a hashing type.
- It is already present in the host `Cargo.lock` transitively (three versions), so it adds no new supply-chain surface, only a direct declaration.
- It is not on the offline guard's banned list (`scripts/check-offline-deps.sh` bans network clients only) and it goes into `apps/desktop/src-tauri`, never a shared `kivori-*` crate, so `scripts/check-crate-boundaries.sh` is unaffected.
- Rejected alternative: `SystemTime::now()` nanos. Zero new dependencies, but clocks move backwards under NTP, resolution is platform-dependent, and truncating to `u32` wraps every ~4.3 s — caveats we would have to document and could not test cleanly.
- Rejected alternative: `std::hash::RandomState`. Works, but its per-process randomness is an implementation detail of a hashing type rather than a documented entropy API.

The nonce is a **freshness token, not a security credential**, and nothing in this slice treats it as authentication.

**Files:**
- Create: `apps/desktop/src-tauri/src/device/nonce.rs`
- Modify: `apps/desktop/src-tauri/src/device/mod.rs`
- Modify: `apps/desktop/src-tauri/src/device/session.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Test: `apps/desktop/src-tauri/tests/nonce.rs`

**Interfaces:**
- Produces: `device::nonce::{NonceSource, OsNonceSource, FixedNonceSource}`; `NonceSource::next_nonce(&mut self) -> u32`; `Session::current_session(&self) -> Option<u32>`.

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src-tauri/tests/nonce.rs`:

```rust
use kivori_desktop::device::nonce::{FailingNonceSource, FixedNonceSource, NonceError, NonceSource, OsNonceSource};

// NOTE: there is deliberately NO test asserting that two OS-random nonces differ.
// That assertion is probabilistic and would flake roughly 1 in 2^32 runs while
// proving nothing a deterministic source cannot prove. Session-freshness BEHAVIOUR
// is tested with FixedNonceSource; OsNonceSource gets a smoke and an error-path test.

#[test]
fn os_nonce_source_smoke_produces_a_value() {
    let mut src = OsNonceSource;
    assert!(src.next_nonce().is_ok(), "OS entropy must be available on a desktop");
}

#[test]
fn os_nonce_source_can_be_called_repeatedly_without_error() {
    let mut src = OsNonceSource;
    for _ in 0..16 {
        src.next_nonce().expect("OS entropy remains available");
    }
}

#[test]
fn fixed_nonce_source_is_deterministic_for_tests() {
    let mut src = FixedNonceSource::new(vec![10, 20, 30]);
    assert_eq!(src.next_nonce(), Ok(10));
    assert_eq!(src.next_nonce(), Ok(20));
    assert_eq!(src.next_nonce(), Ok(30));
    // Exhausted sequences hold the last value rather than panicking mid-test.
    assert_eq!(src.next_nonce(), Ok(30));
}

/// A nonce source that cannot produce a value must fail the CONNECTION ATTEMPT,
/// never crash the desktop process.
#[test]
fn a_failing_nonce_source_reports_an_error_rather_than_panicking() {
    let mut src = FailingNonceSource;
    assert_eq!(src.next_nonce(), Err(NonceError::Unavailable));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-desktop --test nonce`
Expected: FAIL — `could not find nonce in device`.

- [ ] **Step 3: Add the dependency**

In `apps/desktop/src-tauri/Cargo.toml`, under `[dependencies]`:

```toml
getrandom = "0.2"  # session-nonce freshness (Slice 002). NOT a security credential; not a network crate.
```

- [ ] **Step 4: Write the implementation**

Create `apps/desktop/src-tauri/src/device/nonce.rs`:

```rust
//! Session-nonce freshness.
//!
//! The handshake nonce doubles as connection-scoped session identity: it is
//! stamped on every `InputEvent` and `Presentation` so stale traffic from a
//! previous connection cannot be mistaken for current traffic.
//!
//! The nonce therefore MUST be distinct across desktop process restarts, not
//! merely within one process. It is a freshness token, NOT a security credential.

/// Why a nonce could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceError {
    /// The OS entropy source was unavailable.
    Unavailable,
}

/// Supplies a fresh session nonce per handshake attempt.
///
/// Returns a `Result` deliberately: if the OS cannot provide randomness, the
/// correct behaviour is to fail THIS CONNECTION ATTEMPT and back off, not to
/// crash the desktop process out from under the user.
pub trait NonceSource: Send {
    fn next_nonce(&mut self) -> Result<u32, NonceError>;
}

/// Production source: OS-provided randomness.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsNonceSource;

impl NonceSource for OsNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        let mut buf = [0u8; 4];
        getrandom::getrandom(&mut buf).map_err(|_| NonceError::Unavailable)?;
        Ok(u32::from_le_bytes(buf))
    }
}

/// Deterministic source for tests.
#[derive(Debug, Clone)]
pub struct FixedNonceSource {
    values: Vec<u32>,
    index: usize,
}

impl FixedNonceSource {
    pub fn new(values: Vec<u32>) -> Self {
        assert!(!values.is_empty(), "FixedNonceSource needs at least one value");
        Self { values, index: 0 }
    }
}

impl NonceSource for FixedNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        let v = self.values[self.index];
        if self.index + 1 < self.values.len() {
            self.index += 1;
        }
        Ok(v)
    }
}

/// Always fails. Exercises the connection-attempt error path.
#[derive(Debug, Default, Clone, Copy)]
pub struct FailingNonceSource;

impl NonceSource for FailingNonceSource {
    fn next_nonce(&mut self) -> Result<u32, NonceError> {
        Err(NonceError::Unavailable)
    }
}
```

Add `pub mod nonce;` to `apps/desktop/src-tauri/src/device/mod.rs`.

- [ ] **Step 5: Use it in `Session`**

In `apps/desktop/src-tauri/src/device/session.rs`:

1. Replace the `next_nonce: u32` field with `nonce_source: Box<dyn NonceSource>`, defaulting to `Box::new(OsNonceSource)`.
2. Replace the counter body at the `Hello` build site. A nonce failure aborts **this attempt only** — it is handled exactly like a handshake I/O failure (record a `SafeDiagnostic`, apply the existing `ManagerEvent::IoError`, back off, retry). It must never panic:

```rust
        let nonce = match self.nonce_source.next_nonce() {
            Ok(nonce) => nonce,
            Err(NonceError::Unavailable) => {
                // No session identity means no no-stale-replay guarantee, so refuse
                // to open a session rather than proceed without one.
                return Err(SessionError::NonceUnavailable);
            }
        };
```

Add `NonceUnavailable` to the existing session error type and map it to the `io` diagnostic category in `diagnostics/redact.rs`. Add a test asserting that a `Session` built with `FailingNonceSource` returns that error and does **not** reach `Connected`.

3. Keep the sent nonce for the life of the connection and expose it:

```rust
    /// The nonce of the currently established session, if any.
    pub const fn current_session(&self) -> Option<u32> {
        self.current_session
    }
```

Set `self.current_session = Some(nonce)` when the handshake is evaluated as compatible, and `self.current_session = None` on any transition out of `Connected` and on `Bye`.

4. Add a constructor used only by tests: `Session::with_nonce_source(config, source: Box<dyn NonceSource>)`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p kivori-desktop`
Expected: PASS, including the pre-existing `session.rs`, `fsm.rs`, and `offline_smoke.rs` suites.

- [ ] **Step 7: Verify the offline guard still passes**

Run: `bash scripts/check-offline-deps.sh && bash scripts/check-crate-boundaries.sh`
Expected: both succeed.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src-tauri/src/device/nonce.rs apps/desktop/src-tauri/src/device/mod.rs \
        apps/desktop/src-tauri/src/device/session.rs apps/desktop/src-tauri/Cargo.toml \
        apps/desktop/src-tauri/tests/nonce.rs Cargo.lock
git commit -m "feat(desktop): make the handshake nonce fresh across process restarts"
```

---

### Task 6: Desktop input ingress and staleness rejection

Implements spec §4.1 rules 3 and 6, and both adversarial tests.

**Files:**
- Create: `apps/desktop/src-tauri/src/input/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Test: `apps/desktop/src-tauri/tests/rotary_loop.rs`

**Interfaces:**
- Consumes: `kivori_protocol::message::{InputEvent, InputKind, ControlId}`.
- Produces: `input::{InputIngress, LogicalInput, RejectReason}`; `InputIngress::new()`, `fn begin_session(&mut self, session: u32)`, `fn end_session(&mut self)`, `fn accept(&mut self, event: &InputEvent) -> Result<Option<LogicalInput>, RejectReason>`.

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src-tauri/tests/rotary_loop.rs`:

```rust
use kivori_desktop::input::{InputIngress, LogicalInput, RejectReason};
use kivori_model::input::Direction;
use kivori_protocol::message::{ControlId, InputEvent, InputKind};

fn ev(session: u32, gesture_id: u16, kind: InputKind) -> InputEvent {
    InputEvent {
        session,
        gesture_id,
        control: ControlId::Rotary,
        kind,
        device_ms: 0,
    }
}

#[test]
fn a_started_gesture_admits_its_detents() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::GestureStarted)),
        Ok(Some(LogicalInput::GestureStarted { gesture_id: 1 }))
    );
    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::Detent(Direction::Cw))),
        Ok(Some(LogicalInput::Detent {
            gesture_id: 1,
            direction: Direction::Cw
        }))
    );
    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::GestureEnded)),
        Ok(Some(LogicalInput::GestureEnded { gesture_id: 1 }))
    );
}

#[test]
fn a_detent_for_a_gesture_that_never_started_is_rejected() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 7, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}

/// THE ADVERSARIAL CASE that removed the epoch-free design.
/// A COMPLETE stale pair — GestureStarted AND Detent — replayed after a reconnect.
#[test]
fn a_complete_stale_gesture_pair_after_reconnect_executes_nothing() {
    let mut ingress = InputIngress::new();

    ingress.begin_session(0x1111);
    ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)).unwrap();

    // Link drops, a new session is established with a different nonce.
    ingress.end_session();
    ingress.begin_session(0x2222);

    // Both halves of the old gesture arrive, in order, from the stale buffer.
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)),
        Err(RejectReason::StaleSession)
    );
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::StaleSession)
    );
}

#[test]
fn gesture_ids_reused_by_a_new_session_do_not_inherit_old_state() {
    let mut ingress = InputIngress::new();

    ingress.begin_session(0x1111);
    ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)).unwrap();

    ingress.end_session();
    ingress.begin_session(0x2222);

    // Same gesture id, new session: the open-gesture set was cleared, so a bare
    // detent has no start to attach to.
    assert_eq!(
        ingress.accept(&ev(0x2222, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}

#[test]
fn input_outside_a_session_is_rejected() {
    let mut ingress = InputIngress::new();
    assert_eq!(
        ingress.accept(&ev(0x1111, 1, InputKind::GestureStarted)),
        Err(RejectReason::NoSession)
    );
}

#[test]
fn a_gesture_cannot_continue_after_it_ended() {
    let mut ingress = InputIngress::new();
    ingress.begin_session(0xAAAA);
    ingress.accept(&ev(0xAAAA, 1, InputKind::GestureStarted)).unwrap();
    ingress.accept(&ev(0xAAAA, 1, InputKind::GestureEnded)).unwrap();

    assert_eq!(
        ingress.accept(&ev(0xAAAA, 1, InputKind::Detent(Direction::Cw))),
        Err(RejectReason::UnknownGesture)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: FAIL — `could not find input in kivori_desktop`.

- [ ] **Step 3: Write the implementation**

Create `apps/desktop/src-tauri/src/input/mod.rs`:

```rust
//! Input ingress: turns wire `InputEvent`s into validated `LogicalInput`.
//!
//! Two independent freshness rules (design spec section 4.1):
//!   1. the event's session MUST be the current session nonce;
//!   2. a gesture MUST have been observed to start in THIS session.
//!
//! Rule 1 alone is sufficient against a complete stale pair; rule 2 is retained
//! as defence in depth against intra-session reordering.

use kivori_model::input::Direction;
use kivori_protocol::message::{InputEvent, InputKind};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalInput {
    GestureStarted { gesture_id: u16 },
    Detent { gesture_id: u16, direction: Direction },
    GestureEnded { gesture_id: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// No session is currently established.
    NoSession,
    /// The event belongs to a different session than the current one.
    StaleSession,
    /// No `GestureStarted` for this gesture was observed in this session.
    UnknownGesture,
}

#[derive(Debug, Default)]
pub struct InputIngress {
    session: Option<u32>,
    open_gestures: HashSet<u16>,
}

impl InputIngress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_session(&mut self, session: u32) {
        self.session = Some(session);
        self.open_gestures.clear();
    }

    /// Called on every exit from `Connected`.
    pub fn end_session(&mut self) {
        self.session = None;
        self.open_gestures.clear();
    }

    pub fn accept(&mut self, event: &InputEvent) -> Result<Option<LogicalInput>, RejectReason> {
        let Some(current) = self.session else {
            return Err(RejectReason::NoSession);
        };
        if event.session != current {
            return Err(RejectReason::StaleSession);
        }

        match event.kind {
            InputKind::GestureStarted => {
                self.open_gestures.insert(event.gesture_id);
                Ok(Some(LogicalInput::GestureStarted {
                    gesture_id: event.gesture_id,
                }))
            }
            InputKind::Detent(direction) => {
                if !self.open_gestures.contains(&event.gesture_id) {
                    return Err(RejectReason::UnknownGesture);
                }
                Ok(Some(LogicalInput::Detent {
                    gesture_id: event.gesture_id,
                    direction,
                }))
            }
            InputKind::GestureEnded => {
                if !self.open_gestures.remove(&event.gesture_id) {
                    return Err(RejectReason::UnknownGesture);
                }
                Ok(Some(LogicalInput::GestureEnded {
                    gesture_id: event.gesture_id,
                }))
            }
        }
    }
}
```

Add `pub mod input;` to `apps/desktop/src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/input/ apps/desktop/src-tauri/src/lib.rs \
        apps/desktop/src-tauri/tests/rotary_loop.rs
git commit -m "feat(desktop): reject stale-session and unstarted-gesture input"
```

---

### Task 7: `VolumeBackend` trait, fake backend, contract suite

One suite that the fake **and** the real Windows backend must both satisfy.

**Files:**
- Create: `apps/desktop/src-tauri/src/platform/mod.rs`
- Create: `apps/desktop/src-tauri/src/platform/unimplemented.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Test: `apps/desktop/src-tauri/tests/volume_backend_contract.rs`

**Interfaces:**
- Produces: `platform::{VolumeBackend, BackendError, BackendAvailability, ConfirmationClass, ActionAvailability, FakeVolumeBackend}`.
  - `fn availability(&self) -> ActionAvailability`
  - `fn read(&self) -> Result<u8, BackendError>`
  - `fn set(&self, percent: u8) -> Result<u8, BackendError>` — returns the **read-back** value.
  - `FakeVolumeBackend::new(initial: u8)`, `::with_availability(a: ActionAvailability)`, `::unreadable_after_write(initial: u8)`, `fn external_change(&self, percent: u8)`.

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src-tauri/tests/volume_backend_contract.rs`:

```rust
use kivori_desktop::platform::{
    ActionAvailability, BackendError, ConfirmationClass, FakeVolumeBackend, VolumeBackend,
};

/// Restores the endpoint's original volume when dropped, including on panic.
///
/// A test that changes a real person's system volume MUST put it back. `Drop` runs
/// during unwind, so an assertion failure mid-test still restores.
pub struct VolumeGuard<'a> {
    backend: &'a dyn VolumeBackend,
    original: u8,
}

impl<'a> VolumeGuard<'a> {
    pub fn capture(backend: &'a dyn VolumeBackend) -> Option<Self> {
        let original = backend.read().ok()?;
        Some(Self { backend, original })
    }
    pub const fn original(&self) -> u8 {
        self.original
    }
}

impl Drop for VolumeGuard<'_> {
    fn drop(&mut self) {
        let _ = self.backend.set(self.original);
    }
}

/// NON-DESTRUCTIVE contract. Safe against a real endpoint: it captures the original
/// value, nudges it by a small amount, verifies read-back, and restores.
///
/// Exact-boundary behaviour (0 and 100) is NOT exercised here — slamming a real
/// user's speakers to silent or full is not an acceptable test side effect. Those
/// assertions live in `assert_boundary_contract`, which runs on fakes only.
pub fn assert_backend_contract(backend: &dyn VolumeBackend) {
    let ActionAvailability::Available { confirmation } = backend.availability() else {
        panic!("contract suite requires an available backend");
    };
    assert_eq!(
        confirmation,
        ConfirmationClass::StateConfirmed,
        "a backend with read-back must report StateConfirmed"
    );

    let guard = VolumeGuard::capture(backend).expect("read original volume");
    let original = guard.original();

    // A small, safe nudge that stays well inside the range from any starting point.
    let target = if original >= 50 { original - 5 } else { original + 5 };

    // set() returns the READ-BACK value, never the requested one.
    let observed = backend.set(target).expect("set");
    assert_eq!(backend.read().expect("read"), observed);
    assert!(
        observed.abs_diff(target) <= 2,
        "observed {observed} should track requested {target} within quantisation"
    );

    // `guard` restores the original volume here, or on unwind if an assert above failed.
}

/// DESTRUCTIVE exact-boundary contract. Fakes only — never a real endpoint.
pub fn assert_boundary_contract(backend: &dyn VolumeBackend) {
    assert_eq!(backend.set(0).expect("set 0"), 0);
    assert_eq!(backend.set(100).expect("set 100"), 100);
}

#[test]
fn fake_backend_satisfies_the_contract() {
    assert_backend_contract(&FakeVolumeBackend::new(25));
}

#[test]
fn fake_backend_satisfies_the_exact_boundary_contract() {
    assert_boundary_contract(&FakeVolumeBackend::new(25));
}

#[test]
fn the_volume_guard_restores_the_original_value_even_on_panic() {
    let backend = FakeVolumeBackend::new(42);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = VolumeGuard::capture(&backend).expect("capture");
        backend.set(7).expect("set");
        panic!("simulated test failure");
    }));
    assert!(result.is_err());
    assert_eq!(
        backend.read().expect("read"),
        42,
        "the guard must restore the original volume during unwind"
    );
}

#[test]
fn set_returns_read_back_not_the_request() {
    // A backend that quantises must report what it actually achieved.
    let backend = FakeVolumeBackend::quantised(25, /* step */ 5);
    assert_eq!(backend.set(52).expect("set"), 50);
    assert_eq!(backend.read().expect("read"), 50);
}

#[test]
fn an_unavailable_backend_reports_runtime_unavailable_and_refuses_reads() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    assert!(matches!(
        backend.availability(),
        ActionAvailability::RuntimeUnavailable { .. }
    ));
    assert_eq!(backend.read(), Err(BackendError::NoEndpoint));
}

#[test]
fn a_write_without_readback_is_reported_distinctly() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    assert_eq!(backend.set(40), Err(BackendError::ReadBackUnavailable));
}

#[test]
fn external_change_is_observable_without_a_kivori_write() {
    let backend = FakeVolumeBackend::new(10);
    backend.external_change(77);
    assert_eq!(backend.read().expect("read"), 77);
}

#[test]
fn the_unimplemented_backend_says_not_implemented_yet_not_unsupported() {
    // A platform Kivori has not been built for must NOT claim the OS cannot do it.
    let backend = kivori_desktop::platform::unimplemented::UnimplementedVolumeBackend::new("macos");
    match backend.availability() {
        ActionAvailability::NotImplementedYet { target } => assert_eq!(target, "macos"),
        other => panic!("expected NotImplementedYet, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-desktop --test volume_backend_contract`
Expected: FAIL — `could not find platform in kivori_desktop`.

- [ ] **Step 3: Write the implementation**

Create `apps/desktop/src-tauri/src/platform/mod.rs`:

```rust
//! Platform capability boundary.
//!
//! Three orthogonal concepts, deliberately never collapsed (design spec section 6):
//!   * BackendAvailability — a fact about THIS Kivori build and its runtime;
//!   * ConfirmationClass   — how well an executed action's outcome can be observed;
//!   * ActionAvailability  — the derived value the UI consumes.
//!
//! A `PlatformCapability` type (a fact about the MACHINE) is intentionally absent:
//! every action in Slice 002 is one the OS can perform, so such a type would have
//! no producer. It arrives with the first genuinely OS-restricted action.

pub mod unimplemented;

use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendAvailability {
    Available,
    /// Kivori has no backend compiled for this OS. NOT the same as unsupported.
    NotImplemented { target: &'static str },
    /// Implemented, but a runtime dependency is missing right now.
    RuntimeUnavailable { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationClass {
    StateConfirmed,
    ExecutionConfirmed,
    TriggeredUnverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionAvailability {
    Available { confirmation: ConfirmationClass },
    NotImplementedYet { target: &'static str },
    RuntimeUnavailable { reason: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// No default render endpoint exists.
    NoEndpoint,
    /// The write was accepted but the resulting state could not be read back.
    ReadBackUnavailable,
    Os(String),
}

/// Master output volume, 0..=100.
pub trait VolumeBackend: Send + Sync {
    fn availability(&self) -> ActionAvailability;
    fn read(&self) -> Result<u8, BackendError>;
    /// Apply a value and return the value actually observed afterwards.
    ///
    /// Returning the read-back rather than `()` is what makes StateConfirmed
    /// structurally honest: the confirmed value comes from the OS, never the request.
    fn set(&self, percent: u8) -> Result<u8, BackendError>;
}

#[derive(Debug)]
pub struct FakeVolumeBackend {
    state: Mutex<u8>,
    availability: ActionAvailability,
    quantise_step: Option<u8>,
    readable_after_write: bool,
}

impl FakeVolumeBackend {
    pub fn new(initial: u8) -> Self {
        Self {
            state: Mutex::new(initial),
            availability: ActionAvailability::Available {
                confirmation: ConfirmationClass::StateConfirmed,
            },
            quantise_step: None,
            readable_after_write: true,
        }
    }

    /// Models a device whose driver snaps to coarse steps.
    pub fn quantised(initial: u8, step: u8) -> Self {
        Self {
            quantise_step: Some(step),
            ..Self::new(initial)
        }
    }

    pub fn with_availability(availability: ActionAvailability) -> Self {
        Self {
            availability,
            ..Self::new(0)
        }
    }

    pub fn unreadable_after_write(initial: u8) -> Self {
        Self {
            readable_after_write: false,
            ..Self::new(initial)
        }
    }

    /// Simulates a change made outside Kivori (the Windows flyout, a media key).
    pub fn external_change(&self, percent: u8) {
        *self.state.lock().expect("fake backend mutex") = percent;
    }
}

impl VolumeBackend for FakeVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        self.availability.clone()
    }

    fn read(&self) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        Ok(*self.state.lock().expect("fake backend mutex"))
    }

    fn set(&self, percent: u8) -> Result<u8, BackendError> {
        if !matches!(self.availability, ActionAvailability::Available { .. }) {
            return Err(BackendError::NoEndpoint);
        }
        let applied = match self.quantise_step {
            Some(step) if step > 0 => (percent / step) * step,
            _ => percent,
        };
        *self.state.lock().expect("fake backend mutex") = applied;
        if !self.readable_after_write {
            return Err(BackendError::ReadBackUnavailable);
        }
        Ok(applied)
    }
}
```

Create `apps/desktop/src-tauri/src/platform/unimplemented.rs`:

```rust
//! The honest fallback for targets Kivori has not implemented a backend for.
//!
//! This reports `NotImplementedYet`, never "unsupported": macOS Core Audio and
//! Linux PipeWire can both change master volume. Claiming otherwise would tell a
//! user their machine cannot do something it can.

use super::{ActionAvailability, BackendError, VolumeBackend};

#[derive(Debug, Clone, Copy)]
pub struct UnimplementedVolumeBackend {
    target: &'static str,
}

impl UnimplementedVolumeBackend {
    pub const fn new(target: &'static str) -> Self {
        Self { target }
    }
}

impl VolumeBackend for UnimplementedVolumeBackend {
    fn availability(&self) -> ActionAvailability {
        ActionAvailability::NotImplementedYet {
            target: self.target,
        }
    }

    fn read(&self) -> Result<u8, BackendError> {
        Err(BackendError::NoEndpoint)
    }

    fn set(&self, _percent: u8) -> Result<u8, BackendError> {
        Err(BackendError::NoEndpoint)
    }
}
```

Add `pub mod platform;` to `apps/desktop/src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kivori-desktop --test volume_backend_contract`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/platform/ apps/desktop/src-tauri/src/lib.rs \
        apps/desktop/src-tauri/tests/volume_backend_contract.rs
git commit -m "feat(desktop): add VolumeBackend contract, fake, and honest unimplemented fallback"
```

---

### Task 8: Rotary action — binding, fixed step, clamp, outcome

**Files:**
- Create: `apps/desktop/src-tauri/src/action/mod.rs`
- Create: `apps/desktop/src-tauri/src/action/volume.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Test: `apps/desktop/src-tauri/tests/rotary_loop.rs` (append)

**Interfaces:**
- Consumes: `platform::{VolumeBackend, BackendError, ActionAvailability, ConfirmationClass}`, `kivori_model::input::Direction`.
- Produces: `action::{ActionId, Outcome, resolve_binding}`; `action::volume::{BASE_STEP_PERCENT, apply_step}`.
  - `resolve_binding(control: ControlId) -> Option<ActionId>`
  - `apply_step(current: u8, direction: Direction) -> (u8, bool)` → `(next, at_boundary)`

- [ ] **Step 1: Write the failing test**

Append to `apps/desktop/src-tauri/tests/rotary_loop.rs`:

```rust
use kivori_desktop::action::volume::{apply_step, BASE_STEP_PERCENT};
use kivori_desktop::action::{resolve_binding, ActionId, Outcome};
use kivori_desktop::platform::{
    ActionAvailability, ConfirmationClass, FakeVolumeBackend, VolumeBackend,
};

#[test]
fn the_global_rotary_binding_resolves_to_master_volume() {
    assert_eq!(resolve_binding(ControlId::Rotary), Some(ActionId::MasterVolume));
}

#[test]
fn one_detent_moves_exactly_the_base_step() {
    assert_eq!(apply_step(50, Direction::Cw), (50 + BASE_STEP_PERCENT, false));
    assert_eq!(apply_step(50, Direction::Ccw), (50 - BASE_STEP_PERCENT, false));
}

#[test]
fn values_clamp_at_both_bounds_and_flag_the_boundary() {
    assert_eq!(apply_step(100, Direction::Cw), (100, true));
    assert_eq!(apply_step(0, Direction::Ccw), (0, true));
    // Approaching the bound from inside one step lands exactly on it, not past it.
    assert_eq!(apply_step(99, Direction::Cw), (100, false));
    assert_eq!(apply_step(1, Direction::Ccw), (0, false));
}

#[test]
fn the_first_reverse_detent_leaves_the_boundary_immediately() {
    let (at_max, boundary) = apply_step(100, Direction::Cw);
    assert!(boundary);
    assert_eq!(apply_step(at_max, Direction::Ccw), (100 - BASE_STEP_PERCENT, false));
}

#[test]
fn a_successful_set_with_readback_is_state_confirmed() {
    let backend = FakeVolumeBackend::new(40);
    let outcome = kivori_desktop::action::execute_volume(&backend, 60);
    assert_eq!(outcome, Outcome::StateConfirmed { volume_percent: 60 });
}

#[test]
fn state_confirmed_reports_the_observed_value_not_the_requested_one() {
    let backend = FakeVolumeBackend::quantised(40, 5);
    let outcome = kivori_desktop::action::execute_volume(&backend, 62);
    assert_eq!(
        outcome,
        Outcome::StateConfirmed { volume_percent: 60 },
        "confirmation must carry OS truth, never the request"
    );
}

#[test]
fn a_write_without_readback_is_unverified_never_confirmed() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    assert_eq!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::TriggeredUnverified
    );
}

#[test]
fn an_unavailable_backend_fails_rather_than_silently_succeeding() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    assert!(matches!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::Failed { .. }
    ));
}

#[test]
fn an_unimplemented_backend_does_not_attempt_execution() {
    let backend =
        kivori_desktop::platform::unimplemented::UnimplementedVolumeBackend::new("macos");
    assert!(matches!(
        kivori_desktop::action::execute_volume(&backend, 40),
        Outcome::Failed { .. }
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: FAIL — `could not find action in kivori_desktop`.

- [ ] **Step 3: Write the implementation**

Create `apps/desktop/src-tauri/src/action/volume.rs`:

```rust
//! Rotary volume value model: fixed step, immediate clamp, boundary flag.
//!
//! Acceleration is deliberately absent in Slice 002. With a fixed step there is
//! no multiplier state, so direction reversal is correct by construction.

use kivori_model::input::Direction;

/// Volume points moved per validated detent. Provisional; UX tuning parameter.
pub const BASE_STEP_PERCENT: u8 = 2;

const MIN_PERCENT: i16 = 0;
const MAX_PERCENT: i16 = 100;

/// Apply one detent. Returns `(next_value, at_boundary)`.
///
/// `at_boundary` is true only when the motion was ENTIRELY absorbed by the clamp,
/// so continued pressure into a bound does not re-trigger feedback while a value
/// that merely lands on the bound does not raise it.
pub fn apply_step(current: u8, direction: Direction) -> (u8, bool) {
    let delta = match direction {
        Direction::Cw => i16::from(BASE_STEP_PERCENT),
        Direction::Ccw => -i16::from(BASE_STEP_PERCENT),
    };
    let raw = i16::from(current) + delta;
    let clamped = raw.clamp(MIN_PERCENT, MAX_PERCENT);
    let absorbed = clamped == i16::from(current);
    (clamped as u8, absorbed)
}
```

Create `apps/desktop/src-tauri/src/action/mod.rs`:

```rust
//! Action identity, the Slice 002 binding table, and typed execution outcomes.

pub mod volume;

use crate::platform::{ActionAvailability, BackendError, VolumeBackend};
use kivori_protocol::message::ControlId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    MasterVolume,
}

/// Typed execution outcome.
///
/// Execution MUST NOT collapse into `bool success` (user-story-contract US2).
/// Slice 002 produces StateConfirmed, TriggeredUnverified, and Failed; the other
/// arms exist because later slices produce them and the distinction is contractual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    StateConfirmed { volume_percent: u8 },
    ExecutionConfirmed,
    TriggeredUnverified,
    Failed { reason: String },
}

/// The entire Slice 002 binding table: one Global bidirectional rotary binding.
///
/// A bidirectional `Rotate` binding owns BOTH directions as one logical control
/// (user-story-contract invariant 54).
pub const fn resolve_binding(control: ControlId) -> Option<ActionId> {
    match control {
        ControlId::Rotary => Some(ActionId::MasterVolume),
    }
}

/// Execute a volume target against a backend and classify the outcome honestly.
pub fn execute_volume(backend: &dyn VolumeBackend, target_percent: u8) -> Outcome {
    match backend.availability() {
        ActionAvailability::Available { .. } => {}
        ActionAvailability::NotImplementedYet { target } => {
            return Outcome::Failed {
                reason: format!("no volume backend implemented for {target}"),
            }
        }
        ActionAvailability::RuntimeUnavailable { reason } => {
            return Outcome::Failed { reason }
        }
        ActionAvailability::Unknown => {
            return Outcome::Failed {
                reason: "backend availability unknown".to_string(),
            }
        }
    }

    match backend.set(target_percent) {
        Ok(observed) => Outcome::StateConfirmed {
            volume_percent: observed,
        },
        // The write landed but its result cannot be observed: never claim success.
        Err(BackendError::ReadBackUnavailable) => Outcome::TriggeredUnverified,
        Err(BackendError::NoEndpoint) => Outcome::Failed {
            reason: "no default render endpoint".to_string(),
        },
        Err(BackendError::Os(reason)) => Outcome::Failed { reason },
    }
}
```

Add `pub mod action;` to `apps/desktop/src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: PASS, 15 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/action/ apps/desktop/src-tauri/src/lib.rs \
        apps/desktop/src-tauri/tests/rotary_loop.rs
git commit -m "feat(desktop): add fixed-step rotary volume action with typed outcomes"
```

---

### Task 9: Preview ownership and reconciliation — the vertical loop closes

After this task the whole loop runs against the fake backend, with no Windows and no hardware.

**Files:**
- Create: `apps/desktop/src-tauri/src/action/gesture_value.rs`
- Modify: `apps/desktop/src-tauri/src/action/mod.rs`
- Test: `apps/desktop/src-tauri/tests/rotary_loop.rs` (append)

**Interfaces:**
- Consumes: `LogicalInput`, `apply_step`, `execute_volume`, `VolumeBackend`.
- Produces: `action::gesture_value::{GestureValue, ValueUpdate}`; `GestureValue::new()`, `fn on_input(&mut self, input: LogicalInput, backend: &dyn VolumeBackend) -> Option<ValueUpdate>`, `fn on_external_change(&mut self, percent: u8) -> Option<ValueUpdate>`, `fn on_endpoint_rebind(&mut self, percent: u8) -> Option<ValueUpdate>`.
- `ValueUpdate { percent: u8, confidence: ValueConfidence, at_boundary: bool }`.

- [ ] **Step 1: Write the failing test**

Append to `apps/desktop/src-tauri/tests/rotary_loop.rs`:

```rust
use kivori_desktop::action::gesture_value::{GestureValue, ValueUpdate};
use kivori_model::presentation::ValueConfidence;

fn preview(percent: u8) -> Option<ValueUpdate> {
    Some(ValueUpdate {
        percent,
        confidence: ValueConfidence::Preview,
        at_boundary: false,
    })
}

fn confirmed(percent: u8) -> Option<ValueUpdate> {
    Some(ValueUpdate {
        percent,
        confidence: ValueConfidence::Confirmed,
        at_boundary: false,
    })
}

#[test]
fn detents_during_a_gesture_are_preview_never_confirmed() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend
        ),
        preview(52)
    );
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend
        ),
        preview(54)
    );
}

#[test]
fn gesture_end_reconciles_to_the_value_the_backend_reports() {
    // The backend quantises, so the confirmed value differs from the preview.
    let backend = FakeVolumeBackend::quantised(50, 5);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend
        ),
        preview(52)
    );
    assert_eq!(
        gv.on_input(LogicalInput::GestureEnded { gesture_id: 1 }, &backend),
        confirmed(50),
        "confirmed state wins over the local preview"
    );
}

#[test]
fn an_external_change_during_a_gesture_does_not_overwrite_the_preview() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    gv.on_input(
        LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
        &backend,
    );

    // Somebody moves the Windows flyout mid-gesture.
    assert_eq!(
        gv.on_external_change(10),
        None,
        "external updates must not visually fight an active gesture"
    );

    // It is applied once the gesture ends.
    backend.external_change(10);
    assert_eq!(
        gv.on_input(LogicalInput::GestureEnded { gesture_id: 1 }, &backend),
        confirmed(10)
    );
}

#[test]
fn an_external_change_outside_a_gesture_is_confirmed_immediately() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();
    assert_eq!(gv.on_external_change(77), confirmed(77));
}

#[test]
fn a_write_without_readback_reconciles_as_unverified() {
    let backend = FakeVolumeBackend::unreadable_after_write(30);
    let mut gv = GestureValue::new();

    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    let update = gv
        .on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend,
        )
        .expect("update");
    assert_eq!(update.confidence, ValueConfidence::Unverified);
    assert_ne!(
        update.confidence,
        ValueConfidence::Confirmed,
        "an unobservable write must never read as confirmed"
    );
}

#[test]
fn confidence_never_reaches_confirmed_without_a_backend_read() {
    let backend = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    let update = gv.on_input(
        LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
        &backend,
    );
    if let Some(u) = update {
        assert_ne!(u.confidence, ValueConfidence::Confirmed);
    }
}

#[test]
fn boundary_is_flagged_only_while_pressure_continues_into_the_bound() {
    let backend = FakeVolumeBackend::new(100);
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);

    let update = gv
        .on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend,
        )
        .expect("update");
    assert!(update.at_boundary);
    assert_eq!(update.percent, 100);

    let reversed = gv
        .on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Ccw },
            &backend,
        )
        .expect("update");
    assert!(!reversed.at_boundary);
    assert_eq!(reversed.percent, 98);
}

#[test]
fn an_endpoint_rebind_mid_gesture_discards_the_gesture_rather_than_retargeting() {
    let backend = FakeVolumeBackend::new(50);
    let mut gv = GestureValue::new();
    gv.on_input(LogicalInput::GestureStarted { gesture_id: 1 }, &backend);
    gv.on_input(
        LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
        &backend,
    );

    // The user switched output device. The new endpoint is at 20.
    assert_eq!(gv.on_endpoint_rebind(20), confirmed(20));

    // Remaining detents of the abandoned gesture must not drive the NEW endpoint.
    assert_eq!(
        gv.on_input(
            LogicalInput::Detent { gesture_id: 1, direction: Direction::Cw },
            &backend
        ),
        None
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: FAIL — `could not find gesture_value in action`.

- [ ] **Step 3: Write the implementation**

Create `apps/desktop/src-tauri/src/action/gesture_value.rs`:

```rust
//! Preview ownership and reconciliation for the continuous volume value.
//!
//! While a gesture is open the local target owns the display (Preview). External
//! changes are recorded but never rendered over an active gesture. On gesture end
//! the value reconciles to what the backend reports (Confirmed) — desktop truth
//! wins (user-story-contract invariants 1 and 30, US3, US4).

use super::volume::apply_step;
use super::{execute_volume, Outcome};
use crate::input::LogicalInput;
use crate::platform::VolumeBackend;
use kivori_model::presentation::ValueConfidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueUpdate {
    pub percent: u8,
    pub confidence: ValueConfidence,
    pub at_boundary: bool,
}

#[derive(Debug, Default)]
pub struct GestureValue {
    /// `Some(gesture_id)` while a gesture owns the preview.
    active: Option<u16>,
    /// Gesture-local target, seeded from the backend when the gesture opens.
    target: u8,
    /// Set when an endpoint rebind invalidated the in-flight gesture.
    abandoned: bool,
}

impl GestureValue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_input(
        &mut self,
        input: LogicalInput,
        backend: &dyn VolumeBackend,
    ) -> Option<ValueUpdate> {
        match input {
            LogicalInput::GestureStarted { gesture_id } => {
                self.active = Some(gesture_id);
                self.abandoned = false;
                self.target = backend.read().unwrap_or(self.target);
                None
            }
            LogicalInput::Detent { gesture_id, direction } => {
                if self.active != Some(gesture_id) || self.abandoned {
                    return None;
                }
                let (next, at_boundary) = apply_step(self.target, direction);
                self.target = next;

                let confidence = match execute_volume(backend, next) {
                    // A confirmed write still displays as Preview while the gesture
                    // owns the surface; it is promoted at gesture end.
                    Outcome::StateConfirmed { .. } => ValueConfidence::Preview,
                    Outcome::TriggeredUnverified => ValueConfidence::Unverified,
                    _ => ValueConfidence::Unverified,
                };

                Some(ValueUpdate {
                    percent: next,
                    confidence,
                    at_boundary,
                })
            }
            LogicalInput::GestureEnded { gesture_id } => {
                if self.active != Some(gesture_id) {
                    return None;
                }
                self.active = None;
                if self.abandoned {
                    self.abandoned = false;
                    return None;
                }
                let observed = backend.read().ok()?;
                self.target = observed;
                Some(ValueUpdate {
                    percent: observed,
                    confidence: ValueConfidence::Confirmed,
                    at_boundary: false,
                })
            }
        }
    }

    /// A change Kivori did not originate. Ignored while a gesture owns the preview.
    pub fn on_external_change(&mut self, percent: u8) -> Option<ValueUpdate> {
        if self.active.is_some() {
            return None;
        }
        self.target = percent;
        Some(ValueUpdate {
            percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        })
    }

    /// The default render endpoint changed. The value being controlled is now a
    /// different thing, so an in-flight gesture is abandoned rather than retargeted.
    pub fn on_endpoint_rebind(&mut self, percent: u8) -> Option<ValueUpdate> {
        if self.active.is_some() {
            self.abandoned = true;
        }
        self.target = percent;
        Some(ValueUpdate {
            percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        })
    }
}
```

Add `pub mod gesture_value;` to `apps/desktop/src-tauri/src/action/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kivori-desktop --test rotary_loop`
Expected: PASS, 23 tests.

- [ ] **Step 5: Run the full host suite**

Run: `just test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/action/gesture_value.rs \
        apps/desktop/src-tauri/src/action/mod.rs apps/desktop/src-tauri/tests/rotary_loop.rs
git commit -m "feat(desktop): own the preview during a gesture and reconcile to confirmed truth"
```

---

### Task 10: Shared volume overlay and golden frames

One canonical visual model: Device Studio preview and firmware draw the **same** function (engineering principle 2).

**Files:**
- Create: `crates/kivori-renderer/src/overlay.rs`
- Modify: `crates/kivori-renderer/src/lib.rs`
- Create: `tests/golden-frames/tests/overlay_golden.rs`
- Modify: `tests/golden-frames/manifest.toml`

**Interfaces:**
- Consumes: `kivori_model::presentation::ValueConfidence`, `kivori_framebuffer::TileBand`, `kivori_model::color::Rgb565`.
- Produces: `kivori_renderer::overlay::render_volume_overlay(band: &mut TileBand, percent: u8, confidence: ValueConfidence, at_boundary: bool)`.

- [ ] **Step 1: Write the failing test**

Create `tests/golden-frames/tests/overlay_golden.rs`:

```rust
use kivori_framebuffer::TileBand;
use kivori_model::presentation::ValueConfidence;
use kivori_model::{Rect, Rgb565};
use kivori_renderer::hash::frame_hash;
use kivori_renderer::overlay::render_volume_overlay;

const W: u16 = 240;
const H: u16 = 240;

fn draw(percent: u8, confidence: ValueConfidence, at_boundary: bool) -> Vec<Rgb565> {
    let mut pixels = vec![Rgb565::BLACK; (W as usize) * (H as usize)];
    // Rect::new takes (x, y, w, h) in absolute display coordinates.
    let rect = Rect::new(0, 0, W, H);
    let mut band = TileBand::new(rect, &mut pixels).expect("full-frame band");
    render_volume_overlay(&mut band, percent, confidence, at_boundary);
    pixels
}

#[test]
fn overlay_is_deterministic_across_repeated_renders() {
    let a = draw(50, ValueConfidence::Confirmed, false);
    let b = draw(50, ValueConfidence::Confirmed, false);
    assert_eq!(frame_hash(&a), frame_hash(&b));
}

/// The whole point of ValueConfidence: a preview must not LOOK like confirmed truth.
#[test]
fn the_three_confidences_are_visually_distinguishable() {
    let preview = frame_hash(&draw(50, ValueConfidence::Preview, false));
    let confirmed = frame_hash(&draw(50, ValueConfidence::Confirmed, false));
    let unverified = frame_hash(&draw(50, ValueConfidence::Unverified, false));

    assert_ne!(preview, confirmed, "Preview must not render as Confirmed");
    assert_ne!(preview, unverified, "Preview must not render as Unverified");
    assert_ne!(confirmed, unverified, "Confirmed must not render as Unverified");
}

#[test]
fn different_percentages_render_differently() {
    assert_ne!(
        frame_hash(&draw(0, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, false))
    );
    assert_ne!(
        frame_hash(&draw(49, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(51, ValueConfidence::Confirmed, false))
    );
}

#[test]
fn the_boundary_flag_changes_the_rendering() {
    assert_ne!(
        frame_hash(&draw(100, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, true))
    );
}

#[test]
fn bounds_render_without_panicking_and_are_distinct() {
    let zero = frame_hash(&draw(0, ValueConfidence::Confirmed, false));
    let full = frame_hash(&draw(100, ValueConfidence::Confirmed, false));
    assert_ne!(zero, full);
}

/// Out-of-range input is clamped, never allowed to index outside the band.
#[test]
fn percent_above_one_hundred_is_clamped_to_full() {
    assert_eq!(
        frame_hash(&draw(200, ValueConfidence::Confirmed, false)),
        frame_hash(&draw(100, ValueConfidence::Confirmed, false))
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kivori-golden-frames --test overlay_golden`
Expected: FAIL — `could not find overlay in kivori_renderer`.

- [ ] **Step 3: Write the implementation**

Create `crates/kivori-renderer/src/overlay.rs`:

```rust
//! The canonical volume overlay.
//!
//! Solid rects only — no glyphs, no font tables, no asset-blob content. Integer
//! arithmetic throughout, so host preview and firmware produce byte-identical
//! output (engineering principles 2 and 3).
//!
//! Each `ValueConfidence` MUST render distinguishably: an optimistic preview that
//! looked like confirmed truth would defeat the reason the field exists.

use kivori_framebuffer::TileBand;
use kivori_model::presentation::ValueConfidence;
use kivori_model::Rgb565;

const BAR_X: u16 = 24;
const BAR_W: u16 = 192;
const BAR_Y: u16 = 180;
const BAR_H: u16 = 16;
const BORDER: u16 = 2;

/// Dashed-fill period used by `Unverified`, in pixels.
const DASH_PERIOD: u16 = 6;
const DASH_ON: u16 = 3;

/// Draw the volume bar into `band`. Coordinates are frame-absolute; pixels outside
/// the band are clipped by `TileBand::set`.
pub fn render_volume_overlay(
    band: &mut TileBand,
    percent: u8,
    confidence: ValueConfidence,
    at_boundary: bool,
) {
    let percent = if percent > 100 { 100 } else { percent };

    let track = if at_boundary {
        Rgb565::WHITE
    } else {
        Rgb565::from_rgb888(96, 96, 96)
    };
    let fill = Rgb565::WHITE;

    // Track outline.
    for x in BAR_X..BAR_X + BAR_W {
        for y in BAR_Y..BAR_Y + BAR_H {
            let on_edge = x < BAR_X + BORDER
                || x >= BAR_X + BAR_W - BORDER
                || y < BAR_Y + BORDER
                || y >= BAR_Y + BAR_H - BORDER;
            if on_edge {
                band.set(x, y, track);
            }
        }
    }

    // Filled width: integer-only, exact at both bounds.
    let inner_x = BAR_X + BORDER;
    let inner_w = BAR_W - 2 * BORDER;
    let inner_y = BAR_Y + BORDER;
    let inner_h = BAR_H - 2 * BORDER;
    let filled = (u32::from(inner_w) * u32::from(percent) / 100) as u16;

    for x in inner_x..inner_x + filled {
        for y in inner_y..inner_y + inner_h {
            let paint = match confidence {
                // Solid: observed OS truth.
                ValueConfidence::Confirmed => true,
                // Hollow: only the top and bottom rows, so a preview reads as an
                // outline rather than a filled, settled value.
                ValueConfidence::Preview => y == inner_y || y == inner_y + inner_h - 1,
                // Broken fill: dispatched but unobservable.
                ValueConfidence::Unverified => (x - inner_x) % DASH_PERIOD < DASH_ON,
            };
            if paint {
                band.set(x, y, fill);
            }
        }
    }
}
```

Add to `crates/kivori-renderer/src/lib.rs`:

```rust
pub mod overlay;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p kivori-golden-frames --test overlay_golden`
Expected: PASS, 6 tests.

- [ ] **Step 5: Record the committed hashes**

Add an `[overlay]` section to `tests/golden-frames/manifest.toml` following the existing table style, with one entry per `(percent, confidence, at_boundary)` sampled above. Run the test once, copy the printed hashes in, and re-run to confirm they match.

Run: `just golden`
Expected: PASS — existing scene/asset goldens unchanged, new overlay goldens recorded.

- [ ] **Step 6: Verify no_std and boundaries**

Run: `just fw-check && just check-boundaries`
Expected: both succeed.

- [ ] **Step 7: Commit**

```bash
git add crates/kivori-renderer/src/overlay.rs crates/kivori-renderer/src/lib.rs \
        tests/golden-frames/tests/overlay_golden.rs tests/golden-frames/manifest.toml
git commit -m "feat(renderer): add the canonical confidence-aware volume overlay"
```

---

### Task 11: Presentation resolver, desktop egress, firmware rendering

**Files:**
- Create: `apps/desktop/src-tauri/src/presentation/mod.rs`
- Modify: `apps/desktop/src-tauri/src/device/session.rs`
- Modify: `apps/desktop/src-tauri/src/runtime/device_task.rs`
- Modify: `firmware/esp32-c3/src/proto.rs`
- Modify: `firmware/esp32-c3/src/runtime.rs`
- Test: `apps/desktop/src-tauri/tests/rotary_loop.rs` (append), `firmware/esp32-c3/tests/rotary_input.rs` (append)

**Interfaces:**
- Consumes: `ValueUpdate`, `kivori_model::presentation::*`, `Capabilities::PRESENTATION_V1`.
- Produces: `presentation::{PresentationResolver, ProductSnapshot}`; `PresentationResolver::new(session: u32)`, `fn resolve(&mut self, snapshot: &ProductSnapshot) -> Presentation`; firmware `Runtime::apply_presentation`.

- [ ] **Step 1: Write the failing desktop test**

Append to `apps/desktop/src-tauri/tests/rotary_loop.rs`:

```rust
use kivori_desktop::presentation::{PresentationResolver, ProductSnapshot};
use kivori_model::presentation::{PrimaryState, ValueKind};

#[test]
fn revision_increases_strictly_within_a_session() {
    let mut r = PresentationResolver::new(0xAAAA);
    let a = r.resolve(&ProductSnapshot::idle());
    let b = r.resolve(&ProductSnapshot::idle());
    assert!(b.revision > a.revision);
    assert_eq!(a.session, 0xAAAA);
}

#[test]
fn revision_resets_when_a_new_session_begins() {
    let mut r = PresentationResolver::new(0xAAAA);
    for _ in 0..743 {
        r.resolve(&ProductSnapshot::idle());
    }
    let high = r.resolve(&ProductSnapshot::idle()).revision;
    assert!(high >= 743);

    r.begin_session(0xBBBB);
    let fresh = r.resolve(&ProductSnapshot::idle());
    assert_eq!(fresh.session, 0xBBBB);
    assert_eq!(fresh.revision, 1, "a restarted desktop starts at revision 1");
}

#[test]
fn a_value_update_becomes_a_transient_overlay_over_the_underlying_state() {
    let mut r = PresentationResolver::new(1);
    let p = r.resolve(&ProductSnapshot::with_value(ValueUpdate {
        percent: 60,
        confidence: ValueConfidence::Preview,
        at_boundary: false,
    }));

    let value = p.value.expect("overlay present");
    assert_eq!(value.kind, ValueKind::Volume);
    assert_eq!(value.current_percent, 60);
    assert_eq!(value.confidence, ValueConfidence::Preview);
    assert!(p.transient_ms > 0, "the overlay must expire");
    assert_eq!(
        p.primary,
        PrimaryState::Idle,
        "the underlying truth the device restores to"
    );
}

#[test]
fn a_failed_outcome_resolves_to_error_without_a_value_overlay() {
    let mut r = PresentationResolver::new(1);
    let p = r.resolve(&ProductSnapshot::failed());
    assert_eq!(p.primary, PrimaryState::Error);
    assert_eq!(p.value, None);
}
```

- [ ] **Step 2: Write the failing firmware test**

Append to `firmware/esp32-c3/tests/rotary_input.rs`:

```rust
use kivori_model::presentation::{PrimaryState, ValueConfidence, ValueDisplay, ValueKind};
use kivori_protocol::message::Presentation;

fn pres(session: u32, revision: u32, percent: u8, transient_ms: u16) -> Presentation {
    Presentation {
        session,
        revision,
        primary: PrimaryState::Idle,
        value: Some(ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: percent,
            confidence: ValueConfidence::Confirmed,
            at_boundary: false,
        }),
        transient_ms,
    }
}

#[test]
fn a_newer_revision_is_accepted_and_an_older_one_is_dropped() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);

    assert!(state.apply(&pres(0xAAAA, 5, 50, 800), 1_000));
    assert!(!state.apply(&pres(0xAAAA, 4, 10, 800), 1_010), "older revision");
    assert!(!state.apply(&pres(0xAAAA, 5, 10, 800), 1_020), "same revision");
    assert!(state.apply(&pres(0xAAAA, 6, 60, 800), 1_030));
}

#[test]
fn a_presentation_from_another_session_is_dropped() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    assert!(state.apply(&pres(0xAAAA, 5, 50, 800), 1_000));
    assert!(
        !state.apply(&pres(0xBBBB, 900, 10, 800), 1_010),
        "a stale high-revision presentation from a previous session must not render"
    );
}

#[test]
fn a_new_session_accepts_revision_one_again() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    assert!(state.apply(&pres(0xAAAA, 743, 50, 800), 1_000));

    state.begin_session(0xBBBB);
    assert!(
        state.apply(&pres(0xBBBB, 1, 20, 800), 2_000),
        "a restarted desktop must not be rejected as stale"
    );
}

#[test]
fn the_transient_overlay_expires_locally_back_to_primary() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    state.apply(&pres(0xAAAA, 1, 50, 800), 1_000);

    assert!(state.value_at(1_799).is_some(), "still inside the window");
    assert!(
        state.value_at(1_800).is_none(),
        "expired overlays restore the underlying primary state"
    );
    assert_eq!(state.primary(), PrimaryState::Idle);
}

#[test]
fn a_persistent_presentation_never_expires() {
    let mut state = kivori_firmware::runtime::PresentationState::new();
    state.begin_session(0xAAAA);
    state.apply(&pres(0xAAAA, 1, 50, 0), 1_000);
    assert!(state.value_at(600_000).is_some());
}
```

- [ ] **Step 3: Run both tests to verify they fail**

Run: `cargo test -p kivori-desktop --test rotary_loop` and `just fw-test`
Expected: FAIL — `could not find presentation in kivori_desktop`, `no PresentationState in runtime`.

- [ ] **Step 4: Write the desktop resolver**

Create `apps/desktop/src-tauri/src/presentation/mod.rs`:

```rust
//! Pure presentation resolution: product snapshot -> wire `Presentation`.
//!
//! `revision` is strictly increasing WITHIN a session and resets with it, so a
//! restarted desktop at revision 1 is never mistaken for stale traffic
//! (design spec section 4.1).

use crate::action::gesture_value::ValueUpdate;
use kivori_model::presentation::{PrimaryState, ValueDisplay, ValueKind};
use kivori_protocol::message::Presentation;

/// How long a volume overlay stays up. Contract section 5 success-transient target.
const VALUE_TRANSIENT_MS: u16 = 800;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProductSnapshot {
    pub value: Option<ValueUpdate>,
    pub failed: bool,
}

impl ProductSnapshot {
    pub const fn idle() -> Self {
        Self { value: None, failed: false }
    }
    pub const fn with_value(value: ValueUpdate) -> Self {
        Self { value: Some(value), failed: false }
    }
    pub const fn failed() -> Self {
        Self { value: None, failed: true }
    }
}

#[derive(Debug)]
pub struct PresentationResolver {
    session: u32,
    revision: u32,
}

impl PresentationResolver {
    pub const fn new(session: u32) -> Self {
        Self { session, revision: 0 }
    }

    /// Begin a new host session: rebind identity and restart revisions.
    pub fn begin_session(&mut self, session: u32) {
        self.session = session;
        self.revision = 0;
    }

    pub fn resolve(&mut self, snapshot: &ProductSnapshot) -> Presentation {
        self.revision = self.revision.saturating_add(1);

        let primary = if snapshot.failed {
            PrimaryState::Error
        } else {
            PrimaryState::Idle
        };

        let value = snapshot.value.map(|u| ValueDisplay {
            kind: ValueKind::Volume,
            current_percent: u.percent,
            confidence: u.confidence,
            at_boundary: u.at_boundary,
        });

        Presentation {
            session: self.session,
            revision: self.revision,
            primary,
            value,
            transient_ms: if value.is_some() { VALUE_TRANSIENT_MS } else { 0 },
        }
    }
}
```

Add `pub mod presentation;` to `apps/desktop/src-tauri/src/lib.rs`.

- [ ] **Step 5: Write the firmware presentation state**

Add to `firmware/esp32-c3/src/runtime.rs`:

```rust
/// Device-side presentation acceptance and local transient expiry.
///
/// Firmware expires the overlay itself so it cannot stick if the host disappears
/// mid-transient, restoring the underlying `primary` (contract invariant 50).
#[derive(Debug)]
pub struct PresentationState {
    session: Option<Nonce>,
    last_revision: u32,
    primary: PrimaryState,
    value: Option<ValueDisplay>,
    /// `None` = persistent; `Some(t)` = expire at this absolute ms.
    expires_at_ms: Option<u32>,
}

impl PresentationState {
    pub const fn new() -> Self {
        Self {
            session: None,
            last_revision: 0,
            primary: PrimaryState::Idle,
            value: None,
            expires_at_ms: None,
        }
    }

    /// A new compatible host session: rebind identity and reset revision scoping.
    pub fn begin_session(&mut self, session: Nonce) {
        self.session = Some(session);
        self.last_revision = 0;
        self.value = None;
        self.expires_at_ms = None;
    }

    pub fn end_session(&mut self) {
        self.session = None;
        self.last_revision = 0;
        self.value = None;
        self.expires_at_ms = None;
    }

    /// Returns true when the presentation was accepted and applied.
    pub fn apply(&mut self, p: &Presentation, now_ms: u32) -> bool {
        if self.session != Some(p.session) {
            return false;
        }
        if p.revision <= self.last_revision {
            return false;
        }
        self.last_revision = p.revision;
        self.primary = p.primary;
        self.value = p.value;
        self.expires_at_ms = if p.value.is_some() && p.transient_ms > 0 {
            Some(now_ms.wrapping_add(u32::from(p.transient_ms)))
        } else {
            None
        };
        true
    }

    /// The overlay still in force at `now_ms`, if any.
    pub fn value_at(&self, now_ms: u32) -> Option<ValueDisplay> {
        match self.expires_at_ms {
            Some(deadline) if now_ms >= deadline => None,
            _ => self.value,
        }
    }

    pub const fn primary(&self) -> PrimaryState {
        self.primary
    }
}
```

In `firmware/esp32-c3/src/proto.rs`, handle `Message::Presentation` by returning it to the runtime **only** when `PRESENTATION_V1` is negotiated; otherwise drop it with the existing safe-diagnostic path. In `Runtime::step`, after protocol dispatch, call `self.presentation.apply(&p, now_ms)` for an accepted presentation, call `begin_session`/`end_session` alongside the existing session transitions, and in the render gate pass `self.presentation.value_at(now_ms)` to `render_volume_overlay` after `render_tile`.

- [ ] **Step 6: Wire desktop egress**

In `apps/desktop/src-tauri/src/runtime/device_task.rs`, inside the existing tick:

1. On entering `Connected`, call `ingress.begin_session(nonce)` and `resolver.begin_session(nonce)` with `session.current_session()`.
2. On any exit from `Connected`, call `ingress.end_session()`.
3. For each decoded `Message::InputEvent`, run `ingress.accept(...)`; on `Ok(Some(input))` feed `gesture_value.on_input(input, backend)`; on `Err(reason)` record a `SafeDiagnostic` (category `bad_payload`) and **do not** execute.
4. When `gesture_value` yields a `ValueUpdate`, build `ProductSnapshot::with_value(update)`, resolve, and send `Message::Presentation` — **only** when `PRESENTATION_V1` is negotiated.

Select the backend at construction:

```rust
#[cfg(windows)]
let backend: Box<dyn VolumeBackend> = Box::new(platform::windows::WindowsVolumeBackend::new());
#[cfg(not(windows))]
let backend: Box<dyn VolumeBackend> =
    Box::new(platform::unimplemented::UnimplementedVolumeBackend::new(
        std::env::consts::OS,
    ));
```

Task 12 supplies `platform::windows`; until then, gate that arm behind `#[cfg(all(windows, feature = "windows-audio"))]` and fall back to `UnimplementedVolumeBackend` so this task compiles on every platform.

- [ ] **Step 7: Run tests to verify they pass**

Run: `just test && just fw-test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src-tauri/src/presentation/ apps/desktop/src-tauri/src/lib.rs \
        apps/desktop/src-tauri/src/runtime/device_task.rs apps/desktop/src-tauri/src/device/session.rs \
        firmware/esp32-c3/src/runtime.rs firmware/esp32-c3/src/proto.rs \
        apps/desktop/src-tauri/tests/rotary_loop.rs firmware/esp32-c3/tests/rotary_input.rs
git commit -m "feat: resolve session-scoped presentation and render it on the device"
```

---

### Task 12: Windows Core Audio backend

**Files:**
- Create: `apps/desktop/src-tauri/src/platform/windows/mod.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/platform/mod.rs`
- Modify: `.github/workflows/host.yml`
- Test: `apps/desktop/src-tauri/tests/volume_backend_contract.rs` (append)

**Interfaces:**
- Produces: `platform::windows::WindowsVolumeBackend` implementing `VolumeBackend`; `WindowsVolumeBackend::new()`, `fn try_recv_change(&self) -> Option<VolumeChange>`; `VolumeChange { percent: u8, origin: ChangeOrigin }`, `ChangeOrigin { Kivori, External, EndpointRebind }`.

- [ ] **Step 1: Write the failing test**

Append to `apps/desktop/src-tauri/tests/volume_backend_contract.rs`:

```rust
#[cfg(windows)]
mod windows_backend {
    use super::assert_backend_contract;
    use kivori_desktop::platform::windows::WindowsVolumeBackend;
    use kivori_desktop::platform::{ActionAvailability, VolumeBackend};

    /// GitHub-hosted Windows runners generally expose NO audio endpoint. This test
    /// therefore SKIPS loudly rather than passing silently — a skip is evidence of
    /// absence, not evidence of correctness.
    ///
    /// Runs ONLY the non-destructive contract: it captures the user's current volume,
    /// nudges it by 5 points, verifies read-back, and restores via `VolumeGuard`,
    /// including on unwind. `assert_boundary_contract` is NEVER called here — a test
    /// must not slam a real person's output to silent or full.
    #[test]
    fn real_backend_satisfies_the_non_destructive_contract_when_an_endpoint_exists() {
        let backend = WindowsVolumeBackend::new();
        match backend.availability() {
            ActionAvailability::Available { .. } => assert_backend_contract(&backend),
            other => {
                eprintln!(
                    "SKIPPED: no default render endpoint on this machine ({other:?}). \
                     Real Core Audio behaviour is PHYSICAL WINDOWS EVIDENCE, recorded in \
                     docs/features/002-rotary-volume-control/validation-checklist.md"
                );
            }
        }
    }

    /// Belt and braces: prove the endpoint is back where the user left it.
    #[test]
    fn the_real_endpoint_volume_is_unchanged_after_the_contract_run() {
        let backend = WindowsVolumeBackend::new();
        let ActionAvailability::Available { .. } = backend.availability() else {
            eprintln!("SKIPPED: no default render endpoint on this machine.");
            return;
        };
        let before = backend.read().expect("read before");
        assert_backend_contract(&backend);
        assert_eq!(
            backend.read().expect("read after"),
            before,
            "the contract run must leave the user's volume exactly as it found it"
        );
    }

    #[test]
    fn construction_without_an_endpoint_reports_runtime_unavailable_not_a_panic() {
        let backend = WindowsVolumeBackend::new();
        assert!(
            !matches!(backend.availability(), ActionAvailability::NotImplementedYet { .. }),
            "Windows IS implemented; absence of an endpoint is RuntimeUnavailable"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run (on Windows, or via CI): `cargo test -p kivori-desktop --test volume_backend_contract`
Expected: FAIL — `could not find windows in platform`.

- [ ] **Step 3: Add the dependency**

In `apps/desktop/src-tauri/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Media_Audio",
  "Win32_Media_Audio_Endpoints",
  "Win32_System_Com",
  "Win32_System_Com_StructuredStorage",
  "Win32_UI_Shell_PropertiesSystem",
] }
```

- [ ] **Step 4: Write the implementation**

Create `apps/desktop/src-tauri/src/platform/windows/mod.rs`. Structure, with the rules it must obey:

```rust
//! Windows Core Audio master volume.
//!
//! THREADING (research R-50, R-76): there is no tokio in this crate. A dedicated
//! `kivori-audio` OS thread performs CoInitializeEx, owns the endpoint, and holds
//! the callbacks. Callbacks are INGRESS ONLY: they push onto a channel and never
//! perform COM work inline. All rebinding happens on the owning thread.
//!
//! EVENT CONTEXT: every write passes KIVORI_EVENT_CONTEXT so the volume callback
//! can tell Kivori's own echo from a genuinely external change.
//!
//! ENDPOINT: the default render endpoint is a MOVING TARGET. An IMMNotificationClient
//! rebinds on OnDefaultDeviceChanged(eRender, eConsole) and publishes the NEW
//! endpoint's confirmed value.

use crate::platform::{ActionAvailability, BackendError, ConfirmationClass, VolumeBackend};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use windows::core::GUID;

/// Identifies volume changes Kivori itself originated.
const KIVORI_EVENT_CONTEXT: GUID = GUID::from_u128(0x4b49_564f_5249_0002_0000_0000_0000_0001);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOrigin {
    Kivori,
    External,
    EndpointRebind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeChange {
    pub percent: u8,
    pub origin: ChangeOrigin,
}

pub struct WindowsVolumeBackend {
    commands: Sender<Command>,
    changes: Mutex<Receiver<VolumeChange>>,
    // ...
}
```

Implementation requirements, each of which the contract suite or physical validation checks:

1. `new()` spawns the `kivori-audio` thread, which calls `CoInitializeEx(None, COINIT_MULTITHREADED)`, creates an `IMMDeviceEnumerator`, and binds the default `eRender`/`eConsole` endpoint's `IAudioEndpointVolume`.
2. `read()` → `GetMasterVolumeLevelScalar`, scaled `f32 0.0..=1.0` → `u8 0..=100` with rounding; **never** floating point beyond this boundary.
3. `set(percent)` → `SetMasterVolumeLevelScalar(percent as f32 / 100.0, &KIVORI_EVENT_CONTEXT)`, then re-`read()` and return the observed value. If the write succeeds and the read fails, return `Err(BackendError::ReadBackUnavailable)`.
4. `IAudioEndpointVolumeCallback::OnNotify` compares `guidEventContext` to `KIVORI_EVENT_CONTEXT` and pushes `VolumeChange { origin: Kivori | External }`. It performs no COM calls.
5. `IMMNotificationClient::OnDefaultDeviceChanged` for `eRender` pushes a rebind command to the owning thread. The owning thread unregisters the old volume callback, releases the old endpoint, binds the new one, registers the callback, reads the value, and pushes `VolumeChange { origin: EndpointRebind }`.
6. With no default endpoint, `availability()` returns `RuntimeUnavailable { reason: "no default render endpoint" }` and `read`/`set` return `Err(BackendError::NoEndpoint)`. Never `NotImplementedYet` — Windows *is* implemented.
7. With an endpoint, `availability()` returns `Available { confirmation: ConfirmationClass::StateConfirmed }`.
8. `Drop` unregisters both callbacks and calls `CoUninitialize` on the owning thread.

In `device_task.rs`, drain `try_recv_change()` each tick and route: `ChangeOrigin::External` → `gesture_value.on_external_change(percent)`; `ChangeOrigin::EndpointRebind` → `gesture_value.on_endpoint_rebind(percent)`; `ChangeOrigin::Kivori` → ignore, it is our own echo.

- [ ] **Step 5: Update CI**

In `.github/workflows/host.yml`, add to the existing Windows Rust job:

```yaml
      - name: Build the Windows Core Audio backend
        run: cargo build -p kivori-desktop
      - name: Volume backend contract suite (fake everywhere; real backend skips without an endpoint)
        run: cargo test -p kivori-desktop --test volume_backend_contract -- --nocapture
```

`--nocapture` is required so the `SKIPPED:` line is visible. **Do not** add an assertion that the real endpoint exists; hosted runners generally have none, and a silent pass would be false evidence.

- [ ] **Step 6: Run tests to verify they pass**

Run on CI (push the branch) and on the maintainer's Windows machine: `cargo test -p kivori-desktop`
Expected: PASS on both; the real-backend test runs for real on the physical machine and SKIPS loudly on the runner.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src/platform/windows/ apps/desktop/src-tauri/src/platform/mod.rs \
        apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/runtime/device_task.rs \
        .github/workflows/host.yml apps/desktop/src-tauri/tests/volume_backend_contract.rs Cargo.lock
git commit -m "feat(desktop): add Windows Core Audio volume backend with endpoint rebinding"
```

---

### Task 13: Physical HW-040 adapter

**Files:**
- Modify: `firmware/esp32-c3/src/profile.rs`
- Create: `firmware/esp32-c3/src/physical_rotary.rs`
- Modify: `firmware/esp32-c3/src/physical_st7789.rs`
- Modify: `firmware/esp32-c3/src/lib.rs`

**Interfaces:**
- Produces: `profile::physical_st7789::ROTARY: RotaryProfile`; `physical_rotary::PhysicalRotary` implementing `InputSource`.

- [ ] **Step 1: ASK THE MAINTAINER FOR THE PIN MAP**

The HW-040 CLK/DT/SW mapping is recorded **nowhere** in this repository, and the design spec forbids guessing it. Stop and ask:

> "Task 13 needs the HW-040 wiring. Which GPIOs are CLK, DT, and SW on the assembled unit? The verified display profile already claims GPIO2 (D/C), GPIO3 (RST), GPIO6 (SCK), GPIO7 (MOSI), GPIO8 (backlight), plus native USB on GPIO18/19, so those are unavailable."

Do not proceed to Step 2 without an answer. Do not substitute a plausible default.

- [ ] **Step 2: Write the failing test**

Add to `firmware/esp32-c3/tests/rotary_input.rs`:

```rust
#[test]
fn the_rotary_profile_does_not_collide_with_the_display_profile() {
    use kivori_firmware::profile::physical_st7789::{DISPLAY, ROTARY};

    let display_pins = [DISPLAY.sck, DISPLAY.mosi, DISPLAY.dc, DISPLAY.reset, DISPLAY.backlight];
    for pin in [ROTARY.clk, ROTARY.dt, ROTARY.sw] {
        assert!(
            !display_pins.contains(&pin),
            "rotary pin {pin} collides with the verified display profile"
        );
        // GPIO18/19 are the native USB Serial/JTAG pair.
        assert!(pin != 18 && pin != 19, "rotary pin {pin} collides with native USB");
    }

    assert_ne!(ROTARY.clk, ROTARY.dt);
    assert_ne!(ROTARY.clk, ROTARY.sw);
    assert_ne!(ROTARY.dt, ROTARY.sw);
}
```

Adjust the `DISPLAY` field names to match whatever `profile.rs` already uses; do not rename existing constants.

- [ ] **Step 3: Run test to verify it fails**

Run: `just fw-test`
Expected: FAIL — `no ROTARY in physical_st7789`.

- [ ] **Step 4: Add the profile**

In `firmware/esp32-c3/src/profile.rs`, inside the existing `physical_st7789` module:

```rust
/// HW-040 rotary encoder pin map.
///
/// ISOLATED HERE ON PURPOSE: this is the single place to correct if the wiring
/// changes. `sw` is wired and sampled but unused in Slice 002; the push-switch
/// gesture machine is a later slice.
#[derive(Debug, Clone, Copy)]
pub struct RotaryProfile {
    pub clk: u8,
    pub dt: u8,
    pub sw: u8,
}

/// Values supplied by the maintainer in Task 13 Step 1.
pub const ROTARY: RotaryProfile = RotaryProfile {
    clk: /* maintainer-supplied */,
    dt: /* maintainer-supplied */,
    sw: /* maintainer-supplied */,
};
```

- [ ] **Step 5: Write the adapter**

Create `firmware/esp32-c3/src/physical_rotary.rs`:

```rust
//! Physical HW-040 `InputSource`.
//!
//! This adapter does NOTHING but read pin levels. All conditioning and semantics
//! live above the port in `input::quadrature` and `input::gesture`, which is what
//! makes them provable without hardware.
//!
//! HW-040 modules are open-collector with the common pin to ground, so inputs use
//! internal pull-ups and read ACTIVE-LOW. `sample()` inverts to logical levels.

use crate::ports::InputSource;
use esp_hal::gpio::{Input, Pull};
use kivori_model::input::InputLevels;

pub struct PhysicalRotary<'d> {
    clk: Input<'d>,
    dt: Input<'d>,
    sw: Input<'d>,
}

impl<'d> PhysicalRotary<'d> {
    pub fn new(clk: Input<'d>, dt: Input<'d>, sw: Input<'d>) -> Self {
        Self { clk, dt, sw }
    }
}

impl InputSource for PhysicalRotary<'_> {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: self.clk.is_low(),
            b: self.dt.is_low(),
            sw: self.sw.is_low(),
        }
    }
}
```

Add `#[cfg(feature = "embedded")] pub mod physical_rotary;` to `firmware/esp32-c3/src/lib.rs`, matching the gating style of the neighbouring physical modules. In `physical_st7789.rs`, construct the three `Input` pins with `Pull::Up` from `ROTARY`, build a `PhysicalRotary`, and pass it to `runtime::run` in place of the constant-level stub from Task 4.

- [ ] **Step 6: Run tests and build the real firmware**

Run: `just fw-test && just fw-build && just fw-check`
Expected: all succeed; `fw-build` produces the flashable RISC-V binary.

- [ ] **Step 7: Commit**

```bash
git add firmware/esp32-c3/src/profile.rs firmware/esp32-c3/src/physical_rotary.rs \
        firmware/esp32-c3/src/physical_st7789.rs firmware/esp32-c3/src/lib.rs \
        firmware/esp32-c3/tests/rotary_input.rs
git commit -m "feat(firmware): add the physical HW-040 input adapter"
```

---

### Task 14: Physical validation, latency gate, and feature records

Slice 002 is **not complete** until this task's evidence is dated and recorded. CI cannot produce any of it.

**Files:**
- Create: `docs/features/002-rotary-volume-control/requirements.md`
- Create: `docs/features/002-rotary-volume-control/architecture.md`
- Create: `docs/features/002-rotary-volume-control/validation-checklist.md`
- Modify: `docs/features/001-device-connection-foundation/contracts/protocol.md`
- Modify: `docs/features/001-device-connection-foundation/data-model.md`

- [ ] **Step 1: Write the feature records**

`requirements.md` restates the Slice 002 scope and the contract rules it implements. `architecture.md` describes the built chain: `InputSource` port → quadrature → gesture → `InputEvent` → ingress → action → `GestureValue` → `PresentationResolver` → `Presentation` → overlay. Both link back to the design spec.

`architecture.md` must also carry the **input tuning parameter table**, recording each value's provisional setting and the hardware-validation item that must measure it — these are tuning parameters, not settled constants:

| Parameter | Where | Provisional | Measured by |
|---|---|---|---|
| A/B sample interval | `Runtime::step` cadence | every runtime tick | HW-validation #1, #5 |
| detent qualification depth | `QUARTER_STEPS_PER_DETENT` | 4 (full step) | HW-validation #2 |
| invalid-transition threshold | `QuadratureDecoder::invalid_transitions` | diagnostic only | HW-validation #6 |
| gesture-end inactivity | `RotaryGesture::new` | 250 ms | contract §5 |
| base step | `BASE_STEP_PERCENT` | 2 points/detent | UX tuning |
| value transient | `VALUE_TRANSIENT_MS` | 800 ms | contract §5 |

- [ ] **Step 2: Update the protocol contract**

In `docs/features/001-device-connection-foundation/contracts/protocol.md`:

- add tags 11 and 12 to the §3 message table;
- add the two capability bits to §5;
- add a subsection recording the **widened role of the handshake nonce**: it is now connection-scoped session identity stamped on `InputEvent` and `Presentation`, and MUST be unique across desktop process restarts (not the previous counter-from-1). State plainly that it is a freshness token, not a security credential.

In `data-model.md`, add the new `kivori-model` types to §10 and note that presentation revisions are session-scoped.

- [ ] **Step 3: Create the validation checklist with empty result rows**

Follow the Feature 001 checklist format exactly — a `Result` column filled with ✅/❌ plus a date, and `Notes` for measurements. Rows:

| # | Check | Target |
|---|---|---|
| 1 | One physical detent produces exactly one volume step, both directions | fixed 1× step |
| 2 | Slow rotation loses no detents | US1 observed detents |
| 3 | Fast rotation produces no false reversals | R-82 |
| 4 | Bounce/noise produces no phantom detents; invalid-transition counter stays low | HW-validation #2 |
| 5 | Volume changes in the Windows flyout match the device | State Confirmed |
| 6 | External change via the flyout reconciles on the device with no Kivori input | US4 |
| 7 | External change **during** a gesture does not fight the preview, and applies at gesture end | US3 |
| 8 | Preview and Confirmed are visually distinguishable on the physical panel | invariant 3 |
| 9 | Switching the default output device mid-session rebinds and shows the new endpoint's volume | §7.1 |
| 10 | Switching the default output device **mid-gesture** discards the gesture rather than retargeting | §7.1 |
| 11 | Volume clamps at 0 and 100; the first reverse detent leaves the boundary immediately | US1 bounded values |
| 12 | Unplug mid-gesture, replug: the pre-disconnect gesture executes nothing | invariant 5 |
| 13 | Restart Kivori Desktop while the device stays powered: presentation resumes, no stale overlay | §4.1 |
| 14 | **Measured detent → on-panel feedback latency** | **< 50 ms (contract §5)** |

- [ ] **Step 4: Run the physical validation**

Execute every row on the physical ESP32-C3 + HW-040 with Windows. Record dates and measurements. Simulation evidence is never promoted to physical evidence; leave unexecuted rows blank rather than inferring them.

- [ ] **Step 5: THE LATENCY GATE**

Row 14 is a gate, not a note. If measured detent→feedback latency **misses** the < 50 ms target:

1. Replace the fixed `TICK = 50ms` sleep in `apps/desktop/src-tauri/src/runtime/device_task.rs:31,129` with a short-timeout blocking serial read, so the loop wakes on data rather than on a timer.
2. Re-measure and record both numbers.
3. Commit separately: `perf(desktop): wake the device loop on serial data instead of a fixed tick`.

**Slice 002 is not complete while row 14 is failing.** Do not close it with an unmet target.

- [ ] **Step 6: Final verification**

Run: `just lint && just test && just fw-test && just fw-check && just golden && just check-boundaries && bash scripts/check-offline-deps.sh`
Expected: all pass. Then confirm CI is green on the PR head.

- [ ] **Step 7: Evaluate the three candidate ADRs**

The design spec listed three **candidates**, none pre-authorized. Now that the design is built, decide each one and record the decision either way:

1. **Capability / availability / confirmation as three orthogonal concepts.** Write an ADR only if the split survived implementation and a flat enum is a genuinely worse alternative.
2. **Input-semantics ownership split** (firmware forms validated detents, desktop owns meaning). May already be settled by contract invariant 24 — if so, no ADR.
3. **The handshake nonce doubles as connection-scoped session identity.** This widened an existing protocol element and imposed a nonce-generation requirement both peers depend on, so it is the most likely of the three.

Do not write an ADR for a temporary implementation choice. If none is warranted, say so in `docs/features/002-rotary-volume-control/architecture.md` and move on.

- [ ] **Step 8: Commit**

```bash
git add docs/features/002-rotary-volume-control/ docs/adr/ \
        docs/features/001-device-connection-foundation/contracts/protocol.md \
        docs/features/001-device-connection-foundation/data-model.md
git commit -m "docs(002): record Slice 002 contracts and physical validation evidence"
```

---

## Spec deviations

Recorded so review can reject them rather than discover them:

1. **`InputTuning` struct not created.** The spec gathered the tuning parameters into one struct. With only a handful of values spread across two crates and a firmware module, a struct would add indirection without changing behaviour. The parameters are instead named constants at their point of use plus a single documented table in the feature record (Task 14, Step 1), which preserves the intent — these are measured parameters, not settled constants — at lower cost. Revisit when a third consumer needs them together.
2. **`ValueConfidence::Preview` is reported even for a write the backend confirmed** (Task 9). During an open gesture the value is still a gesture-local target that the final read-back may revise, so promoting it to `Confirmed` mid-gesture would be the exact over-claim the field exists to prevent. Promotion happens once, at gesture end.

## Completion criteria

Slice 002 is complete when **all** of these hold:

1. Tasks 1–14 are committed on `docs/product-prd-us-contract`.
2. `just lint`, `just test`, `just fw-test`, `just fw-check`, `just golden`, `just check-boundaries`, and the offline guard all pass, and CI is green on the PR head.
3. Turning the physical knob changes Windows master volume, and the device displays the volume Windows reports.
4. Every validation-checklist row has a dated result, **including row 14's measured latency within target** — with the device-thread polling fix applied and re-measured if the first measurement missed.
5. No existing assertion was weakened or deleted.
6. PR #2 is **not** merged.
