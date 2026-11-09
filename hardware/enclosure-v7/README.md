# Kivori enclosure v7 — Terrace prototype

V7 uses the black and white Terrace exterior, six pillars and a separate display cowl held by four pegs. The RP2040-Zero sits on the **underside of the carrier** with its components facing the bottom cover, its USB-C port in the far wall. There is no RGB LED in this revision: the white halo ring on the face is trim only.

## Before you print

Every assumption that changes the physical build:

1. **Wire**: 0.3 mm copper wire, treated as possibly bare. It is **wire-wrapped** on every pin (about 3 turns, 0.9 mm of pin) and lies in its **own groove** (0.6 wide x 0.5 deep). A wire changes face only through its **own 0.8 mm hole** (9 holes). No two wires cross or share a groove or hole, and different nets keep at least **0.7 mm** of plastic between them (pin holes included; the tightest spot is exactly 0.70). The wires are 18 runs, 809 mm of groove, the longest 114 mm: cut each 10 mm over.
2. **RP2040-Zero** is back at its original height, header plastic against the carrier back (no spacer, no recess). The pins stand **4.0 mm** out of the carrier front, room for two 0.9 mm wraps. The carrier holes grip them (`RP_PIN_FIT_D` 0.90, not a verified fit: print `PinFitCoupon` first). The board is now **centred on the case** (x 43.0, it was 46.0); its USB hole moves with it.
3. **USB-C hole is 1.5 mm lower than the model first had it** (`USB_HOLE_DZ = -1.5`, relative to the modelled receptacle: hole centre z -18.47 instead of -16.97). On the print of the first v7 the hole came out 1.5 mm too high against the real connector, so the real connector is taken to sit 1.5 mm lower than modelled. The board did **not** move; the build check clears the hole against the connector shifted down by the same 1.5 mm (margin 0.32 mm). The pocket inside the wall is lowered to match. The back plate's reset guide is back at its original length. Verify with the real board before fitting the cover.
4. **Display header**: standard 2.54 mm header, pins 6 mm below the plastic (`DISP_PIN_L`). The pin tips stop 0.8 mm above the carrier. **Bench step before mounting the display**: wrap each display wire at the root of its header pin (just under the plastic) and leave a 15 mm tail; GND takes three wires, 3V3 two. Each tail then runs down its pin into the **1.2 mm x 1.0 mm deep pocket** under that pin, which leads into that wire's groove. Check the real module's pin length. The display has all **four standoffs** again; the GND and 3V3 grooves pass below the header-end one.
5. **Pin map is the original one** (GPIO3 / GPIO4 / GPIO5 encoder SW / DT / CLK, GPIO6 / GPIO7 / GPIO8 left / middle / right key), as the firmware uses it. The routes need no change: every left-column wire has a groove on the front to a hole at the top of its own column, and the columns run side by side on the back.
6. **Encoder and key power come from the display header**: the display's GND and VCC wires go straight from the RP2040's GND and 3V3 pins (short, no detour), and the encoder 3V3, encoder GND and key GND wires are wrapped on the display's VCC and GND pins as well. The RP2040's pin fence cannot be crossed (0.7 mm rule) and the display wires are in the way on the other side.
7. **Cowl pegs**: four round pegs, Ø3.0 x 3.0 mm, on the cowl underside; Ø3.5 through holes (0.25 per side) in the shell face. The tips come out 0.5 mm behind the face. Glue them or friction-fit; print one cowl and test the fit. The cowl has a 0.4 mm 45° chamfer on its seating edge and sits flat on the face.
8. **Knob raised 2.5 mm** (`KNOB_GAP` 4.3), same 11.0 mm height, so the shaft enters it only **3.0 mm** (was 5.5), all of it D-flat. Check the grip on the real shaft; add glue or a set screw if it slips. The pressed knob (0.5 mm travel) stays 3.8 mm off the face, 3.2 mm off the halo ring and 4.2 mm off the cowl.
9. **Halo ring** is a plain 0.6 mm trim ring glued to the face. There is no LED, roof or reflector.
10. **No printed feet.** The back plate's bed face is flat. Stick rubber bumpers on it over the six screw counterbores (**Ø4.4 mm**, clearance hole Ø2.4 above it): a round bumper of **Ø8 to Ø12 mm** covers one and stays on the flat bottom (the flat allows up to Ø12.3 at the tightest screw). The stepped bridge over each counterbore is unchanged.

Reprint from the first print: Shell (USB hole and pocket, pegs, no LED), DisplayCowl, Carrier (new grooves, holes, board x), Knob, HaloDiffuser, BackPlate (flat bed face, reset guide at the new x). KnobCap prints the same. Unchanged: caps, bezel. `RpSpacer`, `ScrewFeet` and `HaloReflector` are gone.

## Previews

- `preview-exterior.png` — assembled exterior, raised knob.
- `preview-front-wiring.png` — carrier front: the display wires and the display's power grooves round the east and south of it.
- `preview-rear-wiring.png` — carrier rear: the west corridor from the vias to the encoder pins and the key legs.
- `preview-closeup-display-rp2040.png` — close-up of the RP2040 pins and the display pockets, with the wire-wrap rings.
- `preview-cowl-pegs.png` — cowl underside with the four pegs, seen from below.

## Pin-only RP2040 mount

The board is held solely by its soldered male header pins in the carrier holes (`RP_PIN_FIT_D = 0.90` mm, a CAD value, **not a verified PETG press fit**). Print `PinFitCoupon.stl` in the same orientation and settings first. Its five rows of nine holes have one to five engraved dots: 1 = 0.80, 2 = 0.85, 3 = 0.90, 4 = 0.95, 5 = 1.00 mm. Test the actual header strip without forcing it; change `RP_PIN_FIT_D` and rebuild if needed. A coupon cannot establish the assembled board's resistance to repeated USB insertion and removal; test gently. Do not power an unstable assembly.

Solder the full male header strips to the board: **all 23 pins**, i.e. the nine-pin left column (GPIO0 to GPIO8), the nine-pin right column (5V, GND, 3V3, GPIO29, GPIO28, GPIO27, GPIO26, GPIO15, GPIO14) and the five-pin bottom row (GPIO13, GPIO12, GPIO11, GPIO10, GPIO9). The carrier has a hole for every one of them; the unwired pins are present in the plastic only. The build script fails unless all 23 holes go through the carrier. The controller is rotated physically: its component-side left and right exchange sides when viewed from the carrier front. Use the printed board labels to identify the pins.

## Wiring

| RP2040-Zero | Function |
|---|---|
| GPIO3 / GPIO4 / GPIO5 | Encoder SW / DT / CLK |
| GPIO6 / GPIO7 / GPIO8 | Left / middle / right key |
| GPIO26 | LCD SCL (hardware SPI1 SCK) |
| GPIO15 | LCD SDA (hardware SPI1 TX) |
| GPIO14 / GPIO13 / GPIO12 | LCD RES / DC / BLK |
| 3V3 | Display VCC |
| GND | Display GND |

GPIO0-2, 9-11 and 27-29 are free. Encoder 3V3 and GND and the key ground are taken from the display's VCC and GND pins (see "Before you print"). No RP2040 firmware is in this branch; the pin map above is the original one, which the firmware on `feat/rp2040-zero-board-179` (`firmware/pip/src/physical_st7789.rs`) uses (not checked from here). SPI1 uses SCK on GPIO26 and TX on GPIO15 ([Raspberry Pi's RP2040 datasheet, Table 3](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf)), RES, DC and BLK are plain outputs, and the encoder and key inputs need pull-ups.

Wrap steps (3 turns, tight, tail trimmed; bare wire, so check isolation between neighbours after each group):

1. **Bench, before mounting the display.** On each display pin wrap the wires at the pin root, lowest wrap first: GND three wires (to the RP2040 GND pin, encoder GND via the east groove, key ground via the far-east groove), 3V3/VCC two wires (to the RP2040 3V3 pin, encoder 3V3), the other five pins one wire each. Leave a tail on each long enough to reach the RP2040 end.
2. **Mount the display** (four standoffs and pegs). Press each tail down its pin into the pocket under it, then lay it in its groove. The seven display grooves run to the RP2040's right column and bottom row, and none crosses another.
3. **RP2040 side.** Wrap each display wire on its pin, 4 mm of pin stand above the carrier: GND and 3V3 on the right column, GPIO26, GPIO15, GPIO14 below them, GPIO13 and GPIO12 on the bottom row. Every pin has a free ring of 1.3 mm round it.
4. **Left column to the back.** GPIO3-8 each have a groove west to their own 0.8 mm hole. Wrap the wire on its pin, lay it in its groove, drop it through its hole and, on the back, straight down its column of the west corridor. The key wires (GPIO6-8) are the westmost columns; they turn east above the keys and go straight down to the upper right leg of the left, middle and right key. The encoder wires (GPIO3-5) turn east into the SW, DT and CLK pins.
5. **Encoder power and key ground.** The 3V3 and GND wires from the display pins run on the front east and south of the display, then west and down to a hole east of the encoder pins and a short hop west on the back to the encoder + and GND pins. The key ground wire runs far east (x 75) down to a hole beside the right key and west on the back to its free lower right leg. Link the keys' ground chain with two short wires on the back (left lower-right to middle lower-left, middle lower-right to right lower-left).
6. Check 3V3 to GND for a short before plugging in USB.

Wrap room (model values): RP2040 pins 4.0 mm above the carrier (3 turns need 0.9 mm), encoder pins 4.0 mm bare behind the carrier, key legs 1.5 mm behind it (the legs are flat 0.7 x 0.3, wrap tight). Each key's two lower legs (12.5 mm apart) are internally connected; the ground chain relies on this: check with a multimeter that the switch is not rotated 90°.

A **1.4 mm reset access hole in the bottom cover** is aligned to the flipped board's RESET button; its internal guide ends 0.6 mm before the modeled button. Use a slender blunt nonconductive probe and press gently. BOOT remains accessible with the case open. Verify alignment and travel with the real board before fitting the cover.

## Assembly order

1. Solder headers to the RP2040-Zero, press the board into the carrier.
2. Wrap the display wires on the display pins on the bench (wiring step 1), mount the display, lay the tails.
3. Wrap the RP2040 pins and lay the grooves, then the left-column wires through their holes. On the back wrap the encoder pins and key legs (fit the encoder module after the key legs, as its board covers the area).

## PETG printing

The export folder contains individually oriented STLs. Start with a 0.4 mm nozzle, 0.20 mm layers and your normal PETG profile on textured PEI. Slice and inspect the carrier grooves, the cowl pegs and shell holes, and the reset guide. Print the fit coupon first, then test one keycap, the cowl pegs, the reset guide and a USB plug with the actual hardware.

| Black | White |
|---|---|
| Shell, DisplayCowl, Carrier, Knob, PinFitCoupon | BackPlate, DisplayBezel, KnobCap, CapLeft, CapMiddle, CapRight, HaloDiffuser |

## Rebuild

With FreeCAD 1.1:

```sh
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-v7/build_case.py
```

The script checks modeled fit and exports the FreeCAD assembly, individually oriented STLs and carrier-outline DXF. Electronics and wire are assembly references rather than printed parts. This remains a prototype: check physical fit, retention and operation before relying on it.
