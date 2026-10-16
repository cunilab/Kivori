"""Kivori enclosure: parametric build script (FreeCAD 1.1, Part workbench, no GUI needed).

Run it inside FreeCAD (GUI Python console, the FreeCAD MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure/build_case.py"
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

# Case (blueprint, widened from 100 to 110 mm so the Ø48 knob clears the button arc).
# Design "big keycap": the walls lean in towards the face like a keycap, a 45° bevel frames the
# face, a dark visor carries the display and buttons, and the shell floats on a dark plinth.
W, H = 110.0, 70.0          # face outline (outer edge of the bevel)
R_CORNER = 8.0              # face corner radius
T_NEAR, T_FAR = 20.7, 32.7  # case thickness at the near (y=0) and far (y=H) edge -> ~9.7° wedge
WALL = 2.0                  # side wall thickness (horizontal)
WALL_TAPER = 6.0            # degrees the walls lean in towards the face (keycap draft)
FACE_T = 2.5                # faceplate thickness (room for the visor pocket in front)
FACE_BEVEL = 2.0            # bevel round the face (inset at the face); a true 45° even with the
                            # wall lean, so it prints face down without overhang
BP_T = 2.0                  # back plate thickness
BP_CLR = 0.2                # back plate to wall clearance (per side)

# Knob + encoder (EC11 dimensions from KiCad RotaryEncoder_Alps_EC11E-Switch_Vertical_H20mm)
KNOB_C = (27.5, 35.0)       # knob / shaft centre on the face
KNOB_D = 48.0               # height comes from the shaft: KNOB_H further down
KNOB_GAP = 1.0              # air gap between knob skirt and face (push travel is ~0.5)
KNOB_TOP_T = 2.0            # knob top above the shaft-tip clearance
KNOB_WALL = 2.5             # hollow knob: skirt wall (the grip flutes cut 0.6 into it)
KNOB_HUB_D = 12.0           # hub round the shaft, hanging from the top, tied to the skirt by ribs
KNOB_RIBS, KNOB_RIB_T = 4, 1.2
KNOB_FLUTES, KNOB_FLUTE_D = 60, 1.2   # fine vertical grip ribs round the skirt
KNOB_MARK = (13.0, 20.5, 135.0, 1.2)   # radial line marker: from r, to r, angle, width
KNOB_HALO = None            # (r in, r out, depth) groove ring round the knob: off, it ran into the
                            # face's left bevel and left slivers too thin to print
EC11_BODY = (12.0, 11.6)    # NOMINAL body footprint
# Heights above the HW-040 board front, measured on the real module (side photo next to a ruler,
# scale checked on the 6 mm shaft, about ±0.4 mm)
EC11_BODY_H = 7.7           # body top (bushing base)
EC11_BUSH_D, EC11_BUSH_L = 7.0, 8.5   # M7 bushing: thread ends 16.2 above the board
EC11_SHAFT_D, EC11_SHAFT_H = 6.0, 29.2  # D shaft, tip 29.2 above the board
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
HW040_HDR_X = 10.35         # right-angle header pin row (pins point +x, along the board)
HW040_BACK_KEEPOUT = (-9.0, 9.0, -4.0, 6.0)  # shaft-relative: EC11 legs + 3 pull-up resistors underneath
HW040_BACK_PARTS = 2.0      # how far those (and the header joints) stick out of its back
# The HW-040 hangs behind the printed PCB, on two back-plate columns whose pegs go through its M3
# holes, with the EC11 body reaching forward through a hole in the PCB. Deep in the wedge, the
# long shaft ends low and the knob stays short; the columns also take the knob's push.
HW040_FRONT = -20.2         # board front: the EC11 body top 0.3 behind the printed PCB, only its bushing goes through
# The back plate holds the HW-040 on two pillars with pegs through its two holes (it cannot slide or
# turn) and four corner brackets: a ledge under each board corner (a knob press cannot tip it) and
# a low wall round it, all clear of the resistors and solder joints in the middle of its back.
# With copper traces its five header pins stand straight up (straightened, or a straight header)
# through pads on the printed PCB.
ENC_BORDER = (0.3, 1.2, 1.0, 1.2)  # board clearance, wall thickness, wall height, ledge under the edge
ENC_CORNER = 5.0            # each corner bracket runs this far along both board edges
HW040_COL_D = 5.0           # pillars under its two holes ...
HW040_PEG_CLR = 0.2         # ... pegs 2.8 mm in its 3.2 mm holes
HW040_PIN_BEND = 0.0        # straight pins, up from the header row
HW040_PIN_L = 2.5 + 6.0     # straight header: plastic + pin past it, from the board front (NOMINAL)
EC11_HOLE_CLR = 0.3         # printed PCB round hole round the EC11 bushing, per side (it also centres the encoder)

# Display: ST7789 1.3" 240x240, 7-pin module, mounted rotated 90° (header on the right)
DISP_C = (80.35, 46.0)      # centre of the window on the face (board centre stays at x 78.5)
DISP_ACTIVE = 23.4          # active area (square), several sources agree
DISP_WIN_MARGIN = 0.3       # window opening beyond the active area, per side
DISP_WIN_CHAMFER = 0.8
# Module frame: u runs along the 39.22 mm length, + towards the header end. Glass and visible
# area measured on the real module (photo next to a ruler, about ±0.5 mm).
DISP_PCB = (39.22, 27.78, 1.6)    # module PCB (length, width, thickness); 3.0 total with glass
DISP_GLASS = (28.0, 26.3, 1.4)    # glass incl. the FPC ledge at the end away from the header (measured)
DISP_ACTIVE_U = 1.85              # visible-area centre from the board centre, along u (measured)
DISP_GLASS_U = 0.8                # glass centre from the board centre, along u (measured)
DISP_HDR_U = 2.2                  # 7-pin header row, from the header end
DISP_POCKET = 0.6           # glass pocket depth into the back of the faceplate
DISP_HEADER_SIDE = +1       # +1 header on the right end, -1 on the left end
DISP_HOLE_D = 2.0          # four corner holes, NOMINAL ("mounting holes: 2 mm")
DISP_HOLE_INSET = 2.5      # hole centres from both edges at each corner (measured: 34.2 x 22.8 apart)
DISP_STANDOFF_D = 4.5
DISP_PINS = ["GND", "VCC", "SCL", "SDA", "RES", "DC", "BLK"]  # top to bottom as mounted

# Buttons: 12x12 tactile push switch, 10 mm tall (pads from KiCad SW_PUSH-12mm)
BTN_MID = (78.0, 18.0)      # middle button centre
BTN_PITCH = 18.0            # chord distance to the outer buttons (blueprint)
BTN_ARC_R = 80.0            # soft arc radius -> outer buttons ~2 mm lower, rotated ~13°
BTN_OPEN = 14.0             # face opening (blueprint)
BTN_CAP = 13.4              # cap body
BTN_FLANGE = 15.2           # retaining flange behind the face
BTN_FLANGE_T = 1.2
BTN_CAP_PROUD = 3.5         # cap top above the face
# 12x12 projected-plunger switch, seller drawing TS-G005 (7.5 mm version; this one is 10 mm tall)
SW_BODY, SW_BODY_H = 11.7, 3.6  # housing 11.7 x 11.7 x 3.6 (seller drawing, the 7.3 mm switch)
SW_BOSS_D, SW_BOSS_H = 6.4, 4.5   # round platform on the housing, top 4.5 mm above the PCB
# Snap-fit stem (seller drawing): on the platform a short narrower neck, 1 mm tall, then the square
# head (the grip) up to the top, so a cap clicks on under the head
SW_NECK, SW_NECK_H = 3.0, 1.0  # stem neck, square: width, height above the platform
SW_HEAD, SW_HEAD_H = 3.8, 1.7  # square head with a centre hole: width, height
SW_TOTAL_H = SW_BOSS_H + SW_NECK_H + SW_HEAD_H  # head top 7.2 mm above the PCB
CAP_WALL = 1.0              # hollow cap side wall
# The cap clicks on like a bought round 12x12 cap: four fingers, 2 mm wide, hang from the cap top
# round the head. A lip on each snaps in under the head, and a stop post on the cap top rests on
# it, so the cap is held both ways without play and a press goes straight into the stem. The
# short switch sits deep, so a square sleeve joins the fingers down to the head top: only their
# ends round the head flex.
CAP_TOP_T = 1.2             # flat cap top; the cap prints top down, so the top comes off the bed
CAP_FINGER = (2.0, 0.8)     # finger width (as on the reference cap), thickness
CAP_SNAP = 0.3              # lip reach in under the head (the neck is 0.4 narrower per side)
CAP_SNAP_CLR = 0.1          # finger to head side
CAP_STOP_D = 2.6            # stop post on the head, round its centre hole
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]  # pin 1,1,2,2 (land pattern Ø1.5)
SW_PEGS = [(0.0, -4.5), (0.0, 4.5)]  # clearance for locating bosses some 12x12 switches have (B3F)
SW_PEG_HOLE_D = 2.0
BTN_GPIO = ["IO3", "IO4", "IO5"]  # left, middle, right (copper-trace pin map, see README Wiring)

# Printed PCB ("carrier"): same plane as a future real PCB
C_FRONT = -10.2             # printed PCB front (the 7.2 mm switch top ends 3.0 below the face: the caps reach down)
# knob just tall enough to cover the whole shaft, so the shaft is never cut
KNOB_H = HW040_FRONT + EC11_SHAFT_H + 0.5 + KNOB_TOP_T - KNOB_GAP
C_T = 2.0                   # printed thickness (a real board is 1.6, it grows backwards); leaves a 0.6 mm HW-040 pocket floor
C_BACK = C_FRONT - C_T
C_INSET = 0.8               # carrier outline inset from the inner walls
HOLE_D = 1.2                # 2.54 mm header holes
SW_HOLE_D = HOLE_D          # switch legs: same hole as the header pins
POSTS = [(6.0, 7.0), (6.0, 62.5), (104.0, 62.5), (104.0, 7.0)]  # face posts, symmetric, near the corners
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
PEG_CLR = 0.1               # peg diameter = hole - 2 * clearance (press fit)
PEG_PROUD = 1.0             # peg tip past the module board: press fit, or melt it (heat-stake)
PEG_RELIEF = 0.7            # face-back relief over the display peg tips (room for a staked head)

# ESP32-C3 SuperMini, behind the carrier, components facing the back plate
SM = (18.0, 22.52, 1.0)     # outline, NOMINAL thickness
SM_X = 49.0                 # board centre x: keeps the pin rows clear of the display board
SM_USB = (8.94, 7.35, 3.2)  # receptacle (width, depth, height), NOMINAL
SM_USB_OVERHANG = 0.5       # NOMINAL receptacle front past the board end
# USB-C port: a tight pill-shaped hole. A hidden pocket inside the far wall thins it to USB_WALL
# round the receptacle, and the board sits right up against that pocket, so a cable's plug
# reaches fully into the receptacle without its moulded grip entering the wall.
USB_WALL = 0.8              # wall left in front of the receptacle (measured square to the wall)
USB_HOLE = (9.6, 4.0)       # pill hole round the receptacle: 0.33 / 0.4 mm clearance per side
USB_HOLE_CHAMFER = 0.0      # chamfer round the outside edge: off, it thinned the 0.8 mm wall to a 0.4 lip
USB_FRONT_CLR = 0.15        # receptacle front to the pocket face
SM_ROW = 15.24              # header row spacing
SM_PIN_MARGIN = (SM[1] - 7 * 2.54) / 2  # NOMINAL first pin from the USB end
# Columns as seen from the BACK (component side up, USB at the top). Seen from the face they
# mirror, so the "left" column sits at +x in the face frame.
SM_COL_A = ["5V", "GND", "3V3", "IO4", "IO3", "IO2", "IO1", "IO0"]    # at x = SM_X + row/2
SM_COL_B = ["IO5", "IO6", "IO7", "IO8", "IO9", "IO10", "IO20", "IO21"]  # at x = SM_X - row/2
# Soldered pin headers: the black plastic strip lies on the carrier side and drops into a slot,
# so the board still sits flat on the carrier; the long pins point to the face (measured).
SM_HDR = (2.54, 8 * 2.54, 2.5, 6.0)  # strip width, length, plastic height, long pin past the plastic
SM_HDR_CLR = 0.25           # slot clearance per side
SM_HDR_CHAMFER = 0.6        # 45° border round the slot on the back (prints clean on the bed)
SM_PIN_RELIEF = 1.0         # pocket in the faceplate back over the pin tips
SM_BTN_FROM_USB = 9.5       # NOMINAL BOOT / RST distance from the USB end
SM_BTN_DX = 4.5             # NOMINAL offset either side of the board centre line
SM_BTN = (3.0, 4.0, 1.5)
# With copper traces the SuperMini stands on the front of the printed PCB on its own soldered
# header strips, components towards the face, and its pins go through to the copper on the back.
# (Hand-wired version: behind the printed PCB, components towards the back plate.)
SM_FRONT = None             # set below from COPPER_TRACES
C_TAB_CLR = 0.4             # carrier tab under the SuperMini header ends, to the far wall

# Copper-tape traces on the printed PCB's back instead of hand wiring. Each trace and pad sits flush
# with the back, with a gap along both sides: lay tape flat over the back, cut along the gaps, peel
# off the rest. The gaps are on the bed side (gaps in the first layers, short bridges over them).
# Every trace and pad is square-cornered and runs along x or y, so each cut is a straight knife
# line. Only pins that carry a signal get copper (no pad on unused pins or switch legs).
# One crossover remains (copper_layout).
COPPER_TRACES = True        # False: the hand-wired version (header strips in slots)
TRACE_GROOVE = (0.95, 0.6)  # gap along both sides of every trace and pad: width, depth
# 2.54 mm pin rows: a square pad round each pin, and between two neighbouring pins a PIN_GAP cut
# line that guides the blade (one nozzle width: about 0.37 mm of plastic is left round each hole)
PIN_GAP = 0.6
TRACE_PAD = (3.0, 2.54 - PIN_GAP)  # pads at the 2.54 mm pins: across the row, along it
TRACE_W = 2.54 - TRACE_GROOVE[0]  # side by side traces on the 2.54 mm pin pitch are one gap apart
# Pin holes go right through (PIN_SKIN = 0), clear for the pins; after taping, pierce the tape over
# each one. PIN_SKIN > 0 stops them that far short of the copper side instead (blind holes: the
# pads print as plain solid rectangles in the gap layers, drilled out by hand after taping).
PIN_SKIN = 0.0
TRACE_PAD_D = 2.8           # square pads at the switch legs (0.5 mm of copper round the leg hole)
TAPE_T = 0.06               # copper tape thickness (stand-in only)
TRACE_ISLAND_MIN = 12.0     # mm²: smaller islands of plastic between the gaps become gap (FDM)
TRACE_GROOVE_MIN = 0.5      # gaps thinner than this (crumbs) are filled in (the cut lines stay)
TRACE_RIDGE_MIN = 1.0       # plastic left between two gaps: at least this wide, or it becomes gap
GROOVE_GRID = 0.05          # pixel size of the grid the gaps are cleaned up on (copper_grooves)
KAPTON_T = 0.06             # insulating tape under a crossover (stand-in only)
# No solder: cut an X in the tape over each pin hole; each pin pushed through folds the four copper
# petals along it and touches them.
# RST: a needle hole through the visor and the face over the SuperMini's RST button, with a guide
# tube down to just above the button
RST_HOLE_D = 1.0
RST_GUIDE = (3.0, 1.4)      # guide tube OD, bore

SM_FRONT = COPPER_TRACES
# the receptacle's face-side edge meets the leaning far wall furthest in, so it sets the board end
_usb_hi = (C_FRONT + SM_HDR[2] + SM[2] + SM_USB[2]) if SM_FRONT else (C_BACK - SM[2])
SM_USB_END = (H - USB_WALL / math.cos(math.radians(WALL_TAPER))  # y of the USB end of the board
              - _usb_hi * math.tan(math.radians(WALL_TAPER)) - USB_FRONT_CLR - SM_USB_OVERHANG)

# Back plate
PINHOLE_D = 4.0             # blueprint Ø4, paperclip / SIM pin
TUBE_OD = 7.0
TUBE_GAP = 0.6              # tube top below the lowest part around it
COLUMN_D = 8.0              # columns clamping the carrier against the face posts (screws inside)
HOOKS = [(-1, 18.0), (-1, 52.0), (+1, 18.0), (+1, 52.0)]  # (side, y centre)
HOOK_W, HOOK_T, HOOK_L = 8.0, 1.4, 10.0
HOOK_BARB = 1.0             # barb with 45° ramps both ways: snaps in, pulls out (a detent)
HOOK_GAP = 1.2              # arm to inner wall
RIB_DEPTH, RIB_H = 1.0, 1.5  # catch rib on the inside of the wall: no windows, nothing visible

# Floating plinth under the shell (part of the back plate, printed dark)
PLINTH_H = 3.0
PLINTH_INSET = 2.0          # inset from the face outline; the flared walls make the shadow ~4-5 mm
PLINTH_R = 6.0
# Printed TPU rubber dots under the plinth, one in each corner, centred on the screw there (it
# hides the screw) and inside the plinth's rounded corner; a plug presses into the screw
# counterbore, so the plinth needs no recess and still prints flat.
DOT_D, DOT_T = 6.0, 1.5
DOT_PEG_D, DOT_PEG_L = SCREW_HEAD_D - 0.1, 1.5  # plug: TPU in the counterbore

# Dark visor: separate black panel standing 1 mm proud of the face (a recess would need a 60 mm
# bridge in the face-down print), carrying the display window and the button openings
VISOR_T = 1.0
VISOR_BOX = (44.0, 5.5, 106.5, 62.5)  # x0, y0, x1, y1 before the knob cut-out
VISOR_R = 6.0
VISOR_KNOB_R = 26.3         # concentric cut-out round the knob (even gap ring)
VISOR_EDGE = 0.5            # chamfer round the visor top edge
VISOR_PEGS = [(50.0, 60.0), (103.5, 60.0), (104.5, 32.0)]  # locating pegs into the face
VISOR_PEG_D, VISOR_PEG_H, VISOR_HOLE_D, VISOR_HOLE_DEPTH = 1.8, 1.0, 2.3, 1.2  # (FDM holes print small)
UNDER_CLR = 0.3             # face openings behind the visor are this much bigger per side

FONT = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
TEXT_D = 0.4
TEXT_STROKE = 0.15          # stroke round every letter, so small raised letters still print
LABEL_PAD, LABEL_R = 1.0, 0.8  # label box: recessed round the letters, which stay standing

DOC_NAME = "KivoriCase"
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


def grow_face(f, b):
    """Offset a glyph face: grow its outline by b, shrink each hole by b (a hole that closes is
    dropped). Done wire by wire, so a closing hole cannot break the whole glyph."""
    try:
        out = Part.Face(f.OuterWire).makeOffset2D(b).Faces[0]
    except Exception:  # some outlines (S) only offset in small steps
        out, done = Part.Face(f.OuterWire), 0.0
        while done < b - 1e-6:
            step = min(0.3, b - done)
            out = Part.Face(out.makeOffset2D(step).Faces[0].OuterWire)
            done += step
    out = Part.Face(out.OuterWire)  # drop slivers the offset can leave inside (S, 5)
    for w in f.Wires:
        if w.isSame(f.OuterWire):
            continue
        try:
            hole = Part.Face(w).makeOffset2D(-b)
            if hole.Faces and hole.Area > 0.05:
                out = out.cut(hole).Faces[0]
        except Exception:
            pass  # the hole closed up
    return out


def text_solid(s, x, y, size, z0, depth, mirror=False):
    if not os.path.exists(FONT):
        return None
    try:
        faces = []
        for ch in Part.makeWireString(s, FONT, size, 2 * TEXT_STROKE):
            if not ch:
                continue
            wires = [w if isinstance(w, Part.Wire) else Part.Wire(w) for w in ch]
            # fine polygons offset reliably (some glyph curves do not)
            wires = [Part.makePolygon(w.discretize(Deflection=0.01) + [w.discretize(Deflection=0.01)[0]])
                     for w in wires]
            f = Part.makeFace(wires, "Part::FaceMakerBullseye")
            try:
                f = grow_face(f, TEXT_STROKE)
            except Exception as exc:
                print("text stroke skipped:", s, exc)
            faces.append(f)
        comp = Part.makeCompound(faces)
        bb = comp.BoundBox
        sol = comp.extrude(V(0, 0, depth))
        sol.translate(V(x - (bb.XMin + bb.XMax) / 2, y - (bb.YMin + bb.YMax) / 2, z0))
        if mirror:
            sol = sol.mirror(V(x, 0, 0), V(1, 0, 0))
        return sol
    except Exception as exc:  # text is cosmetic, never fail the build on it
        print("text skipped:", s, exc)
        return None


def label_cut(s, x, y, size, z0, mirror=False):
    """A label: a recessed rounded box with the letters left standing inside it."""
    t = text_solid(s, x, y, size, z0, TEXT_D + 0.1, mirror)
    if not t:
        return None
    bb = t.BoundBox
    box_ = rr_prism(bb.XLength + 2 * LABEL_PAD, bb.YLength + 2 * LABEL_PAD, LABEL_R,
                    (bb.XMin + bb.XMax) / 2, (bb.YMin + bb.YMax) / 2, z0, z0 + TEXT_D + 0.1)
    return box_.cut(t)


def rot(pt, ang):
    a = math.radians(ang)
    return (pt[0] * math.cos(a) - pt[1] * math.sin(a), pt[0] * math.sin(a) + pt[1] * math.cos(a))


# ---------------------------------------------------------------------------------------------
# Derived layout
# ---------------------------------------------------------------------------------------------

def button_poses():
    """(x, y, angle) for left, middle, right along the soft arc."""
    th = math.degrees(math.asin(BTN_PITCH / BTN_ARC_R))
    cy = BTN_MID[1] - BTN_ARC_R
    drop = BTN_ARC_R * math.cos(math.radians(th))
    return [
        (BTN_MID[0] - BTN_PITCH, cy + drop, th),
        (BTN_MID[0], BTN_MID[1], 0.0),
        (BTN_MID[0] + BTN_PITCH, cy + drop, -th),
    ]


BTNS = button_poses()
SW_PLUNGER_TOP = C_FRONT + SW_TOTAL_H

# Display: module u axis lies along face x (DISP_HEADER_SIDE says which way the header points)
dpcb_c = (DISP_C[0] - DISP_HEADER_SIDE * DISP_ACTIVE_U, DISP_C[1])
glass_c = (dpcb_c[0] + DISP_HEADER_SIDE * DISP_GLASS_U, DISP_C[1])
glass_z0 = -FACE_T + DISP_POCKET          # glass front
glass_z1 = glass_z0 - DISP_GLASS[2]       # glass back = module PCB front
dpcb_z1 = glass_z1 - DISP_PCB[2]          # module PCB back
disp_hdr_x = dpcb_c[0] + DISP_HEADER_SIDE * (DISP_PCB[0] / 2 - DISP_HDR_U)
disp_pins = {n: (disp_hdr_x, dpcb_c[1] + (3 - i) * 2.54) for i, n in enumerate(DISP_PINS)}
disp_holes = [(dpcb_c[0] + su * (DISP_PCB[0] / 2 - DISP_HOLE_INSET), dpcb_c[1] + sv * (DISP_PCB[1] / 2 - DISP_HOLE_INSET))
              for su in (-1, 1) for sv in (-1, 1)]

# HW-040: module frame axes match the face frame (header end towards +x, holes towards the near edge)
hw_c = (KNOB_C[0] - HW040_SHAFT[0], KNOB_C[1] - HW040_SHAFT[1])
hw_holes = [(hw_c[0] + hx, hw_c[1] + hy) for (hx, hy) in HW040_HOLES]
hw_hdr = (hw_c[0] + HW040_HDR_X, hw_c[1] + HW040_SHAFT[1])  # header row centre
hw_pin_tip_x = hw_c[0] + HW040[0] / 2 + 6.0                    # right-angle pins end about here
hw_back = HW040_FRONT - HW040[2]           # HW-040 board back
enc_x = hw_hdr[0] + HW040_PIN_BEND         # where its pins come up through the printed PCB
enc_pins = {n: (enc_x, hw_hdr[1] + (2 - i) * 2.54) for i, n in enumerate(["CLK", "DT", "SW", "+", "GND"])}

sm_c = (SM_X, SM_USB_END - SM[1] / 2)
if SM_FRONT:  # on its header strips on the carrier front, components up: column A on the left
    sm_lo = C_FRONT + SM_HDR[2]
    sm_hi = sm_lo + SM[2]
    usb_lo, usb_hi = sm_hi, sm_hi + SM_USB[2]
    xa_, xb_ = SM_X - SM_ROW / 2, SM_X + SM_ROW / 2
else:  # behind the carrier, components towards the back plate
    sm_lo, sm_hi = C_BACK - SM[2], C_BACK
    usb_lo, usb_hi = sm_lo - SM_USB[2], sm_lo
    xa_, xb_ = SM_X + SM_ROW / 2, SM_X - SM_ROW / 2
sm_z_comp = sm_lo                          # component side (hand-wired version)
sm_pins = {}
for i, n in enumerate(SM_COL_A):
    sm_pins[n] = (xa_, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
for i, n in enumerate(SM_COL_B):
    sm_pins[n] = (xb_, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
sm_rows = [xa_, xb_]
sm_hdr_y = SM_USB_END - SM_PIN_MARGIN - 3.5 * 2.54
_bx = SM_BTN_DX if SM_FRONT else -SM_BTN_DX     # RST, BOOT (NOMINAL; the board mirrors when it faces up)
sm_btns = [(SM_X + _bx, SM_USB_END - SM_BTN_FROM_USB), (SM_X - _bx, SM_USB_END - SM_BTN_FROM_USB)]
usb_z = (usb_lo + usb_hi) / 2
tube_top = sm_z_comp - SM_USB[2] - TUBE_GAP


def btn_pin(idx, pad):
    x, y, a = BTNS[idx]
    dx, dy = rot(SW_PADS[pad], a)
    return (x + dx, y + dy)


def btn_peg(idx, k):
    x, y, a = BTNS[idx]
    dx, dy = rot(SW_PEGS[k], a)
    return (x + dx, y + dy)


# ---------------------------------------------------------------------------------------------
# Parts
# ---------------------------------------------------------------------------------------------


def visor_outline(z0, z1, grow=0.0):
    x0, y0, x1, y1 = VISOR_BOX
    v = rr_prism(x1 - x0 + 2 * grow, y1 - y0 + 2 * grow, VISOR_R + grow, (x0 + x1) / 2, (y0 + y1) / 2, z0, z1)
    v = v.cut(cyl(2 * (VISOR_KNOB_R - grow), KNOB_C[0], KNOB_C[1], z0 - 1, z1 + 1))
    try:  # soften the two concave corners where the knob cut-out meets the edges
        sharp = [e for e in v.Edges if abs(e.BoundBox.ZLength - (z1 - z0)) < 1e-6
                 and e.Curve.TypeId == "Part::GeomLine" and e.BoundBox.XMax < KNOB_C[0] + VISOR_KNOB_R + 1]
        if sharp:
            v = v.makeFillet(1.5, sharp)
    except Exception as exc:
        print("visor corner fillet skipped:", exc)
    return v


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
    if SM_FRONT:  # needle guide down to just above the RST button
        rx, ry = sm_btns[0]
        z_btn = sm_hi + SM_BTN[2]
        adds.append(cyl(RST_GUIDE[0], rx, ry, z_btn + 0.6, -FACE_T + 0.01).cut(
            cyl(RST_GUIDE[1], rx, ry, z_btn, -FACE_T - 0.5)))
    shell = shell.fuse(adds)

    cuts = [cyl(ENC_HOLE_D, KNOB_C[0], KNOB_C[1], -FACE_T - 1, 1)]
    if KNOB_HALO:
        cuts.append(cyl(2 * KNOB_HALO[1], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2], 1).cut(
            cyl(2 * KNOB_HALO[0], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2] - 1, 2)))
    cuts += [cyl(VISOR_HOLE_D, x, y, -VISOR_HOLE_DEPTH, 1) for (x, y) in VISOR_PEGS]
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN + 2 * UNDER_CLR
    cuts.append(rr_prism(win, win, 1.0, DISP_C[0], DISP_C[1], -FACE_T - 1, 1))
    cuts.append(rr_prism(DISP_GLASS[0] + 0.5, DISP_GLASS[1] + 0.5, 0.5, glass_c[0], glass_c[1],
                         -FACE_T - 1, -FACE_T + DISP_POCKET))
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN + 2 * UNDER_CLR, BTN_OPEN + 2 * UNDER_CLR, 1.8, x, y, -FACE_T - 1, 1, a))
    for (x, y) in disp_holes:
        cuts.append(cyl(DISP_STANDOFF_D, x, y, -FACE_T - 1, -FACE_T + PEG_RELIEF))
    for (x, y) in POSTS:
        cuts.append(cyl(SCREW_PILOT, x, y, C_BACK - SPIGOT_INTO_COL - 1, -FACE_T - 1.5))
    for xr in ([] if COPPER_TRACES else sm_rows):  # room for the long header pin tips
        cuts.append(cbox(xr, sm_hdr_y, SM_HDR[0] + 2 * SM_HDR_CLR, 7 * 2.54 + 1.2,
                         -FACE_T - 1, -FACE_T + SM_PIN_RELIEF))
    cuts += usb_port_cuts()
    if SM_FRONT:  # RST needle hole
        rx, ry = sm_btns[0]
        cuts.append(cyl(RST_HOLE_D, rx, ry, -FACE_T - 1, 1))
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


def make_visor():
    """Black panel on the face: display window with a bevel, three button openings, locating pegs."""
    v = visor_outline(0, VISOR_T)
    try:
        top = [e for e in v.Edges if abs(e.BoundBox.ZMin - VISOR_T) < 1e-6 and abs(e.BoundBox.ZMax - VISOR_T) < 1e-6]
        v = v.makeChamfer(VISOR_EDGE, top)
    except Exception as exc:
        print("visor edge chamfer skipped:", exc)
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN
    cuts = [rr_prism(win, win, 1.0, DISP_C[0], DISP_C[1], -1, VISOR_T + 1)]
    cham = Part.makeLoft([rr_wire(win + 2 * DISP_WIN_CHAMFER, win + 2 * DISP_WIN_CHAMFER, 1.8, VISOR_T + 0.01),
                          rr_wire(win, win, 1.0, VISOR_T - DISP_WIN_CHAMFER)], True)
    cuts.append(place(cham, DISP_C[0], DISP_C[1]))
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN, BTN_OPEN, 1.5, x, y, -1, VISOR_T + 1, a))
    if SM_FRONT:
        cuts.append(cyl(RST_HOLE_D, sm_btns[0][0], sm_btns[0][1], -1, VISOR_T + 1))
    v = v.cut(cuts)
    v = v.fuse([cyl(VISOR_PEG_D, x, y, -VISOR_PEG_H, 0.01) for (x, y) in VISOR_PEGS])
    return v.removeSplitter()


def make_back_plate():
    plate = tapered(W - 2 * WALL - 2 * BP_CLR, H - 2 * WALL - 2 * BP_CLR, R_CORNER - WALL - BP_CLR, 0, -60)
    plate = plate.common(above_back(0)).cut(above_back(BP_T))

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
    for (x, y) in ([] if SM_FRONT else sm_btns):
        adds.append(cyl(TUBE_OD, x, y, z_back(y) + 0.5, tube_top))
    adds.append(enc_cradle())
    if not SM_FRONT:  # a pad presses up under the USB connector
        usb_y0 = max(SM_USB_END - SM_USB[1] + 1.5, sm_btns[0][1] + TUBE_OD / 2 + 0.5)
        adds.append(box(SM_X - 3.5, usb_y0, z_back(SM_USB_END) + 0.5, SM_X + 3.5, SM_USB_END - 0.5,
                        sm_z_comp - SM_USB[2] - 0.1))
    plate = plate.fuse([a.common(above_back(0)) for a in adds])

    # floating plinth, built in the desk frame (back plane = z 0). Its sides slope out at 45° up to
    # the plate edge, so the plate prints desk side down with no flat overhang round the plinth.
    pw, ph, pr = W - 2 * PLINTH_INSET, H_DESK - 2 * PLINTH_INSET, PLINTH_R
    g = PLINTH_H + 0.01
    plinth = Part.makeLoft([rr_wire(pw, ph, pr, -PLINTH_H), rr_wire(pw + 2 * g, ph + 2 * g, pr + g, 0.01)], True, True)
    plinth = place(plinth, W / 2, H_DESK / 2)
    zb = -PLINTH_H
    sa, ca = math.sin(ALPHA), math.cos(ALPHA)
    under = []  # desk-side details, mirrored so they read correctly with the case turned over
    # the wordmark is engraved into the bed side: a recessed box would leave a bridge over it
    for t in (text_solid("KIVORI", W / 2, H_DESK / 2, 5.0, zb - 0.1, TEXT_D + 0.1, mirror=True),):
        if t:
            under.append(t)
    for (x, y), label in ([] if SM_FRONT else zip(sm_btns, ["RST", "BOOT"])):  # BOOT left, RST right
        yd = (y + PLINTH_H * sa) / ca  # where the vertical pinhole leaves the plinth bottom
        t = label_cut(label, x, yd - 5.5, 2.0, zb - 0.1, mirror=True)
        if t:
            under.append(t)
    plinth = plinth.cut(under)
    # clip the slope to the plate's own (tapered) outline continued past the back plane
    env = tapered(W - 2 * WALL - 2 * BP_CLR, H - 2 * WALL - 2 * BP_CLR, R_CORNER - WALL - BP_CLR, 0, -80)
    plinth = desk_to_face(plinth).common(env.cut(above_back(0.02)))
    plate = plate.fuse(plinth)

    cuts = [cyl(PINHOLE_D, x, y, -60, tube_top + 1) for (x, y) in ([] if SM_FRONT else sm_btns)]
    for (x, y) in POSTS:  # screw through each column: clearance hole and a counterbore from below
        cuts.append(cyl(SCREW_CLR, x, y, -60, C_BACK + 1))
        cuts.append(cyl(SCREW_HEAD_D, x, y, -60, SCREW_SEAT_Z))
        cuts.append(cyl(SPIGOT_HOLE_D, x, y, C_BACK - SPIGOT_INTO_COL - SPIGOT_SOCKET_CLR, C_BACK + 1))
    return plate.cut(cuts).removeSplitter()


def enc_cradle():
    """The HW-040's corner brackets and pillars on the back plate (see ENC_BORDER)."""
    clr, wt, wh, ledge = ENC_BORDER
    L, Wd = HW040[0] + 2 * clr, HW040[1] + 2 * clr
    outer = lambda z0, z1: cbox(hw_c[0], hw_c[1], L + 2 * wt, Wd + 2 * wt, z0, z1)
    ring = outer(z_back(hw_c[1] + Wd) - 1, hw_back).cut(
        cbox(hw_c[0], hw_c[1], HW040[0] - 2 * ledge, HW040[1] - 2 * ledge, -60, 0))
    wall = outer(hw_back - 0.01, hw_back + wh).cut(cbox(hw_c[0], hw_c[1], L, Wd, -60, 0))
    a = ENC_CORNER + clr + wt
    corners = fuse_all([cbox(hw_c[0] + su * (L / 2 + wt - a / 2), hw_c[1] + sv * (Wd / 2 + wt - a / 2), a, a, -60, 0)
                        for su in (-1, 1) for sv in (-1, 1)])
    parts = [ring.fuse(wall).common(corners)]
    for (x, y) in hw_holes:
        parts += [cyl(HW040_COL_D, x, y, z_back(y) - 1, hw_back),
                  peg(x, y, HW040_HOLE_D, hw_back - 0.01, HW040_FRONT + PEG_PROUD, HW040_PEG_CLR)]
    return fuse_all(parts)


def dot_positions():
    """(dot centre, screw hole) pairs in the desk frame: each dot centred on its screw."""
    sa, ca = math.sin(ALPHA), math.cos(ALPHA)
    holes = [(x, (y + PLINTH_H * sa) / ca) for (x, y) in POSTS]  # screw axis at the plinth bottom
    return [(h, h) for h in holes]


def make_rubber_dots():
    """All four TPU dots as one part (one print), each with a plug on top."""
    zb = -PLINTH_H
    dots = []
    for (fx, fy), (hx, hy) in dot_positions():
        d = cyl(DOT_D, fx, fy, zb - DOT_T, zb)
        try:
            d = d.makeChamfer(0.4, [e for e in d.Edges if abs(e.BoundBox.ZMax - (zb - DOT_T)) < 1e-6])
        except Exception as exc:
            print("dot chamfer skipped:", exc)
        dots.append(d.fuse(cyl(DOT_PEG_D, hx, hy, zb - 0.01, zb + DOT_PEG_L)))
    return desk_to_face(Part.makeCompound(dots))


def carrier_outline():
    c = rr_prism(W - 2 * WALL - 2 * C_INSET, H - 2 * WALL - 2 * C_INSET, R_CORNER - WALL - C_INSET,
                 W / 2, H / 2, C_BACK, C_FRONT)
    # tab towards the far wall (it leans out, so measured at the carrier front): keeps the
    # SuperMini's first pin holes inside the board now that the board sits up against the wall
    y_tab = H - WALL - C_FRONT * TAPER - C_TAB_CLR
    hw = SM[0] / 2 + 1.0
    if not (COPPER_TRACES and SM_FRONT):
        return c.fuse(box(SM_X - hw, H / 2, C_BACK, SM_X + hw, y_tab, C_FRONT)).removeSplitter()
    # copper: the right row's last pad (B3, which runs on along the edge) ends flush with the board
    # edge, so no thin strip of plastic is left past its gap (it would turn to gap and leave the
    # board above it hanging over); the step down lines up with the pad's gap
    xb = sm_rows[1]
    y_pad = max(y for (x, y) in sm_pins.values() if abs(x - xb) < 1e-6) + TRACE_PAD[1] / 2
    x_step = xb - TRACE_PAD[0] / 2 - TRACE_GROOVE[0]
    return c.fuse([box(SM_X - hw, H / 2, C_BACK, x_step, y_tab, C_FRONT),
                   box(x_step - 0.01, H / 2, C_BACK, xb + TRACE_PAD[0] / 2, y_pad, C_FRONT)]).removeSplitter()


def carrier_through_cuts(pcb=False):
    """Everything that goes right through the board. pcb=True gives what a real PCB keeps (the
    SuperMini pin holes); the printed carrier gets slots for the header strips instead."""
    cuts = []
    zp = C_BACK + PIN_SKIN if COPPER_TRACES and PIN_SKIN > 0 and not pcb else C_BACK - 1  # blind if PIN_SKIN
    for (side, yc) in HOOKS:  # clear the snap hooks
        x0, x1 = (0, WALL + 2.8) if side < 0 else (W - WALL - 2.8, W)
        cuts.append(box(x0, yc - HOOK_W / 2 - 1.2, C_BACK - 1, x1, yc + HOOK_W / 2 + 1.2, C_FRONT + 1))
    for (x, y) in POSTS:
        cuts.append(cyl(SPIGOT_HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
    # the EC11 bushing reaches through from the HW-040 hanging behind (its body stops just behind)
    cuts.append(cyl(EC11_BUSH_D + 2 * EC11_HOLE_CLR, KNOB_C[0], KNOB_C[1], C_BACK - 1, C_FRONT + 1))
    if COPPER_TRACES:  # a hole per display pin, each in a pad
        cuts += [cyl(HOLE_D, x, y, zp, C_FRONT + 1) for (x, y) in disp_pins.values()]
    else:
        cuts.append(cbox(disp_hdr_x, dpcb_c[1], 2.6, 7 * 2.54 + 1.0, C_BACK - 1, C_FRONT + 1))
    for i in range(3):
        for p in range(4):
            x, y = btn_pin(i, p)
            cuts.append(cyl(SW_HOLE_D, x, y, zp, C_FRONT + 1))
        for k in ([] if COPPER_TRACES else range(len(SW_PEGS))):  # bosses some switches have (not TS13)
            x, y = btn_peg(i, k)
            cuts.append(cyl(SW_PEG_HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
    if pcb:
        cuts += [cyl(HOLE_D, x, y, C_BACK - 1, C_FRONT + 1) for (x, y) in sm_pins.values()]
        return cuts
    if SM_FRONT:  # its soldered pins go through to the copper
        cuts += [cyl(HOLE_D, x, y, zp, C_FRONT + 1) for (x, y) in sm_pins.values()]
    if COPPER_TRACES:  # the HW-040's bent pins
        cuts += [cyl(HOLE_D, x, y, zp, C_FRONT + 1) for (x, y) in enc_pins.values()]
        return cuts
    w, l = SM_HDR[0] + 2 * SM_HDR_CLR, SM_HDR[1] + 2 * SM_HDR_CLR
    c = SM_HDR_CHAMFER
    for xr in sm_rows:
        cuts.append(cbox(xr, sm_hdr_y, w, l, C_BACK - 1, C_FRONT + 1))
        border = Part.makeLoft([rr_wire(w + 2 * c, l + 2 * c, 0.3 + c, C_BACK - 0.01),
                                rr_wire(w, l, 0.3, C_BACK + c)], True)
        cuts.append(place(border, xr, sm_hdr_y))
    return cuts


def pin_skins():
    """The PIN_SKIN plugs left over the pin holes on the copper side (drilled out by hand)."""
    if not COPPER_TRACES or PIN_SKIN <= 0:
        return []
    pins = list(disp_pins.values()) + list(enc_pins.values()) + (list(sm_pins.values()) if SM_FRONT else [])
    z0, z1 = C_BACK - 1, C_BACK + PIN_SKIN + 0.01
    return ([cyl(HOLE_D, x, y, z0, z1) for (x, y) in pins] +
            [cyl(SW_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(3) for k in range(4)])


def copper_layout():
    """Nets as {name: (pads, traces)} plus the crossovers, laid out by hand for the current part
    positions; check_traces() reports the clearance between nets after any change.
    Pads: ("rect", x, y, size_x, size_y), square-cornered. Traces: point lists, or ("w", width,
    points), every segment along x or y. Crossovers: [(x0, y0, x1, y1), [nets on top]].

    Pin map chosen for this board (README, Wiring): the right SuperMini row feeds the whole display
    and the right button; the left row feeds the encoder and the other two buttons. Unused pins
    (5V, IO9, IO21) and the unused leg of each switch's signal pair get no copper.
    - Display: SCL, SDA, RES, DC, BLK leave the right row side by side and step down onto the
      display pins in order; VCC and GND come in from the right.
    - Encoder: DT, SW, 3V3 and GND run down between the rows and into its pins from the east,
      CLK down the outside of the left row onto its pin from the west.
    - B1 and B2 go round the outside of the knob and along the bottom edge, rising into their
      switches between the switch legs (the switch's own pin pairs bridge them). B3 runs along
      the top edge and down the right edge.
    - GND runs from the encoder down to the switches, from switch to switch through each switch's
      lower pin pair, and up the right edge to the display. VCC crosses GND once (crossover)."""
    xa, xb = sm_rows                                  # left / right pin row
    ys = {n: p[1] for n, p in sm_pins.items()}
    yd = {n: p[1] for n, p in disp_pins.items()}
    ye = {n: p[1] for n, p in enc_pins.items()}
    leg = lambda i, k: btn_pin(i, k)                  # k: 0/1 lower pin pair, 2/3 upper pair

    def sw(i, k):  # square pad; the right-hand pair (GND below, signal above) grow towards each
        x, y = leg(i, k)  # other until one gap is left, so no wide gap opens between them
        if k not in (1, 3):
            return ("rect", x, y, TRACE_PAD_D, TRACE_PAD_D)
        oy = leg(i, 4 - k)[1]
        ext = (abs(oy - y) - TRACE_PAD_D - TRACE_GROOVE[0]) / 2
        return ("rect", x, y + math.copysign(ext / 2, oy - y), TRACE_PAD_D, TRACE_PAD_D + ext)
    pin = lambda x, y: ("rect", x, y, TRACE_PAD[0], TRACE_PAD[1])  # every pin row runs along y
    sm = lambda n: pin(*sm_pins[n])
    dd = lambda n: pin(*disp_pins[n])
    ep = lambda n: pin(*enc_pins[n])
    P = TRACE_W + TRACE_GROOVE[0]                     # pitch of side-by-side traces: one gap apart
    # between the rows: four lanes, DT SW 3V3 GND. GND runs one gap from the right row's pads, so
    # its gap lines up with theirs (also by the unused pins there); DT passes the encoder's CLK pad
    # with TRACE_RIDGE_MIN of plastic between the two gaps; SW and 3V3 spread evenly between them
    gr = TRACE_GROOVE[0]
    l0 = max(xa + TRACE_PAD[0] / 2 + gr, enc_x + TRACE_PAD[0] / 2 + 2 * gr + TRACE_RIDGE_MIN) + TRACE_W / 2
    l3 = xb - TRACE_PAD[0] / 2 - gr - TRACE_W / 2
    lane = [l0 + k * (l3 - l0) / 3 for k in range(4)]

    def down(n, k, y_end):  # from a left-row pin east into lane k, down, and west onto the encoder
        return [(xa, ys[n]), (lane[k], ys[n]), (lane[k], y_end), (enc_x, y_end)]

    # display fan: right-row pins east, then down at these x, then east onto the display pins
    xf = 64.0
    fan = {"SCL": ("IO6", xf + 4 * P), "SDA": ("IO7", xf + 3 * P), "RES": ("IO8", xf + 2 * P),
           "DC": ("IO10", xf + P), "BLK": ("IO20", xf)}
    yv = yd["BLK"] - P                                # VCC's run under the display
    xv = disp_hdr_x + TRACE_PAD[0] / 2 + TRACE_GROOVE[0] + TRACE_W / 2  # VCC up the east side of the display pins
    xg, x5 = xv + P, xv + 2 * P                       # GND outside it, B3 outside that
    # B3 keeps 1 mm of plastic between its gap and the top-right post hole
    post = POSTS[2]
    keep = SPIGOT_HOLE_D / 2 + TRACE_RIDGE_MIN + TRACE_GROOVE[0] + TRACE_W / 2
    # ... and its run under the post lies one gap above GND's run onto the display (any further
    # up, the plastic strip left between the two gaps is too thin and turns into one wide gap)
    xb3, yb3 = post[0] - keep, min(post[1] - keep, yd["GND"] + P)
    xc = xa - TRACE_PAD[0] / 2 - TRACE_GROOVE[0] - TRACE_W / 2  # CLK down the outside of the left row
    # GND down to the switches, 1 mm of plastic clear of the left switch's unused leg
    xge = btn_pin(0, 2)[0] - SW_HOLE_D / 2 - TRACE_RIDGE_MIN - TRACE_GROOVE[0] - TRACE_W / 2
    bl3 = leg(0, 0)[1] - TRACE_PAD_D / 2 - TRACE_GROOVE[0] - TRACE_W / 2  # B1 along the bottom, under switch 1
    bl2 = bl3 - P                                     # B2 below it
    xw3 = 13.6                                        # B1 down the left side ...
    xw2 = xw3 - P                                     # ... B2 outside it
    b1x = (leg(0, 0)[0] + leg(0, 1)[0]) / 2           # rise between the legs of switch 1 ...
    b2x = (leg(1, 0)[0] + leg(1, 1)[0]) / 2           # ... and 2
    y_sw = (leg(0, 1)[1] + leg(1, 0)[1]) / 2          # GND from switch to switch, inside both pads
    y_gx = leg(2, 3)[1] + TRACE_PAD_D / 2 + TRACE_GROOVE[0] + TRACE_W / 2  # GND steps over the right switch's pad
    xgl = leg(2, 3)[0] - TRACE_PAD_D / 2 - TRACE_GROOVE[0] - TRACE_W / 2  # ... after rising one gap west of it
    nets = {
        "GND": ([sm("GND"), dd("GND"), ep("GND")] + [sw(i, k) for i in range(3) for k in (0, 1)],
                [down("GND", 3, ye["GND"]),
                 # to the left switch's lower-left leg, west of its unused upper-left leg
                 [(xge, ye["GND"]), (xge, leg(0, 0)[1]), leg(0, 0)],
                 [leg(0, 1), (leg(0, 1)[0], y_sw), (leg(1, 0)[0], y_sw), leg(1, 0)],
                 [leg(1, 1), (leg(1, 1)[0], y_sw), (leg(2, 0)[0], y_sw), leg(2, 0)],
                 [leg(2, 1), (xgl, leg(2, 1)[1]), (xgl, y_gx), (xg, y_gx), (xg, yd["GND"]), (disp_hdr_x, yd["GND"])]]),
        "3V3": ([sm("3V3"), dd("VCC"), ep("+")],
                [down("3V3", 2, ye["+"]),
                 [(lane[2], yv), (xv, yv), (xv, yd["VCC"]), (disp_hdr_x, yd["VCC"])]]),
        "DT": ([sm("IO0"), ep("DT")], [down("IO0", 0, ye["DT"])]),
        "SW": ([sm("IO1"), ep("SW")], [down("IO1", 1, ye["SW"])]),
        "CLK": ([sm("IO2"), ep("CLK")], [[(xa, ys["IO2"]), (xc, ys["IO2"]), (xc, ye["CLK"])]]),
        "B1": ([sm("IO3"), sw(0, 3)],
               [[(xa, ys["IO3"]), (xw3, ys["IO3"]), (xw3, bl3), (b1x, bl3), (b1x, leg(0, 3)[1]), leg(0, 3)]]),
        "B2": ([sm("IO4"), sw(1, 3)],
               [[(xa, ys["IO4"]), (xw2, ys["IO4"]), (xw2, bl2), (b2x, bl2), (b2x, leg(1, 3)[1]), leg(1, 3)]]),
        # along the top edge (the board edge trims its outer side), down the right edge
        "B3": ([sm("IO5"), sw(2, 3)],
               [[(xb, ys["IO5"]), (xb3, ys["IO5"]), (xb3, yb3), (x5, yb3), (x5, leg(2, 3)[1])]]),
    }
    for name, (p, x) in fan.items():
        y0, y1 = ys[p], yd[name]
        nets[name] = ([sm(p), dd(name)], [[(xb, y0), (x, y0), (x, y1), (disp_hdr_x, y1)]])
    crossovers = [((lane[3] - 2.1, yv - 1.6, lane[3] + 2.1, yv + 1.6), ["3V3"])]  # VCC over GND
    return nets, crossovers


def net_parts(nets, crossovers):
    """(copper on the board, copper laid on the crossover patches): the nets on top lose their
    part inside each patch from the first, and only that part goes in the second."""
    over = {}
    for rect, names in crossovers:
        for n in names:
            over.setdefault(n, []).append(rect)
    return over


def net_boxes(net, grow=0.0):
    """A net as axis-aligned rectangles (x0, y0, x1, y1): its pads, and each trace segment with a
    square end at every point (so every corner is square), all grown by grow."""
    pads, traces = net
    out = [(x - sx / 2 - grow, y - sy / 2 - grow, x + sx / 2 + grow, y + sy / 2 + grow) for (_, x, y, sx, sy) in pads]
    for t in traces:  # a point list, or ("w", width, points) for a narrower trace
        w, pts = (t[1], t[2]) if t and t[0] == "w" else (TRACE_W, t)
        h = w / 2 + grow
        for (ax, ay), (bx, by) in zip(pts, pts[1:] or pts):
            assert abs(ax - bx) < 1e-9 or abs(ay - by) < 1e-9, "trace segments run along x or y"
            out.append((min(ax, bx) - h, min(ay, by) - h, max(ax, bx) + h, max(ay, by) + h))
    return out


def net_solid(net, z0, z1, grow=0.0):
    return fuse_all([box(x0, y0, z0, x1, y1, z1) for (x0, y0, x1, y1) in net_boxes(net, grow)])


def patch_box(rect, z0, z1):
    return box(rect[0], rect[1], z0, rect[2], rect[3], z1)


def board_nets(z0, z1, grow=0.0, clip=True):
    """Each net's copper that lies on the board: nets on top of a crossover lose the part inside
    the patch (that part sits on the insulating tape, above the copper below). clip=False keeps
    them whole (the gaps draw every line in full, through the crossovers)."""
    nets, crossovers = copper_layout()
    over = net_parts(nets, crossovers) if clip else {}
    out = {}
    for k, n in nets.items():
        sol = net_solid(n, z0, z1, grow)
        for rect in over.get(k, []):
            sol = sol.cut(patch_box(rect, z0 - 1, z1 + 1))
        out[k] = sol
    return out


def bare_holes():
    """Through holes with no copper pad round them (unused pins and switch legs): (x, y, d)."""
    nets, _ = copper_layout()
    pads = [(x, y) for (pads_, _) in nets.values() for (_, x, y, _, _) in pads_]
    holes = [(x, y, HOLE_D) for d in (sm_pins, disp_pins, enc_pins) for (x, y) in d.values()]
    holes += [btn_pin(i, k) + (SW_HOLE_D,) for i in range(3) for k in range(4)]
    return [(x, y, d) for (x, y, d) in holes if not any(math.hypot(x - px, y - py) < 1e-6 for (px, py) in pads)]


def grid_open(m, n):
    """Square opening of a pixel mask: what is left of m after an n x n pixel square has swept
    every place it fits. Anything narrower than n pixels goes; square corners stay square."""
    import numpy as np
    lo, hi = n // 2, n - 1 - n // 2

    def window_sum(a, l, h):  # sum over [i - l, i + h] x [j - l, j + h]
        c = np.zeros((a.shape[0] + l + h + 1, a.shape[1] + l + h + 1), np.int32)
        c[l + 1:l + 1 + a.shape[0], l + 1:l + 1 + a.shape[1]] = a
        c = c.cumsum(0).cumsum(1)
        H, W = a.shape
        return (c[l + h + 1:l + h + 1 + H, l + h + 1:l + h + 1 + W] - c[:H, l + h + 1:l + h + 1 + W]
                - c[l + h + 1:l + h + 1 + H, :W] + c[:H, :W])

    eroded = window_sum(m, lo, hi) == n * n
    return window_sum(eroded, hi, lo) > 0


def grid_rects(mask, x0, y0, res, xs, ys, keep):
    """Connected pieces of a pixel mask, keep(piece) decides, as rectangles (x0, y0, x1, y1) with
    their edges snapped onto the nearest exact edge in xs / ys (within a pixel)."""
    import bisect
    runs = []  # (row, i0, i1)
    for j in range(mask.shape[0]):
        row = mask[j]
        if not row.any():
            continue
        d = [int(k) for k in (row[1:] != row[:-1]).nonzero()[0] + 1]
        edges = ([0] if row[0] else []) + d + ([len(row)] if row[-1] else [])
        runs += [(j, edges[k], edges[k + 1]) for k in range(0, len(edges), 2)]
    parent = list(range(len(runs)))

    def find(k):
        while parent[k] != k:
            parent[k] = parent[parent[k]]
            k = parent[k]
        return k
    by_row = {}
    for k, (j, i0, i1) in enumerate(runs):
        by_row.setdefault(j, []).append(k)
        for q in by_row.get(j - 1, []):
            if runs[q][1] < i1 and i0 < runs[q][2]:
                parent[find(k)] = find(q)
    pieces = {}
    for k in range(len(runs)):
        pieces.setdefault(find(k), []).append(runs[k])

    def snap(v, vals):
        i = bisect.bisect_left(vals, v)
        best = min(vals[max(i - 1, 0):i + 1], key=lambda s: abs(s - v), default=v)
        return best if abs(best - v) <= 1.01 * res else v
    rects = []
    for piece in pieces.values():
        if not keep(piece):
            continue
        open_ = {}  # (i0, i1) -> [j0, j1]: runs stacked into rectangles
        for (j, i0, i1) in sorted(piece):
            r = open_.get((i0, i1))
            if r and r[1] == j:
                r[1] = j + 1
            else:
                if r:
                    rects.append((i0, r[0], i1, r[1]))
                open_[(i0, i1)] = [j, j + 1]
        rects += [(i0, j0, i1, j1) for (i0, i1), (j0, j1) in open_.items()]
    out = [(snap(x0 + i0 * res, xs), snap(y0 + j0 * res, ys), snap(x0 + i1 * res, xs), snap(y0 + j1 * res, ys))
           for (i0, j0, i1, j1) in rects]
    return [r for r in out if r[2] - r[0] > 1e-3 and r[3] - r[1] > 1e-3]  # (snapping can close one)


def copper_grooves():
    """The gap along both sides of every trace and pad, cut into the printed PCB's back, cleaned up
    for FDM on a GROOVE_GRID pixel grid of the back:
    - plastic left between two gaps (or a gap and a hole or the edge) narrower than
      TRACE_RIDGE_MIN becomes gap, and so does an island of plastic under TRACE_ISLAND_MIN;
    - a gap narrower than TRACE_GROOVE_MIN (a crumb next to a hole or the edge) is filled in;
    - the plastic round a hole without copper (unused pin or switch leg) stays whole.
    The changes are rectangles snapped onto the traces' edges, so every corner stays square.
    Between neighbouring pins of a pin row the gap is the PIN_GAP cut line. Every line is drawn in
    full, through the crossovers too (cut the crossing apart by hand there)."""
    import numpy as np
    z0, z1, res = C_BACK - 1, C_BACK + TRACE_GROOVE[1], GROOVE_GRID
    nets, _ = copper_layout()
    gap_b = [b for n in nets.values() for b in net_boxes(n, TRACE_GROOVE[0])]
    cu_b = [b for n in nets.values() for b in net_boxes(n)]
    board = carrier_outline().cut(carrier_through_cuts())
    plate = Part.makeCompound([f.copy() for f in board.Faces
                               if abs(f.normalAt(0, 0).z + 1) < 1e-6 and abs(f.CenterOfMass.z - C_BACK) < 1e-6])
    bb = plate.BoundBox
    X0, Y0 = bb.XMin - 2 * res, bb.YMin - 2 * res
    nx, ny = int(bb.XLength / res) + 5, int(bb.YLength / res) + 5
    xc = X0 + (np.arange(nx) + 0.5) * res  # pixel centres
    yc = Y0 + (np.arange(ny) + 0.5) * res

    def fill(m, rects):
        for (a0, b0, a1, b1) in rects:
            i0, i1 = int(np.ceil((a0 - X0) / res - 0.5)), int(np.floor((a1 - X0) / res - 0.5)) + 1
            j0, j1 = int(np.ceil((b0 - Y0) / res - 0.5)), int(np.floor((b1 - Y0) / res - 0.5)) + 1
            m[max(j0, 0):max(j1, 0), max(i0, 0):max(i1, 0)] = True
        return m
    board_m = np.zeros((ny, nx), bool)  # the back, row by row
    for j, y in enumerate(yc):
        line = Part.makeLine(V(X0, y, C_BACK), V(X0 + nx * res, y, C_BACK))
        for e in plate.common(line).Edges:
            a, b = sorted((e.Vertexes[0].X, e.Vertexes[-1].X))
            board_m[j, (xc >= a) & (xc <= b)] = True
    solid_m = board_m.copy()  # holes without copper (and the post holes) count as plastic: it stays whole round them
    for (x, y, d) in bare_holes() + [(x, y, SPIGOT_HOLE_D) for (x, y) in POSTS]:
        solid_m |= ((xc[None, :] - x) ** 2 + (yc[:, None] - y) ** 2) <= (d / 2) ** 2
    gap_m = fill(np.zeros((ny, nx), bool), gap_b)
    cu_m = fill(np.zeros((ny, nx), bool), cu_b)
    near_gap = gap_m.copy()
    near_gap[1:] |= gap_m[:-1]; near_gap[:-1] |= gap_m[1:]; near_gap[:, 1:] |= gap_m[:, :-1]; near_gap[:, :-1] |= gap_m[:, 1:]
    xs = sorted({v for b in gap_b + cu_b for v in (b[0], b[2])} | {e.Vertexes[0].X for e in plate.Edges
                if e.Curve.TypeId == "Part::GeomLine" and abs(e.Vertexes[0].X - e.Vertexes[-1].X) < 1e-6})
    ys = sorted({v for b in gap_b + cu_b for v in (b[1], b[3])} | {e.Vertexes[0].Y for e in plate.Edges
                if e.Curve.TypeId == "Part::GeomLine" and abs(e.Vertexes[0].Y - e.Vertexes[-1].Y) < 1e-6})
    touching = lambda piece: any(near_gap[j, i0:i1].any() for (j, i0, i1) in piece)
    big = lambda piece: sum(i1 - i0 for (_, i0, i1) in piece) * res * res > 0.05 and \
        min(max(i1 for (_, _, i1) in piece) - min(i0 for (_, i0, _) in piece),
            max(j for (j, _, _) in piece) - min(j for (j, _, _) in piece) + 1) > 2  # no hairlines

    # plastic between the gaps narrower than TRACE_RIDGE_MIN: gap
    plastic = solid_m & ~gap_m
    n = int(round(TRACE_RIDGE_MIN / res)) - 1  # one pixel under, so a ridge just over the limit is not aliased away
    thin = plastic & ~grid_open(plastic, n)
    ridge_r = grid_rects(thin, X0, Y0, res, xs, ys, lambda p: touching(p) and big(p))
    # gaps narrower than TRACE_GROOVE_MIN (crumbs next to a hole or the edge): plastic
    gaps = board_m & gap_m & ~cu_m
    n = int(round(TRACE_GROOVE_MIN / res))
    crumb_r = grid_rects(gaps & ~grid_open(gaps, n), X0, Y0, res, xs, ys, big)

    # widest gap: the layer over a gap bridges it, so gaps should stay narrow
    gm = board_m & (gap_m | thin) & ~cu_m
    n = 2
    while n < 200 and grid_open(gm, n + 2).any():
        n += 2
    wide = grid_rects(grid_open(gm, n), X0, Y0, res, xs, ys, lambda p: True)
    print("widest gap (the bridge over it): %.1f mm, at %s" % (n * res, ", ".join(
        "(%.1f, %.1f)" % ((r[0] + r[2]) / 2, (r[1] + r[3]) / 2) for r in wide[:4])))
    slab = lambda rects, za, zb: fuse_all([box(a0, b0, za, a1, b1, zb) for (a0, b0, a1, b1) in rects])
    # the open area at the back face: the gaps, the holes and everything outside the board
    zb, D = C_BACK, TRACE_GROOVE[1]
    back = Part.makeCompound([f.extrude(V(0, 0, 1)) for f in plate.Faces])  # the board, C_BACK .. +1
    cut = slab(gap_b + ridge_r, zb - 1, zb + 1)
    islands = [s_ for s_ in back.cut(cut).Solids if s_.Volume < TRACE_ISLAND_MIN]  # too small to print: gap
    shown = [s_.Volume for s_ in islands if s_.Volume > 0.05]
    print("plastic slivers made gap: %d pieces, %.1f mm²; islands made gap: %d (%s mm²); gap crumbs filled: %d" % (
        len(ridge_r), sum((r[2] - r[0]) * (r[3] - r[1]) for r in ridge_r), len(shown),
        ", ".join("%.1f" % a for a in shown), len(crumb_r)))
    if ridge_r:
        print("  slivers at:", ", ".join("(%.1f, %.1f) %.1fx%.1f" % ((r[0] + r[2]) / 2, (r[1] + r[3]) / 2,
                                                                     r[2] - r[0], r[3] - r[1]) for r in ridge_r))
    outside = box(bb.XMin - 5, bb.YMin - 5, zb - 1, bb.XMax + 5, bb.YMax + 5, zb + 1).cut(back)
    open_ = cut.fuse([outside] + islands).cut(slab(cu_b, zb - 2, zb + 2))
    if crumb_r:
        open_ = open_.cut(slab(crumb_r, zb - 2, zb + 2))
    faces = [f for f in open_.removeSplitter().Faces
             if abs(f.normalAt(0, 0).z - 1) < 1e-6 and abs(f.CenterOfMass.z - (zb + 1)) < 1e-6]
    # straight (90°) walls: a flat-bottomed gap the blade can follow; kept narrow, so the layer
    # over it is only a short bridge
    out = []
    for f in faces:
        f = f.copy()
        f.translate(V(0, 0, z0 - f.CenterOfMass.z))
        out.append(f.extrude(V(0, 0, z1 - z0)))
    board_zone = Part.makeCompound([f.extrude(V(0, 0, z1 - z0)) for f in plate.Faces])
    board_zone.translate(V(0, 0, z0 - C_BACK))
    return board_zone.common(fuse_all(out)).removeSplitter()


def check_traces(pads=True):
    """Gap between every two nets, smallest first (the crossovers left out): at least one PIN_GAP
    between neighbouring pins of a row, and with pads=False (the traces alone) one TRACE_GROOVE."""
    nets, crossovers = copper_layout()
    over = net_parts(nets, crossovers)
    sol = {}
    for k, n in nets.items():
        sol[k] = net_solid(n if pads else ([], n[1]), C_BACK - TAPE_T, C_BACK)
        for rect in over.get(k, []):
            sol[k] = sol[k].cut(patch_box(rect, C_BACK - 1, C_BACK + 1))
    keys, pairs = list(sol), []
    for i in range(len(keys)):
        for j in range(i + 1, len(keys)):
            pairs.append((sol[keys[i]].distToShape(sol[keys[j]])[0], keys[i], keys[j]))
    return sorted(pairs)


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
    c = c.cut(cbox(disp_hdr_x, dpcb_c[1], 2.6, 7 * 2.54 + 1.0, C_FRONT, C_FRONT + 6))
    if not COPPER_TRACES:
        c = c.cut(cbox(disp_hdr_x, dpcb_c[1], 2.6, 7 * 2.54 + 1.0, C_BACK - 1, C_FRONT + 1))
    else:
        c = c.cut(copper_grooves())

    # labels engraved on the back, mirrored so they read correctly from behind
    # (none with copper traces: fine print on the bed side prints badly, the traces say it all)
    labels = [] if COPPER_TRACES else [("KIVORI PROTO v3", 25.0, 60.0, 2.0)]
    for n, (x, y) in ({} if COPPER_TRACES else disp_pins).items():
        labels.append((n, x - DISP_HEADER_SIDE * 5.0, y, 1.4))
    for i, (x, y, a) in enumerate([] if COPPER_TRACES else BTNS):
        labels.append(("B%d %s" % (i + 1, BTN_GPIO[i]), x, y - 9.5, 1.6))
    tcuts = [label_cut(s, x, y, sz, C_BACK - 0.1, mirror=True) for (s, x, y, sz) in labels]
    tcuts = [t for t in tcuts if t]
    if tcuts:
        c = c.cut(tcuts)
    return c.removeSplitter()


def make_knob():
    z0, z1 = KNOB_GAP, KNOB_GAP + KNOB_H
    cx, cy = KNOB_C
    k = cyl(KNOB_D, cx, cy, z0, z1)
    try:
        top = [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle" and abs(e.BoundBox.ZMin - z1) < 1e-6]
        bot = [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle" and abs(e.BoundBox.ZMin - z0) < 1e-6]
        k = k.makeChamfer(1.2, top)  # chamfers, not rounds: the top prints on the bed
        k = k.makeChamfer(0.5, [e for e in k.Edges if e.Curve.TypeId == "Part::GeomCircle"
                                and abs(e.BoundBox.ZMin - z0) < 1e-6 and e.Curve.Radius > KNOB_D / 2 - 0.1])
    except Exception as exc:
        print("knob chamfer skipped:", exc)
    r = KNOB_D / 2
    flutes = []
    for i in range(KNOB_FLUTES):
        a = 2 * math.pi * i / KNOB_FLUTES
        flutes.append(cyl(KNOB_FLUTE_D, cx + r * math.cos(a), cy + r * math.sin(a), z0 - 1, z1 - 1.8))  # out the skirt edge
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
    bore = cyl(EC11_SHAFT_D + 0.25, cx, cy, hub_z0 - 1, shaft_top + 0.5)  # snug once FDM shrinks it
    flat = box(cx - 5, cy + EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, shaft_top - EC11_FLAT_L + 0.5, cx + 5, cy + 5, shaft_top + 1)
    cuts.append(bore.cut(flat))
    m0, m1, mang, mw = KNOB_MARK
    mark = box(m0, -mw / 2, z1 - 0.6, m1, mw / 2, z1 + 1)
    cuts.append(place(mark, cx, cy, mang))
    return k.cut(cuts).removeSplitter()


def cap_snap(z_ceil):
    """Four snap fingers round the stem head and the stop post, round the origin."""
    w, t = CAP_FINGER
    r_i = SW_HEAD / 2 + CAP_SNAP_CLR
    r_o, r_l = r_i + t, SW_HEAD / 2 - CAP_SNAP
    z_k = SW_PLUNGER_TOP - SW_HEAD_H - 0.05   # catch face, just under the head
    z_b = z_k - 0.2 - (r_i - r_l)             # 45° lead-in below the lip
    prof = [(r_o, z_ceil + 0.01), (r_i, z_ceil + 0.01), (r_i, z_k), (r_l, z_k), (r_l, z_k - 0.2),
            (r_i, z_b), (r_o, z_b)]
    f = Part.Face(Part.makePolygon([V(px, 0, pz) for (px, pz) in prof + [prof[0]]])).extrude(V(0, w, 0))
    f.translate(V(0, -w / 2, 0))
    parts = [place(f, 0, 0, k * 90.0) for k in range(4)]
    parts.append(cyl(CAP_STOP_D, 0, 0, SW_PLUNGER_TOP, z_ceil + 0.01))
    if z_ceil - SW_PLUNGER_TOP > 0.5:  # sleeve tying the fingers together above the head
        parts.append(cbox(0, 0, 2 * r_o, 2 * r_o, SW_PLUNGER_TOP, z_ceil + 0.01).cut(
            cbox(0, 0, 2 * r_i, 2 * r_i, SW_PLUNGER_TOP - 1, z_ceil - 0.01)))
    return fuse_all(parts)


def make_cap(i):
    """Hollow cap with a flat top, open underneath, that clicks onto the switch stem (cap_snap).
    Prints top down: the top comes off the bed flat, the fingers grow up from it."""
    x, y, a = BTNS[i]
    z_flange = -FACE_T - 0.3 - BTN_FLANGE_T
    z_ceil = BTN_CAP_PROUD - CAP_TOP_T
    body = rr_prism(BTN_CAP, BTN_CAP, 1.2, x, y, z_flange, BTN_CAP_PROUD, a)
    try:  # a chamfer, not a round: the top edge is on the bed
        top = [e for e in body.Edges if abs(e.BoundBox.ZMin - BTN_CAP_PROUD) < 1e-6 and abs(e.BoundBox.ZMax - BTN_CAP_PROUD) < 1e-6]
        body = body.makeChamfer(0.5, top)
    except Exception as exc:
        print("cap chamfer skipped:", exc)
    # flange: a 0.3 mm land, then 45° in to the body (the cap prints top down: no flat ledge)
    ch = (BTN_FLANGE - BTN_CAP) / 2
    flange = Part.makeLoft([rr_wire(BTN_FLANGE, BTN_FLANGE, 1.2 + ch, z_flange),  # (corners 45° too)
                            rr_wire(BTN_FLANGE, BTN_FLANGE, 1.2 + ch, z_flange + BTN_FLANGE_T - ch),
                            rr_wire(BTN_CAP, BTN_CAP, 1.2, z_flange + BTN_FLANGE_T)], True, True)
    flange = place(flange, x, y, a)
    cap = body.fuse(flange)
    inner = BTN_CAP - 2 * CAP_WALL
    cap = cap.cut(rr_prism(inner, inner, 0.6, x, y, z_flange - 1, z_ceil, a))  # hollow
    cap = cap.fuse(place(cap_snap(z_ceil), x, y, a))
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
    # right-angle header: plastic on the front of the board, pins lying flat and pointing +x
    hdr = cbox(hw_hdr[0], hw_hdr[1], 2.5, 5 * 2.54, f, f + 2.5)
    pins = box(hw_hdr[0], hw_hdr[1] - 5.6, f + 1.0, hw_pin_tip_x, hw_hdr[1] + 5.6, f + 1.64)
    if COPPER_TRACES:  # straight pins, up through the printed PCB
        pins = fuse_all([cbox(x, y, 0.64, 0.64, f, f + HW040_PIN_L) for (x, y) in enc_pins.values()])
    # on its back: EC11 legs and resistors, header solder joints
    kx0, kx1, ky0, ky1 = HW040_BACK_KEEPOUT
    legs = box(KNOB_C[0] + kx0, KNOB_C[1] + ky0, hw_back - HW040_BACK_PARTS, KNOB_C[0] + kx1, KNOB_C[1] + ky1, hw_back + 0.01)
    joints = cbox(hw_hdr[0], hw_hdr[1], 1.6, 4 * 2.54 + 1.8, hw_back - HW040_BACK_PARTS, hw_back + 0.01)
    return fuse_all([pcb, body, bush, shaft, hdr, pins, legs, joints])


def dummy_display():
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], glass_z1, glass_z0)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], dpcb_z1, glass_z1)
    pcb = pcb.cut([cyl(DISP_HOLE_D, x, y, dpcb_z1 - 1, glass_z1 + 1) for (x, y) in disp_holes])
    hdr = cbox(disp_hdr_x, dpcb_c[1], 2.5, 7 * 2.54, dpcb_z1 - 2.5, dpcb_z1)
    if COPPER_TRACES:  # one pin per hole
        pins = fuse_all([cbox(x, y, 0.64, 0.64, C_BACK - 2.5, dpcb_z1 - 2.5) for (x, y) in disp_pins.values()])
    else:
        pins = cbox(disp_hdr_x, dpcb_c[1], 0.64, 7 * 2.54 - 1.9, C_BACK - 2.5, dpcb_z1 - 2.5)
    return fuse_all([glass, pcb, hdr, pins])


def dummy_switch(i):
    x, y, a = BTNS[i]
    body = rr_prism(SW_BODY, SW_BODY, 0.3, x, y, C_FRONT, C_FRONT + SW_BODY_H, a)
    boss = cyl(SW_BOSS_D, x, y, C_FRONT + SW_BODY_H - 0.01, C_FRONT + SW_BOSS_H)
    neck = cbox(x, y, SW_NECK, SW_NECK, C_FRONT + SW_BOSS_H - 0.01, SW_PLUNGER_TOP - SW_HEAD_H + 0.01, a)
    head = cbox(x, y, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H, SW_PLUNGER_TOP, a)
    head = head.cut(cyl(1.6, x, y, SW_PLUNGER_TOP - 1.2, SW_PLUNGER_TOP + 1))
    return fuse_all([body, boss, neck, head])


def dummy_supermini():
    pcb = cbox(sm_c[0], sm_c[1], SM[0], SM[1], sm_z_comp, sm_z_comp + SM[2])
    usb = cbox(SM_X, SM_USB_END - SM_USB[1] / 2 + SM_USB_OVERHANG, SM_USB[0], SM_USB[1], sm_z_comp - SM_USB[2], sm_z_comp)
    btns = [cbox(x, y, SM_BTN[0], SM_BTN[1], sm_z_comp - SM_BTN[2], sm_z_comp) for (x, y) in sm_btns]
    chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_z_comp - 0.9, sm_z_comp)
    z0 = sm_z_comp + SM[2]  # header strips on the carrier side, long pins towards the face
    if SM_FRONT:  # on its header strips on the carrier front, components up, pins through
        pcb = cbox(sm_c[0], sm_c[1], SM[0], SM[1], sm_lo, sm_hi)
        usb = cbox(SM_X, SM_USB_END - SM_USB[1] / 2 + SM_USB_OVERHANG, SM_USB[0], SM_USB[1], usb_lo, usb_hi)
        btns = [cbox(x, y, SM_BTN[0], SM_BTN[1], sm_hi, sm_hi + SM_BTN[2]) for (x, y) in sm_btns]
        chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_hi, sm_hi + 0.9)
        hdr = [cbox(xr, sm_hdr_y, SM_HDR[0], SM_HDR[1], C_FRONT, sm_lo) for xr in sm_rows]
        pins = [cbox(x, y, 0.64, 0.64, C_BACK - 1.5, sm_hi + 0.5) for (x, y) in sm_pins.values()]
        return fuse_all([pcb, usb, chip] + btns + hdr + pins)
    hdr = [cbox(xr, sm_hdr_y, SM_HDR[0], SM_HDR[1], z0, z0 + SM_HDR[2]) for xr in sm_rows]
    pins = [cbox(xr, sm_hdr_y, 0.64, SM_HDR[1] - 1.9, sm_z_comp - 1.5, z0 + SM_HDR[2] + SM_HDR[3]) for xr in sm_rows]
    return fuse_all([pcb, usb, chip] + btns + hdr + pins)


def dummy_copper():
    """Copper tape flush on the back; at each crossover the upper traces' copper lies on the
    insulating tape over the lower copper."""
    nets, crossovers = copper_layout()
    t, k, f = TAPE_T, KAPTON_T, C_BACK
    parts = list(board_nets(f - t, f).values())
    for rect, names in crossovers:  # (the tape itself: Dummy_Kapton)
        for nm in names:
            parts.append(net_solid(nets[nm], f - 2 * t - k, f - t - k).common(patch_box(rect, C_BACK - 5, C_BACK + 5)))
    footprint = carrier_outline()  # the board edge trims the tape (B3 along the top edge)
    footprint.translate(V(0, 0, -C_T / 2))
    return fuse_all(parts).common(footprint).cut(carrier_through_cuts() + pin_skins())  # (drilled)


def dummy_kapton():
    """Insulating tape where the upper traces cross the lower copper."""
    nets, crossovers = copper_layout()
    f, t, k = C_BACK, TAPE_T, KAPTON_T
    parts = []
    for rect, names in crossovers:
        upper = fuse_all([net_solid(nets[n], f - t - k, f - t) for n in names])
        lower = fuse_all([net_solid(n, f - t - k, f - t) for m, n in nets.items() if m not in names])
        parts.append(upper.common(lower).common(patch_box(rect, C_BACK - 5, C_BACK + 5)))
    return fuse_all(parts)


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}

COLORS = {  # two-tone: light matte shell; black visor, knob and plinth; warm accent caps
    "Shell": (0.93, 0.93, 0.91), "Visor": (0.06, 0.06, 0.07), "BackPlate": (0.13, 0.13, 0.15),
    "Carrier": (0.12, 0.45, 0.25), "Knob": (0.10, 0.10, 0.11), "Cap": (0.95, 0.50, 0.18),
    "Dummy": (0.30, 0.45, 0.85), "RubberDots": (0.20, 0.20, 0.22),
    "Dummy_Copper": (0.85, 0.50, 0.25), "Dummy_Kapton": (0.95, 0.70, 0.15),
}


def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["Visor"] = make_visor()
    PRINTED["BackPlate"] = make_back_plate()
    PRINTED["RubberDots"] = make_rubber_dots()
    PRINTED["Carrier"] = make_carrier()
    PRINTED["Knob"] = make_knob()
    for i, n in enumerate(["CapLeft", "CapMiddle", "CapRight"]):
        PRINTED[n] = make_cap(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_SuperMini"] = dummy_supermini()
    if COPPER_TRACES:
        DUMMIES["Dummy_Copper"] = dummy_copper()
        DUMMIES["Dummy_Kapton"] = dummy_kapton()
    for i, n in enumerate(["Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"]):
        DUMMIES[n] = dummy_switch(i)


PRESS_FITS = {("BackPlate", "RubberDots")}  # TPU plugs (the screw counterbores lean 10°)


def check_interference():
    """Clashes between parts; the intended press fits are reported apart."""
    # checked with the pin holes drilled through (the skins are drilled out after taping)
    skins = pin_skins()
    items = [(n, sh.cut(skins) if n == "Carrier" and skins else sh) for n, sh in PRINTED.items()] + list(DUMMIES.items())
    clashes, fits = [], []
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            (na, a), (nb, b) = items[i], items[j]
            if not a.BoundBox.intersect(b.BoundBox):
                continue
            v = a.common(b).Volume
            if v > 0.01:
                (fits if (na, nb) in PRESS_FITS else clashes).append((na, nb, round(v, 3)))
    if fits:
        print("press fits (intended):", fits)
    return clashes


def to_doc():
    if DOC_NAME in App.listDocuments():
        App.closeDocument(DOC_NAME)
    doc = App.newDocument(DOC_NAME)
    for name, shp in list(PRINTED.items()) + list(DUMMIES.items()):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shp
        if App.GuiUp and obj.ViewObject:
            key = name if name in COLORS else ("Dummy" if name.startswith("Dummy") else ("Cap" if name.startswith("Cap") else name))
            obj.ViewObject.ShapeColor = COLORS[key]
            if name == "Shell":
                obj.ViewObject.Transparency = 55
    doc.recompute()
    return doc


def print_pose(name, shp):
    """Rotate a part into its print orientation, resting on z = 0."""
    s = shp.copy()
    if name in ("Shell", "Knob", "Visor") or name.startswith("Cap"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180)       # face / knob top / visor top / cap top on the bed
    elif name in ("BackPlate", "RubberDots"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))  # desk side (plinth) on the bed
    bb = s.optimalBoundingBox()  # the fast box is loose around curved faces
    s.translate(V(-bb.XMin, -bb.YMin, -bb.ZMin))
    return s


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
    doc.saveAs(os.path.join(here, "kivori-case.FCStd"))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    if COPPER_TRACES:
        pairs = check_traces()
        need = min(PIN_GAP, TRACE_GROOVE[0])  # neighbouring pins of a row: the PIN_GAP cut line
        print("trace clearance: %.2f mm (%s / %s), need %.2f: %s" % (pairs[0] + (need,
              "ok" if pairs[0][0] > need - 1e-6 else "TOO CLOSE")))
        t = check_traces(pads=False)[0]
        print("trace to trace: %.2f mm (%s / %s), need %.2f: %s" % (t + (TRACE_GROOVE[0],
              "ok" if t[0] > TRACE_GROOVE[0] - 1e-6 else "TOO CLOSE")))
    print("buttons:", [(round(x, 2), round(y, 2), round(a, 2)) for (x, y, a) in BTNS])
    for name, shp in PRINTED.items():
        bb = shp.BoundBox
        print("%-10s vol %8.0f mm3  bbox %.1f x %.1f x %.1f  valid=%s" % (
            name, shp.Volume, bb.XLength, bb.YLength, bb.ZLength, shp.isValid()))
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):  # freecadcmd imports the file as module "build_case"
    main()
