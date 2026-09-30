# ADR-0006: The handshake nonce doubles as connection-scoped session identity

**Status**: Accepted · **Date**: 2026-09-17 · **Feature**: 002-rotary-volume-control

## Context

Slice 002 adds two capability-gated messages carrying continuous, executable meaning:
device→desktop `InputEvent` and desktop→device `Presentation` (see
[`contracts/protocol.md`](../features/001-device-connection-foundation/contracts/protocol.md) §3, §5,
and the widened-nonce subsection). Unlike Feature 001's `SetState`/`StateReport`, these messages can
cause a real side effect (a volume change) or drive a display overlay, so a stale copy of either one
surfacing after a reconnect is not merely cosmetic — it is exactly the failure contract invariant 5
("No stale replay") forbids: expired input must not execute after a connection or service recovers.

An earlier design revision tried to avoid adding any session concept at all, relying on "the desktop
only accepts events for a gesture whose `GestureStarted` it observed in this connection." That
reasoning has a hole: a *complete* stale pair — `GestureStarted` followed by `Detent`, both buffered
in the OS/USB layer from before a disconnect and delivered together after reconnect — satisfies that
rule exactly as a live pair would, so it would still execute. The argument that such buffered bytes
always surface during `Connecting` (before the rule would apply) is a timing/serial-buffer
assumption, not a guarantee, and cannot support a MUST-level invariant. A free-running `u16`
`gesture_id` also eventually wraps, which is a second reason it cannot carry the whole burden alone.

The protocol already carries a `Nonce = u32` in the handshake (`Hello`/`HelloAck`), but the desktop
generated it as a counter initialized to `1` and incremented per attempt, reinitialized on every
process start (`crates/kivori-protocol/src/message.rs`; desktop `device/session.rs`). That is
adequate for handshake liveness (proving the device answered *this* `Hello`) but not for session
identity: a restarted desktop process begins at `1` again, so a stale event stamped `1` from a
previous process would match a new session's nonce by coincidence.

## Decision

Reuse the handshake nonce as connection-scoped session identity, rather than introducing a dedicated
`SessionEpoch` message or field:

1. The desktop mints a session nonce and sends it in `Hello`, as it already did for liveness.
2. Firmware stores the nonce of the `Hello` it accepted and stamps **every** `InputEvent` with it.
3. The desktop rejects any `InputEvent` whose `session` is not the nonce it sent in the current
   connection's `Hello` (`InputIngress::accept`, `RejectReason::StaleSession`).
4. Symmetrically, the desktop stamps every `Presentation` with the same nonce, and firmware rejects
   any `Presentation` that does not match its currently accepted session.
5. Firmware clears input state and the accepted-revision high-water mark on accepting a new `Hello`,
   and on `Bye`/link loss. The desktop clears its open-gesture set on every exit from `Connected`.
6. `Presentation.revision` is strictly increasing **within a session** and resets when a new session
   is accepted — session-scoped, not device-lifetime-scoped — so a freshly restarted desktop at
   revision 1 is never mistaken for stale traffic against a previous process's high revision number.

This required one behavioral change with a cross-peer consequence: **the nonce must become unique
across desktop process restarts, not only within a process.** The implementation replaces the counter
entirely rather than combining anything with it: `OsNonceSource` draws a fresh `u32` directly from OS
randomness via `getrandom::getrandom()` on every handshake attempt, with no counter component
retained (`apps/desktop/src-tauri/src/device/nonce.rs`). This adds `getrandom` as a new direct
dependency of `kivori-desktop` (`apps/desktop/src-tauri/Cargo.toml`) — acceptable because it was
already present transitively in the lockfile and is not a network client. The nonce remains a
**freshness token, not a security credential** — it does not authenticate the desktop, it only
distinguishes "this connection" from "a previous one" well enough to reject stale traffic.

With session identity established this way, `gesture_id` only needs to be unique *within* a session,
so its `u16` wraparound stops being a correctness concern.

## Alternatives considered

- **A dedicated `SessionEpoch` message/field.** Would express the same guarantee with a new,
  purpose-named wire element instead of overloading the existing nonce. Rejected: it adds a message
  and a handshake field for a concept the existing nonce already structurally satisfies (a value
  minted per connection attempt and echoed by the device), at the cost of one more thing both peers
  must implement and keep in sync. The nonce already flows through exactly the right lifecycle
  points (minted in `Hello`, echoed in `HelloAck`, live for the connection's duration) for session
  identity to piggyback on with no new message.
- **Relying solely on the observed-`GestureStarted` rule with no session concept.** Rejected as
  described above — a complete stale pair defeats it, so it cannot support a MUST-level invariant.
  It is retained as defence-in-depth (`InputIngress`'s `UnknownGesture` rejection) but is no longer
  load-bearing on its own.
- **A free-running, never-reset `gesture_id` as the sole staleness guard.** Rejected: it eventually
  wraps, and wraparound is a real (if distant) correctness gap for a MUST-level guarantee. Session
  scoping removes the need for `gesture_id` to carry cross-session meaning at all.

## Consequences

- The nonce's contract is now wider than "handshake liveness check": it is connection-scoped session
  identity, and both `InputEvent` and `Presentation` depend on peers agreeing on its current value.
  This is documented explicitly in
  [`contracts/protocol.md`](../features/001-device-connection-foundation/contracts/protocol.md#nonce-as-connection-scoped-session-identity)
  so a future protocol change does not silently violate it.
- Nonce generation carries a new cross-restart uniqueness requirement that Feature 001's counter-based
  generator did not meet; this was corrected as part of Slice 002 (`device/session.rs`).
- `Presentation.revision` monotonicity is explicitly session-scoped rather than device-lifetime-scoped,
  which must be preserved by any future message that reuses the same revision mechanism.
- Two adversarial tests are load-bearing evidence for this decision and must not be weakened: a
  complete stale `GestureStarted` + `Detent` pair injected immediately after a reconnect handshake
  must produce no volume change
  (`apps/desktop/src-tauri/tests/rotary_loop.rs::a_complete_stale_gesture_pair_after_reconnect_executes_nothing`),
  and a stale high-revision `Presentation` injected after a new session must not be rendered
  (`firmware/esp32-c3/tests/rotary_input.rs::a_presentation_from_another_session_is_dropped`).
