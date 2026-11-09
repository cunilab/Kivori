# Kivori

> **Control the desktop physically. Understand the desktop visually.**

Kivori is a desk buddy you control your computer with: the buddy is why you want one, the physical
knob and buttons are why you keep using it, and the display shows important desktop and system
state around the buddy.

The first product is **Kivori Pip** (model KV-P1): a knob with a push switch, three buttons and a
240×240 screen where the keycap buddy lives, on a Waveshare RP2040-Zero. A desktop app, Kivori
Desktop, drives it and is where you configure it. Windows is the beta platform; macOS is the daily
test bed. Beta users start with the [user guide](docs/user-guide.md).

## What it does

Two pillars carry equal weight, with monitoring in support. Everything below is built and
host-tested; the hardware checks are still open (see [Status](#status)).

**The buddy**

- The buddy is the home screen. Clock, indicators and the labels of every control sit around it,
  and its face follows what is really happening: muted, mic muted, high load, media playing, an
  error, offline or asleep.
- It stays alive on its own when nothing happens: the idle life runs in the firmware, so it keeps
  moving even when the computer stalls. Brightness is a setting.
- Every waiting, failure and recovery state has its own screen (Sleeping, Locked, Updating,
  Offline, ...), never a frozen frame.

**Control**

- Knob: turn, press, hold and double press. Three buttons: press and hold each. Holding the knob
  switch for about 10 s reboots the device, with no app needed.
- Actions: system and app volume, media keys, master mute, mic mute, keyboard shortcuts, app
  launch and simple ordered macros.
- **Truthful mic mute:** mutes every microphone at the OS level and reads each one back; it says
  "muted" only when all of them are.
- **Profiles for any app:** built-in General, Browser, Code, Media, Zoom and Teams profiles, plus
  up to 16 of your own ("Add the app in front of me"). Labels and bindings switch with the app in
  front; the middle button's Hold pins a profile.
- **Honest results:** each action ends as State Confirmed, Execution Confirmed, Unverified or
  Error, never a success it cannot know. Trigger-only actions (a shortcut, a media key) get a
  quiet acknowledgement instead of a warning badge.

**Monitoring and the app**

- Views: Buddy, Clock, Volume, Media and CPU / RAM.
- **Updates:** the app updates itself from GitHub Releases (signed, with "Install & restart",
  "Later" and "Skip this version"), and offers a firmware update when an older Kivori connects.
- **Identity:** every device reports its product, board and revision, and the app never flashes
  firmware made for another product or board.
- Device internals (flash, RAM, CPU, frame times, link counters) in developer mode, a splash on
  start, launch at login, a tray icon, first-run onboarding, flashing and recovery of the bundled
  firmware. Core features work offline; the only network use is the update check.

## Status

All software through M2.7 is done and host-tested. A milestone only counts as done once its
hardware rows in [`docs/validation.md`](docs/validation.md) pass, and that session is still open.
The paid beta (M3) also waits on items that are not code. Details in
[`docs/roadmap.md`](docs/roadmap.md).

| Milestone | Software | Still open |
|---|---|---|
| M0 Working foundation | done | hardware rows (Phases 1 to 3) |
| M1 Useful desk device | done | hardware rows (Phase 4) |
| M2 Configurable product, M2.5 Contextual profiles | done | hardware rows (Phases 5, 5b, 6) |
| M2.6 Beta hero features | done | hardware rows (Phase 13) |
| M2.7 Identity, versions and updates | done | hardware rows (Phase 14) |
| M3 Paid beta | done for the app (installer, recovery, onboarding) | hardware validation (#38), signing certificates (#36), enclosure (#33) |

## Quick start for developers

### Prerequisites

- **Rust** stable (both workspaces pin it; the firmware workspace adds the
  `thumbv6m-none-eabi` target automatically)
- **[Bun](https://bun.sh)** for the frontend
- **[just](https://github.com/casey/just)** as the command runner
- **[elf2uf2-rs](https://github.com/JoNil/elf2uf2-rs)** only if you are flashing hardware from the
  command line (`just fw-flash`): `cargo install elf2uf2-rs`. The app flashes without it.
- **jq** for the dependency guard scripts

### Run the desktop app

```bash
bun install
just desktop                   # UI dev server + native app with the bundled firmware, one command
```

Desktop recipes embed firmware for every active catalog pair. Single-target `KIVORI_PRODUCT` /
`KIVORI_BOARD` selection applies to `fw-build`, not desktop packages.

The native core owns the serial link; the webview receives only typed IPC commands. Device
discovery is automatic: there is no COM-port picker by design.

### Flash the device

```bash
just fw-build                  # builds catalog-selected firmware (currently Pip physical-st7789)
just fw-flash                  # flash + monitor over USB
```

Both recipes select the `physical-st7789` feature, which is what reaches the real
display-and-input runtime. A plain `--features embedded` build compiles, but falls through to a
bare fallback with no display and no input; worth knowing if you invoke cargo directly.

### Verify without hardware

```bash
just test                      # host workspace + frontend
just fw-test                   # firmware core against host-sim adapters
just golden                    # deterministic rendering, frame-hash goldens
```

Host-sim tests and physical hardware are **separate classes of evidence** here, and a host-sim
result is never promoted to physical proof. Hardware results are recorded in
[`docs/validation.md`](docs/validation.md).

## Hardware and pin map

Kivori Pip (KV-P1) is one Waveshare RP2040-Zero, one ST7789 240×240 panel, one HW-040 rotary
encoder, three momentary push buttons. USB provides power, flashing and the product
data link (USB CDC, `2E8A:000A`), with no external UART bridge. The first beta units used an
ESP32-C3 SuperMini; the board changed with the v7 enclosure ([ADR-0019](docs/adr/0019-rp2040-zero-replaces-esp32-c3.md)).

| Signal | GPIO | Notes |
|---|---:|---|
| SPI1 SCK (display SCL) | 26 | 20 MHz, Mode 3 |
| SPI1 TX (display SDA) | 15 | |
| Display D/C | 13 | |
| Display RST | 14 | |
| Backlight | 12 | active-high, PWM (slice 6A) for the brightness setting |
| Encoder SW | 3 | push switch: Press, Hold, Double press, ~10 s recovery reboot |
| Encoder DT (B) | 4 | |
| Encoder CLK (A) | 5 | |
| Button left / middle / right (B1 / B2 / B3) | 6 / 7 / 8 | to GND, internal pull-up, no resistor |

The display has no chip-select line. GP0/GP1 stay free for a debug UART, used only by the
`debug-payloads` development feature. The board's own WS2812 LED (GP16) is not used.

Encoder COM to **GND**, VCC to **3V3**. **Not 5V: RP2040 GPIOs are not 5 V tolerant.** Firmware
enables internal pull-ups and reads all three encoder lines active-low, so it works whether or not
your HW-040 board populates its own pull-ups.

### Two things worth knowing about these pins

**BOOT is the RP2040-Zero's own button, not a Kivori control.** Holding **BOOT** while plugging in
the USB cable starts the ROM's UF2 bootloader (a drive named `RPI-RP2`), which is the recovery path
when the firmware cannot start; the app, or `just fw-flash`, copies the firmware onto that drive. A
running Kivori also reboots into the bootloader when the app opens its serial port at 1200 baud
(the "1200-baud touch"), which is how the app updates a working unit. The encoder switch stays the
user's recovery control and holding it at power-on boots Kivori normally: no Pip input is a
boot-critical pin.

**The panel parameters are measured evidence; the pin map is not.** The display profile (offset,
colour order, inversion, SPI mode) was physically verified on 2026-08-11 and is recorded in
[`docs/validation.md`](docs/validation.md) (rows 1.9, 1.24, 1.25). The pin map moved to the RP2040-Zero
carrier (#179) and is a *specification* until rows 3.15, 5.1 and 5.2 of the same checklist are
ticked on a board wired into the v7 enclosure.

Both live in one place, [`firmware/pip/src/profile.rs`](firmware/pip/src/profile.rs), so a
rewire is a single constant change. The case and its printed carrier plate are in
[`hardware/enclosure-v7/`](hardware/enclosure-v7/README.md); building a unit is in
[`docs/assembly.md`](docs/assembly.md).

## Products, versions and releases

- **One app for every product.** [`products/registry.toml`](products/registry.toml) hands out
  permanent machine ids and wire numbers; each product has a catalog entry,
  `products/<id>/product.toml` (customer name, short name, model code, boards, bundled firmware,
  required capabilities), compiled into the app ([ADR-0014](docs/adr/0014-versions-product-identity-and-product-catalog.md)).
- **Product ids.** A product's id is its short name in lowercase: `pip` is Kivori Pip, so its
  firmware lives in `firmware/pip` and its catalog entry in
  [`products/pip/product.toml`](products/pip/product.toml). It was the codename `pala` until
  2026-10-30; devices identify themselves by number, so the rename did not affect shipped units.
- **Version numbers.** The app has one version (the `vX.Y.Z` tag, the only thing users install).
  Each product's firmware has its own version (tag `fw-<product>-vA.B.C`). Every bundled product's inputs are guarded before release.
  Compatibility is decided by the protocol version and capabilities alone, never by pairing app
  and firmware numbers. Before the first release the app and the Kivori Pip firmware are 0.1.0 and
  the protocol is 0.1.
- **Releasing.** Actions, **Release**, Run workflow on `main`: pick the app bump and, if the
  firmware changed, a firmware bump (try a dry run first; app bump `none` ships the current version,
  as the first release does). It builds a **draft** titled
  `Kivori X.Y.Z · Pip firmware A.B.C` for today's catalog, adding every further product/version.
  Per-product override maps support different firmware bumps in one run; every product is guarded.
  Notes come from conventional commits; check the draft and
  publish it. Full steps in [`docs/release.md`](docs/release.md).

## Repository layout

```text
Kivori/
├── apps/
│   ├── desktop/     Kivori Desktop: Tauri native core (src-tauri) + webview UI
│   └── web/         the website (Next.js on Cloudflare)
├── packages/ui/     shared UI components for the desktop app and the website
├── crates/          shared no_std crates: model, protocol, renderer, assets, framebuffer
├── firmware/pip/   Kivori Pip firmware (RP2040-Zero, its own Cargo workspace)
├── products/        product registry and catalog (registry.toml, <id>/product.toml)
├── hardware/        3D-printable enclosure and printed carrier plate (FreeCAD, parametric)
├── tests/           golden frames and cross-crate tests
├── tools/           asset compiler
├── scripts/         guard, build and release scripts
└── docs/            product, architecture, ADRs, roadmap, validation, release, guides
```

## Documentation

One file per topic:

| File | What it answers |
|---|---|
| [`docs/product.md`](docs/product.md) | What Kivori must do: scope, behavior rules, numbered invariants, acceptance gates |
| [`docs/architecture.md`](docs/architecture.md) | How it works: components, wire protocol, IPC, rendering, rotary loop, decisions, principles |
| [`docs/adr/`](docs/adr/README.md) | Architecture decision records: one file per decision, with context and rejected options |
| [`docs/roadmap.md`](docs/roadmap.md) | What gets built next, milestone by milestone (M0 to paid beta to v1) |
| [`docs/validation.md`](docs/validation.md) | The hardware checklist and how to run a validation session |
| [`docs/assembly.md`](docs/assembly.md) | Building one beta unit: wiring, checks, closing the case |
| [`docs/release.md`](docs/release.md) | How to cut a Windows or macOS release, and the unsigned-installer limits |
| [`docs/provisioning.md`](docs/provisioning.md) | Why there is no per-unit provisioning, and where a unit's identity comes from |
| [`docs/user-guide.md`](docs/user-guide.md) | Short guide for a beta customer: install, setup, controls, recovery |
| [`docs/bom.md`](docs/bom.md) | Draft bill of materials (prices still to fill in) |

If the docs disagree, `product.md` wins on behavior and the code wins on facts. Older plans, specs and research
are in Git history.

## Common commands

```bash
just desktop           # UI dev server + native app with the bundled firmware
just app               # build the firmware, then run the native app with it embedded
just dev               # Vite dev server for the Device Studio UI
just lint              # Rust + TypeScript formatting/lint/type checks
just test              # host workspace + frontend tests
just build             # host workspace + frontend build
just fw-check          # shared no_std crates compiled for thumbv6m (isolation proof)
just fw-test           # firmware core against host-sim adapters
just fw-build          # build catalog-selected firmware (currently Pip physical-st7789)
just fw-flash          # flash the product firmware (board in BOOT mode, see Hardware)
just bundle-windows    # build the unsigned Windows installer (docs/release.md)
just bundle-macos      # build the macOS .app/.dmg (docs/release.md)
just golden            # deterministic rendering golden-frame tests
just check-boundaries  # shared-crate dependency firewall
just assets            # recompile the canonical asset blob
just web-dev           # website dev server on :3000
```

`just --list` shows every recipe.

## Contributing

- Use trunk-based development: `main` is the only long-lived development branch. Start small,
  short-lived branches from current `main`, target PRs at `main`, and merge frequently rather than
  collecting a milestone-sized stack. Keep incomplete work safe behind existing capability/build
  gates or split it into usable increments.
- Squash-merge after required CI passes and owner review, using the conventional PR title as the
  commit subject; delete the merged branch. Do not push directly to `main`, force-push it or rewrite
  release tags. A solo owner's merge decision supplies approval; a separate reviewer is needed if
  GitHub approval counts are enabled.
- Releases are on demand: the **Release** button prepares a version PR, then its `finalize` stage
  tags the exact merged, CI-green commit and builds a draft. App tags are `vX.Y.Z`; firmware tags
  remain independent per product. See [release.md](docs/release.md).

- A new post-handshake wire message or appended wire-enum variant bumps `PROTOCOL_MINOR` and
  declares its negotiated capability gate. Minor is a descriptive revision, not a feature gate.
  Changing existing wire layouts, including pre-negotiation handshake layouts, requires a protocol
  major change. See
  [ADR-0002](docs/adr/0002-one-wire-protocol-crate.md).
- Pull request titles are conventional commits (`feat:`, `fix:`, `perf:`, scoped like
  `feat(mic): ...`, `!` for a breaking change). They become the release notes, so write them for
  the person reading the release.
- CI runs on pull requests and on `main`: `host` (Rust workspace on Linux and Windows, frontend,
  Windows startup smoke), `firmware` (no_std isolation, RISC-V clippy, host-sim tests),
  `determinism` (golden frames, asset reproducibility), `web` (website changes only) and, for
  packaging changes, `release`.
- Four guard scripts run in CI and are worth running locally before pushing; they fail for
  reasons ordinary tests do not catch:

  ```bash
  bash scripts/check-release-surface.sh   # dev-only surfaces absent from production builds
  bash scripts/check-crate-boundaries.sh  # no std/OS deps in the shared no_std crates
  bash scripts/check-offline-deps.sh      # no first-party crate pulls a network client
  just check-identifiers                # no product names/ids in shared app/tooling; positive controls
  ```

  `check-release-surface.sh` compiles the shared crates for the device target on purpose,
  including a positive control that proves the absence checks are not vacuous. It is the one gate
  most likely to catch a change that every test suite still passes.

## License and notices

Kivori is not published under an open-source license (`license = "UNLICENSED"` in `Cargo.toml`).
Third-party software bundled in the installers, beyond the dependencies in `Cargo.lock` and
`bun.lock`, is listed in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
