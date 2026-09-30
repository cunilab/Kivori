# Kivori Roadmap

The order Kivori is built in. What it must be is in [product.md](./product.md); how it works is in
[architecture.md](./architecture.md); hardware checks are in [validation.md](./validation.md).

**v1 target:** the maintainer uses Kivori every workday on both Windows and macOS.

## Rules

1. A phase is done only when every box is ticked, including its hardware rows in
   [validation.md](./validation.md). Simulation and host tests never replace a physical check.
2. Don't start the next phase while the current one has open hardware rows. Code may run ahead, but
   it doesn't count as done.
3. Every phase keeps all [product acceptance gates](./product.md) intact.
4. Windows and macOS ship together, unless the product doc marks a feature as unavailable on one.
5. Anything not listed waits in [Later](#later).

---

## Phase 1: Device connection ✅ software, 🟡 hardware

- [x] Framed protocol (postcard + CRC), handshake, version and capability negotiation
- [x] Automatic discovery (no port picker), reconnect state machine, heartbeat
- [x] Deterministic renderer, asset pipeline, Device Studio preview
- [x] ESP32-C3 + ST7789 runtime, Wokwi simulation, offline-first boundary
- [x] Host, firmware host-sim, golden-frame and Wokwi tests green in CI
- [ ] Hardware: Phase 1 rows in [validation.md](./validation.md) (5 of 25 done; Windows not run yet)

## Phase 2: Mascot, reactions, activity log ✅ software, 🟡 hardware

- [x] Keycap mascot: fixed base + pressing cap, blink, motion, eased transitions
- [x] Social reactions over the wire; companion director and personality
- [x] 36-tile DMA render path; flashing bundled firmware from Device Studio
- [x] Typed, session-only activity log (the only runtime log)
- [x] Render-parity and golden-frame tests
- [x] Gate: the personality never shows a desktop state that isn't happening
- [ ] Hardware: Phase 2 rows in [validation.md](./validation.md)

## Phase 3: Rotary volume loop ✅ software, 🟡 hardware

- [x] HW-040 decoder (detent only on return to rest), 250 ms gesture boundary
- [x] `InputEvent` / `Presentation` messages; nonce as session identity
- [x] Windows Core Audio backend; fixed-step volume; preview → confirmed overlay
- [x] 300 workspace + 78 firmware tests; RISC-V clippy on every profile; merged with Windows CI green
- [ ] Hardware: Phase 3 rows in [validation.md](./validation.md), including detent → feedback under 50 ms
- [ ] Gates 1 (no false confirmation) and 2 (no stale replay) confirmed on hardware

---

## Phase 4: Daily knob on both OSes (next)

**Outcome:** plug Kivori into either machine and it controls volume all day without attention.

- [ ] macOS volume backend (CoreAudio default output; follows device changes)
- [ ] Launch at login + tray / menu-bar presence; the window stays optional
- [ ] Survive sleep/wake and lock/unlock with Sleeping/Locked and Reconnecting screens, never a frozen frame
- [ ] External volume changes (keyboard, OS slider) show on the device
- [ ] Volume-backend contract tests run against both backends
- [ ] Hardware: detent → feedback under 50 ms on both OSes; sleep/wake ×10 with no stuck state
- [ ] Gates 1, 2, 6, 9
- [ ] Install and run guide for macOS and Windows

## Phase 5: Button and core actions

**Outcome:** pressing the knob does real work and the device shows the result.

- [ ] Press/release input; one action per press, no auto-repeat
- [ ] Media play/pause and next; microphone mute with confirmed state; master mute
- [ ] Indicators for mic mute, audio mute and media activity
- [ ] Hold ~10 s reboots the MCU, with visible hold progress
- [ ] Every result shown as State Confirmed, Execution Confirmed or Unverified
- [ ] Hardware: mic mute stays in sync with the OS and call apps; reboot works without the desktop app
- [ ] Gates 1, 5, 6, 8

## Phase 6: Configuration

**Outcome:** change what the knob and button do without touching code.

- [ ] Config UI for Rotate / Press bindings, sensitivity and acceleration
- [ ] Action catalog with explicit scope (System Volume ≠ App Volume); shortcut and app-launch actions
- [ ] Config stored locally per OS user and machine; survives updates
- [ ] macOS Accessibility permission flow; Permission Required state when missing
- [ ] Hardware: rebind → use → restart → binding still there, on both OSes
- [ ] Gates 3, 4, 5

## Phase 7: App-aware profiles

**Outcome:** the knob means different things in different apps, with no surprises.

- [ ] Detect the focused app on both OSes, with 300–500 ms stabilization
- [ ] Per-app profiles that explicitly override the General profile
- [ ] A gesture stays bound to the app it started in
- [ ] The device briefly shows the active profile on change
- [ ] Hardware: Alt-Tab / Cmd-Tab mid-rotation never leaks input into the new app
- [ ] Gates 2, 3, 5

## Phase 8: Lives on the desk (v1.0)

**Outcome:** a finished object that is always honest about what is going on.

- [ ] Primary state follows the desktop: Idle, Active, Busy, Success, Error, Unknown
- [ ] Display idle: Normal → Dim → Low Motion → Sleep; the first touch only wakes the screen
- [ ] Burn-in protection (pixel shift, reduced motion)
- [ ] Every takeover state has an intentional screen
- [ ] Hardware: 8 h idle soak with no burn-in; every takeover state seen on the panel
- [ ] All 10 gates
- [ ] **v1 exit:** 10 workdays of real use on both machines with no restart, reflash or config fiddling

---

## v2: Beta (other people can use it)

- Signed release manifest, compatibility checks, `Update All` ordering
- A/B firmware slots with post-boot validation and automatic rollback
- Production PCB with accessible ROM recovery; desktop detects recovery mode
- Installers and auto-update on both OSes
- Enclosure and hardware revision reported over the protocol
- Multi-user session switching; a second device stays passive
- Factory reset, separate from recovery

## Later

- More controls, buzzer or haptics
- Call/app integrations (Discord, Teams, OBS) as indicators
- Scripts and macros as actions
- Composite inputs (`Hold + Rotate`) and split-direction bindings
- Linux support
- Monitor Mode for a second device
