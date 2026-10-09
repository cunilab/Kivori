# Kivori enclosure (prototype v3, "big keycap")

A 110 × 70 mm wedge case (30 mm thick at the far edge, 18 mm at the near edge, so the face tilts
about 10° towards you). It follows the blueprints in [`docs/assets/`](../../docs/assets/) and is
10 mm wider than drawn: at 100 mm, the Ø48 knob overlaps the left button.

The look is a big keycap, matching the keycap mascot on screen:

- **Shell:** the walls lean in 6° towards the face, and a 45° bevel frames it. The sides are
  clean: no latch windows or screws.
- **Visor:** a black panel, 1 mm proud of the face, carries the display window and the three
  button openings. Its edge follows the knob at an even gap.
- **Knob:** sits inside a thin groove ring. Fine ribs round the edge for grip, and a line marker
  on top.
- **Plinth:** dark and set back under the shell, so the light shell seems to float above the desk.
  The underside carries the wordmark, the RST/BOOT labels and recesses for rubber feet.
- **Colours:** light matte shell; black visor, knob and plinth; accent-colour caps.

Inside is a **printed PCB** (`Carrier`): a plate with the outline, mounting holes and part positions
a real PCB would have. Components sit on it and are hand-wired point to point; the back is
flat. Once the prototype works, `export/pcb-outline.dxf` becomes the Edge.Cuts layer in KiCad.

No screws. Four hooks on the back plate catch ribs inside the walls. Each hook has 45° ramps
both ways, so the plate clicks in and pulls back out. The printed PCB is clamped between posts on
the faceplate and columns on the back plate.

How each part is held on the printed PCB, even with the case open:

| Part | Held by |
|---|---|
| HW-040 | A pocket stops it sliding. Two pegs go through its two M3 holes along the long edge. The pocket floor is open under the encoder legs and the three pull-up resistors. |
| Display | Four standoffs whose pegs go through its four corner holes. |
| SuperMini | Lies against the flat back, over its 16 pin holes. When closed, a pad on the back plate presses up under the USB connector. |
| 12×12 switches | Legs through the plate, bent over on the back before soldering. |

The pegs are 0.2 mm smaller than the holes (press fit) and stick out 1 mm past the module board.
To lock a part for good, touch each peg tip with the soldering iron so it melts into a small head
(heat-staking). To remove the part later, cut the head off. The faceplate has relief pockets over
the display pegs, so a staked head still fits.

## Files

| File | What |
|---|---|
| `build_case.py` | The whole model. Every dimension is a parameter at the top. |
| `kivori-case.FCStd` | Generated FreeCAD document (printed parts plus blue stand-ins for the bought parts). |
| `export/*.stl` | Printed parts, already in print orientation (binary STL). |
| `export/pcb-outline.dxf` | Printed PCB outline, mounting holes and pin holes, in mm. |

To rebuild after changing a parameter, run it headless from the repo root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure/build_case.py
```

or in FreeCAD's Python console (or through the FreeCAD MCP):

```python
p = "<repo>/hardware/enclosure/build_case.py"
exec(open(p).read(), {"__file__": p, "__name__": "__main__"})
```

It prints an interference report (`interference: none` is the goal), regenerates the document
and rewrites `export/`.

## Parts to print

FDM, 0.4 mm nozzle, 0.2 mm layers, 3 walls, 20 % infill. None of the parts needs supports.

| Part | Material | Orientation (already applied in the STL) |
|---|---|---|
| `Shell` | matte light PLA (white or warm grey) | face down |
| `Visor` | black PLA; a smooth PEI sheet gives it a glossy face | top down |
| `BackPlate` (with the plinth) | black **PETG** (the hooks crack in PLA) | desk side down |
| `Carrier` (printed PCB) | any PLA (hidden) | back down, pegs and standoffs up |
| `Knob` | black PLA | top down |
| `CapLeft` / `CapMiddle` / `CapRight` | accent-colour PLA | flange down |

Every part is printed in a single colour; the two-tone look comes from printing the parts in
different filaments, so no filament change mid-print is needed.

## Bought parts

| Part | Notes |
|---|---|
| ESP32-C3 SuperMini | Behind the printed PCB, components facing the back plate. No header pins: solder wires to the pads. |
| ST7789 1.3" 240×240, 7-pin | On four standoffs with pegs, glass in a 0.6 mm pocket behind the window, header pins through the slot. |
| HW-040 rotary encoder | In a pocket on the front of the printed PCB, on two pegs. Its right-angle header points towards the middle of the case; wires drop to the back through the slot past the pin tips. The nut is optional. |
| 3 × 12×12 mm push switch, 10 mm tall | Legs go through the printed PCB. |
| Thin wire (26–30 AWG) | Point to point on the back. |
| 4 × 10 mm stick-on rubber feet | In the recesses under the plinth. |

## Assembly

1. Press the HW-040 onto its two pegs in the pocket on the front of the printed PCB. Press the
   display onto its four pegs, header through the slot. Optionally heat-stake the six peg tips.
2. Put the three switches through their holes and bend the legs over on the back.
3. Lay the SuperMini on the back, components facing out and USB-C towards the open end.
4. Wire point to point (table below).
5. Drop the three caps into the face openings from inside the shell, flange first.
6. Lower the printed PCB onto the four posts, encoder shaft through the face hole.
7. Press the back plate straight in until all four hooks click, then stick the feet on.
8. Press the visor's three pegs into the face. A drop of CA glue or thin double-sided tape keeps
   it there.
9. Push the knob onto the D-shaft.

To open: grip the plinth in the shadow gap and pull. The hooks release without tools. RST and
BOOT are pinholes in the plinth, labelled; a paperclip reaches the SuperMini's buttons.

## Wiring

These are the pins from [`profile.rs`](../../firmware/esp32-c3/src/profile.rs) and the README.

| SuperMini | Net | Goes to |
|---|---|---|
| IO6 | SCL | display SCL |
| IO7 | SDA | display SDA |
| IO2 | DC | display DC |
| IO3 | RES | display RES |
| IO8 | BLK | display BLK (backlight) |
| 3V3 | 3V3 | display VCC, encoder + (**never 5 V**) |
| GND | GND | display GND, encoder GND, one leg of each switch |
| IO4 | CLK | encoder CLK |
| IO5 | DT | encoder DT |
| IO10 | SW | encoder SW |
| IO0 / IO1 / IO20 | B1 / B2 / B3 | left / middle / right switch, other leg |

Validation row 5.1 still applies: check that the SuperMini has no 32.768 kHz crystal on
GPIO0/GPIO1.

## Dimensions to verify (NOMINAL in `build_case.py`)

The HW-040 shaft and holes and the display holes were measured on the real modules from photos
next to a ruler (about ±0.3 mm). The rest is scaled from a dimensioned drawing and product photos
(about ±0.5 mm) or comes from KiCad footprints and seller listings. Ranked by risk:

1. **Display module** (`DISP_*`):
   - 4 corner holes, Ø2, centres 2.5 mm in from both edges (measured).
   - Board 39.22 × 27.78 mm, 3.0 mm thick with the glass.
   - Glass 27.6 mm long including the FPC ledge, centre 1.6 mm from the board centre towards the
     end away from the header. Active area centre 0.5 mm the same way.
   - Header row 2.2 mm in from its end, header on the right as mounted (`DISP_HEADER_SIDE`).
2. **HW-040** (`HW040_*`, in the module's own frame, origin at the board centre):
   - Board 26 × 18.5 × 1.3 mm.
   - Shaft at (−2.3, +1.8) (measured).
   - M3 holes, Ø3.2, at (−8.55, −6.9) and (+8.55, −6.9) (measured).
   - Header row at x = +10.35.
   - Shaft top 21 mm above the board underside. EC11 body 6.5 mm and bushing 7 mm are NOMINAL.
3. **SuperMini**: BOOT/RST positions (`SM_BTN_*`), header pin order (`SM_COL_A/B`), USB-C size.
4. **12 mm switch**: projected-plunger type, seller drawing TS-G005, 10 mm tall version.
   - Housing 12 × 12 × 3.5 mm with a round Ø7 platform up to 4.3 mm.
   - Stem: a 3 mm square neck, then a wider 4 × 4 mm head about 1.8 mm tall, top at 10 mm.
   - The cap socket is 4.3 mm square. It slips over the head and stops the cap turning; the
     flange behind the faceplate is what holds the cap in.
   - Pins on a 12.5 × 5 mm grid (land pattern Ø1.5). Two extra Ø2 holes, 9 mm apart, clear the
     locating bosses some 12 × 12 switches have (Omron B3F); TS-G005 shows none.

A cheap check before the full print: print only the `Carrier` first (about 1 h), then press the
parts on. If a peg misses its hole, measure the hole position, change the parameter and rebuild.
