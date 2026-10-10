# Kivori

Kivori is a desk buddy you control your computer with: the buddy is why you want one, the physical knob and button are why you keep using it, and the display shows important desktop and system state around the buddy.

> **Product thesis:** Control the desktop physically. Understand the desktop visually.

## Project status

**M0 — Working foundation.** Connection, protocol, deterministic rendering, Device Studio, the ESP32-C3/ST7789 runtime, the rotary volume loop and the mascot with its activity log are built. Remaining physical acceptance rows live in [`docs/validation.md`](docs/validation.md).

**M1 — Useful desk device.** Software complete on Windows and macOS, host-tested end to end. The push switch does Press (Play/Pause), Hold (mute) and Double press (next view); holding it ~10 s reboots the device with no app needed. The display has Buddy, Clock, Volume, Media and CPU/RAM views, and every action outcome is shown honestly as Confirmed, Started, Unverified or Failed. **Physical validation is outstanding:** the M0 rows and the Phase 4 rows in [`docs/validation.md`](docs/validation.md) need one human session.

**M2 and M2.5.** Three contextual buttons (Press and Hold each), the bindings config UI and foreground-app profiles are software complete and host-tested, awaiting hardware validation (Phase 5 and 6 rows).

**M3 — Paid beta.** Software complete: host takeovers, sleep and lock handling, firmware update and recovery, launch at login with a tray, the installer and an unsigned release pipeline ([`docs/release.md`](docs/release.md)), first-run onboarding, and per-unit provisioning ([`docs/provisioning.md`](docs/provisioning.md)).

Beta users start with the [user guide](docs/user-guide.md). Still open, and none of it is code: the hardware validation session (issue #38), signing the installer (needs certificates, #36), and the enclosure print (#33). See [`docs/roadmap.md`](docs/roadmap.md). Product behavior is defined in [`docs/product.md`](docs/product.md); durable technical choices are the ADRs in [`docs/architecture.md`](docs/architecture.md).

## Hardware

One ESP32-C3, one ST7789 240×240 panel, one HW-040 rotary encoder, three momentary push buttons. USB provides power, flashing, and the product data link — no external UART bridge.

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
| Encoder SW | 10 | push switch: Press, Hold, ~10 s recovery reboot |
| Button left / middle / right | 0 / 1 / 20 | to GND, internal pull-up, no resistor; GPIO0/1 need a devkit without a 32 kHz crystal |
| USB D− / D+ | 18 / 19 | native USB Serial/JTAG, `0x303A:0x1001` |

Encoder COM to **GND**, VCC to **3V3**. **Not 5V — ESP32-C3 GPIOs are not 5 V tolerant.** Firmware enables internal pull-ups and reads all three encoder lines active-low, so it works whether or not your HW-040 board populates its own pull-ups.

### Two things worth knowing about these pins

**GPIO9 is deliberately avoided for the switch**, even though it is the BOOT button on most devkits. Holding GPIO9 low at reset enters the ROM download mode, and the encoder switch is Kivori's recovery control — a user power-cycling while holding it for recovery would land in the downloader instead of booting Kivori.

**The display pins are measured evidence; the encoder pins are not.** The display profile was physically verified on 2026-08-11 and is recorded in [`docs/validation.md`](docs/validation.md) (rows 1.9, 1.24, 1.25). The encoder pin map is a *specification* wired to on request, pending physical confirmation — row 3.15 of the same checklist. Do not treat the two as equally settled.

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
just desktop                   # UI dev server + native app with the bundled firmware, one command
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

Host-sim, Wokwi simulation, and physical hardware are **separate classes of evidence** in this project, and simulation never gets promoted to physical proof. Hardware results are recorded in [`docs/validation.md`](docs/validation.md).

## Documentation

One file per topic:

| File | What it answers |
|---|---|
| [`docs/product.md`](docs/product.md) | What Kivori must do: scope, behavior rules, numbered invariants, acceptance gates |
| [`docs/architecture.md`](docs/architecture.md) | How it works: components, wire protocol, IPC, rendering, rotary loop, decisions, principles |
| [`docs/roadmap.md`](docs/roadmap.md) | What gets built next, milestone by milestone (M0 to paid beta to v1) |
| [`docs/validation.md`](docs/validation.md) | The hardware checklist and how to run a validation session |
| [`docs/release.md`](docs/release.md) | How to cut a Windows or macOS release, and the unsigned-installer limits |
| [`docs/provisioning.md`](docs/provisioning.md) | Flashing and checking each new unit (`just provision`) |
| [`docs/user-guide.md`](docs/user-guide.md) | Short guide for a beta customer: install, setup, controls, recovery |
| [`docs/bom.md`](docs/bom.md) | Draft bill of materials (prices still to fill in) |

Wokwi simulation setup lives in [`sim/wokwi/README.md`](sim/wokwi/README.md). If the docs disagree,
`product.md` wins on behavior and the code wins on facts. Older plans, specs and research are in Git
history.

## Repository layout

```text
Kivori/
├── docs/        product, architecture, roadmap, validation, release, provisioning, user guide, BOM
├── apps/        Tauri desktop app (native core + Device Studio webview)
├── crates/      shared no_std crates: model, protocol, renderer, assets, framebuffer
├── firmware/    ESP32-C3 firmware
├── hardware/    3D-printable enclosure and printed prototype PCB (FreeCAD, parametric)
├── sim/         Wokwi simulation
├── tests/       golden frames and cross-crate tests
└── tools/       asset compiler and generators
```

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
just desktop           # UI dev server + native app with the bundled firmware
just provision         # flash and check one new unit (docs/provisioning.md)
just bundle-windows    # build the unsigned Windows installer (docs/release.md)
just bundle-macos      # build the macOS .app/.dmg (docs/release.md)
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

See [`docs/validation.md`](docs/validation.md) and [`sim/wokwi/README.md`](sim/wokwi/README.md) for hardware and simulator setup.
