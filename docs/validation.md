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
| Button left / middle / right | 0 / 1 / 20 | Button other leg | GND (no resistor) |

1. Once per machine: `bun install`, `cargo install espflash`, install `just`.
2. Flash the product firmware: `just fw-flash` (builds with `physical-st7789`, then flashes and monitors).
3. Start the app: `just desktop` (one command: builds the firmware, starts the UI dev server, runs the
   app with the firmware embedded so Flash firmware works, and stops the UI server when the app quits).
   Plain `cargo run -p kivori-desktop` bundles no firmware and logs `firmwareUnavailable`.
   Discovery is automatic; there is no port picker. Expect **Connected**, firmware 1.3.0, protocol 1.5.
   On macOS, shortcuts and media keys need Accessibility permission for the app that runs Kivori
   (during development, the terminal running `cargo run`): System Settings > Privacy & Security >
   Accessibility.
4. Latency only: `just fw-flash-latency` (dev-only probe build), then `just fw-flash` to restore the product build.

Suggested order: wiring (3.15) and flash, then Phase 1 (Windows, then macOS), Phase 2, Phase 3, latency,
then Phase 4 (M1). Rows marked macOS run on the Mac; everything else on Windows.

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

## Phase 4: Push switch, actions, monitoring (M1)

Software is done and host-tested (button machine, recovery, desk pipeline, e2e host-sim round trip);
these rows need the real switch, the real OS and eyes on the panel. Default bindings: Press = Play/Pause,
Hold = master mute. Shortcut and launch run from Device Studio's Test action panel until M2.

**Switch and recovery**

- [ ] 4.1 Short press (well under 0.5 s), ten times | exactly one Play/Pause per press; the keycap dips (or a white frame shows) the moment you press; holding never auto-repeats
- [ ] 4.2 Hold about 1 s and release, five times | exactly one mute toggle each time; never also a Play/Pause
- [ ] 4.3 Hold 3 s, release | the recovery screen with a filling bar appears at about 2 s; release cancels it; nothing runs
- [ ] 4.4 Hold 10 s with Kivori Desktop running | "Restarting", the device reboots, the app shows the disconnect and reconnects by itself (gate 8)
- [ ] 4.5 Hold 10 s with Kivori Desktop quit | the device still reboots (gate 8, invariant 24)
- [ ] 4.6 Turn the knob while holding the switch | volume does not change and recovery is not cancelled (invariant 33)
- [ ] 4.7 Press while the knob is still turning | no Play/Pause fires
- [ ] 4.8 Unplug, press and hold during the outage, replug | nothing runs after reconnect (gate 2)

- [ ] 4.24 Double press, ten times at a natural speed | the device moves to the next view each time (Buddy, Clock, Volume, Media, System, Buddy); no Play/Pause fires
- [ ] 4.25 Single press after the double-press change | Play/Pause still fires once, about a quarter second after release; note whether the delay feels acceptable

- [ ] 4.26 Double press through every view and watch the slide | smooth vertical slide, no tearing or leftover pixels; the status-row time sliding past content is acceptable
- [ ] 4.27 Media view with a long title (macOS adapter or Windows) | title and artist correct, long text scrolls smoothly; accented Latin-1 letters render; other scripts show `?`
- [ ] 4.28 Frame cost of the redesigned views | `just fw-flash-latency`, show the Volume view and turn the knob: latency max still under 50 ms (row 3.14); note any visible stutter in Clock / System

**Actions and confirmation**

- [ ] 4.9 Hold to mute, then again to unmute; compare the OS mixer | OS mute matches each time; badge is the green check (State Confirmed). Windows and macOS
- [ ] 4.10 Mute or unmute from the OS (keyboard key, flyout, menu bar) | the panel's mute indicator follows with no Kivori input. Windows and macOS
- [ ] 4.11 Press while Spotify or a browser plays (Windows) | playback toggles; badge is the amber "?" (Unverified, never a check); the media indicator and Media view follow the real state within about 1 s
- [ ] 4.12 Press while something plays (macOS) | playback toggles; badge is the amber "?" (Unverified), never a check; Kivori does not crash (input runs on the main thread)
- [ ] 4.13 Test action: shortcut (Windows `Ctrl+Shift+Esc`, macOS `Cmd+Space`) | the shortcut happens; badge is the amber "?" (Unverified). Windows and macOS
- [ ] 4.14 Test action: launch (`notepad` / `Calculator`), then a name that does not exist | the app starts with the blue arrow (Execution Confirmed); the missing one shows the red cross. Windows and macOS
- [ ] 4.15 macOS with Accessibility permission removed: Press | red cross, the app says Accessibility permission is needed, nothing is sent another way (gate 5); after granting, Press works without restarting Kivori
- [ ] 4.16 Force a volume failure (Windows: disable the output device) and turn the knob | red cross on the panel, never a fake volume (gates 6 and 9)

**Monitoring and display**

- [ ] 4.17 Pick each mode in Overview | the panel shows Buddy, Clock, Volume, Media, System; each is recognisable. Windows and macOS
- [ ] 4.18 Clock view | matches the computer clock; still within a minute after 10 minutes
- [ ] 4.19 System view | CPU and RAM within about 10 points of Task Manager / Activity Monitor; a 10 s CPU stress turns the CPU bar amber and shows the heat cue over the buddy, which clears once load drops
- [ ] 4.20 Media view | Windows follows Playing / Paused / Stopped within about 1 s; macOS shows "No media info"
- [ ] 4.21 Volume view | follows the OS volume and mute; turning the knob shows the hollow preview, then the solid confirmed value
- [ ] 4.22 Quit Kivori Desktop | the device shows the offline buddy within the heartbeat timeout; no stale mode or values remain (gate 9)
- [ ] 4.23 Daily use for a few days (subjective, record notes) | press and hold timing feel natural; accidental holds or missed presses are noted for tuning

**PRD gates**

- [ ] G5 No hidden fallback | an action that cannot run says so and never switches mechanism (4.15)
- [ ] G6 No state invention | unknown values show as `--` on the panel and `—` in the app (4.17 to 4.20)
- [ ] G8 Recoverability | recovery works without healthy desktop software (4.4, 4.5)

## Phase 5: Contextual buttons (M2)

Software is done and host-tested (firmware gesture rules, wire, desktop bindings, e2e round trip).
Wiring: three momentary tactile switches (6x6 mm), each between its GPIO and GND, no resistors or
capacitors; firmware enables the internal pull-ups and debounces 20 ms. Default bindings, left to
right: Previous track, Play/Pause, Next track (Press only; Hold unbound by default).

- [ ] 5.1 Before wiring: inspect the devkit near GPIO0/GPIO1 | no 32.768 kHz crystal fitted (if one is, stop: pick other pins before soldering)
- [ ] 5.2 Power on with each button held in turn | Kivori boots normally every time (none is a strapping pin); the boot log still appears on GPIO21
- [ ] 5.3 Each button, ten short presses while music plays | exactly one Previous / Play-Pause / Next per press; amber "?" badge (Unverified); a white frame on press
- [ ] 5.4 Each button held about 1 s | nothing runs (Hold is unbound); held 12 s | nothing runs and the device never reboots
- [ ] 5.5 Two buttons pressed together; a button pressed while the knob switch is down; a button pressed mid-turn | nothing fires from the second control (no chords)
- [ ] 5.6 Hold the knob switch 10 s while a button is held | the device still reboots (invariant 24)
- [ ] 5.7 Buddy view | button labels sit under the keycap, ticks line up with the physical buttons, `TURN Volume` top left; the keycap is full size
- [ ] 5.8 Unplug, press buttons during the outage, replug | nothing runs after reconnect (gate 2)

## Phase 5b: Config UI (M2)

Software is done and host-tested (config store, resolve, IPC, Controls page with the browser mock).
These rows need a person, the real device and the real OS.

- [ ] 5.9 Controls page: Edit Press, pick a keyboard shortcut, save | the "Custom" badge appears on the row and the device label updates within a moment; Reset puts the built-in back
- [ ] 5.10 Rebind a control, use it on the device, quit and restart Kivori Desktop | the binding is still there (Custom badge, device label) and still works
- [ ] 5.11 Install a newer build over the old one (settings kept) | the custom bindings are still there; no "settings were damaged" banner
- [ ] 5.12 Windows: bind the knob to App Volume `spotify.exe` while Spotify plays, turn the knob | Spotify's slider in the Windows volume mixer moves, the master volume does not; one badge at the end of the turn
- [ ] 5.13 Windows: bind a button to App Mute `spotify.exe`, press it twice | Spotify mutes and unmutes in the mixer, master mute is untouched; badge is the green check
- [ ] 5.14 Controls page Test on a keyboard shortcut | a 3-second countdown appears (Cancel stops it); after it, the shortcut lands in the app you clicked; badge is the amber "?"
- [ ] 5.15 Hold on a contextual button bound in the UI | the action runs; the device does not show a label for it yet (known gap)
- [ ] 5.16 Reset profile and Reset everything | each asks first; afterwards the Custom badges are gone and the device shows the built-in labels
- [ ] 5.17 macOS: open Edit on the knob | App volume is greyed out with a reason; nothing falls back to system volume
- [ ] 5.18 Controls page: create a macro (Mute, Wait 500 ms, Play / Pause), bind it to a button and press it; try to delete it while bound | the steps run in order; the badge is the amber "?" (the least certain step); Delete is refused with a message until the button is unbound; focusing another app mid-macro stops it before any shortcut or launch step

## Phase 6: Profiles and layout (M2.5)

- [ ] 6.1 Windows: focus Chrome, VS Code, Spotify, Zoom or Teams in turn | about half a second later the profile name and all labels change together; the knob does tabs in Chrome
- [ ] 6.2 Turn the knob in Chrome and Alt+Tab mid-turn | tab switching stops at once, one red cross, nothing lands in the new app
- [ ] 6.3 Trigger a UAC prompt, lock with Win+L, open an elevated app (regedit) | "Protected": button shortcut labels gone, pressing one shows the red cross, the volume knob still works
- [ ] 6.4 Hold the middle button repeatedly | General, Browser, Code, Media, Zoom, Teams, then back to following focus; a dot shows while pinned; a UAC prompt still wins over a pin
- [ ] 6.5 macOS: the same app switches (Safari, VS Code, Music) and the lock screen | same behaviour (daily test bed)
- [ ] 6.7 Buddy faces: mute the OS, play music, run a CPU stress, join a Zoom call, trigger a failing and a confirmed action | muted, listening, strained, attentive, error and celebrate faces each appear only for their real cause; a shortcut (Unverified) never celebrates; unplugging shows the offline face, distinct from sleeping; note whether the instant face swap looks like a pop
- [ ] 6.6 Layout at desk distance | profile, clock, knob label and button labels readable; button labels sit over their physical buttons; the buddy is full size

## Evidence log

| Date | What | Result |
|---|---|---|
| 2026-08-04 | macOS hardware inventory, read-only | No board attached, nothing flashed. Desktop launched, 22 threads; in-window checks blocked by missing Accessibility permission |
| 2026-08-11 | Windows, physical board | Rows 1.5, 1.9, 1.23, 1.24, 1.25 passed (see above) |
| 2026-09-07 | CI Windows startup smoke (automated) | `kivori-desktop.exe` alive for 10 s, no stack overflow. Not a manual row |
| 2026-09-08 | Mascot automated checks | 204 workspace tests, 36 firmware host-sim tests, clippy clean, blob 116,634 of 131,072 bytes. Panel fps, SPI time and playback not measured |
| 2026-09-25 | macOS 27, debug build, no device | Rows 1.1, 1.3, 1.4 passed on macOS; 1.22 partial; 1.19 code review only |
| 2026-10-02 | macOS 27, ESP32-C3 over USB, automated (no human input) | `espflash` flashed the product build (4 MB flash, rev v0.4); production `Session` handshake Connected in 25 to 776 ms over repeated warm reconnects, heartbeats stable for 8 s each. Found and fixed: a reconnect to a device already in the desired state never learned it (no `StateReport`); verified fixed on the board. Interrupt edge-capture and M1 firmware boot and connect. Not a manual row: cold plug, knob, switch and panel were not exercised |
| 2026-10-02 | macOS 27, CoreAudio backend, automated | Volume set and read-back, mute toggle and read-back, an external `osascript` change arriving as External, own writes tagged Kivori; original volume and mute restored. CPU, RAM and local time read |

Screenshots and logs from before this consolidation stay in git history.
