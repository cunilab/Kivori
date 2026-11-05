# Kivori enclosure v7 — Terrace prototype

V7 uses the black and white Terrace exterior, six pillars and curved snap-on display cowl. It places the RP2040-Zero on the **underside of the carrier** with its components facing the bottom cover. The sole external 5 mm common-cathode RGB LED sits directly on the carrier, hidden beneath the knob and offset 10 mm from the shaft. The shaft and nut occupy the exact center. A sloped white mixing insert and a hidden white roof sit beneath the visible outer diffuser ring.

## Previews

- `preview-exterior.png` — assembled exterior.
- `preview-front-wiring.png` — carrier front face with every wire labeled by net (RP2040 pin to its destination).
- `preview-rear-wiring.png` — carrier rear face with every wire labeled by net, plus the tape strips.
- `preview-led-resistors.png` — labeled close-up of the three LED resistors and their GPIO wires.

## Test before printing the full carrier

These nominal model margins are small; check each with the real parts and your PETG settings:

| Area | Model value | Check |
|---|---|---|
| RP2040 pin-only hold | 0.90 mm hole (`RP_PIN_FIT_D`) | Print `PinFitCoupon.stl`; test USB plugging gently |
| Plastic between front and rear wire channels | 0.80 mm | Slice and inspect |
| Plastic between a display transfer hole and its pin hole | 0.90 mm | Slice and inspect |
| Plastic between LED lead holes | 0.47 mm (rear fan grooves 0.52 mm) | Slice; test-fit the real LED |
| LED to encoder | 0.53 mm | Measure the real LED and encoder |
| White liner to black bowl | about 0.07 mm normal to the slope | Test-fit the printed liner |
| LED light quality | not simulated | Small LED/cavity test |

The LED opening and mixing insert are intended for a physical lighting test. One LED can still produce an uneven, dim or visibly colored hotspot; no optical simulation or illumination test has been performed. Test the LED with the white cavity and roof first; neither uniform light nor brightness is guaranteed. White PETG and the hidden roof can reduce output. The LED's 8.7 mm height, 5.8 mm flange diameter and lead order are assumptions: measure and identify your actual part first. The RP2040's built-in LED is a separate onboard component.

## Pin-only RP2040 mount

The board is held solely by its soldered male GPIO pins in the printed carrier holes, with no RP2040 retainer, strap or socket. The provisional `RP_PIN_FIT_D = 0.90` mm is a CAD value, **not a verified PETG press fit**. Before printing the carrier, print `PinFitCoupon.stl` in the same orientation and settings. Its five rows of nine holes have one to five engraved dots: 1 = 0.80, 2 = 0.85, 3 = 0.90, 4 = 0.95, 5 = 1.00 mm. Each hole has a short groove stub matching the carrier face. Test the actual header strip without forcing it; change `RP_PIN_FIT_D` in `build_case.py` and rebuild if needed.

A coupon cannot establish the assembled board's resistance to repeated USB insertion and removal. Test the assembled pin fit and USB load gently before relying on it; the board has no secondary retention. Do not power an unstable assembly.

Install the following GPIO pins only; leave the listed unused metal pins out of the plastic header strips so the printed holes and wire paths match the model:

- Left row: **GPIO3, GPIO4, GPIO5, GPIO6, GPIO7, GPIO8**.
- Right row: **GND, 3V3, GPIO26, GPIO15, GPIO14**.
- Bottom row: **GPIO13, GPIO12, GPIO11, GPIO10, GPIO9**.
- Remove/omit **GPIO0, GPIO1, GPIO2, 5V, GPIO29, GPIO28, GPIO27**. The board's built-in GPIO16 LED is unchanged.

Solder the required male pins to the board before inserting them into the carrier. The controller is rotated physically: its component-side left and right exchange sides when viewed from the carrier front. Use the printed board labels to identify the pins. The pins point through the carrier and can be connected on its front side.

## Wiring and reset

The model uses **0.3 mm outside-diameter enamel-insulated wire**, seated in 0.75 mm wide channels on both carrier faces. Short, supported transfer holes link faces. No wire crosses another on either face, so no Kapton is needed. Five wider removable tape strips in the model illustrate how to retain groups of wires. Route wires into the grooves before pressing the RP2040 into place. The opposite-face model leaves a nominal minimum **0.8 mm printed web**; slice and inspect this thin area. Thicker insulated wire requires wider channels and a fresh clearance check.

The board's pins are grouped by what they connect to: the left row is the encoder and keys, the right row is the display, and the bottom row is the RGB LED. Nothing else shares those groups, so each group can be wired as one ribbon.

Each of the seven display wires has its own **1.0 mm transfer hole 2.0 mm to the left of its display header pin** (the BLK hole sits lower, at y = 68.9, so the DC wire can pass between it and the RP2040 pins). The wire runs on the carrier front from an RP2040 pin to this hole, descends through it, and on the rear runs a short hop to the display pin, which sits alone in its own 1.2 mm hole. Wrap the wire around the pin tip on the carrier underside. The web between a transfer hole and its pin hole is 0.90 mm. Keep enamel intact inside the holes; strip it only at the pin wraps and check isolation between pins.

The LED flange seats on the carrier front without a separate keeper. Its four nominal 0.5 mm leads pass through four **0.8 mm holes at 1.27 mm pitch**. The adjacent holes leave only **0.47 mm nominal web**; the rear fan grooves have about 0.52 mm minimum web. Slice and test this small area with the real LED and PETG settings. Insert the leads straight through, bend them gently into their separate rear grooves, make and insulate the correct connections, and only then trim excess lead length. Avoid heating the printed carrier when soldering. The model's closest LED-to-encoder clearance is 0.53 mm and its flange-to-nut clearance is 1.90 mm; real parts must be checked. If your LED's common-cathode leg is not in the modeled position, adjust the wiring/model rather than crossing uninsulated leads.

Complete the LED lead fan and its connections before installing the encoder module, whose PCB covers part of this wiring area.

There are three separate LED series resistors, one for each color. Each resistor's output lead ends close to its LED lead: 2.69 mm for red and 1.69 mm for green and blue, measured in the model from the resistor output hole to the end of the LED lead groove. The red resistor sits 1 mm farther from the LED to clear the shell. Each LED signal (GPIO9 red, GPIO10 green, GPIO11 blue) runs on the carrier front face straight down from its pin to a transfer hole, then on the rear to its resistor input. The rear runs are nested so they do not cross. The modeled axial footprint is 3.2 × 1.8 mm body with 7.62 mm lead pitch. Identify the LED's common cathode and each color on the actual part. Three individual 1 kΩ resistors at 3.3 V are a conservative starting point for low-current testing; choose final values for the actual LED and desired brightness. Never connect a color lead directly to a GPIO without its resistor. Remove enamel only at intended connections and check continuity and isolation before power.

| RP2040-Zero | Function |
|---|---|
| GPIO3 / GPIO4 / GPIO5 | Encoder SW / DT / CLK |
| GPIO6 / GPIO7 / GPIO8 | Left / middle / right key |
| GPIO26 | LCD SCL (hardware SPI1 SCK) |
| GPIO15 | LCD SDA (hardware SPI1 TX) |
| GPIO14 / GPIO13 / GPIO12 | LCD RES / DC / BLK |
| GPIO9 / GPIO10 / GPIO11 | RGB LED red / green / blue, through individual resistors |
| 3V3 | Display VCC and encoder + |
| GND | Display, encoder, keys and RGB common cathode |

Each key's two lower legs (12.5 mm apart) are internally connected; the ground chain relies on this — check with a multimeter that the switch is not rotated 90°.

No RP2040 firmware exists in the repo yet (the repo firmware targets the ESP32-C3). Implement the pin map above: SPI1 with SCK on GPIO26 and TX on GPIO15 ([Raspberry Pi's RP2040 datasheet, Table 3](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf)), RES, DC and BLK as plain outputs, and pull-ups on the encoder and key inputs.

## Wiring order

1. **Left ribbon (encoder and keys).** On the rear, ground first: GND to the left key's lower-left leg and to the encoder GND pin. Then the encoder: 3V3 to encoder +, GPIO3 to SW, GPIO4 to DT, GPIO5 to CLK. Then the keys: GPIO6, GPIO7 and GPIO8 to the upper-right leg of the left, middle and right key. Their runs nest, so the later key turns innermost. Link the keys' ground chain (left lower-right to middle lower-left, middle lower-right to right lower-left).
2. **Right ribbon (display).** On the front, run GPIO26, GPIO15, GPIO14, GPIO13 and GPIO12 to the transfer hole beside the display SCL, SDA, RES, DC and BLK pins, and GND and 3V3 to the GND and VCC holes. On the rear, link each transfer hole to its display pin and wrap the pin.
3. **Bottom ribbon (LED).** Front: GPIO9, GPIO10 and GPIO11 straight down to their transfer holes. Rear: each to its resistor input (red, green, blue, nested). Then each resistor output to its LED lead, and the LED common cathode to ground.
4. **Power.** The GND wires go through the rear via beside the GND pin to the switch, encoder and display. The 3V3 wire runs on the front along the row of the removed GPIO2 pin to a hole beside the encoder, then on the rear to encoder +. Check 3V3 to GND for a short before plugging in USB.

A **1.4 mm reset access hole in the bottom cover** is aligned to the flipped board's RESET button; its internal guide ends 0.6 mm before the modeled button. Use a slender blunt nonconductive probe and press gently. BOOT remains accessible with the case open. Verify alignment and travel with the real board before fitting the cover.

## PETG printing

The export folder contains individually oriented STLs. Start with a 0.4 mm nozzle, 0.20 mm layers and your normal PETG profile on textured PEI. Slice and inspect the two-sided carrier channels, thin inter-face web, LED lead holes, sloped mixing liner, cowl clips and reset guide. The liner has a 0.8 mm-wide print-bed contact ring. Its nominal 0.1 mm vertical separation from the black bowl is only about 0.07 mm normal to the slope; test the white liner in the shell and adjust the fit for your printer. Print the fit coupon and a small LED/cavity test first, then test one keycap, the reset guide and USB plug with the actual hardware.

| Black | White |
|---|---|
| Shell, DisplayCowl, Carrier, Knob, PinFitCoupon | BackPlate, DisplayBezel, KnobCap, CapLeft, CapMiddle, CapRight, HaloDiffuser, HaloReflector, ScrewFeet |

## Rebuild

With FreeCAD 1.1:

```sh
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-v7/build_case.py
```

The script checks modeled fit and exports the FreeCAD assembly, individually oriented STLs and carrier-outline DXF. Electronics, wire and tape are assembly references rather than printed parts. This remains an unprinted prototype: check physical fit, retention, operation, clip durability and LED appearance before relying on it.
