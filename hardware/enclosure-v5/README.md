# Kivori enclosure v5, "Visor Dashboard"

The inside of [`../enclosure-v4`](../enclosure-v4/) in the outside of [`../enclosure-v2`](../enclosure-v2/)
(the "big keycap"): a light matte wedge with leaning walls and a bevelled face, a black visor
carrying the screen and the keys, and a big Ø48 knob. Same parts, part positions, pin map and
solder-free wiring (0.3 mm enamelled wire, see [Wire wiring](#wire-wiring)) as v2 and v4; the
printed PCB, wire channels, nut-mounted encoder, key mechanism, five screw posts, back plate,
hooks and plinth construction are v4's. Only the look comes from v2: v4's full top plate, bezel and
light ring are gone. The TPU feet (two rails and a disc in grooves) and the underside text are new.

A 110 × 70 mm face on a wedge case (30.8 mm thick at the far edge, 18.8 mm at the near edge, so
the face tilts about 10° towards you; on the desk it is 34.5 mm to the top of the shell and keys, 45.4 mm
to the top of the knob, feet included). The walls lean in 6° like a keycap, so the footprint is about
117 × 76 mm (v4: 113 × 73). The thickness is set by the parts inside (the encoder hangs deep for
its long shaft): what limits it is listed under [How thin the case can get](#how-thin-the-case-can-get).

- **Shell:** white, matte, the walls lean in 6° towards the face, R8 corners and a 2 mm 45° bevel
  frames the face. The sides are clean: no latch windows or screws.
- **Visor:** a black panel, 1 mm proud of the face, carries the display window (bevelled down to
  the glass) and the three key openings, plus the RST needle hole. Its edge follows the knob at an
  even 2.3 mm gap. It is separate from the shell (a recess in the face would need a 60 mm bridge
  in the face-down print) and goes on with three small pegs (Ø1.8) that drop into holes in the
  shell face, so it cannot go on turned round. Printed on a smooth PEI sheet it comes out glossy.
- **Knob:** Ø48 × 11.3 mm. The encoder hangs deep in the case, so a knob this low still covers
  its whole shaft and nothing needs cutting. Fine ribs round the skirt give grip, a 1.2 mm 45°
  chamfer round the top and a radial line marker. Inside it is hollow: a hub round the shaft
  (round bore, then D-shaped over the shaft's flat), tied to the skirt by four ribs. Its skirt
  clears the face by 1.5 mm (so 0.5 mm above the visor, and the visor's knob cut-out leaves a
  further 2.3 mm all round).
- **Keys:** three low keys, 5 mm above the face (4 mm above the visor, as in v4), tapering slightly
  from 13.4 to 12.4 mm above the visor, all three grey. Each clicks on with two fingers and is held
  square by two ribbed walls, so a press on a corner still pushes the switch straight down. The
  visor's key openings (14 mm) leave 0.3 mm round the 13.4 mm cap.
- **Plinth:** set back under the shell, the back plate's own outline chamfered 45° all round (no
  overhang). Grey, so a thin grey line sits under the white shell and it seems to float above the desk.
- **Feet:** three black TPU feet stand 1.5 mm proud of the plinth's flat bottom, so the plinth never
  touches the desk and the case sits level on them: a 5.6 mm wide rail along the near edge and one
  along the far edge (102.1 mm long, rounded ends, 0.5 mm chamfer on the sole), each covering the two
  corner screw counterbores on its side so the screws are hidden, and a Ø6 disc over the middle screw.
  Each drops into a 0.9 mm deep groove in the plinth's bottom face (0.2 mm clearance per side), so it
  locates itself and can be glued (CA or contact glue) or pressed in. A Ø4.2 plug under each screw
  position presses into the counterbore. The flange of the foot covers the groove's edge. At the rail
  ends the cap covers the counterbore to within 0.15 mm (the plinth's flat is tight at the corners:
  the groove keeps 0.7 mm from its edge).
- **Underside:** the wordmark KIVORI (cap height 7.5 mm) and under it the model code KV-P1 (5 mm),
  in Arial Black, recessed 0.4 mm (two layers) into the bed face, not raised. Strokes are at least 1.1 mm
  (the hyphen), counters at least 0.9 mm and letter gaps at least 1.1 mm, so they print cleanly. Print the colour change
  below and the letters come out black through the grey plate.
- **Back plate window:** the HW-040 hangs lower than anything else, so the plate has a window right
  through its 2 mm under the module (the board outline plus 1 mm each side, 28 × 20.5 mm); the
  plinth below it stays closed. That is what lets the plate sit 1.9 mm higher than in the first v5.
- **Colours:** shell white; visor black; knob black; all three keys grey; plinth and back plate grey;
  feet black TPU; underside letters black (colour change).

Inside is a **printed PCB** (`Carrier`): a plate with the outline, mounting holes and part positions
a real PCB would have. Components sit on its front; the wire runs in channels on its back. Once the prototype works, `export/pcb-outline.dxf` becomes the Edge.Cuts layer in KiCad.

Five M2 × 12 self-tapping screws hold it together, so it stays shut when dropped. Each one goes
up from under the plinth through a back-plate column and the printed PCB into a post on the
faceplate, and clamps all three together. Four posts sit symmetrically near the corners
(left and right mirror each other), and a fifth near the middle (x 57, y 29: the nearest spot to
the centre clear of the wire channels and the display) holds the back plate there too; they are slim (Ø4.2) so the front-right one clears the right
cap's flange. A spigot on each post runs right through the printed PCB and 1.5 mm on into a
socket in the back-plate column, so the PCB is held over its whole thickness. The four
hooks on the back plate (45° ramps both ways) stay: they line the plate up and hold it while you
screw.

How each part is held on the printed PCB, even with the case open:

| Part | Held by |
|---|---|
| HW-040 | It hangs behind the printed PCB from the EC11 itself, mounted like a panel encoder: the square EC11 body sits flat against the printed PCB's back, the M7 bushing goes through a round Ø7.6 hole that centres it, and the EC11's own nut (with its washer) on the printed PCB's front clamps it. A knob press goes into the printed PCB and on into the face posts; the back plate doesn't touch the encoder. Its five header pins stand straight up (no bend) into holes in the printed PCB, and the wire is wound round them in the 4.0 mm gap between the HW-040's header and the printed PCB. Sitting this deep (board front 18.7 mm behind the face) lets the long shaft end low. |
| Display | Four standoffs whose pegs go through its four corner holes. It hangs on them only: the glass has 0.5 mm of air in front of it, so the screws never clamp it against the face. Over the LCD's flex cable (where it leaves the glass at the end away from the header and bends onto the board) the face back has a 1.2 mm deep relief, leaving 2.5 mm of air, so the face cannot pinch the flex (that broke a display). |
| SuperMini | Stands on the front on its soldered header strips; the long pins go through to the back (4 mm past it), where the wire is wound round them. The board end sits up against a hidden pocket in the far wall, which leaves 0.8 mm of wall round the receptacle, so the port outside is just a small pill-shaped hole (9.6 × 4 mm) and a cable still plugs in fully. |
| 12×12 switches | Legs through the plate; a leg that takes a wire is folded flat over it on the back. Each cap clicks onto the switch's 3.8 mm square head like a bought round 12×12 cap. Its grip copies the bought cap's drawing: two opposite sides are long plates (5.5 mm wide, past the head's corners, 1.6 mm thick) whose lips snap 0.3 mm in under the head; each is tied to the cap wall by a 1 mm rib (`CAP_RIB_T`) that runs down 37% of the plate's height (`CAP_RIB_SHARE`; the free rest is what bends, see Small parts). The other two sides are 2 mm wide snap plates too, their lips 0.2 mm in under the head, with a 1 mm rib down 50% of theirs. Gripped on all four sides, so the cap cannot rock on the head and a press on a corner goes into the switch. A Ø3 stop post on the cap top rests on the head, so the cap is held both ways without play. (Four free fingers alone were springy and the cap wobbled; a stiff socket all round had no snap.) |

The display pegs are 0.2 mm smaller than the holes (press fit) and stick out 1 mm past the module board.
To lock a part for good, touch each peg tip with the soldering iron so it melts into a small head
(heat-staking). To remove the part later, cut the head off. The faceplate has relief pockets over
the display pegs, so a staked head still fits.

## Files

| File | What |
|---|---|
| `build_case.py` | The whole model. Every dimension is a parameter at the top; the wire runs are in `wire_layout()`. |
| `kivori-case-v5.FCStd` | Generated FreeCAD document (printed parts plus stand-ins for the bought parts; `Dummy_Wire` is the wire). |
| `export/*.stl` | Printed parts, already in print orientation (binary STL). |
| `export/pcb-outline.dxf` | PCB outline, mounting holes and pin holes, in mm (for KiCad Edge.Cuts). |

To rebuild after changing a parameter, run it headless from the repo root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-v5/build_case.py
```

or in FreeCAD's Python console (or through the FreeCAD MCP):

```python
p = "<repo>/hardware/enclosure-v5/build_case.py"
exec(open(p).read(), {"__file__": p, "__name__": "__main__"})
```

It prints an interference report (`interference: none` is the goal) and a wire-channel check
(`plastic under 0.7 mm: none` is the goal), an underside check (foot grooves, text, letter strokes, the feet standing level
and below the plinth; `underside problems: none` is the goal), an overhang check of every printed part in its
print pose (`overhang problems: none` is the goal; the line above it lists each part's bridges), the
encoder-to-back-plate clearance (1.24 mm; keep it over 0.5), regenerates the document and rewrites `export/`.

## Parts to print

FDM, 0.4 mm nozzle, 0.2 mm layers, 3 walls, 20 % infill. None of the parts needs supports.

Checked for FDM in each part's print orientation:

- **Overhangs:** `build_case.py` checks every part in its print pose (faces on the bed excepted): no
  downward face steeper than 45° from vertical, flat roofs only as bridges up to 10 mm across or as
  ledges up to 0.5 mm (a line width). What is left, all short:
  - `Shell` (face down): the 6° wall lean and the 2 mm bevel are 6° and 45° (the bevel is exactly
    45° with the lean added); bridges are the roofs of the three visor-peg holes (Ø2.3) and the USB
    hole's roof, about 8 mm: the 5.6 mm flat of the pill plus the first 45° of its rounded ends, in a
    0.8 mm wall.
  - `Visor` (top down): none. Its window chamfer and 0.5 mm edge chamfer are 45°; the pegs point up.
  - `BackPlate` (desk side down): the plinth slopes 45°; the hook barbs come out at 44° (the wedge tilt
    shortens their ramps); the foot grooves are recesses in the bed face, so each bridges its 5.6 mm
    width at 0.9 mm (the disc's ring 1.6 mm); the 0.4 mm letters bridge up to 2.7 mm (the bridge layer
    is what shows black in the colour-change print; the stepped first layer leaves 0.1 mm ledges); the
    five screw-head seats (Ø4.4, tilted 10° with the screw) are stepped bridges, see Screw seats
    below: two D-shaped pads, each bridging 3.7 mm, and two 2.0 mm bridges.
    The window under the encoder is open to the top: no overhang.
  - `TpuFeet` (sole down): sole chamfer, flange, body, plugs: none beyond a 0.3 mm ledge where a
    plug stands 0.3 mm past the rail's end.
  - `Carrier` (back down): bridges over the 0.8 mm wire channels, 1.4 mm leg pockets and the 2 mm
    Kapton seats.
  - `Knob` (top down): the line marker bridges 1.2 mm; the grip grooves run out of the skirt edge.
  - Caps (top down): the 45° top chamfer is lofted in as its own section (`CAP_TOP_EDGE`; the
    makeChamfer version came out 47-54° on the sides and corners), the flange slopes 45°; the only
    steeper details are the 0.3 mm snap-lip ledges (0.2 mm on the short walls), under one line width.
- **Thin features:** nothing thinner than 0.7 mm (about two nozzle widths). On the printed PCB's
  back (on the bed) the wire channels are 0.8 mm wide and the leg pockets 1.4 mm, so the layer
  over each is a short bridge. The back is flat along the pin rows.
  Every channel keeps at least 0.7 mm of plastic to anything that is not its own (the build
  checks it). On the plinth bottom the foot grooves keep 0.7 mm to the flat face's edge and the text
  keeps 0.7 mm to the grooves (checked too); the letters have no stroke or counter under 0.8 mm.
- **Fits:** FDM prints holes 0.1-0.2 mm small, so the fits allow for it: spigot 3.4 in 3.9, knob bore 6.25 on the 6 mm shaft, cap 13.4 in a 14 mm opening, cap fingers 0.05 mm onto the 3.8 mm switch head.
- **Screw seats:** each Ø4.4 counterbore ends in a seat square to the screw, so the pan head sits
  flat; in the print pose that seat is a 10° roof. Printed as a plain ring it was a 1 mm wide
  overhang round the hole, so it is stepped (`SEAT_LAYER`, one 0.2 mm layer per step, measured along
  the screw): layer 1 has a 2.4 mm slot (the clearance hole's width) across the whole counterbore along
  x (the tilt axis), which leaves two D-shaped pads at the seat plane, each bridging 3.7 mm between
  the counterbore's walls (the head rests on them, about 3.6 mm2 of contact, 45% of the old ring);
  layer 2 is solid except the 2.4 mm square of the hole, so what is left of it bridges the slot
  (2 mm) along y between the pads; above that the Ø2.4 hole is round, and the four corners it leaves
  are 0.24 mm ledges. The opening for the screw is the 2.4 mm square, never smaller than the hole.
  The tilt makes each layer a 10° slab (0.4 mm of rise over the 2.4 mm span), so the printer steps it
  over two layers: an approximation, not a flat bridge. The seat and the plastic round it keep their
  thickness (nothing under 0.7 mm; the two layers sit inside the counterbore's footprint). The TPU
  plugs enter from the other end and are not touched.
- **Small parts:** the caps' four snap plates are tied to the cap wall by a 1 mm rib down part of
  their height (`CAP_RIB_T`, `CAP_RIB_SHARE`), which leaves the rest of each plate free to bend: the long
  fingers get a 37% share (5.9 mm free of 9.3), the short side plates 50% (4.6 mm free of 9.25).
  Cantilever estimate, strain = 3 t d / (2 L^2) with d = lip reach plus the 0.05 mm preload of
  `CAP_SNAP_CLR`: fingers (1.6 mm, d 0.35) **2.5%**, short plates (1.2 mm, d 0.2) **1.7%**, at
  PETG's snap-fit limit of about 2-2.5%, so the caps click on. (A 75% share on all four plates gave
  15.5% and 6.7%, which would not click on: the plates or ribs break first.)

| Part | Material | Orientation (already applied in the STL) |
|---|---|---|
| `Shell` | light matte PLA | face down |
| `Visor` | black PLA; a smooth PEI sheet gives it a glossy face | top down (pegs up) |
| `BackPlate` (with the plinth) | grey **PETG** (the hooks crack in PLA), black for layers 3-4 (colour change below) | desk side down |
| `Carrier` (printed PCB) | PETG preferred (the folded switch legs and pegs press on it for years); PLA works | back down, pegs and standoffs up |
| `Knob` | black PLA | top down |
| `CapLeft` / `CapMiddle` / `CapRight` | grey PLA, PETG if a finger cracks | top down |
| `TpuFeet` (both rails and the disc in one file) | **TPU** (95A), black | sole down, plugs up |

Every part is printed in a single colour except the back plate, which has one colour change for the
underside text; the rest of the look comes from printing the parts in different filaments.

**Back plate colour change (black underside letters).** The letters are 0.4 mm deep holes, exactly
two 0.2 mm layers (use 0.2 mm layers with a 0.2 mm first layer):

| Print Z | What |
|---|---|
| 0 to 0.4 mm (layers 1-2) | grey; the letters are holes |
| **0.4 mm** | change filament to **black**: layers 3-4 (0.4 to 0.8 mm) are black; layer 3 bridges the holes, so the letters show black through the grey |
| **0.8 mm** | change back to **grey** for the rest of the plate |

Slicers that name a layer by its top height (PrusaSlicer, OrcaSlicer) take the change at the layers
labelled 0.6 mm (first black layer) and 1.0 mm (first grey layer again). Without a colour change the
letters are clean 0.4 mm recesses in a single colour. Black layers 3-4 also show as a 0.4 mm black
pinstripe on the plinth's lowest 45° slope; to avoid it, change back after one layer (Z 0.6 mm, first
grey layer 0.8 mm): the single black bridge layer still shows the letters, a little thinner. The first-layer relief (`TEXT_EF`, 0.1 mm per side, the
first layer only) offsets the elephant foot that squeezes the letters narrower at the bed.

## How thin the case can get

The case is as thin as the stack at the encoder allows. Lowest points of the inside parts (face
frame, z = 0 at the face, down is negative; the carrier front is at -10.2, its back at -12.2):

| Part | Lowest point | Where | To the back plate's inner face (now / was) |
|---|---|---|---|
| HW-040 (board back -20.0, EC11 legs and header joints 2 mm lower) | -22.0 | y 29 to 41 | 1.24 / 0.47 mm (the tightest; the 6.5 mm EC11 body let the board sit 1.2 mm higher, it was 0.57) |
| Display header pins (11 mm long-pin header) | -18.9 | y 37 to 55 | 4.4 / 6.3 mm |
| SuperMini header pins | -16.2 | y 49 to 67 | 8.8 / 10.7 mm |
| Switch legs | -13.7 | y 9 to 24 | 4.1 to 5.7 mm (the right switch's legs sit 1.3 mm beside a column) |
| Carrier back, wire turns | -12.2, -13.9 | everywhere | 0.1 mm to the columns (they clamp it) |

- **The back plate:** without a window its inner face is 0.47 mm from the HW-040's near corner,
  so it could not come up at all. With the 2 mm window under the module the plate comes up 1.9 mm
  (`T_NEAR`, `T_FAR` 20.7 / 32.7 to 18.8 / 30.8) and keeps 1.24 mm clearance (0.57 before the encoder body was re-measured at 6.5 mm). The wedge stays at 9.7°:
  the limit is the module in the middle of the plate, not one edge, so flattening the wedge would
  only make the near edge thicker. Everything that hangs on the plate comes up with it (columns,
  hooks, plinth, feet): the columns are 7.2 mm long at the near screws and 16.7 mm at the far ones.
- **The carrier cannot come closer to the face**, so the encoder (its shaft tip is 7.3 mm above the
  face, the knob 11.3 mm) stays where it is. The USB port stops it: the SuperMini's receptacle
  top is 1.0 mm under the faceplate's back, and the port hole's top edge is 0.9 mm under the start
  of the face bevel at the far edge. Every 0.1 mm the carrier rises takes 0.1 mm of that lintel,
  and 0.7 mm is the least that prints, so the carrier could come up about 0.2 mm, which is not worth
  a taller knob. The next limits behind it, if the USB were solved: the display header 2.3 mm over
  the carrier front, the switch housings 2.6 mm under the key flanges, and the key snap plates (each
  mm the carrier rises makes them 1 mm shorter, about 1.2 times the strain per mm). Solving the USB
  limit takes a hardware change: slide the 2.5 mm black plastic off the SuperMini's header pins so
  it sits on the carrier. That frees 2.5 mm at the port; the key snap plates then set how far the
  carrier can go (about 1 mm, and the knob grows by the same).
- **Screws:** unchanged, M2 × 12 (the seat is stepped now, see Screw seats). The heads still seat at -16.5 and the tips end at -4.5, 0.5 mm
  below the pilot's top; the thread runs through the spigot and post, 9.2 mm of plastic. The seat
  is now inside the column at the near screws (1.5 mm above the plate's inner face), with 1.8 mm of
  column wall round the Ø4.4 counterbore.

## Bought parts

| Part | Notes |
|---|---|
| ESP32-C3 SuperMini | On the front of the printed PCB, components facing the face, on its own soldered header strips (long pins down through the printed PCB). |
| ST7789 1.3" 240×240, 7-pin | On four standoffs with pegs, glass in a 0.6 mm pocket behind the window with 0.5 mm of air in front (never clamped), its header pins through the printed PCB. Fit a **long-pin header** (2.54 mm, 7 pins, about 17 mm long overall, long side down): the usual header is too short to wind the wire on. Buy the display without a header, or swap it once. |
| HW-040 rotary encoder | Hangs behind the printed PCB on its own nut and washer (see above). Straighten its five right-angle pins with pliers, or fit a straight 5-pin header. If your EC11 has a small anti-rotation tab next to the bushing, cut it off or file a notch for it in the printed PCB, so the body sits flat. |
| 3 × 12×12 mm push switch, 7.3 mm tall | Legs go through the printed PCB. |
| Enamelled copper wire, 0.3 mm, about 1 m | The wiring. |
| 400-grit sandpaper or a craft knife | Scraping the enamel off the wire ends (no heat). |
| Kapton (polyimide) tape, a few cm | Five 2 × 2 mm squares at the wire crossings; short strips also hold the wire down in long channels. |
| 5 × M2 × 12 self-tapping screws, pan head up to Ø4 | From under the plinth into the face posts. |

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
5. Push the HW-040's bushing through its round hole from the back, its straightened pins into
   their five holes, and screw the washer and nut onto the bushing on the front until the EC11 body
   sits flat and firm. Wrap the encoder runs onto its pins, in the gap under the printed PCB.
6. Check every run with a meter.
7. Drop the three caps into the face openings from inside the shell, flange first.
8. Lower the printed PCB over the five spigots and press it down onto the posts: each switch head
   clicks into its cap. The spigots stick out past the back and locate the back plate's columns.
9. Press the back plate straight in until all four hooks click. Then drive the five screws in from under the plinth until snug (do not overtighten in
   PLA). Press the TPU feet into their grooves (a drop of glue in each groove if you want them fixed), the rail
   plugs into the corner screw holes and the disc plug into the middle one; the rails hide the four corner screws.
10. Press the visor's three pegs into their holes in the face (key openings over the caps, the
   window over the screen). A drop of CA glue or a few small pieces of thin double-sided tape on
   the face keep it down.
11. Push the knob onto the D-shaft.

To open: pull the three TPU feet off, take the screws out, grip the plinth in the shadow
gap and pull. RST is a Ø1 needle hole in the visor, above the SuperMini (a needle or SIM pin
reaches the button through a guide tube).

The underside text is deliberately big and heavy (Arial Black, 7.5 mm and 5 mm cap height) and
recessed, not raised: it prints on the bed, where small raised letters come out unclear. The printed
PCB's back carries the pin and button names next to their holes.

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
lower channel dips to 1.3 mm deep for 4 mm, and a 2 × 2 mm square of Kapton tape goes over the
lower wire in a small flat pocket, so the upper wire crosses on the tape. The enamel alone
insulates, but a wire nicked while it is laid could short at a crossing; the Kapton rules that
out. In the FreeCAD model the Kapton squares are `Dummy_Kapton` (yellow), and the lower wire is
drawn down in its dip. Where two runs of one net share a channel (GND and 3V3 next to the
encoder), the two wires lie side by side; they need no tape. Every bend is rounded (1.5 mm radius at the wire) so the wire does not kink.

The channels are 0.8 × 0.7 mm. The wire lies loose in them, so hold long stretches down with
short pieces of Kapton or masking tape across the channel.

**Joints, no solder:**

- **Header pins (SuperMini, display):** the same open-air joint as at the encoder. Scrape about
  12 mm of enamel off the wire end, lift it out of its channel at the pin and wind 4 tight turns
  round the pin (1.2 mm of pin) with tweezers, right behind the printed PCB. The square pin's
  corners bite into the copper. The SuperMini pins stand 4 mm past the back; the display sits on
  a long-pin header, so its pins stand 6.2 mm past the back (the usual header reaches only
  1.2 mm). Keep
  each winding neat and round its own pin: neighbouring pins are only 2.54 mm apart, and the
  scraped turns are bare. A dot of nail varnish over each finished winding keeps it from
  unwinding (optional).
- **Encoder pins:** the same 4 turns, in the 4.0 mm gap between the HW-040's header and the printed
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
spare, scrape that end, then make the other joint. At a crossing, lay the lower run first (down
in the dip), stick the Kapton square over it in its pocket, then lay the upper run across. The
lower runs are 3V3 (both of its crossings with GND) and B3 (under B1, B2 and GND), so lay those
first. Do the encoder runs last (assembly step 5).

## Wiring

The same pin map as the copper-trace version in [`../enclosure-v1`](../enclosure-v1/). It follows the real
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
   - Long-pin header: 11 mm past the plastic (`DISP_PIN_L`, NOMINAL: a 2.54 mm header 17 mm
     long overall). That puts the tips 6.2 mm past the printed PCB's back. Measure yours: at
     least 1.5 mm past the back is needed for 4 turns.
2. **HW-040** (`HW040_*`, in the module's own frame, origin at the board centre):
   - Board 26 × 18.5 × 1.3 mm.
   - Shaft at (−2.3, +1.8) (measured).
   - M3 holes, Ø3.2, at (−8.55, −6.9) and (+8.55, −6.9) (measured).
   - Header row at x = +10.25 (`HW040_HDR_X`, measured: 0.1 mm nearer the shaft than drawn).
   - Above the board front (the encoder side), measured by the user on the real module,
     2026-10-31: body ("box") top 6.5 (`EC11_BODY_H`), shaft tip 26.0 (`EC11_SHAFT_H`). The bushing
     length (8.5, `EC11_BUSH_L`) was not re-measured: thread ends 15.0, shaft round to 18.9, then
     a 7.1 mm D-flat to the tip. The body sits flat on the printed PCB's back (-12.2), so the board
     front is at -18.7 (`HW040_FRONT`), the bushing tip at -3.7 (nut clamp unchanged) and the shaft
     tip 7.3 mm above the face. The knob height follows the shaft tip, so the shaft is never cut.
3. **SuperMini**: BOOT/RST positions (`SM_BTN_*`), header pin order (`SM_COL_A/B`), USB-C size,
   and how far the receptacle sticks out past the board end (`SM_USB_OVERHANG`, 0.5 mm): that sets
   how close the receptacle comes to the port hole.
   Pin headers: plastic 2.5 mm, long pins 6 mm past it (measured).
4. **12 mm switch**: snap-fit stem, the 7.3 mm tall version (seller drawing).
   - Housing 11.7 × 11.7 × 3.6 mm with a round Ø6.4 platform up to 4.5 mm.
   - Stem: on the platform a 3.0 mm square neck only 1 mm tall, then the 3.8 × 3.8 mm square head
     (the grip), 1.7 mm tall, top at 7.2 mm. The cap's two finger lips snap in under the head,
     into that 1 mm neck; its two short walls cover the head's other sides. The switch top sits
     3.0 mm under the face.
   - The lips reach 0.3 mm under the head (`CAP_SNAP`) and catch 0.4 mm lower than drawn
     (`CAP_SNAP_DROP`, measured: at 0.2 the cap was loose). If the cap falls off, raise
     `CAP_SNAP_DROP` or `CAP_SNAP` a little; if it will not click on, lower `CAP_SNAP`. If it
     rocks, close the short walls up to the head (`CAP_SIDE`, play now 0). All four plates have a
     rib down `CAP_RIB_SHARE` (0.37 fingers, 0.50 short plates) of their height, which keeps the strain at
     2.5% / 1.7% (see Small parts); lower the share if the cap will not click on.
   - Pins on a 12.5 × 5 mm grid (land pattern Ø1.5). Legs 3.5 mm long (measured, `SW_LEG`).
     Some 12 × 12 switches (Omron B3F) have locating bosses; TS-G005 and TS13 show none, so
     there are no holes for them.

A cheap check before the full print: print only the `Carrier` first (about 1 h), then press the
parts on. If a peg misses its hole, measure the hole position, change the parameter and rebuild.
