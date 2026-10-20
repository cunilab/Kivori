"""Kivori enclosure v3, "Living Keycap" (0.3 mm enamelled wire, no solder): parametric build script (FreeCAD 1.1, Part workbench, no GUI needed).

Run it inside FreeCAD (GUI Python console, the FreeCAD MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-v3/build_case.py"
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
R_CORNER = 12.0             # face corner radius (soft, keycap-like)
T_NEAR, T_FAR = 20.7, 32.7  # case thickness at the near (y=0) and far (y=H) edge -> ~9.7° wedge
WALL = 2.0                  # side wall thickness (horizontal)
WALL_TAPER = 10.0           # degrees the walls lean in towards the face: the keycap skirt
FACE_T = 2.5                # faceplate thickness (room for the visor pocket in front)
FACE_BEVEL = 4.0            # keycap shoulder round the face (inset at the face); a true 45° even with the
                            # wall lean, so it prints face down without overhang
BP_T = 2.0                  # back plate thickness
BP_CLR = 0.2                # back plate to wall clearance (per side)

# Knob + encoder (EC11 dimensions from KiCad RotaryEncoder_Alps_EC11E-Switch_Vertical_H20mm)
KNOB_C = (27.5, 35.0)       # knob / shaft centre on the face
KNOB_D = 34.0               # height comes from the shaft: KNOB_H further down
KNOB_GAP = 1.0              # air gap between knob skirt and face (push travel is ~0.5)
KNOB_TOP_T = 6.0            # knob top above the shaft-tip clearance: a thick top makes the knob tall
KNOB_WALL = 2.5             # hollow knob: skirt wall (the grip flutes cut 0.6 into it)
KNOB_HUB_D = 12.0           # hub round the shaft, hanging from the top, tied to the skirt by ribs
KNOB_RIBS, KNOB_RIB_T = 4, 1.2
KNOB_FLUTES, KNOB_FLUTE_D = 48, 1.0   # fine vertical grip ribs round the skirt
KNOB_CHAMFER = 1.6          # 45° chamfer round the top edge (prints on the bed)
KNOB_MARK = (11.0, 135.0, 2.2, 0.6)    # dot marker: at r, angle, Ø, depth (fill it with accent paint)
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
HW040_HDR_X = 10.25         # header pin row (measured on the module: 0.1 nearer the shaft than drawn)
HW040_BACK_KEEPOUT = (-9.0, 9.0, -4.0, 6.0)  # shaft-relative: EC11 legs + 3 pull-up resistors underneath
HW040_BACK_PARTS = 2.0      # how far those (and the header joints) stick out of its back
# The HW-040 hangs behind the printed PCB from the EC11 itself, mounted like a panel encoder: its
# body sits flat against the printed PCB's back, the threaded bushing goes through a round hole, and
# the EC11's own M7 nut (with its washer) on the front clamps it. A knob press goes into the printed
# PCB and on into the face posts; the back plate does not touch the encoder. Deep in the wedge, the
# long shaft ends low and the knob stays short.
HW040_FRONT = -19.9         # board front: the EC11 body (7.7 tall) flat against the printed PCB's back
EC11_NUT = (9.0, 2.0, 0.5)  # M7 nut: across flats, thickness; washer thickness (NOMINAL, check yours)
# Its five header pins stand straight up (straightened, or a straight header) into holes in the
# printed PCB; the wire is wound round them in the gap between the two boards.
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
DISP_HDR_V = 0.1                  # header row centre past the board centre, towards the far (USB) edge (measured)
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
BTN_CAP_PROUD = 6.6         # cap top above the face (the top plate is TOP_T thick)
BTN_CAP_TOP = 11.0          # cap top width: the sides taper in like a keycap
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
BTN_GPIO = ["IO3", "IO4", "IO5"]  # left, middle, right (pin map, see README Wiring)

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

# ESP32-C3 SuperMini: it stands on the front of the printed PCB on its own soldered header strips,
# components towards the face, and its long pins go through to the wiring on the back
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
# Pin rows, from the USB end, seen from the component side with USB-C at the top: the power row
# on the right, the IO row on the left (board photo; the GPIO numbers are printed on the underside).
# The board faces the face here, so seen from the face too: power row at +x.
SM_COL_A = ["5V", "GND", "3V3", "IO4", "IO3", "IO2", "IO1", "IO0"]    # power row, at x = SM_X + row/2
SM_COL_B = ["IO5", "IO6", "IO7", "IO8", "IO9", "IO10", "IO20", "IO21"]  # IO row, at x = SM_X - row/2
# Soldered pin headers: the black plastic strip stands on the carrier front, the long pins go
# through it (measured).
SM_HDR = (2.54, 8 * 2.54, 2.5, 6.0)  # strip width, length, plastic height, long pin past the plastic
SM_BTN_FROM_USB = 9.5       # NOMINAL BOOT / RST distance from the USB end
SM_BTN_DX = 4.5             # NOMINAL offset either side of the board centre line
SM_BTN = (3.0, 4.0, 1.5)
C_TAB_CLR = 0.4             # carrier tab under the SuperMini header ends, to the far wall
C_TAB_PAST = 1.5            # tab edges past the outer pin rows

# Wiring, no solder: 0.3 mm enamelled copper wire laid in channels on the printed PCB's back. Each
# run is one piece of wire from a pin to a pin (wire_layout). At a header pin the scraped wire end
# is wound tight round the pin (its square corners bite into the copper), in open air just behind
# the printed PCB, the same way as at the HW-040: every header pin stands well past the back (the
# display on a long-pin header), so the back stays flat at the pin rows. At a switch the leg is folded flat over the wire end into a pocket and pressed. The
# encoder pins are wrapped in the gap between the HW-040 and the printed PCB. The enamel insulates,
# so runs may cross: there the lower channel dips deeper.
WIRE_D = 0.3                # wire diameter (stand-in)
WIRE_CH = (0.8, 0.7)        # channel width, depth: room for two wires side by side where runs share it
WIRE_BEND_R = 1.5           # channel centre-line radius at every bend (a sharp corner kinks the wire)
WIRE_CROSS = (4.0, 1.3)     # where two runs cross, the lower channel dips: length, depth
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
# RST: a needle hole through the visor and the face over the SuperMini's RST button, with a guide
# tube down to just above the button
RST_HOLE_D = 1.0
RST_GUIDE = (3.0, 1.4)      # guide tube OD, bore

# the receptacle's face-side edge meets the leaning far wall furthest in, so it sets the board end
_usb_hi = C_FRONT + SM_HDR[2] + SM[2] + SM_USB[2]
SM_USB_END = (H - USB_WALL / math.cos(math.radians(WALL_TAPER))  # y of the USB end of the board
              - _usb_hi * math.tan(math.radians(WALL_TAPER)) - USB_FRONT_CLR - SM_USB_OVERHANG)

# Back plate
COLUMN_D = 8.0              # columns clamping the carrier against the face posts (screws inside)
HOOKS = [(-1, 18.0), (-1, 52.0), (+1, 18.0), (+1, 52.0)]  # (side, y centre)
HOOK_W, HOOK_T, HOOK_L = 8.0, 1.4, 10.0
HOOK_BARB = 1.0             # barb with 45° ramps both ways: snaps in, pulls out (a detent)
HOOK_GAP = 1.2              # arm to inner wall
RIB_DEPTH, RIB_H = 1.0, 1.5  # catch rib on the inside of the wall: no windows, nothing visible

# Floating plinth under the shell (part of the back plate, printed dark)
PLINTH_H = 3.0
# Printed TPU rubber dots under the plinth, one in each corner, centred on the screw there (it
# hides the screw) and inside the plinth's rounded corner; a plug presses into the screw
# counterbore, so the plinth needs no recess and still prints flat.
DOT_D, DOT_T = 6.0, 1.5
DOT_PEG_D, DOT_PEG_L = SCREW_HEAD_D - 0.1, 1.5  # plug: TPU in the counterbore

# Top plate: the dished top of the keycap, a separate part laid on the face inside the shoulder.
# It prints flat side down, dish up, so the dish needs no support. A round well round the knob
# lets the knob skirt reach down to the face; a graphite bezel sits in a cut-out round the screen.
TOP_T = 2.6                 # plate thickness at its edge
TOP_RIM = 1.0               # face left showing between the shoulder and the plate
TOP_DISH = 1.2              # spherical dish depth at the centre (0: flat)
TOP_EDGE = 0.8              # chamfer round the plate's top edge
TOP_KNOB_WELL = 1.2         # gap round the knob skirt in the well
BEZEL = (38.0, 34.0, 5.0)   # screen bezel insert: width, height, corner radius (round the window)
BEZEL_CLR = 0.1             # bezel in its cut-out, per side
BEZEL_DROP = 0.4            # bezel top below the lowest point of the dish
UNDER_CLR = 0.3             # face openings behind the visor are this much bigger per side

FONT = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
TEXT_D = 0.4
TEXT_STROKE = 0.15          # stroke round every letter, so small raised letters still print
LABEL_PAD, LABEL_R = 1.0, 0.8  # label box: recessed round the letters, which stay standing

DOC_NAME = "KivoriCaseV3"
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
disp_hdr_y = dpcb_c[1] + DISP_HDR_V
disp_pins = {n: (disp_hdr_x, disp_hdr_y + (3 - i) * 2.54) for i, n in enumerate(DISP_PINS)}
disp_holes = [(dpcb_c[0] + su * (DISP_PCB[0] / 2 - DISP_HOLE_INSET), dpcb_c[1] + sv * (DISP_PCB[1] / 2 - DISP_HOLE_INSET))
              for su in (-1, 1) for sv in (-1, 1)]

# HW-040: module frame axes match the face frame (header end towards +x, holes towards the near edge)
hw_c = (KNOB_C[0] - HW040_SHAFT[0], KNOB_C[1] - HW040_SHAFT[1])
hw_holes = [(hw_c[0] + hx, hw_c[1] + hy) for (hx, hy) in HW040_HOLES]
hw_hdr = (hw_c[0] + HW040_HDR_X, hw_c[1] + HW040_SHAFT[1])  # header row centre
hw_back = HW040_FRONT - HW040[2]           # HW-040 board back
enc_x = hw_hdr[0] + HW040_PIN_BEND         # where its pins come up through the printed PCB
enc_pins = {n: (enc_x, hw_hdr[1] + (2 - i) * 2.54) for i, n in enumerate(["CLK", "DT", "SW", "+", "GND"])}

sm_c = (SM_X, SM_USB_END - SM[1] / 2)
# on its header strips on the carrier front, components up: column A on the left
sm_lo = C_FRONT + SM_HDR[2]
sm_hi = sm_lo + SM[2]
usb_lo, usb_hi = sm_hi, sm_hi + SM_USB[2]
xa_, xb_ = SM_X + SM_ROW / 2, SM_X - SM_ROW / 2   # power row, IO row
sm_pins = {}
for i, n in enumerate(SM_COL_A):
    sm_pins[n] = (xa_, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
for i, n in enumerate(SM_COL_B):
    sm_pins[n] = (xb_, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
sm_rows = [xa_, xb_]
sm_x_pw, sm_x_io = xa_, xb_
sm_hdr_y = SM_USB_END - SM_PIN_MARGIN - 3.5 * 2.54
_bx = SM_BTN_DX                             # RST, BOOT (NOMINAL; the board mirrors when it faces up)
sm_btns = [(SM_X + _bx, SM_USB_END - SM_BTN_FROM_USB), (SM_X - _bx, SM_USB_END - SM_BTN_FROM_USB)]
usb_z = (usb_lo + usb_hi) / 2


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
    rx, ry = sm_btns[0]  # needle guide down to just above the RST button
    z_btn = sm_hi + SM_BTN[2]
    adds.append(cyl(RST_GUIDE[0], rx, ry, z_btn + 0.6, -FACE_T + 0.01).cut(
        cyl(RST_GUIDE[1], rx, ry, z_btn, -FACE_T - 0.5)))
    shell = shell.fuse(adds)

    cuts = [cyl(ENC_HOLE_D, KNOB_C[0], KNOB_C[1], -FACE_T - 1, 1)]
    if KNOB_HALO:
        cuts.append(cyl(2 * KNOB_HALO[1], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2], 1).cut(
            cyl(2 * KNOB_HALO[0], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2] - 1, 2)))
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
    cuts += usb_port_cuts()
    cuts.append(cyl(RST_HOLE_D, rx, ry, -FACE_T - 1, 1))  # RST needle hole
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


def top_outline():
    """The top plate's outline: the flat face inside the shoulder, less the rim."""
    m = FACE_BEVEL + TOP_RIM
    return W - 2 * m, H - 2 * m, R_CORNER - m


def top_surface_z(x, y):
    """Height of the dished top surface above the face."""
    if TOP_DISH <= 0:
        return TOP_T
    w, h, _ = top_outline()
    half = math.hypot(w / 2, h / 2)
    R = (half ** 2 + TOP_DISH ** 2) / (2 * TOP_DISH)
    d2 = (x - W / 2) ** 2 + (y - H / 2) ** 2
    return TOP_T - (TOP_DISH - (R - math.sqrt(max(0.0, R * R - d2))))


def bezel_box(grow=0.0):
    w, h, r = BEZEL
    return w + 2 * grow, h + 2 * grow, r + grow


def make_top():
    """The dished keycap top: knob well, bezel cut-out, three key openings and the RST needle hole.
    Prints flat side down, dish up, so nothing may stick out of its underside: it has no pegs. The
    key caps, already standing in the face openings, centre it through its own key openings; thin
    double-sided tape holds it down."""
    w, h, r = top_outline()
    t = rr_prism(w, h, r, W / 2, H / 2, 0, TOP_T)
    try:
        top = [e for e in t.Edges if abs(e.BoundBox.ZMin - TOP_T) < 1e-6 and abs(e.BoundBox.ZMax - TOP_T) < 1e-6]
        t = t.makeChamfer(TOP_EDGE, top)
    except Exception as exc:
        print("top edge chamfer skipped:", exc)
    if TOP_DISH > 0:
        half = math.hypot(w / 2, h / 2)
        R = (half ** 2 + TOP_DISH ** 2) / (2 * TOP_DISH)
        t = t.cut(Part.makeSphere(R, V(W / 2, H / 2, TOP_T - TOP_DISH + R)))
    bw, bh, br = bezel_box(BEZEL_CLR)
    cuts = [cyl(KNOB_D + 2 * TOP_KNOB_WELL, KNOB_C[0], KNOB_C[1], -1, TOP_T + 1),
            rr_prism(bw, bh, br, DISP_C[0], DISP_C[1], -1, TOP_T + 1)]
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN, BTN_OPEN, 1.5, x, y, -1, TOP_T + 1, a))
    cuts.append(cyl(RST_HOLE_D, sm_btns[0][0], sm_btns[0][1], -1, TOP_T + 1))
    t = t.cut(cuts)
    return t.removeSplitter()


def bezel_top_z():
    w, h, _ = BEZEL
    zs = [top_surface_z(DISP_C[0] + sx * w / 2, DISP_C[1] + sy * h / 2) for sx in (-1, 1) for sy in (-1, 1)]
    return min(zs + [top_surface_z(*DISP_C)]) - BEZEL_DROP


def make_bezel():
    """Graphite frame round the screen, set into the top plate's cut-out on the face, its window
    bevelled down to the glass."""
    w, h, r = BEZEL
    zt = bezel_top_z()
    b = rr_prism(w, h, r, DISP_C[0], DISP_C[1], 0, zt)
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN
    c = min(DISP_WIN_CHAMFER + 0.4, zt - 0.2)
    cuts = [rr_prism(win, win, 1.0, DISP_C[0], DISP_C[1], -1, zt + 1),
            place(Part.makeLoft([rr_wire(win + 2 * c, win + 2 * c, 1.0 + c, zt + 0.01),
                                 rr_wire(win, win, 1.0, zt - c)], True), DISP_C[0], DISP_C[1])]
    return b.cut(cuts).removeSplitter()


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
    plate = plate.fuse([a.common(above_back(0)) for a in adds])

    # floating plinth, built in the desk frame (back plane = z 0): the plate's own outline on the back
    # plane, extruded down and chamfered 45° all round, so its slope meets the plate edge everywhere
    # and the plate prints desk side down with no flat overhang round the plinth (the steep keycap
    # walls make the plate's outline a trapezoid, wider at the far edge)
    env = tapered(W - 2 * WALL - 2 * BP_CLR, H - 2 * WALL - 2 * BP_CLR, R_CORNER - WALL - BP_CLR, 0, -80)
    env_d = env.copy()  # face frame -> desk frame
    env_d.translate(V(0, 0, T_NEAR))
    env_d.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))
    outline = max(env_d.slice(V(0, 0, 1), 0.0), key=lambda w_: Part.Face(w_).Area)
    plinth = Part.Face(outline).extrude(V(0, 0, -PLINTH_H))
    try:
        plinth = plinth.makeChamfer(PLINTH_H - 0.01, [e for e in plinth.Edges
                                                     if abs(e.BoundBox.ZMax + PLINTH_H) < 1e-6])
    except Exception as exc:
        print("plinth chamfer skipped:", exc)
    zb = -PLINTH_H
    under = []  # desk-side details, mirrored so they read correctly with the case turned over
    # the wordmark is engraved into the bed side: a recessed box would leave a bridge over it
    for t in (text_solid("KIVORI", W / 2, H_DESK / 2, 5.0, zb - 0.1, TEXT_D + 0.1, mirror=True),):
        if t:
            under.append(t)
    plinth = plinth.cut(under)
    plinth = desk_to_face(plinth)
    plate = plate.fuse(plinth)

    cuts = []
    for (x, y) in POSTS:  # screw through each column: clearance hole and a counterbore from below
        cuts.append(cyl(SCREW_CLR, x, y, -60, C_BACK + 1))
        cuts.append(cyl(SCREW_HEAD_D, x, y, -60, SCREW_SEAT_Z))
        cuts.append(cyl(SPIGOT_HOLE_D, x, y, C_BACK - SPIGOT_INTO_COL - SPIGOT_SOCKET_CLR, C_BACK + 1))
    return plate.cut(cuts).removeSplitter()


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
    pins = list(disp_pins.values()) + list(sm_pins.values()) + list(enc_pins.values())
    cuts += [cyl(HOLE_D, x, y, z0, z1) for (x, y) in pins]
    cuts += [cyl(SW_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(3) for k in range(4)]
    return cuts


def wire_layout():
    """Wire runs as [(net, points)]: each run is one piece of wire from a pin to a pin, laid in a
    channel through the points (bends rounded to WIRE_BEND_R). A run may start where another ends
    (two wraps on one pin) and share its channel. Check with check_wires() after any change.

    Pin map for this board (README, Wiring). Seen from the face the SuperMini's power row (5V, GND,
    3V3, IO4..IO0) is on the right, next to the display; its IO row (IO5..IO10, IO20, IO21) on the
    left, next to the knob.
    - Display: IO4..IO0 leave the power row side by side and step down onto SCL, SDA, RES, DC, BLK
      in order; VCC and GND come in from the right.
    - Encoder: five lanes between the rows, onto its pins from the east: CLK, DT, SW from the IO row
      (IO7, IO6, IO5), 3V3 and GND from the power row (3V3 crosses GND once there).
    - B1, B2, B3 (IO9, IO8, IO10) go west round the outside of the knob and along the bottom edge,
      B2 outermost, B3 innermost; B1 and B2 rise into their switches between the legs, B3 runs on
      to the right switch, crossing them.
    - GND: SuperMini to the encoder; on from the encoder to the left switch; switch to switch (each
      switch joins its lower pin pair inside); right switch up the right edge to the display.
    - 3V3: SuperMini to the encoder; on from the encoder to the display (crossing GND once).
    Every run ends at a switch leg on a straight stretch at least LEG_POCKET long (the leg folds
    back along it). Where two runs cross, the later one in the list dips under."""
    x_io, x_pw = sm_x_io, sm_x_pw                     # IO row (left), power row (right)
    ys = {n: p[1] for n, p in sm_pins.items()}
    yd = {n: p[1] for n, p in disp_pins.items()}
    ye = {n: p[1] for n, p in enc_pins.items()}
    sm, dp, ep, leg = (lambda n: sm_pins[n]), (lambda n: disp_pins[n]), (lambda n: enc_pins[n]), btn_pin
    # five lanes between the rows, evenly spread between the two pin rows: CLK DT SW 3V3 GND
    s = (x_pw - x_io - PIN_ROW[0]) / 5
    lane = [x_io + PIN_ROW[0] / 2 + (k + 0.5) * s for k in range(5)]

    def down(n, k, e):  # from a SuperMini pin into lane k, down, and west onto encoder pin e
        return [sm(n), (lane[k], ys[n]), (lane[k], ye[e]), ep(e)]

    P = 2.6                                           # pitch of side-by-side channels
    # VCC and GND up the right edge, between the display pin row and the snap-hook cut-out
    x_l, x_r = disp_hdr_x + PIN_ROW[0] / 2, W - WALL - 2.8
    xv = x_l + (x_r - x_l) / 6
    xg = (xv + leg(2, 3)[0]) / 2
    x_g2 = 49.9                                       # GND down to the left switch
    yb = [leg(0, 0)[1] - 2.5 - k * P for k in range(3)]  # along the bottom: B3, B1, B2 (outermost)
    xw = [16.2 - k * P for k in range(3)]             # down the left side: B3, B1, B2 (outermost)
    b1x = (leg(0, 0)[0] + leg(0, 1)[0]) / 2           # rise between the legs of switch 1 ...
    b2x = (leg(1, 0)[0] + leg(1, 1)[0]) / 2           # ... switch 2 ...
    b3x = (leg(2, 0)[0] + leg(2, 1)[0]) / 2           # ... and switch 3
    xgr = leg(2, 1)[0] - LEG_POCKET[1] - 0.6          # GND leaves the right switch west first
    ygr = leg(2, 3)[1] + 4.6                          # ... and passes north of its signal leg
    xf = 64.0                                         # display fan: power-row pins east, down at these x
    fan = {"BLK": ("IO0", xf), "DC": ("IO1", xf + P), "RES": ("IO2", xf + 2 * P),
           "SDA": ("IO3", xf + 3 * P), "SCL": ("IO4", xf + 4 * P)}
    runs = [
        ("GND", down("GND", 4, "GND")),
        ("GND", [ep("GND"), (x_g2, ye["GND"]), (x_g2, leg(0, 0)[1]), leg(0, 0)]),
        ("GND", [leg(0, 1), leg(1, 0)]),
        ("GND", [leg(1, 1), leg(2, 0)]),
        ("GND", [leg(2, 1), (xgr, leg(2, 1)[1]), (xgr, ygr), (xg, ygr), (xg, yd["GND"]), dp("GND")]),
        ("3V3", down("3V3", 3, "+")),
        ("3V3", [ep("+"), (xv, ye["+"]), (xv, yd["VCC"]), dp("VCC")]),
        ("CLK", down("IO7", 0, "CLK")),
        ("DT", down("IO6", 1, "DT")),
        ("SW", down("IO5", 2, "SW")),
        ("B1", [sm("IO9"), (xw[1], ys["IO9"]), (xw[1], yb[1]), (b1x, yb[1]), (b1x, leg(0, 3)[1]), leg(0, 3)]),
        ("B2", [sm("IO8"), (xw[2], ys["IO8"]), (xw[2], yb[2]), (b2x, yb[2]), (b2x, leg(1, 3)[1]), leg(1, 3)]),
        ("B3", [sm("IO10"), (xw[0], ys["IO10"]), (xw[0], yb[0]), (b3x, yb[0]), (b3x, leg(2, 3)[1]), leg(2, 3)]),
    ]
    for name, (p, x) in fan.items():
        runs.append((name, [sm(p), (x, ys[p]), (x, yd[name]), dp(name)]))
    return runs


def header_rows():
    """The wrapped header rows: (x, y centre, pins) of the two SuperMini rows and the display."""
    return [(x, sm_hdr_y, 8) for x in sm_rows] + [(disp_hdr_x, disp_hdr_y, 7)]


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
    P = [V(x, y, z) for (x, y) in pts]
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
        n = V(-u.y, u.x, 0) if (u.x * v.y - u.y * v.x) > 0 else V(u.y, -u.x, 0)
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


def wire_crossings():
    """Where two runs of different nets cross: (x, y, run on top, run that dips, its direction)."""
    runs = wire_layout()
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


def wire_cuts():
    """Everything cut into the printed PCB's back for the wiring: the channels, the dips under the
    crossings and a pocket at each switch leg the wire ends on."""
    z0, w, dep = C_BACK - 1, WIRE_CH[0], WIRE_CH[1]
    cuts = [band(pts, w, z0, C_BACK + dep) for _, pts in wire_layout()]
    for (x, y, _, _, (dx, dy)) in wire_crossings():
        L = WIRE_CROSS[0]
        cuts.append(place(box(-L / 2, -w / 2, z0, L / 2, w / 2, C_BACK + WIRE_CROSS[1]), x, y,
                          math.degrees(math.atan2(dy, dx))))
    cuts += kapton_squares(z0, C_BACK + dep + KAPTON[1])  # a flat seat for the Kapton square
    for ((x, y), (dx, dy)) in leg_ends():  # the leg folds back along the arriving wire
        a = math.degrees(math.atan2(dy, dx))
        cuts.append(place(box(0, -LEG_POCKET[0] / 2, z0, LEG_POCKET[1], LEG_POCKET[0] / 2, C_BACK + dep), x, y, a))
    return cuts


def check_wires():
    """Narrowest plastic left between a channel and anything that is not its own: another net's
    channel (where two runs cross, that pair is left out), a hole, pin row or leg pocket it does not
    end in, the board edge. Returns [(mm, net, what)], smallest first."""
    runs = wire_layout()
    zt = C_BACK
    chans = [band_face(pts, WIRE_CH[0], zt) for _, pts in runs]
    crossed = {(min(a, b), max(a, b)) for (_, _, a, b, _) in wire_crossings()}
    holes = [((x, y), HOLE_D) for d in (sm_pins, disp_pins, enc_pins) for (x, y) in d.values()]
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
        own = lambda p: any(math.hypot(p[0] - e[0], p[1] - e[1]) < 1e-6 for e in ends)
        for j in range(k + 1, len(runs)):
            if runs[j][0] != net and (k, j) not in crossed:
                out.append((ch.distToShape(chans[j])[0], net, "channel " + runs[j][0]))
        for (p, dia) in holes:
            if not own(p):
                out.append((ch.distToShape(Part.Vertex(V(p[0], p[1], zt)))[0] - dia / 2, net, "hole (%.1f, %.1f)" % p))
        mine = []
        for (x, y, n) in header_rows():
            tr = Part.Face(Part.makePolygon([V(x + sx * PIN_ROW[0] / 2, y + sy * row_len(n) / 2, zt)
                                             for (sx, sy) in ((-1, -1), (1, -1), (1, 1), (-1, 1), (-1, -1))]))
            if any(abs(e[0] - x) < 1e-6 and abs(e[1] - y) <= n * 1.27 for e in ends):
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
        flutes.append(cyl(KNOB_FLUTE_D, cx + r * math.cos(a), cy + r * math.sin(a), z0 - 1, z1 - KNOB_CHAMFER - 1.2))  # out the skirt edge
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
    mr, mang, md, mdep = KNOB_MARK
    a = math.radians(mang)
    cuts.append(cyl(md, cx + mr * math.cos(a), cy + mr * math.sin(a), z1 - mdep, z1 + 1))
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
    # straight up through the face and the top plate, then tapering in like a keycap
    z_taper = TOP_T + 0.5
    body = rr_prism(BTN_CAP, BTN_CAP, 1.2, x, y, z_flange, z_taper + 0.01, a).fuse(place(Part.makeLoft(
        [rr_wire(BTN_CAP, BTN_CAP, 1.2, z_taper), rr_wire(BTN_CAP_TOP, BTN_CAP_TOP, 2.0, BTN_CAP_PROUD)], True, True), x, y, a))
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
    inner = BTN_CAP_TOP - 2 * CAP_WALL - 0.6
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
    hdr = cbox(hw_hdr[0], hw_hdr[1], 2.5, 5 * 2.54, f, f + 2.5)
    pins = fuse_all([cbox(x, y, 0.64, 0.64, f, f + HW040_PIN_L) for (x, y) in enc_pins.values()])  # straightened
    # on its back: EC11 legs and resistors, header solder joints
    kx0, kx1, ky0, ky1 = HW040_BACK_KEEPOUT
    legs = box(KNOB_C[0] + kx0, KNOB_C[1] + ky0, hw_back - HW040_BACK_PARTS, KNOB_C[0] + kx1, KNOB_C[1] + ky1, hw_back + 0.01)
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


def dummy_supermini():
    """On its header strips on the carrier front, components up, the long pins through to the back."""
    pcb = cbox(sm_c[0], sm_c[1], SM[0], SM[1], sm_lo, sm_hi)
    usb = cbox(SM_X, SM_USB_END - SM_USB[1] / 2 + SM_USB_OVERHANG, SM_USB[0], SM_USB[1], usb_lo, usb_hi)
    btns = [cbox(x, y, SM_BTN[0], SM_BTN[1], sm_hi, sm_hi + SM_BTN[2]) for (x, y) in sm_btns]
    chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_hi, sm_hi + 0.9)
    hdr = [cbox(xr, sm_hdr_y, SM_HDR[0], SM_HDR[1], C_FRONT, sm_lo) for xr in sm_rows]
    pins = [cbox(x, y, 0.64, 0.64, C_FRONT - SM_HDR[3], sm_hi + 0.5) for (x, y) in sm_pins.values()]
    return fuse_all([pcb, usb, chip] + btns + hdr + pins)


def shared_offsets():
    """Side-by-side offsets for runs of one net that share a channel: {(run, segment): mm}, one
    wire to each side of the channel's centre line."""
    runs = wire_layout()
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
    """The wire: in the bottom of each channel, wound round each header pin it ends on, in open
    air just behind the printed PCB (at the encoder, in the gap to the HW-040). Where two runs
    cross, the lower one dips into the deeper channel under the Kapton; where two runs of one net
    share a channel, they lie side by side."""
    zc = C_BACK + WIRE_CH[1] - WIRE_D / 2
    zd = C_BACK + WIRE_CROSS[1] - WIRE_D / 2       # in the dip under a crossing
    r = WIRE_D / 2
    off = shared_offsets()
    dips = {}
    for (x, y, _, kb, d) in wire_crossings():
        dips.setdefault(kb, []).append((x, y, d))
    parts = []
    for k, (_, pts) in enumerate(wire_layout()):
        P = offset_run(pts, k, off)
        path = Part.Wire(run_edges(P, zc))
        e0 = path.OrderedEdges[0]
        prof = Part.Wire(Part.makeCircle(r, e0.Vertexes[0].Point, e0.tangentAt(e0.FirstParameter)))
        # it leaves the channel at the pin (down onto the turns), not through it
        w = path.makePipeShell([prof], True, True).cut([cyl(1.0, x, y, zc - 1, zc + 1) for (x, y) in (pts[0], pts[-1])])
        for (x, y, (dx, dy)) in dips.get(k, []):
            # the segment's sideways offset, so the dip lines up with the wire
            j = min(range(len(pts) - 1), key=lambda i: Part.LineSegment(V(*pts[i], 0), V(*pts[i + 1], 0)).toShape()
                    .distToShape(Part.Vertex(V(x, y, 0)))[0])
            px, py = x + (P[j][0] - pts[j][0]), y + (P[j][1] - pts[j][1])
            h = WIRE_CROSS[0] / 2
            w = w.cut(place(box(-h, -1, zc - 1, h, 1, zc + 1), px, py, math.degrees(math.atan2(dy, dx))))
            pp = [V(px + dx * s, py + dy * s, z) for (s, z) in ((-h, zc), (-h + 0.8, zd), (h - 0.8, zd), (h, zc))]
            for a, b in zip(pp, pp[1:]):
                w = w.fuse(Part.makeCylinder(r, (b - a).Length, a, b - a))
            w = w.fuse([Part.makeSphere(r, q) for q in pp])
        parts.append(w)
    ring = lambda x, y, z0, z1: cyl(WRAP_OD, x, y, z0, z1).cut(cyl(0.95, x, y, z0 - 1, z1 + 1))
    tall = WRAP_TURNS * WIRE_D
    wrapped = set()
    for _, pts in wire_layout():
        for p in (pts[0], pts[-1]):
            wrapped.add((round(p[0], 4), round(p[1], 4)))
    for (x, y) in list(sm_pins.values()) + list(disp_pins.values()):
        if (round(x, 4), round(y, 4)) in wrapped:
            parts.append(ring(x, y, C_BACK - 0.3 - tall, C_BACK - 0.3))
    for (x, y) in enc_pins.values():
        parts.append(ring(x, y, C_BACK - 0.5 - tall, C_BACK - 0.5))
    return Part.makeCompound(parts)


def kapton_squares(z0, z1):
    """A Kapton square at each crossing, square to the lower run, from z0 to z1."""
    s = KAPTON[0] / 2
    return [place(box(-s, -s, z0, s, s, z1), x, y, math.degrees(math.atan2(dy, dx)))
            for (x, y, _, _, (dx, dy)) in wire_crossings()]


def dummy_kapton():
    """The Kapton squares, on the floor of their pockets, between the lower and the upper wire."""
    z = C_BACK + WIRE_CH[1]
    return Part.makeCompound(kapton_squares(z, z + KAPTON[1]))


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}

COLORS = {  # two-tone: light matte shell; black visor, knob and plinth; warm accent caps
    "Shell": (0.93, 0.91, 0.86), "Top": (0.95, 0.93, 0.89), "Bezel": (0.16, 0.17, 0.19), "BackPlate": (0.17, 0.17, 0.19),
    "Carrier": (0.12, 0.45, 0.25), "Knob": (0.10, 0.10, 0.11), "Cap": (0.95, 0.50, 0.18),
    "Dummy": (0.30, 0.45, 0.85), "RubberDots": (0.20, 0.20, 0.22),
    "Dummy_Wire": (0.85, 0.45, 0.20), "Dummy_Kapton": (0.95, 0.70, 0.15),
}


def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["Top"] = make_top()
    PRINTED["Bezel"] = make_bezel()
    PRINTED["BackPlate"] = make_back_plate()
    PRINTED["RubberDots"] = make_rubber_dots()
    PRINTED["Carrier"] = make_carrier()
    PRINTED["Knob"] = make_knob()
    for i, n in enumerate(["CapLeft", "CapMiddle", "CapRight"]):
        PRINTED[n] = make_cap(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_EncoderNut"] = dummy_encoder_nut()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_SuperMini"] = dummy_supermini()
    DUMMIES["Dummy_Wire"] = dummy_wire()
    if wire_crossings():
        DUMMIES["Dummy_Kapton"] = dummy_kapton()
    for i, n in enumerate(["Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"]):
        DUMMIES[n] = dummy_switch(i)


PRESS_FITS = {("BackPlate", "RubberDots")}  # TPU plugs (the screw counterbores lean 10°)


def check_interference():
    """Clashes between parts; the intended press fits are reported apart."""
    items = list(PRINTED.items()) + list(DUMMIES.items())
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
        t = n if n in ("GND", "3V3") else "%s: %s" % (n, net(p))
        out.append((t, p[0] + side * gap, p[1], side))
    for n, p in disp_pins.items():
        out.append(("Disp " + n, p[0] + gap, p[1], 1))
    for n, p in enc_pins.items():                     # the wires come in from the east: text west
        out.append(("Enc " + n, p[0] - 1.2, p[1], -1))
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
    if name in ("Shell", "Knob") or name.startswith("Cap"):
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
    doc.saveAs(os.path.join(here, "kivori-case-v3.FCStd"))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    tight = [c for c in check_wires() if c[0] < WIRE_RIDGE_MIN - 1e-6]
    print("wire channels: %d runs, %d crossing(s), plastic under %.1f mm: %s" % (
        len(wire_layout()), len(wire_crossings()), WIRE_RIDGE_MIN,
        "none" if not tight else ", ".join("%.2f %s / %s" % c for c in tight)))
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
