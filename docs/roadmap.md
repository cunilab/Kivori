# Kivori Roadmap

The order Kivori is built in. What it must be is in [product.md](./product.md); how it works is in
[architecture.md](./architecture.md); hardware checks are in [validation.md](./validation.md).

**Direction:** a desk buddy you control your computer with. Two equal pillars, **buddy** (the
reason to buy) and **control** (the daily value), with **monitoring** in support. Next work must
serve at least one pillar and never make the buddy smaller to fit more text.

**Sequence:** working foundation → useful device → configurable device → paid beta → customer
feedback → v1 investment. The aim is to learn whether people pay for Kivori before building every
long-term concern. Engineering rigor stays; only the order changes.

**Beta platforms:** Windows and Apple Silicon macOS are software targets; Mac distribution and
physical acceptance are tracked in [macOS support](./macos.md). Linux is later.

## Rules

1. A milestone is done only when every box is ticked, including its hardware rows in
   [validation.md](./validation.md). Host-sim and other host tests never replace a physical check.
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
unit or the production website. Software completion and physical acceptance are recorded separately
in [validation.md](./validation.md). CAD and enclosure work is owner-managed and not part of M2.9.

- [ ] **Confirm the live release feed:** check that the website's `/download` page reads the live
  GitHub release feed in production, after the D1 release snapshot fix (runbook:
  [apps/web/README.md](../apps/web/README.md#products-and-published-releases)). The original
  failure was never confirmed.
- [ ] **Exercise a real Windows unit**: record measured detent-to-feedback latency, press/hold/
  double-press fidelity, microphone mute across multiple devices and hotplug, repeated USB
  disconnect/reconnect, sleep/wake and lock/unlock. Re-run affected existing rows in
  [validation.md](./validation.md), including no false confirmation and no stale replay.
  [Session procedure and result record](./validation.md#m29-windows-unit-acceptance-open) are prepared;
  physical execution is still to do.
- [ ] **Prove installation and recovery**: on a clean Windows machine, install the beta
  package (unsigned, see [release.md](./release.md)), launch at login, update the app, update
  firmware and recover through BOOT after an interrupted flash. Verify app installation and firmware
  flashing cannot overlap.
  [Artifact prerequisites and session record](./validation.md#m29-windows-installation-and-recovery-acceptance-open)
  are prepared; a clean Windows host and physical recovery results are pending.
- [ ] **Run sustained use before paid beta**: complete an 8 h idle soak and 10 workdays of actual
  Windows use. Record build, unit, duration, disconnects, missed/wrong inputs, stale indicators,
  crashes and recovery outcomes; fix blockers and repeat affected checks.
  [Soak and workday record](./validation.md#m29-sustained-use-acceptance-open) is prepared;
  neither the timed soak nor any qualifying Windows workday has been recorded yet.
- [ ] **Observe an unfamiliar user**: have someone install, configure, use and recover the device
  without developer coaching. Check discovery of button Hold, middle-button profile pin and
  double press, whether the 250 ms single-press wait feels acceptable, and whether the screen's
  labels/results explain what happened. Record friction and resolve blockers before acceptance.
  [Neutral tasks and observer record](./validation.md#m29-unfamiliar-user-acceptance-open) are prepared;
  participant observation and acceptance are still to do.

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
- [ ] Device flashing process (UF2 through the app, or `just fw-flash`; no per-unit provisioning, see [provisioning.md](./provisioning.md); stays open until a person has run it on real RP2040-Zero units)
- [ ] Hardware QA checklist per unit (built from the remaining Phase 1–3 rows in [validation.md](./validation.md); validation Phase 12 is now a manual checklist and still needs a human run)
- [ ] Windows installer bundling the known-compatible firmware. It ships unsigned: no paid code-signing certificate (owner decision, 2026-10-31); `release.yml` builds, install-tests and drafts the NSIS installer. The SmartScreen and Defender warnings are documented in [release.md](./release.md)
- [ ] macOS build (if shipped; `scripts/bundle-macos.sh` bundles the adapter, entitlements and Info.plist; it ships unsigned, no Developer ID certificate; the hardware rows are open)
- [ ] Launch at login and tray presence; the window stays optional. Software done in M3 S4 (autostart plugin, hidden start, tray status and switch, Device page switch); stays open until the S4 hardware rows in [validation.md](./validation.md) pass
- [ ] Survive sleep/wake and lock/unlock with intentional screens, never a frozen frame (gate 9). Software done in M3 S1+S2 (host takeovers: Sleeping, Updating and a 4 s host-silence Offline; sleep, lock and console-switch handling in Desktop); stays open until the S2 hardware rows (Phase 7) in [validation.md](./validation.md) pass
- [ ] First-run onboarding. Software done in M3 S6 (stepper for plug in, macOS Accessibility, a capability-filtered non-verifying guide (replacing the earlier live-tour heuristic), default view, buddy intensity and launch at login, with "Run setup again" on the Device page); stays open until the S6 hardware rows in [validation.md](./validation.md) pass
- [ ] User-facing firmware update and recovery flow (flash the bundled build; a failed flash says so). Software done in M3 S3 (flash, classified failures, BOOT-button restore, incompatible recovery); the app writes UF2 itself, so no flashing tool ships in the installer; stays open until the S3 hardware rows (Phase 8) in [validation.md](./validation.md) pass
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
