# Hardware validation

The one checklist for everything CI cannot prove: real ESP32-C3 + ST7789 + HW-040 hardware, real
OS behavior, real timing. Host-sim and other host tests are never promoted to physical evidence.

Current M2.9 scope, updated 2026-10-26: CAD/enclosure work is owner-managed and outside the
software milestone. The [roadmap](./roadmap.md#m29-reliability-truthful-feedback-and-beta-readiness)
now has **13/18 complete**, with five configuration or real-use acceptance items open. Earlier
13/19 counts are dated historical snapshots; the enclosure records below remain reference material
for the owner and do not count toward current M2.9 completion.

Row numbers: `1.N` is row N of the old Phase 1 (device connection) checklist, `3.N` is row N of the old
rotary checklist, `2.N` are new Phase 2 rows, `G` rows are PRD gates. Tick a row only with a date and a
measurement or observation on the same line. A phase closes only when all its rows are ticked.

## How to run a session

**You need:** the board, a Windows PC and a Mac, one unrelated USB-serial device (Arduino or UART
adapter), and a phone that films slow motion (backup for latency only).

Current wiring specification (source of truth: `firmware/pala/src/profile.rs`). The GPIO map moved
for the enclosure; earlier display measurements do not verify this map. Check continuity against
the current profile and complete rows 3.15 and 5.1 before powering/flashing an assembled unit.

| Signal | GPIO | Signal | GPIO |
|---|---:|---|---:|
| SPI SCK (SPI2, 20 MHz, Mode 3) | 4 | Encoder CLK | 7 |
| SPI MOSI | 3 | Encoder DT | 6 |
| Display D/C | 1 | Encoder SW | 5 |
| Display RST | 2 | USB D- / D+ | 18 / 19 |
| Backlight (active-high) | 0 | Encoder COM / VCC | GND / **3V3** (never 5 V) |
| Button left / middle / right | 9 / 8 / 10 | Button other leg | GND (no resistor) |

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

Suggested order: wiring/crystal checks (3.15 and 5.1) and flash, then Phase 1 (Windows, then macOS), Phase 2, Phase 3, latency,
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
- [x] 1.24 Record the real SPI pin map | 2026-08-11 historical unit: SCK 6, MOSI 7, no CS, D/C 2, RST 3, backlight 8 active-high; SPI2 20 MHz Mode 3. This predates the current enclosure GPIO map and does not validate it
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

The unsigned development check in 10.1 does not close M2.9's signed-beta requirement. Updater `.sig`
files and Windows code signing are separate; use the
[M2.9 installation/recovery record](#m29-windows-installation-and-recovery-acceptance-open).

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


## M2.9 onboarding recovery software acceptance

Implemented the first S3 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
Plug in now embeds the existing Device firmware component in recovery-only mode instead of
keeping a local restoring flag after command enqueueing. The component stays mounted while
its card is hidden, so its existing 750 ms native polling observes preparation, flashing,
reconnection, success, failure and product/board choices. Reopening setup reads native progress.
Normal setup hides flash controls; Continue still requires a known usable session.

Disconnected terminal failures retain BOOT retry. Ambiguous targets use the existing catalog
choices and a fresh confirmation naming both product and board; unplugging for BOOT does not
discard the choice, while a changed native choice list does. Pending requests cannot queue a
second restore. A usable reconnect clears inactive recovery and request errors, including an
older request rejecting after the reconnect. Device-page behavior keeps its default presentation.

The seven initial recovery regressions failed against the old onboarding flow. The final 18
regressions cover native progress through success, every failure token, disconnected retry,
product/board selection and cancel, stale choices, reopening setup, duplicate submission,
status polling failure/recovery, normal setup and both request-error/reconnect orderings.
Independent review identified the missing disconnected-request retry and stale-error transitions;
these were fixed and covered before acceptance.

Local validation on Andress-MacBook-Air:

- `bun run --cwd apps/desktop test src/features/onboarding/__tests__/recovery.test.tsx src/features/onboarding/__tests__/onboarding.test.tsx src/features/onboarding/__tests__/capabilities.test.tsx src/features/device/__tests__/firmware.test.tsx src/features/device/__tests__/recovery.test.tsx`:
  65 tests passed across five files, including all 18 new recovery cases.
- Desktop TypeScript, targeted ESLint with warnings denied, frontend Prettier, local documentation
  links and `git diff --check`: passed.
- Independent review passed and separately reran all 18 recovery tests.
- Local browser smoke at 1200×767 with the incompatible-device mock: the recovery card and BOOT
  instructions rendered, Continue remained disabled, and confirmation produced the expected
  native-runtime-unavailable error with retry still available. The dialog screenshot was visually
  inspected; no device command reached hardware. Browser artifacts remain local under `/tmp`.

Previous commit `39f27df` passed all required CI, including Windows Rust, packaging and startup;
the macOS CI job was skipped. Native/firmware source was unchanged in this increment, so the
previous 316-library/54-integration native baseline was not rerun locally. Actual Windows USB
recovery and small-window/large-text layout acceptance remain open in later roadmap items.


## M2.9 control-test correlation software acceptance

Implemented the second S3 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
`test_action` and `test_rotate` now return the existing `DeskActionDto` for their own invocation
after the action completes. Each owns a bounded async channel, moved through the device command
and worker request. The worker sends the exact outcome independently of global `Finished` events
and the replaceable feedback ladder. Protected refusal replies immediately; rejected enqueue,
worker shutdown and skipped requests drop the sender and reject the waiting command. Awaiting the
reply keeps the main thread available for macOS input dispatch and holds no configuration lock.
Global desk status, feedback and activity reporting remain in use for general device activity.

Controls displays Processing until that invocation resolves. It replaces stale success with an
error on rejection, ignores global `lastAction` changes, and discards replies superseded by a new
test/countdown, configuration/profile change or unmount. Countdown cancellation still sends no
request. The IPC bridge validates each reply using the same action parser as desk status; browser
mocks return their own outcome while retaining global desk updates. Device Studio still accepts
the same parameters and can ignore the returned result.

Native regressions interleave two test replies with a separate hardware-style worker request and
verify exact outcomes and permission fields. They also prove queued session cancellation runs no
stale action, unavailable workers close replies, protected refusal replies immediately, and an
expired request closes its reply. UI/IPC regressions cover old/global results while pending,
unrelated later actions, rejected and superseded tests, new countdowns, changed bindings/profiles,
unmount and malformed reply payloads.

Local validation on Andress-MacBook-Air:

- `cargo test -p kivori-desktop --lib --test config_store --test config_persistence --test runtime --test ipc_vocabulary --test firmware_flash --locked -j 2`:
  319 library and 79 integration tests passed; three library tests intentionally ignored.
- Native desktop library/tests Clippy with warnings denied: passed.
- `bun run --cwd apps/desktop test src/features/controls/__tests__/controls.test.tsx src/lib/ipc/__tests__/ipc.test.ts src/features/device-studio/__tests__/test-action.test.tsx`:
  119 frontend tests passed across three files. TypeScript, targeted ESLint, frontend Prettier,
  Rust formatting, local documentation links and `git diff --check`: passed.
- Independent review passed and separately ran five native correlation/protection/cancellation
  checks and 115 Controls/IPC tests.
- Browser mock smoke at 1200×767: a Press test stayed Unverified after an unrelated mock mute
  changed the global result to State Confirmed. The result was asserted in the browser and its
  screenshot visually inspected. Browser evidence remains local under `/tmp`.

No real native input or hardware actions were exercised. There is no new test timeout: an OS
backend that blocks can leave tests pending. Epoch cancellation closes queued replies when the
worker dequeues them, so cancellation is not prompt behind a blocked backend. Pending is retained
rather than borrowing another action's result or claiming completion from a timer.


## M2.9 small-window dialog software acceptance

Implemented the third S3 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
The clipping concern is now reproduced: the app-update dialog at 360×420 extended from −1.5 to
421.5 px at default text size. Enlarging the root font from 16 to 32 px (200%) made it 787 px high,
extending from −183.5 to 603.5 px, with no vertical scroll container.

Shared `AlertDialogContent` now caps height at `calc(100dvh - 2rem)` and scrolls vertically.
Base UI still owns focus trapping and dismissal, and footer actions stay in normal scroll flow.
App-update release notes gain a keyboard tab stop and cap their own pane at the smaller of 12 rem
and half the viewport height, so the nested pane also fits at enlarged text sizes. Notes remain
plain text. The implementation changes two component files, with no new resize handlers or
layout helpers.

Local browser checks on Andress-MacBook-Air exercised the real app's app-update, required-firmware
and BOOT-recovery dialogs at 400×480 and the configured 360×420 minimum. Each ran with 16, 24 and
32 px root fonts (100%, 150%, 200%), for 18 combinations. App-update notes were temporarily replaced
in the browser DOM with 40 copies of the existing mock notes to exercise long content; no shipping
mock or production data changed. Firmware/recovery checks used their actual instructions.

| Dialog | Height at 400×480: 100% / 150% / 200% | Height at 360×420: 100% / 150% / 200% |
| --- | --- | --- |
| App update, long notes | 448 / 432 / 416 px | 388 / 372 / 356 px |
| Required firmware offer | 207 / 376 / 416 px | 207 / 372 / 356 px |
| BOOT recovery | 387 / 432 / 416 px | 387 / 372 / 356 px |

Every measured popup stayed inside the viewport with no horizontal overflow. Tab cycling stayed
inside the popup and scrolled every enabled action fully into view. The release-notes pane could
receive keyboard focus and scroll with End; repeated Page Up returned recovery instructions to
the top. Measurements waited for CSS transitions/focus restoration and allowed a one-pixel scroll
rounding tolerance. Screenshots were captured for every combination; the 200% screenshots at both
sizes were visually inspected, including recovery instructions and footer actions. Artifacts remain
local under `/tmp/kivori-m29-dialog-browser-4a4b63f`. The Browser skill reported no connected browser;
automated Chromium supplied these local checks instead.

Local validation:

- `bun run --cwd apps/desktop test src/features/updates/__tests__/updates.test.tsx src/features/updates/__tests__/firmware-offer.test.tsx src/features/device/__tests__/firmware.test.tsx src/features/device/__tests__/recovery.test.tsx src/features/onboarding/__tests__/recovery.test.tsx`:
  71 tests passed across five files.
- Desktop and shared UI TypeScript, targeted ESLint with warnings denied, frontend Prettier,
  local documentation links and `git diff --check`: passed.
- Independent review passed and separately ran all 26 update/firmware-offer tests.
- Previous commit `4a4b63f` passed all required CI, including Windows Rust, packaging and startup;
  the macOS job was skipped. Native and firmware code were unchanged in this increment.

These are browser viewport/root-font checks, not native Windows WebView2, OS display-scaling,
screen-reader or full browser-zoom verification. No installer, firmware flash or hardware action
was run. Native platform and unfamiliar-user acceptance remain in S4.


## M2.9 signup attribution software acceptance

Implemented the fourth S3 item on `feat/m2.9-reliability` for PR #127, 2026-10-25.
Oversized optional UTM/referrer values no longer reject an otherwise valid signup. One shared
normalizer trims string values, retains those within the existing limits (100 for UTM fields,
500 for referrer), and discards oversized/non-string values. Discarding rather than slicing keeps
partial URLs, campaign names or split Unicode surrogate pairs out of storage. Both the browser
form and server validator use the helper; the server remains authoritative for direct requests.
Discarded values are empty hidden inputs, then NULL in storage; discarded source becomes an empty
analytics source. Email, product, use case, token and honeypot validation remain unchanged, as do
Turnstile, rate limiting, parameterized inserts and duplicate-signup behavior.

The nine attribution regressions first failed on the old validator, including a valid signup
with a 101-character campaign. The final 19 new cases cover every attribution field below/at/above
its limit, trimming, empty and non-string data, whole Unicode at the boundary, successful storage,
analytics, and continued rejection of invalid user/security fields. Handler tests verify that
oversized attribution still runs Turnstile and rate limiting, with no row on either failure.

Local validation on Andress-MacBook-Air:

- `bun run --cwd apps/web test`: 104 website tests passed across 14 files; 42 are waitlist tests.
- Website TypeScript (`next typegen && tsc --noEmit`), targeted ESLint with warnings denied,
  frontend Prettier, local documentation links and `git diff --check`: passed.
- Independent review passed and separately reran all 42 waitlist tests.
- Browser checks used the real homepage form with locally stubbed Turnstile and signup responses.
  Source/medium/campaign/referrer lengths of 101/100/101/501 submitted as 0/100/0/0; values within
  the limit were retained, and surrounding whitespace was trimmed. Both submissions preserved
  email, selected product and token, and displayed success. Server storage/security outcomes were
  verified separately with the injected D1 stand-in rather than the browser stub.

No real signup, deployment, production D1 write or live Turnstile verification was performed.
Browser evidence remains local under `/tmp/kivori-m29-attribution-browser-c304f63`. The initial browser
attempt on 127.0.0.1 timed out; the checks passed on the server’s advertised `localhost` origin
without changing dev security configuration. Next dev generated an untracked AGENTS.md, moved to the local evidence directory
after shutdown. Native/firmware code was unchanged. Real public widget/secret/contact configuration,
a signup and a deletion request remain the next roadmap item.

## M2.9 public website readiness audit (open)

Read-only audit on Andress-MacBook-Air, 2026-10-25, for item 13 of PR #127.
At this audit snapshot the item remained unchecked and M2.9 progress was 12/19.
Current completion status is tracked in [the roadmap](./roadmap.md#m29-reliability-truthful-feedback-and-beta-readiness).

| Check | Observed result |
| --- | --- |
| Committed configuration | `apps/web/wrangler.jsonc` uses the always-pass test site key and an empty `CONTACT_EMAIL`. |
| Active production version | Wrangler deployment/version inspection reports version `cdd9f9bb-99c3-4e63-977a-1a22b9f76529`, deployed 2026-10-10 at 03:45:20 UTC with 100% traffic. Reported bindings are `ASSETS`, `DOWNLOADS` and `WORKER_SELF_REFERENCE`. No waitlist database, limiter, signup analytics, Turnstile/contact variables or secret binding is reported. |
| Worker secrets | `bunx wrangler secret list` returned `[]`. No secret values were retrieved. |
| D1 database | `bunx wrangler d1 info kivori-web --json` confirms the configured database exists with two tables. This metadata does not verify the waitlist schema, contents or production binding. No signup rows were queried. |
| Turnstile widget | `bunx wrangler turnstile widget list --json` failed with authentication error 10000; local OAuth reports missing `challenge-widgets.write`. Widget existence/configuration remains unverified. |
| Anonymous public access | GET requests to `https://kivori-web.andres12holivin.workers.dev/` and `/privacy` returned HTTP 403 from this environment. The root response identified Cloudflare Access, including with a browser User-Agent. Live form/privacy content could not be inspected. |
| Deployment path | `.github/workflows/web.yml` uploads PR preview versions; pushes to main migrate D1 and deploy. The feature branch is not the active production release. |

The owner must provide a public privacy mailbox and consented test address, arrange access to the
real Turnstile widget, and confirm the intended public access policy. Configuration review and an
approved production deployment must precede live verification. A successful signup must have a
verified stored row; the deletion request must reach the mailbox, and deletion must leave zero
rows for the normalized test email across all products. Follow the
[launch and deletion procedure](../apps/web/README.md#public-launch-and-deletion-verification).

Astra's independent read-only scope review agreed to keep this item open. No new production/test
mode switch was added: it would not establish the required live evidence and would change the
supported preview behavior. That initial audit increment changed documentation only. Local
documentation links and `git diff --check` were checked; runtime suites were not rerun. No deployment,
configuration or Access change, real signup, email, database write or deletion was performed in it.

### Local signup and deletion rehearsal

On 2026-10-25, the owner authorized dummy/testing/mocked approaches. Added
`apps/web/lib/waitlist.integration.test.ts` on Andress-MacBook-Air: the real Next API route uses a
disposable local D1 database initialized from the committed migration. Wrangler remote bindings,
persistence and environment-file loading are disabled. Worker context, Turnstile verification,
rate-limit decisions and analytics are mocked; addresses use `example.com`.

Seven checks passed: normalized signup/duplicate responses and deletion across products; missing
database; missing real-widget secret; rejected human check; rejected rate limit; honeypot; and
server-rendered Worker/local/missing privacy contact states. The signup/deletion check verifies
two matching product rows become zero and another address remains, oversized campaign data is
discarded, and analytics excludes the address, test IP and token. It exercises the documented
bound-parameter SQL, not an email-based deletion service. The local runtime and temporary
configuration are cleaned up after the suite.

Validation: `bun --filter kivori-web test` passed 111 tests across 15 files, including these seven.
Website typecheck, targeted ESLint/Prettier and `git diff --check` also passed. Run only this fixture
with `bun --filter kivori-web test lib/waitlist.integration.test.ts`.

Clean-checkout follow-up: CI on `8da593e` failed website build and frontend typecheck because the
fixture referenced `D1Database`, available locally only through the ignored generated
`cloudflare-env.d.ts`. Replaced that ambient dependency with a local structural type extending the
handler's existing `WaitlistDb` contract. With the generated file temporarily excluded, website
typecheck, all 111 tests and the production Next build passed; the original generated file was
restored afterward. Targeted ESLint/Prettier and diff checks also passed. No production code changed.

This adds repeatable local evidence without changing public configuration or runtime behavior.
No browser widget, actual rate-limit threshold, mailbox delivery, remote D1 or public deployment
was tested. Item 13 remains open and overall progress remains 13/19; live acceptance is separate.

## M2.9 software baseline acceptance

Completed item 14 on Andress-MacBook-Air, 2026-10-25, for PR #127. Runtime source tested was
`1899b70d4af797082ddee92c2be6e3218db257e0`; this increment only records evidence. M2.9 progress is
13/19, with public website configuration still open.

The configured Rust channel is `stable`, not an exact numeric pin; this machine resolved rustc
1.98.0 (`88d9e12ae`, LLVM 22.1.8), Cargo 1.98.0 and host `aarch64-apple-darwin`. Node was 24.14.0.
The system Bun was 1.4.0, so Bun 1.4.2 (the repository/CI pin) was obtained with `bunx bun@1.4.2`
and placed first on PATH for child scripts. Frozen-lockfile install passed without tracked changes.

| Local check | Final result |
| --- | --- |
| `cargo test --workspace --locked --no-fail-fast -j 2 -- --test-threads=1` | 887 passed, 4 intentionally ignored; includes native, shared crates, golden frames, asset compiler and host end-to-end suites. |
| `cargo test --manifest-path firmware/pala/Cargo.toml --locked --features host-sim --target aarch64-apple-darwin -j 2` | 207 passed, none ignored. |
| `bun run --cwd apps/desktop test --maxWorkers=2` | 417 passed across 36 files. |
| `bun run --cwd apps/web test` | 104 passed across 14 files. |
| Firmware recipe/catalog tests | 34 passed: `bun test scripts/build-firmware.test.mjs scripts/build-firmware-set.test.mjs scripts/check-product-identifiers.test.mjs`. |
| Release tooling | 78 passed: `node --test --test-concurrency=1 scripts/release-tools.test.mjs scripts/firmware-release-guard.test.mjs scripts/release-finalize.test.mjs`. |
| Clippy, warnings denied | Host workspace all targets, desktop all targets with `--no-default-features`, firmware host-sim all targets; all passed with locked dependencies and two Cargo jobs. |
| Formatting and frontend checks | Root/firmware `cargo fmt --all --check`, `bun run typecheck`, `lint`, `format` and `build` passed. Both frontend production builds completed. |
| Guards | `check-crate-boundaries.sh`, `check-offline-deps.sh`, `check-product-identifiers.mjs`, `check-frontend-offline.mjs`, `check-mock-excluded.mjs` and website `check:leaks` passed. |

Failed/interrupted attempts are retained rather than counted as passing runs:

- The initial concurrent run on this 8 GB Mac produced two desktop timeouts and one 5-second
  recipe-test timeout while memory was under pressure. Competing validation jobs were stopped.
  The complete desktop suite then passed with two workers and unchanged time limits; all 34 recipe
  tests passed sequentially, including the timed-out case (1.34 seconds). This supports resource
  contention as the timeout explanation, without establishing the cause of every historical timeout.
- The first workspace run failed `preview_stream::per_frame_render_fits_the_30fps_budget` at
  40.3 ms/frame against 33 ms. A focused debug rerun failed at 91.1 ms/frame. The final complete
  workspace run with one test thread passed this check and every other non-ignored test. Sol's
  independent read-only review found no changes from `origin/main` throughout the test's render,
  asset and build inputs. The local debug timing failure is intermittent; its exact cause and
  optimized performance were not established at that baseline. No threshold, renderer or test skip was changed.
- Earlier native/firmware and release-tool runs stopped during resource isolation are incomplete
  attempts, not additional failures or acceptance evidence. Logs remain local under
  `/tmp/kivori-m29-baseline-*.log`; final sequential exit codes are in
  `/tmp/kivori-m29-baseline-sequential-results.json`.

All applicable GitHub checks passed on the exact tested commit: [host Linux/Windows and startup](https://github.com/cunilab/Kivori/actions/runs/40163852422),
[firmware isolation and host simulation](https://github.com/cunilab/Kivori/actions/runs/40163852407),
[cross-platform determinism/assets](https://github.com/cunilab/Kivori/actions/runs/40163852413),
[website build/preview](https://github.com/cunilab/Kivori/actions/runs/40163852424), and
[Windows packaging and firmware release builds](https://github.com/cunilab/Kivori/actions/runs/40163852446).
Luna independently checked the exact head and final results. The Linux release-surface guard
passed every absence check and positive control; the Windows release artifact check also passed.
RISC-V adapter/feature builds were verified by CI, not repeated locally. Optional macOS release
packaging was skipped. These are observed workflow gates; branch-protection enforcement was not
established (`main` reported unprotected).

The four ignored native tests inspect live apps, foreground state and microphones, or change real
volume/mute. They were not enabled. During that baseline, no physical input, USB, installer execution,
flashing, optimized performance benchmark or live signup/deletion was tested locally. This software baseline does not
close the website, Windows hardware, recovery, sustained-use or unfamiliar-user acceptance rows.

### Optimized preview budget follow-up

On 2026-10-26, Andress-MacBook-Air ran the existing test without changing the renderer or threshold:

```sh
CARGO_BUILD_JOBS=2 cargo test --release -p kivori-desktop --test preview_stream per_frame_render_fits_the_30fps_budget -- --exact --test-threads=1
```

The optimized Device Studio preview test passed. The resulting test binary then passed all eleven
preview-stream tests and a further focused budget run: three observed budget-test passes in total.
Each budget check warms assets, renders thirty frames and asserts average frame time below the
existing 33 ms limit. The test does not print the measured per-frame value; its 0.03 s focused-suite
duration is not a per-frame measurement.

The cold release build took 51 minutes with two Cargo jobs. This 8 GB Mac showed approximately
13.5 GiB of swap in use during compilation; the build cache remains local to this device.
The result establishes the tested optimized path meeting its local budget, not the cause of the
earlier intermittent debug failures. Windows/WebView timing, IPC/display overhead, physical device
latency and sustained-use acceptance were not measured. No new test, skip, threshold change or
production code change was needed. Documentation diff/checklist checks passed; M2.9 remains 13/19.

## M2.9 Windows unit acceptance (open)

Item 15 preparation, 2026-10-25, Andress-MacBook-Air. The owner deferred the physical run to a later
retry. This session runs on macOS; no Windows host with a connected unit was used. No physical checks below have run.
Item 15 stays unchecked and progress remains 13/19. Windows CI startup/packaging and host simulation
are software evidence only. The existing [provisioning tool](./provisioning.md) flashes firmware and
does not cover double-press, microphone hotplug, latency or sleep/lock behavior; do not run it as a
read-only substitute for this session.

Preparation checks: the wiring table matches the nine corresponding firmware declarations,
the new roadmap link resolves, the checklist remains 13/19, and `git diff --check` passes.
These are documentation/source-consistency checks; runtime suites were not rerun for this edit.

Before testing, record: UTC date/operator, source SHA and dirty status, app version/build type,
firmware version/product/board and image SHA-256, short unit label, Windows version/build, USB
port/hub/power arrangement, microphone types/count, selected profiles/bindings and negotiated
capabilities. Use the matching app and firmware from this PR; older historical row results do not
validate the reliability changes. Verify the current wiring table above against the physical unit.

Use a temporary test profile and a scratch document or non-sensitive playback to count actions.
Record each row as PASS/FAIL/NOT RUN with actual counts/times and a local evidence reference.
NOT RUN, missing measurements and unsupported machine power modes cannot become PASS.

| Case / existing rows | Procedure and required evidence | Result |
| --- | --- | --- |
| Connection / 1.6, 1.15–1.17, 1.20 | Time cold connect (<5 s), disconnect indication and reconnect with state restore (<10 s). Repeat rapid unplug/replug 10 times; record each duration and any duplicate/stuck session. | NOT RUN |
| Rotary / 3.1–3.13, G1/G2 | Count slow detents both ways, fast turns, edge wiggles, 0/100 reversal and default-output change mid-turn. Compare Windows mixer read-back with the panel; no phantom/reversed/stale action or invented confirmation. | NOT RUN |
| Latency / 3.14, 4.28 | After wiring checks and an agreed firmware-flash step, use the dev-only latency image. Capture about 30 distinct detents in both directions, `L` samples and final `H` (<50 ms), plus `F/t/o/n` counters. Note the device view and dropped/invalid input. Restore the matching product image afterwards and record its version/hash. | NOT RUN |
| Press/hold/double / 4.1–4.2, 4.24–4.25, 5.15 | On each supported control count 10 short presses and 5 roughly 1 s holds; on the double-press-capable knob count 10 natural double presses. Use recorded bindings; each gesture has exactly its intended action, no extra press on hold/double, no merged distinct presses. Note the 250 ms single-press delay and any lost/duplicate input. | NOT RUN |
| Microphone / 13.1–13.5 | With built-in and USB mics active, compare every endpoint's actual mute state in Windows before/after Mic Mute. Hotplug a third mic while muted, then unmute externally and unplug/replug an endpoint. Record time to reconcile and per-endpoint result; incomplete/failed coverage must remain unknown/unverified, never a muted or confirmed claim. App-call mute is distinct from OS endpoint mute. | NOT RUN |
| Disconnect during input / 3.12, 4.8, G2 | Disconnect during a turn and during a queued scratch-document shortcut/macro; press during the outage, then reconnect. Count output before/after; no pre-disconnect or outage input replays. Record actual delivery and feedback separately: keyboard delivery remains Unverified. | NOT RUN |
| Context changes / G2, 5.18 | Queue a scratch-document action, switch focus/profile before dispatch or during a macro wait, and cancel a pending Controls Test. The remaining old-target input must not execute or retarget; another action's result must not become that test's result. Record which input had already completed before cancellation. | NOT RUN |
| Lock/unlock / 7.1–7.2 | Repeat 3 cycles, with attempted shortcut input while locked. Record sleeping indication (<1 s), restored profile/state, allowed volume behavior and no old shortcut replay after unlock. | NOT RUN |
| Sleep/wake / 7.3–7.7 | Record `powercfg /a`, then exercise each available sleep/resume mode (3 cycles for the normal mode). Record sleeping/offline indication, USB power behavior, reconnect (<5 s) and absence of old press/turn replay. Record unsupported modes explicitly; required coverage stays open. | NOT RUN |
| Feedback / G1/G2, 4.16, 13.12 | Across all cases compare Activity, panel and independent Windows state. A failed/unreadable endpoint never confirms; shortcuts/media remain Unverified. Record any stale overlay or result separately from whether the OS action happened. | NOT RUN |

The latency probe records the device timestamp of a successfully transmitted detent and measures
to completed tile flush after a new Presentation, not physical-contact-to-visible-pixel time.
It pairs by order rather than a unique per-detent response ID: use isolated detents in a stable
Volume view without other control actions. It tracks the oldest unmatched detent per burst;
unanswered samples can expire after 1 s and displayed values clamp at 999 ms. Record missed
responses separately rather than interpreting a low `H` as proof none occurred. Slow-motion video
can supplement physical-to-visible timing; retain frame rate, frame counts and method separately.

Acceptance requires measured results for all cases, dated updates to the affected existing rows,
and reproduction/fix/retest evidence for failures. Run on the product image except the explicitly
identified latency pass; do not leave a probe image installed. This does not close item 16's clean
installation/interrupted-flash recovery, item 17's soak/workdays, or unfamiliar-user/enclosure checks.

## M2.9 Windows installation and recovery acceptance (open)

Item 16 preparation, 2026-10-25, Andress-MacBook-Air. No clean Windows host or physical unit was used;
no install, login, app update, firmware flash or interruption was performed. Progress remains 13/19.
Initial documentation-only preparation validation: new relative links/internal anchor, eight NOT RUN
cases, the 13/19 checklist and `git diff --check` passed. Runtime suites were not rerun for that increment.

### Local development packaging prerequisite

The local `just bundle-windows` recipe requested updater artifacts even when no updater private key
was supplied. It now disables those artifacts for an unset or empty `TAURI_SIGNING_PRIVATE_KEY`,
matching PR CI. A nonempty key retains the signing configuration and normal validation; a signing
failure does not retry unsigned. This does not implement Windows publisher signing or close item 16.

Validation on 2026-10-25, Andress-MacBook-Air: the regression failed against the previous recipe;
the firmware recipe/catalog suites then passed 35 tests with Bun 1.4.2. A mocked execution of the
actual Bash recipe checks unset, empty and supplied keys, child failure propagation and temporary
firmware cleanup. Independent Sol review passed and reran the focused test successfully.
`just --show bundle-windows`, targeted ESLint/Prettier and `git diff --check` passed. Local logs:
`/tmp/kivori-m29-local-bundle-red.log` and `/tmp/kivori-m29-local-bundle-tests.log`.
The Bash fixture is skipped on Windows; no actual Windows package, key validation, installer or
updater was executed. The live acceptance rows below remain NOT RUN.

### Simulated interrupted flash and recovery

On 2026-10-25, added a continuous scenario in `tests/firmware_flash.rs` using the production
`FlashWorkflow` and `run_accepted_firmware_flash`, with injected flasher outcomes. For each of
Cancelled, Timeout, PortBusy and Unknown, failure returns to discovery, releases busy state,
rejects a late handshake and emits failure without a success event. A retry reserves a new port
(COM8 after COM7), clears the prior failure and refuses a second reservation. Handshakes before
flashing, on the old port or with incompatible firmware cannot verify recovery. Only a compatible
handshake on the reserved recovery port completes it, records the recovered identity and emits
one verification event; native details stay out of activity metadata.

Validation on Andress-MacBook-Air:
`cargo test -p kivori-desktop --test firmware_flash --test recovery_target --test updater_policy -- --test-threads=1`
passed 58 tests (26 flash, 9 target policy, 23 updater policy). Rust formatting, targeted Clippy with
warnings denied and diff checks passed. The test injects failure classes directly; it does not
run an espflash child, exercise USB/BOOT, validate an installer signature or perform an app update.
No production code changed. Item 16 and all live acceptance rows remain open; progress stays 13/19.

### Artifact prerequisites

- Owner decision, 2026-10-26: no Windows publisher signing setup yet. Provider selection,
  credentials and integration are deferred; development testing may continue with unsigned builds.
  This does not disable updater signing, change repository secrets/settings, relax the signed-beta
  prerequisite or mark installation/recovery accepted.
- Windows code signing is an unimplemented placeholder in `.github/workflows/release.yml`;
  setting `WINDOWS_SIGNING=true` currently fails deliberately. Owner certificates/provider setup
  and the signing implementation remain open in [issue #36](https://github.com/cunilab/Kivori/issues/36).
  A signed-beta acceptance run needs reviewed signing integration and trusted signed artifacts first.
- Tauri updater signing is separate. The release config has a public verification key and requires
  signed updates, but the PR workflow explicitly omits the updater private key and builds without
  updater artifacts. A passing PR installer smoke cannot prove updater signature verification.
  Check signatures on actual release candidates; configured keys alone do not prove they match.
- Select approved app versions A and newer B with recorded source SHAs, installer hashes, publisher
  identity, updater signature/manifest provenance and exact bundled firmware identities/versions.
  Use the [existing release procedure](./release.md). Publishing a release affects installed users;
  preparing this session does not publish one. A draft or PR artifact is invisible to the production
  updater endpoint. Any isolated update-endpoint test build must follow the developer procedure in
  [release.md](./release.md#verifying-an-app-update-end-to-end) and be identified as such in evidence.
- Use a clean Windows user/machine without Rust, Cargo or a separately installed `espflash`.
  Record Windows build, standard-user status, WebView2 availability/network use, USB arrangement,
  unit label and matching current wiring checks. Preserve the initial machine snapshot for retries.

On Windows, these read-only checks record artifact hashes and Windows signature status:

```powershell
$installerPath = 'C:\QA\Kivori_VERSION_x64-setup.exe' # replace with the reviewed candidate
Get-FileHash -LiteralPath $installerPath -Algorithm SHA256
Get-AuthenticodeSignature -LiteralPath $installerPath |
    Select-Object Status, StatusMessage, SignerCertificate, TimeStamperCertificate
```

Compare the hash with the approved artifact record and require a valid signature from the expected
publisher; inspect installed app/sidecar signature policy too. This command does not validate the
Tauri `.sig`. The snippet has not been executed in this macOS preparation session.

### Live session record

Record UTC/operator, unit and source/artifact metadata above, before/after app and firmware versions,
settings checksum/selected bindings, actual outcome, durations and local evidence references for
each row. Redact machine paths/usernames from shared logs. Missing prerequisites stay NOT RUN.

| Case / existing rows | Required observation | Result |
| --- | --- | --- |
| Clean install / 10.1–10.2, 10.4, 11.1–11.2 | Install signed A as a standard user; record publisher/trust prompts, WebView2 setup and Defender outcome. App starts, setup completes once, device connects, bundled sidecar works without dev tools. Unsigned development smoke is recorded separately. | NOT RUN |
| Login/tray / 9.1–9.2, 9.5–9.6 | Enable launch at login, sign out/in, verify hidden window, tray, one process and device connection. Disable it and repeat; no automatic launch. | NOT RUN |
| Settings-preserving app update / 10.3, 11.4 | Save a distinctive profile/binding and update A to B through the real updater. Observe download/install/restart, reported B version, reconnect and preserved settings; onboarding must not repeat. Record endpoint and signature-verification outcome. | NOT RUN |
| Rejected update / release update procedure | With the documented isolated test build, attempt a corrupted installer or mismatched updater signature. Installation is refused and current app/settings remain usable. Keep its endpoint/build evidence separate from production-endpoint acceptance. | NOT RUN |
| Installed firmware update / 8.1, 8.6, 8.8 | On the dev-tool-free machine, flash the exact bundled product/board image through the installed app. Observe progress, updating display, reconnect, identity and reported version. Confirm version-based offers are accurate. | NOT RUN |
| Interrupted flash and BOOT restore / 8.2, 8.7 | On the test unit, disconnect USB during a real flash. Record failure/retry feedback and actual sidecar error. Follow the app's BOOT/Restore instructions, explicitly select the matching target if needed, and verify successful reconnect/identity/version plus normal controls. Retain redacted real errors for classifier regressions. | NOT RUN |
| Install/flash exclusion / M2.9 S1 | Exercise both admission orders with an update ready: active firmware flash blocks installer start; admitted installation blocks flash/restore. Observe child processes, progress and runtime shutdown where the app exits. No flasher and installer overlap; after a failed operation the appropriate retry becomes usable. | NOT RUN |
| Uninstall / 9.3 | Uninstall and verify the app is gone and both Kivori startup registry values are removed. Record settings retention/removal behavior and actual sign-in result; preserve evidence before resetting the test machine. | NOT RUN |

The current CI smoke installs/uninstalls silently and tests a ten-second `--autostart` process; it
manually inserts a Run entry to test cleanup. It does not observe a real login, manual trust prompts,
an A-to-B updater installation, a physical flash or BOOT recovery. Its success cannot fill this table.
Tick item 16 only after the artifact prerequisites and all live rows pass with evidence. Physical
input quality, soak/workdays and unfamiliar-user acceptance remain separate items.

## M2.9 sustained-use acceptance (open)

Item 17 preparation, 2026-10-25, Andress-MacBook-Air. No Windows soak or workday was started here.
Soak: NOT RUN, 0 verified hours. Workdays: 0/10. Item 17 remains unchecked; M2.9 remains 13/19.
The software baseline, CI runtime and time spent waiting for builds do not count toward either run.
Preparation checks passed for the roadmap/internal links, one unstarted soak, ten unstarted workday
rows, the 13/19 checklist and `git diff --check`. Runtime suites were not rerun for this docs-only edit.

### Candidate and evidence

Record the tested app/firmware versions, source SHA, exact artifact hashes, product/board, short unit
label, Windows build, USB cable/port/hub, power/sleep settings, audio devices and saved bindings.
Use the matching product firmware; a latency-probe image is a separate development measurement.
Perform the relevant [Windows unit checks](#m29-windows-unit-acceptance-open) before sustained use
and retain any unresolved failures. A soak does not replace installation/recovery or usability tests.

At start/end and after an incident, use Device page **Copy diagnostics** and note observed Activity
events locally. Available snapshots include connected duration/reconnects, last-pong age, RTT,
estimated device uptime, free device bytes and malformed/sequence-gap/device-error counts.
Compare deltas within the same app/device session; record every restart and counter reset.
Unknown fields stay unknown. Activity is a session-only ring of at most 256 events, with no disk
history; capture incidents before rollover/restart. A final empty history or zero reset counter is
not proof that nothing failed. Developer internals may supplement snapshots when available.
Keep evidence references and redacted observations in this record; do not commit raw private logs.

### Accelerated session-protocol check

On 2026-10-25, added a deterministic in-memory `Session` scenario in `tests/session.rs`.
It exchanges 65,552 real encoded Ping/Pong frames with injected protocol timestamps, crossing
16-bit sequence and 32-bit millisecond wraparound. Every reply keeps the same session connected,
reports the injected 17 ms round trip and leaves no pending fixture wire bytes. No sequence gaps,
malformed frames or device errors accumulate. Three unanswered pings then reach the timeout
threshold; reopening clears old RTT/uptime, advances connection generation, handshakes successfully
and resynchronizes the previous Busy desired state.

On Andress-MacBook-Air,
`cargo test -p kivori-desktop --test session --test reconnect --test runtime_connection_deadlines -- --test-threads=1`
passed 51 tests (43 session, 6 reconnect, 2 deadlines). Targeted Clippy with warnings denied,
Rust formatting and diff checks passed. No production code changed or failure was found in this
scenario. The timestamps are synthetic; this is not elapsed soak time or a memory/CPU, renderer,
Windows power-management or real USB stability measurement. The actual soak remains at zero
verified hours and workdays at 0/10; item 17 stays open and progress remains 13/19.

### Eight-hour idle soak

1. On the Windows candidate, confirm normal connection and counted control actions, then capture
   baseline diagnostics, process identity, app memory/CPU and the panel's state. Record UTC start.
2. Leave the device connected and app running for one uninterrupted 8-hour idle interval with no
   deliberate control input. Record the actual power policy; OS sleep, USB power loss, app/device
   restart or missing observation coverage breaks the interval. Screen-off alone is not OS sleep.
3. At least hourly, record connection/heartbeat health, panel state, process memory/CPU and any
   unexpected reconnect, reset or crash. Record observation times and gaps rather than assuming
   continuity. Do not infer pixel animation health solely from a connected process.
4. At the end, record UTC finish, actual elapsed time and final diagnostics; repeat counted knob,
   press/hold and relevant microphone checks and compare OS state with feedback. Record spontaneous
   actions, stale indicators, missed/wrong inputs and whether recovery was needed.

| Trial / candidate | UTC start/end and elapsed | Snapshots / observations | Incidents / recovery | Result |
| --- | --- | --- | --- | --- |
| Not started | — / 0 verified hours | None | Not observed | NOT RUN |

Passing needs a fully evidenced interval of at least 8 hours, a responsive final control check and
no unresolved stability or feedback failure. Do not add interrupted intervals together to claim a
continuous soak. Fix a blocker, retain its failed trial and repeat the affected check and full soak.

### Ten workdays of actual Windows use

Use the unit during normal Windows work on ten distinct workdays. Record actual use hours and
breaks each day; ten restarts or ten short smoke tests are not ten workdays. Exercise the controls
and profiles used in real work, including audio-device changes, ordinary lock/sleep transitions and
USB reconnects when they occur. Distinguish planned disconnects from unexplained ones. Record a
count for a defined action sample each day so a claim of no missed/wrong input has an observed basis.

| Day | Date / candidate / unit | Use start/end, active hours, breaks | Action sample expected/actual; disconnects, missed/wrong input, stale feedback, crashes | Recovery / evidence | Result |
| --- | --- | --- | --- | --- | --- |
| 1 | — | — | Not observed | — | NOT RUN |
| 2 | — | — | Not observed | — | NOT RUN |
| 3 | — | — | Not observed | — | NOT RUN |
| 4 | — | — | Not observed | — | NOT RUN |
| 5 | — | — | Not observed | — | NOT RUN |
| 6 | — | — | Not observed | — | NOT RUN |
| 7 | — | — | Not observed | — | NOT RUN |
| 8 | — | — | Not observed | — | NOT RUN |
| 9 | — | — | Not observed | — | NOT RUN |
| 10 | — | — | Not observed | — | NOT RUN |

For each incident record UTC time, candidate/unit, trigger, expected versus actual OS/device/UI
behavior, count/duration, recovery steps and evidence. False confirmation, stale replay, wrong-target
input, lost settings, crashes or failed normal recovery block acceptance. Link the issue/fix and
retest outcome; a successful restart alone does not close the defect. A behavior-changing fix creates
a new candidate: retain earlier history and review affected soak/workday coverage before counting
it toward acceptance. Documentation-only changes do not change an installed artifact's identity.
Close item 17 only after the 8-hour soak and all ten qualifying workdays have evidence and blockers
are resolved. Idle soak hours do not substitute for days of actual use, or vice versa.

## M2.9 unfamiliar-user acceptance (open)

Item 18 preparation, 2026-10-25, Andress-MacBook-Air. No participant was recruited or observed and
no physical usability session ran. Item 18 remains unchecked; M2.9 remains 13/19. Automated UI
tests, developer walkthroughs and a prepared task sheet do not count as unfamiliar-user evidence.
Initial preparation checks passed for new links/anchors, seven NOT RUN tasks, guide/source consistency,
the 13/19 checklist and `git diff --check`. Runtime suites were not rerun for that docs-only edit.

### Local walkthrough correction: profile pin instructions

On 2026-10-25, a source/UI review of the pin/Auto task found that Customize promised a second
middle-button Hold would return to automatic switching. The native `Context::cycle_pin` instead
visits the enabled profiles before returning to Auto. Updated the onboarding instruction to explain
that each further hold advances through the cycle, then returns to Auto. Controls guidance and the
user guide already describe cycling; their behavior and the pin implementation are unchanged.

On Andress-MacBook-Air, all 38 existing onboarding UI tests across three files passed with two
workers. The two native tests for built-in wrap-to-Auto and marked-custom-profile inclusion passed.
Desktop UI typecheck, targeted ESLint/Prettier and diff checks passed. No browser visual inspection,
physical gesture or unfamiliar participant session ran; this correction does not close item 18.
Progress remains 13/19.

### Session setup

Use someone who has not developed or previously learned Kivori. Record an anonymous participant
label, Windows experience, date, app/firmware/artifact and unit metadata, guide revision, machine
setup and delivered materials. Use the actual [user guide](./user-guide.md) and intended quick-start
materials; record missing packaging/instructions rather than silently supplying developer knowledge.
The signed installer and recovery prerequisites remain in
[item 16](#m29-windows-installation-and-recovery-acceptance-open).

Give the participant the goal prompts below, not the evaluator column. They may use the normal
app guidance and supplied guide. Let them choose their approach, explain expectations and ask for
help. Record requests and any assistance; do not demonstrate a gesture, point at a control or repair
a setup problem and then count the task as unassisted. A needed safety intervention ends that
unassisted attempt. Record task start/end, attempts, outcome, help and observed friction.

### Participant goals and evaluator record

| Neutral goal prompt | Evaluator observation (keep separate from participant prompts) | Result / time / help / evidence |
| --- | --- | --- |
| Get Kivori ready to use on this computer. | Install and complete setup, identify connection readiness and understand Tour guidance. Observe trust prompts, cable confusion, permission/setup errors and reliance on developer help. | NOT RUN |
| Adjust the sound and control playback, then silence the sound. | Observe knob turn, Press versus Hold discovery, actual OS state and accidental repeats/holds. Record what the participant expected before interpreting feedback. | NOT RUN |
| Give a button a second action and show both actions working. | Find the Hold binding in Controls, distinguish Press/Hold, save, test in a scratch app and confirm the physical bindings. Observe shortcut countdown/focus and whether pending feedback is understood. | NOT RUN |
| Keep these controls the same while you switch between apps, then restore automatic switching. | Discover middle-button profile pin/cycle and return to Auto. Record confusion about active, edited and pinned profiles or the on-device labels. | NOT RUN |
| Show another view on the device, then return to your starting view. | Discover double press and distinguish it from two single actions. Count unintended playback/actions and repeated presses caused by uncertainty. | NOT RUN |
| Tell me what happened after these actions and what you would do next. | Use real successful, Unverified and benign failure cases prepared for this candidate. Ask what labels, pulses, badges and Activity mean; a pulse or queued request must not be mistaken for proof the OS action completed. | NOT RUN |
| Get this disconnected or recoverable test unit working again. | Observe discovery of Device/setup recovery and following BOOT/target-confirmation steps. Use an agreed recoverable unit state; record its setup, actual flash/reconnect result and any coaching. A screenshot or simulated failure is labelled separately and cannot prove physical recovery. | NOT RUN |

After the first attempts, ask neutrally whether responses matched expectations and what felt slow
or unclear. Record the single-press delay experience, spontaneous extra presses, accidental double
presses, Hold discoverability, pin/Auto comprehension, and whether labels/results explained the
observed outcome. Then discuss the roughly 250 ms single-press wait; do not teach its cause before
the first attempt. Participant impressions are usability evidence, not measured hardware latency.

For each friction point retain the task, expected/actual behavior, observation or participant words,
assistance, consequence, proposed fix and retest reference. Blocked installation/configuration/use/
recovery, wrong-target actions or misleading confirmation must be resolved before acceptance.
Keep assisted attempts visible. Retest changed behavior; a discovery fix needs a fresh unfamiliar
participant because a coached participant already knows the answer. Do not generalize one person's
successful session to all users or close the separate hardware, sustained-use or enclosure gates.

Guide preparation corrected source-verified discrepancies: Tour is guidance without automatic
completion ticks; shortcut tests have a countdown and await their own result; a failed macro does
not undo earlier steps; trigger-only feedback stays Unverified with a quiet device pulse. The guide
also explains the single-press wait and no longer promises that signing removes a Windows warning.
These documentation corrections have not yet been evaluated with a participant.

## M2.9 assembled-enclosure acceptance (open)

Owner-managed reference only as of 2026-10-26; this former item 19 is excluded from M2.9 software
scope. Retained measurements, nominal checks and procedures are historical supporting material,
not an active software task or a claim that the enclosure passed physical acceptance.

Item 19 preparation, 2026-10-25, Andress-MacBook-Air. No component was physically measured, printed
or assembled here. Item 19 remains unchecked; M2.9 remains 13/19.

### Generator prerequisite fix and local evidence

The hand-wired generator still used the old display/encoder GPIOs and button GPIO0/1/20, while
firmware and its wiring README specify display 4/3/2/1/0, encoder 7/6/5 and buttons 9/8/10.
These assignments drive real wire grooves and engraved labels, so correcting only the README
would leave misleading printable parts. The generator is now aligned with `profile.rs`.
`test_pin_map.py` independently reads the firmware's literal declarations and checks all thirteen
nets (eleven signals plus power/ground), including button order, without importing FreeCAD.
It failed on the old source and passes after the fix; Linux host CI runs it. Unrecognized future
Rust declaration syntax fails the check for review rather than silently omitting the comparison.

Local FreeCAD 1.1.4 regenerated original and corrected models in separate temporary directories:

- Both generation reports: eight valid printed parts and `interference: none`. The corrected
  document contains fourteen valid, nonempty single solids; every object's bounds are unchanged.
- The original regeneration matched all nine pre-fix tracked exports at `918d2d4` byte-for-byte. After the fix, only
  `Carrier.stl` differs; the seven other STLs and `pcb-outline.dxf` remain byte-identical. Only that
  STL and the complete `kivori-case.FCStd` document were replaced. Carrier volume changes by
  +20.066403 mm³; the other thirteen object volumes are unchanged.
- The actual Carrier back projection was inspected for revised routes near the SuperMini and
  updated B1/B2/B3 labels. This does not verify wire accessibility, insulation, groove capacity or
  physical fit. The independent pin check is needed even when the interference report is clean.
- Astra advised the bounded scope; its follow-up hit a usage limit. Sol independently reviewed the
  final source/test/CI/docs, passed the test, rejected the old source in memory, verified export
  hashes and document ZIP integrity, and inspected the back projection. No blocker was found for
  this prerequisite fix; no reviewer claimed physical acceptance.

Checks: Python unittest, targeted workflow Prettier, new links/anchors, nine NOT MEASURED rows,
13/19 checklist and `git diff --check` passed. Desktop/firmware runtime suites were not rerun for
this CAD/source-guard increment. Local evidence remains under `/tmp/kivori-m29-enclosure-*`;
models, reports and images there are not cross-device memory or physical observations.

The selected variant must be explicit: this checkout's `hardware/enclosure/` is hand-wired and
screwless; `docs/assembly.md` describes the separate copper-tape/screw branch. Their mounting
orientation, printed parts and assembly steps cannot be mixed. Experimental modules or another
display/controller require their own evidence and do not inherit this design's acceptance.

### Nominal closed-case BOOT/RST access check

On 2026-10-25, FreeCAD 1.1.4 on Andress-MacBook-Air opened the tracked hand-wired document
without regenerating or saving it. A 1 mm cylindrical probe follows each modeled access axis
from face-frame z = -60 mm to 0.01 mm before the nominal button face at z = -12.9 mm.
Its common volume with every printed part and bought-part stand-in is zero. A separate 0.1 mm
tip segment straddling the button face intersects the nominal SuperMini switch stand-in.

| Modeled access | Face-frame centre (mm) | Clear-path overlap (mm³) | Tip contact (mm³) | Probe shifted +2.5 mm in x: BackPlate overlap (mm³) |
| --- | --- | --- | --- | --- |
| RST | 50.5, 58.0 | 0 | 0.039270 | 12.398782 |
| BOOT | 59.5, 58.0 | 0 | 0.039270 | 12.398782 |

Both deliberately off-centre probes fail the clearance check, providing obstruction controls.
The document SHA-256 before/after is
`8e925575dbbc0d2219bbf046bbd653c2ef39fdbc2b0eebbc4777f21fda2b4cc5`.
The source/firmware pin-map unittest and diff checks also passed; no CAD or export changed.
The FreeCAD connector could not locate its headless executable, so the installed application at
`/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd` ran the check directly. The one-off
script and JSON evidence are local-only at `/tmp/kivori-m29-enclosure-access.py` and
`/tmp/kivori-m29-enclosure-access.json`.

This verifies access only to nominal stand-ins along the modeled axes. It does not establish
real switch positions/labels, printable tolerances, tool angle or actuation force, wiring clearance
after assembly, or successful BOOT/RST operation. Item 19 and every physical row below remain open;
progress stays 13/19. Desktop/firmware runtime suites were not rerun for this read-only CAD check.

### Physical record

Record operator/date, variant/source SHA, generator parameters and CAD/STL hashes, print/slicer
revision, printer/material/orientation/layer/scaling, component suppliers/revisions and short unit
label. Name the instrument, resolution and method for each measurement. Compare measured values
with the selected CAD and actual component specifications; a nominal CAD clearance is not an
observed clearance. Record missing dimensions/force limits as open rather than inventing a tolerance.

| Check | Measurements and required observation | Result / evidence |
| --- | --- | --- |
| Bought parts | Measure actual display PCB/glass/FPC/header, SuperMini USB/buttons, encoder PCB/shaft and switch body/stem/pin locations. Compare with the chosen model before committing to a full print; record each deviation. | NOT MEASURED |
| Carrier and display | Dry-fit components and mounting features; record hole/peg alignment, PCB seating, glass/FPC clearance and window alignment. No forced glass/PCB bend, obscured active area or trapped cable. | NOT MEASURED |
| Button caps | Measure resting/pressed clearance, available travel, stem/socket engagement and return for each cap. Exercise centre and edge presses on all three; record counts, rubbing, preload, sticking and unintended holds. | NOT MEASURED |
| Knob | Measure shaft engagement, resting and pressed axial gap and clearance through a full turn and push at several angles. No rubbing, neighbouring-cap contact, loss of push travel or loose/slipping shaft fit. | NOT MEASURED |
| USB | Use the actual intended data-cable plug. Record opening/plug clearance and measured peak insertion/removal force with the measurement method. Observe connector/PCB displacement and support path; the shell must not prevent full seating or transfer damaging loads to the board. Force without an instrument remains unmeasured. | NOT MEASURED |
| Wiring and closure | With power removed, check continuity against the firmware map and inspect insulation/routing, especially crossings and the SuperMini recess/edge. Close/open the selected case design, record fit and retention, then recheck for pinched wires, shifted parts and shorts before power. | NOT MEASURED |
| RST access | With the case fully closed, verify the labelled access point reaches the actual reset switch without disassembly or contacting other circuitry. Observe reset and normal reconnect; record tool, alignment and actuation. Do not infer location from another variant's instructions. | NOT MEASURED |
| BOOT access/recovery | With the case closed, demonstrate access to the actual BOOT switch, entry into recovery and return to normal operation using the existing recovery procedure. Record tool clearance, labels and actual app/device result; link item 16's flash/recovery evidence when applicable. | NOT MEASURED |
| Closed-unit operation | With the product firmware, repeat physical control and display checks after closure, including mic feedback and USB reconnect. Record any changed behaviour versus the open assembly and failures after reopening/reclosing. | NOT MEASURED |

Retain photos/dimensioned notes and any failed fit. Link every parameter/design correction to the
specific measured discrepancy, regenerate/review the affected geometry, reprint and remeasure.
Do not fix only the CAD stand-in to match another unverified drawing. Close item 19 only when
the actual selected assembled unit has evidence for every row and unresolved fit/access problems
are fixed. Firmware/CAD source consistency, interference reports and electrical provisioning PASS
remain distinct from these physical measurements.
