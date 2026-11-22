# Kivori Roadmap

The order Kivori is built in. What it must be is in [product.md](./product.md); how it works is in
[architecture.md](./architecture.md); open hardware checks are in [M2.9](#m29-reliability-truthful-feedback-and-beta-readiness).

**Direction:** a desk buddy you control your computer with. Two equal pillars, **buddy** (the
reason to buy) and **control** (the daily value), with **monitoring** in support. Next work must
serve at least one pillar and never make the buddy smaller to fit more text.

**Sequence:** working foundation → useful device → configurable device → paid beta → customer
feedback → v1 investment. The aim is to learn whether people pay for Kivori before building every
long-term concern. Engineering rigor stays; only the order changes.

**Beta platforms:** Windows and Apple Silicon macOS are software targets; Mac distribution and
physical acceptance are tracked in [macOS support](./macos.md). Linux is later.

## Rules

1. A milestone is done only when every box is ticked, including its hardware checks.
   Host-sim and other host tests never replace a physical check.
2. Don't start the next milestone while the current one has open hardware rows. Code may run ahead,
   but it doesn't count as done.
3. Every milestone keeps all [product acceptance gates](./product.md#product-acceptance-gates) intact.
   Moving a feature later never relaxes a gate for what does ship.
4. Record platform-specific evidence for Windows and macOS separately. A passing check on one
   platform does not close acceptance on the other.
5. Anything not listed waits in [Later](#later). Move something earlier only if it blocks the beta.

---

## M2.9: Reliability, truthful feedback and beta readiness

**Outcome:** controls execute only while they are still valid, confirmation reflects what Kivori
can actually observe, settings survive restart, and an unfamiliar Windows user can set up and
recover a real unit without developer help.

The software for this milestone is built and host-tested. What remains needs a person with a real
unit or the production website. CAD and enclosure work is owner-managed and not part of M2.9.

- [ ] **Confirm the live release feed:** check that the website's `/download` page reads the live
  GitHub release feed in production, after the D1 release snapshot fix (runbook:
  [apps/web/README.md](../apps/web/README.md#products-and-published-releases)). The original
  failure was never confirmed.
- [ ] **Exercise a real Windows unit**: record measured detent-to-feedback latency, press/hold/
  double-press fidelity, microphone mute across multiple devices and hotplug, repeated USB
  disconnect/reconnect, sleep/wake and lock/unlock. Re-run the
  affected checks below, including no false confirmation and no stale replay.
- [ ] **Prove installation and recovery**: on a clean Windows machine, install the beta
  package (unsigned, see [release.md](./release.md)), launch at login, update the app, update
  firmware and recover through BOOT after an interrupted flash. Verify app installation and firmware
  flashing cannot overlap.
- [ ] **Run sustained use before paid beta**: complete an 8 h idle soak and 10 workdays of actual
  Windows use. Record build, unit, duration, disconnects, missed/wrong inputs, stale indicators,
  crashes and recovery outcomes; fix blockers and repeat affected checks.
- [ ] **Observe an unfamiliar user**: have someone install, configure, use and recover the device
  without developer coaching. Check discovery of button Hold, middle-button profile pin and
  double press, whether the 250 ms single-press wait feels acceptable, and whether the screen's
  labels/results explain what happened. Record friction and resolve blockers before acceptance.

Open physical checks on the RP2040-Zero unit (v8 enclosure), moved here from the old validation
checklist. Tick each with a date and a measurement. Windows unless noted.

- [ ] **Background run:** closing the window hides it and the process keeps running; after 2 min hidden the heartbeat is still live; taskbar click and a second launch bring back one window, one process; tray Quit exits and stops the device task (macOS done 2026-09-25)
- [ ] **Connect:** cold plug-in connects in under 5 s; an unrelated USB-serial device is not shown as connected; wrong-major firmware shows incompatible within 5 s and no state commands are sent
- [ ] **Reconnect:** unplug while busy shows Disconnected; replug returns to busy within 10 s; 10 quick replugs settle into one Connected; 30 min of state changes with no USB stall; set happy, quit and restart the app, the device returns to idle
- [ ] **Timings:** state change under 1 s, connect under 5 s, reconnect plus restore under 10 s, SPI frame rate read from Health or Log
- [ ] **Privacy of the activity view:** it shows only semantic state, no raw bytes, ids, paths or usernames, and no log file is created (macOS partly done 2026-09-25)
- [ ] **Buddy on the panel:** all six states (booting, idle, happy, busy, sleeping, offline) look right and match the Device Studio preview side by side; happy sinks the keycap; Greet, Pet, Tickle, Surprise and Comfort play in Idle, do nothing in Busy, are gentle in Sleeping, and stop on a state change; 2 min of idle is smooth with no crawling edges or flashes
- [ ] **Flash from Device Studio:** Flash firmware completes and the app reconnects to the same port
- [ ] **Knob:** one volume step per detent; slow turns lose none; fast spin has no false reversal; wiggling at a detent edge gives no phantom detent; clamps at 0 and 100 and the first reverse detent leaves the limit; the Volume view appears on a turn, stays about 1.2 s, then returns
- [ ] **Windows volume:** panel matches the flyout and follows flyout changes with no Kivori input; a flyout change mid-turn does not fight the preview; Preview and Confirmed look different; switching the default output between turns rebinds, mid-turn discards the gesture; unplug mid-turn and replug executes nothing; restarting the app leaves no stale overlay
- [ ] **Latency:** with the dev-only latency build, about 30 single detents, max detent to flush under 50 ms (it is a gate; compose was 80 ms on 2026-10-31), then reflash the product build; also check frame cost of the Clock, System and Volume views
- [ ] **Truth gates:** no false confirmation, no stale replay after unplug, restart or device switch, no hidden fallback when an action cannot run, unknown values show `--` on the panel and `—` in the app, recovery works with no healthy desktop software
- [ ] **Push switch:** ten short presses give ten Play/Pause with no auto-repeat; five 1.5 s holds give five mutes, never a Play/Pause; 3 s hold shows the recovery bar at about 2 s and release cancels it; 10 s hold reboots with the app running and with it quit; a claimed hold-and-turn gesture held over 10 s does not reboot; a press while turning fires nothing; a press during an unplug outage does nothing after replug
- [ ] **Profile pin with the knob:** hold the knob and turn to pick a profile, release pins it, "Pinned" shows only after Kivori reports it, and no volume, Press, Hold or double-press action leaks; the hold ring fills over 1 s, an unbound Hold shows no ring
- [ ] **Double press and views:** ten double presses step Buddy, Clock, Volume, Media, System, Buddy with no Play/Pause; the single-press wait of about 250 ms feels acceptable; the slide is smooth; a long media title scrolls and Latin-1 letters render; each view matches the real clock, CPU/RAM, media and volume; quitting the app shows the offline buddy within the heartbeat timeout
- [ ] **Action badges:** mute toggles match the OS mixer (green check) and OS mute changes follow with no Kivori input; media press and shortcut show the amber "?"; launch shows the blue arrow, a missing app the red cross; a forced volume failure shows a red cross, never a fake volume; macOS without Accessibility shows a red cross and works after granting without a restart; Windows and macOS
- [ ] **Three buttons:** hold each button at power-on and in every combination, the unit boots normally and the RP2040 BOOT button still opens `RPI-RP2`; ten presses each give exactly Previous, Play/Pause, Next with the amber "?"; 1.5 s and 12 s holds do nothing and never reboot; two controls together fire nothing; knob hold of 10 s with a button held still reboots; labels line up with the physical buttons; presses during an outage do nothing after replug
- [ ] **Config UI on the device:** a rebound control works and survives a restart and an update over the old build; App Volume and App Mute on `spotify.exe` move only Spotify (macOS greys App volume out); the shortcut Test shows a 3 s countdown; a macro runs in order, cannot be deleted while bound, and stops when focus changes; both Resets ask first and the device shows built-in labels again
- [ ] **Profiles:** focusing Chrome, VS Code, Spotify, Zoom or Teams changes the name and labels together in about 0.5 s; Alt+Tab mid-turn stops at once with one red cross; UAC, Win+L or regedit shows "Protected"; middle button cycles the pins and back to Auto; same on macOS; faces (muted, listening, strained, attentive, error, celebrate) appear only for their real cause; layout is readable at desk distance
- [ ] **Sleep, lock, switch user:** Win+L makes the buddy sleep within 1 s and unlock restores it; Sleep, Hibernate and Modern Standby show Sleeping, then reconnect within 5 s with no replay of input made while asleep; a PC that cuts USB power reconnects on wake; switch user hands the port over; macOS lock, Sleep and lid close behave the same, and `cargo test -p kivori-desktop` passes on the Mac
- [ ] **Assembled unit:** the panel shows the buddy, and the knob, press and three keys act on it; the app's own Update and Restore (BOOT) flows work against this board
- [ ] **Firmware update and recovery:** flash from the installed app on a machine with no Rust toolchain, the panel shows Updating (or goes dark cleanly), never garbage; unplug mid-flash gives a failure and offers Restore; erase then Restore with the BOOT steps recovers the unit; a held-open serial port shows "close other apps"; a wrong-major firmware can be restored; each failure is reported as its own kind and nothing else is written; "update available" shows only when the bundled firmware is newer
- [ ] **Launch at login and tray:** Start with Windows then sign out and in gives the tray icon, no window, device connected; turning it off stops the start; uninstall removes the `Run\Kivori` entry; macOS LaunchAgent works and shows the Background item notice once; tray menu items work and the line reads Connected, Not connected, Paused while locked
- [ ] **Installer:** on a clean Windows 11 VM, the unsigned installer runs through SmartScreen "More info, Run anyway" and as a normal user with no admin prompt; an install over an older build keeps the config; Defender does not quarantine it; macOS `scripts/bundle-macos.sh` puts `mediaremote-adapter` in the `.app` and now-playing works with the Automation usage text
- [ ] **First-run onboarding:** a fresh Windows user sees onboarding once and it does not return after finish or after an update from a build with a `config.json`; macOS "Allow access" turns green without restarting setup; "Run setup again" reopens it and Finish with Launch at login adds the login entry
- [ ] **Per unit QA:** a new unit (BOOT held) flashes by the app or `just fw-flash`, shows up as `RPI-RP2`, restarts, connects with the catalog-required capabilities, identity matches the target and the heartbeat holds; knob right and left three times, press, hold and the three buttons are each seen in the app
- [ ] **Microphone:** with two mics active (built-in and USB), Mic Mute mutes both in Windows Sound settings and the buddy shows the covered-mouth face; a mic plugged in while muted is muted within about 1 s, or the panel shows it live; unmuting in Windows settings is followed within about 1 s; the Live cue works in a Zoom or Teams call; Kivori reports the OS mic, not Zoom's button
- [ ] **Add app and custom profiles:** "Add the app in front of me" makes a new profile with its icon and the panel shows its name within about 0.5 s; an app moves between profiles and never sits in two; "Show on device" profiles appear after the built-ins after holding the middle button
- [ ] **Idle life and brightness:** with the app idle 5 min, then suspended, the buddy keeps blinking and glancing; touching a control makes it look that way first; brightness follows at once, the lowest step is clearly on and survives a restart; media key and shortcut buttons give a quiet pulse with no celebration; unplugging audio then Mute shows the amber "?"
- [ ] **Developer mode and splash:** the Device internals card updates about once a second and hides when developer mode is off; image and blob sizes match the build; "Copy diagnostics" has internals only when on and no ports, ids or paths; cold start shows the splash once, a warm start does not, and "Start with Windows" shows no splash
- [ ] **Identity and offers:** a current unit shows product "Kivori Pip", board "RP2040-Zero" and its revision (or "unknown") in Diagnostics; an older unit that reports no identity is treated as legacy Kivori Pip and offered the firmware update a few seconds after it connects; "Later" brings the offer back after replug and "Update now" shows Updating then the buddy; an image for another board is refused naming the mismatch and nothing is written
- [ ] **App updates:** with an older release installed and a newer one published, a tray badge shows within a day or at next start; "Install & restart" installs without questions, restarts on the new version and reconnects; "Skip this version" stops the offer until a newer one; with the network off or "Check for updates" off there is no error and no request; a wrong-signature update is refused and nothing installs
- [ ] **Daily use for a few days:** press and hold timing feels natural; note accidental holds and missed presses for tuning

**Exit:** the items above are done and recorded, and no unresolved issue can falsely confirm an
action, execute stale input, discard successfully saved settings or block common recovery. M2.9 does
not close M3 cost, assembly and selling gates without their own evidence.

Out of scope: architecture rewrite, a general simulator/plugin framework, new platforms, new product features, controller migration,
cloud services, CAD/enclosure design and fit validation, and a broad visual redesign.

## M3: Paid beta

**Outcome:** an external user receives Kivori, installs it, configures it, recovers from common
problems and uses it without developer help. Start charging for beta units here.

- [ ] M2.9 reliability, recovery, usability and sustained Windows-use acceptance complete
- [ ] Enclosure (keeps the USB port and the RP2040-Zero BOOT button reachable for recovery; the three
  buttons sit in a row directly under the screen, centres one third of the panel's active width
  apart, each under its 80 px label column (#14))
- [ ] BOM and real unit cost (draft parts list in [bom.md](./bom.md); prices, suppliers and the real unit cost still need the owner)
- [ ] Repeatable assembly process (wiring and build steps are in [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md); stays open until a second person has built a unit with them)
- [ ] Device flashing process (UF2 through the app, or `just fw-flash`; no per-unit provisioning, see [release.md](./release.md#firmware-flashing); stays open until a person has run it on real RP2040-Zero units)
- [ ] Hardware QA checklist per unit (built from the per-unit QA and knob, button and connect checks in M2.9; manual, still needs a human run)
- [ ] Windows installer bundling the known-compatible firmware. It ships unsigned: no paid code-signing certificate (owner decision, 2026-10-31); `release.yml` builds, install-tests and drafts the NSIS installer. The SmartScreen and Defender warnings are documented in [release.md](./release.md)
- [ ] macOS build (if shipped; `scripts/bundle-macos.sh` bundles the adapter, entitlements and Info.plist; it ships unsigned, no Developer ID certificate; the hardware rows are open)
- [ ] Launch at login and tray presence; the window stays optional. Software done in M3 S4 (autostart plugin, hidden start, tray status and switch, Device page switch); stays open until the launch-at-login checks in M2.9 pass
- [ ] Survive sleep/wake and lock/unlock with intentional screens, never a frozen frame (gate 9). Software done in M3 S1+S2 (host takeovers: Sleeping, Updating and a 4 s host-silence Offline; sleep, lock and console-switch handling in Desktop); stays open until the sleep, lock and switch-user checks in M2.9 pass
- [ ] First-run onboarding. Software done in M3 S6 (stepper for plug in, macOS Accessibility, a capability-filtered non-verifying guide (replacing the earlier live-tour heuristic), default view, buddy intensity and launch at login, with "Run setup again" on the Device page); stays open until the first-run onboarding checks in M2.9 pass
- [ ] User-facing firmware update and recovery flow (flash the bundled build; a failed flash says so). Software done in M3 S3 (flash, classified failures, BOOT-button restore, incompatible recovery); the app writes UF2 itself, so no flashing tool ships in the installer; stays open until the firmware update and recovery checks in M2.9 pass
- [ ] Regulatory check before selling (FCC Part 15 / CE for the finished unit, not just the module)
- [ ] Basic packaging and a short user guide (guide written: [user-guide.md](./user-guide.md); stays open until the physical packaging exists)
- [ ] Build and ship the first 5–20 beta units
- [ ] All 10 gates for what ships

## M4: v1

Use paid-beta evidence to pick what deserves deeper investment. None of these is assumed required
before demand is proven.

- Better macros; more monitoring (GPU temperature, mic where available, custom sources)
- More physical inputs
- macOS support (CoreAudio backend, Accessibility permission flow, menu-bar presence)
- Integrations (Discord, Teams, OBS) as indicators
- Production PCB with an app-independent ROM recovery path, on the v1 controller chosen in ADR-0010
  (proposed: native USB so basic controls work with no app installed)
- `Update All` in one step; A/B firmware with rollback
- Secure boot on production units with native USB (ADR-0016)
- Update channels (beta / stable) and, if firmware cadence demands it, firmware fetched apart from
  the app through the same catalog format (ADR-0014)
- macOS auto-update
- Display idle and burn-in protection; baseline soak and sustained Windows use are required in M2.9

## Later

Unless it blocks the beta:

- Windows/macOS feature parity
- Exhaustive sleep/lock edge cases
- Multi-user session switching; a second device stays Passive
- Factory reset, separate from recovery
- Composite inputs (`Hold + Rotate`) and split-direction bindings
- Buzzer or haptics
- Linux support
- Monitor Mode for a second device
- Focus timer on the device (truthful, since the device owns it, but not core)
- External status sources (CLI / local HTTP / WebSocket). A local listener is a new attack surface
  and breaks the no-network rule, so it needs its own ADR first
