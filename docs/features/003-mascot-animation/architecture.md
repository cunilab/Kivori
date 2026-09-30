# Feature 003 — Mascot animation, social reactions, and activity log

Implementation record for [PR #3](https://github.com/Vellixia/Kivori/pull/3) (merged 2026-09-25), layered
on the [Feature 001 foundation](../001-device-connection-foundation/architecture.md). Design plan:
[`2026-09-07-mascot-animation.md`](../../superpowers/plans/2026-09-07-mascot-animation.md). Evidence:
[`evidence/mascot-animation.md`](./evidence/mascot-animation.md).

## Protocol

- `PlayMascotAction` (desktop → device, wire tag 11) and `MascotActionApplied` (device → desktop, wire
  tag 12), gated by capability `MASCOT_INTERACTION` (bit 0). Slice 002 appends after these (tags 13/14,
  bits 1/2) — see the [Feature 002 architecture](../002-rotary-volume-control/architecture.md).
- Frame version 2; desktop and firmware must be updated together.

## Mascot rendering


The mascot is a living keycap drawn in `assets/mascot.svg`. Format v2 stores cropped RGB565 sprites
with packed alpha4, drawn in order: the fixed **base** (`Body`: side walls and front lip), the
**cap** (`Cap`: top face, upper wall wedges, legend), then the two eyes and the mouth. Base and cap
are shared across all six states; each eye texture is reused for both eyes. Base, cap and mouth are
stored trimmed to their drawn pixels (lossless) so the full pack, limited to 128 KiB at build time,
is 116,113 bytes.

A pose's `press_q8` sinks the cap and the face printed on it while the base stays on the desk, so
the walls visibly shorten. Happy holds a 16 px press, eased in and out with the other pose fields.
`MascotAnimator` resolves blinking, state-specific motion, and 350 ms eased transitions (600 ms entering
sleep) in fixed-point arithmetic. Expression changes are hidden inside a blink so eye and mouth sprites
do not overlap. The firmware retains this controller between state changes and renders its pose through
the same compositor as native Device Studio.

Physical ST7789 mode stages a complete RGB565 pose in 112.5 KiB of static SRAM before sending changed
40x40 tiles through SPI DMA. Composition and hashing never interrupt the transfer phase. This is one
prepared frame, not panel double buffering; the unwired TE signal means refresh synchronization is
not available. Simulation can retain the small-buffer path through the same runtime and compositor.

Device Studio keeps up to 256 semantic selection events for replayable pause, step, and backward
seek. While playing, one externally-clocked stream coalesces timestamp updates and discards frames
superseded during rendering; two in-flight RGBA frames remain the maximum. The browser-only mock
is still a placeholder. Use the native Tauri app to review actual mascot/device pixels.


Slice 002's volume overlay composites on top of the mascot pose inside this same tile pass; it never
replaces the mascot or companion state. While it shows, the pose is drawn through
`MascotPose::with_overlay_room` (x0.72, lifted 34 px) so the keycap sits above the bar. The switch is
instant, not eased.

## Companion director, firmware update, activity log

- The companion director drives personality and self-play reactions; Device Studio preview reactions
  are forwarded to a connected device with `MASCOT_INTERACTION`.
- Bundled firmware flashing from Device Studio: [`firmware-update.md`](../../firmware-update.md).
- Typed, session-only activity log (the only runtime log): [`activity-log.md`](../../activity-log.md).
