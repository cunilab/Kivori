# Kivori enclosure v6 — RP2040-Zero

V6 adapts [v5's Visor Dashboard](../enclosure-v5/README.md) to the **Waveshare RP2040-Zero**, with the **whole HW-040 encoder module rotated 180° around its stationary shaft**. The five-pin encoder header faces the left wall. The knob, display, buttons, shell size and center clamp retain v5's positions; the larger YD-RP2040 layout has been replaced.

## Board and mounting

The board is **18 × 23.5 mm**, matching the supplied dimension drawing and [Waveshare's drawing](https://www.waveshare.com/w/upload/f/f4/900px-RP2040-Zero-details-size.jpg). Pin centers use 2.54 mm pitch, 15.24 mm between the two side rows, and a 1.59 mm offset from the USB end. All **23 edge pins** have carrier holes: two nine-pin strips and the five middle bottom pins. The corner GPIO14/GPIO8 holes are shared with the side rows, so they are not drilled twice. The bottom five-pin strip is GPIO13,12,11,10,9 from left to right, seen from the USB/button side with USB at the top.

USB and buttons face the case face. Ordinary 2.5 mm header plastic rests directly on the carrier; extra spacers are unnecessary. The RP2040 chip, crystal and regulator are on the board's **back**, facing the carrier; their envelopes are included in the collision model. USB, reset-button and backside-component dimensions come from [Waveshare's official STEP](https://files.waveshare.com/upload/f/f7/RP2040_Zero_stp.zip) and should still be checked against the purchased board. The USB opening is 10.4 × 4.8 mm. The needle guide reaches RESET; BOOT is accessed with the case open.

Sources: [official wiki and resources](https://www.waveshare.com/wiki/RP2040-Zero), [schematic](https://files.waveshare.com/upload/4/4c/RP2040_Zero.pdf). GPIO16 drives the onboard RGB LED and is not used here. Backside-only GPIO pads are unused.

## Proposed firmware pin map

Assignments are chosen for short, orderly wire routes: encoder signals fan left, display signals fan right, and bottom-row GPIOs feed the buttons. The display uses **hardware SPI0**: [RP2040 GPIO functions](https://datasheets.raspberrypi.org/rp2040/rp2040_datasheet.pdf) assign SPI0 SCK to GPIO2 and TX/MOSI to GPIO3. The existing ESP32 firmware has not been changed; implement this map in your RP2040 firmware.

| RP2040-Zero pin | Connection |
|---|---|
| GPIO2 | Display SCL / SPI0 SCK |
| GPIO3 | Display SDA / SPI0 MOSI |
| GPIO4 | Display RES |
| GPIO5 | Display DC |
| GPIO6 | Display BLK / backlight |
| GPIO27 | Encoder CLK |
| GPIO28 | Encoder DT |
| GPIO29 | Encoder SW |
| GPIO9 / GPIO10 / GPIO11 | Left / middle / right switch |
| 3V3 | Encoder +, then display VCC; never use 5V for these |
| GND | Encoder GND, then switch ground chain and display GND |

With the encoder rotated, its pins run **CLK, DT, SW, +, GND from the near edge toward the far/USB edge**. Their physical order reverses; their electrical labels do not. Follow the labels in the FreeCAD document rather than mirroring the wiring by eye.

The channels use 0.3 mm enamelled wire and insulated crossings as in v5. `Dummy_Wire`, `Dummy_Kapton` and `PinLabels` show the wiring. Both supply chains branch at the encoder pins; leave room for both wrapped ends. V5's display long-pin header, nut mounting, screw/foot assembly, print orientations and other nominal dimensions still apply.

## Build

```sh
/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd hardware/enclosure-v6/build_case.py
```

The script checks interference, wire ridges, underside/overhangs, valid printed shapes, board clearances and left-button travel before exporting `kivori-case-v6.FCStd`, nine print-oriented STLs and `export/pcb-outline.dxf`. Exterior and internal previews are `preview-exterior.png` and `preview-interior.png`.

This is an unprinted prototype. Fit-test the actual board, header spacing, USB/cable seating, reset alignment and encoder clearances before a full case print. Firmware and physical fit have not been tested.
