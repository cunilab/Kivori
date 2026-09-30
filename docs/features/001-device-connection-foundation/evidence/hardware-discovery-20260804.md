# Hardware discovery — 2026-08-04

Safe, read-only inventory. No device was flashed, reset, or written to.

| Probe | Command | Result |
|---|---|---|
| Serial ports | `ls /dev/cu.*` | `Bluetooth-Incoming-Port`, `debug-console`, `wlan-debug` — all built-in virtual ports |
| USB serial ports | `ls /dev/tty.*` filtered | none |
| USB device tree | `system_profiler SPUSBDataType` | 0 `Vendor ID` / `Product ID` entries |
| USB registry | `ioreg -p IOUSB -w0` | two `AppleT8103USBXHCI` controllers, **no attached devices** |
| Espressif VID | `ioreg -p IOUSB -w0 -l \| grep -ci '303a\|espressif'` | 0 |
| Flash tooling | `espflash board-info` | `Error: espflash::no_serial — No serial ports could be detected` |

**Conclusion: no ESP32-C3 board is connected.** No uniquely identifiable flash target exists,
so no flash was attempted (T115–T122 stay unchecked).

## Board facts still required

exact model · flash size · native USB Serial/JTAG vs UART bridge · schematic/pinout · unique flash target identity

## Display facts still required

Tracked as `docs/validation-checklist.md` items 23–25, all with empty Result columns:

controller (GC9A01 vs ST7789) · resolution · voltage · SCK · MOSI · CS · D/C · RESET · backlight pin ·
backlight active level · rotation · X offset · Y offset · RGB/BGR order · supported SPI speed

`firmware/esp32-c3/src/profile.rs` contains **only** the simulation profile (SCK 4, MOSI 5, CS 6, D/C 7,
RST 10). Those are Wokwi diagram pins and **must not** be reused as physical pins.
