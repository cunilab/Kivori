# Kivori experimental enclosure: square, turning ring round a static screen, 4 buttons

A 100 × 100 mm square body with a Ø68 knurled ring standing 13.6 mm above the top. Like v1 it
is a wedge (9.7°): about 25 mm tall at the front edge and 41.4 mm at the back.
The screen sits still in the middle under a square window in a round black bezel. The ring round
it is the knob, and pressing the bezel is the click. The whole top surface is the four buttons:
four quadrant tiles round the ring, split in a "+", the same colour as the body.

It uses the same parts as [v1](../enclosure): the ST7789 1.3" square LCD, the HW-040 encoder
(with its own push switch), 12×12 push switches and the ESP32-C3 SuperMini with its pins soldered
on. Like v1 it has no wires: everything connects through copper tape on one printed PCB, with the
same GPIO map (issue #67) plus a fourth button.

Experimental: checked only in CAD. The model has no interference, the gears mesh all the way
round, and every copper net is a single piece with at least 0.94 mm between nets. Nothing has been
printed yet.

## The engineering problem, and how it is solved

The EC11 on the HW-040 has a solid shaft in its centre, and the screen needs that centre. So the
encoder cannot sit on the ring's axis.

1. **The ring drives the encoder through gears.** Teeth on the inside of the ring (56 teeth,
   module 1) mesh with a 20-tooth gear on the encoder shaft, which sits 17.65 mm in front of the
   centre. In an internal gear pair both gears turn the same way, so turning the ring clockwise
   turns the encoder clockwise. The pinion's teeth sit low on the shaft, just above the bushing,
   and its tall hub grips the D-flat at the top, so the body stays low and the ring stands tall.
   - 1 ring turn = 2.8 encoder turns. With a 20-detent EC11 that is 56 clicks per ring turn,
     6.4° each.
   - The detent is felt 2.8× stronger at the ring, so the clicks stay crisp on a big ring.
   - Tooth play is ±0.25° at the ring. For an internal pair the play comes from moving the
     pinion *towards* the axis (`BACKLASH_CD`).
2. **The encoder sits on two floor pillars under the printed PCB**, as on v1: their pegs go
   through the HW-040's two holes, so it cannot move, and they take the press. Its M7 bushing goes
   up through a snug round hole in the printed PCB, which centres it, so the gear centre distance
   comes from that one printed hole. The nut is not used. The encoder's measured shaft (29.2 mm)
   is used whole; that length is what sets the body height. Four small L-shaped corner borders
   on the floor (a ledge under each board corner and a low wall) keep the board square; the floor
   has no holes, the encoder sits just high enough for the parts on its back to clear it.
3. **Pressing the bezel uses the HW-040's own push switch.** The screen stays still on four posts.
   The bezel round it is hinged on the left (a pin each side of a lug snaps into C holes in two
   posts on the printed PCB), and a nub under it rests on the encoder shaft. Pressing the bezel
   pushes the shaft down 0.5 mm and clicks the EC11's switch. The hinge is as far from the shaft
   as from the bezel centre, so a press in the middle needs the same force as the switch itself.
   The bezel sits 1.1 mm above the glass, so it can tip without touching the screen. The pinion
   moves down with the shaft and stays in mesh.
4. **The ring rests on a fixed sub-deck** under the tiles. It turns in a Ø66 hole, and a
   retainer ring screwed to its underside (3 × M2) sits under the sub-deck, so it cannot lift
   out. The bezel's flange sits under the ring's top lip, so the bezel cannot lift either.
5. **Square window.** 24 × 24 mm with 1.5 mm corners and a chamfer, over the 23.4 mm square
   active area: the whole screen shows.
6. **The top is the buttons.** Four quadrant tiles cover the whole top, from the ring out to the
   body edge, with 0.3 mm gaps between them and round the ring.
   - **The switch holds the tile**, the way a standard 12×12 switch cap is held. Under the
     middle of each tile (44 mm out along the diagonal) a stem hangs down to the switch, ending in
     a square socket that snaps over the switch's square tip (the v1 TS13 stem: 3.8 mm head on a
     3.1 mm neck). Its four walls flex (slits at the corners, 5 mm up the stem). Two of them have
     hooks with a 45° lead-in below and a flat top that catches under the tip.
   - 3 legs sliding in guide bosses in the sub-deck keep a big tile from tilting, so pressing
     near a corner still moves it evenly onto its switch.
   - A 1.2 mm lip on each tile's outer edge hangs over the wall top. The walls and floor sit
     1.5 mm inside the tile outline, so the top seems to float over a shadow line.
   - The tiles print face down, so all four faces get the same smooth bed finish.

## Printed PCB with copper-tape traces

One printed board (`Carrier`) carries everything:

| Part | How it is held |
|---|---|
| HW-040 | Under the board, on two floor pillars with pegs through its holes and four corner borders; bushing up through a snug Ø7.2 hole. Its five header pins are straight (straighten the right-angle pins, or fit a straight header) and reach 0.5 mm into their holes, soldered to the pads round them. |
| 12×12 switches | On top, legs through, bent over onto their pads underneath. |
| SuperMini | On top, in the middle of the back, standing on its soldered header strips, pins through to the copper. USB-C out of the back wall through a 9.6 × 4 mm pill hole (a hidden pocket leaves 0.8 mm of wall round the receptacle, as v1). |
| Display | Stays as bought: its own short header plugs into **two standard 7-pin female headers (8.5 mm tall) stacked** on the board, the lower one soldered to the copper. Four posts with pegs through its corner holes hold it level. It pulls off for repair. |
| Bezel hinge | Two posts on the board, braced to the display posts. |

The copper is on the underside (the bed side when printing). Every trace and pad is flush with
the surface, with a 1.0 mm wide, 0.6 mm deep gap along both sides, so taping is: lay the tape flat,
cut along the gaps, peel off the rest. Traces are 1.2 mm wide and at least 0.94 mm apart. Where two
gaps would leave less than 1 mm of plastic, that strip becomes gap too; islands of plastic under
12 mm² become gap, and gaps thinner than 0.6 mm are filled in, so nothing small prints. The pin
rows (SuperMini, display, encoder) stay solid copper bands: one short blade cut between
neighbouring pins separates them, like a stripboard.

**One layer, no crossovers:**

- Display: its five signals (1.0 mm wide) leave its sockets to the west, step east above them,
  run up beside the SuperMini's right row and turn into their pins from the east.
- Encoder: DT, SW, 3V3 and GND run down between the SuperMini rows (stepping west twice to clear
  the display's lanes and the encoder bushing), CLK and B1 down beside its left row. Under the
  encoder pins they turn east and come up into them from the front.
- B2 and B3 go straight into the back switches. B4 comes down between the rows too, passes north
  of the encoder and drops into the front-right switch. B1 goes into the front-left switch.
- GND runs round the edge of the board, past all four switches (their front pin pair) and over
  to the display. VCC leaves the display to the east, runs down between the legs of the
  front-right switch, under it and back up into the encoder's + pin, which is 3V3.
- Each switch's second leg of a pair is joined to the first inside the switch.

### Wiring (same map as v1, issue #67)

| From | To |
|---|---|
| SuperMini 3V3 | HW-040 +, display VCC |
| SuperMini GND | HW-040 GND, display GND, one pin pair of each switch |
| GPIO6 / 7 / 8 / 10 / 20 | display SCL / SDA / RES / DC / BLK |
| GPIO2 / 0 / 1 | HW-040 CLK / DT / SW |
| GPIO3 | B1, front-left tile |
| GPIO4 | B2, back-left tile |
| GPIO5 | B3, back-right tile |
| GPIO9 | B4, front-right tile (new) |

## Parts

| Part | Material | Print orientation (in the STL) |
|---|---|---|
| `Shell` (walls, sub-deck, the posts down onto the printed PCB) | dark PLA | sub-deck down |
| `Ring` | light or silver PLA (or PETG) | top down; 0.12–0.16 mm layers for the teeth |
| `Retainer` | any PLA | flat |
| `Floor` (corner screw posts, encoder pillars and corner borders) | dark PLA | flat |
| `Carrier` (printed PCB, display and hinge posts) | **PETG** (PLA softens under the iron) | copper side down |
| `Pinion` (teeth and hub) | PETG or PLA | teeth down; 0.12–0.16 mm layers |
| `Cap` (bezel) | glossy black PLA (smooth PEI sheet) | face down |
| `TileBackLeft` / `TileBackRight` / `TileFrontLeft` / `TileFrontRight` | dark matte PLA | face down |

None of the parts needs supports. FDM rules as v1: nothing steeper than 45° except short
bridges (the copper gaps, the USB pocket, the hinge C holes open upward), nothing thinner than
0.7 mm, the hinge pins have a 45° pointed top so they print on the flipped bezel.

## Bought parts

| Part | Notes |
|---|---|
| ST7789 1.3" 240×240 (GMT130) | As bought, plugged into the sockets. Header on the right, as on v1, so `Rotation::Deg90` stays. |
| 2 × 7-pin 2.54 mm female header, 8.5 mm tall | Stacked: the display's sockets. |
| HW-040 rotary encoder | On the floor pillars under the printed PCB (its nut is not used). The pinion pushes onto the D-shaft. Its SW pin is the bezel press. |
| 4 × 12×12 push switch, 10 mm tall, snap-fit stem | One more than v1. |
| ESP32-C3 SuperMini with soldered header strips | On the printed PCB, USB-C to the back. |
| Copper tape, 0.06 mm, a roll | All traces. |
| 3 × M2 × 6 self-tapping | Retainer to ring. |
| 4 × M2 × 12 self-tapping | In the four corners: from under the floor, through the floor posts and the printed PCB into the shell's corner posts (deep counterbores at the back, where the wedge is thick). |
| 4 × 10 mm stick-on rubber feet | In the marker rings. |

## Assembly

1. Print the printed PCB in PETG. Lay copper tape over its underside, cut along the gaps, peel
   off the rest, and make one cut between neighbouring pins of each pin row.
2. Fit the switches, the SuperMini and the lower female header from the top; bend the switch
   legs over and solder every pin underneath. Plug the second header into the first and the
   display into that, onto its posts. Low-temperature solder (Sn42Bi58) and quick touches.
3. Push the HW-040 bushing up through its hole, rest its board on a 5 mm spacer and solder its
   five straight pins to their pads (the iron goes into the gap above its header). Push the pinion
   onto the shaft.
4. Snap the bezel's hinge pins into the two hinge posts; its nub now rests on the shaft.
5. Put the ring into the sub-deck hole from the top and screw the retainer to it from below.
6. Put the four tiles on top of the shell, legs down through the guide bosses.
7. Lower the shell over the printed PCB (its four spigots drop into the board's holes). Put the
   floor on: the pillars' pegs go into the HW-040's holes and its corners drop into the corner
   borders. Turn the
   ring a little until its teeth slide over the pinion. The ring's lip stops above the bezel's
   flange.
8. Press each tile firmly once: its socket clicks onto the switch tip. Drive the four screws up
   through the floor (they go in at the wedge angle), and stick the feet on.

## Firmware notes

- **Pins:** the same map as v1 (issue #67), so one firmware fits both cases.
- **Encoder:** one ring turn is 56 detents instead of 20, so a volume step per detent feels finer.
  Scale it in software if needed.
- **Press:** the HW-040 SW pin on GPIO1, as v1.
- **Fourth button:** GPIO9, the SuperMini's BOOT pin: it already has a pull-up and the BOOT
  button on it. The only side effect is that holding this button while plugging in USB starts
  the ROM download mode. `CONTEXT_BUTTONS` in `profile.rs` needs a fourth entry.

## Open points (measure or test first)

1. **Gear fit.** Print the `Pinion` and the `Ring` first. If the gears bind, raise `BACKLASH_CD`;
   if they skip, lower it.
2. **Ring friction.** PLA on PLA: a little PTFE dry lube on the sub-deck round the hole.
3. **Switch tip.** The socket fits the v1 stem (3.8 mm head, 3.1 mm neck). If the snap is too
   tight or too loose, change `SOCK_CLR` and `SOCK_HOOK`. Print the tiles in PETG if PLA walls
   crack when snapping.
4. **Tile feel.** The legs have 0.15 mm clearance in the guide bosses. If a tile sticks, open
   `LEG_HOLE_D`; if it rattles, close it.
5. **Press feel.** The bezel's far edge moves about 1.1 mm for the shaft's 0.5 mm. If the press
   feels too heavy or too long, move the hinge (`HINGE_X`).
6. **Socket stack.** The display's pins go fully into the upper socket. If your sockets are taller
   than 8.5 mm, lower the posts (`SOCKET_H`).
7. **SuperMini BOOT/RST** are not reachable with the case closed. Flashing over USB normally does
   not need them.

## Upgrade path

- A **magnetic angle sensor** (for example an AS5600 under a magnet on the ring) would remove the
  gears altogether, but that is a new part and new firmware.

## Files

`build_case.py` is the whole model; every dimension is a parameter at the top. Rebuild from the
repo root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-experiment/build_case.py
```

It prints the interference check, the smallest gap between copper nets, and how many small
islands and crumbs the gap clean-up removed.
