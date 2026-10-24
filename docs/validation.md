# Hardware validation

The one checklist for everything CI cannot prove: real ESP32-C3 + ST7789 + HW-040 hardware, real
OS behavior, real timing. Host-sim and other host tests are never promoted to physical evidence.

Row numbers: `1.N` is row N of the old Phase 1 (device connection) checklist, `3.N` is row N of the old
rotary checklist, `2.N` are new Phase 2 rows, `G` rows are PRD gates. Tick a row only with a date and a
measurement or observation on the same line. A phase closes only when all its rows are ticked.

## How to run a session

**You need:** the board, a Windows PC and a Mac, one unrelated USB-serial device (Arduino or UART
adapter), and a phone that films slow motion (backup for latency only).

Pins (source of truth: `firmware/pala/src/profile.rs`). Display pins are measured; encoder pins are a
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
   Discovery is automatic; there is no port picker. Expect **Connected**, firmware 0.1.0, protocol 0.1. A board flashed before the version reset
   (2026-10-12) speaks protocol 1.x and shows as incompatible until this step reflashes it.
   On macOS, shortcuts and media keys need Accessibility permission for the app that runs Kivori
   (during development, the terminal running `cargo run`): System Settings > Privacy & Security >
   Accessibility.
4. Latency only: `just fw-flash-latency` (dev-only probe build), then `just fw-flash` to restore the product build.

Suggested order: wiring (3.15) and flash, then Phase 1 (Windows, then macOS), Phase 2, Phase 3, latency,
then Phase 4 (M1), then Phases 5 to 12 in order (5b is the Config UI half of the M2 rows; 7 to 12 are the M3 rows). Rows marked macOS run on the Mac; everything else on Windows.

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
- [ ] 3.15 Check wiring with a continuity test | CLK=GPIO7, DT=GPIO6, SW=GPIO5, COM to GND, VCC to 3V3. Do this first: rows 3.1 to 3.4 depend on it

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
Wiring: three momentary tactile switches (12×12 mm, 10 mm tall, seller drawing TS-G005: the size the enclosure in `hardware/enclosure/` is built for; any momentary switch works electrically), each between its GPIO and GND, no resistors or
capacitors; firmware enables the internal pull-ups and debounces 20 ms. Default bindings, left to
right: Previous track, Play/Pause, Next track (Press only; Hold unbound by default).

- [ ] 5.1 Before wiring: inspect the devkit near GPIO0/GPIO1 (display BLK/DC since #123) | no 32.768 kHz crystal fitted (if one is, stop: pick other pins before soldering)
- [ ] 5.2 Power on with each button held in turn | B2 (GPIO8) and B3 (GPIO10): Kivori boots normally and the boot log still appears on GPIO21. B1 (GPIO9, BOOT): the board enters ROM download mode, as documented. With the display connected and no button held, the board boots (GPIO2 is display RES)
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

## Phase 7: Host presence (M3 S2)

Software is done (tracker, device-thread and Windows window tests, macOS IOKit registration test); these rows need a real sleep, lock and switch.

- [ ] 7.1 Windows: press Win+L | the buddy sleeps within 1 s; the volume knob still works; button shortcut labels are gone
- [ ] 7.2 Windows: unlock | back to normal, profile labels restored, the state you had chosen returns
- [ ] 7.3 Windows: Start, then Sleep (S3) | the device shows Sleeping, not Offline, before the PC goes dark
- [ ] 7.4 Windows: wake | reconnected and normal within 5 s; no knob turn or press made during sleep runs
- [ ] 7.5 Windows: Hibernate, then resume | same as 7.4
- [ ] 7.6 Windows: Modern Standby laptop, close the lid for 2 min, open it | Sleeping, then reconnect. Note whether the goodbye arrived in time (if not, the device shows Offline after about 4 s)
- [ ] 7.7 Windows: a machine that cuts USB power in sleep | the device boots and reconnects on wake
- [ ] 7.8 Windows: switch user | user A's Kivori releases the port; user B's Kivori connects
- [ ] 7.9 macOS: `cargo test -p kivori-desktop` on the Mac | passes, including the IOKit registration test
- [ ] 7.10 macOS: lock with Ctrl-Cmd-Q, then unlock | the buddy sleeps within about 1 s; unlock restores it
- [ ] 7.11 macOS: Apple menu, Sleep, then wake | Sleeping before the screen goes dark, then reconnect
- [ ] 7.12 macOS: close the lid on battery, then open | same as 7.11

## Phase 8: Firmware update and recovery (M3 S3)

Automated image-set checks compile a generated two-product index outside shipping catalog roots and
verify exact lookup, deterministic order and duplicate/invalid-image refusal. Multi-image runtime
fixtures cover exact product/board selection, per-target version/size/advice, active metadata retention,
IPC reservation preservation, generic recovery offers without an assumed version, and explicit
provisioning selection before I/O. The legacy single-image path and empty host builds remain supported.

Packaging fixtures build two products and multiple boards outside shipping roots, including a
mocked shared Cargo artifact to verify immediate snapshots. They cover deterministic order,
failed-build/no-manifest behavior, exact transferred filenames, nonregular/empty files, receiving-host
absolute paths and native complete-set enforcement. Unix/Windows wrapper contracts mock child
commands to check input isolation, cleanup and environment restoration without executing the app.

Automated M2.8 resolver/workflow checks cover missing/unknown images, stale or invalid product choices, unknown
USB hints, unknown reported identities, connected-session pending-identity refusal, same-port identity binding, shared-USB board
ambiguity with explicit product/board choices, distinct-board selection, refusal status cleanup and no port
reservation/flasher start on refusal. The fixtures are not shipping products. These checks do not
complete the hardware rows below.

Software is done (workflow, classifier fixtures, recovery rules, sidecar lookup, DTO allowlist, UI tests). The classifier fixtures are conservative guesses at `espflash` 4.x wording; row 8.7 refreshes them. Rows 8.1 to 8.6 need a real device and the installed app (the S5 installer bundles the `espflash` 4.6.0 sidecar) or a dev build with `espflash` on `PATH`.

- [ ] 8.1 Flash from the installed app on a machine with no Rust toolchain | it succeeds and the device reconnects (needs the S5 installer)
- [ ] 8.2 Unplug the board mid-flash | the app says it failed and offers Restore with the BOOT button
- [ ] 8.3 Erase the flash, then Restore with the BOOT steps | the unit recovers and shows its hash
- [ ] 8.4 Hold the port open in a serial monitor, then flash | the "close other apps" message appears
- [ ] 8.5 Flash a wrong-major firmware, then use Restore | Restore is offered on the Device page and works
- [ ] 8.6 Flash from the app | the panel shows Updating (or goes dark cleanly) during the flash, never garbage, then returns to the buddy
- [ ] 8.7 Capture `espflash` stderr for each failure above | add each as a classifier fixture in `tests/firmware_flash.rs`
- [ ] 8.8 With an older app-bundled version than the device, and with a newer one | the Device page and Home show "update available" only when the bundled firmware is newer

## Phase 9: Launch at login and tray (M3 S4)

Software is done (tray model table, hidden-start lifecycle, `--autostart` parsing, Device page switch). Every row needs a real sign-in, so none are checked yet. The Windows uninstall row needs a real uninstall of the S5 installer (its hook removes the Run entry).

- [ ] 9.1 Windows: turn on Start with Windows, sign out and back in | the tray icon is present, no window opens, the device connects
- [ ] 9.2 Windows: turn it off, sign in again | Kivori does not start
- [ ] 9.3 Windows: uninstall | the `HKCU\...\Run\Kivori` entry is gone (needs the S5 installer)
- [ ] 9.4 macOS: same as 9.1 and 9.2 through the LaunchAgent | and the "Background item" notice appears once
- [ ] 9.5 Tray menu: Launch at login, Open Kivori and Quit | each works; the check mark matches the Device page switch after reopening the window
- [ ] 9.6 Connect, unplug, lock the screen | the tray line reads Connected, Not connected, Paused while locked; the tooltip shows both versions

## Phase 10: Installer and release (M3 S5)

CI builds the NSIS installer, installs it silently, checks the app and the `espflash` sidecar, runs the startup smoke test with `--autostart`, and uninstalls (including the Run entry). Everything below needs a real machine. Rows 8.1 and 9.3 depend on this phase.

- [ ] 10.1 Clean Windows 11 VM with no dev tools: run the installer | SmartScreen "More info, Run anyway" works (unsigned build)
- [ ] 10.2 Run the installer as a normal user | it installs without an admin prompt
- [ ] 10.3 Install a newer build over an older one | the config is kept
- [ ] 10.4 After installing | Windows Defender does not quarantine `espflash.exe`
- [ ] 10.5 macOS: `scripts/bundle-macos.sh` | the `.app` has `Contents/Resources/mediaremote-adapter` and `Contents/MacOS/espflash`
- [ ] 10.6 macOS signed build: `codesign -d --entitlements - Kivori.app` | shows `com.apple.security.automation.apple-events` (needs the owner's Developer ID)
- [ ] 10.7 macOS bundled app | now-playing works, and the Automation prompt shows the usage text

## Phase 11: First-run onboarding (M3 S6)

Automated: Plug-in/permission gates follow current readiness or Skip; the capability-filtered Tour is non-blocking guidance and does not verify hardware. Tests cover session changes, offline saved settings, launch consent, completion/restart and skipping permissions on Windows. These unchecked rows still need a real machine and a Kivori.

- [ ] 11.1 Fresh Windows user (no `config.json`), start Kivori | onboarding appears once; Plug-in waits for a known usable session; the Tour shows only supported rotary/push-switch/context-button guidance with no automatic completion ticks. Manually try the controls against their saved bindings; this walkthrough is not a hardware test
- [ ] 11.2 Finish setup, quit and relaunch | onboarding does not appear again
- [ ] 11.3 macOS without Accessibility: choose "Allow access" | the macOS prompt appears; after ticking Kivori in System Settings the step turns green without restarting setup
- [ ] 11.4 Update from a build that already had a `config.json` | onboarding does not appear
- [ ] 11.5 Device page, "Run setup again" | setup reopens; Finish with Launch at login on adds the login entry, Skip setup leaves it unchanged

## Phase 12: Per-unit QA (M3 S7)

Run for every assembled beta unit with `just provision` ([provisioning.md](./provisioning.md)). The tool's evaluator is unit tested; the rows need a real unit and a person.

- [ ] 12.1 Plug in one new unit, `just provision` | it flashes, then prints PASS (catalog-required capabilities incl. HOST_TAKEOVERS_V1 for Pip, post-flash Build/Chip identity matches target, firmware equals image identity and version marker, state and health seen, heartbeat held 5 s)
- [ ] 12.2 Follow the prompts | knob right x3, left x3, press, hold and the three buttons are each seen
- [ ] 12.3 Open `provisioning-log.csv` | one row for the unit: short hash and versions, no raw device id, no port name

## Phase 13: Beta hero features (M2.6)

Software rules in [product.md](./product.md); design in ADR-0011 and ADR-0012. Windows first.

- [ ] 13.1 Two mics active (built-in and a USB headset), Mic Mute | both read muted in Windows Sound settings; buddy shows the covered-mouth face; Confirmed (celebrates), not amber
- [ ] 13.2 Mic muted, plug in another mic | it is muted too within about 1 s; if it cannot be, the panel shows the mic live, never muted
- [ ] 13.3 Unmute from Windows settings while Kivori shows muted | the panel follows within about 1 s (External); no Kivori action runs
- [ ] 13.4 Start a Zoom or Teams call, mic unmuted | a Live cue shows; mute it and the cue changes to muted; with the cue unknown nothing shows
- [ ] 13.5 Zoom's own mute on, Kivori's off | Kivori says unmuted (it reports the OS mic, not Zoom's button); the Controls page explains the difference
- [ ] 13.6 Focus an app with no profile, "Add the app in front of me" | a new profile appears with that app's icon, starting as General; the panel shows its name within about ½ s of focusing it
- [ ] 13.7 Add `idea64.exe` (or any app) to Code, then to a custom profile | it moves, and the app says it moved; never in two profiles
- [ ] 13.8 Mark a custom profile "In pin cycle", hold the middle button repeatedly | it appears after the built-ins; unmarked profiles do not
- [ ] 13.9 Leave the app idle 5 minutes, then pause the desktop app (debugger or Task Manager suspend) | the buddy keeps blinking and glancing; no frozen frame; eyes never close fully outside Sleeping
- [ ] 13.10 Touch each control slowly with the app idle | the buddy looks towards it before the action's own feedback
- [ ] 13.11 Move the brightness setting from highest to lowest | the backlight follows at once; the lowest step is still clearly on (never looks like Display Sleep); the setting survives a restart
- [ ] 13.12 Press a media key button and a shortcut button | a quiet press and label pulse, no amber badge, no celebration; Activity says "Unverified"
- [ ] 13.13 Unplug the audio device and press Mute | the amber "?" appears (state action without read-back)
- [ ] 13.14 Developer mode on, Device page | a Device internals card shows flash, RAM, CPU, frame time, chip and link numbers that update about once a second; developer mode off hides it
- [ ] 13.15 Compare with the build | app image size matches the built firmware within a few KB; asset blob matches the blob size; spin the knob fast and busy % and frame time rise, then fall back
- [ ] 13.16 "Copy diagnostics" with developer mode on, then off | internals are included only when on; no port names, ids or file paths either way
- [ ] 13.17 Start the app from cold (after a reboot) | a splash with the buddy and "Starting" shows, then the main window; no percentage; start it again while warm and the splash does not flash up
- [ ] 13.18 Sign in with "Start with Windows" on | no splash and no window, only the tray icon

## Phase 14: Identity and updates (M2.7)

Rules in [product.md](./product.md#updates-us10-us11); design in ADR-0014 to ADR-0016.

- [ ] 14.1 Plug in a unit with M2.7 firmware, open Diagnostics | product "Kivori Pip", board "C3 SuperMini", revision shown (or "unknown" before provisioning)
- [ ] 14.2 Plug in a unit with older firmware | the app treats it as Kivori Pip on the C3 SuperMini and offers the firmware update a few seconds after it connects
- [ ] 14.3 Choose "Later", unplug and replug | the offer comes back; choose "Update now" | "Updating" on the panel, then the buddy; Device page shows the new firmware
- [ ] 14.4 Try to flash an image built for another board (developer build with a test image) | the app refuses and names the mismatch; nothing is written
- [ ] 14.5 Provisioning with `--product pala --board c3-supermini --rev 1` on a spare unit: first without, then with `--burn-identity` (permanent; [provisioning.md](./provisioning.md)) | without it the record is previewed and firmware is still flashed/checked, with no eFuse write; with it the record is shown and the product id must be typed; the run prints identity PASS and the log row says `chip`; afterwards Diagnostics shows the identity from the chip; a second burn attempt on the same unit says "already provisioned"
- [ ] 14.6 Install an older release, publish a newer one | within a day, or at the next start, a tray badge; opening the window shows the update dialog with the new version and notes
- [ ] 14.7 "Install & restart" | the installer runs without questions, Kivori restarts on the new version, the device reconnects; the firmware offer follows if the new app bundles newer firmware
- [ ] 14.8 "Skip this version", restart | no offer for that version; a newer one is offered again
- [ ] 14.9 Turn off the network, start the app | no error, no wait; turn off "Check for updates" | no request to GitHub (check with a firewall log)
- [ ] 14.10 Tamper with the downloaded update (developer test: wrong signature) | it is refused and nothing installs


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


## M2.8 product identifier guard (automated)

`just check-identifiers` runs a real-repository scan and isolated positive controls. Fixtures cover
catalog-derived names/ids/models/boards/reservations, camel/separator variants, untracked source and
binary filenames, exact exception counts, and leaks after Rust test modules. Structural checks
exercise raw/byte strings, character literals, lifetimes, nested comments, fake test markers in
production strings and unterminated inputs. They preserve source line numbers for diagnostics.

The offline dependency guard fixture uses mocked Cargo metadata with two firmware workspaces,
including a custom path with spaces. It asserts all workspaces are checked, a network API in the
second source root fails and catalog discovery errors fail closed. The mocked shell fixture runs
on Unix; the real dependency guard still runs in both Rust CI jobs. No guard fixture flashes,
burns identity or launches the desktop. These checks supplement behavioral acceptance
and the remaining hardware/recovery rows; a clean textual scan cannot prove capability behavior.

M2.8 pair recovery fixtures cover two boards with shared USB hints, exact-pair selection, stale/unknown
choices, board-only refusal, live-identity precedence and unchanged busy reservations. UI tests prove
that choice buttons only open a BOOT dialog, show both catalog names and queue the exact pair only
after confirmation; cancel, lost choices and a new live connection make no request. Unplugging for the BOOT steps keeps confirmation open. Payload tests reject
missing/oversized board fields, duplicate pairs and choices outside a failed-choice status. These
are synthetic policy/UI checks; actual BOOT/flashing/reconnect hardware rows remain unchecked.

M2.8 app-shell acceptance (`product-acceptance.test.tsx`) uses test-only connection payloads,
the production connection parser, real hooks/pages/navigation and the browser IPC backend. It
checks Pip's live view/action path, a minimal dial without display/mascot/media features, and mixed
capabilities with working display/context-button actions, saved-only push-switch bindings and no
mascot action.
A loaded Pip image disappears on a product switch; missing artwork keeps the new product's
captions and a neutral figure. Product switches preserve a
custom binding; an offline display edit preserves profile bindings. Pending, unknown and
identity-only sessions remove live display controls; original firmware remains identified as legacy.
These tests do not exercise the native Tauri event bridge, backend action refusal or real hardware.
The existing IPC tests cover the bridge contract separately; the native/catalog evidence is mapped below.

### M2.8 product acceptance evidence

These automated checks close the product acceptance test item, not the hardware acceptance rows.
Synthetic products and ELF bytes remain in test code or temporary directories outside shipping roots.

| Criterion | Automated evidence |
| --- | --- |
| Pip, a minimal dial, mixed features, usable supported controls, missing art, offline edits and preserved bindings | `__tests__/product-acceptance.test.tsx` mounts the real App/hooks/navigation with parsed fixture sessions and browser IPC. |
| Setup identity readiness, absent controls and saved-only settings | `features/onboarding/__tests__/capabilities.test.tsx` covers pending/unknown/identity-only/legacy and generation changes. |
| Session switch with late internals reply while the card stays mounted | `features/device/__tests__/internals.test.tsx` clears the old card and refuses the old session's reply. |
| No feature writes before identity resolves or for an unknown product | Native `tests/session.rs::every_feature_writer_is_inert_for_pending_and_unknown_identity` checks the writers and an empty wire. `reactions_require_current_identity_and_rejected_requests_are_never_replayed` adds a positive accepted action, reconnect-to-pending and unknown-board refusal without replay. |
| Current-session display/flash admission | Native `tests/session.rs::runtime_display_and_flash_admission_follow_current_session_support` uses real handshakes and the helpers called by the device loop; covers pending, unknown product/board, identity-only, known, legacy, disconnected and busy display states, plus missing exact images. Existing flash workflow tests cover exclusive reservations. |
| State versus preview intent | Native session tests prove saved desired state resyncs only when usable; Mirror never stages state for pending/unknown/identity-only/offline sessions. Frontend `features/controls/__tests__/controls.test.tsx` exercises host action tests against the disconnected browser backend. |
| Catalog additions need no maintained shared-source list | `catalog_build.rs::second_product_is_generated_without_a_source_list_and_reservations_stay_inactive` adds a temporary product and compiles the generated constants and snapshot art. |
| Exact images and complete packaging independent of order | `catalog_build.rs::firmware_set_generation_is_deterministic_and_compiles_with_exact_lookup`, native `tests/firmware_bundle.rs` and `tests/recovery_target.rs`, plus `scripts/build-firmware{,-set}.test.mjs`, validate identity, reversed inputs, complete sets, explicit selection and missing/mismatched refusal. |

Native paths above are relative to `apps/desktop/src-tauri`; frontend paths are relative to
`apps/desktop/src`. Rust fixtures compile generated source against synthetic marker-only ELF bytes; build-script fixtures
use the real generators with simulated Cargo outputs. They do not build or flash a second physical product. Native Tauri event-bridge coverage remains
separate from the app-shell browser tests. Request activity is intent, and queued commands use the
execution-time session as documented in [desktop capabilities](./desktop-capabilities.md).

### M2.8 software closure audit

The M2.8 software stack ([PR #77](https://github.com/cunilab/Kivori/pull/77) through
[PR #103](https://github.com/cunilab/Kivori/pull/103)) merged into `main` on 2026-10-19, ending at
[`a0355b9`](https://github.com/cunilab/Kivori/commit/a0355b93a5f47ea4e555aeb9d9ea942025d1af10).
Post-merge [host](https://github.com/cunilab/Kivori/actions/runs/39260985475),
[firmware](https://github.com/cunilab/Kivori/actions/runs/39260985385) and
[determinism](https://github.com/cunilab/Kivori/actions/runs/39260985155) workflows all passed.

The Product contract and Capability-driven UI items are implemented in that merged software.
The audit reuses the product-acceptance evidence above and checks the broader contract boundaries:

| Item | Implementation and evidence |
| --- | --- |
| Product contract | Native `catalog_build.rs` discovers metadata/art and compiles fixture additions without a shared product list. Provisioning selects QA/identity from the chosen image (`tests/provision_policy.rs`), with eFuse burns limited to the implemented C3 board backend. Build, desktop packaging and release helpers use the catalog's complete image set (`scripts/build-firmware{,-set}.test.mjs`, `scripts/firmware-release-guard.test.mjs`). The identifier guard and its CLI positive controls enforce the shared-source boundary with exact reviewed legacy/hardware/fixture exceptions. |
| Capability-driven UI | `Session::effective_caps` combines host/device negotiation with resolved current identity; frontend `device-support.ts` requires that connected known session. The documented surface mapping is exercised by app-shell, setup, Controls/Display, art and focus tests. Native session and runtime-admission tests prevent ordinary feature traffic from pending/unknown identities; saved settings and host-only action tests remain usable offline. |

The audit reruns the product identifier/build-script fixtures, provisioning/recovery/presentation
tests, and app-shell/support/art tests. Catalog registration, complete image indexes, release
orchestration and production-feature boundaries also retain their existing automated CI coverage.
This is software evidence: the Rust image fixtures contain synthetic markers, simulated Cargo
outputs do not validate a second physical product, and browser UI tests do not exercise native
serial hardware or the Tauri event bridge. The documented extension boundary still requires feature
work for a new transport, flash method or interaction model; metadata cannot provide those.

All M2.8 software checklist items are checked and merged. This evidence does not close issue #76
or claim a release, signing, real BOOT recovery, flashing or eFuse burn. Existing M2.7/M3 owner and
hardware rows remain open. ADR approval status is separate from implementation status.
Execution-time command checks and request-intent activity semantics remain the documented limits
above.

BOOT recovery regression coverage distinguishes an allowlisted port still attempting its handshake
from a connected session waiting for Identity. The runtime passes its connection state directly to
the shared resolver: Connecting may use guarded USB/choice recovery, while Connected without
identity remains refused. The fixtures cover explicit pairs, unknown product/board refusal and
ambiguous targets still requiring a choice. This is policy evidence, not a real BOOT/flashing run.

Diagnostics clipboard regression coverage checks that both ordinary and developer bundles omit
internal session counters while preserving diagnostic values and fresh measurements. Session
correlation remains in the IPC/cache data for rejecting stale replies; unavailable and stale
measurements stay excluded. These are mocked clipboard tests, not a native app clipboard run.

## M2.9 microphone coverage software acceptance

Implemented the first S1 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
This is software evidence from Andress-MacBook-Air; Phase 13 and M2.9 S4 Windows hardware checks
remain open.

Windows discovery now retains usable bindings when enumeration or identity inspection is
incomplete. An unreadable coverage entry prevents aggregate Muted and State Confirmed while
healthy microphones remain controllable. Endpoint and device-list notification registration retry
on subsequent scans; failed subscription never removes the ability to read/write mute. Reads,
writes and notification-driven rescans publish changed snapshots, including unknown coverage,
instead of retaining a previously confirmed state after failure.

Permanent regressions:

- `platform/mic_coverage.rs`: production reconciliation and observation helpers cover unknown
  collection-item/device-id results, activation failure, failed whole scans, retained bindings,
  missing notifications, restored coverage and a known empty device list.
- `desk/actions.rs`: incomplete coverage cannot confirm either mute or unmute; a known failure
  still takes precedence over an unknown result.
- `desk/mod.rs`: an incomplete-coverage event clears an already muted wire state before the next
  periodic sample, and a later complete observation restores it.

Local validation:

- `cargo test -p kivori-desktop --lib mic --locked -j 2`: 18 passed, one intentionally ignored.
- `cargo test -p kivori-desktop --lib --locked -j 2`: 286 passed, three intentionally ignored.
- `cargo clippy -p kivori-desktop --lib --tests --locked -j 2 -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- An isolated `x86_64-pc-windows-msvc` Clippy check of the production microphone backend,
  coverage helper and copied production boundary types against the real Windows APIs passed
  with warnings denied. This checks the changed Windows code, not the full Tauri application.

The full `x86_64-pc-windows-msvc` cross-check stopped in `ring` because Windows SDK C headers
(`assert.h`) are unavailable on this Mac. Host tests model subscription failure; they do not
exercise real COM registration, callback teardown or physical microphone hotplug. The real-device
Phase 13 checks must supply that evidence before beta acceptance.

Full host CI for the microphone implementation (`1509a87`) subsequently passed, including
[Windows Rust integration](https://github.com/cunilab/Kivori/actions/runs/40081910499/job/120837033069)
and Windows startup. This closes the software integration uncertainty for that commit, not the
real-device microphone checks above.

## M2.9 queued input software acceptance

Implemented the second S1 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
These checks use fake dispatchers and input effects; no real keyboard or media input was injected.

The action worker retains the request's original foreground app, session epoch and admission
deadline through main-thread queueing. Input is checked before and after connection/permission
preparation. An atomic pending/start/cancel decision prevents a timed-out pending or preparing
job from injecting later; the earlier of the request deadline and the two-second dispatch timeout
controls the wait. Input already started reports uncertain completion as Unverified, not Error.
Shortcuts refuse a changed/protected target; system media keys retain session/deadline checks
without depending on the foreground app. Worker shutdown invalidates pending input too.

An admitted macro keeps its intentional delays beyond the original admission deadline, with
session and target checks retained for later steps. An unfinished native input step stops further
steps as Unverified so it cannot overlap a later launch; a completed write with unknown read-back
can still continue. Product and architecture documentation record this distinction.

Permanent regressions in `platform/synth.rs` and `desk/actions.rs` cover late callbacks after
timeout, timeout during preparation, timeout after input starts, exactly-once normal dispatch,
request expiry, slow guards, session/app/protected-context changes, worker queue target retention,
macro delays, uncertain macro cutoff and completed-write continuation. Unsupported keys still fail
before queueing or opening a native input connection.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --locked -j 2`: 301 passed, three intentionally ignored.
- `cargo test -p kivori-desktop --test input_freshness --test runtime --test ipc_vocabulary --locked -j 2`:
  17 passed across the three integration suites.
- `cargo clippy -p kivori-desktop --lib --tests --locked -j 2 -- -D warnings`: passed.
- An isolated Windows-target Clippy check of the production synth, shortcut and copied production
  guard/error/trait definitions against Enigo's real Windows backend passed with warnings denied.
- Rust formatting, documentation formatting and `git diff --check`: passed.

Independent review found and verified the correction for uncertain macro continuation. Full CI,
including Windows Rust and startup, passed for commit `e1c08e8`
([Windows Rust job](https://github.com/cunilab/Kivori/actions/runs/40083766099/job/120842418793)). Native Windows/macOS delivery,
permission prompts and real scheduling require runtime acceptance; OS focus changes after the last
check cannot be made atomic with global keyboard injection.

## M2.9 installation and flashing software acceptance

Implemented the third S1 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
These checks use a stand-in device thread and command receiver; they launch no installer or flasher.

An installation admission lock serializes the final firmware-busy check and installation reservation
with every flash/restore reservation and queue send. Download and signature verification still
precede reservation. Installation waits for firmware preparation, programming and reconnect to end,
and for an idle knob gesture. Once admitted, an RAII guard refuses new firmware work until the
plugin returns an error or exits/restarts the process. The updater exit hook requires installation
ownership before stopping the runtime. Device-thread startup no longer overwrites the shared
initial status, which could erase an early queued `Preparing` reservation.

Six deterministic regressions in `runtime/state.rs` cover both admission orders on separate threads,
all three flash/restore paths, all busy firmware phases, failed installer scope releasing admission,
failed firmware queue rolling back status, and updater exit refusing to cancel an admitted flash.
They also prove that reserved installer shutdown keeps firmware admission closed.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --test firmware_flash --test updater_policy --test firmware_offer --test runtime --locked -j 2`:
  307 library tests and 72 integration tests passed; three library tests intentionally ignored.
- Native Clippy with warnings denied, Rust/document formatting and `git diff --check`: passed.
- Independent review passed for admission, lock ordering, production callers and guard lifetime.

The locked updater plugin invokes its exit hook before launching the Windows installer. If that
launch fails after shutdown, the reservation is released, but the device runtime stays stopped;
restart the app to resume device use. This existing lifecycle limitation is not runtime recovery
evidence. Real Windows installer launch, process exit and simultaneous hardware flashing remain
part of M2.9 S4 acceptance.

Full CI, including Windows Rust and startup, passed for installation-exclusion commit `0cc1f09`
([Windows Rust job](https://github.com/cunilab/Kivori/actions/runs/40086016590/job/120849005963)).

## M2.9 buffered button software acceptance

Implemented the first S2 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
Tests use timestamped fake input snapshots through the production firmware runtime and wire
decoder; no GPIO, interrupt timing or physical switch was exercised.

`ButtonGesture::update` settles the previous raw interval before replacing its level/timestamp.
The runtime polls every button at each captured sample before checking control ownership, clearing
a released control before admitting another. A session boundary also makes queued button downs
captured at or before that boundary ineligible for Press/Hold, including an old down whose release
comes in the new session. Equal-ms downs are suppressed conservatively. Signed wrapping arithmetic
handles rollover; the boundary check ends once the chronological drain reaches current levels,
while per-press eligibility stays latched. Recovery is still independent of session eligibility.

Four new gesture tests first failed on the original code, then passed: a complete 100–200 ms press
polled at 230 ms, two 420 ms presses separated by a 30 ms release, captured contact bounce, and
eligibility retained from the raw down. Eight new runtime regressions cover every switch, negotiated
double press, cross-control release/ownership, recovery cancellation, debouncing across a session
change, wholly unconsumed old/equal-time presses, old-down recovery, and uptime wrap. The unconsumed
old-session regression also failed before the capture-time guard was added.

Local validation on Andress-MacBook-Air:

- Full firmware host simulation: 202 passed (49 library and 153 integration tests), no failures or
  ignored tests, using `cargo test --manifest-path firmware/pala/Cargo.toml --locked --features host-sim --target aarch64-apple-darwin`.
- Host-sim Clippy for all targets and RISC-V `physical-st7789` library Clippy, both with warnings
  denied: passed.
- Shipping `physical-st7789` release firmware compiled and linked for `riscv32imc-unknown-none-elf`.
  A consistent rustup toolchain and its LLVM library search path were required locally; Cargo
  reported an optional debug-stripping warning from the earlier mixed-toolchain attempt.
- Independent review passed; the reviewer separately reran runtime and reconnect/wrap regressions.

Physical contact bounce, interrupt capture and sustained board behavior remain M2.9 S4 hardware
acceptance. Existing limits outside this change include capture-queue overflow and a complete
ten-second hold/release without any intervening poll: recovery may miss reboot during such a loop
stall. Continuous-held recovery and captured release-before-deadline cancellation are tested.

## M2.9 receive burst software acceptance

Implemented the second S2 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
These checks use fake transports and the production dispatcher/runtime; no physical USB or board
was exercised.

The original dispatcher read a complete available burst before parsing, erasing buffered valid
packets on overflow. The permanent 100-Ping regression reproduced exactly 41 replies on that code.
The replacement decodes at each delimiter and answers all 100 once, in order, across multiple
ticks. Processing stops after 512 inbound bytes per tick, retaining a partial packet and leaving
unread bytes in the transport. Storage needs only one packet, reducing `RX_CAPACITY` from 1,060
to 530 bytes; the temporary packet stays within its previous capacity, without allocation.

Oversized delimiterless packets are discarded through their delimiter, never interpreting a valid
looking tail as a new frame. One framing rejection is recorded per overflow. Empty delimiters are
ignored; malformed complete frames still use the existing typed decoder. Link loss clears partial
and discard state. Sequence suppression and existing latest-value pending-message policies remain
in force.

Five new regressions cover the 100-frame burst, partial frames spanning the tick budget with
one-/seven-/128-byte reads, overflow with a valid-looking tail followed by malformed and valid
frames, link reset from partial/discard state, and endless valid or delimiterless traffic. The last
test drives the actual runtime through ten seconds of recovery, asserting every tick samples input,
renders on cadence and requests reboot on time even though reads never return empty.

Local validation on Andress-MacBook-Air:

- Full firmware host simulation: 207 passed (49 library and 158 integration tests), no failures or
  ignored tests, using `cargo test --manifest-path firmware/pala/Cargo.toml --locked --features host-sim --target aarch64-apple-darwin`.
- Host all-target and RISC-V `physical-st7789` library Clippy with warnings denied: passed.
- Shipping `physical-st7789` release firmware compiled and linked for RISC-V using the consistent
  rustup/LLVM environment described above; the existing local optional debug-strip warning remains.
- Independent review passed, including a separate 35-test dispatcher/runtime run and a 16-test
  dispatcher rerun after reducing the buffer capacity.
- Rust/document formatting and `git diff --check`: passed.

Physical USB buffering/backpressure, receive throughput, and stack high-water remain hardware
acceptance. The byte budget bounds receive work, rather than promising a measured wall-clock
latency. Traffic beyond transport capacity can still be dropped below this dispatcher.

## M2.9 configuration capacity software acceptance

Implemented the third S2 item on `feat/m2.9-reliability` for PR #127, 2026-10-24.
Checks use temporary directories and the production configuration store; no existing user settings
were changed.

The shared atomic writer checks the complete serialized UTF-8 byte length, including indentation,
JSON escaping and the final newline, against the loader's existing 262,144-byte limit. It rejects
oversized settings with `ConfigError::TooLarge` and a fixed capacity message before creating a
directory or touching the temporary/current file. Save and reset propagate that error instead of
converting it to a generic I/O failure. Cache publication and revision changes still follow a
successful atomic write. Migration write-back uses the same size gate and retains its existing
retry-on-next-open behavior when writing fails.

The oversized-macro regression first failed on the old code because save returned success. Three
new regressions cover oversized ASCII, UTF-8 and JSON-escaped launch targets; preservation of the
existing file, temporary-file sentinel, cached configuration and revision; accepted files at
262,143 and 262,144 bytes reopening unchanged; and rejection at 262,145 bytes without creating
the settings directory. The exact-size cases include the trailing newline, catching an off-by-one
serialization check.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --test config_store --test config_persistence --test runtime --locked -j 2`:
  307 library and 52 integration tests passed; three library tests intentionally ignored.
- Native Clippy for the desktop library and tests with warnings denied: passed.
- Independent review passed; the reviewer separately ran the 43 configuration tests.
- Rust/document formatting, local documentation links and `git diff --check`: passed.

Windows filesystem behavior and the live UI presentation of the error remain untested locally.
The following acceptance section covers preservation when recovery backups fail.


## M2.9 recovery backup software acceptance

Implemented the fourth S2 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
Tests use temporary directories, injected backup operations and a synthetic migration result;
no existing user settings were changed. The released schema still has no real migration steps,
so the private loader seam exercises the production migration branch without changing versions.

The store retains a pending preservation reason when quarantine or migration backup fails.
Defaults remain usable for corrupt/newer files; converted valid settings remain usable in memory.
Every save/reset retries preservation before replacing the original, and refuses the change while
that retry fails. Candidate validation and serialized size checks precede save-side backup work.
A successful backup clears the gate, while active settings and revision still change only after
an atomic write succeeds.

Quarantine first tries rename, then copies into a fresh file, preserves source permissions and
flushes the copy before removing the source. Failed copies attempt partial-output cleanup and do
not authorize replacement. Migration backup collisions get fresh names, preserving earlier
originals. Cleanup runs only after quarantine succeeds and explicitly protects the new original,
even when existing backup timestamps are in the future. `backupFailed` travels through native
DTOs, parsing, the Controls notice and diagnostics; startup logs failure rather than recovery.
The notice no longer claims a backup exists when preservation failed.

Seven new native regressions cover failed move/copy, save/reset refusal and subsequent successful
preservation; migration failure with usable converted settings; preservation of prior migration
backups; failed write-back after a successful backup; fallback copy; failed partial copy cleanup;
clock-skew-safe pruning; and size rejection before pending preservation. Initial injected
PermissionDenied failures keep the original at its path. Public save/reset retry failures use a
source parked elsewhere inside the test directory, then restore it for a successful real retry;
these are deterministic guard checks, rather than real OS permission/antivirus tests.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --test config_store --test config_persistence --test runtime --test ipc_vocabulary --locked -j 2`:
  314 library and 53 integration tests passed; three library tests intentionally ignored.
- Targeted frontend IPC, Controls and legacy-settings tests: 110 passed, including the new notice.
- Native Clippy with warnings denied, targeted frontend ESLint, desktop TypeScript, Rust/document
  formatting, local documentation links and `git diff --check`: passed.
- Independent review passed and separately reran all eight store unit tests.

The prior size-limit commit `dddf9fc` passed full CI, including Windows Rust, packaging and startup
([Windows Rust job](https://github.com/cunilab/Kivori/actions/runs/40113384294/job/120929570116)).
Real Windows permission/antivirus failures, visual UI inspection and power-loss durability remain
unverified. File-copy synchronization is tested through software outcomes; this is not a power-cut
proof for filesystem directory updates.


## M2.9 shortcut execution software acceptance

Implemented the fifth S2 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
The installed Enigo 0.6.1 source exposes and maps F21–F24 to Windows virtual keys, but its
macOS key enum and Quartz mapping end at F20. The Windows sender now maps the missing four
keys. The shared parser accepts F1–F24 on Windows and F1–F20 elsewhere. Bindings, knob pairs,
macro steps and test requests already use this parser, so unsupported keys fail native validation
before save/dispatch; no separate frontend key parser was added.

The regression first failed on the old macOS code with “unsupported F21 was accepted.” Parser
and sender tests cover F0 through F255, checking every accepted key against its exact Enigo
variant, canonical text, and rejection outside the platform range. A store regression exercises
F0 through F25: supported shortcuts save and reopen unchanged; unsupported shortcuts preserve
the previous file bytes, live configuration and revision. Tests do not connect Enigo or inject keys.
A Windows configuration containing F21–F24 is not portable to macOS: on open it follows the
existing invalid-config preservation/recovery path rather than retaining a non-runnable binding.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --test config_store --test config_persistence --test runtime --test ipc_vocabulary --locked -j 2`:
  316 library and 54 integration tests passed; three library tests intentionally ignored.
- Native desktop library/tests Clippy with warnings denied: passed.
- Isolated Windows-target Clippy compiled the production shortcut parser, sender and their tests
  against Enigo's real Windows backend with warnings denied. This is compilation, not Windows
  test execution; the full desktop cross-build still requires Windows SDK headers unavailable here.
- Independent review passed and separately ran all three new regressions without native input.
- Rust formatting, local documentation links and `git diff --check`: passed.

Actual key delivery, OS permission handling and receiving-application behavior were not exercised.
Windows runtime coverage remains a CI/platform check; Linux input remains unimplemented.
