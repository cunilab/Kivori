# Kivori enclosure, wire version (prototype v3, "big keycap")

A clone of [`../enclosure`](../enclosure/) with the copper-tape traces replaced by **0.3 mm
enamelled copper wire, no solder** (see [Wire wiring](#wire-wiring)). The case, the part positions and the pin
map are the same; only the printed PCB's back and the assembly change.

A 110 × 70 mm wedge case (32.7 mm thick at the far edge, 20.7 mm at the near edge, so the face tilts
about 10° towards you). It follows the blueprints in [`docs/assets/`](../../docs/assets/) and is
10 mm wider than drawn: at 100 mm, the Ø48 knob overlaps the left button.

The look is a big keycap, matching the keycap mascot on screen:

- **Shell:** the walls lean in 6° towards the face, and a 45° bevel frames it. The sides are
  clean: no latch windows or screws.
- **Visor:** a black panel, 1 mm proud of the face, carries the display window and the three
  button openings. Its edge follows the knob at an even gap.
- **Knob:** Ø48 × 10.5 mm. The encoder hangs deep in the case (see below), so a knob this low
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
a real PCB would have. Components sit on its front; the wire runs in channels on its back. Once the prototype works, `export/pcb-outline.dxf` becomes the Edge.Cuts layer in KiCad.

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
| HW-040 | It sits behind the printed PCB on the back plate: two pillars with pegs through its two holes (2.8 mm in 3.2 mm) stop it sliding or turning, and four corner brackets (a 1.2 mm ledge under each board corner and a 1 mm high wall, 0.3 mm clear of the board) stop it tipping when the knob is pressed. The resistors and solder joints in the middle of its back stay clear. Its five header pins stand straight up (no bend) into holes in the printed PCB, and the wire is wound round them in the 5.5 mm gap between the HW-040's header and the printed PCB. Its square EC11 body stops 0.3 mm behind the printed PCB; only the round M7 bushing goes through, in a round Ø7.6 hole that also centres it. Sitting this deep (board front 20.2 mm behind the face) lets the long shaft end low, so the knob stays short. |
| Display | Four standoffs whose pegs go through its four corner holes. |
| SuperMini | Stands on the front on its soldered header strips; the long pins go through to the back (4 mm past it), where the wire is wound round them. The board end sits up against a hidden pocket in the far wall, which leaves 0.8 mm of wall round the receptacle, so the port outside is just a small pill-shaped hole (9.6 × 4 mm) and a cable still plugs in fully. |
| 12×12 switches | Legs through the plate; a leg that takes a wire is folded flat over it on the back. Each cap clicks onto the switch like a bought round 12×12 cap. The stem is a 3.8 mm square head on a narrower neck; four 2 mm wide fingers hang from the cap top round the head, and a lip on each snaps in under it (45° lead-in below the lip). A stop post on the cap top rests on the head, so the cap is held both ways without play and a press goes straight into the switch. |

The display pegs are 0.2 mm smaller than the holes (press fit), the HW-040 pegs (on the back plate) 0.4 mm smaller
(2.8 mm in its 3.2 mm holes). All stick out 1 mm past the module board.
To lock a part for good, touch each peg tip with the soldering iron so it melts into a small head
(heat-staking). To remove the part later, cut the head off. The faceplate has relief pockets over
the display pegs, so a staked head still fits.

## Files

| File | What |
|---|---|
| `build_case.py` | The whole model. Every dimension is a parameter at the top; the wire runs are in `wire_layout()`. |
| `kivori-case-wire.FCStd` | Generated FreeCAD document (printed parts plus stand-ins for the bought parts; `Dummy_Wire` is the wire). |
| `export/*.stl` | Printed parts, already in print orientation (binary STL). |
| `export/pcb-outline.dxf` | PCB outline, mounting holes and pin holes, in mm (for KiCad Edge.Cuts). |

To rebuild after changing a parameter, run it headless from the repo root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-wire/build_case.py
```

or in FreeCAD's Python console (or through the FreeCAD MCP):

```python
p = "<repo>/hardware/enclosure-wire/build_case.py"
exec(open(p).read(), {"__file__": p, "__name__": "__main__"})
```

It prints an interference report (`interference: none` is the goal) and a wire-channel check
(`plastic under 0.7 mm: none` is the goal), regenerates the document and rewrites `export/`.

## Parts to print

FDM, 0.4 mm nozzle, 0.2 mm layers, 3 walls, 20 % infill. None of the parts needs supports.

Checked for FDM in each part's print orientation:

- **Overhangs:** nothing is steeper than 45° except short bridges (at most 2.3 mm: the visor-peg
  holes and the top of the USB hole) and the caps' 0.3 mm snap lips. The face bevel is a true 45°
  once the wall lean is added, the cap flanges slope 45° (the caps print top down), the plinth's
  sides are 45° to the bed, and the knob's grip grooves run out of the skirt edge.
- **Thin features:** nothing thinner than 0.7 mm (about two nozzle widths). On the printed PCB's
  back (on the bed) the wire channels are 0.8 mm wide and the leg pockets 1.4 mm, so the layer
  over each is a short bridge; the grooves along the pin rows have 45° sides, so they need no bridge.
  Every channel keeps at least 0.7 mm of plastic to anything that is not its own (the build
  checks it). Small text is gone from the bed sides; the wordmark is engraved.
- **Fits:** FDM prints holes 0.1-0.2 mm small, so the fits allow for it: spigot 3.4 in 3.9, visor
  peg 1.8 in 2.3, knob bore 6.25 on the 6 mm shaft, cap 13.4 in a 14 mm opening.
- **Small parts:** the caps' snap
  fingers are 0.8 mm: if one cracks, print the caps in PETG.

| Part | Material | Orientation (already applied in the STL) |
|---|---|---|
| `Shell` | matte light PLA (white or warm grey) | face down |
| `Visor` | black PLA; a smooth PEI sheet gives it a glossy face | top down |
| `BackPlate` (with the plinth) | black **PETG** (the hooks crack in PLA) | desk side down |
| `Carrier` (printed PCB) | PETG preferred (the folded switch legs and pegs press on it for years); PLA works | back down, pegs and standoffs up |
| `Knob` | black PLA | top down |
| `CapLeft` / `CapMiddle` / `CapRight` | accent-colour PLA (PETG if the fingers crack) | top down |
| `RubberDots` (all four in one file) | **TPU** (95A), black | flat, plugs up |

Every part is printed in a single colour; the two-tone look comes from printing the parts in
different filaments, so no filament change mid-print is needed.

## Bought parts

| Part | Notes |
|---|---|
| ESP32-C3 SuperMini | On the front of the printed PCB, components facing the face, on its own soldered header strips (long pins down through the printed PCB). |
| ST7789 1.3" 240×240, 7-pin | On four standoffs with pegs, glass in a 0.6 mm pocket behind the window, its soldered header pins through the printed PCB. |
| HW-040 rotary encoder | Hangs behind the printed PCB (see above). Straighten its five right-angle pins with pliers, or fit a straight 5-pin header. The nut is not used (the bushing ends inside the face). |
| 3 × 12×12 mm push switch, 7.3 mm tall | Legs go through the printed PCB. |
| Enamelled copper wire, 0.3 mm, about 1 m | The wiring. |
| 400-grit sandpaper or a craft knife | Scraping the enamel off the wire ends (no heat). |
| Kapton (or masking) tape, a few cm | Holding the wire down in long channels. |
| 4 × M2 × 12 self-tapping screws, pan head up to Ø4 | From under the plinth into the face posts. |

The SuperMini and display arrive with their header pins soldered (or have them soldered once);
everything after that is solderless.

## Assembly

1. Press the display onto its four pegs on the printed PCB, pins through their holes. Optionally
   heat-stake the four peg tips.
2. Push the SuperMini's header pins through the two rows of holes, components towards you, USB-C
   at the open end, until the header strips sit on the printed PCB.
3. Put the three switches through their holes. Clip the unused leg of each (upper left, as seen
   from the face) short.
4. Wire the back (see [Wire wiring](#wire-wiring)): every run except the encoder's.
5. Push the HW-040's straightened pins up into their five holes (it hangs on them) and wrap the
   encoder runs onto them, in the gap under the printed PCB.
6. Check every run with a meter.
7. Drop the three caps into the face openings from inside the shell, flange first.
8. Lower the printed PCB over the four spigots and press it down onto the posts: each switch head
   clicks into its cap. The spigots stick out past the back and locate the back plate's columns.
9. Press the back plate straight in, the encoder through the hole in the printed PCB and its
   shaft through the face hole, its two holes onto the back plate's pegs, until all four hooks
   click. Then drive the four screws in from under the plinth until snug (do not overtighten in
   PLA). Press the four TPU dots on, one plug into each screw hole.
10. Press the visor's three pegs into the face. A drop of CA glue or thin double-sided tape keeps
   it there.
11. Push the knob onto the D-shaft.

To open: pull the four dots off the screws, take the screws out, grip the plinth in the shadow
gap and pull. RST is a Ø1 needle hole in the visor, above the SuperMini (a needle or SIM pin
reaches the button through a guide tube).

Labels are small (5 mm wordmark, 1.4–2 mm otherwise). Each sits in a recessed rounded box,
0.4 mm deep, with its letters left standing inside, so the text shows up out of the recess. A
0.15 mm stroke (`TEXT_STROKE`) keeps the small raised letters printable. The printed PCB's back
carries the pin and button names next to their holes.

## Wire wiring

The back of the printed PCB carries 18 wire channels, one per run, each one piece of 0.3 mm
enamelled wire from a pin to a pin (`wire_layout()` in `build_case.py`). Seen from the face, the
SuperMini's power row (5V, GND, 3V3, IO4 to IO0) is on the right, next to the display, and its IO
row (IO5 to IO10, IO20, IO21) on the left, next to the knob:

- Display: IO4 to IO0 leave the power row side by side and step down onto SCL, SDA, RES, DC and
  BLK in order. VCC and GND come in from the right.
- Encoder: five lanes between the SuperMini rows, onto its pins from the east: CLK, DT and SW from
  the IO row, 3V3 and GND from the power row.
- B1, B2 and B3 go west round the outside of the knob and along the bottom edge (B2 outermost,
  B3 innermost). B1 and B2 rise into their switches between the legs; B3 runs on under them to
  the right switch.
- GND: SuperMini to the encoder, then a second wire from the encoder to the left switch, one
  from switch to switch (each switch joins its lower leg pair inside) and one from the right
  switch up the right edge to the display.
- 3V3: SuperMini to the encoder, then a second wire from the encoder to the display VCC.

Without solder there is nowhere to join two wires in the middle of a run, so a net that splits does it at a pin:
the encoder's GND and + pins each take two wires (two windings, one above the other). Where two
runs of one net leave a pin together they share a channel (0.8 mm wide: room for two wires).
The enamel insulates, so runs may cross. There are five crossings: 3V3 under GND twice (at the
top of the lanes and below them), and B3 under B1, B2 and GND near the switches. At each, the
lower channel dips to 1.3 mm deep for 4 mm so the two wires don't press on each other. Every bend is rounded (1.5 mm radius at the wire) so the wire does not kink.

The channels are 0.8 × 0.7 mm. The wire lies loose in them, so hold long stretches down with
short pieces of Kapton or masking tape across the channel.

**Joints, no solder:**

- **Header pins (SuperMini, display):** scrape about 12 mm of enamel off the wire end, then wind
  4 tight turns round the pin (1.2 mm of pin) with tweezers, starting at the bottom of the
  groove. The square pin's corners bite into the copper. Every pin row (both SuperMini rows and
  the display) sits in the same 45° V-groove, 2.4 mm wide and 1.2 mm deep, so every pin is wound
  the same way: the turns sit 0.45 mm down in it. The SuperMini pins stand 4 mm past the back,
  the display pins only 1.2 mm, which still leaves 0.55 mm of pin above the top turn. Keep
  each winding neat and round its own pin: neighbouring pins are only 2.54 mm apart, and the
  scraped turns are bare. A dot of nail varnish over each finished winding keeps it from
  unwinding (optional).
- **Encoder pins:** the same 4 turns, in the 5.5 mm gap between the HW-040's header and the printed
  PCB. The turns can't slip off: the header is below them and the printed PCB above.
- **Switch legs (wrap, then fold):** each 12×12 switch has two pairs of legs joined inside. The
  lower pair (as seen from the face) is GND: both legs are used, GND comes in on one and leaves on
  the other towards the next switch. One upper leg takes the button signal (B1, B2 or B3); clip the
  other upper leg short. The legs are 3.5 mm long. Each leg comes out into a pocket (1.4 × 3 mm,
  0.7 mm deep) lined up with its channel, with 2.2 mm of leg standing past the pocket floor. Wind 2–3 tight turns of the
  scraped wire end round the leg, close to the floor (the leg's edges bite into the copper),
  then fold the leg flat along the pocket over the turns with pliers and press it down. The fold
  squeezes the turns against the leg, so it holds even if the leg springs back a little.

**Enamel:** no heat. Fold 400-grit sandpaper over the wire and pull it through 4–5 times,
turning it, or scrape it with a blade held at 90°, until it is bright salmon-pink all round. Then
check from the pin to the far end of the wire: under 1 Ω.

Order: wind one end on its pin, lay the wire along its channel (tape it), cut it with 12 mm to
spare, scrape that end, then make the other joint. Do the encoder runs last (assembly step 5).

## Wiring

The same pin map as the copper-trace version in [`../enclosure`](../enclosure/). It follows the real
SuperMini layout: seen from its component side with USB-C at the top, the power row (5V, GND,
3V3, IO4 to IO0) is on the right and the IO row (IO5 to IO10, IO20, IO21) on the left.

| SuperMini | Net | Goes to |
|---|---|---|
| IO4 | SCL | display SCL |
| IO3 | SDA | display SDA |
| IO2 | RES | display RES |
| IO1 | DC | display DC |
| IO0 | BLK | display BLK, backlight |
| 3V3 | 3V3 | display VCC, encoder + (**never 5 V**) |
| GND | GND | display GND, encoder GND, one lower leg of each switch |
| IO7 | CLK | encoder CLK |
| IO6 | DT | encoder DT |
| IO5 | SW | encoder SW |
| IO9 / IO8 / IO10 | B1 / B2 / B3 | left / middle / right switch, upper-right leg |
| IO20, IO21, 5V | none | IO20 / IO21 stay free for the serial port |

Firmware: display `SCL = 4, SDA = 3, RST = 2, DC = 1, BL = 0`; `ROTARY = { clk: 7, dt: 6, sw: 5 }`;
`CONTEXT_BUTTONS = [9, 8, 10]` (this replaces the pin map first planned in issue #67). Strapping
pins: GPIO2 must not be low at reset; it drives the display's RES input, which only listens, so
it floats as on a bare board. GPIO9 is the BOOT pin: holding B1 while pressing reset starts the
download mode, which is harmless. Validation row 5.1 still applies: check that the SuperMini has
no 32.768 kHz crystal on GPIO0/GPIO1 (now the display's DC and BLK).

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
   - Header row 2.2 mm in from its end, header on the right as mounted (`DISP_HEADER_SIDE`), its
     centre 0.1 mm towards the far edge from the board centre (`DISP_HDR_V`, measured).
   - Header pins 6 mm past the plastic (`DISP_PIN_L`, NOMINAL: the usual header). That puts the
     tips 1.2 mm past the printed PCB's back, 1.75 mm above where the turns sit in the V-groove:
     enough for 4 turns. If yours are shorter, measure them before printing.
2. **HW-040** (`HW040_*`, in the module's own frame, origin at the board centre):
   - Board 26 × 18.5 × 1.3 mm.
   - Shaft at (−2.3, +1.8) (measured).
   - M3 holes, Ø3.2, at (−8.55, −6.9) and (+8.55, −6.9) (measured).
   - Header row at x = +10.25 (`HW040_HDR_X`, measured: 0.1 mm nearer the shaft than drawn).
   - Above the board front (measured from a side photo, scale checked on the 6 mm shaft): body
     top 7.7, bushing thread ends 16.2, shaft round to 22.1, then a 7.1 mm D-flat to the tip at
     29.2. The knob height follows the shaft tip (`EC11_SHAFT_H`), so the shaft is never cut.
3. **SuperMini**: BOOT/RST positions (`SM_BTN_*`), header pin order (`SM_COL_A/B`), USB-C size,
   and how far the receptacle sticks out past the board end (`SM_USB_OVERHANG`, 0.5 mm): that sets
   how close the receptacle comes to the port hole.
   Pin headers: plastic 2.5 mm, long pins 6 mm past it (measured).
4. **12 mm switch**: snap-fit stem, the 7.3 mm tall version (seller drawing).
   - Housing 11.7 × 11.7 × 3.6 mm with a round Ø6.4 platform up to 4.5 mm.
   - Stem: on the platform a 3.0 mm square neck only 1 mm tall, then the 3.8 × 3.8 mm square head
     (the grip), 1.7 mm tall, top at 7.2 mm. The cap's lips snap in under the head, into that
     1 mm neck. The switch top sits 3.0 mm under the face, so a square sleeve inside the cap
     reaches down to it and only the finger ends round the head flex.
   - The cap's lips reach 0.3 mm under the head (`CAP_SNAP`). Measure your head: if the cap
     falls off, raise `CAP_SNAP` a little (or lower `CAP_SNAP_CLR`); if it will not click on or a
     finger cracks, lower it.
   - Pins on a 12.5 × 5 mm grid (land pattern Ø1.5). Legs 3.5 mm long (measured, `SW_LEG`).
     Some 12 × 12 switches (Omron B3F) have locating bosses; TS-G005 and TS13 show none, so
     there are no holes for them.

A cheap check before the full print: print only the `Carrier` first (about 1 h), then press the
parts on. If a peg misses its hole, measure the hole position, change the parameter and rebuild.
