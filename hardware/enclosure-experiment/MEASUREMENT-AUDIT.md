# Experiment compared with v4

All dimensions are millimetres. V4 is the reference for the shared bought components, not for
this experiment's larger case or central press mechanism. V4 files were not changed by this audit.

| Interface | V4 reference | Experiment result |
| --- | --- | --- |
| HW-040 header row from module origin | 10.25, measured in v4 | Corrected from 10.35 to 10.25 |
| Encoder shaft | Ø6; 1.5 flat depth in CAD | Same nominal shaft profile retained as requested |
| Printed shaft bore | V4 knob Ø6.25; flat offset +0.1 | Pinion Ø6.15; same flat offset; tighter round clearance |
| Flat to opposite side | CAD implies 4.5; old v4 comment says 4.8 | Follow v4 CAD; 4.8 is not verified |
| Shaft tip above mounted carrier face | Derived from board/body/carrier stack | User's 17 retained; equivalent board-to-tip 26.7 vs v4 26.4 |
| Switch housing and actuator | 11.7 square body; 3.8 square head; 7.2 tip height | Same for four tiles and fifth central switch |
| Switch flat legs | 0.7 × 0.3; 3.5 below housing | Corrected round 0.7 × 3.0 dummy legs to v4 dimensions |
| Carrier switch-leg holes | Ø1.2 | Corrected Ø1.8 to Ø1.2 |
| SuperMini header tails | 6 past the plastic | Same; 4 exposed below the 2 mm carrier |
| Encoder nut/washer | 2 + 0.5 stack | Fits 6.5 bushing projection above carrier |

The 0.3 difference in board-to-tip shaft length follows the user's mounted 17 mm measurement.
It does not change the nominal D-flat profile. The pinion has 0.075 radial round clearance,
versus 0.125 in the v4 knob, so check a pinion print on the shaft before assembling the gears.
That tighter pinion fit is retained; it has not been physically validated. The
experiment's Ø7.2 bushing bore has only 0.1 radial clearance; v4 uses more clearance. Do not
force the bushing into a tight print or enlarge it until the gear can move sideways.

## Other errors corrected

- The Ø33 display aperture clipped the screen edge. It is now Ø35 with a 2 mm full-depth
  flare. Sampled rays pass at 0, 10 and 20 degrees from normal in eight directions, including
  0.5 optical depth behind the glass and 0.15 centring allowance.
- A seated-only clearance check missed the rear wall hitting the SuperMini during assembly.
  The internal USB relief now opens through the shell's bottom edge. The local outer skin
  remains 0.8 thick; inspect its print quality and stiffness before plugging in USB.
- The current revision replaces press screws with shouldered snap pillars and removes the
  separate RingKeeper in favour of concealed Cap hooks and an internal ring ledge. Actual
  pillars and flexure end pads are included in tilted-press and insertion checks.

## What still needs the actual hardware

Use the same switches as v4. Their modeled height is 7.2; v4 does not establish their actual
click travel. The trial assumes 0.35 click and 0.45 stop. A switch needing 0.45 may fail to click
at a tilted edge before the first stop. Measure the fitted switch and test all eight ring
positions before treating this mechanism as accepted. Printed flexure force, return, creep,
fatigue and carrier bending are not established by these geometric checks.

The seven-pin display follows the user's first drawing, not the eight-pin Surenoo drawing.
Verify the assumed 14 mm flex-tail keepout, connector orientation, rear components and actual
module thickness. Inspect the carrier labels and check continuity before power. Firmware
still needs GC9A01 conversion and fourth tile support; this audit changes no firmware.

Rebuild with `build_case.py` to run its dimension, display, insertion, interference, wiring,
latch, press, gear and flexure checks before exporting the matching CAD/STLs. These are sampled
geometric checks; they do not certify every intermediate position or manufacturing tolerance.

## Verification scope

The earlier screwed revision passed a complete source rebuild and six-height insertion audit.
The hidden-snap revision changes the carrier, Cap, bridge, four flexures and ring. Its builder
adds checks for the clean Cap face, twelve snap pillars, compressed insertion space and a
continuous ring capture land. CAD clearance checks do not establish snap strength or the full
case's disassembly tool path. See PRESS-PROTOTYPE.md for the current acceptance procedure.

The hidden-snap revision subsequently passed its full fresh source rebuild and export:
module datums, display rays, positive snap lands, squeezed insertion envelopes at eight lift
heights, static interference, wiring, latches, nominal press/stop, eight gear phases, twelve
tile-corner presses, eight ring-edge presses and the flexure envelopes. Hidden hook contact
was 0.169–0.230 degrees in eight rocking directions. The continuous capture annulus was
checked against the actual ring, so its retention land does not depend on rotation phase.
All printed solids passed validity checks. The CAD is reloaded in FreeCAD; 48 pin labels remain.
Three new `hidden-snaps-*.png` previews show the complete case, cutaway and Cap underside.

## Retainer clip revision

The ring/retainer joint now uses three integral upward tongues and pocket shoulders, replacing
its three screws. The retainer bottom, shell capture gap and gear teeth stay at their existing
datums. Added checks cover nominal seating, positive separation restraint, squeezed insertion,
local release openings and a full-turn envelope of the taller retainer. Physical snap force,
printed tolerance and releasing all three catches remain prototype checks.

The retainer-clip revision passed a complete fresh source rebuild/export: positive catch,
compressed insertion, local release-probe openings, static interference, all previous datum,
wiring, latch, press, gear and rocking checks, plus the new full-turn retainer envelope against
fixed parts and flexures. A separate GUI check found no collision between the local probe
approaches and the unchanged case/electronics. The CAD is reloaded with the retainer in its
actual seated position, and the exploded/detail previews are saved. This remains geometric
validation; real insertion force and a practical three-catch release sequence are unverified.
