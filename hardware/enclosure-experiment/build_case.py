"""Kivori experimental enclosure: square body, a turning ring round a static screen, 4 buttons.

FreeCAD 1.1, Part workbench, no GUI needed. Run it inside FreeCAD (GUI Python console, the FreeCAD
MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-experiment/build_case.py"
    exec(open(p).read(), {"__file__": p, "__name__": "__main__"})

How it works with the standard parts (ST7789 1.3" square LCD, HW-040 / EC11 encoder):
  * The screen sits still in the middle, under a square window in a round black bezel.
  * The ring turns round it. Teeth on the inside of the ring drive a small gear on the encoder
    shaft, which sits off-centre in front of the screen (an internal gear pair: both turn the same way).
  * The bezel is hinged on the left. Pressing it tips it onto the top of the encoder shaft, so the
    HW-040's own push switch is the click.
  * The whole top is four quadrant tiles round the ring: those are the buttons.
  * Everything is wired by copper tape on one printed PCB (same pin map as v1, issue #67).
  * Like v1 the body is a wedge: thin at the front, thick at the back.

Frames. Every part is designed in the deck frame: the deck (tiles) is horizontal, the ring axis is
the z axis, +x is right and +y is away from the user. The assembly is then tipped forward by
WEDGE_DEG onto the flat floor (the desk is z = 0), so the front sits low and the back high.

Values marked NOMINAL come from datasheets or seller listings; "measured" from the real modules.
"""

import math
import os
import sys

import FreeCAD as App
import Part

sys.path.append(os.path.join(App.getResourceDir(), "Mod", "PartDesign"))
from fcgear import fcgear, involute  # noqa: E402  (FreeCAD's own involute gear generator)

V = App.Vector

# ---------------------------------------------------------------------------------------------
# Parameters
# ---------------------------------------------------------------------------------------------

# Square body. The whole top is four button tiles; under them a fixed sub-deck carries the ring
# and guides the tiles. Walls and floor sit WALL_INSET inside the tile outline, so the top
# seems to float over a shadow line. The heights all follow from the encoder (below).
S = 100.0                   # outer size (tile outline)
S_R = 10.0                  # corner radius
WALL_INSET = 1.5
WALL = 2.0
FLOOR_T = 2.0
WEDGE_DEG = math.degrees(math.atan(12.0 / 70.0))  # v1's wedge (9.7°): the deck tips towards you
FLOOR_CLR = 0.3             # lowest part (the parts on the HW-040's back) above the floor

# HW-040 + EC11 (measured on the real module). It sits on two floor pillars whose pegs go through
# its two holes (as v1), its bushing through a snug round hole in the printed PCB and its five
# header pins straight up (no bend) into pads on the copper. Its 29.2 mm shaft is used whole.
EC11_BODY = (12.0, 11.6)    # NOMINAL body footprint
EC11_BODY_H = 7.7           # body top (bushing base) above the board front (measured)
EC11_BUSH_D, EC11_BUSH_L = 7.0, 8.5   # M7 bushing: thread ends 16.2 above the board (measured)
EC11_SHAFT_D, EC11_SHAFT_H = 6.0, 29.2  # D shaft, tip 29.2 above the board (measured)
EC11_FLAT = 1.5             # D-flat depth (measured)
EC11_FLAT_L = 7.1           # D-flat length from the tip (measured)
EC11_PUSH = 0.5             # push-switch travel, NOMINAL
EC11_TOP_GAP = 0.3          # body top under the printed PCB (the pillars carry it, not the nut)
EC11_HOLE_CLR = 0.1         # round hole round the bushing, per side: it centres the encoder
HW040 = (26.0, 18.5, 1.3)
HW040_SHAFT = (-2.3, 1.8)   # measured: shaft from board centre (+x header end, -y holes)
HW040_HOLES = [(-8.55, -6.9), (8.55, -6.9)]  # two M3 holes, from the board centre (measured)
HW040_HOLE_D = 3.2
HW040_HDR_X = 10.35         # header pin row from the board centre (measured)
HW040_BACK_KEEPOUT = (-9.0, 9.0, -4.0, 6.0)  # shaft-relative: EC11 legs + 3 pull-up resistors underneath
HW040_BACK_PARTS = 2.0      # how far those (and the header joints) stick out of its back
HW040_ROT = -90.0           # board turned so the header points to the front
HW040_PINS = ["CLK", "DT", "SW", "+", "GND"]  # module frame: +y to -y along the header
HW040_COL_D = 5.0           # floor pillars under its two holes ...
HW040_PEG_CLR = 0.2         # ... pegs 2.8 mm in its 3.2 mm holes
ENC_BORDER = (0.3, 1.2, 1.0, 1.2)  # corner borders round its board: clearance, wall, wall height, ledge
ENC_CORNER = 5.0            # each runs this far along both board edges
ENC_BOARD_FRONT = 5.6       # deck frame: board front (the whole stack is placed from here)

# Gear pair, module 1: internal teeth on the ring, pinion on the encoder shaft. The pinion's teeth
# sit low on the round part of the shaft, just above the bushing, and its tall hub grips the
# D-flat at the top: so the deck can stay low and the ring stands tall above it.
GEAR_M = 1.0
RING_TEETH, PINION_TEETH = 56, 20   # 2.8 encoder turns per ring turn
GEAR_BAND_R = 30.4          # outer radius of the toothed band
GEAR_H = 5.6                # ring teeth band height
PINION_GAP = 0.7            # pinion above the bushing (the shaft moves down 0.5 when pressed)
PINION_TEETH_H = 4.6
PINION_HUB_D = 10.0         # hub up the shaft, D-shaped bore over the D-flat
PINION_TIP_GAP = 0.4        # hub top under the shaft tip (the bezel nub rests on the tip)
BACKLASH_CD = 0.35          # pinion moved this much towards the axis: tooth play for printed gears
                            # (an internal pair gets play from a SMALLER centre distance)
PINION_PHASE = 0.5          # pinion turned by this many tooth pitches so it meshes
SHAFT_C = (0.0, -((RING_TEETH - PINION_TEETH) * GEAR_M / 2 - BACKLASH_CD))  # in front of the screen

# Ring (turns): rests on the sub-deck, a screwed-on retainer under it keeps it in
RING_R = 34.0               # outer radius above the sub-deck
RING_HOLE = 33.0            # sub-deck hole radius
RING_NECK_R = 32.8          # ring radius through the sub-deck hole
RING_BORE = 28.3            # top lip round the bezel
RING_LIP_T = 2.0
RING_BAND_R = 29.0          # inner radius between the lip and the gear teeth
RING_CHAMFER = 2.0
RING_ABOVE_CAP = 1.0        # ring top above the bezel top
FLUTES, FLUTE_D = 72, 1.4
RET_GAP = 0.2               # retainer top below the sub-deck underside
RET_T = 1.5
RET_R = (30.6, 34.6)
RET_SCREWS = [90.0, 210.0, 330.0]  # M2 x 6 into the ring, at radius 31.7
RET_SCREW_R = 31.7
SUBDECK_T = 2.0
SUBDECK_DROP = 2.8          # sub-deck top under the tile tops

# Bezel (hinged on the left) with the square window over the fixed screen
CAP_R = 28.0
CAP_FACE_T = 1.6
CAP_SKIRT_T = 1.2
CAP_SKIRT_H = 3.0
CAP_FLANGE_R = 28.7         # under the ring lip: keeps the bezel in
WINDOW = 24.0               # square window over the 23.4 mm active area
WINDOW_R = 1.5
WINDOW_CHAMFER = 0.8
NUB_D = 5.0                 # under the bezel: rests on the encoder shaft tip

# Hinge on the left, parallel to y: as far from the encoder shaft as from the bezel centre, so a
# press in the middle needs the same force as the encoder's own push switch. Two posts on the
# printed PCB, a lug with a pin under the bezel that snaps into their C holes.
HINGE_X = -23.4
HINGE_DROP = 2.2            # hinge axis under the bezel underside
HINGE_PIN_D = 2.5
HINGE_POST_X = (-24.4, -22.4)  # posts, beside the display board (its edge is at x -21.46)
HINGE_POST_BASE_X = -20.5   # ... wider below the display board
HINGE_POST_Y = (4.4, 8.4)
HINGE_LUG = (2.4, 3.6)      # lug width along x, half-length along y
HINGE_HOLE_D, HINGE_SLOT = 2.65, 2.0

# Display: ST7789 1.3" 240x240 (GMT130), header on the right (same as v1: firmware rotation stays).
# Glass and visible area measured on the real module (see v1). It stands still on four posts, its
# own short header plugged into two standard 8.5 mm female headers stacked on the printed PCB.
DISP_C = (0.0, 0.0)         # active-area centre = window centre
DISP_PCB = (39.22, 27.78, 1.6)
DISP_GLASS = (28.0, 26.3, 1.4)
DISP_ACTIVE_U = 1.85        # visible-area centre from the board centre, along x (measured)
DISP_GLASS_U = 0.8          # glass centre from the board centre (measured)
DISP_HDR_U = 2.2            # 7-pin header row, from the header end
DISP_HOLE_D, DISP_HOLE_INSET = 2.0, 2.5
DISP_STANDOFF_D = 4.5
DISP_GAP = 1.1              # glass top under the bezel, so the bezel can tip
DISP_PINS = ["GND", "VCC", "SCL", "SDA", "RES", "DC", "BLK"]  # +y to -y
DISP_HDR_H = 2.5            # its header plastic under the board
SOCKET_H, SOCKETS = 8.5, 2  # 2.54 mm female headers, stacked
PEG_CLR, PEG_PROUD = 0.1, 1.0

# Buttons: the whole top is four quadrant tiles, split along the x and y axes, from the ring out
# to the body edge. Like a standard 12x12 switch cap, each tile snaps onto its switch: a square
# socket under the tile middle grips the switch's square tip, two of its four flexing walls hook
# under it. The switch holds the tile down and sets its height. 3 legs sliding in guide bosses
# in the sub-deck only keep the tile from tilting when pressed near a corner.
BTN_D = 44.0                # switch distance from the centre along the diagonals (tile centroid)
TILE_T = 2.0
TILE_GAP = 0.3              # between tiles, and round the ring
TILE_ROUND = 1.0            # rounding of the tile corners in plan
TILE_CHAMFER = 0.5          # top edges
TILE_SKIRT_T = 1.2          # lip over the wall top
TILE_LEGS = [(44.0, 11.5), (11.5, 44.0), (42.0, 42.0)]  # for the +x +y tile (clear of the SuperMini)
LEG_D, LEG_HOLE_D, LEG_BOSS_D = 4.0, 4.3, 7.5
SOCK_CLR = 0.025            # socket = switch tip + this, per side (a light grip)
SOCK_WALL = 0.7             # flexing walls round the socket
SOCK_HOOK = (0.3, 0.6)      # hooks under the tip: depth, height (45° lead-in below, flat on top)
SOCK_SLIT = 0.5             # slits at the socket corners free the walls ...
SOCK_STEM = 0.6             # tile underside above the switch tip
SW_HOLE_D = 9.0             # socket through the sub-deck
# 12x12 switch, 10 mm tall, snap-fit stem (same as v1: Same Sky TS13 drawing)
SW_BODY, SW_BODY_H, SW_TOTAL_H = 12.0, 3.5, 10.0
SW_BOSS_D, SW_BOSS_H = 7.1, 4.3
SW_NECK, SW_HEAD, SW_HEAD_H = 3.1, 3.8, 1.8
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]  # pin 1,1 (front), 2,2 (back)
SW_LEG_HOLE_D = 1.8

# ESP32-C3 SuperMini on its soldered header strips on the printed PCB, components up, in the middle
# of the back, USB-C out of the back wall through a small pill hole (as v1: a hidden pocket thins
# the wall round the receptacle, so a cable's plug reaches fully in)
SM = (18.0, 22.52, 1.0)     # NOMINAL
SM_X = 0.0                  # board centre x (its long axis along y)
SM_ROW = 15.24
SM_PIN_MARGIN = (SM[1] - 7 * 2.54) / 2
SM_HDR = (2.54, 8 * 2.54, 2.5, 6.0)  # strip width, length, plastic height, pin past the plastic
SM_USB = (8.94, 7.35, 3.2)  # NOMINAL
SM_USB_OVERHANG = 0.5
USB_WALL = 0.8
USB_HOLE = (9.6, 4.0)
USB_FRONT_CLR = 0.15
# components up, USB-C away from you: column A (5V ... IO0) on the left, B (IO5 ... IO21) right
SM_COL_A = ["5V", "GND", "3V3", "IO4", "IO3", "IO2", "IO1", "IO0"]
SM_COL_B = ["IO5", "IO6", "IO7", "IO8", "IO9", "IO10", "IO20", "IO21"]

# Printed PCB ("carrier"): copper-tape traces on its underside (the bed side), flush, with a gap
# along both sides of every trace and pad: lay tape flat, cut along the gaps, peel off the rest.
C_T = 2.0
C_INSET = 0.8               # outline inset from the inner walls
HOLE_D = 1.2                # 2.54 mm header holes
TRACE_W = 1.2
DISP_TRACE_W = 1.0          # the display's five signals (they share a narrow strip)
TRACE_GROOVE = (1.0, 0.6)   # gap along both sides: width, depth
TRACE_PAD_D = 3.4           # round pads at the switch legs
SM_PAD_D = 1.6
PIN_PAD_D = 1.6             # display and encoder pins
TRACE_ISLAND_MIN = 12.0     # mm²: smaller islands of plastic between the gaps become gap (FDM)
TRACE_GROOVE_MIN = 0.6      # gaps thinner than this (crumbs) are filled in
TRACE_RIDGE_MIN = 1.0       # plastic left between two gaps: at least this wide, or it becomes gap
PIN_ROW_BAND = 1.5          # half-width of the solid strip along each pin row (one cut per pin)
TAPE_T = 0.06               # copper tape (stand-in only)
GAP_MIN = 0.94              # copper to copper, as v1
BTN_GPIO = ["IO4", "IO3", "IO5", "IO9"]  # back-left, front-left, back-right, front-right: B2 B1 B3 B4

# Assembly: M2 x 12 self-tapping screws from under the floor, up through a floor post and the
# printed PCB into a post hanging from the sub-deck: floor, printed PCB and shell clamped together.
# The posts' spigots locate the printed PCB, so the gear centre distance holds.
SCREWS = [(-42.0, 42.0), (42.0, 42.0), (-42.0, -42.0), (42.0, -42.0)]  # in the corners, on the tiles' corner legs
POST_D = 6.5
SPIGOT_D, SPIGOT_HOLE_D = 3.4, 3.9
SCREW_PILOT, SCREW_CLR, SCREW_HEAD_D, SCREW_L = 1.6, 2.4, 4.4, 12.0
SCREW_IN = 5.8              # pilot depth into the corner post (the tile leg's hole is above it)
FOOT_RING = (10.5, 0.8, 0.4)  # marker rings for 10 mm stick-on feet: diameter, width, depth

FONT = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
TEXT_D = 0.4
DOC_NAME = "KivoriCaseExperiment"
EXPORT = True
COPPER_GROOVES = True       # False skips the (slow) gap cutting for quick layout checks

# ---------------------------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------------------------


def rr_wire(w, h, r, z=0.0):
    hw, hh = w / 2.0, h / 2.0
    r = min(r, hw - 1e-3, hh - 1e-3)

    def arc(cx, cy, a0, a1):
        c = Part.Circle(V(cx, cy, z), V(0, 0, 1), r)
        return Part.ArcOfCircle(c, math.radians(a0), math.radians(a1)).toShape()

    def line(a, b):
        return Part.LineSegment(V(a[0], a[1], z), V(b[0], b[1], z)).toShape()

    return Part.Wire([
        line((-hw + r, -hh), (hw - r, -hh)), arc(hw - r, -hh + r, -90, 0),
        line((hw, -hh + r), (hw, hh - r)), arc(hw - r, hh - r, 0, 90),
        line((hw - r, hh), (-hw + r, hh)), arc(-hw + r, hh - r, 90, 180),
        line((-hw, hh - r), (-hw, -hh + r)), arc(-hw + r, -hh + r, 180, 270),
    ])


def rr_prism(w, h, r, z0, z1, cx=0.0, cy=0.0):
    s = Part.Face(rr_wire(w, h, r, z0)).extrude(V(0, 0, z1 - z0))
    s.translate(V(cx, cy, 0))
    return s


def box(x0, y0, z0, x1, y1, z1):
    return Part.makeBox(abs(x1 - x0), abs(y1 - y0), abs(z1 - z0), V(min(x0, x1), min(y0, y1), min(z0, z1)))


def cbox(cx, cy, w, h, z0, z1):
    return box(cx - w / 2, cy - h / 2, z0, cx + w / 2, cy + h / 2, z1)


def cyl(d, cx, cy, z0, z1):
    return Part.makeCylinder(d / 2.0, z1 - z0, V(cx, cy, z0), V(0, 0, 1))


def ycyl(d, x, z, y0, y1):
    return Part.makeCylinder(d / 2.0, y1 - y0, V(x, y0, z), V(0, 1, 0))


def ring_solid(r0, r1, z0, z1):
    s = cyl(2 * r1, 0, 0, z0, z1)
    return s.cut(cyl(2 * r0, 0, 0, z0 - 1, z1 + 1)) if r0 > 0 else s


def revolve(rz):
    """Solid of revolution about z from a closed (r, z) polygon."""
    pts = [V(r, 0, z) for (r, z) in rz] + [V(rz[0][0], 0, rz[0][1])]
    return Part.Face(Part.makePolygon(pts)).revolve(V(0, 0, 0), V(0, 0, 1), 360)


def polar(r, a):
    return (r * math.cos(math.radians(a)), r * math.sin(math.radians(a)))


def fuse_all(shapes):
    shapes = [s for s in shapes if s is not None]
    if len(shapes) == 1:
        return shapes[0]
    return shapes[0].fuse(shapes[1:]).removeSplitter()


def gear_face(teeth, internal):
    w = fcgear.FCWireBuilder()
    if internal:
        involute.CreateInternalGear(w, GEAR_M, teeth, 20)
    else:
        involute.CreateExternalGear(w, GEAR_M, teeth, 20)
    return Part.Face(Part.Wire([e.toShape() for e in w.wire]))


def text_solid(s, x, y, size, z0, depth, mirror=False):
    if not os.path.exists(FONT):
        return None
    try:
        faces = []
        for ch in Part.makeWireString(s, FONT, size, 0):
            if not ch:
                continue
            wires = [w if isinstance(w, Part.Wire) else Part.Wire(w) for w in ch]
            faces.append(Part.makeFace(wires, "Part::FaceMakerBullseye"))
        comp = Part.makeCompound(faces)
        bb = comp.BoundBox
        sol = comp.extrude(V(0, 0, depth))
        sol.translate(V(x - (bb.XMin + bb.XMax) / 2, y - (bb.YMin + bb.YMax) / 2, z0))
        if mirror:
            sol = sol.mirror(V(x, 0, 0), V(1, 0, 0))
        return sol
    except Exception as exc:
        print("text skipped:", s, exc)
        return None


def teardrop_y(d, x, z, y0, y1):
    """A pin along y whose top is a 45° point (prints without support when the part is flipped)."""
    r = d / 2
    pts = [V(x + r * math.cos(math.radians(a)), y0, z + r * math.sin(math.radians(a))) for a in range(135, 406, 15)]
    pts += [V(x, y0, z + r * math.sqrt(2)), pts[0]]  # the arc round the bottom, then the point on top
    f = Part.Face(Part.makePolygon(pts))
    return f.extrude(V(0, y1 - y0, 0))


# ---------------------------------------------------------------------------------------------
# Derived layout (deck frame)
# ---------------------------------------------------------------------------------------------

BODY, BODY_R = S - 2 * WALL_INSET, S_R - WALL_INSET  # walls and floor
INNER = BODY / 2 - WALL                      # inner wall face

# the stack, from the encoder up
enc_board_front = ENC_BOARD_FRONT
enc_board_back = enc_board_front - HW040[2]
enc_body_top = enc_board_front + EC11_BODY_H
bush_top = enc_body_top + EC11_BUSH_L
enc_shaft_top = enc_board_front + EC11_SHAFT_H
C_BOT = enc_body_top + EC11_TOP_GAP          # printed PCB underside (the copper side)
C_TOP = C_BOT + C_T
GEAR_Z = (bush_top, bush_top + GEAR_H)       # ring teeth band
PINION_Z = (bush_top + PINION_GAP, bush_top + PINION_GAP + PINION_TEETH_H)  # pinion teeth
PINION_TOP = enc_shaft_top - PINION_TIP_GAP
sw_base = C_TOP
SW_PLUNGER_TOP = sw_base + SW_TOTAL_H       # switch tip top = socket ceiling
DECK_TOP = SW_PLUNGER_TOP + SOCK_STEM + TILE_T  # top face of the tiles
tile_under = DECK_TOP - TILE_T
SUBDECK_TOP = DECK_TOP - SUBDECK_DROP
subdeck_under = SUBDECK_TOP - SUBDECK_T
ret_top = subdeck_under - RET_GAP
TILE_SKIRT_Z = DECK_TOP - 3.6               # lip over the wall top: bottom
LEG_BOTTOM = DECK_TOP - 7.0

# display on its sockets, the bezel 1.1 above the glass, the ring just above the bezel
dpcb_c = (DISP_C[0] - DISP_ACTIVE_U, DISP_C[1])
glass_c = (dpcb_c[0] + DISP_GLASS_U, DISP_C[1])
socket_top = C_TOP + SOCKETS * SOCKET_H
dpcb_z0 = socket_top + DISP_HDR_H           # board back: its pins fully in the sockets
dpcb_z1 = dpcb_z0 + DISP_PCB[2]
glass_z0, glass_z1 = dpcb_z1, dpcb_z1 + DISP_GLASS[2]
cap_under = glass_z1 + DISP_GAP
CAP_TOP = cap_under + CAP_FACE_T
CAP_BOTTOM = cap_under - CAP_SKIRT_H
HINGE_Z = cap_under - HINGE_DROP
RING_TOP = CAP_TOP + RING_ABOVE_CAP
FLUTE_Z = (DECK_TOP + 1.0, RING_TOP - 3.0)
disp_hdr_x = dpcb_c[0] + DISP_PCB[0] / 2 - DISP_HDR_U
disp_pins = {n: (disp_hdr_x, DISP_C[1] + (3 - i) * 2.54) for i, n in enumerate(DISP_PINS)}
disp_holes = [(dpcb_c[0] + su * (DISP_PCB[0] / 2 - DISP_HOLE_INSET), dpcb_c[1] + sv * (DISP_PCB[1] / 2 - DISP_HOLE_INSET))
              for su in (-1, 1) for sv in (-1, 1)]

c_btn = BTN_D / math.sqrt(2)
BTNS = [(sx * c_btn, sy * c_btn) for (sx, sy) in ((-1, 1), (1, 1), (-1, -1), (1, -1))]


def btn_pin(i, k):
    return (BTNS[i][0] + SW_PADS[k][0], BTNS[i][1] + SW_PADS[k][1])


SM_USB_END = BODY / 2 - USB_WALL - USB_FRONT_CLR - SM_USB_OVERHANG  # board end, in the back wall pocket
sm_c = (SM_X, SM_USB_END - SM[1] / 2)
sm_lo = C_TOP + SM_HDR[2]                    # board underside, on its header strips
sm_hi = sm_lo + SM[2]
usb_zc = sm_hi + SM_USB[2] / 2
sm_rows = (SM_X - SM_ROW / 2, SM_X + SM_ROW / 2)
sm_pins = {}
for _i, _n in enumerate(SM_COL_A):
    sm_pins[_n] = (sm_rows[0], SM_USB_END - SM_PIN_MARGIN - _i * 2.54)
for _i, _n in enumerate(SM_COL_B):
    sm_pins[_n] = (sm_rows[1], SM_USB_END - SM_PIN_MARGIN - _i * 2.54)


def hw_place(shape):
    """HW-040 local frame (shaft at the origin) -> deck frame."""
    s = shape.copy()
    s.rotate(V(0, 0, 0), V(0, 0, 1), HW040_ROT)
    s.translate(V(SHAFT_C[0], SHAFT_C[1], 0))
    return s


def hw_xy(p):
    a = math.radians(HW040_ROT)
    return (SHAFT_C[0] + p[0] * math.cos(a) - p[1] * math.sin(a), SHAFT_C[1] + p[0] * math.sin(a) + p[1] * math.cos(a))


_bc = (-HW040_SHAFT[0], -HW040_SHAFT[1])    # board centre relative to the shaft
_hdr_local = _bc[0] + HW040_HDR_X           # header row from the shaft, module x
enc_pins = {n: hw_xy((_hdr_local, (2 - i) * 2.54)) for i, n in enumerate(HW040_PINS)}
enc_holes = [hw_xy((_bc[0] + hx, _bc[1] + hy)) for (hx, hy) in HW040_HOLES]


def hw_back_parts(grow=0.0, z0=None):
    """What sticks out of the HW-040's back (EC11 legs and resistors, header joints), module frame."""
    kx0, kx1, ky0, ky1 = HW040_BACK_KEEPOUT
    z0 = enc_board_back - HW040_BACK_PARTS - grow if z0 is None else z0
    legs = box(kx0 - grow, ky0 - grow, z0, kx1 + grow, ky1 + grow, enc_board_back + 0.01)
    joints = cbox(_hdr_local, 0, 1.6 + 2 * grow, 4 * 2.54 + 1.8 + 2 * grow, z0, enc_board_back + 0.01)
    return hw_place(legs.fuse(joints))


def _tile_base():
    """The +x +y tile in plan, as a face at z = 0."""
    g = TILE_GAP / 2
    outline = Part.Face(rr_wire(S, S, S_R))
    quad = Part.Face(Part.makePolygon([V(g, g, 0), V(S, g, 0), V(S, S, 0), V(g, S, 0), V(g, g, 0)]))
    hole = Part.Face(Part.Wire(Part.makeCircle(RING_R + TILE_GAP)))
    f = outline.common(quad).cut(hole).Faces[0]
    return f.makeOffset2D(-TILE_ROUND).makeOffset2D(TILE_ROUND).Faces[0]


TILE = _tile_base()
TILE_ANGLES = [90.0, 0.0, 180.0, 270.0]  # turns of the +x +y tile to each BTNS corner


def turned(pts, i):
    a = math.radians(TILE_ANGLES[i])
    return [(x * math.cos(a) - y * math.sin(a), x * math.sin(a) + y * math.cos(a)) for (x, y) in pts]


def tile_face(i):
    f = TILE.copy()
    f.rotate(V(0, 0, 0), V(0, 0, 1), TILE_ANGLES[i])
    return f


def tile_prism(i, z0, z1):
    f = tile_face(i)
    f.translate(V(0, 0, z0))
    return f.extrude(V(0, 0, z1 - z0))


LEGS = [turned(TILE_LEGS, i) for i in range(4)]


# The wedge: deck frame -> desk, tipped forward about x, lifted so the HW-040 board just clears the floor
def _tilt(c):
    m = App.Matrix()
    m.rotateX(math.radians(WEDGE_DEG))
    m.move(V(0, 0, c))
    return m


def _lift():
    b = hw_back_parts()
    b.transformShape(_tilt(0.0))
    return FLOOR_T + FLOOR_CLR - b.BoundBox.ZMin


TILT = _tilt(_lift())
UNTILT = TILT.inverse()


def tilted(shape, m=None):
    s = shape.copy()
    s.transformShape(TILT if m is None else m)
    return s


# ---------------------------------------------------------------------------------------------
# Copper-tape traces
# ---------------------------------------------------------------------------------------------


def copper_layout():
    """Nets as {name: (pads, traces)}, laid out by hand for these part positions; check_traces()
    reports the clearance between nets after any change. One layer, no crossovers.
    Pads: ("round", x, y, d) or ("rect", x, y, length, width). Traces: point lists, or
    ("w", width, points).

    Pin map as v1 (issue #67): display SCL 6, SDA 7, RES 8, DC 10, BLK 20; encoder CLK 2, DT 0,
    SW 1; buttons B1 3, B2 4, B3 5, and the fourth B4 on GPIO9.
    - Display: its five signals leave its pins to the west, step east above them, run up beside
      the SuperMini's right row and turn into their pins from the east.
    - Encoder: DT, SW, 3V3 and GND run down between the SuperMini rows, CLK and B1 down beside its
      left row; under the encoder pins they turn east and come up into them from the front.
    - B2 and B3 go straight into the back switches. B4 comes down between the rows too, passes
      north of the encoder and drops into the front-right switch. B1 into the front-left switch.
    - GND runs round the edge of the board, past all four switches (their front pin pair), and
      over to the display. VCC leaves the display to the east, runs down between the legs of the
      front-right switch, under it and back up into the encoder's + pin, which is 3V3."""
    xa, xb = sm_rows
    ys = {n: p[1] for n, p in sm_pins.items()}
    yd = {n: p[1] for n, p in disp_pins.items()}
    xe = {n: p[0] for n, p in enc_pins.items()}
    ye = enc_pins["SW"][1]
    dx = disp_hdr_x
    sm = lambda n: ("round", sm_pins[n][0], sm_pins[n][1], SM_PAD_D)
    dd = lambda n: ("rect", disp_pins[n][0], disp_pins[n][1], 2.6, 1.5)
    ep = lambda n: ("round", enc_pins[n][0], enc_pins[n][1], PIN_PAD_D)
    sw = lambda i, k: ("round", btn_pin(i, k)[0], btn_pin(i, k)[1], TRACE_PAD_D)
    leg = btn_pin
    W, DW, G = TRACE_W, DISP_TRACE_W, GAP_MIN

    def lanes(x0, x1, n, w):
        g = (x1 - x0 - n * w) / (n + 1)
        return [x0 + g + w / 2 + k * (w + g) for k in range(n)]

    # between the SuperMini rows: DT SW 3V3 GND B4
    ch = lanes(xa + PIN_ROW_BAND, xb - PIN_ROW_BAND, 5, W)
    # the display's five signals (BLK DC RES SDA SCL): up west of its pins, east above them, up
    # beside the right row
    p = DW + G
    d1 = [dx - PIN_ROW_BAND - G - DW / 2 - (4 - k) * p for k in range(5)]
    yj = [disp_pins["GND"][1] + PIN_ROW_BAND + G + DW / 2 + (4 - k) * p for k in range(5)]
    d2 = [xb + PIN_ROW_BAND + G + DW / 2 + k * p for k in range(5)]
    # below the SuperMini the channel lanes step west: first clear of the display's lanes ...
    s1 = (d1[0] - DW / 2 - G - W / 2) - ch[4]
    y1 = [ys["IO0"] - PIN_ROW_BAND - 0.12 - k * 0.85 for k in range(5)]
    # ... then (all but B4) on to the west of the encoder
    x_clk = xa - PIN_ROW_BAND - G - W / 2 - 4.0
    x_b1 = x_clk - W - 1.0
    fin = [x_clk + (W + G) * (k + 1) for k in range(4)]       # DT SW 3V3 GND
    y2 = [12.0 - k * 0.85 for k in range(4)]
    lane = lambda k: ([(ch[k], y1[k]), (ch[k] + s1, y1[k] + s1)] +
                      ([(ch[k] + s1, y2[k]), (fin[k], y2[k] - (ch[k] + s1 - fin[k]))] if k < 4 else []))
    yh = [ye - 3.1 - k * 2.4 for k in range(4)]                # under the encoder pins: 3V3 SW DT CLK
    y_b4 = -10.6                                               # B4 between the display pins and the encoder
    edge = INNER - C_INSET - 1.9                               # GND round the edge
    c45 = 10.0                                                 # its 45° corners, clear of the corner screws
    y_vcc_lo, x_vcc_up, y_vcc_in = -37.2, 10.0, -25.1
    x_vcc = (leg(3, 0)[0] + leg(3, 1)[0]) / 2                  # down between the front-right switch legs
    gnd_in = fin[3]
    nets = {
        "GND": ([sm("GND"), dd("GND"), ep("GND")] + [sw(i, k) for i in range(4) for k in (0, 1)],
                [[(xa, ys["GND"]), (ch[3], ys["GND"])] + lane(3) + [(gnd_in, ye + 1.0), (xe["GND"], ye)],
                 [(xa, ys["GND"]), (-edge + c45, ys["GND"]), (-edge, ys["GND"] - c45), (-edge, -edge + c45),  # 45° past the back-left screw
                  (-edge + c45, -edge), (edge - c45, -edge), (edge, -edge + c45), (edge, leg(1, 1)[1])],
                 [(-edge, leg(0, 0)[1]), leg(0, 0)], [(-edge, leg(2, 0)[1]), leg(2, 0)],
                 [(edge, leg(1, 1)[1]), leg(1, 1)], [(edge, leg(3, 1)[1]), leg(3, 1)],
                 [(dx, yd["GND"]), (edge, yd["GND"])]]),
        "3V3": ([sm("3V3"), dd("VCC"), ep("+")],
                [[(xa, ys["3V3"]), (ch[2], ys["3V3"])] + lane(2) + [(fin[2], yh[0]), (xe["+"], yh[0]), (xe["+"], ye)],
                 [(dx, yd["VCC"]), (x_vcc, yd["VCC"]), (x_vcc, y_vcc_lo), (x_vcc_up, y_vcc_lo),
                  (x_vcc_up, y_vcc_in), (xe["+"], y_vcc_in), (xe["+"], ye)]]),
        "SW": ([sm("IO1"), ep("SW")], [[(xa, ys["IO1"]), (ch[1], ys["IO1"])] + lane(1) +
                                       [(fin[1], yh[1]), (xe["SW"], yh[1]), (xe["SW"], ye)]]),
        "DT": ([sm("IO0"), ep("DT")], [[(xa, ys["IO0"]), (ch[0], ys["IO0"])] + lane(0) +
                                       [(fin[0], yh[2]), (xe["DT"], yh[2]), (xe["DT"], ye)]]),
        "CLK": ([sm("IO2"), ep("CLK")], [[(xa, ys["IO2"]), (x_clk, ys["IO2"]), (x_clk, yh[3]), (xe["CLK"], yh[3]), (xe["CLK"], ye)]]),
        "B1": ([sm("IO3"), sw(2, 2), sw(2, 3)], [[(xa, ys["IO3"]), (x_b1, ys["IO3"]), (x_b1, leg(2, 3)[1]), leg(2, 3)]]),
        "B2": ([sm("IO4"), sw(0, 2), sw(0, 3)], [[(xa, ys["IO4"]), (leg(0, 3)[0], ys["IO4"]), leg(0, 3)]]),
        "B3": ([sm("IO5"), sw(1, 2), sw(1, 3)], [[(xb, ys["IO5"]), (leg(1, 2)[0], ys["IO5"]), leg(1, 2)]]),
        "B4": ([sm("IO9"), sw(3, 2), sw(3, 3)],
               [[(xb, ys["IO9"]), (ch[4], ys["IO9"])] + lane(4) + [(ch[4] + s1, y_b4), (leg(3, 2)[0], y_b4), leg(3, 2)]]),
        "nc5V": ([sm("5V")], []), "ncIO21": ([sm("IO21")], []),
    }
    for k, (name, pin) in enumerate([("BLK", "IO20"), ("DC", "IO10"), ("RES", "IO8"), ("SDA", "IO7"), ("SCL", "IO6")]):
        nets[name] = ([sm(pin), dd(name)],
                      [("w", DW, [(dx, yd[name]), (d1[k], yd[name]), (d1[k], yj[k]), (d2[k], yj[k]), (d2[k], ys[pin]), (xb, ys[pin])])])
    return nets


def trace_solid(pts, w, z0, z1):
    """A trace: a w wide strip along the points, round at every corner."""
    parts = [cyl(w, x, y, z0, z1) for (x, y) in pts]
    for a, b in zip(pts, pts[1:]):
        L = math.hypot(b[0] - a[0], b[1] - a[1])
        if L > 1e-6:
            s = box(0, -w / 2, z0, L, w / 2, z1)
            s.rotate(V(0, 0, 0), V(0, 0, 1), math.degrees(math.atan2(b[1] - a[1], b[0] - a[0])))
            s.translate(V(a[0], a[1], 0))
            parts.append(s)
    return fuse_all(parts)


def net_solid(net, z0, z1, grow=0.0):
    pads, traces = net
    parts = []
    for pad in pads:
        if pad[0] == "rect":
            parts.append(rr_prism(pad[3] + 2 * grow, pad[4] + 2 * grow, 0.4 + grow, z0, z1, pad[1], pad[2]))
        else:
            parts.append(cyl(pad[3] + 2 * grow, pad[1], pad[2], z0, z1))
    for t in traces:
        w, pts = (t[1], t[2]) if t and t[0] == "w" else (TRACE_W, t)
        parts.append(trace_solid(pts, w + 2 * grow, z0, z1))
    return fuse_all(parts)


def board_nets(z0, z1, grow=0.0):
    return {k: net_solid(n, z0, z1, grow) for k, n in copper_layout().items()}


def carrier_outline():
    w, r = 2 * (INNER - C_INSET), BODY_R - WALL - C_INSET
    c = rr_prism(w, w, r, C_BOT, C_TOP)
    tab = box(SM_X - SM[0] / 2 - 0.5, INNER - C_INSET - 1, C_BOT, SM_X + SM[0] / 2 + 0.5, INNER - 0.4, C_TOP)
    return c.fuse(tab).removeSplitter()


def carrier_through_cuts():
    z0, z1 = C_BOT - 1, C_TOP + 1
    cuts = [cyl(EC11_BUSH_D + 2 * EC11_HOLE_CLR, SHAFT_C[0], SHAFT_C[1], z0, z1)]
    cuts += [cyl(HOLE_D, x, y, z0, z1) for (x, y) in list(sm_pins.values()) + list(disp_pins.values()) + list(enc_pins.values())]
    cuts += [cyl(SW_LEG_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(4) for k in range(4)]
    cuts += [cyl(SPIGOT_HOLE_D, x, y, z0, z1) for (x, y) in SCREWS]
    return cuts


def copper_grooves():
    """The gap along both sides of every trace and pad, cut into the printed PCB's underside. Where
    two gaps leave less than TRACE_RIDGE_MIN of plastic between them (or to a hole or the edge),
    that sliver becomes gap too, and so does any island of plastic smaller than TRACE_ISLAND_MIN;
    gaps thinner than TRACE_GROOVE_MIN are filled in. The pin rows stay solid copper bands."""
    z0, z1 = C_BOT - 1, C_BOT + TRACE_GROOVE[1]
    flat = lambda shp: [f for f in shp.Faces if abs(f.normalAt(0, 0).z) > 0.99 and abs(f.CenterOfMass.z - z0) < 1e-6]
    outer = fuse_all(list(board_nets(z0, z1, TRACE_GROOVE[0]).values()))
    copper = fuse_all(list(board_nets(z0 - 1, z1 + 1).values()))
    board = carrier_outline().cut(carrier_through_cuts())
    plate = Part.makeCompound([f.copy() for f in board.Faces
                               if abs(f.normalAt(0, 0).z + 1) < 1e-6 and abs(f.CenterOfMass.z - C_BOT) < 1e-6])
    plate.translate(V(0, 0, z0 - C_BOT))
    field = plate.cut(Part.makeCompound(flat(outer)))  # plastic left between the gaps
    plate = Part.makeCompound(plate.Faces)
    r = TRACE_RIDGE_MIN / 2

    def offset(faces, d):
        out = []
        for f in faces:
            try:  # fine polygons offset reliably (some outlines do not)
                wires = [Part.makePolygon(w.discretize(Deflection=0.01) + [w.discretize(Deflection=0.01)[0]])
                         for w in f.Wires]
                out += Part.makeFace(wires, "Part::FaceMakerBullseye").makeOffset2D(d).Faces
            except Exception as exc:
                if "no wires" not in str(exc):  # a real failure: keep the face as it is
                    out.append(f)              # (no wires = it shrank away: a sliver)
        return out

    kept = offset(offset(field.Faces, -r), r)
    small = [f for f in kept if f.Area < TRACE_ISLAND_MIN]
    print("plastic islands made gap: %d (%s mm²)" % (len(small), ", ".join("%.1f" % f.Area for f in small)))
    kept = Part.makeCompound([f for f in kept if f.Area >= TRACE_ISLAND_MIN])
    keep = kept.extrude(V(0, 0, z1 - z0 + 1)) if kept.Faces else None
    grooves = Part.makeCompound([f.extrude(V(0, 0, z1 - z0)) for f in plate.Faces]).cut(copper)
    rows = [[sm_pins[n] for n in SM_COL_A], [sm_pins[n] for n in SM_COL_B], list(disp_pins.values()), list(enc_pins.values())]
    for pts in rows:  # solid strips along the pin rows
        xs, ys_ = [q[0] for q in pts], [q[1] for q in pts]
        grooves = grooves.cut(box(min(xs) - PIN_ROW_BAND, min(ys_) - PIN_ROW_BAND, z0 - 1,
                                  max(xs) + PIN_ROW_BAND, max(ys_) + PIN_ROW_BAND, z1 + 1))
    grooves = grooves.cut(keep) if keep else grooves
    opened, crumbs = [], []
    for f in flat(grooves):  # a face that opens away is a crumb; one the offset mangles is kept
        o = offset(offset([f], -TRACE_GROOVE_MIN / 2), TRACE_GROOVE_MIN / 2)
        a = sum(g.Area for g in o)
        if a < 1e-6 and f.Area < 3.0:
            crumbs.append(f.Area)
        else:
            opened += o if a > 0.5 * f.Area else [f]
    print("gap crumbs filled: %d (%s mm²)" % (len(crumbs), ", ".join("%.2f" % c for c in crumbs)))
    return Part.makeCompound([f.extrude(V(0, 0, z1 - z0)) for f in opened])


def check_traces():
    """Smallest gap between two different nets (the plastic wall between their copper)."""
    sol = board_nets(C_BOT - TAPE_T, C_BOT)
    keys, worst = list(sol), (99.0, "", "")
    for i in range(len(keys)):
        for j in range(i + 1, len(keys)):
            d = sol[keys[i]].distToShape(sol[keys[j]])[0]
            worst = min(worst, (d, keys[i], keys[j]))
    return worst


# ---------------------------------------------------------------------------------------------
# Parts (deck frame, except the floor)
# ---------------------------------------------------------------------------------------------


def peg(x, y, hole_d, z0, z1, clr=PEG_CLR):
    """Locating peg with a 0.3 mm lead-in chamfer at the tip."""
    d = hole_d - 2 * clr
    body = cyl(d, x, y, z0, z1 - 0.3)
    tip = Part.makeCone(d / 2, d / 2 - 0.3, 0.3, V(x, y, z1 - 0.3), V(0, 0, 1))
    return body.fuse(tip)


def make_shell():
    """Walls and sub-deck, printed sub-deck down. The walls run on down to the floor (cut there
    once the assembly is tipped)."""
    z_low = -30.0
    s = rr_prism(BODY, BODY, BODY_R, z_low, SUBDECK_TOP)
    s = s.cut(rr_prism(BODY - 2 * WALL, BODY - 2 * WALL, BODY_R - WALL, z_low - 1, subdeck_under))
    adds = []
    for legs in LEGS:  # guide bosses for the tile legs
        adds += [cyl(LEG_BOSS_D, x, y, LEG_BOTTOM, subdeck_under + 0.01) for (x, y) in legs]
    for (x, y) in SCREWS:  # corner posts down onto the printed PCB (round the tile legs), a spigot into its hole
        adds.append(cyl(LEG_BOSS_D, x, y, C_TOP, subdeck_under + 0.01))
        adds.append(cyl(SPIGOT_D, x, y, C_BOT + 0.2, C_TOP + 0.01))
    s = s.fuse(adds)
    cuts = [cyl(2 * RING_HOLE, 0, 0, subdeck_under - 1, SUBDECK_TOP + 1)]
    cuts += [cyl(SW_HOLE_D, x, y, subdeck_under - 1, SUBDECK_TOP + 1) for (x, y) in BTNS]
    for legs in LEGS:
        cuts += [cyl(LEG_HOLE_D, x, y, LEG_BOTTOM - 1, SUBDECK_TOP + 1) for (x, y) in legs]
    cuts += [cyl(SCREW_PILOT, x, y, C_BOT, C_BOT + 0.2 + SCREW_IN) for (x, y) in SCREWS]
    # the tile leg's hole in each corner post ends in a 45° cone (prints without a flat ceiling)
    cuts += [Part.makeCone(LEG_HOLE_D / 2, SCREW_PILOT / 2, (LEG_HOLE_D - SCREW_PILOT) / 2,
                           V(x, y, LEG_BOTTOM - 1 + 0.01), V(0, 0, -1)) for (x, y) in SCREWS]
    # USB-C: a hidden pocket thins the back wall to USB_WALL round the receptacle and the board end
    yw = BODY / 2 - USB_WALL
    cuts.append(box(SM_X - SM[0] / 2 - 0.3, INNER - 1, sm_lo - 0.3, SM_X + SM[0] / 2 + 0.3, yw, sm_hi + SM_USB[2] + 0.3))
    pill = Part.Face(rr_wire(USB_HOLE[0], USB_HOLE[1], USB_HOLE[1] / 2)).extrude(V(0, 0, 3))
    pill.rotate(V(0, 0, 0), V(1, 0, 0), -90)
    pill.translate(V(SM_X, yw - 1, usb_zc))
    cuts.append(pill)
    return s.cut(cuts).removeSplitter()


def make_ring():
    above = ring_solid(GEAR_BAND_R - 0.4, RING_R, SUBDECK_TOP, GEAR_Z[1]).fuse(
        ring_solid(RING_BAND_R, RING_R, GEAR_Z[1], RING_TOP))
    try:
        top = [e for e in above.Edges if e.Curve.TypeId == "Part::GeomCircle"
               and abs(e.BoundBox.ZMin - RING_TOP) < 1e-6 and e.Curve.Radius > RING_R - 0.1]
        above = above.makeChamfer(RING_CHAMFER, top)
    except Exception as exc:
        print("ring chamfer skipped:", exc)
    lip = ring_solid(RING_BORE, RING_BAND_R + 0.5, RING_TOP - RING_LIP_T, RING_TOP)
    neck = ring_solid(GEAR_BAND_R - 0.4, RING_NECK_R, ret_top, SUBDECK_TOP + 0.01)
    hole = gear_face(RING_TEETH, True).extrude(V(0, 0, GEAR_Z[1] - GEAR_Z[0] + 2))
    hole.translate(V(0, 0, GEAR_Z[0] - 1))
    teeth = cyl(2 * GEAR_BAND_R, 0, 0, GEAR_Z[0], GEAR_Z[1]).cut(hole)
    # a 45° shoulder from the band down onto the tooth tips, so the teeth print on it (top down)
    tip = RING_TEETH * GEAR_M / 2 - GEAR_M
    shoulder = revolve([(tip, GEAR_Z[1] - 0.01), (RING_BAND_R + 0.01, GEAR_Z[1] - 0.01),
                        (RING_BAND_R + 0.01, GEAR_Z[1] + RING_BAND_R - tip)])
    r = fuse_all([above, lip, neck, teeth, shoulder])
    try:
        inner = [e for e in r.Edges if e.Curve.TypeId == "Part::GeomCircle"
                 and abs(e.BoundBox.ZMin - RING_TOP) < 1e-6 and abs(e.Curve.Radius - RING_BORE) < 0.01]
        r = r.makeChamfer(0.6, inner)
    except Exception as exc:
        print("ring bore chamfer skipped:", exc)
    cuts = []
    for i in range(FLUTES):
        x, y = polar(RING_R + 0.2, 360.0 * i / FLUTES)
        cuts.append(cyl(FLUTE_D, x, y, *FLUTE_Z))
    for a in RET_SCREWS:
        x, y = polar(RET_SCREW_R, a)
        cuts.append(cyl(1.6, x, y, ret_top - 1, ret_top + 6))
    return r.cut(cuts).removeSplitter()


def make_retainer():
    r = ring_solid(RET_R[0], RET_R[1], ret_top - RET_T, ret_top)
    return r.cut([cyl(2.4, *polar(RET_SCREW_R, a), ret_top - RET_T - 1, ret_top + 1) for a in RET_SCREWS])


def make_floor():
    """Flat on the desk, its edge under the (tipped) walls; four posts under the printed PCB with
    the screws, two pillars with pegs through the HW-040's holes, and pockets under the parts on
    the HW-040's back. Built on the desk from tipped deck-frame pieces."""
    slab = box(-80, -80, 0, 80, 80, FLOOR_T)
    p = tilted(rr_prism(BODY, BODY, BODY_R, -40, 40)).common(slab)
    above = box(-80, -80, 0, 80, 80, 80)
    adds = [tilted(cyl(POST_D, x, y, -30, C_BOT)) for (x, y) in SCREWS]
    for (x, y) in enc_holes:
        adds.append(tilted(cyl(HW040_COL_D, x, y, -30, enc_board_back).fuse(
            peg(x, y, HW040_HOLE_D, enc_board_back - 0.01, enc_board_front + PEG_PROUD, HW040_PEG_CLR))))
    adds.append(tilted(enc_corners()))
    p = p.fuse([a.common(above) for a in adds])
    seat = C_BOT + 0.2 + SCREW_IN - 0.5 - SCREW_L  # every screw head seats here (deck frame, along the screw)
    cuts = []
    for (x, y) in SCREWS:
        cuts.append(tilted(cyl(SCREW_CLR, x, y, -40, C_BOT + 1)))
        cuts.append(tilted(cyl(SCREW_HEAD_D, x, y, -40, seat)))
        cuts.append(tilted(Part.makeCone(SCREW_HEAD_D / 2 + 0.01, SCREW_CLR / 2, 1.6, V(x, y, seat - 0.01), V(0, 0, 1))))
    d, wd, dp = FOOT_RING
    for sx in (-1, 1):
        for sy in (-1, 1):
            fx, fy = sx * 33.0, sy * 33.0  # inside the corner screws
            cuts.append(cyl(d, fx, fy, -1, dp).cut(cyl(d - 2 * wd, fx, fy, -2, dp + 1)))
    t = text_solid("KIVORI", 0.0, 0.0, 7.0, -0.1, TEXT_D + 0.1, mirror=True)
    if t:
        cuts.append(t)
    return p.cut(cuts).removeSplitter()


def enc_corners():
    """Four L-shaped corner borders under the HW-040 board (deck frame): a ledge under each corner
    and a low wall round it, clear of the parts on its back."""
    clr, wt, wh, ledge = ENC_BORDER
    L, Wd = HW040[0] + 2 * clr, HW040[1] + 2 * clr
    outer = lambda z0, z1: cbox(_bc[0], _bc[1], L + 2 * wt, Wd + 2 * wt, z0, z1)
    shelf = outer(-30, enc_board_back).cut(cbox(_bc[0], _bc[1], HW040[0] - 2 * ledge, HW040[1] - 2 * ledge, -40, 40))
    wall = outer(enc_board_back - 0.01, enc_board_back + wh).cut(cbox(_bc[0], _bc[1], L, Wd, -40, 40))
    a = ENC_CORNER + clr + wt
    corners = fuse_all([cbox(_bc[0] + su * (L / 2 + wt - a / 2), _bc[1] + sv * (Wd / 2 + wt - a / 2), a, a, -40, 40)
                        for su in (-1, 1) for sv in (-1, 1)])
    return hw_place(shelf.fuse(wall).common(corners))


def make_carrier():
    """The printed PCB. Prints copper side (underside) down; its posts point up: four for the
    display, two hinge posts for the bezel, braced to the display posts."""
    c = carrier_outline().cut(carrier_through_cuts())
    if COPPER_GROOVES:
        c = c.cut(copper_grooves())
    adds = []
    for (x, y) in disp_holes:
        adds.append(cyl(DISP_STANDOFF_D, x, y, C_TOP - 0.01, dpcb_z0))
        adds.append(peg(x, y, DISP_HOLE_D, dpcb_z0 - 0.01, dpcb_z1 + PEG_PROUD))
    for s in (-1, 1):
        y0, y1 = sorted((s * HINGE_POST_Y[0], s * HINGE_POST_Y[1]))
        post = box(HINGE_POST_X[0], y0, C_TOP - 0.01, HINGE_POST_BASE_X, y1, dpcb_z0 - 0.5)
        post = post.fuse(box(HINGE_POST_X[0], y0, dpcb_z0 - 0.6, HINGE_POST_X[1], y1, HINGE_Z + 1.8))
        hx, hy = [h for h in disp_holes if h[0] < 0 and h[1] * s > 0][0]
        rib = box(HINGE_X, min(s * HINGE_POST_Y[1], hy), C_TOP - 0.01, hx - 0.5, max(s * HINGE_POST_Y[1], hy), dpcb_z0 - 2.0)
        post = post.fuse(rib)
        post = post.cut(ycyl(HINGE_HOLE_D, HINGE_X, HINGE_Z, y0 - 1, y1 + 1))
        post = post.cut(box(HINGE_X - HINGE_SLOT / 2, y0 - 1, HINGE_Z, HINGE_X + HINGE_SLOT / 2, y1 + 1, HINGE_Z + 5))
        adds.append(post)
    return c.fuse(adds).removeSplitter()


def make_pinion():
    """Teeth low on the round part of the shaft, a tall hub with a D bore over the D-flat."""
    g = gear_face(PINION_TEETH, False).extrude(V(0, 0, PINION_Z[1] - PINION_Z[0]))
    g.rotate(V(0, 0, 0), V(0, 0, 1), PINION_PHASE * 360.0 / PINION_TEETH)
    g.translate(V(SHAFT_C[0], SHAFT_C[1], PINION_Z[0]))
    hub = cyl(PINION_HUB_D, SHAFT_C[0], SHAFT_C[1], PINION_Z[1] - 0.01, PINION_TOP)
    # bore built in the encoder's own frame, so its flat matches the turned shaft
    z_flat = enc_shaft_top - EC11_FLAT_L + 0.3
    bore = cyl(EC11_SHAFT_D + 0.15, 0, 0, PINION_Z[0] - 1, PINION_TOP + 1)
    yf, yb = EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, (EC11_SHAFT_D + 0.15) / 2
    prof = [(yf, z_flat + (yb - yf)), (yb, z_flat), (5, z_flat), (5, PINION_TOP + 2), (yf, PINION_TOP + 2)]
    flat = Part.Face(Part.makePolygon([V(-5, y, z) for (y, z) in prof] + [V(-5, prof[0][0], prof[0][1])])).extrude(V(10, 0, 0))
    # (the flat starts on a 45° ramp from the bore wall, so it prints without a ledge)
    return g.fuse(hub).cut(hw_place(bore.cut(flat))).removeSplitter()


def make_cap():
    """The bezel: a round face with the square window, a skirt, a flange under the ring lip, the
    hinge lug and pins on the left and the nub that rests on the encoder shaft."""
    face = cyl(2 * CAP_R, 0, 0, cap_under, CAP_TOP)
    try:
        face = face.makeChamfer(0.6, [e for e in face.Edges if abs(e.BoundBox.ZMin - CAP_TOP) < 1e-6])
    except Exception as exc:
        print("cap chamfer skipped:", exc)
    skirt = ring_solid(CAP_R - CAP_SKIRT_T, CAP_R, CAP_BOTTOM, cap_under + 0.01)
    rf = CAP_FLANGE_R  # flange under the ring lip; 45° top so the bezel prints face down
    flange = revolve([(CAP_R - 0.5, cap_under + 0.3), (CAP_R - 0.01, cap_under + 0.3),
                      (rf, cap_under + 0.3 - (rf - CAP_R)), (rf, CAP_BOTTOM + 1.8), (CAP_R - 0.5, CAP_BOTTOM + 1.8)])
    lw, lh = HINGE_LUG
    lug = box(HINGE_X - lw / 2, -lh, HINGE_Z - 1.5, HINGE_X + lw / 2, lh, cap_under + 0.01)
    pin = teardrop_y(HINGE_PIN_D, HINGE_X, HINGE_Z, -(HINGE_POST_Y[1] - 0.4), HINGE_POST_Y[1] - 0.4)
    nub = cyl(NUB_D, SHAFT_C[0], SHAFT_C[1], enc_shaft_top, cap_under + 0.01)
    c = face.fuse([skirt, flange, lug, pin, nub])
    win = rr_prism(WINDOW, WINDOW, WINDOW_R, cap_under - 1, CAP_TOP + 1)
    cham = Part.makeLoft([rr_wire(WINDOW, WINDOW, WINDOW_R, CAP_TOP - WINDOW_CHAMFER),
                          rr_wire(WINDOW + 2 * WINDOW_CHAMFER, WINDOW + 2 * WINDOW_CHAMFER,
                                  WINDOW_R + WINDOW_CHAMFER, CAP_TOP + 0.01)], True)
    win = win.fuse(cham)
    win.translate(V(DISP_C[0], DISP_C[1], 0))
    return c.cut(win).removeSplitter()


def make_tile(i):
    """A quarter of the top: plate, lip over the wall top, three legs, switch socket.
    Printed face down, so all four faces have the same bed finish."""
    plate = tile_prism(i, tile_under, DECK_TOP)
    try:
        top = [e for e in plate.Edges if abs(e.BoundBox.ZMin - DECK_TOP) < 1e-6 and abs(e.BoundBox.ZMax - DECK_TOP) < 1e-6]
        plate = plate.makeChamfer(TILE_CHAMFER, top)
    except Exception as exc:
        print("tile chamfer skipped:", exc)
    lip = rr_prism(S, S, S_R, TILE_SKIRT_Z, tile_under + 0.01).cut(
        rr_prism(S - 2 * TILE_SKIRT_T, S - 2 * TILE_SKIRT_T, S_R - TILE_SKIRT_T, TILE_SKIRT_Z - 1, DECK_TOP))
    lip = lip.common(tile_prism(i, TILE_SKIRT_Z - 1, DECK_TOP))
    legs = [cyl(LEG_D, x, y, LEG_BOTTOM, tile_under + 0.01) for (x, y) in LEGS[i]]
    x, y = BTNS[i]
    sock = SW_HEAD + 2 * SOCK_CLR
    tip_bottom = SW_PLUNGER_TOP - SW_HEAD_H
    hd, hh = SOCK_HOOK
    z_hook_top = tip_bottom - 0.1
    z_bot = z_hook_top - hh
    slit_top = DECK_TOP - 1.0
    boss = cbox(x, y, sock + 2 * SOCK_WALL, sock + 2 * SOCK_WALL, z_bot, tile_under + 0.01)
    t = fuse_all([plate, lip, boss] + legs)
    cuts = [cbox(x, y, sock, sock, z_bot - 1, SW_PLUNGER_TOP)]
    o = sock / 2 + SOCK_WALL / 2  # slits at the four corners, up into the tile
    cuts += [cbox(x + sx * o, y + sy * o, SOCK_SLIT + SOCK_WALL, SOCK_SLIT + SOCK_WALL, z_bot - 1, slit_top)
             for sx in (-1, 1) for sy in (-1, 1)]
    # a gap round the socket up into the tile, so the walls flex from 1 mm under the face
    bo = sock + 2 * SOCK_WALL
    cuts.append(cbox(x, y, bo + 1.2, bo + 1.2, tile_under - 0.01, slit_top).cut(
        cbox(x, y, bo, bo, tile_under - 1, slit_top + 1)))
    t = t.cut(cuts)
    hooks = []
    for sx in (-1, 1):  # two opposite walls get a hook: 45° lead-in below, flat top holds the tip
        w = sock / 2
        prof = [(w + 0.01, z_bot), (w - hd, z_bot + hd), (w - hd, z_hook_top), (w + 0.01, z_hook_top)]
        hw = sock / 2 - SOCK_SLIT  # clear of the corner slits
        f = Part.Face(Part.makePolygon([V(sx * px, -hw, pz) for (px, pz) in prof]
                                       + [V(sx * prof[0][0], -hw, prof[0][1])]))
        hk = f.extrude(V(0, 2 * hw, 0))
        hk.translate(V(x, y, 0))
        hooks.append(hk)
    return t.fuse(hooks).removeSplitter()


# Stand-ins for the bought parts (deck frame)


def dummy_encoder():
    t = enc_board_front
    pcb = cbox(_bc[0], _bc[1], HW040[0], HW040[1], enc_board_back, t).cut(
        [cyl(HW040_HOLE_D, _bc[0] + hx, _bc[1] + hy, enc_board_back - 1, t + 1) for (hx, hy) in HW040_HOLES])
    body = cbox(0, 0, EC11_BODY[0], EC11_BODY[1], t, enc_body_top)
    bush = cyl(EC11_BUSH_D, 0, 0, enc_body_top, bush_top)
    shaft = cyl(EC11_SHAFT_D, 0, 0, bush_top, enc_shaft_top)
    shaft = shaft.cut(box(-5, EC11_SHAFT_D / 2 - EC11_FLAT, enc_shaft_top - EC11_FLAT_L, 5, 5, enc_shaft_top + 1))
    hdr = cbox(_hdr_local, 0, 2.5, 5 * 2.54, t, t + 2.5)
    pins = fuse_all([cbox(_hdr_local, (2 - i) * 2.54, 0.64, 0.64, t, t + 2.5 + 6.0) for i in range(5)])
    return hw_place(fuse_all([pcb, body, bush, shaft, hdr, pins])).fuse(hw_back_parts())


def dummy_display():
    """The display with its own header, plugged into the two stacked female headers."""
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], glass_z0, glass_z1)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], dpcb_z0, dpcb_z1).cut(
        [cyl(DISP_HOLE_D, x, y, dpcb_z0 - 1, dpcb_z1 + 1) for (x, y) in disp_holes])
    hdr = cbox(disp_hdr_x, dpcb_c[1], 2.5, 7 * 2.54, socket_top, dpcb_z0)
    sockets = cbox(disp_hdr_x, dpcb_c[1], 2.54, 7 * 2.54, C_TOP, socket_top)
    tails = fuse_all([cbox(x, y, 0.64, 0.64, C_BOT - 1.0, C_TOP) for (x, y) in disp_pins.values()])
    return fuse_all([glass, pcb, hdr, sockets, tails])


def dummy_switch(i):
    x, y = BTNS[i]
    body = cbox(x, y, SW_BODY, SW_BODY, sw_base, sw_base + SW_BODY_H)
    boss = cyl(SW_BOSS_D, x, y, sw_base + SW_BODY_H - 0.01, sw_base + SW_BOSS_H)
    neck = cbox(x, y, SW_NECK, SW_NECK, sw_base + SW_BOSS_H - 0.01, SW_PLUNGER_TOP - SW_HEAD_H + 0.01)
    head = cbox(x, y, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H, SW_PLUNGER_TOP)
    legs = [cyl(0.7, x + dx, y + dy, C_BOT - 1.0, sw_base) for (dx, dy) in SW_PADS]
    return fuse_all([body, boss, neck, head] + legs)


def dummy_supermini():
    hdr_y = SM_USB_END - SM_PIN_MARGIN - 3.5 * 2.54
    strips = [cbox(x, hdr_y, SM_HDR[0], SM_HDR[1], C_TOP, sm_lo) for x in sm_rows]
    pins = fuse_all([cbox(x, y, 0.64, 0.64, C_BOT - 0.5, sm_lo) for (x, y) in sm_pins.values()])
    pcb = cbox(sm_c[0], sm_c[1], SM[0], SM[1], sm_lo, sm_hi)
    usb = cbox(SM_X, SM_USB_END + SM_USB_OVERHANG - SM_USB[1] / 2, SM_USB[0], SM_USB[1], sm_hi, sm_hi + SM_USB[2])
    chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_hi, sm_hi + 0.9)
    return fuse_all(strips + [pins, pcb, usb, chip])


def dummy_copper():
    """Copper tape flush on the underside."""
    return fuse_all(list(board_nets(C_BOT - TAPE_T, C_BOT).values())).cut(carrier_through_cuts())


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

LOCAL = {}                  # printed parts in the deck frame (how they print)
PRINTED = {}                # printed parts on the desk
DUMMIES = {}
BUTTON_NAMES = ["TileBackLeft", "TileBackRight", "TileFrontLeft", "TileFrontRight"]
COLORS = {
    "Ring": (0.80, 0.80, 0.82), "Shell": (0.13, 0.13, 0.15), "Floor": (0.13, 0.13, 0.15),
    "Retainer": (0.12, 0.45, 0.25), "Cap": (0.04, 0.04, 0.05), "Carrier": (0.12, 0.45, 0.25),
    "Pinion": (0.95, 0.50, 0.18), "Dummy": (0.30, 0.45, 0.85), "Copper": (0.85, 0.50, 0.20),
}
FLIP = ("Ring", "Shell", "Cap") + tuple(BUTTON_NAMES)  # printed upside down


def build():
    LOCAL["Shell"] = make_shell()
    LOCAL["Ring"] = make_ring()
    LOCAL["Retainer"] = make_retainer()
    LOCAL["Carrier"] = make_carrier()
    LOCAL["Pinion"] = make_pinion()
    LOCAL["Cap"] = make_cap()
    for i, n in enumerate(BUTTON_NAMES):
        LOCAL[n] = make_tile(i)
        DUMMIES["Dummy_" + n.replace("Tile", "Switch")] = tilted(dummy_switch(i))
    for name, shp in LOCAL.items():
        PRINTED[name] = tilted(shp)
    # the walls end on the floor
    PRINTED["Shell"] = PRINTED["Shell"].cut(box(-80, -80, -50, 80, 80, FLOOR_T)).removeSplitter()
    LOCAL["Shell"] = tilted(PRINTED["Shell"], UNTILT)
    PRINTED["Floor"] = make_floor()
    DUMMIES["Dummy_Encoder"] = tilted(dummy_encoder())
    DUMMIES["Dummy_Display"] = tilted(dummy_display())
    DUMMIES["Dummy_SuperMini"] = tilted(dummy_supermini())
    DUMMIES["Dummy_Copper"] = tilted(dummy_copper())


def check_interference():
    items = list(PRINTED.items()) + list(DUMMIES.items())
    clashes = []
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            (na, a), (nb, b) = items[i], items[j]
            if not a.BoundBox.intersect(b.BoundBox):
                continue
            v = a.common(b).Volume
            if v > 0.01:
                clashes.append((na, nb, round(v, 3)))
    return clashes


def color_key(name):
    if name == "Dummy_Copper":
        return "Copper"
    if name.startswith("Dummy"):
        return "Dummy"
    return "Shell" if name.startswith("Tile") else name


def to_doc():
    if DOC_NAME in App.listDocuments():
        App.closeDocument(DOC_NAME)
    doc = App.newDocument(DOC_NAME)
    for name, shp in list(PRINTED.items()) + list(DUMMIES.items()):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shp
        if App.GuiUp and obj.ViewObject:
            obj.ViewObject.ShapeColor = COLORS[color_key(name)]
            obj.ViewObject.Visibility = True
    doc.recompute()
    return doc


def print_pose(name, shp):
    s = shp.copy()
    if name in FLIP:
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180)
    bb = s.optimalBoundingBox()
    s.translate(V(-bb.XMin, -bb.YMin, -bb.ZMin))
    return s


def export(doc, here):
    exp = os.path.join(here, "export")
    os.makedirs(exp, exist_ok=True)
    for f in os.listdir(exp):  # drop STLs of parts that no longer exist
        if f.endswith(".stl") and f[:-4] not in PRINTED:
            os.remove(os.path.join(exp, f))
    import MeshPart

    for name in PRINTED:
        shp = LOCAL.get(name, PRINTED[name])
        mesh = MeshPart.meshFromShape(Shape=print_pose(name, shp), LinearDeflection=0.02,
                                      AngularDeflection=math.radians(10), Relative=False)
        mesh.write(os.path.join(exp, "%s.stl" % name))
    doc.saveAs(os.path.join(here, "kivori-case-experiment.FCStd"))


def heights():
    """Body height at the front and back edge (tile tops), and the ring's highest point."""
    tiles = fuse_all([PRINTED[n] for n in BUTTON_NAMES])
    front = tiles.common(box(-80, -80, -10, 80, -40, 80)).BoundBox.ZMax
    back = tiles.BoundBox.ZMax
    return front, back, PRINTED["Ring"].BoundBox.ZMax


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    d, a, b = check_traces()
    print("trace clearance: %.2f mm (%s / %s), need 0.8" % (d, a, b))
    print("height: front %.1f, back %.1f, ring top %.1f mm (wedge %.1f°)" % (heights() + (WEDGE_DEG,)))
    for name, shp in PRINTED.items():
        bb = shp.BoundBox
        print("%-16s vol %8.0f mm3  bbox %.1f x %.1f x %.1f  valid=%s solids=%d" % (
            name, shp.Volume, bb.XLength, bb.YLength, bb.ZLength, shp.isValid(), len(shp.Solids)))
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):
    main()
