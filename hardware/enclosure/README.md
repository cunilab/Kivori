# Kivori enclosure (prototype v3, "big keycap")

A 110 × 70 mm wedge case (30 mm thick at the far edge, 18 mm at the near edge, so the face tilts
about 10° towards you). It follows the blueprints in [`docs/assets/`](../../docs/assets/) and is
10 mm wider than drawn: at 100 mm, the Ø48 knob overlaps the left button.

The look is a big keycap, matching the keycap mascot on screen:

- **Shell:** the walls lean in 6° towards the face, and a 45° bevel frames it. The sides are
  clean: no latch windows or screws.
- **Visor:** a black panel, 1 mm proud of the face, carries the display window and the three
  button openings. Its edge follows the knob at an even gap.
- **Knob:** Ø48 × 13.2 mm. The encoder hangs deep in the case (see below), so a knob this low
  still covers its whole shaft and nothing needs cutting. Fine ribs round the edge give grip, with a line
  marker on top. Inside it is hollow: a hub round the shaft (round bore, then D-shaped over the
  shaft's flat), tied to the skirt by four ribs.
- **Plinth:** dark and set back under the shell, so the light shell seems to float above the desk.
  Its sides slope out at 45° to the back plate's edge, so the plate prints with no overhang.
- **Underside:** the 5 mm wordmark (no fine print: it prints on the bed), and four printed
  TPU rubber dots, one in each corner. Each dot (Ø6) is centred on the screw there (so it hides
  it), sits fully inside the plinth's rounded corner and presses a plug into the screw
  counterbore, so the plinth needs no recess.
- **Colours:** light matte shell; black visor, knob and plinth; accent-colour caps.

Inside is a **printed PCB** (`Carrier`): a plate with the outline, mounting holes and part positions
a real PCB would have. Components sit on it and are hand-wired point to point; the back is
flat. Once the prototype works, `export/pcb-outline.dxf` becomes the Edge.Cuts layer in KiCad.

Four M2 × 12 self-tapping screws hold it together, so it stays shut when dropped. Each one goes
up from under the plinth through a back-plate column and the printed PCB into a post on the
faceplate, and clamps all three together. The four posts sit symmetrically near the corners
(left and right mirror each other); they are slim (Ø4.2) so the front-right one clears the right
cap's flange. A spigot on each post runs right through the printed PCB and 1.5 mm on into a
socket in the back-plate column, so the PCB is held over its whole thickness. The four
hooks on the back plate (45° ramps both ways) stay: they line the plate up and hold it while you
screw.

How each part is held (on the printed PCB, except the HW-040), even with the case open:

| Part | Held by |
|---|---|
| HW-040 | It hangs behind the printed PCB. Copper-trace version: on two printed spacer tubes, an M2 × 12 self-tapping screw from its back through each hole and spacer into a boss on the printed PCB; its five pins are bent up into pads on the copper. Hand-wired version: screwed onto two back-plate columns. Either way a small pad on the back plate stops a knob press tipping it. The EC11 body reaches forward through a hole in the printed PCB. Sitting this deep (board front 17.5 mm behind the face) lets the long shaft end low, so the knob stays short. |
| Display | Four standoffs whose pegs go through its four corner holes. |
| SuperMini | With copper traces: stands on the front on its soldered header strips, pins soldered on the back (see Copper-tape traces). Hand-wired version: soldered pin headers: the two black header strips drop into two slots in the printed PCB, so the board lies flat on the back. The long pins point to the face; a 1 mm pocket in the faceplate gives their tips room. When closed, a pad on the back plate presses up under the USB connector. The board end sits up against a hidden pocket in the far wall, which leaves 0.8 mm of wall round the receptacle, so the port outside is just a small pill-shaped hole (9.6 × 4 mm) and a cable still plugs in fully. |
| 12×12 switches | Legs through the plate, bent over on the back before soldering. Each cap clicks onto the switch like a bought round 12×12 cap. The stem is a 3.8 mm square head on a narrower neck; four 2 mm wide fingers hang from the cap top round the head, and a lip on each snaps in under it (45° lead-in below the lip). A stop post on the cap top rests on the head, so the cap is held both ways without play and a press goes straight into the switch. |

The display pegs are 0.2 mm smaller than the holes (press fit), the HW-040 pegs (on the back plate) 0.4 mm smaller
(2.8 mm in its 3.2 mm holes). All stick out 1 mm past the module board.
To lock a part for good, touch each peg tip with the soldering iron so it melts into a small head
(heat-staking). To remove the part later, cut the head off. The faceplate has relief pockets over
the display pegs, so a staked head still fits.

## Files

| File | What |
|---|---|
| `build_case.py` | The whole model. Every dimension is a parameter at the top. |
| `kivori-case.FCStd` | Generated FreeCAD document (printed parts plus blue stand-ins for the bought parts). |
| `export/*.stl` | Printed parts, already in print orientation (binary STL). |
| `export/pcb-outline.dxf` | PCB outline, mounting holes and pin holes, in mm (the real-PCB version: SuperMini pin holes, not the printed slots). |

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

Checked for FDM in each part's print orientation:

- **Overhangs:** nothing is steeper than 45° except short bridges (at most 2.3 mm: the visor-peg
  holes and the top of the USB hole) and the caps' 0.3 mm snap lips. The face bevel is a true 45°
  once the wall lean is added, the cap flanges slope 45° (the caps print top down), the plinth's
  sides are 45° to the bed, and the knob's grip grooves run out of the skirt edge.
- **Thin features:** nothing thinner than 0.7 mm (about two nozzle widths). On the printed PCB's
  copper side every gap is about 0.8-1 mm wide, plastic left between two gaps is at least 1 mm,
  and the pin rows stay solid. Small text is gone from the bed sides; the wordmark is engraved.
- **Fits:** FDM prints holes 0.1-0.2 mm small, so the fits allow for it: spigot 3.4 in 3.9, visor
  peg 1.8 in 2.3, knob bore 6.25 on the 6 mm shaft, cap 13.4 in a 14 mm opening.
- **Small parts:** the two `Spacers` are only Ø4.4: print them with a brim. The caps' snap
  fingers are 0.8 mm: if one cracks, print the caps in PETG.

| Part | Material | Orientation (already applied in the STL) |
|---|---|---|
| `Shell` | matte light PLA (white or warm grey) | face down |
| `Visor` | black PLA; a smooth PEI sheet gives it a glossy face | top down |
| `BackPlate` (with the plinth) | black **PETG** (the hooks crack in PLA) | desk side down |
| `Carrier` (printed PCB) | any PLA (hidden) | back down, pegs and standoffs up |
| `Knob` | black PLA | top down |
| `CapLeft` / `CapMiddle` / `CapRight` | accent-colour PLA (PETG if the fingers crack) | top down |
| `RubberDots` (all four in one file) | **TPU** (95A), black | flat, plugs up |
| `Spacers` (both in one file, copper-trace version) | any PLA/PETG | on end |

Every part is printed in a single colour; the two-tone look comes from printing the parts in
different filaments, so no filament change mid-print is needed.

## Bought parts

| Part | Notes |
|---|---|
| ESP32-C3 SuperMini | Behind the printed PCB, components facing the back plate, pin headers soldered with the plastic strips on the non-component side. Wires solder to the long pins. |
| ST7789 1.3" 240×240, 7-pin | On four standoffs with pegs, glass in a 0.6 mm pocket behind the window, header pins through the slot. |
| HW-040 rotary encoder | Hangs behind the printed PCB (see above). Its right-angle header points towards the middle of the case. The nut is not used (the bushing ends inside the face). |
| 3 × 12×12 mm push switch, 10 mm tall | Legs go through the printed PCB. |
| Thin wire (26–30 AWG) | Point to point on the back. |
| 4 × M2 × 12 self-tapping screws, pan head up to Ø4 | From under the plinth into the face posts. |
| 2 × M2 × 12 self-tapping screws, pan head up to Ø4 | Copper-trace version: HW-040 onto the printed PCB (hand-wired version: 2 × M2 × 8 into the back plate). |
| Kapton (polyimide) tape, a few cm | Copper-trace version: the two crossover patches. |

## Assembly

1. Press the display onto its four pegs on the printed PCB, header through the slot. Press the
   HW-040 onto its two pegs on the back plate, encoder pointing up. Optionally heat-stake the
   six peg tips.
2. Put the three switches through their holes and bend the legs over on the back.
3. Drop the SuperMini's header strips into the two slots on the back, components facing out and
   USB-C towards the open end.
4. Wire point to point (table below); the HW-040's wires run from the back plate to the printed
   PCB, so leave them a little slack.
5. Drop the three caps into the face openings from inside the shell, flange first.
6. Lower the printed PCB over the four spigots and press it down onto the posts: each switch head
   clicks into its cap. The spigots stick out past the back and locate the back plate's columns.
7. Press the back plate straight in, the encoder through the hole in the printed PCB and its
   shaft through the face hole, until all four hooks click. Then drive the four screws in
   from under the plinth until snug (do not overtighten in PLA). Press the four TPU dots on,
   one plug into each screw hole.
8. Press the visor's three pegs into the face. A drop of CA glue or thin double-sided tape keeps
   it there.
9. Push the knob onto the D-shaft.

To open: pull the four dots off the screws, take the screws out, grip the plinth in the shadow
gap and pull. Copper-trace version: RST is a Ø1 needle hole in the visor, above the SuperMini
(a needle or SIM pin reaches the button through a guide tube). Hand-wired version: RST and BOOT
are pinholes in the plinth, labelled; a paperclip reaches the
SuperMini's buttons.

Labels are small (5 mm wordmark, 1.4–2 mm otherwise). Each sits in a recessed rounded box,
0.4 mm deep, with its letters left standing inside, so the text shows up out of the recess. A
0.15 mm stroke (`TEXT_STROKE`) keeps the small raised letters printable. The printed PCB's back
carries the pin and button names next to their holes.

## Copper-tape traces (`COPPER_TRACES = True`)

Instead of hand wiring, the back of the printed PCB carries copper-tape traces, like a one-sided
PCB: parts on the front, copper on the back. Each trace and pad is flush with the back, with a gap
0.8 mm wide and 0.6 mm deep along both sides, so taping is: lay the tape flat, cut along the gaps,
peel off the rest. Traces are 1.2 mm wide (1.0 mm beside the encoder and along the top edge), at
least 0.8 mm apart. Wherever two gaps would leave less than 1 mm of plastic between them (or to a
hole or the edge), that strip becomes gap too, so nothing too thin prints. The gaps are on the bed
side: gaps in the first layers with short bridges over them. The 2.54 mm pin rows (SuperMini,
display, encoder) stay solid, because a gap between two pins would leave rings too thin to print:
one short blade cut between neighbouring pins separates them, like a stripboard track.

The SuperMini stands on the front on its own soldered header strips (components towards the face,
USB-C at the far edge), and its pins come through to the copper. On the back its left pin row
(5V, GND, 3V3, IO4 to IO0) faces the knob side and its right row (IO5 to IO21) faces the display.
The pin map is chosen for that (see Wiring, and issue #67 for the firmware change):

- Six traces run down the channel between the rows: encoder DT and SW straight into the
  encoder's pins, and GND, 3V3, RES and DC fanning out to the display.
- CLK, 3V3 and GND (1.0 mm wide) run down the outside of the left row to the encoder; GND goes
  on to the switches.
- From the right row, SCL and SDA go down onto the display pins, BLK goes over the top of the
  display pins and in from the right, B3 runs along the edge to the right switch, and B1 and B2
  go straight down to their switches.

Two places need a crossover, shown amber in the model (`Dummy_Kapton`): SCL and SDA over BLK,
GND and VCC next to the display, and B1 and B2 over the display bundle next to the SuperMini.
There the gap of the upper traces stops at the patch: cover the lower copper with a small piece
of Kapton tape and lay the upper traces' copper over it.

The HW-040 hangs from the printed PCB on two printed spacer tubes (`Spacers`), with an M2 × 12
self-tapping screw from its back through each hole and spacer into a boss on the printed PCB. Bend
its five right-angle pins up 90° right at the plastic, so they come up through five pads on the
copper. All five are copper: CLK, + and GND come in from the west, DT and SW from the east.

1. Print the printed PCB in **PETG** (PLA softens under the iron).
2. Lay copper tape flat over the back and rub it down so the gaps show through.
3. Run a fresh blade along every gap (the gap guides it) and peel the copper off everything that is
   not a trace or pad. Make one short cut between neighbouring pins of the pin rows. Leave out the
   upper traces over the two crossovers for now.
4. Stick a piece of Kapton tape over each crossover, then lay the upper traces (SCL, SDA; B1, B2)
   across it, joining the copper on both sides; trim it to the trace width.
5. Fit the display, the three switches and the SuperMini from the front; solder every pin on the
   back. Bend the switch legs over onto their pads first.
6. Screw the HW-040 on with its spacers, pins bent up through their pads, and solder them.
7. Solder with low-temperature solder (Sn42Bi58, about 140 °C) and quick touches.

GND reaches the three switches down the outside of the left pin row and then through each
switch's own lower pin pair (the two pins of a pair are joined inside the switch). The SuperMini
faces the face, so RST is reached through a Ø1 needle hole in the visor with a guide tube down to
the button (`RST_HOLE_D`; RST/BOOT positions are NOMINAL, `SM_BTN_*`). Set
`COPPER_TRACES = False` for the hand-wired version (SuperMini behind the printed PCB, header
strips in slots, RST/BOOT pinholes in the plinth).

## Wiring

Pins of the copper-trace version (the hand-wired version uses the current [`profile.rs`](../../firmware/esp32-c3/src/profile.rs)).

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
| IO0 | DT | encoder DT (copper-trace version; hand-wired: IO5) |
| IO1 | SW | encoder SW (copper-trace version; hand-wired: IO10) |
| IO20 / IO10 / IO5 | B1 / B2 / B3 | left / middle / right switch, other leg (hand-wired: IO0 / IO1 / IO20) |

The copper-trace pin map moves the encoder's DT and SW and the three buttons; the display pins
stay. The firmware change is issue #67 (`ROTARY = { clk: 4, dt: 0, sw: 1 }`,
`CONTEXT_BUTTONS = [20, 10, 5]`). All buttons stay on non-strapping pins. Validation row 5.1 still
applies: check that the SuperMini has no 32.768 kHz crystal on GPIO0/GPIO1 (now the encoder).

## Dimensions to verify (NOMINAL in `build_case.py`)

The HW-040 shaft and holes, the display holes, the display glass and the SuperMini header were
measured on the real parts from photos next to a ruler (about ±0.3–0.5 mm). The rest is scaled from a dimensioned drawing and product photos
(about ±0.5 mm) or comes from KiCad footprints and seller listings. Ranked by risk:

1. **Display module** (`DISP_*`):
   - 4 corner holes, Ø2, centres 2.5 mm in from both edges (measured).
   - Board 39.22 × 27.78 mm, 3.0 mm thick with the glass.
   - Glass 28.0 × 26.3 mm including the FPC ledge, centre 0.8 mm from the board centre towards the
     header (measured). The visible area is centred 1.85 mm from the board centre towards the
     header, and the window is centred on it (measured: v1 showed the blue ledge at the window's
     left edge).
   - Header row 2.2 mm in from its end, header on the right as mounted (`DISP_HEADER_SIDE`).
2. **HW-040** (`HW040_*`, in the module's own frame, origin at the board centre):
   - Board 26 × 18.5 × 1.3 mm.
   - Shaft at (−2.3, +1.8) (measured).
   - M3 holes, Ø3.2, at (−8.55, −6.9) and (+8.55, −6.9) (measured).
   - Header row at x = +10.35.
   - Above the board front (measured from a side photo, scale checked on the 6 mm shaft): body
     top 7.7, bushing thread ends 16.2, shaft round to 22.1, then a 7.1 mm D-flat to the tip at
     29.2. The knob height follows the shaft tip (`EC11_SHAFT_H`), so the shaft is never cut.
3. **SuperMini**: BOOT/RST positions (`SM_BTN_*`), header pin order (`SM_COL_A/B`), USB-C size,
   and how far the receptacle sticks out past the board end (`SM_USB_OVERHANG`, 0.5 mm): that sets
   how close the receptacle comes to the port hole.
   Pin headers: plastic 2.5 mm, long pins 6 mm past it (measured).
4. **12 mm switch**: snap-fit stem (Same Sky TS13 drawing), 10 mm tall version.
   - Housing 12 × 12 × 3.5 mm with a round Ø7 platform up to 4.3 mm.
   - Stem: a 3.1 mm square neck, then a 3.8 × 3.8 mm head 1.8 mm tall, top at 10 mm (sellers of
     the round caps describe the same stem as "3.4 × 3.8 mm with a locking notch").
   - The cap's lips reach 0.3 mm under the head (`CAP_SNAP`). Measure your head: if the cap
     falls off, raise `CAP_SNAP` a little (or lower `CAP_SNAP_CLR`); if it will not click on or a
     finger cracks, lower it.
   - Pins on a 12.5 × 5 mm grid (land pattern Ø1.5). Two extra Ø2 holes, 9 mm apart, clear the
     locating bosses some 12 × 12 switches have (Omron B3F); TS-G005 and TS13 show none, so the
     copper-trace version leaves them out.

A cheap check before the full print: print only the `Carrier` first (about 1 h), then press the
parts on. If a peg misses its hole, measure the hole position, change the parameter and rebuild.
