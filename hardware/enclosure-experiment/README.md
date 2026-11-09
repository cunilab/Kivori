# Kivori experimental enclosure: turning ring, static screen, four tiles

A 106 × 106 mm square desk controller with a geared Ø76 ring around a fixed 7-pin GC9A01 round screen.
The four quadrant tiles are the buttons; pressing either the rotating ring or the vertically
sliding black bezel clicks a separate central tactile switch beneath the fixed display. The body tilts 9.7° towards the user.

This revision brings the v4 mechanical and wire-wiring details into the experiment:

- Reinforced, ribbed four-sided tile grips with a stop resting on each switch head.
- M7 nut-and-washer encoder mounting on the carrier. **No encoder supports on the floor.**
- Locating spigots through the carrier into sockets in the floor.
- Four hidden v4-style floor hooks engaging internal shell ribs; the four screws clamp the case.
- A guided RST pinhole, cable relief, central press stops, four replaceable flexures and a captured ring.
- Four removable printed TPU feet covering the screws.
- 0.3 mm enamelled copper wire in channels, with Kapton at crossings, replacing copper-tape traces.

**Prototype status:** geometry and sampled movement are checked by the builder. Printed gear fit,
snap fits, switch force, friction, and wiring reliability still need physical testing.

## GC9A01 module reference

This revision uses the **seven-pin Syalis module and the first supplied drawing**:
[dimension drawing](reference/gc9a01-7pin-dimensions.png),
[module/pinout photo](reference/gc9a01-7pin-module.png).
The later eight-pin Surenoo drawing is not used.

- PCB: 38 × 45.5 × 1.6 mm, connector tab 22.92 mm wide.
- Active circular display: Ø32.4 mm; bezel opening Ø35 mm with a full-depth 2 mm flare.
- LCD assembly: 35.6 × 38.1 mm outline, 1.5 mm front thickness.
- Two Ø2 mm holes: 19.26 mm apart, 5.83 mm from the connector end.
- Seven pins, 2.54 mm pitch; pin row 1.76 mm from the connector end.
- The body is 6 mm wider and deeper than the ST7789 version. USB stays centred at the back;
  the extra rear clearance keeps its reset guide outside the larger retainer.
- The tab points right in the enclosure. From +y to -y the row is VCC, GND, SCL, SDA, DC, CS, RST.
- The two pegs locate the board; two pads support its far side. Four columns outside the
  PCB connect the moving bezel to the flexure-supported bridge. Its skirt has connector/flex relief.

The drawing does not dimension the narrow LCD flex tail in full. Its 14 mm keepout width is
provisional and must be checked on the actual module. The rear component keepout is 1.2 mm.

**Firmware is not ready for this display.** Existing ST7789 firmware drives GPIO0 as backlight
PWM, but this revision wires GPIO0 to CS. It requires GC9A01 initialization, active-low CS on
GPIO0, and removal of that PWM output. This seven-pin board has no exposed backlight control.
Round-screen UI layout also needs validation; the former square screen's corners are not visible.
No firmware has been changed or flashed as part of this enclosure revision.

## How it works

The off-centre encoder drives a 64-tooth internal ring through a 20-tooth pinion, module 1.
Both turn in the same direction. One ring turn makes 3.2 encoder turns: with a 20-detent EC11,
that gives 64 detents per ring turn. `BACKLASH_CD` sets the printed gear play.

The encoder is now **rotation only**. Its unpressed shaft tip is 17 mm above the carrier's
nut-side face, as confirmed by the user. Its body rests against the carrier back; the bushing
passes through a Ø7.2 mm hole and the M7 nut/washer clamp it from above. The pinion keeps its
shortened hub. There is no press nub touching the encoder shaft and no encoder support on the floor.

The display stays fixed on its posts and stacked sockets. Four Cap columns outside the PCB
snap into blind sockets in the removable `PressBridge` below it. That X-shaped bridge pushes a **fifth tactile
switch at the enclosure centre**. Four separate folded `Flexure*` parts connect the bridge to
carrier pedestals through shouldered split snap pillars. No flexure screws are needed.

The Cap face is continuous, with no exposed mounting holes. Four concealed Cap tongues catch
an internal ring ledge with 0.18 mm axial running clearance. This replaces the separate
`RingKeeper`. The ring, Cap, bridge and moving flexure ends depress together; the display,
encoder and pinion stay fixed. The hooks provide retention while the flat bearing carries presses.

The bridge occupies deck z29–31 mm. Its top leaves 2.64 mm to the modeled display rear-component
keepout. Its central actuator reaches the switch at z22.5 mm. Four stops beside that switch
limit nominal travel to 0.45 mm. The trial click stroke is 0.35 mm, matching the typical travel
in the [TS13 datasheet](https://www.sameskydevices.com/product/resource/ts13.pdf).
This is a setting for a measured prototype switch, not a guarantee for every TS13 variant:
the datasheet range reaches 0.45 mm, and a tilted stop can engage earlier. Measure the actual
switch before approving the stop setting. The switch actuator height in this model remains
7.2 mm, matching the existing tile model; check the bought fifth switch against it.

Read [the press prototype assembly and checks](PRESS-PROTOTYPE.md) before printing. Four folded
leaves provide elastic support; neither their stiffness nor parallel movement is established
by the clearance tests. This revision is intended for an eight-position physical trial.

### Why the retainer stays

The **retainer snaps into the rotating ring under the fixed sub-deck** with three upward
printed tongues. Their flat barbs catch matching pocket floors. The retainer underside stays
flat: there are no screw heads near the SuperMini. Its top remains 0.2 mm below the deck at
rest, preventing lift out while allowing the 0.45 mm press stroke. The retainer captures the
ring in the shell; the separate concealed Cap hooks limit ring-only rocking. It is not an
encoder support. Small release openings expose the retainer tongues for a thin tool when
accessible; a practical release sequence in the complete case still needs checking.

See [the ring/retainer snap preview](reference/retainer-snap-exploded.png). Test the three
joints before enclosing the carrier; do not force a tongue that has not fully expanded.

Each tile now has a 2.8 mm face and three longer guide legs with 7.6 mm engagement. The
third guide is separate from the case-screw pillar; the other two move outward to clear the
SuperMini. The tile underside starts 1.4 mm above the fixed deck, leaving 0.9 mm during the
0.5 mm clearance-test stroke. Its legs retain 1.1 mm above the carrier at that position.
These are movement clearances, not a specification for the actual switch travel.

Three guide legs resist tilt under a corner press. Its switch grip uses v4's two wide flexible snap
plates, two shorter ribbed plates, and a central stop. The wide plates are 1.6 mm thick.
The tile faces print against the bed, with 0.3 mm seams between tiles and 0.6 mm radial
clearance to the ring. The outer running clearance is separate from the centring fits.

### Press feel and fitting

See the [clearance review](CLEARANCE-REVIEW.md) for current dimensions and the previous
encoder-press failure. The 17 mm shaft correction fixed contact height but did not make an
off-centre encoder actuate evenly from all ring edges; the centred-switch trial replaces that load path.

The Cap/ring rotation fits and the tile guides still need printer calibration. Do not enlarge
the central stop travel to hide an actuator-height error. Test the bridge, flexures, snap joints
and one actual switch first. Every ring position must click, return and rotate freely.
Four folded leaves do not mechanically synchronize all corners; a full physical test is
required before claiming uniform feel, and further guidance changes may still be needed.
Carrier bending, snap-joint movement and switch force can also affect feel.

The existing four tile guides and their v4-style switch grips are unchanged. Check each tile
at its centre and three corners after the new mechanism has passed its own trial.

### Shell and floor grips (like v4)

Four flexible hooks grow from the floor: two along each side, at deck y = ±24 mm. They click
over ribs inside the shell, so there are no external latch windows. As on v4, these detents
align and hold the floor during assembly; the four M2 × 12 screws still clamp the shell,
carrier and floor together.

The hook dimensions match v4: 8 mm wide, 1.4 mm thick, 10 mm long, with a 1 mm barb and 45°
ramps both ways. The ramps let the floor pull out after removing the screws. Required inward
flex to pass the catch is about 0.8 mm; the builder checks the released position and a sampled
release path with 0.85 mm flex. All hooks and catches stay below the carrier, clear of its
wiring, so no carrier cutouts are needed. Print the floor in PETG and test latch feel before
repeated use; actual flex force and fatigue are not simulated.

## Wire wiring

The carrier prints **wire-channel side down**. There are no copper-tape pads or blind pin-hole
skins to drill. Lay 0.3 mm enamelled copper wire in the 0.8 mm wide, 0.7 mm deep channels.
Bends are rounded. The builder checks at least 0.7 mm of plastic between unrelated channels,
holes, and the board edge.

Each run is a single wire from pin to pin. Branches join at a pin, never midway along a run.
There are 21 runs and 39 crossings; the builder reports the current number of Kapton crossings. The square layout requires more crossings than v4; each crossing has a
lower dip and a Kapton seat. Adjacent seats can use one continuous strip of Kapton rather than
separate overlapping squares. Use `wire_crossings()` and the `Dummy_Kapton` model to locate them.

At each crossing the lower channel dips to 1.3 mm, leaving 0.7 mm of carrier beneath it.
Lay the lower wire first, put a 2 × 2 mm Kapton square in its seat, then lay the upper wire.
The model shows `Dummy_Wire` and `Dummy_Kapton`. Use short Kapton strips to hold long runs down.

**Joints, as on v4:** make a sample wrap and strip the full length that contacts the pin, then wind four
neat, tight turns behind the carrier. Four turns around a 0.64 mm square pin need roughly
14 mm of 0.3 mm wire before lead allowance; do not rely on the previous 12 mm estimate. Keep bare turns clear of adjacent
pins. At the switches, wind the wire around the leg, then fold the leg over it into the printed
pocket. Each switch's two front legs are connected inside: use one for ground in and the other
for ground out. Clip unused rear legs short. Scrape enamel with fine sandpaper or a blade;
verify bright copper all around and check continuity before closing the case.

The SuperMini pins expose 4 mm behind the carrier. Use a **long-tail lower female display
header**, 8.5 mm body with 6 mm tails: that exposes 4 mm for wrapping. A usual short-tail header
is too short here. The upper socket and display header stay standard. The encoder pins are
wrapped in the gap between the HW-040 header and carrier. Two wires share the SuperMini GND, encoder + and encoder GND pins; keep their windings
separate and stacked. The model sizes these wrap envelopes from endpoint counts. SuperMini
6 mm tails mean length past the header plastic, leaving 4 mm below the 2 mm carrier.

Use bought modules with their headers already soldered. Wire-wrapped joints are experimental:
check every connection with a meter and repeat the check after handling. No soldering onto a
printed copper-tape board is needed in this revision.

### GPIO map (GC9A01 revision)

| From | To |
|---|---|
| SuperMini 3V3 | HW-040 +, display VCC |
| SuperMini GND | HW-040 GND, display GND, ground pair of each switch |
| GPIO4 / 3 / 2 / 1 / 0 | Display SCL / SDA / RST / DC / CS |
| GPIO7 / 6 | HW-040 CLK / DT |
| GPIO5 | Central ring-click switch, signal pair; opposite pair to GND |
| GPIO9 | B1, front-left tile |
| GPIO8 | B2, back-left tile |
| GPIO10 | B3, back-right tile |
| GPIO20 | B4, front-right tile |

The component-side view has USB-C at the back: the power row (5V, GND, 3V3, IO4–IO0) is on
the right, and the IO row (IO5–IO10, IO20, IO21) on the left. This fixes the mirrored rows in the
previous experiment. Encoder rotation and B1–B3 match v4 and `firmware/pala/src/profile.rs` on current main.
The central click uses the former encoder-SW GPIO5 input; leave the HW-040 SW pin unwired.
Current main configures GPIO5 with an internal pull-up. No firmware was changed.
The four SCL/SDA/RST/DC assignments are retained; GPIO0 is now CS instead of BLK. The older firmware checked out in this enclosure worktree uses another map.
B4 uses GPIO20; it is not supported by the current three-button firmware. GPIO21 and 5V stay
unwired. The display header stays on the right, retaining the experiment's display rotation.

### Inspecting the pin labels

Open the FreeCAD document, hide `Floor` and `RubberDots`, show the `PinLabels` group, and choose
Bottom view. There are 48 v4-style annotations with leader lines: all SuperMini pins, GC9A01 display
pins, encoder pins, and 20 switch legs, including the new centre switch. Each label names the pin and the modeled net (`IO7: CLK`,
`Enc +: 3V3`, or `n/c` for no external wire). Switch labels use B1 front-left, B2 back-left, B3
back-right, B4 front-right; FL/FR/BL/BR refer to a switch's legs from the component side, with
USB-C at the back. Looking from underneath mirrors the component-side view; follow the leaders.
The labels and leaders are document-only, excluded from STLs and interference checks.
Compare the silkscreen on the actual boards before connecting power, then meter continuity and
shorts; annotations describe CAD connections, not measured hardware.

## Parts and printing

| Printed part | Material | Exported print orientation |
|---|---|---|
| Shell | PLA or PETG | Sub-deck down |
| Ring | PLA or PETG | Top down; 0.12–0.16 mm layers for teeth |
| Retainer | PETG trial for integral tongues | Flat annulus down, tongues up |
| Carrier | PETG preferred for sustained clamp loads | Wire channels down, posts up |
| Pinion | PLA or PETG | Teeth down; 0.12–0.16 mm layers |
| Cap (moving bezel) | Black PETG trial for integral snaps | Face down |
| PressBridge | PETG trial for integral snaps | Flat top down, posts/nub up |
| Four Flexure parts | PETG trial | Flat top down; 0.8 mm leaves |
| Four tiles | PETG preferred for snap fingers | Face down |
| Floor | PETG preferred for flexible hooks | Desk side down |
| RubberDots (four feet) | TPU, about 95A | Flat, plugs up |

Use a 0.4 mm nozzle and 0.2 mm layers except the gears. The design uses 45° ramps and short
bridges; inspect the slicer preview, especially channels, snap lips, reset guide, and foot seats.
The 10 mm feet have compliant plugs that grip their sockets and pull out for screw access.

Bought parts:

- GC9A01 1.28-inch 240 × 240 seven-pin display, matching the supplied first drawing and photo.
- Two 7-pin 2.54 mm female headers, 8.5 mm body height; **lower one with 6 mm tails**.
- HW-040 encoder with its M7 nut and washer; straighten its five right-angle header pins or
  fit a straight header. The pinion grips the measured 6 mm D-shaft.
- Five 12 × 12 switches with the specified 3.8 mm square snap head over a 3.0 mm neck.
- ESP32-C3 SuperMini on soldered headers with 6 mm long pins past their plastic.
- 0.3 mm enamelled copper wire and a few centimetres of Kapton tape.
- Four M2 × 12 for the case, under the feet.
- No ring-retainer or press-mechanism screws are needed in this revision.

## Assembly

Follow [PRESS-PROTOTYPE.md](PRESS-PROTOTYPE.md) for the suspension, bridge, display and Cap
assembly order. Assemble the ring and its snap retainer in the shell separately, then
lower the shell/ring onto the Cap and carrier. The internal ledge snaps beneath the Cap hooks.
Mesh the teeth gently; do not force the ring over the pinion. The rear internal relief now
opens down to the shell edge so the SuperMini board can pass during lowering. Its outer
wall remains 0.8 mm thick across that local relief; inspect this area after printing.

Seat the four tiles, fit the floor onto the locating spigots, engage its four hooks and tighten
the case screws. Fit the TPU feet. Test ring rotation, all eight ring press positions and the
tile centres/corners before closing the wiring inspection.

To reopen, pull the TPU feet out and remove the four screws, then pull the floor evenly away
from the shell to release the four ramped hooks. Do not pry against the carrier or wiring. RST is reached with a needle
through the back-right tile and fixed guide. Its position is nominal: **confirm RST on your
actual SuperMini before printing**. BOOT remains internal. B1 is on GPIO9: holding B1 while resetting selects the ESP32 ROM
download mode. B2 is on GPIO8 (also the onboard LED); release B2 while entering download mode.

## Validation and remaining physical checks

Rebuild from the repository root:

```bash
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-experiment/build_case.py
```

`build_case.py` checks the revised pin assignments and nets, endpoint-specific wrap lengths,
wire-channel clearance and different-net wire separation. It checks valid printed solids,
v4 module datums, display sight lines, assembly insertion, rest interference, central press
strokes, stationary pinion mesh, central-stop engagement,
tile corners, sampled ring/frame tilt, ring-only rocking up to first hidden-hook contact, retainer full-turn
envelopes and snap insertion space, and vertical flexure clearance envelopes. Elastic stiffness, strain, creep,
fatigue, joint preload and real switch travel are not simulated.

Print the new mechanism before committing to the entire enclosure. Test guide/gear fits and
confirm the fifth switch, snap seating, retainer clips and actuator height against the drawing. Do not scale
the flexure STLs: their 0.8 mm thickness is an intentional starting value, not a validated spring.
Use a little plastic-compatible dry lubricant if the ring drags. Confirm the central switch's actual
click travel, display socket height, LCD cable shape, RST position, and TPU plug fit.

No firmware changes are included. GC9A01 initialization/CS and the fourth button need firmware work before use. The ring
makes 64 encoder detents per turn rather than 20. A magnetic sensor could remove the gears in a
future hardware revision, but is outside this change.

See [MEASUREMENT-AUDIT.md](MEASUREMENT-AUDIT.md) for the v4 dimension comparison, corrected
module details and remaining physical measurements.
