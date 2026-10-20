# Kivori assembly (one beta unit)

How to build one Kivori, from parts to a provisioned, closed unit. It follows the copper-tape
printed PCB (enclosure prototype v3, `COPPER_TRACES = True`) and the firmware pin map from #67.
The geometry, print settings and why each part is held the way it is are in
[hardware/enclosure/README.md](../hardware/enclosure/README.md); this page is only the order of work
and the checks between steps. Related: [bom.md](./bom.md), [provisioning.md](./provisioning.md),
[validation.md](./validation.md).

> The v3 enclosure files (copper traces, `Spacers`, `RubberDots`, screws) are on the branch
> `worktree-enclosure-freecad` until it is merged. `hardware/enclosure/` on `main` is still the
> older hand-wired, screwless version.

Write the unit's short hash (step 20) on a label inside the plinth.

## What you need

Per unit:

| Item | Qty | Notes |
|---|---:|---|
| ESP32-C3 SuperMini | 1 | No 32.768 kHz crystal (step 1) |
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
flush cutters, tweezers, a needle or SIM pin, and a computer with a repo checkout for
`just provision`.

## A · Before you start

1. **Check the SuperMini.** Look at the board next to GPIO0/GPIO1. If a small metal-can or
   two-pad 32.768 kHz crystal is fitted, put that board aside: on it, the display's BLK and DC pins
   do not work (validation 5.1).
2. **Print the parts** (settings in the enclosure README). The `Carrier` must be **PETG** (PLA
   softens under the iron), the `BackPlate` PETG, the `RubberDots` TPU, the `Spacers` with a brim.
3. **Test-fit the `Carrier` first** on the very first unit of a new print: press the display, a
   switch and the SuperMini on. If a peg or hole misses, fix the parameter before printing the rest.

## B · Copper traces (back of the `Carrier`)

4. Lay copper tape flat over the back and rub it down until the gaps show through.
5. Run the blade along every gap and peel off the copper that is not a trace or a pad. Make one
   short cut between each pair of neighbouring pins in the pin rows (SuperMini, display, encoder).
   Leave out the upper traces at the two crossovers for now.
6. Stick a piece of Kapton over each crossover (the amber patches in the model): SCL and SDA over
   BLK, GND and VCC next to the display; B1 and B2 over the display bundle next to the SuperMini.
   Lay the upper traces' copper across it and trim it to the trace width.
7. **Check:** with the multimeter, no trace beeps against its neighbour, and each crossover's upper
   trace does not beep against the trace under it.

## C · Fit and solder

8. Fit the display on its four standoffs from the front, header through the slot.
9. Put the three switches through from the front, bend their legs over onto their pads.
10. Stand the SuperMini on the front on its soldered header strips, components towards the face,
    USB-C at the far edge. Its pins come through to the copper.
11. Solder every pin on the back with quick touches.
12. Screw the HW-040 on from its back: an M2 × 12 through each hole and `Spacer` into the boss.
    Bend its five pins up 90° right at the plastic so they come up through their pads, then solder
    them.
13. Optionally heat-stake the display peg tips (touch each with the iron until it forms a small
    head).

## D · Check before power

Do this with the USB cable unplugged.

14. **No short:** 3V3 to GND does not beep. 5V to GND does not beep. 5V is not connected to
    anything but the SuperMini.
15. **Wiring:** each SuperMini pin beeps to the right part, and to nothing else:

    | SuperMini | Goes to |
    |---|---|
    | IO4 / IO3 | display SCL / SDA |
    | IO1 / IO2 | display DC / RES |
    | IO0 | display BLK |
    | 3V3 | display VCC, encoder + |
    | GND | display GND, encoder GND, one leg of each switch |
    | IO7 / IO6 / IO5 | encoder CLK / DT / SW |
    | IO9 / IO8 / IO10 | left / middle / right switch |

16. **Switches:** each switch beeps between its pin and GND only while pressed.

## E · Provision (case still open)

Testing before closing means a fault is still easy to reach.

17. Plug the bare assembly into the computer with the data cable. Unplug any other Kivori.
18. Run `just provision` and follow the prompts, turning the encoder shaft by hand: right 3, left
    3, press, hold, then the left, middle and right switches ([provisioning.md](./provisioning.md)).
19. **Check:** the display lights and shows the buddy, and the tool prints **PASS**. On FAIL, fix
    what it names (step 15 is the usual suspect) and run it again.
20. Note the short hash it prints on a label for the inside of the plinth.

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
28. Push a needle through the RST hole in the visor: the unit restarts and reconnects.
29. Wipe the visor, pack the unit with a data cable.

To open a closed unit: pull the four dots off, take the screws out, grip the plinth in the shadow
gap and pull.
