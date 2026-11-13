# Kivori enclosure v8 — Terrace, display raised and tilted

V8 is v7 with the display **raised and tilted 12° towards the user** (concept "B, raised, tilted"): the screen faces the seated user, a slim white lip replaces the 2 mm bezel, and the glass sits 1.2 mm under the cowl top (0.6 mm of air, 0.6 mm of roof). The display is still carried by its **own four pillars** (no wedge bracket): the pillars are of different heights, their tops are cut square to the tilted board and the pegs lean with it. The carrier is 2.0 mm thick and prints flat; the later revisions slim its far-east display pillar and widen the lip’s bed-contact face. The back plate, RP2040-Zero position and pin map stay fixed.

![v8 exterior](preview-exterior.png)

## What changed from v7

| | v7 | v8 |
|---|---|---|
| Display | flat, glass 2.1 mm above the face | **tilted 12°** (19.2° to the desk with the 7.2° case wedge), glass 5.8 mm above the face at the window centre (10.4 mm to the cowl top at the far end) |
| Pillars | 4, equal | 4: near pair **10.7 mm**, far pair **15.4 mm** (4.7 taller), vertical, seat cut at 12°, peg leans 12°, a slot in each far pillar clears the leaning header pins |
| Cowl | 4.5 mm, 2.5 mm roof, face-back relief | hood with the top plane parallel to the glass, 10 mm tall at the far end, **0.6 mm roof 0.6 mm above the glass**, four hold-down pads, window 25.2 mm; prints with the tilted top on the bed |
| Bezel | `DisplayBezel` 28 x 28, 2.0 x 0.8, 4 pegs | `DisplayLip`: **1.4 wide x 0.6 high**, 45° bevel over 0.4 mm, a collar locates it in the cowl window (glue it) |
| Header pins | tips 0.8 above the carrier, tail down each pin | pins **stand free** (0.8 over the cradle top and the pillar slots); wire tails drop from the pin roots to a **wire cradle** (bores and funnels) |
| Pegs | 2.0 past the board (heat-stake) | 1.0 past the board, locate only; the cowl's pads hold the board down |
| Window / DISP_C | y 78.0 | y 77.4 (the tilted header's pocket row keeps its v7 y) |

## Before you print

Every assumption that changes the physical build. The v7 assumptions that are not about the display are unchanged (see `hardware/enclosure-v7/README.md` for the wording): 0.3 mm wire wrapped on every pin in its own groove (net to net plastic 0.70 mm, tightest spot exactly 0.70), RP2040 pins in the carrier at `RP_PIN_FIT_D` 0.90 (print `PinFitCoupon` first), USB hole 1.5 mm low, pin map, encoder and key power from the display's GND and VCC, cowl pegs Ø3.0 x 3.0 (glue or friction fit; **v8: friction fit only, so the cowl comes off for service**), knob raised 2.5 mm from the first print (air gap 4.3 mm, flush hub; see Knob below), plain halo ring, flat plinth with rubber bumpers.

New or changed in v8 (the v7 display items 4, 7 and the display standoffs are replaced by these):

1. **Tilt**: `DISP_TILT` 12° about the x axis (front edge, towards the knob, low), pivot at the window centre, `DISP_Z` 5.8 mm. Pillar heights follow from it (10.7 / 15.4 mm). The far-west pillar remains Ø4.5; the far-east pillar (x 58.26, y 89.16) is **Ø3.2** beside the header, with an unchanged peg and tilted seat. It retains a Ø4.5 × 2.0 mm foot, a 0.65 mm high 45° taper and R0.25 fillets at both joins. Its weakest notched section has I = **2.591 mm⁴**, **12.9%** of plain Ø4.5 bending stiffness; seat contact is **8.22 mm²**. Print the Carrier with 3 perimeters and gently press the display in the first print to check pillar flex; the pegs lean 12° relative to the carrier normal, and 12° from build Z when printed flat.
2. **Glass is never loaded.** The module hangs on the four seats by its PCB, located by 1.8 mm pegs in the 2.0 mm holes. The cowl's four **pads** (Ø3.6, bore Ø3.0 round each peg tip) stand `DISP_PAD_GAP` **0.2 mm** over the PCB corner holes and only stop the board lifting off the pegs; they never press. Nothing printed is closer than **0.51 mm** to the glass (0.60 under the roof, 0.70 under the lip's collar, 0.51 beside the header-end pads). The lip stands on the cowl and overlaps the glass edge by **1.05 mm** (near/far edge), 0.85 (header end), 2.95 (flex end): air, never contact.
3. **Flex cable (FPC)** leaves the glass at the end away from the header and lies on the board (assumed: **18 mm wide, never higher than the glass front, 1.4 mm over the board**: measure yours). The cowl's ceiling is one plane 0.6 mm over the glass, so the flex has **2.0 mm** over the board (v7 pinched it at 1.3 and was relieved to 2.5 by a face-back pocket; a pocket would not fit a 0.6 mm roof): the keep-out box is **0.59 mm** from the cowl. Pads and pegs are clear of the 18 mm band. If your flex is wider, higher or stiffer, raise `DISP_GLASS_AIR` (the lip then hides more, see 6). Nothing of the carrier, cowl or lip touches the cable, and the cowl is lowered straight down onto the finished display, so nothing drags it.
4. **Header pins are never bent.** The smaller far-east pillar clears pins by **0.800 mm**, free wire tails by **0.984 mm**, and the cradle by **0.840 mm** (the cradle far end is shaved 0.7 mm). Including the wrap rings, the nearest modeled wire surface is **0.470 mm** away; keep the bench wraps compact. The pins (6 mm below the plastic, `DISP_PIN_L`) tilt with the board and stand free: tips **0.80 mm** over the cradle's top plane (it is parallel to the board, `DISP_PIN_TIP_CLR`) and 0.80 from the slot cut in each far pillar for them. Nothing is plugged into anything. The wires are wrapped at the pin roots (3 turns, ring Ø1.3, bench step as in v7) and leave **0.7 mm west of the pin**, so they clear it: each tail then drops **6.2 mm in a straight line** to a funnel (Ø1.7 at the top, 45°) in the cradle top, leaning 5° one way while the pins lean 12° the other (**0.23 mm minimum from any pin**, growing). The free span is the strain relief: the funnel and the Ø1.2 bore hold the wire still, so a pull on the cable never reaches the pin wrap.
5. **Wire loom, not a ribbon**: the display's ten wires (GND x3, VCC x2, five signals) are the same 0.3 mm wire as everywhere else. A ribbon or FPC extension was rejected because GND and VCC each fan out to the encoder and the keys, a ribbon would add a part, two connectors and a bend radius of its own, and the carrier already has a groove and pocket for every wire. The bend radii are: free span straight; wrap ring R 0.65 mm; the foot (vertical bore to the groove) R 0.6 mm, set by the Ø1.2 bore (all in soft 0.3 mm copper, bent once; do not reposition a wrapped wire more than a few times); grooves `WIRE_BEND_R` 1.5 mm.
6. **Rim**: the lip's window is 24.2 mm (active area 23.4 + 0.4 per side). For a seated eye 40° over the desk (59.2° over the glass, `EYE_ELEV`) the near rim hides **0.42 mm** of the active area (limit 0.5), checked by ray test in the build. The ray is steeper than the 45° bevel, and the 0.2 mm vertical section above the cowl sets the limit with the reduced bevel. The build measures the assembled rim by ray test.
7. **Cowl and lip**: the lip is now **1.4 mm wide**, 0.6 mm high, with a **0.4 mm × 45° bevel** and 0.2 mm vertical edge. Its top face on the bed is **103.85 mm²** (was about 42); check the first layer for adhesion and a continuous ring. The collar and cowl seat are unchanged, so only DisplayLip needs reprinting for this change. The lip prints top down, glue it with the collar in the cowl window (collar play 0.1 per side) on the cowl top. The cowl's roof is 0.6 mm (3 layers) over the glass: print one and check it; thicken `DISP_ROOF_T` (and `DISP_GLASS_AIR`, same pivot) if it is too weak. The cowl is 10.4 mm high at the far end and low at the knob side: the knob stays **4.16 mm** from it at rest and 4.06 pressed (0.5 mm travel; the owner restored the 4.3 mm skirt gap), the halo ring 1.00 mm.
8. **Assembly order** (see below) and service: cowl off, lift the board.
9. **Knob**: owner-approved **4.3 mm** skirt gap (`KNOB_GAP`), **11.0 mm** height (`KNOB_H`), top at z 15.3. The hub and skirt rim are **flush at z 4.3**; the build measures the underside and rejects a protruding hub. The fixed shaft tip is z 7.3, giving **3.0 mm engagement**, all on the D flat. Shaft-tip air remains **1.0 mm**, top thickness **4.5 mm**, and KnobCap peg fit is unchanged. The hub clears the bushing by **8.00 mm at rest / 7.50 pressed**, and the nut by **11.50 mm pressed**. At 0.5 mm travel the knob clears the face by **3.80 mm**, cowl **4.06 mm** (4.16 at rest), and halo **3.20 mm**. Printed top down, its ribs and hub grow from the top, with only the existing **2.00 mm** cap-peg-hole bridges. The previous 5.8 mm gap and protruding hub are superseded; increasing height cannot increase engagement on a fixed shaft.

Reprint from v7: **Carrier** (pillars, cradle and pegs; display wires move 0.7 mm west), **DisplayCowl** (new), **DisplayLip** (new, replaces `DisplayBezel`), **Knob** (flush hub, restored 4.3 mm gap), **Shell** (only the opening in the face, now 40.6 x 28.5 mm: it moves with the display). Reuse from v7: BackPlate, KnobCap, caps, HaloDiffuser, PinFitCoupon.

## Previews

- `preview-exterior.png` — assembled exterior.
- `preview-closeup-display.png` — the slim lip and the tilted screen, seen from the front.
- `preview-section-display-centre.png` — side section through the display centre (x 43): tilt, pillars, glass under the roof and the lip, the cradle and wires behind it.
- `preview-section-header-wires.png` — section through the header (x 58.9): the leaning pins, the straight wire tails, the cradle with its funnels and the slot in the far pillar.
- `preview-section-flex-end.png` — section at x 25: the left pillars, the pads over the pegs and the flex cable keep-out (yellow) under the 2.0 mm of air.
- `preview-cradle-wires.png` — the display on its four pillars over the carrier, with the cradle, pins and wires (no shell or cowl).
- `preview-front-wiring.png` — carrier front: the display wires and the display's power grooves round the east and south of it (the cradle is the rib at the right).

## Carrier thickness and wiring

The carrier is **2.0 mm** thick, from its unchanged front plane z −10.2 to its back z −12.2, with **no back-side component pockets or groove ramps**. Print it **flat, back side on the bed**, with all pillars and the wire cradle growing upwards. The display seats and glass remain at the same height: pillar dimensions are measured from the unchanged carrier front.

The six west wiring columns, encoder power vias and key-ground via are restored to their original positions from `9b9e2d7`; the PCB-outline DXF is byte-for-byte identical to that revision. Front and back grooves are 0.5 mm deep, with straight back-side wire runs. Pin wrap lengths remain **4.0 mm at the RP2040 front, 1.5 mm behind the keys and 4.0 mm behind the encoder**; the seven small display wire entry bores remain 1.0 mm deep. USB connector, RP2040 board height, encoder position, back plate, screw columns, hooks and reset guide remain fixed.

Relative to `9b9e2d7`, Carrier geometry changes only at the far-east display support: the Ø3.2 pillar with its filleted Ø4.5 foot, plus the associated **0.7 mm trim at the adjacent cradle end** that preserves 0.840 mm clearance. The display seats, pegs, carrier plate and all wiring positions are unchanged.

- `preview-knob-flush.png` — before/after sections showing the flush hub and unchanged shaft engagement.
- `preview-rear-wiring.png` — original back-side wiring on the flat 2.0 mm carrier.
- `preview-section-button.png` — side section through the middle button (x 43): keycap on the stem, MX switch, plate and the carrier's seat pocket.
- `preview-section-switch-clips.png` — close section through the clips (x 43): plate, the hooks in the blind pockets under its underside, the 45° gable at the back of each pocket.
- `preview-switch-frame.png` — one plate frame close up (the clip pocket).
- `preview-backplate-pillars.png` — the back plate seen from the face side: four screw columns, ten load pillars, the HW-040 window.
- `preview-section-pillar-key.png` — section at y 12 through the left key: pillar, carrier, switch.
- `preview-dummy-switch-front.png`, `preview-dummy-switch-iso.png` — the dummy Outemu switch, to compare with the drawing's front view.
- `preview-closeup-keys.png` (= `-well`) and `preview-closeup-keys-plate.png` — the keys from a seated eye (40° over the desk, from the front) in each `KEY_STYLE`; `preview-exterior-plate.png` is the isometric of the `plate` style.

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

1. **Bench, before mounting the display.** On each display pin wrap the wires at the pin root, lowest wrap first (the ring sits just under the header plastic): GND three wires (to the RP2040 GND pin, encoder GND via the east groove, key ground via the far-east groove), 3V3/VCC two wires (to the RP2040 3V3 pin, encoder 3V3), the other five pins one wire each. Leave every tail **at least 60 mm** long (6 mm to the cradle, 11 mm down its bore and on into the groove, the rest to reach the RP2040 end and for the rewrap in service).
2. **Thread, then seat the display.** Hold the module just above its pegs and feed each tail through its funnel in the wire cradle (the rib under the header), down the bore into the pocket and into its groove; the funnels are 0.8 mm bigger than the wire and the module is in your hand, so do them one at a time from the near pin (BLK) to the far pin (GND). Pull the slack out until the wire runs straight from the ring to the funnel, then lower the module onto the four pegs (board flat on the four pillar seats). The seven display grooves run to the RP2040's right column and bottom row, and none crosses another.
3. **RP2040 side.** Wrap each display wire on its pin, 4 mm of pin stand above the carrier: GND and 3V3 on the right column, GPIO26, GPIO15, GPIO14 below them, GPIO13 and GPIO12 on the bottom row. Every pin has a free ring of 1.3 mm round it.
4. **Left column to the back.** GPIO3-8 each have a groove west to their own 0.8 mm hole. Wrap the wire on its pin, lay it in its groove, drop it through its hole and, on the back, straight down its column of the west corridor. The key wires (GPIO6-8) are the westmost columns; they turn east above the keys and go to the signal pin of the left, middle and right key (the pin at local +2.54, +5.08). The encoder wires (GPIO3-5) turn east into the SW, DT and CLK pins.
5. **Encoder power and key ground.** The 3V3 and GND wires from the display pins run on the front east and south of the display, then west and down to a hole east of the encoder pins and a short hop west on the back to the encoder + and GND pins. The key ground comes down the east side (x 75) to a via and, on the back, to the right key's ground pin, staying south of the centre posts. Two short back wires continue that ground to the middle pin and then the left pin. The right and middle ground pins take two wraps each; the left pin takes one.
6. **Check the display before the cowl goes on**: nothing touches the glass, the flex cable lies flat on the board and nothing presses on it (a 0.8 mm feeler, or the 0.8 mm tip of a pin, must go under where the roof will be), the pins are straight and clear of every wire except their own ring.
7. Check 3V3 to GND for a short before plugging in USB.

Wrap room (model values): RP2040 pins 4.0 mm above the carrier (3 turns need 0.9 mm), encoder pins 4.0 mm bare behind the carrier, key pins 1.7 mm behind it (2.85 mm pins, the carrier seat pocket 0.8 mm deep). One 0.9 mm wrap fits on a key pin. Where a ground pin takes two wires, wrap both into that same ring; there is not enough pin standing out for two rings stacked. The ground pin is the one toward the left of the switch as you look at the face (local −3.81, +2.54). The other pin is B1, B2 or B3.

A **1.4 mm reset access hole in the bottom cover** is aligned to the flipped board's RESET button; its internal guide ends 0.6 mm before the modeled button. Use a slender blunt nonconductive probe and press gently. BOOT remains accessible with the case open. Verify alignment and travel with the real board before fitting the cover.

## Assembly order

1. Solder headers to the RP2040-Zero, press the board into the carrier.
2. Wrap the display wires on the display pins on the bench (wiring step 1), thread them through the cradle and seat the display on its four pegs (step 2). The module goes in from the front through the face opening, with the cowl off.
3. Wrap the RP2040 pins and lay the grooves, then the left-column wires through their holes. On the back wrap the encoder pins and the key pins (fit the encoder module after the key pins, as its board covers the area). Seat the three switches in the plate from the front.
4. Check the display (step 6), glue the white lip into the cowl window, lower the cowl over the module (its four pads land on the board corners over the pegs, its four pegs go into the shell face) and press it flat. Nothing of the cowl or lip touches the glass or the flex cable.

**Service**: pull the cowl (friction pegs) and lift the board straight off its pegs; 6 mm of free wire between each pin root and the cradle lets you tilt it for a look, no more. To go further, unwrap the ten wires at the pin roots (they are re-wrapped like new; keep the 60 mm tails) and the board is free.

## PETG printing

The export folder contains individually oriented STLs. Keep their exported orientation. Start with a 0.4 mm nozzle, 0.20 mm layers and your normal PETG profile on textured PEI. Slice and inspect grooves, bores, funnels and first-layer contact. **Carrier prints flat on its back**, without the former −45° rotation or edge brim. Its pillars are vertical and display pegs lean 12° from build Z. Print with 3 perimeters and gently check the slimmer far-east pillar on the first print.

The overhang audit samples **every BRep face**, including faces below the previous 0.05 mm² cutoff. Tessellation interior/boundary samples supplement a full UV grid, so trimmed curved faces cannot disappear between sample rows. The carrier has no component pockets. Unsupported slopes must be ≤45° from vertical, bridges must have supported edges and span ≤**10.0 mm**, and the existing one-extrusion-width ledge allowance remains **0.5 mm** (the cap socket's bridge is 0.40 mm; the switch-plate clip lip is 0.5 mm). A 0.001° tolerance handles OCCT approximation of 45° chamfers. Bed faces are counted separately; the report distinguishes self-supporting angles from steep faces certified as bridges/line-width ledges.

| Printed part | Exported orientation / bed face | Worst self-supporting angle | Worst angle including bridges / ledges | Longest bridge | Change |
| --- | --- | ---: | ---: | ---: | --- |
| Shell | Face down, X 180° | 45.00° | 84.00° | 4.94 mm | Face openings 16.4 mm for the MX housings |
| DisplayCowl | Tilted top down, X 168° | 12.38° | 12.38° | 0 | None |
| DisplayLip | Tilted top down, X 168° | 44.93° | 44.93° | 0 | None |
| KnobCap | Top down, X 180° | 0° | 0° | 0 | Assembly position follows knob; export unchanged |
| HaloDiffuser | Face down, X 180° | 0° | 0° | 0 | None |
| PinFitCoupon | Flat underside down, X 0° | 0° | 0° | 0 | None |
| BackPlate | Desk/plinth bottom down, X 7.20° | 45.00° | 82.80° | 2.00 mm | None; supported stepped screw seats retained |
| Carrier | Flat, back down, X 0° | 12.00° | 90.00° | 5.20 mm | MX plate frame on each key; clip-side lip bridges 5.2 mm |
| Knob | Top down, X 180° | 45.00° | 90.00° | 2.00 mm | 4.3 mm gap, flush hub, H 11.0 |
| CapLeft | Top down, X 180° | 22.02° | 90.00° | 0.00 mm | Flat top on the bed; 1u keycap, top 8.8 mm above the face |
| CapMiddle | Top down, X 180° | 22.02° | 90.00° | 0.00 mm | Flat top on the bed; 1u keycap, top 8.8 mm above the face |
| CapRight | Top down, X 180° | 22.02° | 90.00° | 0.00 mm | Flat top on the bed; 1u keycap, top 8.8 mm above the face |

The cowl has a 0.6 mm roof (3 layers at 0.20); inspect its pads, pegs and window in the slicer. The lip keeps its 0.4 mm bevel and 103.85 mm² first-layer ring. Print the fit coupon first, then test one keycap, the switch plate, cowl pegs, reset guide and USB plug with the actual hardware.

**Switch (Outemu, from the owner's drawing).** All numbers are from it unless marked ASSUMED:

| Feature | Value | Source |
|---|---|---|
| Top housing | 15.60 x 15.60 | drawing |
| Bottom housing | 13.95 x 13.95, 15.03 over the clips (clips on ±Y) | drawing |
| Bottom to housing top | 11.00 | drawing |
| Plate level / flange top / clip height | 5.00 / 6.00 / 1.90 | drawing |
| Stem above the housing | 4.10 (top at 15.10) | drawing |
| Stem cross | 4.00 x 4.00, arms 1.10 (Y bar) and 1.30 (X bar) | drawing |
| Centre post | dia 3.85, 2.85 long (length ASSUMED = pins) | drawing / assumed |
| Pins | (-3.81, 2.54) and (2.54, 5.08) from the post; 0.85 x 0.78 section; 2.85 below the bottom | drawing (3.80 / 2.54 / 5.08, 0.78-0.85 callouts) |
| Travel | 4.0 | Cherry/Outemu, unchanged |
| Upper housing taper 13.0 to 11.6, stem well 8.0 x 5.0 | | ASSUMED (read off the front view) |
| Plate thickness | 1.5 | ASSUMED (Cherry plate) |
| Printed plate cutout | 14.1 (drawing plate 14.00 + 0.1 for FDM; 14.3 let the clips slip out) | choice |

The dummy switch is stepped like the drawing: bottom housing with its clips, 15.6 flange on the plate, tapered upper housing with the stem well, centre post, two pins; the stem is a separate dummy so it can be coloured.

**Plate mount (held, not just placed).** Each key has a real plate on the carrier: **1.5 mm thick, its top exactly 5.0 mm above the switch seat** (the floor of the 0.8 mm seat pocket, not the carrier front), cutout **14.1 mm**, continuous all round. The 15.6 flange rests on the plate top and the two clips (on ±Y, out to 15.03 over the clips) snap in under it and hook **0.46 mm under the plate's underside**. The wall under the plate is flush with the cutout all round, so there is no window and no flat bridge: each clip side has only a **blind pocket on the inside of the wall**, a slot 5.4 mm wide at its floor with rounded ends (r 0.5) and 0.9 mm deep. The **only flat ceiling is the catch strip: 4.6 mm wide (the 4.0 mm hook plus 0.3 per side) and 0.5 mm deep (one line width)**, where the plate overhangs the pocket and the hook sits against the cutout edge. From there, over the last 0.4 mm under the plate, the pocket's ends and back wall lean in at 45° from vertical (rounded in plan, internal corners 135° not 90°), so nothing else in the pocket is an overhang and nothing needs support; the cutout corners are rounded (r 0.5) and the frame's outer corners r 1.0. The audit reports the catch strip as a 1.0 mm bridge (it stands on three sides) and the carrier's longest bridge falls from 5.20 mm to 2.56 mm (strip plus its two rounded chamfer corners). Cracking: the plate is tied to the walls in one piece, the clip pull is carried in-plane by the plate and as tension across layers by roughly 80 mm² of wall (east and west walls 1.55 mm thick, clip-side walls 1.15 to 1.85 mm), far more than a switch pull-out needs; the thinnest section is the south wall at the pocket floor, 0.75 mm. The east and west walls run from the carrier up to the plate (1.55 mm thick), so the plate is part of the carrier, and the carrier is screwed to the shell: pulling the cap pulls on the clips and the plate, not on a loose part. `check_switch_plate()` measures the built solids and fails the build unless the plate top is 5.00 ± 0.05 over the pocket floor, the plate is 1.50 ± 0.05 thick, the cutout is at most 14.1 (and at least the 13.95 housing), and a clip hook of the dummy switch reaches at least 0.3 mm under the plate with its top touching the underside (the build prints "switch plate: top 5.00 over the seat, 1.50 thick, cutout 14.10; clips reach 0.46 mm under it"). `preview-section-switch-clips.png` is the section through the clips (x 43); **screws and pillars:** the case now has **four screws**, the corner posts at (6, 7), (6, 87.5), (80, 87.5) and (80, 7); the middle two (16, 40) and (70, 40) and their posts, columns, spigots and sockets are gone (the corners clamp the carrier best: they are the widest spread, the middle pair only fought the display and the west wire columns). The loads are carried by **ten back plate pillars**, Ø3.2, which stop `PILLAR_GAP` = 0.2 mm short of the carrier back so a press goes straight into the back plate: under each key's east side (29.5, 12), (49.5, 12), (69.5, 12) (under the pocket floor, away from the pins, post and ground chain), the west of the row (15.2, 12), between the keys (33, 17) and (53, 17), and round the HW-040 window for the encoder: (36, 29), (50, 29) and (36, 54.2), (50, 54.2). The back plate is a wedge, so the pillars are 5.9 mm tall at y 12, 6.6 at y 17, 8.1 at y 29 and 11.3 at y 54.2 (from the plate's inner surface; each top is at z -12.4, 0.2 under the carrier back). `check_pillars()` measures the built solids: the gap of every pillar to the carrier back (0.20), its plan clearance to every back groove, via, pin hole and post (the 0.7 mm rule; the closest is 2.03), its distance to the HW-040 window (at least 0.5) and its 3D clearance to wires, wrap rings, key pins, the encoder and the RP2040 (at least 0.5; the closest is 1.40). The pillars are vertical in the face frame like the columns, so they print leaning 9.7° and the back plate goes in and out as before. `preview-switch-frame.png` and `preview-switch-frame-with-switch.png` show one frame close up. The seat is sunk 0.8 mm into a pocket in the carrier front (`SW_SINK`), so the 2.85 mm pins stand **1.7 mm behind the carrier**: a 0.9 mm wrap fits (without the pocket they would stand only 0.85 mm). A key pin is 0.85 x 0.78, so its wrap bore is 1.2 mm (the others stay 1.0).

**Keycap (from the owner's keycap drawing).** 18 x 18 base, side walls 80°, front wall 70° (the back wall is ASSUMED 80°), wall 0.91, stem tube dia 5.50, socket mouth 1.30 above the skirt bottom, height 9.5 (the drawing's 9.4 back / 9.68 front, with a flat top), four ribs 0.8 thick (ASSUMED, they start 5.0 above the skirt bottom so they clear the housing at a full press). **The top is flat, with no dish and no rounding**; the audit needs no chamfer. The socket is the stem plus 0.10 mm (cross 4.10, arms 1.20 / 1.40); the drawing's 1.26 arm would pinch the 1.30 stem arm. Printed white, top down on its flat top, no bridges.

**Keys.** The skirt wraps the housing and rides down through the face, so the face opening is the cap base plus 0.35 per side: 18.7 mm. That needs a **20.0 mm pitch** (webs 1.3 mm; keys at x 23 / 43 / 63) and the row at **y 11.5** (the opening then stays clear of the near wall); the halo ring is 38 mm (was 42) so it clears the middle cap. `KEY_STYLE = 'well'` (default) cuts one opening per key; `'plate'` cuts one 58.7 x 18.7 mm slot over the row. Both give the same view from a seated eye. Cap top is **8.8 mm above the face**; at rest the skirt bottom is 0.65 mm below the face, and at a full 4.0 mm press it stops 0.35 mm above the flange, 0.35 mm from the opening and 0.3 mm or more from the housing and the plate. The face itself is not lowered: the openings do that job.

Test-fit one switch in the plate and one cap on the stem before printing the other two. The clips are on ±Y; if a switch will not stay, the plate may have to hang from the shell instead. The socket is sized for PETG shrink in the model, not from a printed cap: if it is tight or loose, change `CAP_SLOT_L` / `CAP_SLOT_ARMS` by about 0.05 mm and reprint one cap.

| Black | White |
|---|---|
| Shell, DisplayCowl, Carrier, Knob, PinFitCoupon | BackPlate, DisplayLip, KnobCap, CapLeft, CapMiddle, CapRight, HaloDiffuser |

## Rebuild

With FreeCAD 1.1:

```sh
freecad.cmd hardware/enclosure-v8/build_case.py          # snap; or freecadcmd
```

The script checks modeled fit and exports the FreeCAD assembly, individually oriented STLs and carrier-outline DXF. Electronics and wire are assembly references rather than printed parts. This remains a prototype: check physical fit, retention and operation before relying on it.
