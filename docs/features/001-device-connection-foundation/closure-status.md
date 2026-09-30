# Feature 001 Closure Status

**Status:** software complete; manual acceptance outstanding

**Reconciled:** 2026-09-07

This file is the current closure ledger for `001-device-connection-foundation`. The retired Spec Kit `tasks.md` and related planning files remain available in Git history as audit evidence, but they are no longer part of the working documentation tree. Where historical task annotations conflict with this ledger and the current source, this ledger records the newer evidence.

## Automated/software acceptance

The Feature 001 software surface is implemented and covered by automated gates:

- shared model, protocol, deterministic renderer, compiled assets, and golden frames;
- Device Studio controls and canonical preview rendering;
- host-testable firmware core and host-sim end-to-end coverage;
- ESP32 hardware adapters and the production physical runtime path;
- Windows USB-serial discovery, handshake, reconnect policy, lifecycle policy, diagnostics, and offline guards;
- Wokwi external serial, SPI/RGB565 probe, and production-runtime simulation;
- Windows real-binary startup smoke added in PR #1.

PR #1 host run #16 executed the new `windows-device-studio-startup` job on Windows Server 2025. The job built the frontend and default-feature `kivori-desktop.exe`, launched the real Tauri binary, and recorded `KIVORI-WINDOWS-STARTUP PASS`: the process remained alive through the 10-second observation window with no `stack overflow` or `fatal runtime error` signature. The historical startup failure is therefore not reproducible on this corrected baseline. No production runtime change is justified without a new reproduction.

## Reconciled task evidence

### T074 — complete

T074 is complete in the current source. `firmware/esp32-c3/src/physical_st7789.rs` defines `physical_st7789::run_mode`, which owns and binds the production ports to the host-tested core:

- `UsbJtagTransport` + `TxBuffered` for USB Serial/JTAG;
- SPI2 configured from the verified physical profile;
- the physical ST7789 model, reset/DC/backlight wiring, orientation, color order, inversion, and geometry;
- `MipidsiSink` as the real `DisplaySink`;
- the compiled Kivori asset blob;
- the shared production `runtime::run` loop.

The old T074 `PARTIAL` text described the repository before the physical profile and `physical_st7789::run_mode` were added. It is superseded by the current source.

### T115 — complete

T115's enumeration/identity requirement was physically verified on 2026-08-11. The validation checklist records `0x303A:0x1001`, automatic discovery with no manual COM-port selection, and a successful versioned handshake. The separate `< 5 s` timing requirement remains unmeasured and is still tracked by T121/checklist items 6 and 20.

### T118 — complete

T118's display-initialization/offset requirement was physically verified on 2026-08-11. The checklist records a working physical ST7789 at 240x240 with controller offset `(0,0)`, and separately records the controller identity and physical pin/profile facts.

### Still manual / incomplete

These historical task IDs remain open because their required evidence is not present and cannot be inferred from CI:

- **T093:** real OS hide/background/reactivate/explicit-Quit behavior while connection work continues;
- **T116:** sustained USB throughput and host-not-draining/transmit-stall recovery;
- **T117:** physical Windows unplug/replug and rapid-cycle reconnect reliability;
- **T119:** sustainable physical SPI frame-rate measurements;
- **T120:** physical preview/golden parity for all required companion states;
- **T121:** measured state/connect/reconnect latency targets;
- **T122:** pinned esp-hal `usb_serial_jtag` behavior verified on physical hardware.

Additional blank rows in [`validation-checklist.md`](./validation-checklist.md) remain manual evidence gaps even where related logic has strong host-sim or Wokwi coverage.

## Verified physical profile

The current production profile is not the old Wokwi placeholder. Physical validation on 2026-08-11 established:

- ESP32-C3, native USB Serial/JTAG;
- ST7789, 240x240 RGB565;
- SPI2, 20 MHz, Mode 3;
- SCK GPIO6, MOSI GPIO7, no CS;
- D/C GPIO2, reset GPIO3, backlight GPIO8 active-high;
- offset `(0,0)`;
- 90° rotation, RGB color order, inversion enabled.

Wokwi remains simulation evidence only. Its generic probe does not become physical evidence merely because the physical profile is now known; simulator and physical observations remain separate evidence classes.

## Closure rule

Feature 001 may be described as **software complete** once the final PR #1 head is green across host, firmware, determinism, Wokwi, and the Windows startup smoke. It must not be described as **fully manually accepted** until the open items above have dated evidence in [`validation-checklist.md`](./validation-checklist.md).
