# ADR-0018: Product-owned virtual device processes

- **Status:** Proposed (PR not merged); owner authorized implementation on 2026-10-26
- **Date:** 2026-10-26
- **Milestone:** M2.9 S5
- **Related:** [ADR-0001](./0001-two-cargo-workspaces.md), [ADR-0002](./0002-one-wire-protocol-crate.md), [ADR-0017](./0017-product-contract-and-capability-driven-desktop.md)

## Context

Pip already has a hardware-neutral production Runtime and host-sim adapters. Interactive virtual
hardware could connect real input/firmware/desktop behavior without a physical unit. Existing
dispatcher/session tests and desktop-rendered previews do not establish that complete path.
Future products need independent simulators without pulling their firmware into desktop builds
or depending on Pip's hardware assumptions. The no_std feature firewall and offline rules remain.

## Decision

Propose one product-owned simulator executable in its own host workspace beside that product's
firmware: initially `firmware/pala/simulator/`. It consumes the parent firmware with `host-sim`,
default features disabled, and existing shared crates. It must select the host target explicitly
rather than inherit the embedded target/runner/linker configuration. Do not rename the permanent
`pala` machine id. No runtime plugin system, shared simulator framework or second product is added.

Desktop uses a generic, explicit, development-only child-process pipe bridge. Bounded envelopes
carry simulator controls/captured frames separately from unchanged protocol bytes. Protocol
identity, version and capability validation still apply. Physical discovery and the shipping
catalog receive no fake devices. Desktop must not acquire a production dependency on firmware.

Raw timestamped levels feed `InputSource::drain` and the actual `Runtime`; semantic input events
are outputs, not the simulator's input shortcut. Display captures come from that runtime. Inject
fake OS services, isolated settings and fake flash/install executors before enabling interaction;
never fall back to real actions. Keep simulation visible in both UI and reports. Use real elapsed
time interactively and explicit injected time in headless scenarios; do not promise whole-app time
travel while desktop scheduling uses `Instant`.

## Consequences

The process boundary keeps product dependencies and toolchains independent but introduces pipe
framing, bounded queues and child lifecycle work. These are part of the first vertical slice,
alongside exclusion from release bundles and safe fake backends. Extend existing boundary,
offline and release-surface checks with positive controls; preserve standalone embedded builds.
Future products own separate runner/scenario implementations and share existing wire/model
contracts only; extract additional common harness code when a second implemented product needs it.

The [virtual Pip plan](../virtual-pip.md) maps pending checks and implementation order. Passing
simulation closes only explicitly scoped software tasks, never physical, signing, deployment,
real-use or participant acceptance. S5.1 is implemented in the review branch; broader scenarios remain planned. See the
[software evidence](../validation.md#m29-virtual-pip-software-acceptance). The ADR remains Proposed
until merge, following the ADR index policy.

## Alternatives considered

- UI-only mocks: useful for layout but skip native session and firmware behavior.
- In-process desktop firmware dependency: simpler for a demo, but couples desktop builds to
  product toolchains and features. Existing test-only linkage may remain in test harnesses.
- MCU/USB emulation or virtual COM drivers: more infrastructure than the initial software checks
  require, without proving the real electrical/driver behavior.
- Loopback TCP/WebSocket: unnecessary and conflicts with the existing native offline boundary.
