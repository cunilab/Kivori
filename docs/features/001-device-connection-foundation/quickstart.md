# Quickstart: Device Connection Foundation

**Feature:** `001-device-connection-foundation`  
**Status:** implemented foundation; manual acceptance still has open items  
**Last reconciled:** 2026-09-16

This is the current setup and validation guide for the implemented Device Connection Foundation. Feature requirements live in [`requirements.md`](./requirements.md), technical shape in [`architecture.md`](./architecture.md) and [`data-model.md`](./data-model.md), wire/IPC contracts in [`contracts/`](./contracts/), and remaining physical acceptance work in [`validation-checklist.md`](./validation-checklist.md).

The retired Spec Kit plan/task files are intentionally not part of the working documentation tree. Git history remains available if historical task-level investigation is required.

## Prerequisites

- Rust toolchain from `rust-toolchain.toml`.
- RISC-V target: `rustup target add riscv32imc-unknown-none-elf`.
- Bun for the desktop frontend workspace.
- Tauri v2 system prerequisites for the host OS.
- `espflash` for physical ESP32-C3 flashing.
- `just` for repository command recipes.

Core operation is offline-first after dependencies are installed. See [`offline-boundary.md`](./offline-boundary.md).

## Repository shape

The host crates/Desktop app are in the root Cargo workspace. `firmware/esp32-c3/` is a deliberately separate Cargo workspace so firmware features cannot be accidentally unified with host-only/std dependencies. Shared model/protocol/renderer/assets crates are consumed by both sides.

See [`architecture.md`](./architecture.md) and [ADR-0001](../../adr/0001-two-workspace-cargo-split.md).

## Setup

```bash
bun install
cargo fetch
(cd firmware/esp32-c3 && cargo fetch)
```

## Common commands

```bash
just lint
just test
just build
just fw-check
just fw-test
just fw-build
just fw-flash
just sim-test
just golden
just check-boundaries
```

Important commands:

| Command | Purpose |
|---|---|
| `just dev` | Run the Tauri Desktop in development mode with Device Studio available. |
| `just lint` | Rust formatting/clippy plus frontend lint/type/format checks. |
| `just test` | Host Rust workspace and frontend tests. |
| `just build` | Host/frontend production builds. |
| `just fw-check` | Compile shared `no_std` code for the RISC-V target. |
| `just fw-test` | Run host-sim firmware tests. |
| `just fw-build` | Build the physical ESP32-C3 firmware. |
| `just fw-flash` | Flash/monitor the verified physical firmware profile. |
| `just sim-test` | Run Wokwi integration scenarios. |
| `just golden` | Run deterministic renderer/golden-frame tests. |
| `just check-boundaries` | Enforce shared-crate dependency boundaries. |

## Automated validation

The repository's CI covers the software-verifiable part of Feature 001:

- protocol framing, malformed-input handling, version/capability negotiation, and sequence behavior;
- deterministic shared rendering and golden frames;
- host-simulation firmware behavior and production-runtime compile checks;
- desktop connection state/reconnect/session behavior;
- diagnostics redaction and release-surface guards;
- frontend type/lint/test/build checks;
- Wokwi protocol/display/production-runtime scenarios;
- a real Windows Tauri startup smoke.

Passing simulation or host tests does **not** establish physical panel timing, USB reliability under stress, real reconnect latency, or physical preview/display parity.

## Device Studio validation

1. Run `just dev`.
2. Open Device Studio.
3. Select each Feature 001 companion state: `booting`, `idle`, `happy`, `busy`, `sleeping`, `offline`.
4. Confirm the preview is produced by the shared/native renderer rather than reimplemented by the canvas.
5. Exercise deterministic elapsed-time inspection and confirm identical inputs reproduce identical frames.

## Wokwi validation

Run:

```bash
just sim-test
```

The Wokwi production-runtime scenario executes the same high-level firmware runtime used by the physical firmware through simulated adapters. It is useful for protocol/lifecycle/render integration but is **not** physical hardware evidence.

See [`../../../sim/wokwi/README.md`](../../../sim/wokwi/README.md).

## Verified physical hardware profile

Physical validation on 2026-08-11 established:

| Property | Value |
|---|---|
| MCU | ESP32-C3 |
| USB | Native USB Serial/JTAG |
| USB VID:PID | `0x303A:0x1001` |
| Display | ST7789, 240x240 RGB565 |
| SPI | SPI2, 20 MHz, Mode 3 |
| SCK | GPIO6 |
| MOSI | GPIO7 |
| CS | unused |
| D/C | GPIO2 |
| Reset | GPIO3 |
| Backlight | GPIO8, active-high |
| Offset | `(0,0)` |
| Rotation | 90° |
| Color order | RGB |
| Inversion | enabled |

These values supersede older pre-hardware assumptions in historical commits.

## Physical validation procedure

### 1. Build and flash

```bash
just fw-build
just fw-flash
```

Confirm the physical device boots and that USB Serial/JTAG enumerates normally.

### 2. Desktop discovery and handshake

Run the desktop with `just dev` or a production build.

Confirm:

- no manual port selection is required;
- the Kivori device is selected rather than unrelated serial devices;
- the versioned handshake succeeds;
- Desktop reports Connected and shows the reported firmware/protocol version.

The basic discovery/handshake path was physically demonstrated on 2026-08-11. The controlled `< 5 s` latency measurement remains open in [`validation-checklist.md`](./validation-checklist.md).

### 3. Companion-state rendering

Exercise `idle`, `happy`, `busy`, and `sleeping` through the desktop and confirm the matching physical scenes.

`idle` has been physically observed; the remaining state-by-state validation and measured `< 1 s` propagation target remain open.

### 4. Preview parity

For a fixed state and elapsed time, compare Device Studio output with the physical panel. Record the result in [`validation-checklist.md`](./validation-checklist.md). This remains manual evidence; golden frames alone do not prove the panel output.

### 5. Disconnect/reconnect

With a non-default desired state active:

1. unplug the device;
2. confirm Desktop reports the connection loss;
3. reconnect it;
4. confirm automatic handshake/resynchronization restores current desired state without replaying unrelated stale activity;
5. record reconnect timing.

Rapid unplug/replug stability and controlled reconnect timing are still manual acceptance items.

### 6. Background window lifecycle

On a real desktop:

1. close/hide the Kivori window;
2. verify the process and device work continue;
3. reactivate the app and verify the single window returns;
4. explicitly Quit and verify the device task/process stop.

The Windows startup smoke proves the binary starts and remains alive; it does not prove this full hide/reactivate/Quit lifecycle.

### 7. Safety and diagnostics

Confirm ordinary diagnostics/logs do not expose raw payload bytes, raw device identity, secrets, usernames, or machine paths beyond the documented allowlist. See [`activity-log.md`](../../activity-log.md).

## Acceptance record

Do not infer manual success from automated tests. Record measured physical/platform evidence in [`validation-checklist.md`](./validation-checklist.md). The current summary of completed vs outstanding Feature 001 work lives in [`closure-status.md`](./closure-status.md).
