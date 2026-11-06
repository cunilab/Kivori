# Kivori assembly (one beta unit)

This draft covers the copper-tape printed-PCB variant (`COPPER_TRACES = True`) developed on
`worktree-enclosure-freecad`. Its steps require that variant's matching CAD/exports and the wiring
specification in [`profile.rs`](../firmware/pip/src/profile.rs).

The current enclosure is [`hardware/enclosure-v7`](../hardware/enclosure-v7/README.md): the
RP2040-Zero is held by its soldered pins on the underside of the carrier and wired by hand with
enamel wire, with no screws, `Spacers` or `RubberDots`. Its README is the reference for mounting and
wiring; the pin map below applies to both. Select matching instructions and exported parts before assembling. Record the exact
variant and physical measurements in the [M2.9 enclosure session](./validation.md#m29-assembled-enclosure-acceptance-open).
The [BOM](./bom.md) also describes the hand-wired design; copper-specific materials are listed below.
Related: [validation.md](./validation.md). There is no per-unit provisioning ([provisioning.md](./provisioning.md)).

Write the unit's short hash (step 20) on a label inside the plinth.

## What you need

Per unit:

| Item | Qty | Notes |
|---|---:|---|
| Waveshare RP2040-Zero | 1 | Male header pins on the GPIOs in step 15 only (step 1) |
| 5 mm common-cathode RGB LED, 1 kΩ resistor | 1, 3 | One resistor per colour |
| ST7789 1.3" 240×240, 7-pin | 1 | |
| HW-040 rotary encoder | 1 | Nut not used |
| 12×12 mm push switch, 10 mm tall | 3 | |
| Printed parts | 1 set | `Shell`, `Visor`, `BackPlate`, `Carrier`, `Knob`, `CapLeft`, `CapMiddle`, `CapRight`, `RubberDots`, `Spacers` |
| M2 × 12 self-tapping screw, pan head up to Ø4 | 6 | 4 close the case, 2 hold the HW-040 |
| Copper tape | enough to cover the `Carrier` back | Wide tape means fewer seams; conductive adhesive is not needed, every joint is soldered |
| Kapton (polyimide) tape | a few cm | The two crossovers |
| Low-temperature solder, Sn42Bi58 | | About 140 °C; PETG softens under a hotter iron |
| USB-C **data** cable | 1 | Not charge-only |

Tools: soldering iron with a fine tip, a fresh craft blade, a multimeter with a continuity beeper,
flush cutters, tweezers, a needle or SIM pin, and a computer with the Kivori app (or a repo
checkout for `just fw-flash`).

## A · Before you start

1. **Check the board.** It must be the Waveshare RP2040-Zero (USB-C, BOOT and RESET buttons) with
   the pins in step 15 available. A clone with another pinout does not match the wiring: put it aside.
2. **Print the parts** (settings in the enclosure README). The `Carrier` must be **PETG** (PLA
   softens under the iron), the `BackPlate` PETG, the `RubberDots` TPU, the `Spacers` with a brim.
3. **Test-fit the `Carrier` first** on the very first unit of a new print: press the display, a
   switch and the RP2040-Zero on. If a peg or hole misses, fix the parameter before printing the rest.

## B · Copper traces (back of the `Carrier`)

4. Lay copper tape flat over the back and rub it down until the gaps show through.
5. Run the blade along every gap and peel off the copper that is not a trace or a pad. Make one
   short cut between each pair of neighbouring pins in the pin rows (RP2040-Zero, display, encoder).
   Leave out the upper traces at the two crossovers for now.
6. Stick a piece of Kapton over each crossover (the amber patches in the model): SCL and SDA over
   BLK, GND and VCC next to the display; B1 and B2 over the display bundle next to the RP2040-Zero.
   Lay the upper traces' copper across it and trim it to the trace width.
7. **Check:** with the multimeter, no trace beeps against its neighbour, and each crossover's upper
   trace does not beep against the trace under it.

## C · Fit and solder

8. Fit the display on its four standoffs from the front, header through the slot.
9. Put the three switches through from the front, bend their legs over onto their pads.
10. Fit the RP2040-Zero on its soldered header pins as the enclosure README for your variant shows
    (v7: components towards the bottom cover, USB-C at the far edge). Its pins come through to the
    wiring.
11. Solder every pin on the back with quick touches.
12. Screw the HW-040 on from its back: an M2 × 12 through each hole and `Spacer` into the boss.
    Bend its five pins up 90° right at the plastic so they come up through their pads, then solder
    them.
13. Optionally heat-stake the display peg tips (touch each with the iron until it forms a small
    head).

## D · Check before power

Do this with the USB cable unplugged.

14. **No short:** 3V3 to GND does not beep. 5V to GND does not beep. 5V is not connected to
    anything but the board.
15. **Wiring:** each RP2040-Zero pin beeps to the right part, and to nothing else:

    | RP2040-Zero | Goes to |
    |---|---|
    | GP26 / GP15 | display SCL / SDA |
    | GP13 / GP14 | display DC / RES |
    | GP12 | display BLK |
    | 3V3 | display VCC, encoder + |
    | GND | display GND, encoder GND, one leg of each switch, LED common cathode |
    | GP5 / GP4 / GP3 | encoder CLK / DT / SW |
    | GP6 / GP7 / GP8 | left / middle / right switch |
    | GP9 / GP10 / GP11 | LED red / green / blue, each through its 1 kΩ resistor |

16. **Switches:** each switch beeps between its pin and GND only while pressed.

## E · Flash and check (case still open)

Testing before closing means a fault is still easy to reach.

17. Hold the **BOOT** button and plug the bare assembly into the computer with the data cable. Unplug
    any other Kivori. A drive named `RPI-RP2` appears.
18. Flash the firmware: open the Kivori app and use **Restore with the BOOT button** on the Device page, or
    run `just fw-flash` from a checkout, or copy a `.uf2` onto the drive (on macOS `cp -X`; plain `cp`
    complains about extended attributes but still writes the file). The board restarts on its own and
    the `RPI-RP2` drive disappears: that is success. The drive is only the bootloader; a running
    firmware shows up as a serial port instead.
19. **Check:** the display lights and shows the buddy and the app shows the unit as connected. If not, fix what is wrong (step 15 is the
    usual suspect) and flash again.
20. Note the short hash the app shows (Diagnostics) on a label for the inside of the plinth.

## F · Close the case

21. Drop the three caps into the face openings from inside the shell, flange first.
22. Lower the `Carrier` over the four spigots and press it onto the posts: each switch clicks into
    its cap.
23. Press the back plate straight in, the encoder shaft through the face hole, until all four hooks
    click. Drive the four case screws in from under the plinth until snug; do not overtighten.
24. Press the four TPU dots on, one plug into each screw hole.
25. Press the visor's three pegs into the face; a drop of CA glue or thin double-sided tape keeps
    it there.
26. Push the knob onto the D-shaft, line marker up.

## G · Final check

27. Plug in again and open the Kivori app. Turn the knob both ways, press it, press the three
    buttons left to right: each does what its label says, and no button sticks.
28. Verify the RST access location against the selected copper-variant CAD and actual board before
    inserting a tool. Operate RST through that verified access point: the unit restarts and reconnects.
29. Wipe the visor, pack the unit with a data cable.

To open a closed unit: pull the four dots off, take the screws out, grip the plinth in the shadow
gap and pull.
