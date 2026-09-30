# Kivori

Kivori is a physical desktop companion that lets people control their computer through tactile input while representing important desktop state on a dedicated display.

> **Product thesis:** Control the desktop physically. Understand the desktop visually.

## Project status

**Feature 001 — Device Connection Foundation.** Established the connection, protocol, deterministic rendering, Device Studio, firmware simulation, and ESP32-C3/ST7789 runtime foundation. Software complete; remaining physical acceptance items live in its validation ledger.

**Slice 002 — Rotary Volume Control Loop.** Implementation complete and green on CI. Turning the HW-040 knob changes Windows master volume, and the device displays the volume Windows actually reports — the first slice that delivers the product thesis rather than only the link beneath it. **Physical validation is outstanding:** all 15 rows of its checklist are blank, including the measured detent→feedback latency gate, so the slice is *not* closed.

**Feature 003 — Mascot animation and activity log** ([PR #3](https://github.com/Vellixia/Kivori/pull/3), merged). Expressive, interactive mascot shared by Device Studio and the device, social reactions over the wire (`MASCOT_INTERACTION`), bundled firmware flashing from Device Studio, and a typed session-only activity log. Manual on-device check outstanding.

The product contract extends beyond these. Product behavior is defined by the PRD and User Story Contract. Technical research records possibilities and uncertainties; accepted durable technical choices belong in ADRs.

## Hardware

One ESP32-C3, one ST7789 240×240 panel, one HW-040 rotary encoder. USB provides power, flashing, and the product data link — no external UART bridge.

### Pin map

| Signal | GPIO | Notes |
|---|---:|---|
| SPI SCK | 6 | SPI2, 20 MHz, Mode 3 |
| SPI MOSI | 7 | |
| Display D/C | 2 | strapping pin, driven after boot |
| Display RST | 3 | |
| Backlight | 8 | active-high; strapping pin, must be high at reset anyway |
| Encoder CLK (A) | 4 | |
| Encoder DT (B) | 5 | |
| Encoder SW | 10 | |
| USB D− / D+ | 18 / 19 | native USB Serial/JTAG, `0x303A:0x1001` |

Encoder COM to **GND**, VCC to **3V3**. **Not 5V — ESP32-C3 GPIOs are not 5 V tolerant.** Firmware enables internal pull-ups and reads all three encoder lines active-low, so it works whether or not your HW-040 board populates its own pull-ups.

### Two things worth knowing about these pins

**GPIO9 is deliberately avoided for the switch**, even though it is the BOOT button on most devkits. Holding GPIO9 low at reset enters the ROM download mode, and the encoder switch is Kivori's recovery control — a user power-cycling while holding it for recovery would land in the downloader instead of booting Kivori.

**The display pins are measured evidence; the encoder pins are not.** The display profile was physically verified on 2026-08-11 and is recorded in the Feature 001 validation checklist. The encoder pin map is a *specification* wired to on request, pending physical confirmation — row 15 of the Slice 002 checklist. Do not treat the two as equally settled.

Both live in one place, [`firmware/esp32-c3/src/profile.rs`](firmware/esp32-c3/src/profile.rs), so a rewire is a single constant change.

## Getting started

### Prerequisites

- **Rust** stable (both workspaces pin it; the firmware workspace adds the `riscv32imc-unknown-none-elf` target automatically)
- **[Bun](https://bun.sh)** for the frontend
- **[just](https://github.com/casey/just)** as the command runner
- **[espflash](https://github.com/esp-rs/espflash)** only if you are flashing hardware — `cargo install espflash`
- **jq** for the dependency guard scripts
- A `WOKWI_CLI_TOKEN` only if you intend to run the Wokwi simulation gate

### Run the desktop app

```bash
bun install
just dev                       # Vite dev server for the Device Studio UI
```

The native core owns the serial link; the webview receives only typed IPC commands. Device discovery is automatic — there is no COM-port picker by design.

### Flash the device

```bash
just fw-build                  # builds the product firmware (physical-st7789)
just fw-flash                  # flash + monitor over USB
```

Both recipes select the `physical-st7789` feature, which is what reaches the real display-and-input runtime. A plain `--features embedded` build compiles, but falls through to a bare fallback with no display and no input — useful to know if you invoke cargo directly.

### Verify without hardware

Most of the system is provable on a host machine:

```bash
just test                      # host workspace + frontend
just fw-test                   # firmware core against host-sim adapters
just golden                    # deterministic rendering, frame-hash goldens
just sim-test                  # Wokwi scenarios (needs a token)
```

Host-sim, Wokwi simulation, and physical hardware are **separate classes of evidence** in this project, and simulation never gets promoted to physical proof. See any feature's validation checklist for how results are recorded.

## Documentation map

| Need | Source |
|---|---|
| Product goals, scope, and acceptance gates | [`docs/product/prd.md`](docs/product/prd.md) |
| Build order, phases and checklists | [`docs/roadmap.md`](docs/roadmap.md) |
| Exact user-visible behavior | [`docs/product/user-story-contract.md`](docs/product/user-story-contract.md) |
| Engineering invariants and decision discipline | [`docs/engineering-principles.md`](docs/engineering-principles.md) |
| Cross-platform technical research | [`docs/research/technical-research.md`](docs/research/technical-research.md) |
| Accepted durable architecture decisions | [`docs/adr/`](docs/adr/) |
| Feature 001 requirements, implementation record, contracts, and evidence | [`docs/features/001-device-connection-foundation/`](docs/features/001-device-connection-foundation/) |
| Feature 001 closure status | [`docs/features/001-device-connection-foundation/closure-status.md`](docs/features/001-device-connection-foundation/closure-status.md) |
| Feature 001 validation ledger | [`docs/features/001-device-connection-foundation/validation-checklist.md`](docs/features/001-device-connection-foundation/validation-checklist.md) |
| Wire protocol contract (framing, messages, capabilities, session identity) | [`docs/features/001-device-connection-foundation/contracts/protocol.md`](docs/features/001-device-connection-foundation/contracts/protocol.md) |
| Native core ↔ webview IPC contract | [`docs/features/001-device-connection-foundation/contracts/ipc.md`](docs/features/001-device-connection-foundation/contracts/ipc.md) |
| Slice 002 rotary volume control — records and architecture | [`docs/features/002-rotary-volume-control/`](docs/features/002-rotary-volume-control/) |
| Slice 002 physical validation ledger (**15 rows, all outstanding**) | [`docs/features/002-rotary-volume-control/validation-checklist.md`](docs/features/002-rotary-volume-control/validation-checklist.md) |
| Feature 003 mascot animation — record and evidence | [`docs/features/003-mascot-animation/`](docs/features/003-mascot-animation/) |
| Typed session activity log (the only runtime log) | [`docs/activity-log.md`](docs/activity-log.md) |
| Flashing bundled firmware from Device Studio | [`docs/firmware-update.md`](docs/firmware-update.md) |
| Superpowers design records | [`docs/superpowers/specs/`](docs/superpowers/specs/) |
| Superpowers implementation plans | [`docs/superpowers/plans/`](docs/superpowers/plans/) |
| Wokwi simulation | [`sim/wokwi/README.md`](sim/wokwi/README.md) |

## Document authority

When documents disagree, use this hierarchy:

1. **PRD + User Story Contract** — product behavior and user guarantees.
2. **Engineering Principles** — implementation invariants and development discipline.
3. **ADRs** — durable technical decisions that have been explicitly accepted.
4. **Technical Research** — researched suggestions, alternatives, caveats, and required validation; intentionally challengeable.
5. **Feature records** — requirements, architecture, validation, contracts, and evidence for a particular implemented slice.
6. **Superpowers specs/plans** — design and execution artifacts for individual changes.

## Development workflow

Kivori uses **Superpowers** as the active workflow for new engineering work. Spec Kit is retired.

```text
problem / idea
    ↓
brainstorm + research
    ↓
approved design (docs/superpowers/specs/)
    ↓
implementation plan (docs/superpowers/plans/)
    ↓
implementation + tests
    ↓
verification / review
    ↓
ADR when a durable architecture choice is accepted
```

Research recommendations are not mandates. If implementation evidence disproves a recommendation, update the research or ADR while preserving the product contract.

## Repository layout

```text
Kivori/
├── README.md
├── docs/
│   ├── product/
│   │   ├── prd.md
│   │   └── user-story-contract.md
│   ├── engineering-principles.md
│   ├── research/
│   │   └── technical-research.md
│   ├── adr/
│   ├── features/
│   │   └── 001-device-connection-foundation/
│   └── superpowers/
│       ├── specs/
│       └── plans/
├── apps/
├── crates/
├── firmware/
├── sim/
├── tests/
└── tools/
```

Feature records contain durable project knowledge, not workflow scaffolding. The retired Feature 001 Spec Kit `plan.md`, `tasks.md`, and requirement-writing checklist remain available in Git history if historical investigation is needed.

## Common commands

The repository uses [`just`](https://github.com/casey/just) as a convenience command runner.

```bash
just dev               # Vite dev server for the Device Studio UI
just lint              # Rust + TypeScript formatting/lint/type checks
just test              # host workspace + frontend tests
just build             # host workspace + frontend build
just fw-check          # shared no_std crates compiled for RISC-V (isolation proof)
just fw-test           # firmware core against host-sim adapters
just fw-build          # build the product ESP32-C3 firmware (physical-st7789)
just fw-flash          # flash + monitor the product firmware over USB
just sim-test          # Wokwi integration scenarios (needs WOKWI_CLI_TOKEN)
just golden            # deterministic rendering golden-frame tests
just check-boundaries  # shared-crate dependency firewall
just assets            # recompile the canonical asset blob
```

Three guard scripts run in CI and are worth running locally before pushing — they fail for reasons ordinary tests do not catch:

```bash
bash scripts/check-release-surface.sh   # dev-only surfaces absent from production builds
bash scripts/check-crate-boundaries.sh  # no std/OS deps in the shared no_std crates
bash scripts/check-offline-deps.sh      # no first-party crate pulls a network client
```

`check-release-surface.sh` compiles the shared crates for the device target on purpose, including a positive control that proves the absence checks are not vacuous. It is the one gate most likely to catch a change that every test suite still passes.

See [`docs/features/001-device-connection-foundation/quickstart.md`](docs/features/001-device-connection-foundation/quickstart.md) and [`sim/wokwi/README.md`](sim/wokwi/README.md) for environment-specific setup and validation detail.
