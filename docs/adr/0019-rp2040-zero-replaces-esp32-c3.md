# ADR-0019: RP2040-Zero replaces the ESP32-C3 on real hardware

- **Status:** Accepted
- **Date:** 2026-11-05
- **Related:** [issue #179](https://github.com/cunilab/Kivori/issues/179), [ADR-0010](./0010-v1-controller-with-native-usb.md), [ADR-0014](./0014-versions-product-identity-and-product-catalog.md), [ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md)

## Context

The v7 enclosure (`hardware/enclosure-v7`) is built around the Waveshare RP2040-Zero: its carrier, wiring and pin map assume that board. The beta units were ESP32-C3 SuperMini boards (ADR-0010 kept the C3 for the beta). The C3 identified itself through a user eFuse record and was flashed with `espflash`; the RP2040 has neither an eFuse block nor an ESP flash protocol.

## Decision

- The RP2040-Zero replaces the C3 for real hardware: `rp2040-zero` is the board of the Pip firmware (`thumbv6m-none-eabi`, `rp2040-hal`, USB CDC `2E8A:000A`).
- Flashing is UF2. The app converts the ELF to UF2 and copies it to the `RPI-RP2` drive of the ROM bootloader, reached by a 1200-baud touch on the running unit or by holding BOOT while plugging in (recovery). The `espflash` sidecar is removed.
- There is no eFuse, so identity comes from the firmware build (`source = Build`) and provisioning, `espefuse` and the identity burn are removed.
- The USB id `2E8A:000A` only nominates a port, as ADR-0014 requires of every hint; any Pico-SDK CDC device reports it, so the handshake decides.
- The C3 stays in two places only: the v1 simulator and virtual device (the default; v2 is the RP2040-Zero), and a recognised legacy board `c3-supermini` in the catalog, without firmware.

## Consequences

- C3 beta units no longer get firmware updates from the app; they keep the firmware and the identity they have.
- The v2 timing and resource-budget figures are estimates until measured on a real RP2040-Zero (issue #179, hardware step).
- The RP2040 has no secure boot, so tier B of ADR-0016 needs the RP2350, as ADR-0010 already proposes; until then only tier A applies.

## Validation (2026-11-06, bare RP2040-Zero over USB)

- The app's UF2 output is accepted by the ROM bootloader, the real `INFO_UF2.TXT` matches the drive check, the unit enumerates as `Kivori Pip` `2E8A:000A`, answers the handshake in 54 to 75 ms and reports compose 110 ms (9 fps) and about 9,980 B of never-used stack. The 1200-baud touch reaches the bootloader in about 2 s and a reflash reconnects without a replug.
- The first unit found one bug no host test could: the USB interrupt must read OUT data itself (see architecture, Transport). Screen, knob, keys and LED are still to be checked on the v7 carrier ([validation](../validation.md#rp2040-zero-bring-up-179)).

## Alternatives considered

- Keep the ESP32-C3 as the real board: rejected, the v7 enclosure is built around the RP2040-Zero.
