# Kivori bill of materials (beta draft)

Drafted from the repo, not from purchase orders. Cells marked **OWNER** are blank on purpose:
prices and suppliers need the owner. Roadmap M3 item "BOM and real unit cost" stays open until they
are filled in. Related: [roadmap.md](./roadmap.md), [hardware/enclosure/README.md](../hardware/enclosure/README.md),
[validation.md](./validation.md).

## Electronics and bought parts

| Part | Exact type / spec as used | Qty | Notes | Unit price | Supplier |
|---|---|---:|---|---|---|
| Controller board | Waveshare RP2040-Zero (the enclosure's target, v7). Native USB (`0x2E8A:0x000A`), male header pins soldered on only for the pins the carrier uses (see [hardware/enclosure-v7/README.md](../hardware/enclosure-v7/README.md)) | 1 | BOOT (UF2 recovery) and RESET reachable through the plinth pinholes. The board's own WS2812 LED is not used | **OWNER** | **OWNER** |
| Display | ST7789 1.3" 240x240 SPI module, 7-pin (SCL, SDA, RES, DC, BLK, VCC, GND), no CS | 1 | Verified on the board 2026-08-11 (offset 0,0, RGB order, inversion on). 3.3 V | **OWNER** | **OWNER** |
| Rotary encoder | HW-040 (EC11-type with push switch), CLK/DT/SW/+/GND, 3 pull-up resistors on the module | 1 | Beta part. Decision #8: HW-040 for beta, PEC11R/EC11 for v1. Power from 3V3, never 5 V. Pins CLK GP5, DT GP4, SW GP3 are a spec until row 3.15 is ticked | **OWNER** | **OWNER** |
| Push buttons | Momentary tactile switch, 12×12 mm, 10 mm tall, projected plunger (seller drawing TS-G005), each between its GPIO and GND, no resistors | 3 | Left / middle / right on GP6 / GP7 / GP8. The size the enclosure and its keycaps are built for | **OWNER** | **OWNER** |
| Wire | Thin wire, 26-30 AWG | 1 set | Hand-wired along the grooves on the back of the printed PCB | **OWNER** | **OWNER** |
| USB cable | USB-C **data** cable (not charge-only) | 1 | Power, flashing and the data link; no UART bridge | **OWNER** | **OWNER** |
| Rubber feet | 10 mm stick-on | 4 | In the recesses under the plinth | **OWNER** | **OWNER** |

## Printed parts

Source: `hardware/enclosure/export/*.stl`, already in print orientation. FDM, 0.4 mm nozzle, 0.2 mm
layers, 3 walls, 20 % infill, no supports. The enclosure README gives **no filament estimate**, so
the weight column is for the owner to fill from the slicer.

| Part (STL) | Material | Qty | Notes | Filament (g) | Unit cost |
|---|---|---:|---|---|---|
| `Shell` | Matte light PLA | 1 | Face down | **OWNER** | **OWNER** |
| `Visor` | Black PLA | 1 | Top down; glued or taped with three pegs | **OWNER** | **OWNER** |
| `BackPlate` (with plinth) | Black PETG | 1 | Hooks crack in PLA | **OWNER** | **OWNER** |
| `Carrier` (printed PCB) | Any PLA | 1 | Pegs and standoffs up | **OWNER** | **OWNER** |
| `Knob` | Black PLA | 1 | Pushes onto the D-shaft | **OWNER** | **OWNER** |
| `CapLeft`, `CapMiddle`, `CapRight` | Accent-colour PLA | 3 | One of each | **OWNER** | **OWNER** |

`export/pcb-outline.dxf` is not a printed part: it becomes the KiCad board outline later.

## Fasteners and consumables

The enclosure README lists **no screws or inserts**: the case is snap-fit (four hooks on the back
plate). Optional: a drop of CA glue or thin double-sided tape for the visor, and a soldering iron for
heat-staking the peg tips and soldering wires.

## Open questions

- **Totals.** No unit cost can be computed until the **OWNER** cells are filled in.
- Regulatory cost (FCC / CE, #36) and packaging are not in this list.
