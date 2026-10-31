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

`just virtual-device-endurance` (opt-in, about 14 minutes on an Apple Silicon Mac, on modelled timing; not in CI) keeps
one virtual session turning the knob right then left until both protocol sequence numbers have
wrapped, observed through a feature-gated `Session::sequence_positions`, then requires every detent
delivered, no sequence gap, malformed frame, device error, stale rejection or reconnect, volume back
at 50 and a working press. The first attempt stopped at a fixed detent count and found only the
device side had wrapped (the host sends about 0.6 frames per detent), so the run now continues until
both wraps are seen. The 2026-10-30 run (ideal timing) passed with 107,684 detents in 432 s; the
2026-10-31 run on modelled timing passed with 201,286 detents in 805 s (host 1 / device 3 wraps). It exercises protocol
and session longevity only; it earns no soak hours and does not cover OS sleep, USB or memory growth
on the device.

## Hardware parity

The virtual runner executes the same firmware `Runtime` as the device, so behavior above the
`InputSource`, display and transport ports matches once both run the same firmware build. Keep the
device on repository firmware (`espflash flash --port PORT` with the product build of
`firmware/pip`); an unknown build cannot be compared.

Which firmware a unit runs is reported, not guessed: the device and the virtual Pip both send a
firmware source id (a hash of the committed firmware input files, `BuildInfo`), and the Device page
shows it as "Firmware source". Developer mode warns when it differs from the app's own ("Firmware
source differs"), and everyone is warned when it was built with uncommitted changes. Identical inputs
give an identical id, so a unit flashed from another branch or a squash-merged commit with the same
firmware files still matches. See [architecture.md](architecture.md#firmware-build-id-build_info_v1-adr-0014).

Facts measured on hardware and pinned in host tests:

- Knob direction (2026-10-31): the HW-040 changes CLK first on a right turn. `input::hw040` maps
  DT to channel A and CLK to channel B, so a right turn is `Cw` on the device exactly as a virtual
  `clockwise` (`[2,3,1,0]`) is. Test: `hw040_right_turn_is_clockwise_and_left_turn_is_counter_clockwise`.
- Panel orientation (2026-10-31): the picture was upside down at `Rotation::Deg90`, so the ST7789 runs
  at `Deg270`, unmirrored (`profile::physical_st7789::orientation`). Test:
  `panel_orientation_is_deg270_unmirrored`. The simulator cannot show this, so a change needs the unit.

Comparing a real unit with the virtual one, using the production session (`device_probe`):

| Command | Shows |
|---|---|
| `device_probe PORT session` | handshake and 8 s of heartbeats |
| `device_probe PORT desk-full` | everything the app sends after connecting, with link health |
| `device_probe PORT desk-burst` | the app's connect burst in one instant |
| `device_probe PORT inputs` | each knob/button event (`KIVORI_PROBE_SECS` sets the duration) |
| `device_probe PORT internals` | the device's load figures once a second, with an average |

The desktop logs `device link lost` with its cause (heartbeat timeout, I/O error, port removed),
so a reconnect loop on hardware names its reason. `device_probe PORT internals` samples the device's busy %, compose and flush time and fps once a
second; compare it before and after a rendering change on the unit, since host timings do not
predict the ESP32-C3 (no 64-bit divide, flash-resident assets). Without a unit,
[Frame cost without hardware](#frame-cost-without-hardware) predicts the compose time.

Not covered by the simulator: pin wiring and electrical bounce, the USB CDC transport and its
buffering, SPI/panel orientation and measured frame timing (it is modelled, see
[Modelled timing and internals](#modelled-timing-and-internals)).

### Resource budget without hardware

`just fw-budget` builds the catalog product firmware (`bun scripts/build-firmware.mjs`) and runs
`scripts/firmware-budget.mjs` on the ELF; the firmware CI job runs the same two steps. It
checks `firmware/pip/resource-budget.json`:

- app image size, the asset blob against its cap (`ASSET_BLOB_CAP_BYTES`, read from `internals.rs`),
  static RAM (`_stack_end` minus the start of data RAM) and the stack region;
- the stack frame of every function, summed over the whole prologue (a large frame is
  `lui; addi; sub sp, sp, rX`, not a single `addi`); a frame over 1 KiB must be listed in
  `large_frames` with a ceiling and a reason;
- the worst stack depth over the direct call graph from `_start_rust`, with tail jumps counted
  without the caller's frame. Calls through a register (trait objects, fn pointers) and interrupts
  cannot be followed, so the budget adds a documented indirect-call allowance and the traced
  interrupt path (`interrupt_path`) plus a margin. Peak plus margin must fit the stack region;
- an allowlist of the functions that may call the 64-bit division libcalls (`__udivdi3`, `__divdi3`,
  `__umoddi3`, `__moddi3`, a software loop on this core). A new caller fails, so a 64-bit divide in
  the per-pixel path is caught without a frame-rate measurement.

Validation against the unit (after PR #150): the derived peak is 115,744 B, the device measured
115,740 B (stack region 162,180 B minus the lowest free stack, 46,440 B), at boot in `load_assets`.
The estimate reaches the figure only because that path is all direct calls; paths that go through a
register are covered by the allowance, not by analysis. Inlining can move a call to another function
name, which then needs an allowlist update. The script needs `llvm-objdump` (the rustup
`llvm-tools` component, or `LLVM_OBJDUMP`) and takes `--json` and `--elf PATH`.

To change a limit on purpose, edit `firmware/pip/resource-budget.json` in the same change as the
code that needs it and write the reason in that entry's `why`; reviewers see the new number and the
reason together. After a size or stack change, run `just fw-budget` and copy the new figure with a
small margin; do not raise a limit just to make a failing check pass without explaining the growth.
Re-measure `device_probe PORT internals` on the unit when the stack peak moves.

### Frame cost without hardware

`just fw-frame-cost` predicts the ESP32-C3's compose time per frame without a unit, and the firmware
CI job `frame-cost` runs the same check. It executes the real renderer, not a model of it:

- `firmware/pip/frame-cost/bench/` is a `no_std` binary built for `riscv32imc-unknown-none-elf` with the
  product's release profile (`opt-level = 3`, `lto`). It calls the same
  `TileRenderer::prepare_frame_row` once per tile row into a static frame buffer, as `Runtime::step`
  does, over seven scenarios (below), with the asset blob in a DROM-mapped section and code in IROM
  (`0x4200_0000`, `0x3C00_0000`, RAM `0x3FC8_0000`). Marker stores to an unmapped window tell the
  simulator where each row, frame and scenario starts and ends.
- `firmware/pip/frame-cost/` (own host workspace, like `../simulator`; ADR-0018) is an RV32IMC
  instruction-set simulator: it loads the ELF and executes every instruction (compressed decode,
  M extension, CSRs, ROM/flash/SRAM map) and charges cycles by the model below. Tests in
  `tests/iss.rs` pin instruction semantics against `llvm-mc` encodings, including divide by zero,
  `MIN / -1`, the `mulh*` family and every compressed form the bench uses.
- `tests/native_parity.rs` renders the same scenes natively and requires the simulated frame buffer
  to match bit for bit after every frame (104 frames). The simulator therefore runs the renderer
  correctly; it also confirmed that the pre-#150 renderer draws identical pixels.

Cycle model (`src/model.rs`; every number is an assumption except the one fitted, because Espressif
publishes no per-instruction latencies; it is a 4-stage in-order single-issue core):

| Rule | Cycles beyond the base 1 | Why |
|---|---|---|
| load result used by the next instruction | +1 | load data arrives a stage later than ALU results |
| taken branch / `jal` / `jalr` | +2 / +1 / +2 | redirect in EX, no predictor assumed; `jal` target is known at decode |
| `mul*` | +2 | 3-cycle multiplier (Ibex "slow" class) |
| `div*`, `rem*` | +34 | 35-cycle serial divider; flat, including divide by zero |
| load hitting the flash cache (DROM) | +1 | cache read takes a second stage |
| flash-cache miss (code or data) | +170 | 32-byte line over 80 MHz quad SPI is about 84 flash clocks; a 40 MHz dual-SPI flash would be about 600 |
| cache | 16 KiB, 8-way, 32-byte lines, LRU, shared by IROM and DROM | datasheet: 16 KB cache; ways and line from ESP-IDF's C3 options |
| everything above except misses, times | 2.064 | the one fitted number: see Accuracy |

Scenarios (device time sampled across an animation cycle; per-row figures in the fixture):
`idle_mascot`, `connected_desk` (buddy plus clock, indicators and control legend: what the app shows),
`media_view`, `system_view`, `clock_view`, `buddy_overlay` (reaction and state change in flight,
volume overlay, badge, live mic, pressed knob) and `switch_buddy_system` (the view-switch slide with
both buddy and System drawn: the heaviest frame the device draws).

`firmware/pip/frame-cost/fixtures/frame-cost.json` is the committed output and the input of the
virtual Pip's clock: for each scenario, `row_ms_mean[r]` and `row_ms_max[r]` are the compose time of
tile row `r`, `row_cycles[frame][row]` replays a frame sequence, and `frame_ms` is mean/min/max per
frame; `meta` records the model version, every parameter, the calibration and the scope. Predicted
compose time (`connected_desk` rows 0 to 5, ms): 4.3, 8.6, 20.8, 23.8, 14.6, 7.0.

| Scenario | mean ms | worst frame ms |
|---|---:|---:|
| `media_view` | 30.6 | 30.7 |
| `clock_view` | 50.6 | 50.7 |
| `buddy_overlay` | 67.7 | 68.1 |
| `idle_mascot` | 73.4 | 73.6 |
| `connected_desk` | 79.0 | 79.3 |
| `switch_buddy_system` | 91.2 | 97.7 |
| `system_view` | 97.4 | 97.5 |

#### Accuracy

Two measurements on the owner's unit (ESP32-C3 rev v0.4, 160 MHz):

| Renderer | Measured compose | Predicted | Error |
|---|---:|---:|---:|
| after PR #150 (opt-level 3), app connected, desk view with the mascot | 79 ms | 79.0 ms (fitted) | 0 |
| before PR #150 (commit `20a4b5c`, opt-level `s`, two 64-bit divides per sprite pixel) | 339 ms | 304 ms (not fitted) | -10% |

Only the global factor was fitted, to the first row. The second is a prediction: the bench was built
from a worktree of `20a4b5c` with `opt-level = "s"` and its whole-frame `prepare_frame` (that
commit has no per-row call), and run unchanged. Without the factor the model predicts 41 ms and
150 ms for the two points: a ratio of 3.7 against the measured 4.3, so the model ranks code changes
sensibly, but absolute cycles are about twice what the rules add up to (flash-cache instruction path,
SRAM bus effects and anything not modelled individually are folded into the factor). How much the
assumed miss penalty matters: refitting the factor for penalties of 0, 85, 170, 340 and 600 cycles
moves the old-renderer prediction to 322, 313, 304, 287 and 261 ms (-5% to -23%), so every plausible
flash setting stays inside the 25% target.

Limits and uncertainty:

- Row costs, scenario mixes and the 79 ms fit are validated only in total. Whether row 3 costs 23.8 ms
  on the unit is not measured; the model says where the work is, `internals` only gives the sum.
- The product links the ROM's `memcpy`, `memset` and 64-bit divide (`esp32c3.rom.*.ld`); the bench links
  `compiler-builtins` copies, whose cost differs. They are 3% of the current frame, but 23% of the
  pre-#150 frame, so the old-renderer prediction carries that uncertainty (a slower ROM divide would
  move it toward 339 ms).
- Not modelled: SPI flush and its DMA, USB, GPIO interrupts, the work the run loop does between rows
  (the device measurement leaves it out as well), flash-cache state disturbed by that work (the
  simulation keeps the cache warm across rows and frames), and data-dependent divider time.
- The model is a predictor for regression gating and for relative cost, not a substitute for
  `device_probe PORT internals` on the unit after a rendering change.

#### Gate

`firmware/pip/frame-cost/run.sh check` (`just fw-frame-cost`): builds the bench and simulator, runs
their tests and the native-parity test, then fails if any scenario's mean or worst frame is more than
10% above the fixture, or a scenario is missing or new. The result is deterministic; the 10% absorbs
code-generation drift between stable Rust releases (CI uses the current stable). A 64-bit divide back
in the sprite path shows as +285% on `connected_desk`. Runtime: about 20 s locally once built (the
simulation takes 8 s); the CI job builds two small workspaces from cold, so allow a few minutes in
parallel with `no-std-isolation`.

To accept an intended cost change: `just fw-frame-cost-update`, review the diff of
`fixtures/frame-cost.json` and commit it with the reason in the message. A faster result is reported
as a note (not a failure); refresh the fixture to keep the gate tight. Changing a cost rule in
`src/model.rs` requires bumping `MODEL_VERSION`; the check refuses a fixture of another version.

#### Recalibrating

1. On the unit, flash the repository firmware and run `device_probe PORT internals`; note the average
   compose ms in the state to calibrate (app connected, desk view, mascot animating).
2. `cd firmware/pip/frame-cost && ./run.sh run` builds the bench and ELF
   (`bench/target/riscv32imc-unknown-none-elf/release/kivori-frame-cost-bench`); then
   `target/<host>/release/kivori-pip-frame-cost calibrate --elf <ELF> --scenario connected_desk --target-ms <ms>`
   prints the `scale_milli` that reproduces it (add `--miss-penalty N` to try another flash setting).
3. Put the value in `FITTED_SCALE_MILLI` (`src/model.rs`), update the calibration text in `src/main.rs`
   and the table above, run `just fw-frame-cost-update`, and keep the second data point
   (`run --elf <old ELF>` on a build of an older renderer) as a check; a prediction more than 25% off
   means a cost rule needs revisiting before the fixture is trusted.

### Modelled timing and internals

The virtual Pip runs the same `Runtime::step` as the device, but its clock used to stand still: the
runner set it once per request, so inside a frame every read returned the same time. Compose and
flush measured 0 ms, `busy` was about 0 and the frame rate followed the host's request pacing. Input
and host bytes stamped "during" a frame reached the runtime at once, where the unit services them
between tile rows. With modelled timing the virtual Pip has the unit's time behaviour and fills its
Device internals like the unit does, so rendering and responsiveness work needs no hardware. It
remains a model: [Limits](#limits-of-the-model) says what it does not know.

#### Two timings

| Mode | Used by | One request is |
|---|---|---|
| Modelled (default) | `kivori-pip-simulator` with no arguments: the desktop bridge and `just virtual-device` | "run the device until `now_ms`": any number of ticks, on the device's own clock |
| Ideal (`--ideal-time`) | in-process `Simulator::new()`: `--input-smoke`, `--io-smoke`, `--replay`, the runner tests and the committed frame goldens | exactly one tick with the clock frozen at `now_ms`, frames cost 0 ms |

Ideal timing is kept for everything whose assertions or goldens depend on a tick being one instant
(exact event timestamps, frame hashes at a given `now_ms`). Modelled timing is for looking at the
device as a user would. The desktop, connection, recovery and presence scenarios of
`just virtual-device-smoke` and `just virtual-device-endurance` run on modelled timing and pass.

The endurance run first looked as if it needed ideal timing: a modelled run was reported to lose its
heartbeat after about 30 s (5,700 detents), and the suspect was the runtime's inbound limit,
`RX_BYTES_PER_POLL` (512 bytes a tick, about 6.4 KB/s at 12.5 frames a second). That failure did not
reproduce at this commit: runs of 198 s, 109 s with 8 busy loops, 40 s with 32 busy loops and a full
805 s run (201,286 detents at 250 a second) all kept the heartbeat, with a round trip of at most 94 ms
(2 s is the host's allowance), no inbound backlog and every detent delivered. What the limit does
is measured without wall-clock noise by `a_detent_flood_with_presentations_back_keeps_the_heartbeat_inside_its_allowance`
(virtual time only): 200 detents a second with a presentation for each (5.6 KB/s inbound) is
answered within 70 ms, while 2 presentations a detent (11 KB/s) is not (a 6.5 s round trip and
growing). The endurance flood makes the host send 0.3 to 0.65 frames per detent (the host counter wrapped
once in 201,286 detents), about 2 to 4.5 KB/s, so it sits
below the ceiling; a fast human spin (50 to 100 detents a second) is far below it. The cause of the
earlier failure was not found; the likeliest explanations are a host stall or an earlier state of the
model, and it is treated as unexplained rather than as a device limit.

#### How time is modelled

All of it lives in `firmware/pip/simulator/src/timing.rs` and `modelled.rs`. The product firmware has
one new line for it: `Runtime::step` calls `Clock::row_composed(row, view, overlay)` after each tile row,
and that method exists only with the `host-sim` feature (the default does nothing). Device builds do
not contain it; the release-surface check that compiles `host-sim` for riscv32imc as a positive
control is unchanged.

- **Compose.** After each tile row the clock advances by that row's cost from
  `firmware/pip/frame-cost/fixtures/frame-cost.json` (`row_ms_mean`, the RV32IMC simulation of the real
  renderer, fitted to the unit's 79 ms). The scenario is picked from what the runtime is drawing:

  | The frame shows | Scenario |
  |---|---|
  | Buddy, desktop data present (legend, clock or now-playing) | `connected_desk` |
  | Buddy, nothing from the desktop | `idle_mascot` |
  | Buddy with a volume overlay, action feedback or a held switch | `buddy_overlay` |
  | Clock, Media or System view | `clock_view`, `media_view`, `system_view` |
  | The switch slide between Buddy and System | `switch_buddy_system` |
  | Any other slide | the dearer of the two views (an approximation) |
  | Volume view, recovery or update takeover | `connected_desk` (not measured) |

- **Flush.** Each panel window advances the clock by its SPI time: `(pixels x 16 + 11 x 8) bits at
  20 MHz` (`SPI_CLOCK_HZ`; 11 command bytes for the column and row address and the memory write).
  Which windows go out is the real renderer's choice (changed tiles only, a full tile row or two as one
  band), so a quiet desk flushes in under a millisecond and a full redraw takes about 46 ms. DMA
  set-up and the CPU time around a transfer are not modelled.
- **Service points.** The runtime drains input and USB between tile rows and between windows. Each
  such point costs 150 microseconds (`SERVICE_POINT_US`). That figure is an assumption fitted to one
  fact: the unit reports 12 fps with a 79 ms compose, which needs a frame of at least 80 ms; without it
  the model reports 13. Compose and flush figures exclude it, as the runtime's own accounting does.
- **Idle.** A tick that took no modelled time advances the clock 1 ms, the resolution of input
  timestamps; the real loop spins, the host cannot.

The device runs back to back and never sleeps, as on the board. A request only says how far the host's
time has come, so the device may stop part way through a frame. It runs on its own thread and parks at
every point where the runtime looks at input (the start of a tick, between tile rows, between windows)
until the host's time has reached it. Input edges and wire bytes are handed over only when device time
reaches their stamp, and wire bytes carry the request's `now_ms`. So an edge stamped mid-frame is
serviced at the next row boundary, not after the frame, and the result depends on the sequence of
requests only, not on thread scheduling: exactly one side runs at a time, and slicing the same
interval into different request sizes gives the same device figures (tested). Output such as a frame
capture can lead the host's time by up to one row or window.

#### Virtual figures against the unit

Connected desk (buddy with clock, volume, media and legend), as in `device_probe PORT internals`:

| Figure | Unit | Virtual Pip |
|---|---:|---:|
| compose | 79 ms | 79 ms |
| flush | 0 to 5 ms | 0 ms (under 1) |
| fps | 12 | 12 |
| busy | about 100% | 99% |
| input serviced while composing | about every 13 ms | 2 to 22 ms after the edge across 12 frame phases (a row is 4 to 24 ms) |

The unit's figures are the ones recorded with the frame-cost fit; the virtual row is what the tests
below assert (`modelled::tests`: compose within 1 ms of 79, fps 12, busy 99 or 100, flush at most 5 ms,
worst service delay at most 25 ms against 79 ms for a frame).

#### Internals

The runner installs a chip probe (`src/probe.rs`) so the Device internals hold the facts the board
would read from itself. They come from `firmware/pip/simulator/fixtures/resource-facts.json`:

| Field | Source |
|---|---|
| app image, asset blob (and its cap), static RAM, stack region | `scripts/firmware-budget.mjs --json` on the product ELF |
| stack free minimum | stack region minus the estimated peak (115,744 B; the unit measured 115,740 B) |
| RAM total | static RAM plus the stack region |
| app partition, total flash | the unit's layout: 4 MiB module, factory app 0x3F0000 B (not in the ELF) |
| chip | ESP32-C3 rev v0.4, the unit the model is fitted on |
| busy, compose, flush, fps, uptime, frame counters | the runtime itself, on the modelled clock |
| temperature, reset reason, live free RAM | no model: Unavailable, never 0 |

The chip row is the real unit's value rather than a virtual marker: `ChipModel` is wire-significant
and there is no wire field for "virtual". The desktop marks the virtual device instead. It identifies
the connection by the device id's short hash (`VIRTUAL-PIP-0001` hashes to `847c795d`) and shows
"Modelled (virtual Pip)" with a one-sentence explanation in the Device internals card
(`VIRTUAL_PIP_DEVICE_ID_HASH`; a test pins the constant to the id). Nothing was added to the wire.

The development window reuses the card. The `virtual_device` example registers `get_device_internals`
with the shipping command's name and shape; the sandbox projects it from its session with
`device_internals_dto`, and starts sending one `InternalsRequest` a second only once the card has
asked. `just virtual-device-smoke` (vertical slice) reads the figures back through that path.

Keeping the fixture current: `just fw-budget` ends with `bun scripts/virtual-resource-facts.mjs
--check`, and the firmware CI job runs it after the budget check. It builds nothing; it reads the
product ELF, derives the five ELF facts and fails when one differs from the fixture by more than 2%.
After an intended size or stack change run `just fw-virtual-facts-update` (rebuilds the ELF and
rewrites the `elf` block) and commit the fixture with the change. The simulator also asserts that the
fixture's asset blob size equals the blob it compiles. Compose cost is kept current by the frame-cost
gate above.

#### Limits of the model

- Compose per row is the frame-cost model's, validated on the unit only as a total
  ([Accuracy](#accuracy)). The scenario mapping covers seven situations; the others are
  approximations as in the table above.
- Flush is bandwidth only, and the 150 microsecond service cost is fitted to one integer. USB, interrupts
  and flash-cache disturbance are absent, so absolute latency to the host is optimistic.
- The state token and `reboot` in a response describe the last finished tick, so they can trail the
  host's time by one frame.
- Internals that need hardware stay unavailable. The app image size and stack peak describe the
  product ELF at the last fixture refresh, not the simulator binary.
- Ideal-timing scenarios say nothing about timing; use modelled timing (or the unit) for that.
- The development virtual window shows the Device internals card (the shipping component, with the
  "Modelled (virtual Pip)" note); the main app still cannot connect to the virtual device.
