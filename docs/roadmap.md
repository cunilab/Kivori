# Kivori Roadmap

The path from today's code to the product in the [PRD](./product/prd.md). Each phase delivers
something usable and ends with a checklist. The PRD says what Kivori must be; this file says the order
it is built in.

**Target for v1:** the maintainer uses Kivori every workday, on both Windows and macOS (PRD §2.1).

## Rules

1. **A phase is closed only when every checkbox is ticked.** Physical validation cannot be replaced by
   simulation or host tests. Write the evidence (date, measurement) in that phase's validation
   checklist.
2. **Don't start the next phase while the current one still has open physical-validation rows.**
   Software can run ahead, but it doesn't count as done.
3. **Each phase must keep every [PRD acceptance gate](./product/prd.md#18-product-acceptance-gates)
   intact.** Tick the gates that apply in the phase's checklist.
4. **Both OSes ship together.** A desktop capability is not done until it works on Windows *and*
   macOS, or the PRD explicitly marks it as unavailable on one of them.
5. **Keep scope small.** Anything not listed goes to [Later](#later) until a phase needs it.

Open physical rows for Phases 1–3 are walked in one sitting via [`validation-session.md`](./validation-session.md).

Every phase checklist has the same five parts: **Build**, **Automated**, **Physical**,
**PRD gates**, **Docs**.

---

## Phase 1: Device connection foundation ✅ software, 🟡 physical

Record: [`features/001-device-connection-foundation/`](./features/001-device-connection-foundation/)

- [x] Build: framed protocol (postcard + CRC), handshake, version and capability negotiation
- [x] Build: automatic discovery with no port picker; reconnect state machine; heartbeat
- [x] Build: deterministic renderer, asset pipeline, Device Studio preview
- [x] Build: ESP32-C3 + ST7789 runtime; Wokwi simulation; offline-first boundary
- [x] Automated: host, firmware host-sim, golden frames, Wokwi scenarios green in CI
- [ ] Physical: the remaining open rows in the [validation checklist](./features/001-device-connection-foundation/validation-checklist.md)
      (5 of 25 done; rows 1, 3 and 4 also passed on macOS, and Windows is still to run), including plug-in → connected in under 5 s, and device output matching the
      preview pixel for pixel

## Phase 2: Mascot, reactions, activity log ✅ software, 🟡 physical

Record: [`features/003-mascot-animation/`](./features/003-mascot-animation/) (PR #3)

- [x] Build: layered mascot sprites, `MascotAnimator` (blink, motion, eased transitions)
- [x] Build: social reactions over the wire (`MASCOT_INTERACTION`); companion director and personality
- [x] Build: 36-tile DMA render path; bundled firmware flashing from Device Studio
- [x] Build: typed, session-only activity log, the only runtime log (Log view)
- [x] Automated: render-parity and golden tests; Wokwi vectors for frame version 2
- [ ] Physical: every mascot state and reaction on the panel; flash → reconnect to the same port
- [x] PRD gate: the personality layer never shows a desktop state that isn't happening (PRD §9.5).
      [Review](./features/003-mascot-animation/personality-review.md) found and fixed 3 violations; regression tests added

## Phase 3: Rotary volume loop (Slice 002) ✅ software, 🟡 physical

Record: [`features/002-rotary-volume-control/`](./features/002-rotary-volume-control/) (PR #2)

- [x] Build: HW-040 quadrature decoder, detent-qualified gestures, 250 ms gesture boundary
- [x] Build: `InputEvent` / `Presentation` (tags 13/14); nonce as connection-scoped session identity
- [x] Build: Windows Core Audio backend; fixed-step volume; preview → confirmed overlay
- [x] Build: consolidated on top of Phase 2 (tags, capability bits, activity log, mascot layering)
- [x] Automated: 294 workspace + 74 firmware tests; RISC-V clippy on every profile
- [ ] Automated: PR #2 CI green on Windows, then merged
- [ ] Physical: all 15 rows of the [validation checklist](./features/002-rotary-volume-control/validation-checklist.md),
      including measured detent → feedback latency under 50 ms
- [ ] PRD gates: no false confirmation (1), no stale replay (2)

---

## Phase 4: Daily knob on both OSes

**Outcome:** plug Kivori into either machine and it controls volume all day without any attention.

- [ ] Build: macOS volume backend (CoreAudio default output device; follows device changes)
- [ ] Build: launch at login + tray/menu-bar presence; the app window stays optional (PRD §6.5)
- [ ] Build: survives host sleep/wake and lock/unlock; shows Sleeping/Locked and Reconnecting
      screens, not a frozen frame (PRD §10)
- [ ] Build: external volume changes (keyboard or OS slider) show up on the device (PRD §5.1)
- [ ] Automated: the volume-backend contract tests run against both backends
- [ ] Physical: on Windows and macOS, measure detent → feedback under 50 ms; sleep/wake ×10 with
      no stuck state
- [ ] PRD gates: 1, 2, 6, 9
- [ ] Docs: install and run guide for macOS and Windows

## Phase 5: Button and core actions

**Outcome:** pressing the knob does real work, and the device shows the resulting state.

- [ ] Build: press/release input; one action per press, no auto-repeat (PRD §7.1)
- [ ] Build: media play/pause and next; **microphone mute** with confirmed state; master mute
- [ ] Build: secondary indicators for mic mute, audio mute and media activity (PRD §9.4)
- [ ] Build: holding the button ~10 s reboots the MCU, with visible hold progress (PRD §14.1)
- [ ] Build: each action result is shown as State Confirmed, Execution Confirmed or Unverified
      (PRD §7.3)
- [ ] Automated: action outcome tests for each confirmation level; gesture arbitration tests
- [ ] Physical: mic mute stays in sync when toggled from the OS or the call app; reboot gesture
      works when the desktop app is not running
- [ ] PRD gates: 1, 5, 6, 8

## Phase 6: Configuration

**Outcome:** change what the knob and button do without touching code.

- [ ] Build: config UI covering General-profile bindings for Rotate / Press, sensitivity and
      acceleration (PRD §7.2)
- [ ] Build: explicit action catalog with scope (`System Volume` ≠ `App Volume`); keyboard
      shortcut and app-launch actions
- [ ] Build: config stored locally per OS user and per machine (PRD §8.4–8.5); survives updates
- [ ] Build: macOS Accessibility permission flow; Permission Required state when it is missing
      (PRD §11)
- [ ] Automated: config migration and round-trip tests; permission-denied behavior tests
- [ ] Physical: rebind → use → restart → the binding is still there, on both OSes
- [ ] PRD gates: 3, 4, 5

## Phase 7: App-aware profiles

**Outcome:** the knob means different things in different apps, with no surprises.

- [ ] Build: detect the focused app (Windows and macOS), with 300–500 ms stabilization (PRD §8.1)
- [ ] Build: per-app profiles that override the General profile explicitly; no split-direction
      mixing (PRD §8.3)
- [ ] Build: a gesture stays bound to the app it started in (PRD §8.2)
- [ ] Build: the device shows which profile is active, briefly, on change
- [ ] Automated: focus-flapping and mid-gesture focus-change tests
- [ ] Physical: Alt-Tab / Cmd-Tab during rotation never leaks the input into the new app
- [ ] PRD gates: 2, 3, 5

## Phase 8: Lives on the desk (v1.0)

**Outcome:** a finished-feeling object that is always honest about what is going on.

- [ ] Build: the buddy's primary state follows the desktop: Idle, Active, Busy, Success, Error,
      Unknown (PRD §9.3)
- [ ] Build: display idle Normal → Dim → Low Motion → Display Sleep; the first touch only wakes the
      screen (PRD §12)
- [ ] Build: burn-in protection (pixel shift, reduced motion)
- [ ] Build: every takeover state has an intentional screen (PRD §9.1)
- [ ] Physical: 8 h idle soak with no burn-in artifacts; every takeover state seen on the panel
- [ ] PRD gates: all 10
- [ ] **v1 exit:** 10 workdays of real use on both machines, with no restart, no reflash and no
      opening the config window except to change a setting on purpose (PRD §2.1)

---

## v2: Beta track (other people can use it)

- [ ] Signed release manifest; desktop and firmware compatibility checks; `Update All` ordering
      (PRD §14.4)
- [ ] A/B firmware slots with post-boot validation and automatic rollback (PRD §14.3)
- [ ] Production PCB with an accessible ROM-recovery mechanism; Desktop detects recovery mode
      (PRD §14.2)
- [ ] Installers and auto-update for the desktop app on both OSes (PRD §14.5)
- [ ] Enclosure and hardware revision; hardware revision reported over the protocol
- [ ] Multi-user session switching; a second device stays Passive (PRD §8.4, §15)
- [ ] Factory reset procedure, separate from recovery

## Later

- [ ] Input hardware revision: more controls, buzzer or haptic feedback (PRD §13, §19)
- [ ] Call/app state integrations (Discord, Teams, OBS) shown as indicators
- [ ] Scripts and macros as first-class actions
- [ ] Explicit composite inputs (`Hold + Rotate`) and split-direction bindings
- [ ] Linux support
- [ ] Monitor Mode for a second device
