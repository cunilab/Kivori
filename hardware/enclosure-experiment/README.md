# Kivori experimental enclosure: square, turning ring round a static screen, 4 buttons

A 100 × 100 mm square body, 24 mm tall, with a Ø68 knurled ring standing 9 mm above the top.
The screen sits still in the middle under a square window; the ring round it is the knob;
pressing the screen is the click. The whole top surface is the four buttons: four quadrant tiles
round the ring, split in a "+", the same colour as the body. It uses the same parts
as [v1](../enclosure): the ST7789 1.3" square LCD, the HW-040 encoder (with its own push switch)
and 12×12 push switches.

Experimental: checked only in CAD. The model has no interference, the gears mesh all the way
round, and pressing the screen moves the encoder shaft without anything else touching. Nothing
has been printed yet.

## The engineering problem, and how it is solved

The EC11 on the HW-040 has a solid shaft in its centre, and the screen needs that centre. So the
encoder cannot sit on the ring's axis.

1. **The ring drives the encoder through gears.** Teeth on the inside of the ring (56 teeth,
   module 1) mesh with a 20-tooth gear on the encoder shaft, which sits 17.65 mm off-centre under
   the screen. In an internal gear pair both gears turn the same way, so turning the ring clockwise
   turns the encoder clockwise.
   - 1 ring turn = 2.8 encoder turns. With a 20-detent EC11 that is 56 clicks per ring turn,
     6.4° each.
   - The detent is felt 2.8× stronger at the ring, so the clicks stay crisp on a big ring.
   - Tooth play is ±0.25° at the ring. For an internal pair the play comes from moving the
     pinion *towards* the axis (`BACKLASH_CD`).
2. **The encoder hangs from a bracket by its own nut.** The M7 bushing goes through the bracket
   and the nut clamps it, so the gear centre distance comes from one printed hole, not from the
   HW-040 board's holes.
3. **Pressing the screen uses the HW-040's own push switch.** The black cap is hinged at the back
   (two pins snap into C-holes in two posts). A nub under the display clamp rests on top of the
   encoder shaft. Pressing the cap pushes the shaft down 0.5 mm and clicks the EC11's switch
   (GPIO10, unchanged). The hinge is as far from the shaft as from the cap centre, so a press in
   the middle needs the same force as the switch itself. The pinion moves down with the shaft
   and stays in mesh.
4. **The ring rests on a fixed sub-deck** under the tiles. It turns in a Ø66 hole, and a
   retainer ring screwed to its underside (3 × M2) sits under the sub-deck, so it cannot lift
   out. The cap's flange sits under the
   ring's top lip, so the cap cannot lift either.
5. **Square window.** 24 × 24 mm with 1.5 mm corners and a chamfer, over the 23.4 mm square
   active area: the whole screen shows.
6. **The top is the buttons.** Four quadrant tiles cover the whole top, from the ring out to the
   body edge, with 0.3 mm gaps between them and round the ring.
   - **The switch holds the tile**, the way a standard 12×12 switch cap is held. Under the
     middle of each tile (44 mm out along the diagonal) hangs a square socket that snaps over the
     switch's square tip. Its four walls flex (slits at the corners, freed 1 mm up into the
     tile). Two of them have hooks with a 45° lead-in below and a flat top that catches under the
     tip. The tile rests on the tip and lifts at most 0.1 mm before the hooks catch.
   - 3 legs sliding in guide bosses in the sub-deck keep a big tile from tilting, so pressing
     near a corner still moves it evenly onto its switch.
   - A 1.2 mm lip on each tile's outer edge hangs over the wall top. The walls and floor sit
     1.5 mm inside the tile outline, so the top seems to float over a shadow line.
   - The tiles print face down, so all four faces get the same smooth bed finish.

## Parts

| Part | Material | Print orientation (in the STL) |
|---|---|---|
| `Shell` (walls and sub-deck) | dark PLA | sub-deck down |
| `Ring` | light or silver PLA (or PETG) | top down; 0.12–0.16 mm layers for the teeth |
| `Retainer` | any PLA | flat |
| `Floor` (hinge posts, switch pedestals, SuperMini seat) | dark PLA | flat |
| `Bracket` (encoder bracket) | any PLA | plate down |
| `Pinion` | PETG or PLA | flat; 0.12–0.16 mm layers |
| `Cap` | glossy black PLA (smooth PEI sheet) | face down |
| `Clamp` (display clamp with the shaft nub) | any PLA | plate down |
| `TileBackLeft` / `TileBackRight` / `TileFrontLeft` / `TileFrontRight` | dark matte PLA | face down |

None of the parts needs supports.

## Bought parts

| Part | Notes |
|---|---|
| ST7789 1.3" 240×240 (GMT130) | Glass in a pocket behind the window, clamped by `Clamp` (4 × M2 × 6 self-tapping). Header on the right, as on v1, so `Rotation::Deg90` stays. |
| HW-040 rotary encoder | Through `Bracket`, its nut on top. The pinion pushes onto the D-shaft. Its SW pin is the screen press. |
| 4 × 12×12 push switch, 10 mm tall | One more than v1. In the corner pedestals; legs down through the holes, wires out through the side channel. |
| ESP32-C3 SuperMini | In its seat on the floor, components up, USB-C out of the right wall. A drop of hot glue. |
| 3 × M2 × 6 self-tapping | Retainer to ring. |
| 4 × M3 × 8 self-tapping | Through the floor into the wall bosses. |
| 4 × 10 mm stick-on rubber feet | In the marker rings. |

## Assembly

1. Press the bracket legs into the floor. Put the encoder bushing up through the bracket and
   tighten the nut. Push the pinion onto the shaft.
2. Put the four switches into the corner pedestals, legs down through the holes.
3. Glue the SuperMini into its seat, USB-C towards the right wall.
4. Put the display into the cap (glass into the pocket) and screw the clamp on.
5. Wire point to point. The display header ends right above the SuperMini.
6. Snap the cap's two hinge pins into the posts. The clamp's nub now rests on the shaft.
7. Put the ring into the sub-deck hole from the top and screw the retainer to it from below.
8. Put the four tiles on top of the shell, legs down through the guide bosses.
9. Lower the shell over the floor. Turn the ring a little until its teeth slide over the
   pinion. The ring's lip stops above the cap's flange.
10. Press each tile firmly once: its socket clicks onto the switch tip. Screw the floor on
    (4 screws) and stick the feet on.

## Firmware notes

- **Encoder:** same pins and the same direction. One ring turn is 56 detents instead of 20, so a
  volume step per detent feels finer. Scale it in software if needed.
- **Press:** the HW-040 SW pin on GPIO10, unchanged.
- **Fourth button:** the firmware has three context buttons (GPIO0, 1, 20). The cleanest free pin
  for a fourth is **GPIO9**, the SuperMini's BOOT pin: it already has a pull-up and the BOOT
  button on it. The only side effect is that holding this button while plugging in USB starts
  the ROM download mode. GPIO21 (UART TX, the boot log) is the alternative, with a 1 kΩ series
  resistor. Either way `CONTEXT_BUTTONS` in `profile.rs` needs a fourth entry.

## Open points (measure or test first)

1. **EC11 shaft length.** The pinion sits 1.2–5.8 mm above the bushing and the nub rests on the
   shaft top. Check the D-flat reaches that far (`EC11_SHAFT_H`, `EC11_BUSH_L` are NOMINAL).
2. **Gear fit.** Print the `Pinion` and the `Ring` first. If the gears bind, raise `BACKLASH_CD`;
   if they skip, lower it.
3. **Ring friction.** PLA on PLA: a little PTFE dry lube on the sub-deck round the hole.
4. **Switch tip size.** The socket is made for a 4.0 mm square tip on a 3.0 mm neck, 1.8 mm
   tall (`SW_HEAD`, `SW_NECK`, `SW_HEAD_H`, NOMINAL from the seller drawing). Measure your
   switch's tip before printing: the red cap in the reference photos fits it, so the tip is
   the part to copy. If the snap is too tight or too loose, change `SOCK_CLR` and `SOCK_HOOK`.
   Print the tiles in PETG if PLA walls crack when snapping.
5. **Tile feel.** The legs have 0.15 mm clearance in the guide bosses. If a tile sticks, open
   `LEG_HOLE_D`; if it rattles, close it.
6. **Press feel.** The cap's front edge moves 1.2 mm for the shaft's 0.5 mm. If the press feels
   too heavy or too long, move the hinge (`HINGE_Y`).
7. **Display glass size** (`DISP_GLASS`) is still NOMINAL; the pocket has 0.3 mm per side.
8. **SuperMini BOOT/RST** are not reachable. Flashing over USB normally does not need them.

## Upgrade path

- A **magnetic angle sensor** (for example an AS5600 under a magnet on the ring) would remove the
  gears altogether, but that is a new part and new firmware.

## Files

`build_case.py` is the whole model; every dimension is a parameter at the top. Rebuild from the
repo root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-experiment/build_case.py
```
