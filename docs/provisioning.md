# Provisioning

There is no per-unit provisioning. The `just provision` tool and the identity burn were removed with
the move to the RP2040-Zero (#179, [ADR-0019](./adr/0019-rp2040-zero-replaces-esp32-c3.md)).

- **No eFuse identity.** The RP2040 has no user eFuse block to burn a product/board record into, so
  a unit's identity (product, board, revision) comes from its firmware build: the `Identity` the
  firmware sends after `Ready` has `source = Build`, and the image carries the same record next to its
  version marker. The app's wrong-product guard (tier A, [ADR-0016](./adr/0016-locking-a-board-to-its-firmware-in-tiers.md))
  still refuses to flash an image for another product or board.
- **Flashing a new unit** is the app's firmware update, or `just fw-flash` from a checkout: the board
  starts in the ROM UF2 bootloader when BOOT is held while plugging in (a blank board does this on
  its own), and the firmware is copied onto the `RPI-RP2` drive.
- **Per-unit QA** is the hardware checklist in [validation.md](./validation.md) and the build steps
  in [assembly.md](./assembly.md). The old guided input check (turn the knob, press each
  button) is now a manual checklist step.
- The ESP32-C3 SuperMini units that shipped in the beta keep the identity they were provisioned with;
  the app no longer updates their firmware.
