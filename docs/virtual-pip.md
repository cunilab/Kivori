# Virtual Pip: software verification plan

Status: implementation authorized on 2026-10-26 after owner review. M2.9 S5.1–S5.6 are implemented
and verified locally and in Linux/Windows CI. See
[ADR-0018](./adr/0018-product-owned-virtual-device-processes.md), the
[roadmap](./roadmap.md#s5-isolated-virtual-pip) and
[acceptance evidence](./validation.md#m29-virtual-pip-software-acceptance).

## Scope and ownership

Start with Kivori Pip (permanent machine id `pip`), one simulator process and one selected
desktop connection. Keep the actual no_std firmware core and existing protocol/model/renderer;
replace hardware adapters, not product behavior. Future products own independent executables,
input models and scenarios. They must not import Pip's simulator. Shared contracts are the
existing crates; a universal simulator SDK, plugins and multi-device UI are out of scope.

Implemented paths:

```text
firmware/pip/
  src/                         production no_std firmware; existing sim adapters
  simulator/                   separate host Cargo workspace and executable
    src/                       raw inputs, runner, capture and process control
    src/input_scenarios.rs     deterministic raw-input cases through production Runtime
    src/io_scenarios.rs        bounded FIFO, timed display and input-arrival stress checks
    src/input_scenarios/       fixed-seed replay and structured expected/actual results
    fixtures/                 committed frame goldens
    smoke-report.mjs           file-backed report and replay validation
apps/desktop/src-tauri/src/     generic development-only child-process bridge
apps/desktop/src/               explicit virtual connection and controls in developer UI
```

Runner Cargo commands must name their manifest and host target, with separate target output and
without inheriting Pip's embedded runner/linker flags. Product release recipes remain unchanged.
The dedicated `virtual_device` desktop example owns the development bridge and UI, sharing
production Session/RotaryPipeline/DeskRuntime and session synchronization without starting the
shipping app/device loop. S5.4 separately checks temporary ConfigStore persistence and setup UI
contracts; it does not establish the shipping app's native setup/device-loop integration.
The bridge selects an explicit local runner; no arbitrary shell command text, native simulator network listener,
virtual COM driver, fake catalog registration or bypass of identity/capability admission.

## Execution and safety boundaries

```text
Virtual raw A/B/button levels, timestamp and seed
                      ↓
Pip InputSource::drain → production Runtime → CaptureDisplay → virtual panel
                              ↕ existing protocol bytes
             dev-only local child-process pipe bridge
                              ↕
              real desktop session and action logic
                              ↕
               fake OS services + isolated settings
```

The process control envelope is development-only and bounded: raw wire traffic, input/time/reset
commands and captured frames are distinct records. Reject malformed/oversized records, bound queues
and partial I/O work, and define EOF, child failure, cancellation and cleanup. Frame backpressure
must not starve protocol/input. No pixel transport is added to the product wire schema. The first
slice must include these basics; exact framing/limits are implemented and tested with that slice.

Inputs use timestamped adapter-normalized levels, including quadrature transitions and button
down/up. Physical low reads as true at this firmware port: normalized rotary rest is mask `0`,
CW is `[2,3,1,0]` and CCW is `[1,3,2,0]` (mask bits A=0, B=1). The
runtime owns debounce, gesture classification, ownership and event production. Current
`ScriptedInput` snapshots and decoder-only helpers are useful unit tools but do not substitute
for buffered-edge Runtime coverage. Existing `tests/e2e-host-sim` dispatcher/session coverage
also remains distinct from complete Runtime-to-desktop execution.

The virtual panel displays firmware `CaptureDisplay` pixels; Device Studio's separately rendered
preview may be compared, but must not be labelled firmware execution. Transport ownership and shutdown extend beyond the `SerialLink` trait: the first slice uses
a dedicated sandbox owner. Shipping device-task discovery remains unchanged.

Simulation must select fake platform services before constructing the active runtime, use a
separate settings location and never fall back to actual keyboard/media/audio/microphone/launch
operations. Flash/install/eFuse operations use explicit fake executors or are unavailable; no
real hardware subprocess, updater network request or installation can originate from the virtual
session. Keep real USB connection and simulator connection mutually exclusive initially. Display
clear simulated-device and simulated-OS labels. Production bundles exclude the bridge, runner,
controls and fake service selection, proven by release guards and positive controls.

Interactive mode uses real elapsed time for firmware and desktop. Headless cases inject explicit
firmware time and seeds and control only the clocks their harness actually exposes. Firmware-only
time advancement cannot prove desktop `Instant` deadlines, OS scheduling or physical latency.

## Pending documentation coverage

The pre-S5 audit found 205 unchecked entries: 34 in `roadmap.md` and 171 in `validation.md`.
These include repeated references, not 205 separate missing features. Counts below are that dated
snapshot, before adding S5. Each phase retains all its physical rows; simulation adds software
evidence alongside them, not substitute PASS marks.

| Validation phase | Open entries | Software behavior to exercise | What simulation cannot establish |
|---|---:|---|---|
| 1: Connection | 20 | Handshake/rejection, heartbeat, reconnect, restored state, semantic activity and privacy. | Real USB enumeration/stalls, native window/process/tray behavior and measured connection timings. |
| 2: Mascot, reactions, flashing | 9 | Firmware scenes, reactions, interruptions, overlays and animation frames. | Physical panel appearance, tearing/scan-out and successful flash. |
| 3: Rotary volume | 17 | Quadrature, invalid edges, bounds, read-back truth, endpoint change, stale gesture rejection. | Actual switch/wiring fidelity, Windows mixer and detent-to-panel latency. |
| 4: Push switch/actions/views | 31 | Press/hold/double thresholds, recovery state, action certainty, unknown values and view transitions. | Physical reboot, native action/permission delivery, real metrics/clock drift and human timing comfort. |
| 5: Context buttons | 8 | Ownership, hold suppression, labels, outage rejection and reboot request policy. | Crystal/strapping checks, power-on BOOT behavior and physical label alignment. |
| 5b: Config UI | 10 | Save/reopen/reset, labels, macro order/cancel/delete protection, capability restrictions. | Installed-upgrade retention, actual app mixer changes and shortcut destination. |
| 6: Profiles/layout | 7 | Focus/protected state, pin cycle, labels and causally correct faces. | Native UAC/focus detection, desk-distance readability and physical alignment. |
| 7: Host presence | 12 | Injected lock/suspend/resume/console events, stale input rejection and state restoration. | Actual notification registration/delivery, power modes, USB power and OS user switching. |
| 8: Firmware recovery | 8 | Production workflow decisions with fake flasher: failure, progress, restore, advice and exclusion. | Real sidecar stderr, interrupted flash, erased-chip or BOOT recovery and installed resources. |
| 9: Login/tray | 6 | Status/settings logic; device state for tray presentation. | Real sign-in, startup registration, native tray and uninstall cleanup. |
| 10: Installer/release | 7 | Device readiness after a modeled install; use separate packaging/VM checks for installation. | SmartScreen, Defender, privilege prompts, signing, bundled helpers and Automation permission. |
| 11: Onboarding | 5 | Known usable sessions, capability guidance, completion/reopen/persistence with isolated config. | Native permission prompts, installed migration and actual fresh-user environment. |
| 12: Per-unit QA | 3 | Protocol/identity checks, scripted input reporting and redacted log format with a test harness. | Real flash, physical operator inputs and per-unit manufacturing PASS. |
| 13: Hero features | 18 | Modeled mic coverage/hotplug/errors, profiles, autonomous animation, brightness request, diagnostics privacy. | Real mic/call observations, backlight, chip/performance measurements and cold/login startup. |
| 14: Identity/updates | 10 | Legacy/known/unknown identity, wrong-target refusal, offers, ordering, skip and failure decisions. | eFuse writes, deployed authenticated update/install, real network-off observations and physical restore. |

The 34 roadmap references comprise M0 (1), M1 (2), M2 (3), M2.5 (2), M2.6 (2), M2.7 (3),
M2.9 (5) and M3 (16). Earlier milestone hardware/gate references map to the phases above. Gate
invariants such as no false confirmation, stale replay, context mutation, hidden fallback or
invented state can receive modeled regression evidence. Actual per-user separation, physical
recoverability and authenticated installation still need their OS/hardware/release checks.

The additional M2.9 open acceptance records and M3 dependencies stay separate:

| Pending record | Useful virtual/software work | Remaining acceptance |
|---|---|---|
| Public website configuration | Existing local API/D1 rehearsal; independent web integration tests. | Public Access policy, real Turnstile/contact mailbox, approved deployment, signup and deletion. A device simulator does not solve this. |
| Windows unit | Rehearse all control/state/fault cases and record modeled outcomes. | Actual Windows unit, driver/OS state, microphone coverage and measured physical timings. Owner's deferred run remains deferred. |
| Installation/recovery | Fake executors and workflow integration; separate unsigned Windows VM/packaging checks. | Trusted signed artifacts, real updater, installed sidecar, interrupted flash and BOOT recovery. Publisher signing remains owner-deferred; updater signing is separate. |
| Sustained use | Deterministic traffic/sequence/time rollover and interruption stress, with explicit clock scope. | Eight actual idle hours and ten actual Windows workdays. Accelerated virtual time earns no hours/days. |
| Unfamiliar user | Optional virtual UI/instructions study, explicitly scoped to virtual interaction. | Uncoached participant using/installing/recovering the physical unit; subjective gesture feel. |
| M3 production readiness | Rehearse provisioning policy and documentation flows with fake executors. | Physical assembly, real QA/provisioning/eFuse, BOM/suppliers/cost, packaging, regulatory review, signing, building/shipping units and all shipping gates. |

CAD/enclosure reference measurements are owner-managed and excluded. The BOM's OWNER cells,
release key/certificate setup and unimplemented macOS notarization are not device-simulation
tasks. M4/Later items (new controllers/inputs, A/B/secure boot, native USB, integrations, extra
platforms, multiple devices, reset, haptics and external sources) remain future scope. Simulation
may test their logic once implemented; do not add them or relax their physical/security gates now.
The ADR-0017 identifier guard's documented limits still require behavioral tests for capability
mixes and unnamed layout assumptions; test fixtures must not become shipping catalog products.

## Implementation and evidence order

The six authoritative checkboxes are [M2.9 S5](./roadmap.md#s5-isolated-virtual-pip):
runner/vertical slice, connection faults, raw input, simulated desktop behavior, recovery rehearsal,
then repeatable reports/CI and final boundary proof. Implement and report one item at a time.
The first slice includes safe fake services and initial release exclusion; safety is not deferred
until the final CI task. Ask for review of the working slice before expanding scenario tooling.

Each implemented scenario has explicit expected actions/state/frame assertions, not just a demo
or screenshot. Record source SHA/build, scenario id, seed, clock mode, raw inputs, expected/actual
outcomes, exercised layers and trace/frame references. Failures fail the command/CI job. Mark
unexercised layers NOT RUN. Replaying the same inputs, injected time and seed must produce the
same firmware action trace and frame hashes. Interactive desktop timing is reported separately.

Use affected tests first and retain the no_std, offline, capability/product-identifier and release
guards. Do not add a new dependency, common crate or framework unless the implemented slice
demonstrates the need. The owner authorized S5 implementation after reviewing this plan. No external deployment is
authorized by simulator testing; report each item before proceeding.

## Running the first slice

From the repository root, with Bun and the pinned Rust toolchain installed:

```sh
just virtual-device          # owned Vite server + dedicated dev window + separate Pip runner
just virtual-device-smoke    # headless real core path; writes a firmware RGB565-derived PPM
```

Port 1420 must be free; the tool binds its own Vite server and never attaches to an unknown one.
Closing the window stops the native owner, reaps the simulator and closes the owned server. The
launcher clears inherited firmware/flag overrides, selects the Rust host target and uses separate
runner build output. macOS uses a local unsigned development .app under ignored target output;
it is not installed or a signed release candidate. Linux/Windows smoke is wired into CI but local
interactive verification was on macOS only.

The UI labels simulated device and OS, displays actual firmware pixels and shows fake volume/media
results. `Press once` schedules 120 ms raw down/up; Hold accepts pointer or Space/Enter and Release
sends raw up. A 2 s hold failsafe prevents a missed browser release from keeping the knob held.
Only in-memory settings are used. Do not count this as normal-app configuration/setup coverage.
The runner pipe contract and explicit-host commands are in its
[README](../firmware/pip/simulator/README.md). No new framework, common simulator crate or runtime
network transport was added.

## Connection and fault controls (S5.2)

The development window adds Disconnect, Reconnect and a fixed Fault mode selector. Reconnect
always starts a fresh normal child/session, retaining fake OS state but dropping old wire, raw
edges, holds, frames and session-bound actions. Controls are disabled while disconnected and every
raw-control IPC carries its originating connection generation. A delayed old UI event cannot act
on the new connection; stale snapshots cannot repaint a completed connection operation.

Available faults: `fragmented` (seven-byte per-tick traffic budgets), `wrong-major`,
`unknown-product`, `unknown-board`, `silent` (suppress wire output while firmware keeps running),
and `exit` (actual child EOF). Identity/protocol modes start a fresh child; silent/exit require an
existing usable connection. These controls remain outside the production protocol and release UI.
Use Reconnect after a simulated error/reboot; only clock rollover still requires restarting the tool.
The smoke command includes nine connection scenarios with real desktop time and bounded deadlines;
see [results and CI evidence](./validation.md#s52-connection-and-interruption-evidence).

## Raw input scenarios (S5.3)

The product-owned runner adds `--input-smoke`, also run by `just virtual-device-smoke`. It drives
raw timestamped levels through the actual firmware runtime and checks complete wire-event traces,
current session, captures and recovery lifecycle. Coverage and limits are recorded in
[S5.3 evidence](./validation.md#s53-raw-input-fidelity-evidence). No new desktop command or
production wire message is added. Normal invocation still serves bounded newline-delimited JSON.
Firmware milliseconds are injected; desktop deadlines and physical timing are not simulated by
this mode. Input-port overflow injection is explicitly distinct from a physical ISR queue test.

## Simulated desktop scenarios (S5.4)

The native development example's `--desktop-smoke` runs production desktop policies with existing
fake OS backends and a CLI-owned disposable configuration directory. The normal development window
retains memory-only settings. Raw firmware/session cases and direct desktop action tests are
labelled separately; setup UI is checked with the existing browser mock rather than presented as
a native IPC-to-runner path. See [S5.4 evidence](./validation.md#s54-simulated-desktop-behavior-evidence)
for verified layers and limits. The smoke launcher includes these cases; production releases must
omit the `PASS S5.4` marker, with a positive control in the opt-in example.

## Recovery and compatibility scenarios (S5.5)

The native development example adds `--recovery-smoke`; the launcher runs it after the desktop
cases. Production flash workflow, identity/image selection, version-offer and updater/admission
policies are rehearsed with counting in-process fake executors and local fixtures. An actual
virtual reconnect checks post-flash compatibility admission, while the runner's firmware binary
remains unchanged. No actual flash, eFuse, download or installer path is invoked.
See [S5.5 evidence](./validation.md#s55-recovery-and-compatibility-rehearsal-evidence) for the
separate policy/session/UI layers and limits. The `PASS S5.5` marker must be absent from production
and observable in the explicitly enabled example.

## Repeatable reports and replay (S5.6)

`just virtual-device-smoke` prints a new `target/reports/run-*/report.json` location below this
product's simulator. It retains command logs, captured firmware pixels, source/build fingerprints
and layer/clock labels. Seeds 17 and 29 each replay four raw recipes twice and must match committed
action/frame expectations and repeat byte-for-byte. Desktop/session checks keep real elapsed time.
The report's `passed` status covers its declared stages; offline, production and no_std proof comes
from the separate CI checks listed in it. See [S5.6 evidence](./validation.md#s56-repeatable-reports-ci-and-boundary-evidence).

Each product owns its replay recipes and golden fixture. A deliberate renderer change may require
a reviewed fixture update after checking intended pixels and actions; the verification command
does not accept or regenerate new goldens. CI retains reports even after smoke failure, while a
job that fails before smoke starts may have no report. Physical and real-use acceptance stays open.

## Interactive macOS pass (2026-10-30)

Driven with synthetic clicks on the dev window at the M2.9 PR #127 head; complements, does not
replace, the headless smoke (which also passed locally on macOS: 70 PASS lines, all 15 report stages,
seeds 17/29 byte-identical). Observed: three right detents 50→56% and one left →54% with the
firmware volume bar; Press once → Play/Pause (media triggers +1); Hold shows the PRESS/HOLD hint, and
release toggles output mute with a confirmed check (firmware muted icon, fake OS muted), a second
hold unmutes. Faults: `silent` → Unavailable with "virtual heartbeat timeout" and a cleared panel;
`exit` → "virtual child closed its output"; `wrong-major` → "device protocol major v1 is unsupported
(host supports v0)"; `unknown-product`/`unknown-board` → "virtual product or board identity is
unknown"; each Reconnect started a fresh normal session (generation advanced to 9). Closing the
window and SIGTERM to the owner both left no runner, simulator or Vite process. `fragmented` input
was not exercised interactively (the run was stopped). The panel now shows the fake output mute,
and disabled controls are visibly dimmed.

## Host presence scenarios

Added 2026-10-30 at the owner's request, after S5. The native example's `--presence-smoke` (stage
`presence` in `just virtual-device-smoke`, markers `PASS PRESENCE`) feeds host events through the
production `HostPresenceTracker` and `apply_presence_action`. Releasing the link keeps the runner
process alive with no session reading it, like a unit that stays powered over USB; rediscovery
reattaches it with a fresh handshake, or boots a new runner when the scenario cuts its power. The
runner now reports the firmware companion `state` with each response, so Sleeping and Offline are
asserted directly rather than inferred from pixels.

Cases (real elapsed time): lock/unlock, sleep/wake with input made while asleep, console switch
without a goodbye, USB power cut during sleep, and a stray wake. Expectations follow validation
[Phase 7](./validation.md#phase-7-host-presence-m3-s2) 7.1–7.4, 7.6 and 7.7. Mutation checks: skipping
the sleep goodbye or the lock override fails the stage. Not simulated: OS notification registration
and delivery, the Windows suspend acknowledgement budget, real USB power and measured timings.

## Endurance run

`just virtual-device-endurance` (opt-in, about 7 minutes on an Apple Silicon Mac; not in CI) keeps
one virtual session turning the knob right then left until both protocol sequence numbers have
wrapped, observed through a feature-gated `Session::sequence_positions`, then requires every detent
delivered, no sequence gap, malformed frame, device error, stale rejection or reconnect, volume back
at 50 and a working press. The first attempt stopped at a fixed detent count and found only the
device side had wrapped (the host sends about 0.6 frames per detent), so the run now continues until
both wraps are seen. The 2026-10-30 run passed with 107,684 detents in 432 s. It exercises protocol
and session longevity only; it earns no soak hours and does not cover OS sleep, USB or memory growth
on the device.
