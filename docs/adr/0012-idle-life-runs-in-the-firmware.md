# ADR-0012: Idle life runs in the firmware

- **Status:** Accepted
- **Date:** 2026-10-11
- **Milestone:** M2.6
- **Related:** [ADR-0003](./0003-integer-millisecond-timebase.md)

## Context

Today the desktop companion director (`companion.rs`) schedules self-play and the device only animates what it is sent, so a busy or stalled host leaves a still buddy.

## Decision

Self-play scheduling moves into `kivori-model` (`no_std`, beside `MascotAnimator`), so the firmware runs it and Device Studio runs the same code for the same pixels. It is deterministic: the ADR-0003 timebase plus a seed (the device id hash on a unit, so units differ; fixed in Device Studio and golden frames), never a hardware RNG. Desktop keeps sending personality, reactions on/off, intensity and the local clock.

New idle moments reuse existing eye frames by moving the eye layer (glances, looking towards a control). The face-uniqueness test extends to every idle pose.

The brightness setting drives the backlight on GPIO8 with LEDC PWM, started after boot (GPIO8 is a strapping pin, sampled only at reset); there is no time-based dimming.

## Consequences

The asset blob is at ~126 KB of its 128 KiB cap; the mic-muted face is new art and needs trimming first.

## Alternatives considered

- Streaming idle frames from the host: rejected, stalls freeze the buddy.
- A random source: rejected, breaks golden frames.
