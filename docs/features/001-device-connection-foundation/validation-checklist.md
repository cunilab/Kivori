# Manual validation checklist

Human-run checks that host CI **cannot** cover: the window-hidden background behaviour and on-device integration on a physical ESP32-C3. These do not gate ordinary merge CI. The full procedure lives in [`quickstart.md`](./quickstart.md); this file is where results are recorded. Targets come from the Feature 001 requirements and the [hardware-validation research](./research.md#hardware-validation-required-do-not-trust-without-a-physical-esp32-c3).

**Software-closure note (2026-09-07):** GitHub Actions host run #16 on PR #1 added and passed a real Windows startup smoke for the default-feature Tauri binary: `kivori-desktop.exe` remained alive for the 10-second observation window with no `stack overflow` or `fatal runtime error` signature. That removes the previous startup blocker from automated acceptance, but it does not manufacture any of the manual checks below. Rows without a result remain outstanding until they are executed on a real desktop and/or physical device.

Fill `Result` with ✅ / ❌ and date; put measurements and observations in `Notes`.

## Background operation

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 1 | Close the window → it hides; the process keeps running | FR-030 | ✅ macOS 2026-09-25 · Windows pending | macOS 27, debug build, no device. Closing via the title-bar button hid the window. The process stayed alive, and bringing the app to the front showed no window, so it was hidden rather than moved to another Space. Checked with the CGWindowList on-screen flag. |
| 2 | While hidden, the device heartbeat + synchronization stay active | SC-007 | | |
| 3 | Re-activate → the single window reappears | FR-030 | ✅ macOS 2026-09-25 · Windows pending | Second launch: the new instance exits, the original window returns, and there is still one process. **Dock click failed at first:** `RunEvent::Reopen` was not handled. Fixed in `lib.rs` and re-verified: the window returns with one process. |
| 4 | Explicit Quit → the process terminates and the device task stops | FR-030 | ✅ macOS 2026-09-25 · Windows pending | Tray → Quit: the process exited with code 0 within about 1 s. `AppState::shutdown()` signals and joins the device thread before exit. |

## Hardware — US1 discovery & connection

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 5 | Device enumerates as `0x303A:0x1001`; auto-discovered with **no** port selection; identity confirmed via handshake | SC-001 | ✅ 2026-08-11 | Windows detected the ESP32-C3 as `0x303A:0x1001` on COM3. Kivori Desktop auto-discovered the device with no manual port selection and completed the handshake. Desktop reported Connected, firmware 1.0.0, protocol 1.0. |
| 6 | Time from plug-in → `connected` | < 5 s (SC-001) | | Connection was observed successfully, but plug-in → connected latency was not measured from a controlled plug-in event. |
| 7 | An unrelated USB-serial device is **not** reported as connected | FR-004 | | |
| 8 | A wrong-major firmware → `incompatible` with a clear reason, no state commands sent | < 5 s (SC-003) | | |

## Hardware — US2 companion states

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 9 | Display initialization + panel offsets correct | R-3 | ✅ 2026-08-11 | Physical ST7789 240x240 initialized successfully on ESP32-C3 with offset `(0,0)`. |
| 10 | Each of `idle/happy/busy/sleeping` shows the matching animated scene | — | | `idle` confirmed on physical hardware; the remaining states still need physical verification. |
| 11 | State change appears on the device | < 1 s (SC-004) | | Desktop → device `idle` propagation was observed, but latency was not measured. |
| 12 | On-device image matches Device Studio preview for a fixed state + elapsed time | SC-005 | | Preview ↔ physical-panel parity has not been manually executed. |
| 13 | Sustainable SPI frame rate recorded | SC-004 / R-4 | | |
| 14 | Only semantic state is sent, verified through typed session activity (not payload bytes) | FR-015 | | |

## Hardware — US3 recovery & restoration

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 15 | Unplug while `busy` → app shows `disconnected` | — | | |
| 16 | Replug → auto-reconnect, device returns to `busy` with no user action | < 10 s reconnect+restore (SC-002) | | |
| 17 | Rapid unplug/replug settles into a stable connected state | FR-010 | | |
| 18 | USB throughput + transmit-stall recovery under sustained traffic | R-1/R-2 | | |
| 19 | Full desktop restart → desired state resets to `idle` | clarified | | Code review 2026-09-25 (does not tick the row): `Orchestrator::new()` starts at `Idle` with no persistence. The webview saves only personality and self-play, and sends a desired state only on an explicit Mirror click. Needs a connected device to observe. |

## Hardware — platform & safety

| # | Check | Target | Result | Notes |
|---|---|---|---|---|
| 20 | End-to-end latencies within targets: state < 1 s, connect < 5 s, reconnect+restore < 10 s | SC-001/002/004 | | |
| 21 | esp-hal `usb_serial_jtag` behavior verified on the pinned version | R-2/R-11 | | |
| 22 | No sensitive data appears in the typed session activity view while connected; no activity log file is created | SC-010 | | Partial, macOS 2026-09-25: no activity or log file is created. The app writes only WebKit's own storage and stdout. The "while connected" part needs the device. |
| 23 | Physical panel controller identified and device profile updated | R-3 | ✅ 2026-08-11 | Physical panel confirmed as ST7789. |
| 24 | Real SPI pin map recorded | R-3 | ✅ 2026-08-11 | SCK GPIO6, MOSI GPIO7, no CS, D/C GPIO2, RST GPIO3, backlight GPIO8 active-high; SPI2 at 20 MHz Mode 3. |
| 25 | Panel offsets measured and passed to `PanelGeometry` | R-3 | ✅ 2026-08-11 | Physical ST7789 uses controller offset `(0,0)`. |

Items 23–25 are physical facts established from the real ESP32-C3 + ST7789 hardware. The Wokwi generic SPI probe remains simulation-only and does not establish controller identity, physical pin routing, panel offsets, orientation, color order, inversion, backlight polarity, or electrical timing margins.

## Verified physical Kivori profile

| Property | Verified value |
|---|---|
| MCU | ESP32-C3 |
| USB | Native USB Serial/JTAG |
| USB VID:PID | `0x303A:0x1001` |
| Display controller | ST7789 |
| Resolution | 240x240 |
| Pixel format | RGB565 |
| SPI peripheral | SPI2 |
| SPI clock | 20 MHz |
| SPI mode | Mode 3 |
| SCK | GPIO6 |
| MOSI | GPIO7 |
| MISO | Unused |
| CS | Unused |
| D/C | GPIO2 |
| Reset | GPIO3 |
| Backlight | GPIO8, active-high |
| Panel offset | `(0,0)` |
| Rotation | 90° |
| Color order | RGB |
| Color inversion | Enabled |

This profile is the physical Kivori hardware profile and remains separate from Wokwi simulation evidence.
