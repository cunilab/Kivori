# Kivori Roadmap

The order Kivori is built in. What it must be is in [product.md](./product.md); how it works is in
[architecture.md](./architecture.md); hardware checks are in [validation.md](./validation.md).

**Direction:** a programmable desktop controller, a desk buddy and a PC monitor, in that order of
pillars: **control, buddy, monitoring.** Next work must serve at least one of them.

**Sequence:** working foundation → useful device → configurable device → paid beta → customer
feedback → v1 investment. The aim is to learn whether people pay for Kivori before building every
long-term concern. Engineering rigor stays; only the order changes.

**Beta platform:** Windows is supported. macOS is later or experimental. Linux is later.

## Rules

1. A milestone is done only when every box is ticked, including its hardware rows in
   [validation.md](./validation.md). Simulation and host tests never replace a physical check.
2. Don't start the next milestone while the current one has open hardware rows. Code may run ahead,
   but it doesn't count as done.
3. Every milestone keeps all [product acceptance gates](./product.md#product-acceptance-gates) intact.
   Moving a feature later never relaxes a gate for what does ship.
4. Until v1, a feature is done when it works on Windows. macOS code may land but is not a blocker.
5. Anything not listed waits in [Later](#later). Move something earlier only if it blocks the beta.

---

## M0: Kivori Core (mostly done)

Software for the device link, mascot, rotary volume and flashing is built and green in CI
(about 300 workspace and 80 firmware tests; RISC-V clippy on every profile; Windows CI).

- [x] ESP32-C3 ↔ desktop protocol: postcard + CRC frames, handshake, version and capability negotiation, nonce as session identity
- [x] Automatic discovery, reconnect state machine, heartbeat, offline-first boundary
- [x] ST7789 rendering: deterministic renderer, 36-tile DMA path, Device Studio preview, golden-frame and Wokwi tests
- [x] Rotary input: HW-040 decoder (detent only on return to rest), 250 ms gesture boundary
- [x] Windows volume control: Core Audio backend, fixed-step volume, preview → confirmed overlay
- [x] Mascot: keycap buddy, social reactions, companion personality, typed session-only activity log
- [x] Firmware flashing of the bundled build from Device Studio
- [x] Hardening before the hardware session (2026-10-02): interrupt-captured encoder and switch edges
  (no lost quarter-steps during a frame flush), render on the tick feedback arrives, a desktop device
  thread that wakes on serial bytes instead of a fixed 50 ms sleep (row 3.14 remedies 1 and 2), and a
  `StateReport` answer to every `SetState` (row 1.16). Flashing and the session handshake verified on
  the real board from macOS; every row below still needs the human session
- [ ] Hardware sanity checks needed to build on safely (Windows only), in [validation.md](./validation.md):
  - wiring and rotary fidelity: 3.15, 3.1–3.4
  - discovery and recovery: 1.5–1.8, 1.15–1.17
  - volume truth and bounds: 3.5, 3.6, 3.11–3.13
  - detent → feedback under 50 ms: 3.14
  - panel and flashing: 2.1, 2.9
  - gates G1 (no false confirmation) and G2 (no stale replay)

**Exit:** the foundation is reliable enough to build product features on. The remaining Phase 1–3
rows move to M3 hardware QA.

## M1: Useful desk device

**Outcome:** Kivori gives daily value beyond being a volume knob.

Software is complete and host-tested (2026-10-02); the hardware rows are Phase 4 in
[validation.md](./validation.md). Built for Windows and macOS: macOS is the daily test bed, Windows
stays the beta platform and its rows are tracked in a GitHub issue.

- [x] Button press / hold input: short press released within 500 ms, mapped Hold released between 500 ms and ~2 s, one action per press, no auto-repeat
- [x] Recovery hold: ~10 s reboots the MCU, with visible progress and without the desktop app (gate 8)
- [x] Media play/pause, master mute (Press and Hold by default)
- [x] Keyboard shortcut action (shown as Unverified, never Success); runs from Device Studio until M2 bindings
- [x] Launch application action (Execution Confirmed); runs from Device Studio until M2 bindings
- [x] CPU and RAM monitoring
- [x] Display modes: buddy, clock, volume, media, CPU / RAM (picked in Overview)
- [x] Buddy reacts to real state: volume change → the keycap shrinks and lifts for the volume bar; muted → mute indicator; high load → heat cue (distinct from the Busy job face); media playing → bobbing note (Windows; macOS cannot observe playback); action outcome → badge; disconnected → offline
- [x] Every result shown as State Confirmed, Execution Confirmed or Unverified
- [ ] Hardware: press/hold timing on the real switch; mute and media state stay in sync with Windows; reboot works without the desktop app (Phase 4 rows)
- [ ] Gates 1, 5, 6, 8, 9

## M2: Configurable product

**Outcome:** someone who did not build Kivori can configure and use it without editing code.

- [ ] Config UI for Rotate / Press / Hold bindings
- [ ] Action catalog with explicit scope (System Volume ≠ App Volume)
- [ ] Simple ordered macros (a macro reports its least-confirmed step; no rollback)
- [ ] Choose what the display shows (default and secondary modes)
- [ ] Buddy settings: reactions on/off, intensity
- [ ] Config stored locally per OS user and machine; survives restarts and updates; reset to defaults
- [ ] Basic device status and diagnostics (versions, connection, health)
- [ ] Hardware: rebind → use → restart → binding still there
- [ ] Gates 3, 4, 5

Example config:

```text
Control   Rotate: Volume   Press: Play / Pause   Hold: Open Spotify
Display   Default: Buddy   Secondary: CPU / RAM
Buddy     Reactions: On    Intensity: Normal
```

## M3: Paid beta

**Outcome:** an external user receives Kivori, installs it, configures it, recovers from common
problems and uses it without developer help. Start charging for beta units here.

- [ ] Enclosure (keeps the USB port and the ESP32-C3 BOOT path reachable for recovery)
- [ ] BOM and real unit cost
- [ ] Repeatable assembly process
- [ ] Device flashing and provisioning process
- [ ] Hardware QA checklist per unit (built from the remaining Phase 1–3 rows in [validation.md](./validation.md))
- [ ] Signed Windows installer, bundling the known-compatible firmware (this is the beta's update authentication, gate 10)
- [ ] Launch at login and tray presence; the window stays optional
- [ ] Survive sleep/wake and lock/unlock with intentional screens, never a frozen frame (gate 9)
- [ ] First-run onboarding
- [ ] User-facing firmware update and recovery flow (flash the bundled build; a failed flash says so)
- [ ] Device and software version shown to the user
- [ ] Regulatory check before selling (FCC Part 15 / CE for the finished unit, not just the module)
- [ ] Basic packaging and a short user guide
- [ ] Build and ship the first 5–20 beta units
- [ ] All 10 gates for what ships

## M4: v1

Use paid-beta evidence to pick what deserves deeper investment. None of these is assumed required
before demand is proven.

- Better macros; more monitoring (GPU temperature, mic where available, custom sources)
- App-aware profiles (stable focus, 300–500 ms stabilization, gesture stays bound to its app)
- More physical inputs
- macOS support (CoreAudio backend, Accessibility permission flow, menu-bar presence)
- Integrations (Discord, Teams, OBS) as indicators
- Production PCB with an app-independent ROM recovery path
- Signed release manifest, compatibility checks, `Update All`; A/B firmware with rollback
- Installers with auto-update
- Long-term reliability: 8 h idle soak, display idle and burn-in protection, 10 workdays of real use

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
