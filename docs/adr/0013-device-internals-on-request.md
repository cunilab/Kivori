# ADR-0013: Device internals on request

- **Status:** Accepted
- **Date:** 2026-10-11
- **Milestone:** M2.6
- **Related:** [ADR-0005](./0005-typed-activity-log-with-allowlist.md)

## Context

Developer mode shows flash, RAM, CPU, frame timing, chip and link counters. `Health` is a fixed struct, and `Health` stays the always-on minimum.

## Decision

Desktop sends a stats request about once a second while the Device internals card is visible; the device answers with one fixed-size snapshot, behind a new capability. Request and reply are new messages, not fields on `Health`.

Sources: image and partition sizes from the app descriptor and partition table read at boot; static RAM from linker symbols; lowest free stack by painting the stack at boot and scanning for the high-water mark when asked; busy % from time spent outside the run loop's idle wait; compose and flush times from the same instants the `latency-probe` build uses; temperature from the C3 internal sensor where the HAL supports it, else unavailable; reset reason from the ROM.

Every field is optional on the wire, so a missing measurement is unavailable, never 0. Counters are `u32`, wrap, and reset at boot. No ids, MACs, payloads or port names (ADR-0005).

## Consequences

As built: image size is read by walking the app image's header and segments in flash (the app descriptor does not carry it) after finding the app partition in the partition table, both read at boot through the ROM flash reader; total flash is the size the bootloader image header declares; static RAM, total RAM and the stack bounds come from linker symbols. The run loop never sleeps, so busy % is the share of the last second spent in ticks that did work (rendered, or handled or sent a frame), not time outside an idle wait. Compose and flush times are read from the device clock around the same two steps the `latency-probe` build measures. The firmware has no heap, so the snapshot carries its own `ram_free_bytes` (stack headroom now) while `Health.free_bytes` stays 0, "not measured".

## Alternatives considered

- Streaming stats with every `Pong`: rejected, cost on every product session for a developer view.
- A debug-only build: rejected, the numbers that matter are the release build's.
