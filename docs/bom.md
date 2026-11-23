# Kivori bill of materials

## Electronics and bought parts

| Part | Exact type / spec as used | Qty | Notes | Unit price | Supplier |
|---|---|---:|---|---|---|
| Controller board | Waveshare RP2040-Zero. Native USB (`0x2E8A:0x000A`), the full male header (all 23 pins) soldered on, as the carrier has a hole for every one (see [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md)) | 1 | BOOT (UF2 recovery) is reachable with the case open; a 1.4 mm hole in the bottom cover is aligned to RESET. The board's own WS2812 LED is not used | — | — |
| Display | ST7789 1.3" 240x240 SPI module, 7-pin (SCL, SDA, RES, DC, BLK, VCC, GND), no CS | 1 | Verified on the board 2026-08-11 (offset 0,0, RGB order, inversion on). 3.3 V | — | — |
| Rotary encoder | HW-040 (EC11-type with push switch), CLK/DT/SW/+/GND, 3 pull-up resistors on the module | 1 | Power from 3V3, never 5 V. Pins CLK GP5, DT GP4, SW GP3. Its 3V3 and GND are wired from the display's VCC and GND pins, not from the RP2040 pins | — | — |
| Keys | Outemu mechanical switch, one per key (top housing 15.6 mm, bottom housing 13.95 mm, 4.0 mm travel; dimensions in [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md)) | 3 | Left / middle / right on GP6 / GP7 / GP8, each between its GPIO and GND, no resistors. They sit in the printed switch plate under the printed caps | — | — |
| Wire | 0.3 mm copper wire, treated as possibly bare | 1 set | Wire-wrapped on the pins and laid in the carrier's grooves | — | — |
| USB cable | USB-C **data** cable (not charge-only) | 1 | Power, flashing and the data link; no UART bridge | — | — |
| Screws | M2 x 12 self-tapping, pan head up to 4 mm | 4 | One from under the plinth through each back-plate column, the carrier and into a face post; clamps back plate, carrier and shell | — | — |

## Printed parts

Source: `hardware/enclosure-v8/export/*.stl`, already in print orientation. PETG, 0.4 mm nozzle,
0.20 mm layers, textured PEI; the carrier prints flat on its back with 3 perimeters. Filament use depends on slicer settings.

| Part (STL) | Colour | Qty | Notes | Filament (g) | Unit cost |
|---|---|---:|---|---|---|
| `Shell` | Black | 1 | Face down | — | — |
| `DisplayCowl` | Black | 1 | Tilted top down; friction-fit pegs | — | — |
| `DisplayLip` | White | 1 | Tilted top down; glued into the cowl window | — | — |
| `Carrier` (printed PCB) | Black | 1 | Flat, back down | — | — |
| `BackPlate` | White | 1 | Plinth bottom down | — | — |
| `Knob` | Black | 1 | Top down | — | — |
| `KnobCap` | White | 1 | Top down | — | — |
| `CapLeft`, `CapMiddle`, `CapRight` | White | 3 | One of each, top down | — | — |
| `HaloDiffuser` | White | 1 | Face down | — | — |
| `PinFitCoupon` | Black | 1 | Test print only: check the header-pin fit before printing the carrier | — | — |

`export/pcb-outline.dxf` is the carrier outline, not a printed part.

## Fasteners and consumables

The four back-plate screw seats take M2 x 12 self-tapping screws (see the table above). The cowl pegs are a friction fit; the lip is glued into the cowl
window. A soldering iron is needed for the header and the wire wraps.
