# ADR-0010: v1 controller with native USB

- **Status:** Proposed (decide before the production PCB)
- **Note:** The current board is the RP2040-Zero ([ADR-0019](./0019-rp2040-zero-replaces-esp32-c3.md)); this ADR is only about the v1 production controller.
- **Date:** 2026-10-11

## Context

The beta board is the RP2040-Zero. For v1, Kivori should be able to work as a plain USB volume/media device when no Kivori Desktop session exists, so it is useful on a locked-down work laptop before (or without) installing anything. That needs native USB device support for a composite CDC + HID device, and, for production units, secure boot ([ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md) tier B), which the RP2040 does not have.

## Decision

Proposed: RP2350 (recommended). It has native USB device for composite CDC + HID on standard Rust (`embassy-usb` or `usb-device`), 520 KB SRAM (room to double-buffer the 115 KB frame), PIO for the display, and a ROM BOOTSEL bootloader (USB drive + PICOBOOT) that no bad firmware can overwrite, which is the app-independent recovery path gate 8 asks of production hardware. No radio also keeps regulatory work to an unintentional radiator.

Rules for app-free mode either way: HID acts only while there is no Desktop session (Waiting), and goes silent the moment a session handshakes, so nothing ever runs twice or switches mechanism silently (invariant 19). HID outcomes cannot be observed, so the device shows a quiet acknowledgement and a direction, never a volume number it cannot know.

## Consequences

Costs of the RP2350: an external QSPI flash (or an RP2354 with flash in the package), a port of the board layer (`bsp`, `physical_*`, transport, device id from the chip id in OTP) and of the app's flashing (UF2 / PICOBOOT). The shared `no_std` crates, the protocol and the renderer do not change.

## Alternatives considered

- ESP32-S3: USB OTG for a composite serial + HID device and a ROM USB download mode, but Xtensa cores, so the firmware leaves standard Rust for Espressif's toolchain fork (`espup`) and keeps a radio Kivori never uses.
