# Hardware validation session: Phases 1–3

One ordered walk through every open physical row in [roadmap](./roadmap.md) Phases 1–3. The steps are
grouped so each setup is done once. Record every result in the checklist the row belongs to (date,
measurement, OS), not here.

- **F1** = [Feature 001 checklist](./features/001-device-connection-foundation/validation-checklist.md)
- **S2** = [Slice 002 checklist](./features/002-rotary-volume-control/validation-checklist.md)
- **P2** = Phase 2 mascot (record in [personality review](./features/003-mascot-animation/personality-review.md) → Still to do)

**You need:** the ESP32-C3 + ST7789 + HW-040 board, a Windows PC, this Mac, one unrelated USB-serial
device (any Arduino or USB-UART adapter), and a phone that records slow motion (backup for latency only).

## 0. Wiring and flash (~15 min)

1. **S2-15**: check CLK=GPIO4, DT=GPIO5, SW=GPIO10, COM→GND and VCC→**3V3** (not 5 V) against the
   README pin map. Check each line for continuity.
2. Flash the product firmware: `just fw-flash`. Confirm the desktop app shows **Connected**.

## 1. Connection and lifecycle (Windows first, then macOS) (~30 min)

| Step | Rows |
|---|---|
| Plug in from cold and time plug-in → Connected in the Log view timestamps | F1-6, F1-20 |
| Plug in the unrelated USB-serial device; it must **not** show as connected | F1-7 |
| Close the window, wait 2 min, and watch the heartbeat continue in the Log view | F1-1, F1-2 |
| Reopen (taskbar/Dock and second launch), then Quit from the tray | F1-3, F1-4 (Windows; macOS done) |
| Set `busy` → unplug → **Disconnected** → replug → back to `busy` with no action; time it | F1-15, F1-16, F1-20 |
| Unplug/replug quickly ×10; it must settle to one stable Connected | F1-17 |
| Set `happy`, fully quit and restart the desktop app → the device returns to `idle` | F1-19 |
| Read the Log view while connected: no raw bytes, device id, paths or usernames | F1-14, F1-22 |

## 2. Display and mascot (~20 min)

| Step | Rows |
|---|---|
| Show each of `idle/happy/busy/sleeping` from Device Studio and time the state change | F1-10, F1-11 |
| Freeze a fixed state and time in Device Studio; compare with the panel side by side | F1-12 |
| Record the SPI frame rate from the device's Health or Log output | F1-13 |
| Play every reaction in Idle. Check that Greet and Surprise don't look like Happy or Booting | P2 |
| Play a reaction in Busy: nothing plays. In Sleeping: a gentle sleepy response | P2 |
| Flash from Device Studio (**Flash firmware**); it reconnects to the same port | P2 |
| Leave it running 30 min under sustained state changes with no stall | F1-18, F1-21 |

## 3. Rotary volume (Windows) (~40 min)

| Step | Rows |
|---|---|
| One detent = one step, both directions; slow rotation loses nothing | S2-1, S2-2 |
| Fast spins give no false reversals; wiggle at a detent edge gives no phantom steps | S2-3, S2-4 |
| The Windows flyout matches the device; changing it in the flyout updates the device with no input | S2-5, S2-6 |
| Change the flyout **during** a turn: no fight, and it applies at gesture end | S2-7 |
| Preview vs Confirmed look different on the panel | S2-8 |
| Switch the default output device between turns (rebinds) and mid-turn (the gesture is discarded) | S2-9, S2-10 |
| Clamp at 0 and 100; the first reverse detent leaves the limit immediately | S2-11 |
| Unplug mid-turn and replug: nothing from the old turn executes | S2-12 |
| Restart the desktop app with the device powered: no stale overlay | S2-13 |

## 4. Latency (~15 min)

1. `just fw-flash-latency` (dev-only build; see S2 "Measuring row 14").
2. Turn the knob in single detents about 30 times. Read `L` (last) and `H` (max) off the panel.
3. **S2-14** passes if the max is under 50 ms. Panel scan-out adds up to one refresh on top, so note
   the value as measured to flush completion.
4. Reflash the product firmware with `just fw-flash` before closing the session.

## Afterwards

Update the roadmap checkboxes for Phases 1–3. A phase closes only when all of its rows are ✅
(roadmap rule 1).
