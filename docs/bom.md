# Kivori bill of materials (beta draft)

Drafted from the repo, not from purchase orders. Cells marked **OWNER** are blank on purpose:
prices and suppliers need the owner. Roadmap M3 item "BOM and real unit cost" stays open until they
are filled in. Related: [roadmap.md](./roadmap.md), [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md),
[validation.md](./validation.md).

## Electronics and bought parts

| Part | Exact type / spec as used | Qty | Notes | Unit price | Supplier |
|---|---|---:|---|---|---|
| Controller board | Waveshare RP2040-Zero. Native USB (`0x2E8A:0x000A`), the full male header (all 23 pins) soldered on, as the carrier has a hole for every one (see [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md)) | 1 | BOOT (UF2 recovery) is reachable with the case open; a 1.4 mm hole in the bottom cover is aligned to RESET. The board's own WS2812 LED is not used | **OWNER** | **OWNER** |
| Display | ST7789 1.3" 240x240 SPI module, 7-pin (SCL, SDA, RES, DC, BLK, VCC, GND), no CS | 1 | Verified on the board 2026-08-11 (offset 0,0, RGB order, inversion on). 3.3 V | **OWNER** | **OWNER** |
| Rotary encoder | HW-040 (EC11-type with push switch), CLK/DT/SW/+/GND, 3 pull-up resistors on the module | 1 | Beta part. Decision #8: HW-040 for beta, PEC11R/EC11 for v1. Power from 3V3, never 5 V. Pins CLK GP5, DT GP4, SW GP3 | **OWNER** | **OWNER** |
| Keys | Outemu mechanical switch, one per key (top housing 15.6 mm, bottom housing 13.95 mm, 4.0 mm travel; dimensions in [hardware/enclosure-v8/README.md](../hardware/enclosure-v8/README.md)) | 3 | Left / middle / right on GP6 / GP7 / GP8, each between its GPIO and GND, no resistors. They sit in the printed switch plate under the printed caps | **OWNER** | **OWNER** |
| Wire | 0.3 mm copper wire, treated as possibly bare | 1 set | Wire-wrapped on the pins and laid in the carrier's grooves | **OWNER** | **OWNER** |
| USB cable | USB-C **data** cable (not charge-only) | 1 | Power, flashing and the data link; no UART bridge | **OWNER** | **OWNER** |
| Rubber bumpers | Round, stick-on, Ø8 to Ø12 mm | **OWNER** | On the flat bottom of the back plate, over the screw counterbores | **OWNER** | **OWNER** |

## Printed parts

Source: `hardware/enclosure-v8/export/*.stl`, already in print orientation. PETG, 0.4 mm nozzle,
0.20 mm layers, textured PEI; the carrier prints flat on its back with 3 perimeters. The enclosure
README gives **no filament estimate**, so the weight column is for the owner to fill from the
slicer.

| Part (STL) | Colour | Qty | Notes | Filament (g) | Unit cost |
|---|---|---:|---|---|---|
| `Shell` | Black | 1 | Face down | **OWNER** | **OWNER** |
| `DisplayCowl` | Black | 1 | Tilted top down; friction-fit pegs | **OWNER** | **OWNER** |
| `DisplayLip` | White | 1 | Tilted top down; glued into the cowl window | **OWNER** | **OWNER** |
| `Carrier` (printed PCB) | Black | 1 | Flat, back down | **OWNER** | **OWNER** |
| `BackPlate` | White | 1 | Plinth bottom down | **OWNER** | **OWNER** |
| `Knob` | Black | 1 | Top down | **OWNER** | **OWNER** |
| `KnobCap` | White | 1 | Top down | **OWNER** | **OWNER** |
| `CapLeft`, `CapMiddle`, `CapRight` | White | 3 | One of each, top down | **OWNER** | **OWNER** |
| `HaloDiffuser` | White | 1 | Face down | **OWNER** | **OWNER** |
| `PinFitCoupon` | Black | 1 | Test print only: check the header-pin fit before printing the carrier | **OWNER** | **OWNER** |

`export/pcb-outline.dxf` is the carrier outline, not a printed part.

## Fasteners and consumables

The back plate has stepped screw seats, but the enclosure README states neither the screw size nor
the count (see Open questions). The cowl pegs are a friction fit; the lip is glued into the cowl
window. A soldering iron is needed for the header and the wire wraps.

## Open questions

- **Totals.** No unit cost can be computed until the **OWNER** cells are filled in.
- Regulatory cost (FCC / CE, #36) and packaging are not in this list.
- **Screws.** Size and count are not stated in the enclosure README; they must be measured from the back plate before purchase.
