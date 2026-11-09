# Central press prototype clearance review

This revision replaces the off-centre encoder push with a central fifth tactile switch, an
X bridge, four removable folded flexures, concealed snap pillars and integral ring hooks. Values below are nominal
CAD dimensions in millimetres, not a force, fatigue or printer-tolerance certification.

## Current interfaces

| Interface | Nominal value | Interpretation |
| --- | ---: | --- |
| Ring to tiles, radial | 0.60 | Outer running gap |
| Ring neck to shell, radial | 0.20 | Printer-sensitive centring fit |
| Ring bore to Cap, radial | 0.30 | Rotation fit |
| Tile pin to bore, radial | 0.15 | Unchanged printer-sensitive guide fit |
| Ring internal ledge to Cap hooks, axial | 0.18 | Hidden capture; no separate keeper |
| Trial central-switch click stroke | 0.35 | Typical TS13 travel; actual switch must be measured |
| Central bridge stop | 0.45 | Prototype setting; not all tolerance extremes are covered |
| Bridge top to display rear keepout | 2.64 | Rest clearance, grows on a straight press |
| Glass to Cap underside | 1.80 | 1.35 at the central stop |
| Ring shoulder above fixed deck | 1.20 | 0.75 at the central stop |
| Ring gear shoulder over stationary pinion teeth | 1.10 | 0.65 at the central stop |
| Pinion over encoder bushing | 1.10 | Encoder is rotation only; pinion does not intentionally depress |
| Retainer to fixed deck | 0.20 | Grows to 0.65 on a straight press |
| Fixed flexure snap head to gear bottom | 1.50 | 1.05 at full straight press; tilted cases also checked |
| Four flexure leaves | 0.80 thick, 2.40 wide | Folded layout; elastic response unverified |
| Bridge feet to carrier top | 4.20 | Flexures join the feet; no sliding guide fit |

The retainer now uses three upward snap tongues instead of screws. Nothing projects below
its former flat underside. Their pockets stay outside the gear band. Full-turn envelopes
cover the original annulus and new taller clips through the trial press stroke. The Ring neck
has localized thin walls around the pockets; print quality, insertion force, retention strength
and practical release need testing. A nominal 0.5 mm probe can approach each release opening
locally; this does not establish access through a fully assembled case.

## Why the old approach changed

The original remote sleeve shoulders could stop a 0.25-degree tilted rear press at about
0.407 mm shaft depression, before the assumed 0.5 mm encoder click. Moving the stops beside
the encoder improved that nominal sampled clearance but did not synchronize the ring edges.
The user then confirmed a 17 mm shaft projection above the carrier, replacing the earlier
19.5 mm assumption. Extending the nub and shortening the hub corrected the contact height.
The user subsequently reported that only the encoder-side edge clicked reliably. That physical
failure supersedes clearance-only expectations and is why the encoder push load path is removed.
The 17 mm shaft projection and shortened pinion remain for rotation.

## What the trial does and does not prove

The ring is captured on the moving Cap. Four Cap columns snap into blind sockets in the separate flat-printable
bridge. Four folded leaves support the bridge at four quadrants, and a central nub contacts
the fifth switch. The old loose sliding sleeves and encoder-contact nub are absent.

Sampled CAD checks cover rest assembly, central-switch contact through the trial stroke,
stationary gears, ring/frame tilt, independent ring rocking up to first hidden-hook contact,
actual snap pillars, moving flexure pads, new retainer clips, conservative vertical leaf envelopes,
module dimensions and the documented insertion order. They do not solve the loaded elastic
shape. Four simple folded leaves do not enforce perfectly parallel movement. Their stiffness,
carrier deflection, fatigue, creep, clamp fit, print anisotropy and the force at each edge must
be measured on the prototype.

The TS13 PDF gives an actuator-travel range reaching 0.45 mm. The stop is set for a nominal
0.35 mm trial, with only 0.10 mm axial reserve. At 0.25-degree tilt the first central stop can
engage near 0.394 mm. A switch needing 0.45 mm may therefore fail to click at an edge. Measure
and tune the actual switch/stop combination; do not claim compatibility across that whole range.

## Display correction and remaining physical checks

- The display flex-tail keepout is still an assumed 14 mm wide. Verify the actual module.
- The original Ø33 window clipped the display edge. The corrected Ø35 opening with a full
  2 mm flare passes sampled sight lines at 0/10/20 degrees in eight directions, with 0.5 mm
  optical depth behind the glass and 0.15 mm centring allowance. Wider views remain unchecked.
- Printed tile and neck fits remain sensitive to printer error. Enlarging every fit adds play.
- Current firmware still requires the GC9A01 display conversion and B4 support. The ring-click
  signal remains GPIO5 with the existing input pull-up, now wired to the central switch.
- The new centre-switch holes require updated carrier wiring: 21 runs, 39 Kapton crossings,
  48 pin annotations. Do not use the previous carrier STL or wiring labels for this revision.

See [PRESS-PROTOTYPE.md](PRESS-PROTOTYPE.md) for assembly and physical acceptance.

The shell originally cleared the SuperMini only when seated: its rear wall hit the board end
while lowering. The internal USB relief now extends to the bottom opening. This retains the
0.8 mm outer skin and clears the sampled insertion path; inspect that thin local wall on the print.

The Cap face has no mounting holes. Shouldered split pillars replace the fixed and moving
flexure screws and Cap-to-bridge screws. Snap insertion checks use displaced halves/hooks as
clearance surrogates; they do not establish strain, force, clamping repeatability or release
access through the closed case. See PRESS-PROTOTYPE.md before snapping the complete trial together.
