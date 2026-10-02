# Hardware validation

The one checklist for everything CI cannot prove: real ESP32-C3 + ST7789 + HW-040 hardware, real
OS behavior, real timing. Simulation and host tests are never promoted to physical evidence.

Row numbers: `1.N` is row N of the old Phase 1 (device connection) checklist, `3.N` is row N of the old
rotary checklist, `2.N` are new Phase 2 rows, `G` rows are PRD gates. Tick a row only with a date and a
measurement or observation on the same line. A phase closes only when all its rows are ticked.

## How to run a session

**You need:** the board, a Windows PC and a Mac, one unrelated USB-serial device (Arduino or UART
adapter), and a phone that films slow motion (backup for latency only).

Pins (source of truth: `firmware/esp32-c3/src/profile.rs`). Display pins are measured; encoder pins are a
specification until row 3.15 is ticked.

| Signal | GPIO | Signal | GPIO |
|---|---:|---|---:|
| SPI SCK (SPI2, 20 MHz, Mode 3) | 6 | Encoder CLK | 4 |
| SPI MOSI | 7 | Encoder DT | 5 |
| Display D/C | 2 | Encoder SW | 10 |
| Display RST | 3 | USB D- / D+ | 18 / 19 |
| Backlight (active-high) | 8 | Encoder COM / VCC | GND / **3V3** (never 5 V) |

1. Once per machine: `bun install`, `cargo install espflash`, install `just`.
2. Flash the product firmware: `just fw-flash` (builds with `physical-st7789`, then flashes and monitors).
3. Start the app: `just dev` (Vite UI), then `cargo run -p kivori-desktop` in a second terminal.
   Discovery is automatic; there is no port picker. Expect **Connected**, firmware 1.0.0, protocol 1.2.
4. Latency only: `just fw-flash-latency` (dev-only probe build), then `just fw-flash` to restore the product build.

Suggested order: wiring (3.15) and flash, then Phase 1 (Windows, then macOS), Phase 2, Phase 3, latency.

## Phase 1: Device connection

Windows unless a row says otherwise. Rows 1, 3, 4 passed on macOS but stay open until Windows runs.

**Background operation**

- [ ] 1.1 Close the window, then check the process | window hides, process keeps running. macOS 2026-09-25 done (debug build, no device; hidden not moved Space, checked with CGWindowList). Windows pending
- [ ] 1.2 While hidden for 2 minutes, watch the Log view | device heartbeat and synchronization stay active
- [ ] 1.3 Reactivate (taskbar or Dock, and a second launch) | one window reappears, one process. macOS 2026-09-25 done (Dock click needed a fix for `RunEvent::Reopen`, re-verified). Windows pending
- [ ] 1.4 Tray, then Quit | process exits and the device task stops. macOS 2026-09-25 done (exit code 0 in about 1 s). Windows pending

**Discovery and connection**

- [x] 1.5 Plug in the board | enumerates as `0x303A:0x1001`, auto-discovered with no port selection, handshake confirms identity. 2026-08-11: COM3, Connected, firmware 1.0.0, protocol 1.0
- [ ] 1.6 Plug in from cold and time it in the Log view timestamps | connected in under 5 s. Connection seen working, latency never measured
- [ ] 1.7 Plug in the unrelated USB-serial device | not reported as connected
- [ ] 1.8 Flash a wrong-major firmware | shows incompatible with a clear reason within 5 s, no state commands sent

**Companion states**

- [x] 1.9 Display init and panel offsets | 2026-08-11: ST7789 240x240 initialized, offset (0,0)
- [ ] 1.10 Show each of idle, happy, busy, sleeping from Device Studio | matching animated scene each time. Only idle confirmed so far
- [ ] 1.11 Change state and time it | appears on the device in under 1 s. Idle propagation seen, latency never measured
- [ ] 1.12 Freeze a fixed state and time in Device Studio, compare side by side with the panel | images match
- [ ] 1.13 Read the SPI frame rate from Health or Log output | a sustainable rate is recorded
- [ ] 1.14 Read the typed session activity view | only semantic state is sent, no raw payload bytes shown

**Recovery and restoration**

- [ ] 1.15 Unplug while busy | app shows Disconnected
- [ ] 1.16 Replug, do nothing | auto-reconnects and returns to busy within 10 s
- [ ] 1.17 Unplug and replug quickly 10 times | settles into one stable Connected
- [ ] 1.18 Run 30 min of sustained state changes | no USB stall, stall recovery works
- [ ] 1.19 Set happy, fully quit and restart the app | device returns to idle. Code review 2026-09-25 says it should (no persistence); needs a device to observe

**Platform and safety**

- [ ] 1.20 Collect the three timings | state change < 1 s, connect < 5 s, reconnect plus restore < 10 s
- [ ] 1.21 Confirm `usb_serial_jtag` behavior on the pinned esp-hal version | no drops or hangs under 1.18 traffic
- [ ] 1.22 Read the activity view while connected | no raw bytes, device id, paths or usernames; no log file is created. Partial macOS 2026-09-25: no log file created, only WebKit storage and stdout; connected part needs the device
- [x] 1.23 Identify the panel controller | 2026-08-11: ST7789
- [x] 1.24 Record the real SPI pin map | 2026-08-11: SCK 6, MOSI 7, no CS, D/C 2, RST 3, backlight 8 active-high; SPI2 20 MHz Mode 3
- [x] 1.25 Measure panel offsets | 2026-08-11: (0,0). Also verified: 90 degree rotation, RGB order, inversion on

## Phase 2: Mascot, reactions, flashing

Software is done (goldens, render parity, 204 workspace tests); these rows need eyes on the panel.
Desktop automation was unavailable, so no native click-through has been verified either.

- [ ] 2.1 Show each of the six states on the panel: booting, idle, happy, busy, sleeping, offline | each is recognisable and matches the Device Studio preview
- [ ] 2.2 Set happy | the keycap cap sinks in a deep press, face is the happy face
- [ ] 2.3 Play every reaction in Idle: Greet, Pet, Tickle, Surprise, Comfort | each plays, returns to idle, and no reaction looks like Happy or Booting (check the recombined Greet and Surprise faces in particular)
- [ ] 2.4 Play a reaction in Busy | nothing plays and Desktop does not claim it did
- [ ] 2.5 Play a reaction in Sleeping | gentle sleepy response, then back to Sleeping
- [ ] 2.6 Change state during a reaction | reaction stops and the normal eased transition runs
- [ ] 2.7 Turn the knob so the volume overlay shows, in each state | the keycap shrinks and lifts to clear the volume bar (y 180 to 196), no overlap or flicker
- [ ] 2.8 Watch idle for 2 minutes | blinks and expression swaps look smooth, no crawling edges, no full-screen flashes
- [ ] 2.9 Click Flash firmware in Device Studio | flashing completes and the app reconnects to the same port

## Phase 3: Rotary volume (Windows)

**Rotary input fidelity**

- [ ] 3.1 Turn one detent each way | exactly one volume step per detent
- [ ] 3.2 Turn slowly | no detent lost
- [ ] 3.3 Spin fast | no false reversals
- [ ] 3.4 Wiggle at a detent edge | no phantom detents, invalid-transition counter stays low

**Windows Core Audio**

- [ ] 3.5 Compare the Windows volume flyout with the device | values match
- [ ] 3.6 Change volume in the flyout | device updates with no Kivori input
- [ ] 3.7 Change the flyout during a turn | no fight with the preview; it applies at gesture end
- [ ] 3.8 Look at the panel during and after a turn | Preview and Confirmed look clearly different
- [ ] 3.9 Switch the default output device between turns | rebinds and shows the new device's volume
- [ ] 3.10 Switch the default output device mid-turn | the gesture is discarded, not retargeted

**Bounds and recovery**

- [ ] 3.11 Turn to 0 and to 100, then reverse | clamps at both ends, first reverse detent leaves the limit immediately
- [ ] 3.12 Unplug mid-turn, replug | the old gesture executes nothing
- [ ] 3.13 Restart Kivori Desktop with the device powered | presentation resumes, no stale overlay

**Latency and wiring**

- [ ] 3.14 Flash `just fw-flash-latency`, turn single detents about 30 times, read `L` (last) and `H` (max) off the panel corner | max under 50 ms detent to flush complete; record several readings plus `H`. This is a gate: the phase is not done while this fails or is unmeasured. Scan-out adds up to one refresh on top. Worst case predicted at about 85 ms (50 ms desktop tick, serial, 33 ms frame), so expect to tune; remedies in order: blocking serial read instead of the 50 ms tick, then a shorter frame interval, then device-local feedback. Re-measure after any change, then `just fw-flash`
- [ ] 3.15 Check wiring with a continuity test | CLK=GPIO4, DT=GPIO5, SW=GPIO10, COM to GND, VCC to 3V3. Do this first: rows 3.1 to 3.4 depend on it

**PRD gates**

- [ ] G1 No false confirmation | the panel never shows Confirmed for a volume Windows did not actually report, across 3.5 to 3.13
- [ ] G2 No stale replay | after unplug, restart or device switch, no old gesture or overlay is ever replayed (3.10, 3.12, 3.13)

## Evidence log

| Date | What | Result |
|---|---|---|
| 2026-08-04 | macOS hardware inventory, read-only | No board attached, nothing flashed. Desktop launched, 22 threads; in-window checks blocked by missing Accessibility permission |
| 2026-08-11 | Windows, physical board | Rows 1.5, 1.9, 1.23, 1.24, 1.25 passed (see above) |
| 2026-09-07 | CI Windows startup smoke (automated) | `kivori-desktop.exe` alive for 10 s, no stack overflow. Not a manual row |
| 2026-09-08 | Mascot automated checks | 204 workspace tests, 36 firmware host-sim tests, clippy clean, blob 116,634 of 131,072 bytes. Panel fps, SPI time and playback not measured |
| 2026-09-25 | macOS 27, debug build, no device | Rows 1.1, 1.3, 1.4 passed on macOS; 1.22 partial; 1.19 code review only |

Screenshots and logs from before this consolidation stay in git history.
