# Manual validation checklist — Slice 002 (rotary volume control)

Human-run checks that host CI **cannot** cover: real HW-040 detent fidelity, real Windows Core Audio
behavior, and end-to-end physical latency. These do not gate ordinary merge CI. Targets come from the
[requirements](./requirements.md), the [architecture record](./architecture.md#input-tuning-parameters),
the [design spec](../../superpowers/specs/2026-09-17-rotary-volume-control-loop-design.md), and the
[user-story contract](../../product/user-story-contract.md).

Follows the [Feature 001 checklist format](../001-device-connection-foundation/validation-checklist.md)
exactly: fill `Result` with ✅ / ❌ and a date; put measurements and observations in `Notes`.

**No row in this table has been executed.** Simulation and host-sim evidence — however strong — is
never promoted to physical evidence; see
[Feature 001's closure status](../001-device-connection-foundation/closure-status.md) for how that
same line was drawn there. Every `Result` cell below is intentionally blank until a human runs the
check on the physical ESP32-C3 + HW-040 with a real Windows host.

## Hardware — rotary input fidelity

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 1 | One physical detent produces exactly one volume step, both directions | fixed 1× step | | |
| 2 | Slow rotation loses no detents | US1 observed detents | | |
| 3 | Fast rotation produces no false reversals | R-82 | | |
| 4 | Bounce/noise produces no phantom detents; invalid-transition counter stays low | `QUARTER_STEPS_PER_DETENT` full-step qualification | | |

## Hardware — Windows Core Audio integration

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 5 | Volume changes in the Windows flyout match the device | State Confirmed | | |
| 6 | External change via the flyout reconciles on the device with no Kivori input | US4 | | |
| 7 | External change **during** a gesture does not fight the preview, and applies at gesture end | US3 | | |
| 8 | Preview and Confirmed are visually distinguishable on the physical panel | invariant 3 | | |
| 9 | Switching the default output device mid-session rebinds and shows the new endpoint's volume | §7.1 | | |
| 10 | Switching the default output device **mid-gesture** discards the gesture rather than retargeting | §7.1 | | |

## Hardware — bounds and recovery

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 11 | Volume clamps at 0 and 100; the first reverse detent leaves the boundary immediately | US1 bounded values | | |
| 12 | Unplug mid-gesture, replug: the pre-disconnect gesture executes nothing | invariant 5 | | |
| 13 | Restart Kivori Desktop while the device stays powered: presentation resumes, no stale overlay | §4.1 | | |

## Hardware — latency

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 14 | **Measured detent → on-panel feedback latency** | **< 50 ms (contract §5)** | | This is a gate, not a note (design spec §10). Slice 002 is not complete while this row is failing or unmeasured. **Two quantisation steps contribute, not one, and every pixel round-trips the desktop — there is no device-local acknowledgement at all.** (a) `apps/desktop/src-tauri/src/runtime/device_task.rs`'s fixed `TICK = 50ms` sleep, and (b) the firmware's `RuntimeConfig::frame_interval_ms = 33` render cadence. Worst case is roughly 50 + serial + 33 ≈ 85 ms against a < 50 ms target, so the measurement is expected to be informative. Neither constant is being pre-emptively changed: the decision is deliberately measure-first. If the measurement misses target, the remedies are, in order, replacing the desktop `TICK` with a short-timeout blocking serial read, then tightening `frame_interval_ms`, then considering device-local feedback; the number must be re-measured before this row can close. |

**Measuring row 14.** Flash the development-only probe build with `just fw-flash-latency`
(`--features physical-st7789,latency-probe`; never a product build), connect Kivori Desktop, and
turn the encoder one detent at a time. The top-left corner shows `L` (last reading) over `H` (running
max) in milliseconds, clamped to 999; nothing is drawn until the first reading completes. Both ends use the
device's own ms clock: the start is when the `Detent` `InputEvent` is transmitted, the end is when
the first frame composed after the desktop's answering `Presentation` (new revision only) finishes its
blocking SPI/DMA tile writes. That is flush completion, not photons — panel scan-out adds up to one
refresh period on top. A detent with no answer within 1000 ms is dropped; in a fast burst only the
first detent is timed. Record several single-detent readings plus the `H` value in `Notes`, then
reflash with `just fw-flash`.

## Hardware — HW-040 wiring itself

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 15 | HW-040 wiring continuity and correct behavior on CLK/DT/SW against the pin map in [`architecture.md`](./architecture.md#the-hw-040-pin-map--specification-not-measured-evidence) (CLK=GPIO4, DT=GPIO5, SW=GPIO10; COM to GND, VCC to 3V3) | specification pending physical verification | | |

Row 15 exists because the CLK/DT/SW pin map recorded in `firmware/esp32-c3/src/profile.rs` is a
**specification the maintainer is wiring to**, not an observation of an existing board — unlike the
SPI/display pin facts in
[Feature 001's validation checklist](../001-device-connection-foundation/validation-checklist.md#verified-physical-kivori-profile),
which are dated physical evidence from 2026-08-11. Rows 1–4 depend on this wiring being correct, so
row 15 is a prerequisite fact-check, not a duplicate of rows 1–4.
