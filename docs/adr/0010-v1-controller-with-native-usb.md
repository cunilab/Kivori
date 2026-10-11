# ADR-0010: v1 controller with native USB

- **Status:** Proposed (decide before the production PCB)
- **Date:** 2026-10-11

## Context

The beta stays on the ESP32-C3. For v1, Kivori should be able to work as a plain USB volume/media device when no Kivori Desktop session exists, so it is useful on a locked-down work laptop before (or without) installing anything. The C3 cannot do this: its USB is a fixed-function Serial/JTAG port with no HID.

## Decision

Proposed: option (c), RP2350 (recommended). It has native USB device for composite CDC + HID on standard Rust (`embassy-usb` or `usb-device`), 520 KB SRAM (room to double-buffer the 115 KB frame), PIO for the display, and a ROM BOOTSEL bootloader (USB drive + PICOBOOT) that no bad firmware can overwrite, which is the app-independent recovery path gate 8 asks of production hardware. No radio also keeps regulatory work to an unintentional radiator.

Rules for app-free mode either way: HID acts only while there is no Desktop session (Waiting), and goes silent the moment a session handshakes, so nothing ever runs twice or switches mechanism silently (invariant 19). HID outcomes cannot be observed, so the device shows a quiet acknowledgement and a direction, never a volume number it cannot know.

## Consequences

Costs of the RP2350: an external QSPI flash (or an RP2354 with flash in the package), a port of the board layer (`bsp`, `physical_*`, transport, device id from the chip id in OTP) and of the app's flashing and provisioning (UF2 / PICOBOOT instead of `espflash`). The shared `no_std` crates, the protocol and the renderer do not change.

## Alternatives considered

- (a) Keep the C3 and add a UART-to-HID bridge chip: a second USB device needs a hub; more parts, two ports to reason about.
- (b) ESP32-S3: USB OTG for a composite serial + HID device and a ROM USB download mode, but Xtensa cores, so the firmware leaves standard Rust for Espressif's toolchain fork (`espup`) and keeps a radio Kivori never uses.
