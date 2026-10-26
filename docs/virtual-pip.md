# Virtual Pip: proposed software verification plan

Status: proposed on 2026-10-26; owner review before implementation. This document plans M2.9 S5;
it does not claim a working interactive simulator or new test results. See
[ADR-0018](./adr/0018-product-owned-virtual-device-processes.md), the
[roadmap](./roadmap.md#s5-isolated-virtual-pip-proposed-owner-review-before-implementation) and
[planned acceptance](./validation.md#m29-virtual-pip-software-acceptance-planned).

## Scope and ownership

Start with Kivori Pip (permanent machine id `pala`), one simulator process and one selected
desktop connection. Keep the actual no_std firmware core and existing protocol/model/renderer;
replace hardware adapters, not product behavior. Future products own independent executables,
input models and scenarios. They must not import Pip's simulator. Shared contracts are the
existing crates; a universal simulator SDK, plugins and multi-device UI are out of scope.

Proposed paths, not existing implementation:

```text
firmware/pala/
  src/                         production no_std firmware; existing sim adapters
  simulator/                   separate host Cargo workspace and executable
    src/                       raw inputs, runner, capture and process control
    scenarios/                 Pip-specific deterministic cases
apps/desktop/src-tauri/src/     generic development-only child-process bridge
apps/desktop/src/               explicit virtual connection and controls in developer UI
```

Runner Cargo commands must name their manifest and host target, with separate target output and
without inheriting Pip's embedded runner/linker flags. Product release recipes remain unchanged.
The bridge selects an explicit local runner; no arbitrary shell command text, network listener,
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

Inputs use timestamped raw levels, including quadrature transitions and button down/up. The
runtime owns debounce, gesture classification, ownership and event production. Current
`ScriptedInput` snapshots and decoder-only helpers are useful unit tools but do not substitute
for buffered-edge Runtime coverage. Existing `tests/e2e-host-sim` dispatcher/session coverage
also remains distinct from complete Runtime-to-desktop execution.

The virtual panel displays firmware `CaptureDisplay` pixels; Device Studio's separately rendered
preview may be compared, but must not be labelled firmware execution. Device-task discovery,
transport ownership and shutdown need integration beyond the `SerialLink` trait.

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

The six authoritative checkboxes are [M2.9 S5](./roadmap.md#s5-isolated-virtual-pip-proposed-owner-review-before-implementation):
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
demonstrates the need. Documentation review authorizes no implementation or external deployment.
