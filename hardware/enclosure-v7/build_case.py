"""Kivori enclosure v7, "Terrace" (centered controls, raised separate display cowl): parametric build script (FreeCAD 1.1, Part workbench, no GUI needed).

Run it inside FreeCAD (GUI Python console, the FreeCAD MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-v7/build_case.py"
    exec(open(p).read(), {"__file__": p, "__name__": "__main__"})

Every dimension lives in the parameter block below. Change a number, run again, the document is
rebuilt from scratch, re-checked for interference and re-exported.

Frame used everywhere (the "face frame"):
  * the outer face surface is z = 0, the case body is at z < 0 (z = depth behind the face),
  * x runs 0..W from left to right as you look at the face,
  * y runs 0..H from the near edge (user side, thin end of the wedge) to the far edge (USB side).

Values marked NOMINAL come from datasheets, KiCad footprints or seller listings, not from a
measured part. Fix them here after the fit-test print.
"""

import math
import os

import FreeCAD as App
import Part

V = App.Vector

# ---------------------------------------------------------------------------------------------
# Parameters
# ---------------------------------------------------------------------------------------------

# Terrace: black flat-faced shell, separate raised black cowl, white bezel/cap/keys/base.
# The measured hardware stack sets thickness; face width/depth are compact layout targets.
W, H = 86.0, 95.0          # face outline (outer edge of the bevel)
R_CORNER = 8.0              # face corner radius
T_NEAR, T_FAR = 18.8, 30.8  # case thickness at the near (y=0) and far (y=H) edge -> ~9.7° wedge (was 20.7 / 32.7: the
                            # back plate now has a window under the HW-040, ENC_RELIEF, which let it come up 1.9 mm)
WALL = 2.0                  # side wall thickness (horizontal)
WALL_TAPER = 6.0            # degrees the walls lean in towards the face (keycap draft)
FACE_T = 2.5                # faceplate thickness (room for the visor pocket in front)
FACE_BEVEL = 2.0            # bevel round the face (inset at the face); a true 45° even with the
                            # wall lean, so it prints face down without overhang
BP_T = 2.0                  # back plate thickness
BP_CLR = 0.2                # back plate to wall clearance (per side)

# Knob + encoder (EC11 dimensions from KiCad RotaryEncoder_Alps_EC11E-Switch_Vertical_H20mm)
KNOB_C = (43.0, 40.0)       # knob / shaft centre on the face
KNOB_D = 36.0               # height comes from the shaft: KNOB_H further down
KNOB_GAP = 1.8              # air gap between knob skirt and face (push travel is ~0.5; at 1.0 a press hit the face)
KNOB_TOP_T = 4.5            # knob top above the shaft-tip clearance: sets the knob 10.5 tall
KNOB_TIP_CLR = 1.0          # free space between the shaft tip and the bore's ceiling
KNOB_WALL = 2.5             # hollow knob: skirt wall (the grip flutes cut 0.6 into it)
KNOB_HUB_D = 12.0           # hub round the shaft, hanging from the top, tied to the skirt by ribs
KNOB_RIBS, KNOB_RIB_T = 4, 1.2
KNOB_FLUTES, KNOB_FLUTE_D = 32, 2.0   # fine vertical grip ribs round the skirt
KNOB_CHAMFER = 1.2          # 45° chamfer round the top edge (prints on the bed)
KNOB_HALO = None            # (r in, r out, depth) groove ring round the knob: off, it ran into the
                            # face's left bevel and left slivers too thin to print
EC11_BODY = (12.0, 11.6)    # NOMINAL body footprint
# Heights above the HW-040 board front (the encoder side): body top and shaft tip measured by the user
# on the real module, 2026-10-31; the bushing length was not re-measured
EC11_BODY_H = 6.5           # body ("box") top (bushing base), measured by the user on the real module, 2026-10-31
EC11_BUSH_D, EC11_BUSH_L = 7.0, 8.5   # M7 bushing: thread ends 15.0 above the board
EC11_SHAFT_D, EC11_SHAFT_H = 6.0, 26.0  # D shaft, tip 26.0 above the board (measured by the user on the real
                                        # module, 2026-10-31)
EC11_FLAT = 1.5             # D-flat depth (4.8 mm across the flat, measured)
EC11_FLAT_L = 7.1           # D-flat length from the tip; round below that (measured)
ENC_HOLE_D = 8.6            # face hole round the shaft and the bushing tip (0.8 play: the encoder
                            # hangs on the back plate, not the shell)
# HW-040 module, in its own frame: origin at the board centre, +x towards the header end,
# -y towards the long edge carrying the two mounting holes. Board size from a dimensioned drawing
# (26 x 18.5 x 1.3, shaft top 21 mm); shaft and holes measured on the real module (photo next
# to a ruler, scaled by the 2.54 mm header pitch), about ±0.3 mm.
HW040 = (26.0, 18.5, 1.3)   # board length, width, thickness
HW040_SHAFT = (-2.3, 1.8)   # shaft centre
HW040_HOLES = [(-8.55, -6.9), (8.55, -6.9)]  # two M3 holes along the long edge, 17.1 mm apart, centred
HW040_HOLE_D = 3.2
HW040_HDR_X = 10.25         # header pin row (measured on the module: 0.1 nearer the shaft than drawn)
HW040_BACK_KEEPOUT = (-9.0, 9.0, -4.0, 6.0)  # shaft-relative: EC11 legs + 3 pull-up resistors underneath
HW040_BACK_PARTS = 2.0      # how far those (and the header joints) stick out of its back
# The HW-040 hangs behind the printed PCB from the EC11 itself, mounted like a panel encoder: its
# body sits flat against the printed PCB's back, the threaded bushing goes through a round hole, and
# the EC11's own M7 nut (with its washer) on the front clamps it. A knob press goes into the printed
# PCB and on into the face posts; the back plate does not touch the encoder. Deep in the wedge, the
# long shaft ends low and the knob stays short.
HW040_FRONT = -18.7         # board front: the EC11 body (6.5 tall) flat against the printed PCB's back, so
                            # the bushing tip and the nut clamp stay where they were (C_BACK - EC11_BODY_H)
EC11_NUT = (9.0, 2.0, 0.5)  # M7 nut: across flats, thickness; washer thickness (NOMINAL, check yours)
# Its five header pins stand straight up (straightened, or a straight header) into holes in the
# printed PCB; the wire is wound round them in the gap between the two boards.
HW040_PIN_BEND = 0.0        # straight pins, up from the header row
HW040_PIN_L = 2.5 + 6.0     # straight header: plastic + pin past it, from the board front (NOMINAL)
EC11_HOLE_CLR = 0.3         # printed PCB round hole round the EC11 bushing, per side (it also centres the encoder)

# Display: ST7789 1.3" 240x240, 7-pin module, mounted rotated 90° (header on the right)
DISP_C = (43.0, 78.0)      # centre of the window on the face (board centre stays at x 78.5)
DISP_ACTIVE = 23.4          # active area (square), several sources agree
DISP_WIN_MARGIN = 0.3       # window opening beyond the active area, per side
# Module frame: u runs along the 39.22 mm length, + towards the header end. Glass and visible
# area measured on the real module (photo next to a ruler, about ±0.5 mm).
DISP_PCB = (39.22, 27.78, 1.6)    # module PCB (length, width, thickness); 3.0 total with glass
DISP_GLASS = (28.0, 26.3, 1.4)    # glass incl. the FPC ledge at the end away from the header (measured)
DISP_ACTIVE_U = 1.85              # visible-area centre from the board centre, along u (measured)
DISP_GLASS_U = 0.8                # glass centre from the board centre, along u (measured)
DISP_HDR_U = 2.2                  # 7-pin header row, from the header end
DISP_HDR_V = 0.1                  # header row centre past the board centre, towards the far (USB) edge (measured)
DISP_POCKET = 0.6           # glass pocket depth into the back of the faceplate
DISP_FPC_RELIEF = (1.2, 0.5)  # relief in the face back over the LCD's flex cable (it leaves the glass at the
                            # end away from the header and bends down onto the board): depth, margin past
                            # the board edge. Without it the face pinched the flex (1.3 mm of air there)
DISP_FACE_GAP = 0.5         # air in front of the glass: the module hangs on the standoffs and pegs only,
                            # never clamped between them and the face (with no gap the screws cracked it)
DISP_HEADER_SIDE = +1       # +1 header on the right end, -1 on the left end
DISP_HOLE_D = 2.0          # four corner holes, NOMINAL ("mounting holes: 2 mm")
DISP_HOLE_INSET = 2.5      # hole centres from both edges at each corner (measured: 34.2 x 22.8 apart)
DISP_STANDOFF_D = 4.5
DISP_PINS = ["GND", "VCC", "SCL", "SDA", "RES", "DC", "BLK"]  # top to bottom as mounted

# Buttons: 12x12 tactile push switch, 10 mm tall (pads from KiCad SW_PUSH-12mm)
BTN_MID = (43.0, 11.0)      # middle button centre
BTN_PITCH = 18.0            # chord distance to the outer buttons (blueprint)
BTN_OPEN = 14.0             # face opening (blueprint)
BTN_CAP = 13.4              # cap body
BTN_FLANGE = 15.2           # retaining flange behind the face
BTN_FLANGE_T = 1.2
BTN_CAP_PROUD = 5.0         # cap top above the face: 4 mm above the visor (v4 kept the same 4 mm above its top plate)
BTN_CAP_TOP = 12.4          # cap top width: a slight taper
# 12x12 projected-plunger switch, seller drawing TS-G005 (7.5 mm version; this one is 10 mm tall)
SW_BODY, SW_BODY_H = 11.7, 3.6  # housing 11.7 x 11.7 x 3.6 (seller drawing, the 7.3 mm switch)
SW_LEG = (0.7, 0.3, 3.5)    # flat legs: width, thickness, length below the housing (measured: 3.5);
                            # 2.2 mm of it comes out of the pocket floor, to wrap and fold
SW_BOSS_D, SW_BOSS_H = 6.4, 4.5   # round platform on the housing, top 4.5 mm above the PCB
# Snap-fit stem (seller drawing): on the platform a short narrower neck, 1 mm tall, then the square
# head (the grip) up to the top, so a cap clicks on under the head
SW_NECK, SW_NECK_H = 3.0, 1.0  # stem neck, square: width, height above the platform
SW_HEAD, SW_HEAD_H = 3.8, 1.7  # square head with a centre hole: width, height
SW_TOTAL_H = SW_BOSS_H + SW_NECK_H + SW_HEAD_H  # head top 7.2 mm above the PCB
CAP_WALL = 1.0              # hollow cap side wall
# The cap holds the stem like a bought round 12x12 cap: round the square head, two opposite sides are
# long fingers whose lips snap in under the head, and the other two are short stiff walls tied to the
# cap's outer wall by a rib each, so the cap cannot rock on the head. A stop post on the cap top rests
# on the head, so the cap is held both ways. (Four free fingers alone were springy and the cap wobbled;
# a stiff socket all round had no snap.)
CAP_TOP_T = 1.2             # flat cap top; the cap prints top down, so the top comes off the bed
CAP_TOP_EDGE = 0.4          # 45° chamfer round the cap top edge
CAP_FINGER = (5.5, 1.6)     # long sides: finger width (5.5 as on a bought round 12x12 cap, past the head's
                            # corners), thickness
CAP_RIB_T = 1.0             # rib from every plate (long and short) out to the cap wall: thickness
CAP_RIB_SHARE = (0.37, 0.55)  # ... and its share of the plate's height from the cap top (the user's grip
                            # support): long fingers, short side plates. The free rest below it is all that
                            # bends as the cap snaps on: 5.9 mm of finger (2.5% strain, 3 t d / 2 L^2 with
                            # d = lip reach + the 0.05 preload of CAP_SNAP_CLR = 0.35) and 4.6 mm of short
                            # plate (1.7%, d 0.2), at PETG's limit of 2-2.5%. (A 0.75 share on both gave 15%
                            # and 7%: the cap would not click on.)
CAP_SNAP = 0.3              # lip reach in under the head (the neck is 0.4 narrower per side)
CAP_SNAP_CLR = -0.05        # finger to head side: they press on it slightly (at 0.1 the printed fingers stood off)
CAP_SNAP_DROP = 0.4         # lip catch this much lower than drawn (measured: at 0.2 the cap was still loose on the head)
CAP_SIDE = (2.0, 1.2, 0.0)  # short sides (2 wide, as on the bought cap): plate width, thickness, play to the head
CAP_SIDE_SNAP = 0.2         # their lips reach this far in under the head (shorter, stiffer plates: less than
                            # the long ones); the ribs make a "+" with the fingers, as on the bought cap
CAP_STOP_D = 3.0            # stop post on the head, round its centre hole
CAP_STOP_ROOT = (3.6, 1.0)  # the post widens to this Ø over this height where it meets the cap top
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]  # pin 1,1,2,2 (land pattern Ø1.5)

# Printed PCB ("carrier"): same plane as a future real PCB
C_FRONT = -10.2             # printed PCB front (the 7.2 mm switch top ends 3.0 below the face: the caps reach down)
# knob just tall enough to cover the whole shaft, so the shaft is never cut
KNOB_H = HW040_FRONT + EC11_SHAFT_H + KNOB_TIP_CLR + KNOB_TOP_T - KNOB_GAP
C_T = 2.0                   # printed thickness (a real board is 1.6, it grows backwards); leaves a 0.6 mm HW-040 pocket floor
C_BACK = C_FRONT - C_T
C_INSET = 0.8               # carrier outline inset from the inner walls
HOLE_D = 1.2                # clearance for other 2.54 mm header pins
DISP_VIA_D = 1.0            # transfer hole of each display wire (the display pin has its own HOLE_D hole)
DISP_VIA_DX = 2.0           # transfer hole this far left of its display pin, same y
RP_PIN_FIT_D = 0.90          # provisional PETG press fit; choose after coupon test
SW_HOLE_D = HOLE_D          # switch legs: same hole as the header pins
POSTS = [(6.0, 7.0), (6.0, H - 7.5), (W - 6.0, H - 7.5), (W - 6.0, 7.0),
         (70.0, 40.0), (16.0, 40.0)]  # mirrored middle supports: six screws total

POST_D = 4.2               # slim: the front-right post sits just outside the right cap's flange
# Post spigot: right through the carrier and on into a socket in the back-plate column, so the
# carrier is held over its whole thickness and the column lines up on the same spigot
SPIGOT_D, SPIGOT_HOLE_D = 3.4, 3.9   # 0.25 mm/side: FDM holes print small
SPIGOT_INTO_COL = 1.5       # spigot past the carrier back, into the column
SPIGOT_SOCKET_CLR = 0.3     # socket deeper than the spigot reaches, so it never bottoms out
# Drop-proof: an M2 x 12 self-tapping screw from under the plinth through each back-plate column
# and the carrier into each face post clamps back plate, carrier and shell together
SCREW_PILOT, SCREW_CLR = 1.6, 2.4
SCREW_HEAD_D = 4.4          # counterbore for a pan head up to 4 mm
SCREW_SEAT_Z = -16.5        # all heads seat here, so every screw is 12 mm
SEAT_LAYER = 0.2            # stepped bridge over each counterbore: one layer (along the screw) per step, see make_back_plate
PEG_CLR = 0.1               # peg diameter = hole - 2 * clearance (press fit)
PEG_PROUD = 2.0             # peg tip past the module board: press fit, or melt it (heat-stake); 1.0 -> 2.0 per user 2026-11-01
PEG_RELIEF = 0.8            # face-back relief over the display peg tips: 0.1 above the 2.0 tips, leaving 0.7 under the visor pocket

# Waveshare RP2040-Zero sits on the carrier underside, components facing the bottom cover.
SM = (18.0, 23.5, 1.1)      # drawing outline; thickness from official STEP
SM_X = 46.0  # underside board, 3 mm off center for the encoder lanes
SM_USB = (9.6, 7.71, 3.25)  # official STEP: width, depth, height above board top
SM_USB_OVERHANG = 1.21      # official STEP, receptacle front past the board end
# USB-C port: a tight pill-shaped hole. A hidden pocket inside the far wall thins it to USB_WALL
# round the receptacle, and the board sits right up against that pocket, so a cable's plug
# reaches fully into the receptacle without its moulded grip entering the wall.
USB_WALL = 0.8              # wall left in front of the receptacle (measured square to the wall)
USB_HOLE = (10.4, 4.8)      # clearance round the STEP's 9.6 x 4.16 mm connector envelope
USB_HOLE_CHAMFER = 0.0      # chamfer round the outside edge: off, it thinned the 0.8 mm wall to a 0.4 lip
USB_FRONT_CLR = 0.15        # receptacle front to the pocket face
SM_ROW = 15.24
SM_PIN_MARGIN = 1.59       # dimensioned first pin from USB end; 2.54 mm pitch
# Component side, USB at top: GPIO0..8 on the right, power/GPIO29..14 on the left.
SM_COL_A = ["IO0", "IO1", "IO2", "IO3", "IO4", "IO5", "IO6", "IO7", "IO8"]
SM_COL_B = ["5V", "GND", "3V3", "IO29", "IO28", "IO27", "IO26", "IO15", "IO14"]
SM_BOTTOM = ["IO13", "IO12", "IO11", "IO10", "IO9"]  # five middle pins; corners belong to side rows
# Nominal board spacing retained from the earlier header-height design.
SM_HDR = (2.54, 9 * 2.54, 2.5, 6.0)  # ordinary headers, no extra printed spacers
SM_BTN_DX = 4.5             # official STEP switch centres
SM_BTN = (3.0, 4.65, 2.5)
C_TAB_CLR = 0.4             # carrier tab under the RP2040-Zero header ends, to the far wall
C_TAB_PAST = 1.5            # tab edges past the outer pin rows

# Enamelled wire runs in carrier channels. Controller and resistor leads are soldered;
# display/encoder headers can be wrapped, and switch legs fold over their wire ends.
# Dipped crossings have Kapton between the insulated wires.
WIRE_D = 0.3                # wire diameter (stand-in)
WIRE_CH = (0.75, 0.6)        # channel width, depth: room for two wires side by side where runs share it
WIRE_BEND_R = 1.5           # channel centre-line radius at every bend (a sharp corner kinks the wire)
WIRE_CROSS = (4.0, 1.0)     # where two runs cross, the lower channel dips: length, depth
KAPTON = (2.0, 0.06)        # Kapton square over the lower wire at each crossing: side, thickness. It
                            # lies in a pocket as deep as the channel plus the tape, so the upper
                            # wire runs on over it at its own channel depth
WIRE_RIDGE_MIN = 0.7        # plastic between two channels, or a channel and a hole, pin row or the edge
WRAP_TURNS = 4              # turns round each header pin (1.2 mm of pin)
WRAP_OD = 1.5               # the turns round a 0.64 mm square pin, outside
PIN_ROW = (2.4, 0.9)        # room kept clear for the turns along each wrapped header row: width, past
                            # the end pins (no other channel runs there)
DISP_PIN_L = 11.0           # display header: long pin past the plastic. A long-pin header (2.54 mm,
                            # 17 mm total, NOMINAL) puts the tips 6.2 mm past the back; the usual
                            # 6 mm header reaches only 1.2 mm, too short to wind on
LEG_POCKET = (1.4, 3.0)     # pocket the folded switch leg lies in, over the wire end: width, length
# RESET uses a bottom access guide; BOOT still requires opening the case.

# the receptacle's face-side edge meets the leaning far wall furthest in, so it sets the board end
RP_FLIP_Z = (C_FRONT + C_BACK) / 2
_rp_ref_hi = C_FRONT + SM_HDR[2] + SM[2]
_usb_hi = 2 * RP_FLIP_Z - (_rp_ref_hi - 0.91)
SM_USB_END = (H - USB_WALL / math.cos(math.radians(WALL_TAPER))  # y of the USB end of the board
              - _usb_hi * math.tan(math.radians(WALL_TAPER)) - USB_FRONT_CLR - SM_USB_OVERHANG)

# Back plate
COLUMN_D = 8.0              # columns clamping the carrier against the face posts (screws inside)
HOOKS = [(-1, 28.0), (-1, 67.0), (+1, 28.0), (+1, 67.0)]  # (side, y centre)
HOOK_W, HOOK_T, HOOK_L = 8.0, 1.4, 10.0
HOOK_BARB = 1.0             # barb with 45° ramps both ways: snaps in, pulls out (a detent)
HOOK_GAP = 1.2              # arm to inner wall
RIB_DEPTH, RIB_H = 1.0, 1.5  # catch rib on the inside of the wall: no windows, nothing visible
ENC_RELIEF = 1.0            # window through the plate (not the plinth below) under the HW-040, its outline plus
                            # this margin: the module hangs lowest there, and the plate's inner face is within
                            # 0.5 mm of it without the window

# White base edge under the black shell (part of the bottom cover)
PLINTH_H = 3.0
# PETG screw covers/feet: six round dots under the plinth, one centred near each corner screw (the dot hides its
# counterbore) and one on the middle screw. Each drops into a shallow round pocket in the plinth's bottom
# face (the bed side), so it locates itself and can be glued or pressed in. A dot is a flange (FOOT_D wide,
# FOOT_PROUD proud, chamfered sole, covers the pocket edge) over a body FOOT_CLR narrower per side that fills
# the pocket, plus a plug up into the counterbore.
FOOT_D = 9.0                # dot Ø = pocket Ø at the plinth face
FOOT_SHIFT = (0.4, 0.4)     # slight inset leaves a pocket rim and covers the screw bore fully

FOOT_PROUD = 1.5            # feet stand this far below the plinth face: the plinth stays off the desk
POCKET_D = 0.9              # pocket depth into the plinth (0.8 to 1.0)
FOOT_CLR = 0.2              # body to pocket wall, per side
FOOT_EDGE = 0.8             # 45° chamfer round the foot's sole

# Separate black display cowl and white trim: every visible face can lie on the print bed.
VISOR_T = 0.0               # key mechanism references the bare main face in v7
COWL_H = 4.5
COWL_T = 2.5
COWL_BOX = (14.0, 56.0, 72.0, 93.0)
COWL_CLIPS = [(16.8, 66.0, -1), (16.8, 88.0, -1),
              (69.2, 66.0, 1), (69.2, 88.0, 1)]
COWL_CLIP_T, COWL_CLIP_W, COWL_CLIP_L = 1.2, 4.0, 9.0
COWL_CLIP_REACH = 0.7
COWL_CURVE_R = 22.0  # follows the halo, with clearance at rest and during knob press
LED_C = (KNOB_C[0] + 10.0, KNOB_C[1])  # hidden beneath knob, 10 mm off-axis
LED_D, LED_H, LED_FLANGE_D = 5.0, 8.7, 5.8  # diameter confirmed; other dimensions NOMINAL
LED_BASE_Z = C_FRONT
LED_TIP_Z = LED_BASE_Z + LED_H
LED_LEAD_HOLE_D, LED_LEAD_D = 0.8, 0.5
LED_FAN_WIDTH, LED_FAN_DEPTH = 0.75, 0.65
HALO_OD, HALO_ID, HALO_T = 42.0, 12.4, 0.6
HALO_R_IN, HALO_R_MID, HALO_R_OUT = 6.0, 13.0, 20.0
RES_C = [(49.19, 54.5), (54.27, 53.5), (56.81, 53.5)]
RES_PITCH = 7.62  # nominal 1/8 W resistor footprint: 3.2 x 1.8 mm body

BEZEL_T = 0.8
BEZEL_W = 28.0
BEZEL_PEG_D, BEZEL_HOLE_D, BEZEL_PEG_H = 1.2, 1.6, 1.4
BEZEL_PEGS = [(DISP_C[0] + sx * 12.5, DISP_C[1] + sy * 12.5)
              for sx in (-1, 1) for sy in (-1, 1)]
KNOB_CAP_T, KNOB_CAP_D = 0.8, 31.0
KNOB_CAP_PEGS = [(KNOB_C[0] + 10 * math.cos(math.radians(a)),
                  KNOB_C[1] + 10 * math.sin(math.radians(a))) for a in (0, 120, 240)]
UNDER_CLR = 0.3


OVH_MAX = 45.0              # steepest overhang that prints without support: degrees from vertical
BRIDGE_MAX = 10.0           # longest bridge accepted (mm)
OVH_LEDGE = 0.5             # a flat ledge this narrow (about a line width) needs no support either

DOC_NAME = "KivoriCaseV7"
EXPORT = True

# ---------------------------------------------------------------------------------------------
# Geometry helpers
# ---------------------------------------------------------------------------------------------

K_WEDGE = (T_FAR - T_NEAR) / H


def z_back(y):
    """Outer back surface (desk side) at y."""
    return -(T_NEAR + K_WEDGE * y)


def rr_wire(w, h, r, z=0.0):
    hw, hh = w / 2.0, h / 2.0
    r = min(r, hw - 1e-3, hh - 1e-3)
    if r <= 1e-6:
        return Part.makePolygon([V(-hw, -hh, z), V(hw, -hh, z), V(hw, hh, z), V(-hw, hh, z), V(-hw, -hh, z)])

    def arc(cx, cy, a0, a1):
        c = Part.Circle(V(cx, cy, z), V(0, 0, 1), r)
        return Part.ArcOfCircle(c, math.radians(a0), math.radians(a1)).toShape()

    def line(a, b):
        return Part.LineSegment(V(a[0], a[1], z), V(b[0], b[1], z)).toShape()

    e = [
        line((-hw + r, -hh), (hw - r, -hh)), arc(hw - r, -hh + r, -90, 0),
        line((hw, -hh + r), (hw, hh - r)), arc(hw - r, hh - r, 0, 90),
        line((hw - r, hh), (-hw + r, hh)), arc(-hw + r, hh - r, 90, 180),
        line((-hw, hh - r), (-hw, -hh + r)), arc(-hw + r, -hh + r, 180, 270),
    ]
    return Part.Wire(e)


def place(shape, cx, cy, ang=0.0):
    s = shape.copy()
    if ang:
        s.rotate(V(0, 0, 0), V(0, 0, 1), ang)
    s.translate(V(cx, cy, 0))
    return s


def rr_prism(w, h, r, cx, cy, z0, z1, ang=0.0):
    f = Part.Face(rr_wire(w, h, r, z0))
    return place(f.extrude(V(0, 0, z1 - z0)), cx, cy, ang)


def box(x0, y0, z0, x1, y1, z1):
    return Part.makeBox(abs(x1 - x0), abs(y1 - y0), abs(z1 - z0), V(min(x0, x1), min(y0, y1), min(z0, z1)))


def cbox(cx, cy, w, h, z0, z1, ang=0.0):
    return place(box(-w / 2, -h / 2, z0, w / 2, h / 2, z1), cx, cy, ang)


def cyl(d, cx, cy, z0, z1):
    return Part.makeCylinder(d / 2.0, z1 - z0, V(cx, cy, z0), V(0, 0, 1))


TAPER = math.tan(math.radians(WALL_TAPER))
ALPHA = math.atan(K_WEDGE)
H_DESK = H / math.cos(ALPHA)  # length of the back plane along the slope


def tapered(w, h, r, z0, z1, cx=W / 2, cy=H / 2):
    """Rounded-rectangle frustum: (w, h, r) at z0, growing outwards towards z1 < z0 at WALL_TAPER."""
    g = (z0 - z1) * TAPER
    lo = rr_wire(w + 2 * g, h + 2 * g, r + g, z1)
    hi = rr_wire(w, h, r, z0)
    return place(Part.makeLoft([lo, hi], True, True), cx, cy)


def inner_x(z):
    """x of the left inner wall at height z (the wall leans outwards going down)."""
    return WALL + z * TAPER


def desk_to_face(shape):
    """Map a shape built in the desk frame (back plane = z 0, +z into the case) to the face frame."""
    s = shape.copy()
    s.rotate(V(0, 0, 0), V(1, 0, 0), -math.degrees(ALPHA))
    s.translate(V(0, 0, -T_NEAR))
    return s


def above_back(offset):
    """Everything on the case side of (back surface + offset)."""
    m = 10.0
    pts = [V(0, -m, 60), V(0, H + m, 60), V(0, H + m, z_back(H + m) + offset), V(0, -m, z_back(-m) + offset)]
    f = Part.Face(Part.makePolygon(pts + [pts[0]]))
    s = f.extrude(V(W + 2 * m, 0, 0))
    s.translate(V(-m, 0, 0))
    return s


def fuse_all(shapes):
    shapes = [s for s in shapes if s is not None]
    if len(shapes) == 1:
        return shapes[0]
    return shapes[0].fuse(shapes[1:]).removeSplitter()


def rot(pt, ang):
    a = math.radians(ang)
    return (pt[0] * math.cos(a) - pt[1] * math.sin(a), pt[0] * math.sin(a) + pt[1] * math.cos(a))


# ---------------------------------------------------------------------------------------------
# Derived layout
# ---------------------------------------------------------------------------------------------

def button_poses():
    """Straight row, centered on the dial/display axis."""
    return [(BTN_MID[0] + i * BTN_PITCH, BTN_MID[1], 0.0) for i in (-1, 0, 1)]


BTNS = button_poses()
SW_PLUNGER_TOP = C_FRONT + SW_TOTAL_H

# Display: module u axis lies along face x (DISP_HEADER_SIDE says which way the header points)
dpcb_c = (DISP_C[0] - DISP_HEADER_SIDE * DISP_ACTIVE_U, DISP_C[1])
glass_c = (dpcb_c[0] + DISP_HEADER_SIDE * DISP_GLASS_U, DISP_C[1])
glass_z0 = COWL_H - COWL_T + DISP_POCKET - DISP_FACE_GAP  # glass front
glass_z1 = glass_z0 - DISP_GLASS[2]       # glass back = module PCB front
dpcb_z1 = glass_z1 - DISP_PCB[2]          # module PCB back
disp_hdr_x = dpcb_c[0] + DISP_HEADER_SIDE * (DISP_PCB[0] / 2 - DISP_HDR_U)
disp_hdr_y = dpcb_c[1] + DISP_HDR_V
disp_pins = {n: (disp_hdr_x, disp_hdr_y + (3 - i) * 2.54) for i, n in enumerate(DISP_PINS)}
disp_vias = {n: (x - DISP_VIA_DX, y) for n, (x, y) in disp_pins.items()}
disp_vias['BLK'] = (disp_vias['BLK'][0], 68.9)  # below the DC wrap, which runs under IO14 and up past it
# Power. A channel cannot pass between two RP pin holes (0.445 mm of plastic each side at 2.54 mm pitch), so GND and 3V3
# leave the right column to the east, go round the display header and the bottom end of the pin rows, and come up on
# the encoder side. GND runs on the front (through the display GND hole, GND_X); 3V3 hops to the rear next to its pin,
# runs east on the rear (V33_X), feeds the display VCC pin from the east and drops to the front south of GND (V33_Y).
GND_X, GND_Y = 62.6, 62.6
V33_X, V33_Y = 64.3, 60.8
del disp_vias['VCC']        # VCC is fed on the rear straight to its pin
disp_vias['GND'] = (GND_X, disp_vias['GND'][1])
disp_holes = [(dpcb_c[0] + su * (DISP_PCB[0] / 2 - DISP_HOLE_INSET), dpcb_c[1] + sv * (DISP_PCB[1] / 2 - DISP_HOLE_INSET))
              for su in (-1, 1) for sv in (-1, 1)]

# HW-040 rotated 180° around the stationary shaft: header to the left, holes to the far side.
hw_c = (KNOB_C[0] + HW040_SHAFT[0], KNOB_C[1] + HW040_SHAFT[1])
hw_holes = [(hw_c[0] - hx, hw_c[1] - hy) for (hx, hy) in HW040_HOLES]
hw_hdr = (hw_c[0] - HW040_HDR_X, hw_c[1] - HW040_SHAFT[1])  # header row centre
hw_back = HW040_FRONT - HW040[2]           # HW-040 board back
enc_x = hw_hdr[0] - HW040_PIN_BEND         # where its pins come up through the printed PCB
enc_pins = {n: (enc_x, hw_hdr[1] + (i - 2) * 2.54) for i, n in enumerate(["CLK", "DT", "SW", "+", "GND"])}

sm_c = (SM_X, SM_USB_END - SM[1] / 2)
rp_ref_lo = C_FRONT + SM_HDR[2]
rp_ref_hi = rp_ref_lo + SM[2]
sm_lo, sm_hi = 2 * RP_FLIP_Z - rp_ref_hi, 2 * RP_FLIP_Z - rp_ref_lo
usb_lo = 2 * RP_FLIP_Z - (rp_ref_hi + SM_USB[2])
usb_hi = 2 * RP_FLIP_Z - (rp_ref_hi - 0.91)
sm_native_pins = {}
for col, x in ((SM_COL_A, SM_X + SM_ROW / 2), (SM_COL_B, SM_X - SM_ROW / 2)):
    for i, n in enumerate(col):
        sm_native_pins[n] = (x, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
sm_bottom_y = SM_USB_END - SM_PIN_MARGIN - 8 * 2.54
for i, n in enumerate(SM_BOTTOM):
    sm_native_pins[n] = (SM_X + (i - 2) * 2.54, sm_bottom_y)
sm_pins = {n: (2 * SM_X - x, y) for n, (x, y) in sm_native_pins.items()}
sm_rows = [SM_X - SM_ROW / 2, SM_X + SM_ROW / 2]
sm_hdr_y = SM_USB_END - SM_PIN_MARGIN - 4 * 2.54
sm_btns = [(SM_X - SM_BTN_DX, SM_USB_END - 16), (SM_X + SM_BTN_DX, SM_USB_END - 16)]
RESET_C = (SM_X - SM_BTN_DX, SM_USB_END - 16)
RESET_SURFACE_Z = sm_lo - SM_BTN[2]
usb_z = (usb_lo + usb_hi) / 2
led_pins = {n: (LED_C[0] + (i - 1.5) * 2.54, LED_C[1])
            for i, n in enumerate(('R', 'C', 'G', 'B'))}
res_pins = {n: (x, y + side * RES_PITCH / 2) for i, (x, y) in enumerate(RES_C)
            for n, side in ((('R', 'G', 'B')[i] + '_IN', 1), (('R', 'G', 'B')[i] + '_OUT', -1))}

# The underside wires terminate on the actual GPIO header pins.
RP_NETS = {'IO3':'SW', 'IO4':'DT', 'IO5':'CLK', 'IO6':'B1', 'IO7':'B2', 'IO8':'B3',
           'IO9':'R_PWM', 'IO10':'G_PWM', 'IO11':'B_PWM',
           'IO26':'SCL', 'IO15':'SDA', 'IO14':'RES', 'IO13':'DC', 'IO12':'BLK',
           '3V3':'3V3', 'GND':'GND'}
rp_vias = {'SW':(24.2,sm_pins['IO3'][1]), 'DT':(22.5,sm_pins['IO4'][1]),
           'CLK':(20.8,sm_pins['IO5'][1]), 'B1':(11.0,sm_pins['IO6'][1]),
           'B2':(12.9,sm_pins['IO7'][1]), 'B3':(19.1,sm_pins['IO8'][1]),
           'R_PWM':(sm_pins['IO9'][0],sm_bottom_y-2.4),
           'G_PWM':(sm_pins['IO10'][0],sm_bottom_y-2.4),
           'B_PWM':(sm_pins['IO11'][0],sm_bottom_y-2.4),
           'GND':(27.6,GND_Y),
           '3V3':(25.9,V33_Y)}
# transfer holes of the power runs that are not at a pin or a header: (net, position)
power_vias = [('3V3',(sm_pins['3V3'][0]+1.78,sm_pins['3V3'][1])),('3V3',(V33_X,V33_Y)),('GND',(9.1,GND_Y))]
v33_join = (V33_X,disp_pins['VCC'][1])
LED_TERM_Y = LED_C[1] + 8.0
led_terminals = {n:(x, LED_TERM_Y) for n,(x,y) in led_pins.items()}
led_native_pins = {n:(LED_C[0]+(i-1.5)*1.27,LED_C[1]) for i,n in enumerate(('R','C','G','B'))}
led_ground_vias = [(47.0,45.8),led_terminals['C']]


def btn_pin(idx, pad):
    x, y, a = BTNS[idx]
    dx, dy = rot(SW_PADS[pad], a)
    return (x + dx, y + dy)


# ---------------------------------------------------------------------------------------------
# Parts
# ---------------------------------------------------------------------------------------------


def hook_geom(yc):
    """Left-side hook in the face frame: arm box, barb points and catch rib (mirror for the right)."""
    z0 = z_back(yc + HOOK_W / 2) + BP_T - 0.5
    zt = z0 + HOOK_L
    xa = inner_x(zt) + HOOK_GAP          # arm outer face
    b = HOOK_BARB
    barb = [(xa, zt - 2 * b), (xa - b, zt - b), (xa, zt)]
    x_r = inner_x(zt - 2.5) + RIB_DEPTH   # rib inner face
    z_rt = zt - 2 * b + (xa - x_r) - 0.05  # barb's lower ramp rests on the rib's top edge
    rib = (inner_x(z_rt - RIB_H) - 0.8, x_r, z_rt - RIB_H, z_rt)
    return z0, zt, xa, barb, rib


def mirror_x(shape, side):
    return shape.mirror(V(W / 2, 0, 0), V(1, 0, 0)) if side > 0 else shape


def make_shell():
    c = FACE_BEVEL
    hb = c / (1 - TAPER)  # bevel height: it grows c + hb * TAPER = hb outwards, i.e. 45°
    outer = Part.makeLoft([rr_wire(W + 2 * 60 * TAPER, H + 2 * 60 * TAPER, R_CORNER + 60 * TAPER, -60),
                           rr_wire(W + 2 * hb * TAPER, H + 2 * hb * TAPER, R_CORNER + hb * TAPER, -hb),
                           rr_wire(W - 2 * c, H - 2 * c, R_CORNER - c, 0)], True, True)
    outer = place(outer, W / 2, H / 2).common(above_back(0))
    cavity = tapered(W - 2 * WALL, H - 2 * WALL, R_CORNER - WALL, 0, -60)
    cavity = cavity.common(box(-50, -50, -60, W + 50, H + 50, -FACE_T))
    shell = outer.cut(cavity)

    adds = []
    for (x, y) in POSTS:
        adds.append(cyl(POST_D, x, y, C_FRONT, -FACE_T + 0.01))
        adds.append(cyl(SPIGOT_D, x, y, C_BACK - SPIGOT_INTO_COL, C_FRONT + 0.01))
    for (side, yc) in HOOKS:  # catch ribs for the back plate hooks
        x0, x1, z0, z1 = hook_geom(yc)[4]
        # the catch face slopes 45° back into the wall: the barb's 45° ramp rests on it, and the
        # shell prints face down without a ledge
        prof = [(x0, z0), (x1, z0), (x1, z1), (x0, z1 + (x1 - x0))]
        rib = Part.Face(Part.makePolygon([V(px, 0, pz) for (px, pz) in prof + [prof[0]]]))
        rib = rib.extrude(V(0, HOOK_W + 0.8, 0))
        rib.translate(V(0, yc - HOOK_W / 2 - 0.4, 0))
        adds.append(mirror_x(rib, side))
    shell = shell.fuse(adds)

    cuts = [cyl(ENC_HOLE_D, KNOB_C[0], KNOB_C[1], -FACE_T - 1, 1)]
    if KNOB_HALO:
        cuts.append(cyl(2 * KNOB_HALO[1], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2], 1).cut(
            cyl(2 * KNOB_HALO[0], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2] - 1, 2)))
    # Full module/peg clearance: the removable cowl forms the display roof.
    cuts.append(rr_prism(DISP_PCB[0] + 1.4, DISP_PCB[1] + 1.4, 1.0,
                         dpcb_c[0], dpcb_c[1], -FACE_T - 1, 1))
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN + 2 * UNDER_CLR, BTN_OPEN + 2 * UNDER_CLR, 1.8, x, y, -FACE_T - 1, 1, a))
    for (x, y) in POSTS:
        cuts.append(cyl(SCREW_PILOT, x, y, C_BACK - SPIGOT_INTO_COL - 1, -FACE_T - 1.5))
    cuts += usb_port_cuts()
    # Rounded slots guide the clip beams; the cowl's seating rim takes downward loads.
    shell = shell.fuse([cbox(x, y, 4.0, 7.0, -7.0, -FACE_T + 0.01) for x, y, side in COWL_CLIPS])
    cuts += [rr_prism(1.8, COWL_CLIP_W + 0.6, 0.3, x, y, -8.0, 1)
             for x, y, side in COWL_CLIPS]
    # The white-lined mixing cavity has a 45-degree V-profile and a six-millimetre nut clearance.
    bowl=annular_profile([(6,-0.8),(12.6,-7.4),(13.4,-7.4),(20,-0.8),
                          (20,-2.0),(13.4,-8.6),(12.6,-8.6),(6,-2.0)])
    cavity=annular_profile([(6,1.0),(20,1.0),(20,-0.8),(13.4,-7.4),(12.6,-7.4),(6,-0.8)])
    shell=shell.fuse(bowl)
    cuts += [cavity,cyl(6.2,*LED_C,C_FRONT-0.1,1)]
    return shell.cut(cuts).removeSplitter()


def usb_port_cuts():
    """Pill hole through the far wall, square to it, with a chamfer round the outside, and the
    hidden pocket inside that thins the wall round the receptacle and the board end."""
    w, h, c = USB_HOLE[0], USB_HOLE[1], USB_HOLE_CHAMFER
    # built round the hole axis along +y, origin on the outer wall surface
    hole = Part.Face(rr_wire(w, h, h / 2 - 1e-3, 0)).extrude(V(0, 0, 8))
    hole.translate(V(0, 0, -6))
    cham = Part.makeLoft([rr_wire(w, h, h / 2 - 1e-3, -c), rr_wire(w + 2 * c + 1, h + 2 * c + 1, h / 2 + c + 0.5, 0.5)],
                         True) if c > 0 else None
    port = hole.fuse(cham) if c > 0 else hole
    port.rotate(V(0, 0, 0), V(1, 0, 0), -90)   # extrusion +z -> +y, pill height along z
    port.rotate(V(0, 0, 0), V(1, 0, 0), WALL_TAPER)  # square to the leaning wall
    port.translate(V(SM_X, H - usb_z * TAPER, usb_z))

    # pocket: from the inside up to USB_WALL short of the outer surface, over the board width and
    # from just behind the carrier down past the receptacle. Its far-from-face side slopes 45°,
    # so the shell still prints face down without an overhang.
    u = USB_WALL / math.cos(math.radians(WALL_TAPER))
    face_ = tapered(W - 2 * u, H - 2 * u, R_CORNER - u, 0, -60)
    z_top, z_bot = max(sm_hi, usb_hi) + 0.3, min(sm_lo, usb_lo) - 0.4
    yf = H - u - z_bot * TAPER        # pocket face at the bottom of the pocket
    prof = [(H - 15, z_top), (H + 5, z_top), (H + 5, z_bot + (H + 5 - yf)), (H - 15, z_bot - (yf - H + 15))]
    pocket = Part.Face(Part.makePolygon([V(0, py, pz) for (py, pz) in prof + [prof[0]]]))
    pocket = pocket.extrude(V(SM[0] + 1.2, 0, 0))
    pocket.translate(V(SM_X - SM[0] / 2 - 0.6, 0, 0))
    return [port, pocket.common(face_)]


def make_cowl():
    """Black removable roof with a 45-degree front ramp; prints top down."""
    x0, y0, x1, y1 = COWL_BOX
    c = rr_prism(x1 - x0, y1 - y0, 4.0, (x0 + x1) / 2, (y0 + y1) / 2, 0, COWL_H)
    curve = Part.makeCone(COWL_CURVE_R, COWL_CURVE_R + 0.2, COWL_H + 0.02,
                          V(*KNOB_C, -0.01), V(0, 0, 1))
    c = c.cut(curve)
    # Module clearance plus FPC air; thick bosses outside the opening take the screws.
    cavity = rr_prism(DISP_PCB[0] + 1.4, DISP_PCB[1] + 1.4, 0.5,
                      dpcb_c[0], dpcb_c[1], -1, COWL_H - COWL_T)
    c = c.cut(cavity)

    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN + 2 * UNDER_CLR
    cuts = [rr_prism(win, win, 1.0, *DISP_C, -1, COWL_H + 1),
            rr_prism(DISP_GLASS[0] + 0.5, DISP_GLASS[1] + 0.5, 0.5, *glass_c,
                     -1, COWL_H - COWL_T + DISP_POCKET)]
    fd, fm = DISP_FPC_RELIEF
    x_glass = glass_c[0] - DISP_GLASS[0] / 2
    x_board = dpcb_c[0] - DISP_PCB[0] / 2 - fm
    cuts.append(box(x_board, glass_c[1] - DISP_GLASS[1] / 2 - fm, -1,
                    x_glass + 0.5, glass_c[1] + DISP_GLASS[1] / 2 + fm, COWL_H - COWL_T + fd))
    cuts += [cyl(DISP_STANDOFF_D, x, y, -1, COWL_H - COWL_T + PEG_RELIEF) for x, y in disp_holes]

    cuts += [cyl(BEZEL_HOLE_D, x, y, COWL_H - BEZEL_PEG_H - 0.2, COWL_H + 1) for x, y in BEZEL_PEGS]
    c = c.cut(cuts)
    for x, y, side in COWL_CLIPS:
        beam = cbox(x, y, COWL_CLIP_T, COWL_CLIP_W, -COWL_CLIP_L, 0.01)
        # Flat retention shoulder below the face; 45-degree insertion lead-in at the tip.
        inner = side * COWL_CLIP_T / 2
        outer = inner + side * COWL_CLIP_REACH
        profile = [(inner, -7.1), (outer, -7.8), (outer, -7.9), (inner, -8.6)]
        f = Part.Face(Part.makePolygon([V(x + px, y - COWL_CLIP_W / 2, z) for px, z in profile + [profile[0]]]))
        c = c.fuse([beam, f.extrude(V(0, COWL_CLIP_W, 0))])
    return c.removeSplitter()


def annular_profile(points):
    """Revolve an r/z section about the knob shaft."""
    cx,cy=KNOB_C
    pts=[V(cx+r,cy,z) for r,z in points]
    return Part.Face(Part.makePolygon(pts+[pts[0]])).revolve(V(cx,cy,0),V(0,0,1),360)


def make_halo():
    """White cover: thick hidden roof under the knob and the existing outer diffuser."""
    cx,cy=KNOB_C
    outer=cyl(HALO_OD,cx,cy,0,HALO_T).cut(cyl(HALO_ID,cx,cy,-1,1))
    roof=cyl(36,cx,cy,-0.6,0.01).cut(cyl(HALO_ID,cx,cy,-1,1))
    return outer.fuse(roof).removeSplitter()


def make_halo_reflector():
    """FDM-sloped white liner; LED opening and light distribution require physical testing."""
    floor=annular_profile([(6.8,-1.5),(12.6,-7.3),(13.4,-7.3),(19.8,-0.9),
                           (19.8,-0.3),(13.4,-6.7),(12.6,-6.7),(6.8,-0.9)])
    # Open the inner edge instead of leaving an unprintable 0.1 mm sliver.
    return floor.cut(cyl(6.8,*LED_C,C_FRONT-1,1)).removeSplitter()


def make_pin_coupon():
    """Five nine-pin rows permit testing a real header strip."""
    coupon=box(0,0,0,31,16,2)
    sizes=(0.80,0.85,0.90,0.95,1.00)
    coupon = coupon.cut([cyl(d,2.34+2.54*i,2.92+2.54*j,-1,3)
                         for j,d in enumerate(sizes) for i in range(9)]).removeSplitter()
    coupon = coupon.cut([band([(2.34+2.54*i,2.92+2.54*j),(1.24+2.54*i,2.92+2.54*j)],
                              WIRE_CH[0], 2-WIRE_CH[1], 3)
                         for j in range(5) for i in range(9)])
    # One through five dot marks identify 0.80 through 1.00 mm rows.
    coupon = coupon.cut([cyl(0.8,24.6+i,2.92+2.54*j,1.5,3)
                         for j in range(5) for i in range(j+1)])
    coupon.translate(V(-40, 0, 0))
    return coupon



def make_bezel():
    v = rr_prism(BEZEL_W, BEZEL_W, 2.5, *DISP_C, COWL_H, COWL_H + BEZEL_T)
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN
    v = v.cut(rr_prism(win, win, 1.0, *DISP_C, COWL_H - 1, COWL_H + BEZEL_T + 1))
    v = v.fuse([cyl(BEZEL_PEG_D, x, y, COWL_H - BEZEL_PEG_H, COWL_H + 0.01) for x, y in BEZEL_PEGS])
    return v.removeSplitter()


def make_knob_cap():
    z = KNOB_GAP + KNOB_H
    cap = cyl(KNOB_CAP_D, *KNOB_C, z, z + KNOB_CAP_T)
    cap = cap.fuse([cyl(1.6, x, y, z - 1.0, z + 0.01) for x, y in KNOB_CAP_PEGS])
    # Through-slot reveals black body below, with no filament swap or paint.
    mark = box(KNOB_C[0] - 0.6, KNOB_C[1] + 9, z - 0.1,
               KNOB_C[0] + 0.6, KNOB_C[1] + 14, z + KNOB_CAP_T + 0.1)
    return cap.cut(mark).removeSplitter()


def make_back_plate():
    plate = tapered(W - 2 * WALL - 2 * BP_CLR, H - 2 * WALL - 2 * BP_CLR, R_CORNER - WALL - BP_CLR, 0, -60)
    plate = plate.common(above_back(0)).cut(above_back(BP_T))
    # window under the HW-040 (the plinth below it stays): the encoder's legs, header joints and board
    # hang lower than anything else, so this is what limits how far the back plate can come up
    ex, ey = HW040[0] / 2 + ENC_RELIEF, HW040[1] / 2 + ENC_RELIEF
    plate = plate.cut(box(hw_c[0] - ex, hw_c[1] - ey, -80, hw_c[0] + ex, hw_c[1] + ey, 60)
                      .common(above_back(-1)).cut(above_back(BP_T + 1)))

    adds = []
    for (side, yc) in HOOKS:
        z0, zt, xa, barb, _ = hook_geom(yc)
        arm = box(xa, yc - HOOK_W / 2, z0, xa + HOOK_T, yc + HOOK_W / 2, zt)
        tri = Part.Face(Part.makePolygon([V(px, 0, pz) for (px, pz) in barb] + [V(barb[0][0], 0, barb[0][1])]))
        tri = tri.extrude(V(0, HOOK_W, 0))
        tri.translate(V(0, yc - HOOK_W / 2, 0))
        adds.append(mirror_x(arm.fuse(tri), side))
    for (x, y) in POSTS:
        adds.append(cyl(COLUMN_D, x, y, z_back(y) + 0.5, C_BACK - 0.1))
    plate = plate.fuse([a.common(above_back(0)) for a in adds])

    # White base: 1.2 mm visible vertical edge and a 45-degree underside chamfer.
    # The flat bottom remains bed-facing; no cosmetic stripe is left as an unsupported shelf.
    plinth = Part.Face(plinth_outline()).extrude(V(0, 0, -PLINTH_H))
    try:
        plinth = plinth.makeChamfer(PLINTH_H - 1.2, [e for e in plinth.Edges
                                                     if abs(e.BoundBox.ZMax + PLINTH_H) < 1e-6])
    except Exception as exc:
        print("plinth chamfer skipped:", exc)
    zb = -PLINTH_H
    # desk-side details: the six round foot pockets
    under = [foot_prism(site, 0, zb - 0.1, zb + POCKET_D) for site in foot_sites()]
    plinth = plinth.cut(under)
    plinth = desk_to_face(plinth)
    plate = plate.fuse(plinth)

    cuts = []
    for (x, y) in POSTS:  # screw through each column: clearance hole and a counterbore from below
        cuts.append(cyl(SCREW_CLR, x, y, -60, C_BACK + 1))
        cuts.append(cyl(SCREW_HEAD_D, x, y, -60, SCREW_SEAT_Z))
        # stepped bridge, so the seat prints over air without support (the screw leans 10° from the
        # print's Z, so the seat is a 10° roof): the seat stays one flat plane square to the screw, but
        # its first layer is cut by a slot as wide as the hole across the whole counterbore (two
        # D-shaped pads, each bridging along x between the counterbore's walls, the head rests on them),
        # the second layer keeps only the hole's square (the plastic left bridges the slot along y, between
        # the pads), and the round hole starts above that
        bore = cyl(SCREW_HEAD_D, x, y, SCREW_SEAT_Z - 0.1, SCREW_SEAT_Z + 2 * SEAT_LAYER)
        cuts.append(box(x - SCREW_HEAD_D, y - SCREW_CLR / 2, SCREW_SEAT_Z - 0.1,
                        x + SCREW_HEAD_D, y + SCREW_CLR / 2, SCREW_SEAT_Z + SEAT_LAYER).common(bore))
        cuts.append(box(x - SCREW_CLR / 2, y - SCREW_CLR / 2, SCREW_SEAT_Z + SEAT_LAYER - 0.01,
                        x + SCREW_CLR / 2, y + SCREW_CLR / 2, SCREW_SEAT_Z + 2 * SEAT_LAYER))
        cuts.append(cyl(SPIGOT_HOLE_D, x, y, C_BACK - SPIGOT_INTO_COL - SPIGOT_SOCKET_CLR, C_BACK + 1))
    # Guide stops 0.6 mm short of the outward-facing RESET button.
    rx, ry = RESET_C
    plate = plate.fuse(cyl(4.0, rx, ry, z_back(ry)+1.0, RESET_SURFACE_Z-0.6))
    cuts.append(cyl(1.4, rx, ry, -60, RESET_SURFACE_Z-0.5))
    return plate.cut(cuts).removeSplitter()


def plinth_outline():
    """Outer shell outline on the back plane: a visible white base edge, chamfered underneath."""
    env = tapered(W, H, R_CORNER, 0, -80)  # white base reaches the black wall seam
    env_d = env.copy()  # face frame -> desk frame
    env_d.translate(V(0, 0, T_NEAR))
    env_d.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))
    return max(env_d.slice(V(0, 0, 1), 0.0), key=lambda w_: Part.Face(w_).Area)


def plinth_flat():
    """The plinth's flat bottom face after the underside chamfer, desk frame."""
    f = Part.Face(plinth_outline()).makeOffset2D(-(PLINTH_H - 1.2)).Faces[0]
    f.translate(V(0, 0, -PLINTH_H))
    return f


def screw_axes():
    """Screw axes at the plinth bottom, desk frame (x, y), in POSTS order."""
    sa, ca = math.sin(ALPHA), math.cos(ALPHA)
    return [(x, (y + PLINTH_H * sa) / ca) for (x, y) in POSTS]


def foot_sites():
    """The six PETG screw-cover feet in the desk frame: (centre x, centre y, plug position), in POSTS order
    (near-left, far-left, far-right, near-right, middle). The middle dot sits on its screw, a corner dot
    FOOT_SHIFT inboard of its screw."""
    ax = screw_axes()
    mid = plinth_flat().CenterOfMass
    sites = []
    for i, (x, y) in enumerate(ax):
        if i >= 4:
            sites.append((x, y, ax[i]))
        else:  # inboard: towards the flat's centre in x and y
            sites.append((x + math.copysign(FOOT_SHIFT[0], mid.x - x), y + math.copysign(FOOT_SHIFT[1], mid.y - y), ax[i]))
    return sites


def foot_prism(site, shrink, z0, z1):
    """A foot's plan (round) shrunk by `shrink` per side, extruded from z0 to z1."""
    return cyl(FOOT_D - 2 * shrink, site[0], site[1], z0, z1)


def make_screw_feet():
    """Six PETG pocket-located covers, soles on the bed; retain with small adhesive spots."""
    zb, zt = -PLINTH_H, -PLINTH_H + POCKET_D
    feet = []
    for site in foot_sites():
        flange = foot_prism(site, 0, zb - FOOT_PROUD, zb)
        try:
            flange = flange.makeChamfer(FOOT_EDGE, [e for e in flange.Edges
                                                    if abs(e.BoundBox.ZMax - (zb - FOOT_PROUD)) < 1e-6])
        except Exception as exc:
            print("foot chamfer skipped:", exc)
        # A rigid PETG pocket locator needs no unsupported, offset TPU-style screw plug.
        feet.append(flange.fuse(foot_prism(site, FOOT_CLR, zb - 0.01, zt)).removeSplitter())
    return desk_to_face(Part.makeCompound(feet))


def check_underside():
    """The bed side's plastic and layout: the foot pockets keep WIRE_RIDGE_MIN from the plinth's flat
    edge, and the feet stand level and alone below the plinth. Returns problems."""
    zb = -PLINTH_H
    flat = plinth_flat().OuterWire
    pockets = [foot_prism(site, 0, zb, zb + POCKET_D) for site in foot_sites()]
    problems = []
    rim = min(g_.distToShape(flat)[0] for g_ in pockets)
    if rim < WIRE_RIDGE_MIN - 1e-6:
        problems.append("pocket to the flat's edge %.2f" % rim)
    # level: the six soles are coplanar at -(PLINTH_H + FOOT_PROUD); no other part goes below the plinth
    low = -(PLINTH_H + FOOT_PROUD)
    for n, shp in PRINTED.items():
        sd = shp.copy()
        sd.translate(V(0, 0, T_NEAR))
        sd.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))
        z = sd.optimalBoundingBox().ZMin
        if n == "ScrewFeet" and abs(z - low) > 1e-3:
            problems.append("feet soles at z %.2f, not %.2f" % (z, low))
        elif n != "ScrewFeet" and z < -PLINTH_H - 1e-3:
            problems.append("%s below the plinth bottom (z %.2f)" % (n, z))
    print("underside: pocket rim %.2f" % rim)
    return problems


def carrier_outline():
    c = rr_prism(W - 2 * WALL - 2 * C_INSET, H - 2 * WALL - 2 * C_INSET, R_CORNER - WALL - C_INSET,
                 W / 2, H / 2, C_BACK, C_FRONT)
    # tab towards the far wall (it leans out, so measured at the carrier front): keeps the
    # RP2040-Zero's first pin holes inside the board now that the board sits up against the wall
    y_tab = H - WALL - C_FRONT * TAPER - C_TAB_CLR
    return c.fuse(box(min(sm_rows) - C_TAB_PAST, H / 2, C_BACK, max(sm_rows) + C_TAB_PAST, y_tab, C_FRONT)).removeSplitter()


def carrier_through_cuts(pcb=False):
    """Everything that goes right through the board (pcb=True: what a real PCB keeps)."""
    cuts = []
    z0, z1 = C_BACK - 1, C_FRONT + 1
    for (side, yc) in HOOKS:  # clear the snap hooks
        x0, x1 = (0, WALL + 2.8) if side < 0 else (W - WALL - 2.8, W)
        cuts.append(box(x0, yc - HOOK_W / 2 - 1.2, z0, x1, yc + HOOK_W / 2 + 1.2, z1))
    for (x, y) in POSTS:
        cuts.append(cyl(SPIGOT_HOLE_D, x, y, z0, z1))
    # the EC11 bushing reaches through from the HW-040 hanging behind (its body stops just behind)
    cuts.append(cyl(EC11_BUSH_D + 2 * EC11_HOLE_CLR, KNOB_C[0], KNOB_C[1], z0, z1))
    pins = list(enc_pins.values()) + list(res_pins.values()) + list(disp_pins.values())
    cuts += [cyl(DISP_VIA_D, x, y, z0, z1) for x, y in disp_vias.values()]
    cuts += [cyl(RP_PIN_FIT_D, x, y, z0, z1) for x, y in sm_pins.values()]
    cuts += [cyl(1.2, x, y, z0, z1) for x, y in list(rp_vias.values()) + [p for _, p in power_vias]]
    cuts += [cyl(LED_LEAD_HOLE_D,x,y,z0,z1) for x,y in led_native_pins.values()]
    cuts += [cyl(1.2,x,y,z0,z1) for x,y in led_ground_vias]
    cuts += [cyl(HOLE_D, x, y, z0, z1) for (x, y) in pins]
    cuts += [cyl(SW_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(3) for k in range(4)]
    return cuts


def front_layout():
    """Direct LCD links plus short, supported GPIO transfers into rear vias."""
    sm,dv=lambda n:sm_pins[n],lambda n:disp_vias[n]
    runs=[('SCL',[sm('IO26'),dv('SCL')]),('SDA',[sm('IO15'),dv('SDA')]),
          ('RES',[sm('IO14'),dv('RES')]),
          ('DC',[sm('IO13'),(sm('IO13')[0],70.4),(55.95,70.4),(55.95,72.0),dv('DC')]),
          ('BLK',[sm('IO12'),(sm('IO12')[0],dv('BLK')[1]),dv('BLK')])]
    for pin,net in [('IO3','SW'),('IO4','DT'),('IO5','CLK'),('IO6','B1'),('IO7','B2'),('IO8','B3'),
                    ('IO9','R_PWM'),('IO10','G_PWM'),('IO11','B_PWM')]:
        runs.append((net,[sm(pin),rp_vias[net]]))
    runs.append(('GND',[led_ground_vias[0],(led_ground_vias[1][0],led_ground_vias[0][1]),led_ground_vias[1]]))
    a,b,gw=power_vias[0][1],power_vias[1][1],power_vias[2][1]
    return runs + [('3V3',[sm('3V3'),a]),('3V3',[b,rp_vias['3V3']]),
                   ('GND',[sm('GND'),(dv('GND')[0],sm('GND')[1]),dv('GND')]),
                   ('GND',[dv('GND'),(dv('GND')[0],GND_Y),rp_vias['GND']]),('GND',[rp_vias['GND'],gw])]


def wire_layout():
    """Rear carrier channels: encoder, keys, LED, power buses and the display pin hops."""
    ep,leg=lambda n:enc_pins[n],btn_pin
    v=rp_vias
    return [
      ('GND',[power_vias[2][1],(9.1,leg(0,0)[1]),leg(0,0)]),
      ('GND',[v['GND'],(27.6,ep('GND')[1]),ep('GND')]),
      ('3V3',[v['3V3'],(v['3V3'][0],ep('+')[1]),ep('+')]),
      ('SW',[v['SW'],(v['SW'][0],ep('SW')[1]),ep('SW')]),
      ('DT',[v['DT'],(v['DT'][0],ep('DT')[1]),ep('DT')]),
      ('CLK',[v['CLK'],(v['CLK'][0],ep('CLK')[1]),ep('CLK')]),
      ('B1',[v['B1'],(11.0,22.5),(25.0,22.5),(25.0,leg(0,3)[1]),leg(0,3)]),
      ('B2',[v['B2'],(12.9,25.0),(43.0,25.0),(43.0,leg(1,3)[1]),leg(1,3)]),
      ('B3',[v['B3'],(v['B3'][0],27.5),(leg(2,3)[0],27.5),leg(2,3)]),
      ('GND',[leg(0,1),leg(1,0)]),('GND',[leg(1,1),leg(2,0)]),
      ('GND',[ep('GND'),(enc_x,led_ground_vias[0][1]),led_ground_vias[0]]),
      ('R_PWM',[v['R_PWM'],(v['R_PWM'][0],62.0),(res_pins['R_IN'][0],62.0),res_pins['R_IN']]),
      ('G_PWM',[v['G_PWM'],(v['G_PWM'][0],64.0),(res_pins['G_IN'][0],64.0),res_pins['G_IN']]),
      ('B_PWM',[v['B_PWM'],(v['B_PWM'][0],66.0),(res_pins['B_IN'][0],66.0),res_pins['B_IN']]),
      ('LED_R',[res_pins['R_OUT'],led_terminals['R']]),
      ('LED_G',[res_pins['G_OUT'],led_terminals['G']]),
      ('LED_B',[res_pins['B_OUT'],led_terminals['B']]),
    ] + [(n,[disp_vias[n],disp_pins[n]]) for n in DISP_PINS if n in disp_vias] + [
      ('3V3',[power_vias[0][1],(V33_X,power_vias[0][1][1]),v33_join]),
      ('3V3',[v33_join,disp_pins['VCC']]),
      ('3V3',[v33_join,power_vias[1][1]])]

def header_rows(front=False):
    """Header strips, every pin fitted: (x, y, pin count, horizontal)."""
    if not front:
        return []
    return [(SM_X, sm_bottom_y, 5, True), (disp_hdr_x, disp_hdr_y, 7, False)] + [(x, sm_hdr_y, 9, False) for x in sm_rows]

def row_len(n):
    return (n - 1) * 2.54 + 2 * PIN_ROW[1]


def leg_ends():
    """Switch legs a run ends on: (leg, the unit direction the run arrives from)."""
    out = []
    for _, pts in wire_layout():
        for a, b in ((pts[0], pts[1]), (pts[-1], pts[-2])):
            for i in range(3):
                for k in range(4):
                    if math.hypot(a[0] - btn_pin(i, k)[0], a[1] - btn_pin(i, k)[1]) < 1e-6:
                        L = math.hypot(b[0] - a[0], b[1] - a[1])
                        out.append((a, ((b[0] - a[0]) / L, (b[1] - a[1]) / L)))
    return out


def run_edges(pts, z, r=WIRE_BEND_R):
    """The centre line of a run as lines and arcs at height z, every bend rounded to r (less where
    a stretch is too short for it)."""
    P = [V(*p) for p in pts] if z is None else [V(x, y, z) for (x, y) in pts]
    edges, start = [], P[0]
    for i in range(1, len(P) - 1):
        u, v = P[i] - P[i - 1], P[i + 1] - P[i]
        lu, lv = u.Length, v.Length
        u.normalize(); v.normalize()
        th = math.acos(max(-1.0, min(1.0, u.dot(v))))
        if th < 1e-6:
            continue
        t = min(r * math.tan(th / 2), lu / 2 if i > 1 else lu, lv / 2 if i < len(P) - 2 else lv)
        rr = t / math.tan(th / 2)
        t1, t2 = P[i] - u * t, P[i] + v * t
        n = u.cross(v).cross(u)
        n.normalize()
        c = t1 + n * rr
        m = (t1 + t2) * 0.5 - c
        m.normalize()
        if (t1 - start).Length > 1e-6:
            edges.append(Part.makeLine(start, t1))
        edges.append(Part.Arc(t1, c + m * rr, t2).toShape())
        start = t2
    if (P[-1] - start).Length > 1e-6:
        edges.append(Part.makeLine(start, P[-1]))
    return edges


def band_face(pts, w, z):
    """A channel's outline at height z: the run's centre line grown to width w (round ends)."""
    if len(pts) == 2:  # one straight stretch: no plane for the 2D offset, so a stadium by hand
        (ax, ay), (bx, by) = pts
        L = math.hypot(bx - ax, by - ay)
        f = Part.Face(Part.makePolygon([V(0, -w / 2, z), V(L, -w / 2, z), V(L, w / 2, z), V(0, w / 2, z), V(0, -w / 2, z)]))
        f = f.fuse([Part.Face(Part.Wire(Part.makeCircle(w / 2, V(t, 0, z)))) for t in (0, L)]).removeSplitter().Faces[0]
        f.rotate(V(0, 0, z), V(0, 0, 1), math.degrees(math.atan2(by - ay, bx - ax)))
        f.translate(V(ax, ay, 0))
        return f
    return Part.Face(Part.Wire(run_edges(pts, z)).makeOffset2D(w / 2, 0, False, False))


def band(pts, w, z0, z1):
    return band_face(pts, w, z0).extrude(V(0, 0, z1 - z0))


def wire_crossings(runs=None):
    """Where two runs of different nets cross: (x, y, run on top, run that dips, its direction)."""
    runs = wire_layout() if runs is None else runs
    segs = [(k, a, b) for k, (_, pts) in enumerate(runs) for a, b in zip(pts, pts[1:])]
    out = []
    for i, (ka, a0, a1) in enumerate(segs):
        for (kb, b0, b1) in segs[i + 1:]:
            if ka == kb or runs[ka][0] == runs[kb][0]:
                continue
            d = (a1[0] - a0[0]) * (b1[1] - b0[1]) - (a1[1] - a0[1]) * (b1[0] - b0[0])
            if abs(d) < 1e-9:
                continue
            s = ((b0[0] - a0[0]) * (b1[1] - b0[1]) - (b0[1] - a0[1]) * (b1[0] - b0[0])) / d
            t = ((b0[0] - a0[0]) * (a1[1] - a0[1]) - (b0[1] - a0[1]) * (a1[0] - a0[0])) / d
            if 1e-6 < s < 1 - 1e-6 and 1e-6 < t < 1 - 1e-6:  # the later run dips under the earlier
                L = math.hypot(b1[0] - b0[0], b1[1] - b0[1])
                out.append((a0[0] + s * (a1[0] - a0[0]), a0[1] + s * (a1[1] - a0[1]), ka, kb,
                            ((b1[0] - b0[0]) / L, (b1[1] - b0[1]) / L)))
    return out


def crossing_pockets(runs, front, z0, z1):
    cuts=[]
    for (x,y,_,_,(dx,dy)) in wire_crossings(runs):
        cuts.append(place(box(-WIRE_CROSS[0]/2-WIRE_D,-WIRE_CH[0]/2,z0,
                              WIRE_CROSS[0]/2+WIRE_D,WIRE_CH[0]/2,z1),x,y,math.degrees(math.atan2(dy,dx))))
    return cuts


def wire_cuts():
    """Both faces retain at least 0.4 mm where ordinary and dipped channels overlap."""
    w,dep=WIRE_CH
    rear=wire_layout();front=front_layout()
    cuts=[band(pts,w,C_BACK-1,C_BACK+dep) for _,pts in rear]
    cuts += crossing_pockets(rear,False,C_BACK-1,C_BACK+WIRE_CROSS[1])
    cuts += kapton_squares(C_BACK-1,C_BACK+dep+KAPTON[1],rear)
    cuts += [band([led_native_pins[n],led_terminals[n]],LED_FAN_WIDTH,C_BACK-1,C_BACK+LED_FAN_DEPTH)
             for n in ('R','C','G','B')]
    cuts += [band(pts,w,C_FRONT-dep,C_FRONT+1) for _,pts in front]
    cuts += crossing_pockets(front,True,C_FRONT-WIRE_CROSS[1],C_FRONT+1)
    cuts += kapton_squares(C_FRONT-dep-KAPTON[1],C_FRONT+1,front)
    for ((x,y),(dx,dy)) in leg_ends():
        cuts.append(place(box(0,-LEG_POCKET[0]/2,C_BACK-1,LEG_POCKET[1],LEG_POCKET[0]/2,C_BACK+dep),
                          x,y,math.degrees(math.atan2(dy,dx))))
    return cuts

def check_wires(runs=None, front=False):
    """Narrowest plastic left between a channel and anything that is not its own: another net's
    channel (where two runs cross, that pair is left out), a hole, pin row or leg pocket it does not
    end in, the board edge. Returns [(mm, net, what)], smallest first."""
    runs = wire_layout() if runs is None else runs
    zt = C_BACK
    chans = [band_face(pts, WIRE_CH[0], zt) for _, pts in runs]
    crossed = {(min(a, b), max(a, b)) for (_, _, a, b, _) in wire_crossings(runs)}
    holes = [((x, y), HOLE_D) for d in (enc_pins, res_pins, disp_pins) for (x, y) in d.values()]
    holes += [(p, DISP_VIA_D) for p in disp_vias.values()]
    holes += [(p, RP_PIN_FIT_D) for p in sm_pins.values()]
    holes += [(p,1.2) for p in list(rp_vias.values())+[q for _,q in power_vias]]
    holes += [(p,LED_LEAD_HOLE_D) for p in led_native_pins.values()]
    holes += [(p,1.2) for p in led_ground_vias]
    holes += [(btn_pin(i, k), SW_HOLE_D) for i in range(3) for k in range(4)]
    holes += [(p, SPIGOT_HOLE_D) for p in POSTS] + [(KNOB_C, EC11_BUSH_D + 2 * EC11_HOLE_CLR)]
    pockets = [(p, place(Part.Face(Part.makePolygon([V(0, -LEG_POCKET[0] / 2, zt), V(LEG_POCKET[1], -LEG_POCKET[0] / 2, zt),
                                                     V(LEG_POCKET[1], LEG_POCKET[0] / 2, zt), V(0, LEG_POCKET[0] / 2, zt),
                                                     V(0, -LEG_POCKET[0] / 2, zt)])), p[0], p[1], math.degrees(math.atan2(d[1], d[0]))))
               for (p, d) in leg_ends()]
    board = carrier_outline().cut(carrier_through_cuts()[:len(HOOKS)])
    board = [f for f in board.Faces if abs(f.normalAt(0, 0).z + 1) < 1e-6][0]
    out = []
    for k, ((net, pts), ch) in enumerate(zip(runs, chans)):
        ends = [pts[0], pts[-1]]
        if front:
            ends += [disp_pins[n] for n,p in disp_vias.items() if p in ends]
        own = lambda p: any(math.hypot(p[0] - e[0], p[1] - e[1]) < 1e-6 for e in ends)
        for j in range(k + 1, len(runs)):
            if runs[j][0] != net and (k, j) not in crossed:
                out.append((ch.distToShape(chans[j])[0], net, "channel " + runs[j][0]))
        for (p, dia) in holes:
            if not own(p):
                out.append((ch.distToShape(Part.Vertex(V(p[0], p[1], zt)))[0] - dia / 2, net, "hole (%.1f, %.1f)" % p))
        mine = []
        for (x, y, n, horizontal) in header_rows(front):
            w, h = (row_len(n), PIN_ROW[0]) if horizontal else (PIN_ROW[0], row_len(n))
            tr = Part.Face(Part.makePolygon([V(x + sx * w / 2, y + sy * h / 2, zt)
                                             for (sx, sy) in ((-1, -1), (1, -1), (1, 1), (-1, 1), (-1, -1))]))
            if any((abs(e[1] - y) < 1e-6 and abs(e[0] - x) <= n * 1.27) if horizontal
                   else (abs(e[0] - x) < 1e-6 and abs(e[1] - y) <= n * 1.27) for e in ends):
                mine.append(tr)
            else:
                out.append((ch.distToShape(tr)[0], net, "pin row x %.1f" % x))
        for (p, pk) in pockets:
            if not own(p):
                out.append((ch.distToShape(pk)[0], net, "leg pocket (%.1f, %.1f)" % p))
        ch = ch.cut(mine) if mine else ch  # (a pin row may run out of the board edge)
        out.append((ch.distToShape(board.OuterWire)[0] if ch.cut(board).Area < 1e-3 else 0.0, net, "board edge"))
    return sorted(out, key=lambda t: t[0])


def peg(x, y, hole_d, z0, z1, clr=PEG_CLR):
    """Locating peg with a 0.3 mm lead-in chamfer at the tip."""
    d = hole_d - 2 * clr
    body = cyl(d, x, y, z0, z1 - 0.3)
    tip = Part.makeCone(d / 2, d / 2 - 0.3, 0.3, V(x, y, z1 - 0.3), V(0, 0, 1))
    return body.fuse(tip)


def make_carrier():
    """The printed PCB. Prints back side down; every holding feature points up from the front."""
    c = carrier_outline()
    cuts = carrier_through_cuts()
    c = c.cut(cuts)

    adds = []
    for (x, y) in disp_holes:  # display standoffs with pegs through its four corner holes
        adds.append(cyl(DISP_STANDOFF_D, x, y, C_FRONT - 0.01, dpcb_z1))
        adds.append(peg(x, y, DISP_HOLE_D, dpcb_z1 - 0.01, glass_z1 + PEG_PROUD))
    c = c.fuse(adds)
    # keep the display header clear of the standoffs
    c = c.cut(cbox(disp_hdr_x, disp_hdr_y, 2.6, 7 * 2.54 + 1.0, C_FRONT, C_FRONT + 6))
    c = c.cut(wire_cuts())
    return c.removeSplitter()


def make_knob():
    z0, z1 = KNOB_GAP, KNOB_GAP + KNOB_H
    cx, cy = KNOB_C
    k = cyl(KNOB_D, cx, cy, z0, z1)
    try:
        top = [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle" and abs(e.BoundBox.ZMin - z1) < 1e-6]
        bot = [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle" and abs(e.BoundBox.ZMin - z0) < 1e-6]
        k = k.makeChamfer(KNOB_CHAMFER, top)  # chamfers, not rounds: the top prints on the bed
        k = k.makeChamfer(0.5, [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle"
                                and abs(e.BoundBox.ZMin - z0) < 1e-6 and e.Curve.Radius > KNOB_D / 2 - 0.1])
    except Exception as exc:
        print("knob chamfer skipped:", exc)
    r = KNOB_D / 2
    flutes = []
    for i in range(KNOB_FLUTES):
        a = 2 * math.pi * i / KNOB_FLUTES
        flutes.append(cyl(KNOB_FLUTE_D, cx + r * math.cos(a), cy + r * math.sin(a), z0 - 1, z1 - KNOB_CHAMFER - 0.6))  # out the skirt edge
    bush_top = HW040_FRONT + EC11_BODY_H + EC11_BUSH_L
    shaft_top = HW040_FRONT + EC11_SHAFT_H
    hub_z0 = max(bush_top + 0.5, z0)  # the bushing ends inside the face hole
    # hollow: skirt, top, and a hub round the shaft tied to the skirt by ribs. Prints top down,
    # so all of it grows up from the top with no overhang.
    inside = cyl(KNOB_D - 2 * KNOB_WALL, cx, cy, z0 - 1, z1 - KNOB_TOP_T)
    keep = [cyl(KNOB_HUB_D, cx, cy, hub_z0, z1)]
    for i in range(KNOB_RIBS):
        rib = box(0, -KNOB_RIB_T / 2, hub_z0, KNOB_D / 2 - KNOB_WALL + 0.1, KNOB_RIB_T / 2, z1)
        keep.append(place(rib, cx, cy, 45.0 + i * 360.0 / KNOB_RIBS))
    cuts = flutes + [inside.cut(keep)]
    # bore: round over the shaft's round part, D-shaped over its flat
    bore = cyl(EC11_SHAFT_D + 0.25, cx, cy, hub_z0 - 1, shaft_top + KNOB_TIP_CLR)  # snug once FDM shrinks it
    flat = box(cx - 5, cy + EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, shaft_top - EC11_FLAT_L + 0.5, cx + 5, cy + 5, shaft_top + 1)
    cuts.append(bore.cut(flat))
    cuts += [cyl(2.0, x, y, z1 - 1.2, z1 + 1) for x, y in KNOB_CAP_PEGS]
    return k.cut(cuts).removeSplitter()


def cap_grip(z_ceil, r_wall):
    """The grip round the stem head, round the origin, a "+" of ribs out to the cap wall at r_wall:
    two wide snap plates (long sides, along x), two short snap plates (along y), each ribbed down
    CAP_RIB_SHARE of its height, and the stop post. Every plate's lip snaps in under the head."""
    z_k = SW_PLUNGER_TOP - SW_HEAD_H - 0.05 - CAP_SNAP_DROP  # catch face, just under the head
    land = 0.1

    def plate(w, t, clr, reach):
        """A snap plate on the +x side: free below its rib, a lip under the head, a 45° lead-in below
        it, ending 0.1 above the switch's round platform."""
        r_i = SW_HEAD / 2 + clr
        r_o, r_l = r_i + t, SW_HEAD / 2 - reach
        z_b = max(z_k - land - (r_i - r_l), C_FRONT + SW_BOSS_H + 0.1)
        prof = [(r_o, z_ceil + 0.01), (r_i, z_ceil + 0.01), (r_i, z_k), (r_l, z_k), (r_l, z_k - land),
                (r_i, z_b), (r_o, z_b)]
        f = Part.Face(Part.makePolygon([V(px, 0, pz) for (px, pz) in prof + [prof[0]]])).extrude(V(0, w, 0))
        f.translate(V(0, -w / 2, 0))
        return f, r_o, z_b

    rt = CAP_RIB_T
    w, t = CAP_FINGER
    f, r_o, z_b = plate(w, t, CAP_SNAP_CLR, CAP_SNAP)
    f = f.fuse(box(r_o - 0.01, -rt / 2, z_ceil - CAP_RIB_SHARE[0] * (z_ceil - z_b), r_wall + 0.3, rt / 2, z_ceil + 0.01))
    parts = [place(f, 0, 0, k * 180.0) for k in range(2)]
    sw, st, sc = CAP_SIDE
    f, r_o, z_b = plate(sw, st, sc, CAP_SIDE_SNAP)
    f = f.fuse(box(r_o - 0.01, -rt / 2, z_ceil - CAP_RIB_SHARE[1] * (z_ceil - z_b), r_wall + 0.3, rt / 2, z_ceil + 0.01))
    parts += [place(f, 0, 0, k * 90.0) for k in (1, 3)]
    parts.append(cyl(CAP_STOP_D, 0, 0, SW_PLUNGER_TOP, z_ceil + 0.01))
    rd, rh = CAP_STOP_ROOT  # the post's foot: a cone into the cap top
    parts.append(Part.makeCone(CAP_STOP_D / 2, rd / 2, rh + 0.01, V(0, 0, z_ceil - rh), V(0, 0, 1)))
    return fuse_all(parts)


def make_cap(i):
    """Hollow cap with a flat top, open underneath, that clicks onto the switch stem (cap_grip).
    Prints top down: the top comes off the bed flat, the grip grows up from it."""
    x, y, a = BTNS[i]
    z_flange = -FACE_T - 0.3 - BTN_FLANGE_T
    z_ceil = BTN_CAP_PROUD - CAP_TOP_T
    # straight up through the face and the visor, then tapering in like a keycap
    z_taper = VISOR_T + 0.5
    # the top edge (on the bed) is a 45° chamfer lofted in as its own section: makeChamfer's faces came
    # out 47-54° from vertical on the leaning sides and the corners
    ch = CAP_TOP_EDGE
    t = (BTN_CAP_PROUD - ch - z_taper) / (BTN_CAP_PROUD - z_taper)
    w_c, r_c = BTN_CAP + (BTN_CAP_TOP - BTN_CAP) * t, 1.2 + (2.0 - 1.2) * t
    body = rr_prism(BTN_CAP, BTN_CAP, 1.2, x, y, z_flange, z_taper + 0.01, a).fuse(place(Part.makeLoft(
        [rr_wire(BTN_CAP, BTN_CAP, 1.2, z_taper), rr_wire(w_c, w_c, r_c, BTN_CAP_PROUD - ch),
         rr_wire(w_c - 2 * ch, w_c - 2 * ch, r_c - ch, BTN_CAP_PROUD)], True, True), x, y, a))
    # flange: a 0.3 mm land, then 45° in to the body (the cap prints top down: no flat ledge)
    ch = (BTN_FLANGE - BTN_CAP) / 2
    flange = Part.makeLoft([rr_wire(BTN_FLANGE, BTN_FLANGE, 1.2 + ch, z_flange),  # (corners 45° too)
                            rr_wire(BTN_FLANGE, BTN_FLANGE, 1.2 + ch, z_flange + BTN_FLANGE_T - ch),
                            rr_wire(BTN_CAP, BTN_CAP, 1.2, z_flange + BTN_FLANGE_T)], True, True)
    flange = place(flange, x, y, a)
    cap = body.fuse(flange)
    inner = BTN_CAP_TOP - 2 * CAP_WALL - 0.6
    cap = cap.cut(rr_prism(inner, inner, 0.6, x, y, z_flange - 1, z_ceil, a))  # hollow
    cap = cap.fuse(place(cap_grip(z_ceil, inner / 2), x, y, a))
    return cap.removeSplitter()


# Stand-ins for the bought parts (interference checks and renders only, never printed)


def dummy_encoder():
    f = HW040_FRONT
    pcb = cbox(hw_c[0], hw_c[1], HW040[0], HW040[1], hw_back, f)
    pcb = pcb.cut([cyl(HW040_HOLE_D, x, y, f - 5, f + 1) for (x, y) in hw_holes])
    body = cbox(KNOB_C[0], KNOB_C[1], EC11_BODY[0], EC11_BODY[1], f, f + EC11_BODY_H)
    bush = cyl(EC11_BUSH_D, KNOB_C[0], KNOB_C[1], f + EC11_BODY_H, f + EC11_BODY_H + EC11_BUSH_L)
    shaft = cyl(EC11_SHAFT_D, KNOB_C[0], KNOB_C[1], f + EC11_BODY_H + EC11_BUSH_L, f + EC11_SHAFT_H)
    shaft = shaft.cut(box(KNOB_C[0] - 5, KNOB_C[1] + EC11_SHAFT_D / 2 - EC11_FLAT, f + EC11_SHAFT_H - EC11_FLAT_L,
                          KNOB_C[0] + 5, KNOB_C[1] + 5, f + EC11_SHAFT_H + 1))
    hdr = cbox(hw_hdr[0], hw_hdr[1], 2.5, 5 * 2.54, f, f + 2.5)
    pins = fuse_all([cbox(x, y, 0.64, 0.64, f, f + HW040_PIN_L) for (x, y) in enc_pins.values()])  # straightened
    # on its back: EC11 legs and resistors, header solder joints
    kx0, kx1, ky0, ky1 = HW040_BACK_KEEPOUT
    legs = box(KNOB_C[0] - kx1, KNOB_C[1] - ky1, hw_back - HW040_BACK_PARTS, KNOB_C[0] - kx0, KNOB_C[1] - ky0, hw_back + 0.01)
    joints = cbox(hw_hdr[0], hw_hdr[1], 1.6, 4 * 2.54 + 1.8, hw_back - HW040_BACK_PARTS, hw_back + 0.01)
    return fuse_all([pcb, body, bush, shaft, hdr, pins, legs, joints])


def dummy_encoder_nut():
    """The EC11's washer and M7 nut on the bushing, on the printed PCB's front."""
    af, t, wt = EC11_NUT
    washer = cyl(af + 1.0, KNOB_C[0], KNOB_C[1], C_FRONT, C_FRONT + wt)
    nut = Part.Face(Part.makePolygon([V(KNOB_C[0] + af / math.sqrt(3) * math.cos(math.radians(60 * k)),
                                        KNOB_C[1] + af / math.sqrt(3) * math.sin(math.radians(60 * k)), C_FRONT + wt)
                                      for k in range(7)])).extrude(V(0, 0, t))
    return washer.fuse(nut).cut(cyl(EC11_BUSH_D, KNOB_C[0], KNOB_C[1], C_FRONT - 1, C_FRONT + wt + t + 1))


def dummy_display():
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], glass_z1, glass_z0)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], dpcb_z1, glass_z1)
    pcb = pcb.cut([cyl(DISP_HOLE_D, x, y, dpcb_z1 - 1, glass_z1 + 1) for (x, y) in disp_holes])
    hdr = cbox(disp_hdr_x, disp_hdr_y, 2.5, 7 * 2.54, dpcb_z1 - 2.5, dpcb_z1)
    pins = fuse_all([cbox(x, y, 0.64, 0.64, dpcb_z1 - 2.5 - DISP_PIN_L, dpcb_z1 - 2.5) for (x, y) in disp_pins.values()])
    return fuse_all([glass, pcb, hdr, pins])


def dummy_switch(i):
    x, y, a = BTNS[i]
    body = rr_prism(SW_BODY, SW_BODY, 0.3, x, y, C_FRONT, C_FRONT + SW_BODY_H, a)
    boss = cyl(SW_BOSS_D, x, y, C_FRONT + SW_BODY_H - 0.01, C_FRONT + SW_BOSS_H)
    neck = cbox(x, y, SW_NECK, SW_NECK, C_FRONT + SW_BOSS_H - 0.01, SW_PLUNGER_TOP - SW_HEAD_H + 0.01, a)
    head = cbox(x, y, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H, SW_PLUNGER_TOP, a)
    head = head.cut(cyl(1.6, x, y, SW_PLUNGER_TOP - 1.2, SW_PLUNGER_TOP + 1))
    legs = [cbox(*btn_pin(i, k), SW_LEG[0], SW_LEG[1], C_FRONT - SW_LEG[2], C_FRONT + 0.01, a) for k in range(4)]
    return fuse_all([body, boss, neck, head] + legs)  # (legs straight, as pushed through)


def dummy_rp2040_zero():
    """PCB, components, and headers physically rotated onto the underside."""
    lo, hi = rp_ref_lo, rp_ref_hi
    pcb = cbox(*sm_c, SM[0], SM[1], lo, hi)
    usb = cbox(SM_X, SM_USB_END - SM_USB[1] / 2 + SM_USB_OVERHANG, *SM_USB[:2], hi - 0.91, hi + SM_USB[2])
    btns = [cbox(SM_X + dx, SM_USB_END - 16, *SM_BTN[:2], hi, hi + SM_BTN[2]) for dx in (-SM_BTN_DX, SM_BTN_DX)]
    chip = cbox(SM_X, SM_USB_END - 12.1, 7.04, 7.04, lo - 0.9, lo)
    crystal = cbox(SM_X, SM_USB_END - 18.74, 3.95, 3.1, lo - 1.0, lo)
    regulator = cbox(SM_X, SM_USB_END - 3.14, 2.56, 2.98, lo - 0.7, lo)
    led = cbox(SM_X, SM_USB_END - 16, 2.4, 2.4, hi, hi + 1.6)
    hdr = [cbox(x, sm_hdr_y, SM_HDR[0], SM_HDR[1], C_FRONT, lo) for x in sm_rows]
    hdr.append(cbox(SM_X, sm_bottom_y, 5 * 2.54, SM_HDR[0], C_FRONT, lo))
    pins = [cbox(x, y, 0.64, 0.64, C_FRONT - SM_HDR[3], hi + 0.5) for n,(x,y) in sm_native_pins.items()]
    board = fuse_all([pcb, usb, chip, crystal, regulator, led] + btns + hdr + pins)
    board.rotate(V(SM_X, 0, RP_FLIP_Z), V(0, 1, 0), 180)
    return board


def rgb_led_lead(name):
    """Trimmed lead, bent into the accessible rear fan groove."""
    x,y=led_native_pins[name];tx,ty=led_terminals[name]
    a=V(x,y,LED_BASE_Z);b=V(x,y,C_BACK-0.05)
    c=V(x+(tx-x)/8.0,y+1.0,C_BACK+0.35);d=V(tx,ty,C_BACK+0.35)
    pieces=[Part.makeCylinder(LED_LEAD_D/2,(v-u).Length,u,v-u) for u,v in ((a,b),(b,c),(c,d))]
    pieces += [Part.makeSphere(LED_LEAD_D/2,q) for q in (b,c)]
    return fuse_all(pieces)


def dummy_rgb_led():
    x,y=LED_C
    body=cyl(LED_D,x,y,LED_BASE_Z+1,LED_TIP_Z-LED_D/2)
    dome=Part.makeSphere(LED_D/2,V(x,y,LED_TIP_Z-LED_D/2)).common(
        box(x-3,y-3,LED_TIP_Z-LED_D/2,x+3,y+3,LED_TIP_Z+0.1))
    flange=cyl(LED_FLANGE_D,x,y,LED_BASE_Z,LED_BASE_Z+1)
    return fuse_all([body,dome,flange]+[rgb_led_lead(n) for n in ('R','C','G','B')])


def dummy_resistor(i):
    x,y=RES_C[i]
    body=Part.makeCylinder(0.9,3.2,V(x,y-1.6,C_FRONT+1.1),V(0,1,0))
    leads=[]
    for side in (-1,1):
        p=V(x,y+side*1.6,C_FRONT+1.1)
        q=V(x,y+side*RES_PITCH/2,C_FRONT+1.1)
        r=V(q.x,q.y,C_BACK-3)
        leads += [Part.makeCylinder(0.25,(q-p).Length,p,q-p),Part.makeCylinder(0.25,(r-q).Length,q,r-q)]
    return fuse_all([body]+leads)


def shared_offsets(runs=None):
    """Side-by-side offsets for runs of one net that share a channel: {(run, segment): mm}, one
    wire to each side of the channel's centre line."""
    runs = wire_layout() if runs is None else runs
    segs = [(k, i, a, b) for k, (_, pts) in enumerate(runs) for i, (a, b) in enumerate(zip(pts, pts[1:]))]
    off = {}
    for n, (k1, i1, a1, b1) in enumerate(segs):
        for (k2, i2, a2, b2) in segs[n + 1:]:
            if k1 == k2 or runs[k1][0] != runs[k2][0]:
                continue
            d1 = (b1[0] - a1[0], b1[1] - a1[1])
            L1 = math.hypot(*d1)
            u = (d1[0] / L1, d1[1] / L1)
            cross = lambda p: (p[0] - a1[0]) * u[1] - (p[1] - a1[1]) * u[0]
            if abs(cross(a2)) > 1e-6 or abs(cross(b2)) > 1e-6:
                continue  # not on the same line
            t = sorted([(a2[0] - a1[0]) * u[0] + (a2[1] - a1[1]) * u[1], (b2[0] - a1[0]) * u[0] + (b2[1] - a1[1]) * u[1]])
            if min(L1, t[1]) - max(0.0, t[0]) > 0.5:  # they overlap: one wire each side
                s = (WIRE_CH[0] - WIRE_D) / 2 - 0.05
                off[(k1, i1)], off[(k2, i2)] = -s, s
    return off


def offset_run(pts, k, off):
    """A run's points with its shared segments moved sideways (see shared_offsets)."""
    P = [list(p) for p in pts]
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        d = off.get((k, i))
        if d:
            L = math.hypot(b[0] - a[0], b[1] - a[1])
            nx, ny = -(b[1] - a[1]) / L * d, (b[0] - a[0]) / L * d
            for p in (P[i], P[i + 1]):
                p[0] += nx
                p[1] += ny
    return [tuple(p) for p in P]


def dummy_wire():
    """Two supported carrier-face routes, local via drops, and terminal wraps."""
    parts=[]
    groups={}
    def add(net,shape):
        parts.append(shape)
        groups.setdefault(net,[]).append(shape)
    r=WIRE_D/2
    for front in (False,True):
        runs=front_layout() if front else wire_layout()
        zc=(C_FRONT-WIRE_CH[1]+r) if front else (C_BACK+WIRE_CH[1]-r)
        zd=(C_FRONT-WIRE_CROSS[1]+r) if front else (C_BACK+WIRE_CROSS[1]-r)
        off=shared_offsets(runs)
        dips={}
        for x,y,_,kb,d in wire_crossings(runs):
            dips.setdefault(kb,[]).append((x,y,d))
        physical=(list(sm_pins.values()) if front else
                  list(enc_pins.values())+list(res_pins.values())+list(disp_pins.values()))
        for k,(net,pts) in enumerate(runs):
            P=offset_run(pts,k,off)
            path=Part.Wire(run_edges(P,zc));e=path.OrderedEdges[0]
            profile=Part.Wire(Part.makeCircle(r,e.Vertexes[0].Point,e.tangentAt(e.FirstParameter)))
            wire=path.makePipeShell([profile],True,True)
            terminals=[cyl(1.0,x,y,zc-1,zc+1) for x,y in (pts[0],pts[-1]) if (x,y) in physical]
            if terminals:wire=wire.cut(terminals)
            for x,y,(dx,dy) in dips.get(k,[]):
                j=min(range(len(pts)-1),key=lambda i:Part.LineSegment(V(*pts[i],0),V(*pts[i+1],0)).toShape().distToShape(Part.Vertex(V(x,y,0)))[0])
                px,py=x+(P[j][0]-pts[j][0]),y+(P[j][1]-pts[j][1]);h=WIRE_CROSS[0]/2
                wire=wire.cut(place(box(-h,-1,zc-1,h,1,zc+1),px,py,math.degrees(math.atan2(dy,dx))))
                pp=[V(px+dx*t,py+dy*t,z) for t,z in ((-h,zc),(-h+0.8,zd),(h-0.8,zd),(h,zc))]
                for u,v in zip(pp,pp[1:]):wire=wire.fuse(Part.makeCylinder(r,(v-u).Length,u,v-u))
                wire=wire.fuse([Part.makeSphere(r,q) for q in pp])
            # A dip may reach a terminal; trim its end caps around the physical pin too.
            if terminals:wire=wire.cut(terminals)
            add(net,wire)
    rear_z=C_BACK+WIRE_CH[1]-r
    front_z=C_FRONT-WIRE_CH[1]+r
    for net,(x,y) in rp_vias.items():
        add(net,cyl(WIRE_D,x,y,rear_z,front_z))
    for net,(x,y) in power_vias:add(net,cyl(WIRE_D,x,y,rear_z,front_z))
    for i,(x,y) in enumerate(led_ground_vias):
        add('GND',cyl(WIRE_D,x,y,rear_z if i==0 else C_BACK-0.8,front_z))
    ring=lambda x,y,z0,z1:cyl(WRAP_OD,x,y,z0,z1).cut(cyl(0.95,x,y,z0-1,z1+1))
    tall=WRAP_TURNS*WIRE_D
    for name,(x,y) in sm_pins.items():
        if name in RP_NETS:add(RP_NETS[name],ring(x,y,C_FRONT+0.3,C_FRONT+0.3+tall))
    for name,(x,y) in disp_pins.items():
        net={'GND':'GND','VCC':'3V3'}.get(name,name)
        if name in disp_vias:add(net,cyl(WIRE_D,*disp_vias[name],rear_z,front_z))
        add(net,ring(x,y,C_BACK-0.3-tall,C_BACK-0.3))
    for name,(x,y) in enc_pins.items():
        add({'CLK':'CLK','DT':'DT','SW':'SW','+':'3V3','GND':'GND'}[name],
            ring(x,y,C_BACK-0.5-tall,C_BACK-0.5))
    for name,(x,y) in res_pins.items():
        color=name[0];net=color+'_PWM' if name.endswith('IN') else 'LED_'+color
        add(net,cyl(WRAP_OD,x,y,C_BACK-1.5,C_BACK-0.3).cut(cyl(0.6,x,y,C_BACK-2,C_BACK)))
    WIRE_BY_NET.clear()
    WIRE_BY_NET.update({net:Part.makeCompound(shapes) for net,shapes in groups.items()})
    return Part.makeCompound(parts)


def kapton_squares(z0,z1,runs=None):
    """Squares span crossings; a normal trace and dipped trace have 0.10 mm air gap."""
    half=KAPTON[0]/2
    squares=[place(box(-half,-half,z0,half,half,z1),x,y,math.degrees(math.atan2(dy,dx)))
             for x,y,_,_,(dx,dy) in wire_crossings(runs)]
    return [fuse_all(squares)] if squares else []


def dummy_kapton():
    rear=C_BACK+WIRE_CH[1]
    front=C_FRONT-WIRE_CH[1]-KAPTON[1]
    return Part.makeCompound(kapton_squares(rear,rear+KAPTON[1],wire_layout())+
                             kapton_squares(front,front+KAPTON[1],front_layout()))


def dummy_wire_straps():
    """A few wider removable tape strips hold adjacent wires, one set per face."""
    rear=[cbox(11,60,8,2,C_BACK-0.08,C_BACK-0.02),
          cbox(11,35,8,2,C_BACK-0.08,C_BACK-0.02),
          cbox(22.8,60,10,2,C_BACK-0.08,C_BACK-0.02),
          cbox(22.8,52,10,2,C_BACK-0.08,C_BACK-0.02)]
    front=[cbox(56.0,79.0,2,13,C_FRONT+0.02,C_FRONT+0.08)]
    return Part.makeCompound(rear+front)


def check_led_fan():
    """Check local lead-hole/fan webs and clearance from unrelated rear channels."""
    names=('R','C','G','B');nets=('LED_R','GND','LED_G','LED_B');z=C_BACK
    fans=[band_face([led_native_pins[n],led_terminals[n]],LED_FAN_WIDTH,z) for n in names]
    runs=wire_layout();channels=[band_face(points,WIRE_CH[0],z) for _,points in runs]
    issues=[]
    if 1.27-LED_LEAD_HOLE_D < 0.4-1e-6:
        issues.append(('lead-hole web',1.27-LED_LEAD_HOLE_D))
    for i,name in enumerate(names):
        for j in range(i+1,len(names)):
            gap=fans[i].distToShape(fans[j])[0]
            if gap < 0.4-1e-6:issues.append(('fan web',name,names[j],gap))
        for j,(net,_) in enumerate(runs):
            if net!=nets[i]:
                gap=fans[i].distToShape(channels[j])[0]
                if gap < WIRE_RIDGE_MIN-1e-6:issues.append(('wire',name,net,gap))
        holes=[(q,LED_LEAD_HOLE_D,0.4,'LED_'+n) for n,q in led_native_pins.items() if n!=name]
        holes += [(q,1.2,WIRE_RIDGE_MIN,'via') for q in list(rp_vias.values())+led_ground_vias+[p for _,p in power_vias] if q!=led_terminals[name]]
        holes += [(q,HOLE_D,WIRE_RIDGE_MIN,'pin') for group in (enc_pins,res_pins,disp_pins) for q in group.values()]
        holes += [(q,DISP_VIA_D,WIRE_RIDGE_MIN,'display via') for q in disp_vias.values()]
        holes += [(p,RP_PIN_FIT_D,WIRE_RIDGE_MIN,'RP') for p in sm_pins.values()]
        holes += [(q,SPIGOT_HOLE_D,WIRE_RIDGE_MIN,'post') for q in POSTS]
        holes += [(KNOB_C,EC11_BUSH_D+2*EC11_HOLE_CLR,WIRE_RIDGE_MIN,'shaft')]
        for (x,y),dia,limit,label in holes:
            gap=fans[i].distToShape(Part.Vertex(V(x,y,z)))[0]-dia/2
            if gap < limit-1e-6:issues.append(('hole',name,label,(x,y),gap))
    return issues


def check_twoface_web():
    """Deep pockets on opposing faces must not overlap; other intersections retain >=0.4 mm."""
    dip=WIRE_CROSS[1] if wire_crossings() or wire_crossings(front_layout()) else WIRE_CH[1]  # no dips without crossings
    web=C_T-WIRE_CH[1]-dip
    if web < 0.4-1e-6:
        raise ValueError('opposite-face channels leave less than 0.4 mm web')
    rear=crossing_pockets(wire_layout(),False,0,1)
    front=crossing_pockets(front_layout(),True,0,1)
    rear_tape=kapton_squares(0,1,wire_layout())
    front_tape=kapton_squares(0,1,front_layout())
    rear_fans=[band([led_native_pins[n],led_terminals[n]],LED_FAN_WIDTH,0,1)
               for n in ('R','C','G','B')]
    if C_T-WIRE_CROSS[1]-LED_FAN_DEPTH < 0.4-1e-6:
        for a in front:
            for b in rear_fans:
                if a.BoundBox.intersect(b.BoundBox) and a.common(b).Volume>1e-7:
                    raise ValueError('front crossing dip overlaps the LED fan')
    for one,other in ((rear,front),(rear,front_tape),(front,rear_tape)):
        for a in one:
            for b in other:
                if a.BoundBox.intersect(b.BoundBox) and a.common(b).Volume>1e-7:
                    raise ValueError('opposite-face deep or tape pockets overlap')
    return web

# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}
WIRE_BY_NET = {}

COLORS = {
    "Shell": (0.07, 0.07, 0.08), "DisplayCowl": (0.07, 0.07, 0.08),
    "DisplayBezel": (0.90, 0.90, 0.88), "KnobCap": (0.90, 0.90, 0.88),
    "HaloDiffuser": (0.90, 0.90, 0.88), "HaloReflector": (0.90, 0.90, 0.88), "PinFitCoupon": (0.15, 0.15, 0.16),
    "BackPlate": (0.90, 0.90, 0.88), "ScrewFeet": (0.90, 0.90, 0.88),
    "Carrier": (0.15, 0.15, 0.16), "Knob": (0.07, 0.07, 0.08),
    "CapLeft": (0.90, 0.90, 0.88), "CapMiddle": (0.90, 0.90, 0.88), "CapRight": (0.90, 0.90, 0.88),
    "Dummy": (0.30, 0.45, 0.85), "Dummy_Wire": (0.85, 0.45, 0.20), "Dummy_Kapton": (0.95, 0.70, 0.15), "Dummy_WireStraps": (0.95, 0.70, 0.15),
}



def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["DisplayCowl"] = make_cowl()
    PRINTED["DisplayBezel"] = make_bezel()
    PRINTED["KnobCap"] = make_knob_cap()
    PRINTED["HaloDiffuser"] = make_halo()
    PRINTED["HaloReflector"] = make_halo_reflector()
    PRINTED["PinFitCoupon"] = make_pin_coupon()
    PRINTED["BackPlate"] = make_back_plate()
    PRINTED["ScrewFeet"] = make_screw_feet()
    PRINTED["Carrier"] = make_carrier()
    PRINTED["Knob"] = make_knob()
    for i, n in enumerate(["CapLeft", "CapMiddle", "CapRight"]):
        PRINTED[n] = make_cap(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_EncoderNut"] = dummy_encoder_nut()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_RP2040_Zero"] = dummy_rp2040_zero()
    DUMMIES["Dummy_RGB_LED"] = dummy_rgb_led()
    for i in range(3): DUMMIES["Dummy_LEDResistor%d" % i] = dummy_resistor(i)
    DUMMIES["Dummy_Wire"] = dummy_wire()
    if wire_crossings() or wire_crossings(front_layout()):
        DUMMIES["Dummy_Kapton"] = dummy_kapton()
    for i, n in enumerate(["Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"]):
        DUMMIES[n] = dummy_switch(i)
    DUMMIES["Dummy_WireStraps"] = dummy_wire_straps()


PRESS_FITS = {("CapLeft", "Dummy_SwitchLeft"), ("CapMiddle", "Dummy_SwitchMiddle"),
              ("CapRight", "Dummy_SwitchRight")}  # cap fingers pressing on the switch head



def check_interference():
    """Clashes between parts; the intended press fits are reported apart."""
    items = list(PRINTED.items()) + list(DUMMIES.items())
    clashes, fits = [], []
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            (na, a), (nb, b) = items[i], items[j]
            if not a.BoundBox.intersect(b.BoundBox):
                continue
            if {na, nb} == {"Dummy_Wire", "Dummy_RGB_LED"}:
                # Only the matching conductor may touch its physical LED lead.
                led = DUMMIES["Dummy_RGB_LED"]
                for net, wire in WIRE_BY_NET.items():
                    if not wire.BoundBox.intersect(led.BoundBox):
                        continue
                    contact = wire.common(led)
                    if contact.Volume <= 0.00001:
                        continue
                    name = 'C' if net == 'GND' else net[4:] if net in ('LED_R','LED_G','LED_B') else None
                    excess = contact.cut(rgb_led_lead(name)).Volume if name else contact.Volume
                    if excess > 0.01:
                        clashes.append(("Dummy_Wire:" + net, "Dummy_RGB_LED", round(excess, 3)))
                    else:
                        fits.append(("Dummy_Wire:" + net, "Dummy_RGB_LED", round(contact.Volume, 3)))
                continue
            if na == "Dummy_Wire" and nb in ("Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"):
                # Electrical contact is intentional only at that net's actual switch legs.
                idx = ("Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight").index(nb)
                x, y, angle = BTNS[idx]
                for net, wire in WIRE_BY_NET.items():
                    if not wire.BoundBox.intersect(b.BoundBox):
                        continue
                    contact = wire.common(b)
                    if contact.Volume <= 0.00001:
                        continue
                    endpoints = [p for n, pts in wire_layout() if n == net for p in (pts[0], pts[-1])]
                    legs = [cbox(*btn_pin(idx, k), SW_LEG[0], SW_LEG[1], C_FRONT - SW_LEG[2], C_FRONT + 0.01, angle)
                            for k in range(4) if btn_pin(idx, k) in endpoints]
                    excess = contact.cut(Part.makeCompound(legs)).Volume if legs else contact.Volume
                    if excess > 0.01:
                        clashes.append((na + ":" + net, nb, round(excess, 3)))
                    else:
                        fits.append((na + ":" + net, nb, round(contact.Volume, 3)))
                continue
            v = a.common(b).Volume
            if v > 0.01:
                (fits if (na, nb) in PRESS_FITS else clashes).append((na, nb, round(v, 3)))
    if fits:
        print("press fits (intended):", fits)
    return clashes



def check_wire_shorts():
    """Reject modeled overlaps between insulated wires carrying different nets."""
    nets = list(WIRE_BY_NET.items())
    shorts = []
    for i, (name, wire) in enumerate(nets):
        for other_name, other_wire in nets[i + 1:]:
            if wire.BoundBox.intersect(other_wire.BoundBox):
                volume = wire.common(other_wire).Volume
                if volume > 0.00001:
                    shorts.append((name, other_name, round(volume, 6)))
    return shorts

def to_doc():
    if DOC_NAME in App.listDocuments():
        if App.getDocument(DOC_NAME).isTouched():
            raise RuntimeError("Save or close the modified V7 document before rebuilding")
        App.closeDocument(DOC_NAME)
    doc = App.newDocument(DOC_NAME)
    for name, shp in list(PRINTED.items()) + list(DUMMIES.items()):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shp
        if App.GuiUp and obj.ViewObject:
            key = name if name in COLORS else ("Dummy" if name.startswith("Dummy") else name)
            obj.ViewObject.ShapeColor = COLORS[key]
            if name == "Shell":
                obj.ViewObject.Transparency = 55
    add_pin_labels(doc)
    doc.recompute()
    return doc


PIN_LABEL_FONT = 20         # pin label text size on screen (FreeCAD GUI)


def pin_labels():
    """(text, anchor x, y, align) for every pin and switch leg: what it is and the net its
    wire carries ("n/c": no wire). align is the side of the anchor the text sits on (+1 right of
    it, -1 left of it, 0 centred), as seen from the face."""
    nets = {}
    for net, pts in wire_layout():
        for p in (pts[0], pts[-1]):
            nets[(round(p[0], 3), round(p[1], 3))] = net
    net = lambda p: nets.get((round(p[0], 3), round(p[1], 3)), "n/c")
    gap = PIN_ROW[0] / 2 + 0.6                        # from the pin to the text, past the turns
    out = []
    for n, p in sm_pins.items():
        side = 1 if p[0] > SM_X else -1               # outside each row
        t = n if n in ("GND", "3V3") else "%s: %s" % (n, RP_NETS.get(n, "n/c"))
        out.append((t, p[0] + side * gap, p[1], side))
    for n, p in disp_pins.items():
        out.append(("Disp " + n, p[0] + gap, p[1], 1))
    for n, p in enc_pins.items():                     # wire channels approach on the left
        out.append(("Enc " + n, p[0] + 1.2, p[1], 1))
    # switch legs: <switch L/M/R>-<leg: LL lower left, LR, UL, UR>, below the lower legs, above the upper
    legs = ["LL", "LR", "UL", "UR"]
    for i, sw in enumerate(["L", "M", "R"]):
        for k in range(4):
            p = btn_pin(i, k)
            out.append(("%s-%s %s" % (sw, legs[k], net(p)), p[0], p[1] + (-1.9 if k < 2 else 1.9), 0))
    return out


def add_pin_labels(doc):
    """The pin labels as plain text in a "PinLabels" group, just behind the printed PCB's back
    (where you wire). In the document only: never printed, exported or checked for interference.
    The text size is a GUI setting, so it is kept when the document is built and saved in the GUI."""
    grp = doc.addObject("App::DocumentObjectGroup", "PinLabels")
    z = C_BACK - 2.5
    for k, (text, x, y, side) in enumerate(pin_labels()):
        a = doc.addObject("App::Annotation", "PinLabel%02d" % k)
        a.LabelText = [text]
        a.Position = V(x, y, z)
        grp.addObject(a)
        if App.GuiUp and a.ViewObject:
            a.ViewObject.FontSize = PIN_LABEL_FONT
            a.ViewObject.TextColor = (1.0, 0.85, 0.3)
            a.ViewObject.Justification = {1: "Left", -1: "Right", 0: "Center"}[side]


def print_pose(name, shp):
    """Rotate a part into its print orientation, resting on z = 0."""
    s = shp.copy()
    if name in ("Shell", "Knob", "DisplayCowl", "DisplayBezel", "KnobCap", "HaloDiffuser") or name.startswith("Cap"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180)       # face / knob top / visor top / cap top on the bed
    elif name in ("BackPlate", "ScrewFeet"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))  # desk side (plinth) on the bed
    bb = s.optimalBoundingBox()  # the fast box is loose around curved faces
    s.translate(V(-bb.XMin, -bb.YMin, -bb.ZMin))
    return s


def flat_roof(s, f):
    """A flat downward face f of the solid s (print pose). Returns ("bridge", span) when the edges that
    stand on something below surround it, ("ledge", reach) when they sit on one side only (a cantilever)
    or ("island", 0) when none does. reach is the farthest the face lies from a supported edge, span twice
    that (a roof with a hole in it counts by its own width, not the void's); a roof with one straight free
    edge that ends on supported edges, such as a screw seat's D-shaped pad, bridges that edge's length."""
    def runs_down(e, g):
        """Does the neighbour face g leave the edge e downwards (a wall under it, not a roof above)?"""
        par = (e.FirstParameter + e.LastParameter) / 2
        p, t = e.valueAt(par), e.tangentAt(par)
        if g.BoundBox.ZMax < p.z + 0.05 and g.BoundBox.ZMin < p.z - 0.3:
            return True  # lies wholly below it, e.g. the rounded end of the USB hole curving away
        d = g.normalAt(*g.Surface.parameter(p)).cross(t)  # in g's surface, across the edge
        d.normalize()
        if g.distToShape(Part.Vertex(p - d * 0.05))[0] < g.distToShape(Part.Vertex(p + d * 0.05))[0]:
            d = d * -1  # the side that is on g
        return d.z < -0.1

    sup = [e for e in f.Edges if any(not g.isSame(f) and runs_down(e, g) for g in s.ancestorsOfType(e, Part.Face))]
    if not sup:
        return "island", 0.0
    cm = f.CenterOfMass
    # a supporting edge counts along its whole length (one long arc round a pocket is not one point)
    ang = sorted(math.atan2(q.y - cm.y, q.x - cm.x) for e in sup for q in e.discretize(Number=24))
    gaps = [b - a for a, b in zip(ang, ang[1:])] + [ang[0] + 2 * math.pi - ang[-1]]
    around = any(e.isClosed() for e in sup) or max(gaps) <= math.pi + 0.05
    sc = Part.makeCompound(sup)
    free = [e for e in f.Edges if not any(e.isSame(e2) for e2 in sup)]
    if not around and free and all(e.Curve.TypeId == "Part::GeomLine" for e in free):
        pts = [v.Point for e in free for v in e.Vertexes]
        a_, b_ = max(((p, q) for p in pts for q in pts), key=lambda pq: (pq[0] - pq[1]).Length)
        if (abs(sum(e.Length for e in free) - (a_ - b_).Length) < 1e-6  # one straight chord, maybe in pieces
                and all(sc.distToShape(Part.Vertex(q))[0] < 1e-6 for q in (a_, b_))):
            return "bridge", (a_ - b_).Length  # a D: the lines parallel to its straight edge stand on both ends
    reach = 0.0
    u0, u1, v0, v1 = f.ParameterRange
    for i in range(11):
        for j in range(11):
            u, v = u0 + (u1 - u0) * i / 10, v0 + (v1 - v0) * j / 10
            if f.isPartOfDomain(u, v):
                reach = max(reach, sc.distToShape(Part.Vertex(f.valueAt(u, v)))[0])
    return ("bridge", 2 * reach) if around else ("ledge", reach)


def check_overhangs():
    """Support-free check of every printed part in its print pose: a downward face steeper than
    OVH_MAX from vertical (faces on the bed excepted) is a flat roof or a real overhang. Roofs are
    bridges (at most BRIDGE_MAX across) or ledges (at most OVH_LEDGE, a line width, wide); the steep
    curved faces that run on from a roof (the rounded ends of the USB hole) widen its bridge. Prints a
    summary line; returns the problems."""
    lim = math.sin(math.radians(OVH_MAX + 0.5))
    problems, summary = [], []
    for name, shp in PRINTED.items():
        s = print_pose(name, shp)
        roofs, steep = [], []
        for f in s.Faces:
            if f.BoundBox.ZMax < 0.01 or f.Area < 0.05:
                continue  # on the bed, or a sliver
            if f.Surface.TypeId == "Part::GeomPlane":
                nz = -f.normalAt(0, 0).z
                bad = [f.CenterOfMass] if nz > lim else []
            else:
                u0, u1, v0, v1 = f.ParameterRange
                bad = []
                for n in (7, 25):  # a coarse look first, the extent of what is steep on a finer grid
                    uv = [(u0 + (u1 - u0) * (i + 0.5) / n, v0 + (v1 - v0) * (j + 0.5) / n) for i in range(n) for j in range(n)]
                    bad = [f.valueAt(u, v) for u, v in uv if f.isPartOfDomain(u, v) and -f.normalAt(u, v).z > lim]
                    if not bad:
                        break
                nz = 0.0
            if not bad:
                continue
            if nz > 0.9:
                roofs.append([f, flat_roof(s, f), 0.0])
            else:
                steep.append((f, bad))
        for f, bad in steep:  # a curved face running on from a roof belongs to its bridge
            xs, ys = [p.x for p in bad], [p.y for p in bad]
            host = [r for r in roofs if any(e.isSame(e2) for e in f.Edges for e2 in r[0].Edges)]
            if host:
                host[0][2] += max(max(xs) - min(xs), max(ys) - min(ys))
            else:
                problems.append("%s: face steeper than %d deg at (%.1f, %.1f), z %.1f" % (
                    name, OVH_MAX, sum(xs) / len(xs), sum(ys) / len(ys), min(p.z for p in bad)))
        bridges = []
        for f, (kind, w), extra in roofs:
            c = f.CenterOfMass
            if kind == "bridge" and w + extra <= BRIDGE_MAX + 1e-6:
                bridges.append(w + extra)
            elif kind == "ledge" and w <= OVH_LEDGE + 1e-6:
                continue
            else:
                problems.append("%s: %s %.1f mm at (%.1f, %.1f), z %.1f" % (name, kind, w + extra, c.x, c.y, c.z))
        summary.append("%s %s" % (name, "%d bridges, max %.1f" % (len(bridges), max(bridges)) if bridges else "none"))
    print("overhangs (over %d deg, bridges up to %.0f mm): %s" % (OVH_MAX, BRIDGE_MAX, "; ".join(summary)))
    return problems


def write_dxf(shape, path):
    """Minimal R12 DXF of the lines / arcs / circles of a planar section (for KiCad Edge.Cuts)."""
    out = ["0", "SECTION", "2", "ENTITIES"]
    for e in shape.Edges:
        c = e.Curve
        if c.TypeId == "Part::GeomLine":
            a, b = e.Vertexes[0].Point, e.Vertexes[-1].Point
            out += ["0", "LINE", "8", "0", "10", "%.4f" % a.x, "20", "%.4f" % a.y,
                    "11", "%.4f" % b.x, "21", "%.4f" % b.y]
        elif c.TypeId == "Part::GeomCircle":
            if e.isClosed():
                out += ["0", "CIRCLE", "8", "0", "10", "%.4f" % c.Center.x, "20", "%.4f" % c.Center.y,
                        "40", "%.4f" % c.Radius]
            else:
                a0, a1 = e.ParameterRange
                if c.Axis.z < 0:
                    a0, a1 = -a1, -a0
                out += ["0", "ARC", "8", "0", "10", "%.4f" % c.Center.x, "20", "%.4f" % c.Center.y,
                        "40", "%.4f" % c.Radius, "50", "%.4f" % math.degrees(a0), "51", "%.4f" % math.degrees(a1)]
        else:
            pts = e.discretize(Deflection=0.02)
            for a, b in zip(pts, pts[1:]):
                out += ["0", "LINE", "8", "0", "10", "%.4f" % a.x, "20", "%.4f" % a.y,
                        "11", "%.4f" % b.x, "21", "%.4f" % b.y]
    out += ["0", "ENDSEC", "0", "EOF"]
    with open(path, "w") as fh:
        fh.write("\n".join(out) + "\n")


def export(doc, here):
    exp = os.path.join(here, "export")
    os.makedirs(exp, exist_ok=True)
    import MeshPart

    for name, shp in PRINTED.items():
        mesh = MeshPart.meshFromShape(Shape=print_pose(name, shp), LinearDeflection=0.02,
                                      AngularDeflection=math.radians(10), Relative=False)
        mesh.write(os.path.join(exp, "%s.stl" % name))  # binary STL
    # Board outline + through holes only (what a real PCB keeps), for KiCad Edge.Cuts
    zc = (C_BACK + C_FRONT) / 2
    section = carrier_outline().cut(carrier_through_cuts(pcb=True)).slice(V(0, 0, 1), zc)
    comp = Part.makeCompound(section)
    comp.translate(V(0, 0, -zc))
    write_dxf(comp, os.path.join(exp, "pcb-outline.dxf"))
    doc.saveAs(os.path.join(here, "kivori-case-v7.FCStd"))


def max_terminal_hop():
    # Deepest modeled encoder wrap gap.
    zc = C_BACK + WIRE_CH[1] - WIRE_D/2
    return math.hypot(zc - (C_BACK - 0.5 - WRAP_TURNS*WIRE_D), 1.0 - WRAP_OD/2)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    tight = [c for c in check_wires() + check_wires(front_layout(),True) if c[0] < WIRE_RIDGE_MIN - 1e-6]
    print("front crossings:",len(wire_crossings(front_layout())),"minimum opposite-face web:",check_twoface_web())
    print("wire channels: %d runs, %d crossing(s), plastic under %.1f mm: %s" % (
        len(wire_layout()), len(wire_crossings()), WIRE_RIDGE_MIN,
        "none" if not tight else ", ".join("%.2f %s / %s" % c for c in tight)))
    fan_issues=check_led_fan()
    print('LED fan clearance:',fan_issues if fan_issues else 'none')
    if fan_issues:raise ValueError('LED lead fan clearance failed')
    if len(wire_crossings())+len(wire_crossings(front_layout())) > 6:
        raise ValueError('more than six modeled wire crossings')
    shorts = check_wire_shorts()
    print("wire net overlaps:", shorts if shorts else "none")
    low = check_underside()
    print("underside problems:", ", ".join(low) if low else "none")
    ovh = check_overhangs()
    print("overhang problems:", "; ".join(ovh) if ovh else "none")
    print("encoder to back plate: %.2f mm" % DUMMIES["Dummy_Encoder"].distToShape(PRINTED["BackPlate"])[0])
    print("buttons:", [(round(x, 2), round(y, 2), round(a, 2)) for (x, y, a) in BTNS])
    for name, shp in PRINTED.items():
        bb = shp.BoundBox
        print("%-10s vol %8.0f mm3  bbox %.1f x %.1f x %.1f  valid=%s" % (
            name, shp.Volume, bb.XLength, bb.YLength, bb.ZLength, shp.isValid()))
    rx,ry=RESET_C
    probe=cyl(1.0,rx,ry,z_back(ry)-3,RESET_SURFACE_Z-0.05)
    for name,shape in list(PRINTED.items())+list(DUMMIES.items()):
        if probe.BoundBox.intersect(shape.BoundBox) and probe.common(shape).Volume>0.00001:
            raise ValueError('reset probe path blocked by '+name)
    board = DUMMIES["Dummy_RP2040_Zero"]
    clearances = {name: board.distToShape(DUMMIES[name])[0]
                  for name in ("Dummy_Encoder", "Dummy_EncoderNut", "Dummy_SwitchLeft")}
    print("board clearances (mm):", clearances)
    led = DUMMIES["Dummy_RGB_LED"]
    led_clearance = min(led.distToShape(DUMMIES[name])[0]
                        for name in ("Dummy_Encoder", "Dummy_EncoderNut"))
    if led_clearance < 0.5 - 1e-6:
        raise ValueError("LED clearance to encoder hardware below 0.5 mm")
    if -0.6 - LED_TIP_Z < 0.5 - 1e-6:
        raise ValueError("LED tip to fixed optical roof clearance below 0.5 mm")
    if HALO_ID / 2 - HALO_R_IN < 0.2 - 1e-6:
        raise ValueError("optical roof to shaft-island radial clearance below 0.2 mm")
    print("LED to encoder hardware: %.2f mm; tip to optical roof: %.2f mm" %
          (led_clearance, -0.6 - LED_TIP_Z))
    if not (enc_x < KNOB_C[0] and all(enc_pins[a][1] < enc_pins[b][1]
              for a, b in zip(("CLK", "DT", "SW", "+"), ("DT", "SW", "+", "GND")))):
        raise ValueError("rotated HW-040 header must preserve its physical pin order")
    if len(sm_pins) != 23 or len(set(sm_pins.values())) != 23:
        raise ValueError("RP2040-Zero header must have 23 distinct physical pins")
    blocked = [n for n, (x, y) in sm_pins.items()  # a line down each pin centre must clear the carrier solid
               if Part.makeLine(V(x, y, C_BACK - 1), V(x, y, C_FRONT + 1)).common(PRINTED["Carrier"]).Length > 1e-6]
    print("carrier through-holes at all %d RP2040-Zero pins: %s" % (len(sm_pins), "yes" if not blocked else "NO " + ", ".join(blocked)))
    if blocked:
        raise ValueError("carrier has no through-hole for RP2040-Zero pin(s): %s" % ", ".join(blocked))
    if sum(n.startswith("Dummy_RGB_LED") for n in DUMMIES) != 1 or len(led_terminals) != 4 or len(RES_C) != 3:
        raise ValueError("exactly one external RGB LED and three series resistors required")
    if max_terminal_hop() > 3.0:
        raise ValueError("terminal connection exceeds 3 mm")
    print("maximum modeled terminal hop: %.2f mm" % max_terminal_hop())
    if clashes or tight or shorts or low or ovh or any(not shape.isValid() or shape.Volume <= 0 for shape in PRINTED.values()):
        raise ValueError("design checks failed; inspect diagnostics above")
    if min(clearances.values()) < 0.5 - 1e-6:
        raise ValueError("RP2040-Zero clearance below 0.5 mm")
    for name in ("CapLeft", "CapMiddle", "CapRight"):
        pressed = PRINTED[name].copy()
        pressed.translate(V(0, 0, -0.5))
        others = [shape for n, shape in PRINTED.items() if n != name]
        others += [board, DUMMIES["Dummy_Display"]]
        if any(pressed.common(shape).Volume > 0.01 for shape in others):
            raise ValueError(name + " collides during 0.5 mm travel")
    pressed_knob = PRINTED["Knob"].fuse(PRINTED["KnobCap"])
    pressed_knob.translate(V(0, 0, -0.5))
    if any(pressed_knob.common(shape).Volume > 0.01 for shape in
           (PRINTED["Shell"], PRINTED["DisplayCowl"], PRINTED["HaloDiffuser"], PRINTED["HaloReflector"], DUMMIES["Dummy_Encoder"])):
        raise ValueError("knob collides during 0.5 mm travel")
    if glass_z0 > COWL_H - COWL_T + DISP_POCKET - DISP_FACE_GAP + 1e-6:
        raise ValueError("LCD glass clearance lost")
    if any(len(shape.Solids) != (6 if name == "ScrewFeet" else 1) for name, shape in PRINTED.items()):
        raise ValueError("printed part contains disconnected islands")
    if KNOB_GAP - 0.5 - HALO_T < 0.3-1e-6:
        raise ValueError("pressed knob to optical roof clearance below 0.3 mm")
    print("all keys and knob: 0.5 mm travel clear")
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):  # freecadcmd imports the file as module "build_case"
    main()
