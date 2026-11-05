"""Kivori experimental enclosure: square body, a turning ring round a static screen, 4 buttons.

FreeCAD 1.1, Part workbench, no GUI needed. Run it inside FreeCAD (GUI Python console, the FreeCAD
MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-experiment/build_case.py"
    exec(open(p).read(), {"__file__": p, "__name__": "__main__"})

How it works with the standard parts (GC9A01 1.28" round LCD, HW-040 / EC11 encoder):
  * The screen sits still in the middle, under a circular window in a round black bezel.
  * The ring turns round it. Teeth on the inside of the ring drive a small gear on the encoder
    shaft, which sits off-centre in front of the screen (an internal gear pair: both turn the same way).
  * The ring is captured on the moving bezel. Four replaceable printed leaves suspend an X
    bridge whose central nub presses a fifth tactile switch below the fixed display.
    The HW-040 measures rotation only; its SW pin is unwired.
  * The whole top is four quadrant tiles round the ring: those are the buttons.
  * Enamelled copper wire in channels connects the parts, with Kapton at crossings (v4/main pin map, plus B4).
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
S = 106.0  # outer size (tile outline)
S_R = 10.0  # corner radius
WALL_INSET = 1.5
WALL = 2.0
FLOOR_T = 2.0
WEDGE_DEG = math.degrees(math.atan(12.0 / 70.0))  # v1's wedge (9.7°): the deck tips towards you
FLOOR_CLR = 0.3  # lowest part (the parts on the HW-040's back) above the floor

# HW-040 + EC11 (measured on the real module). Its body bears on the carrier back and its M7 nut and washer clamp the front.
# Five straight header pins are wrapped behind the carrier; the mounted shaft projection is measured from its top face.
EC11_BODY = (12.0, 11.6)  # NOMINAL body footprint
EC11_BODY_H = 7.7  # body top (bushing base) above the board front (measured)
EC11_BUSH_D, EC11_BUSH_L = (
    7.0,
    8.5,
)  # M7 bushing: thread ends 16.2 above the board (measured)
EC11_SHAFT_D = 6.0
EC11_SHAFT_ABOVE_CARRIER = 17.0  # user measurement: carrier nut-side face to unpressed shaft tip
EC11_FLAT = 1.5  # retain v4 CAD fit: flat-to-back is 4.5 mm; its 4.8 mm note is unverified
EC11_FLAT_L = 7.1  # D-flat length from the tip (measured)
EC11_TOP_GAP = 0.0  # body against the carrier back; the M7 nut carries the press
EC11_HOLE_CLR = 0.1  # round hole round the bushing, per side: it centres the encoder
HW040 = (26.0, 18.5, 1.3)
HW040_SHAFT = (-2.3, 1.8)  # measured: shaft from board centre (+x header end, -y holes)
HW040_HOLES = [
    (-8.55, -6.9),
    (8.55, -6.9),
]  # two M3 holes, from the board centre (measured)
HW040_HOLE_D = 3.2
HW040_HDR_X = 10.25  # v4 measured row: 0.1 mm nearer the shaft than the drawing
HW040_BACK_KEEPOUT = (
    -9.0,
    9.0,
    -4.0,
    6.0,
)  # shaft-relative: EC11 legs + 3 pull-up resistors underneath
HW040_BACK_PARTS = 2.0  # how far those (and the header joints) stick out of its back
HW040_ROT = -90.0  # board turned so the header points to the front
HW040_PINS = ["CLK", "DT", "SW", "+", "GND"]  # module frame: +y to -y along the header
EC11_NUT = (9.0, 2.0, 0.5)  # M7 nut across flats, thickness, washer thickness
ENC_BOARD_FRONT = 5.6  # deck frame: board front (the whole stack is placed from here)

# Gear pair, module 1: internal teeth on the ring, pinion on the encoder shaft. The pinion's teeth
# sit low on the round part of the shaft, just above the bushing, and its tall hub grips the
# D-flat at the top: so the deck can stay low and the ring stands tall above it.
GEAR_M = 1.0
RING_TEETH, PINION_TEETH = 64, 20  # 3.2 encoder turns per ring turn
GEAR_BAND_R = 34.4  # outer radius of the toothed band
GEAR_H = 6.4  # extended ring band clears the axially stationary pinion during a press
PINION_GAP = 1.1  # rotation-only encoder: pinion retains this bushing clearance
PINION_TEETH_H = 4.2  # keep the tooth top unchanged while increasing the lower clearance
PINION_HUB_D = 9.0  # hub up the shaft, D-shaped bore over the D-flat
PINION_TIP_GAP = 0.4  # rotation-only pinion hub below the unpressed shaft tip
BACKLASH_CD = 0.35  # pinion moved this much towards the axis: tooth play for printed gears
# (an internal pair gets play from a SMALLER centre distance)
PINION_PHASE = 0.5  # pinion turned by this many tooth pitches so it meshes
SHAFT_C = (
    0.0,
    -((RING_TEETH - PINION_TEETH) * GEAR_M / 2 - BACKLASH_CD),
)  # in front of the screen

# Ring rotates on the sliding bezel; its retainer prevents lifting out of the sub-deck.
RING_R = 38.0  # outer radius above the sub-deck
RING_HOLE = 37.0  # sub-deck hole radius
RING_NECK_R = 36.8  # ring radius through the sub-deck hole
RING_BORE = 28.3  # top lip round the bezel
RING_LIP_T = 2.0
RING_BAND_R = 33.5  # inner radius between the lip and the gear teeth
RING_CHAMFER = 2.0
RING_ABOVE_CAP = 1.0  # ring top above the bezel top
FLUTES, FLUTE_D = 72, 1.4
RET_GAP = 0.2  # upward capture clearance; grows during a press
RING_PRESS_GAP = 1.2  # 0.75 mm clear of the fixed deck at the central press stop
RET_T = 1.5
RET_R = (34.6, 38.6)
RET_CLIP_ANGLES = (90.0, 210.0, 330.0)
RET_CLIP_L = 5.0
RET_CLIP_FLEX = 0.4  # clearance surrogate only; test printed insertion force and release
SUBDECK_T = 2.0

# Moving bezel carries the ring and connects to the bridge through hidden snap pillars.
CAP_R = 28.0
CAP_FACE_T = 2.0
CAP_SKIRT_T = 1.2
CAP_SKIRT_H = 3.0
CAP_FLANGE_R = 30.5  # flat annular bearing under the ring lip
WINDOW = 35.0  # flared aperture clears the sampled 20-degree view cone
WINDOW_CHAMFER = CAP_FACE_T  # flare through the entire face, avoiding a deep straight throat
DISPLAY_VIEW_ANGLE = 20.0
DISPLAY_VIEW_DEPTH = 0.5  # conservative optical-plane allowance below modeled glass front
DISPLAY_VIEW_MARGIN = 0.15  # active-circle centring allowance for the sight-line test

# Four removable folded leaf flexures suspend a bridge beneath the fixed display.
PRESS_NODES = [(18.0, 18.0), (-18.0, 18.0), (-18.0, -18.0), (18.0, -18.0)]
PRESS_ANCHORS = [(14.0, 27.0), (-27.0, 14.0), (-14.0, -27.0), (27.0, -14.0)]
FLEXURE_PATH = [(18.0, 18.0), (23.0, 18.0), (23.0, 22.0), (14.0, 22.0), (14.0, 27.0)]
FLEXURE_W, FLEXURE_T = 2.4, 0.8
FLEXURE_TOP, FLEXURE_PAD_T = 19.5, 2.0
BRIDGE_Z, BRIDGE_TOP = 29.0, 31.0
PRESS_CLICK = 0.35  # TS13 datasheet typical; calibrate against the actual fifth switch
RING_CATCH_Z, RING_CATCH_R = 32.3, 31.0
RING_CATCH_GAP = 0.18
RING_HOOK_ANGLES = (60.0, 120.0, 240.0, 300.0)
RING_HOOK_ROOT = 39.1

# Seven-pin GC9A01 module, first user drawing (38 x 45.5 PCB; not the 8-pin Surenoo).
# Connector tab rotated to +x. Rear connector pins plug into the existing two socket stack.
DISP_C = (0.0,0.0)
DISP_PCB_D, DISP_PCB_L, DISP_PCB_T = 38.0,45.5,1.6
DISP_TAB_W = 22.92
DISP_GLASS_D, DISP_GLASS_L, DISP_GLASS_T = 35.6,38.1,1.5
DISP_ACTIVE_TOP = 1.6+1.2+32.4/2  # drawing offsets from PCB top to active-area centre
DISP_HDR_X = DISP_PCB_L-1.76-DISP_ACTIVE_TOP
DISP_HOLE_X = DISP_PCB_L-5.83-DISP_ACTIVE_TOP
DISP_HOLE_D, DISP_HOLE_PITCH = 2.0,19.26
DISP_STANDOFF_D = 4.5
DISP_RESTS = [(-16.0,-9.0),(-16.0,9.0)]
DISP_FLEX_W = 14.0  # shortcut: flex-tail width is not dimensioned; verify this keepout on the module
DISP_SMD_H = 1.2  # drawing maximum component height behind PCB
DISP_FPC_RELIEF = (1.2,0.5)
BEZEL_STOP_TRAVEL = 0.45  # prototype stop: measure the central switch before approving travel
PRESS_STOPS = [(-8.0, -8.0), (8.0, -8.0), (-8.0, 8.0), (8.0, 8.0)]
PRESS_STOP_D = 3.0
DISP_SOCKET_TAIL = 6.0
DISP_GAP = 1.8  # keep at least 0.5 mm air even at the press stop
DISP_PINS = ['VCC','GND','SCL','SDA','DC','CS','RST']  # +y to -y after rotating the tab to +x
DISP_HDR_H = 2.54
SOCKET_H, SOCKETS = 8.5,2
PEG_CLR, PEG_PROUD = 0.1,1.0

# Buttons: the whole top is four quadrant tiles, split along the x and y axes, from the ring out
# to the body edge. Like a standard 12x12 switch cap, each tile snaps onto its switch: a square
# socket under the tile middle grips the switch's square tip, two of its four flexing walls hook
# under it. The switch holds the tile down and sets its height. 3 legs sliding in guide bosses
# in the sub-deck only keep the tile from tilting when pressed near a corner.
BTN_D = 44.0  # switch distance from the centre along the diagonals (tile centroid)
TILE_T = 2.8  # thicker face reduces bending under a corner press
TILE_PRESS_GAP = 1.4  # tile underside above fixed deck at rest
TILE_GAP = 0.3  # between adjacent tiles
RING_TILE_GAP = 0.6  # radial running clearance to the ring; does not set its centring fit
TILE_ROUND = 1.0  # rounding of the tile corners in plan
TILE_CHAMFER = 0.5  # top edges
TILE_SKIRT_T = 1.2  # lip over the wall top
TILE_LEGS = [
    (44.0, 14.0),
    (14.0, 44.0),
    (42.0, 35.0),
]  # separate from the case-screw pillar so the sliding guide can be longer; +x +y tile (clear of the SuperMini)
LEG_D, LEG_HOLE_D, LEG_BOSS_D = 4.0, 4.3, 7.5
CAP_FINGER = (
    5.5,
    1.6,
)  # long sides: finger width (5.5 as on a bought round 12x12 cap, past the head's
# corners), thickness; free below its rib, it
CAP_FINGER_RIB = (
    1.0,
    2.0,
)  # rib from each finger out to the cap wall: thickness, height down from the cap top
# (the root only, so the finger below it still bends)
CAP_SNAP = 0.3  # lip reach in under the head (the neck is 0.4 narrower per side)
CAP_SNAP_CLR = -0.05  # finger to head side: they press on it slightly (at 0.1 the printed fingers stood off)
CAP_SNAP_DROP = 0.4  # lip catch this much lower than drawn (measured: at 0.2 the cap was still loose on the head)
CAP_SIDE = (
    2.0,
    1.2,
    0.0,
)  # short sides (2 wide, as on the bought cap): plate width, thickness, play to the head
CAP_SIDE_SNAP = 0.2  # their lips reach this far in under the head (shorter, stiffer plates: less than
# the long plates; verify the snap on a printed sample)
CAP_SIDE_RIB = (
    1.0,
    0.5,
)  # thin rib from each short plate out to the cap wall (a "+" as on the bought cap):
# thickness, share of the plate's length from the cap top (half: the rest bends)
CAP_STOP_D = 3.0  # stop post on the head, round its centre hole
CAP_STOP_ROOT = (
    3.6,
    1.0,
)  # the post widens to this Ø over this height where it meets the cap top
SOCK_STEM = 3.4  # tile underside above the switch head
SW_HOLE_D = 9.0  # socket through the sub-deck
# 12x12 switch, 10 mm tall, snap-fit stem (same as v1: Same Sky TS13 drawing)
SW_BODY, SW_BODY_H = 11.7, 3.6  # same switch as v4, confirmed by user
SW_LEG = (0.7, 0.3, 3.5)  # v4 measured flat-leg width, thickness, length below base
SW_BOSS_D, SW_BOSS_H = 6.4, 4.5  # round platform
SW_NECK, SW_NECK_H = (
    3.0,
    1.0,
)  # stem (seller drawing): a 1 mm tall neck on the platform ...
SW_HEAD, SW_HEAD_H = 3.8, 1.7  # ... then the square head (the grip)
SW_TOTAL_H = SW_BOSS_H + SW_NECK_H + SW_HEAD_H  # head top 7.2 mm above the PCB
SW_PADS = [
    (-6.25, -2.5),
    (6.25, -2.5),
    (-6.25, 2.5),
    (6.25, 2.5),
]  # pin 1,1 (front), 2,2 (back)
SW_LEG_HOLE_D = 1.2  # v4 hole for the measured 0.7 x 0.3 mm flat legs

# ESP32-C3 SuperMini on its soldered header strips on the printed PCB, components up, in the middle
# of the back, USB-C out of the back wall through a small pill hole (as v1: a hidden pocket thins
# the wall round the receptacle, so a cable's plug reaches fully in)
SM = (18.0, 22.52, 1.0)  # NOMINAL
SM_X = 0.0  # board centre x (its long axis along y)
SM_ROW = 15.24
SM_PIN_MARGIN = (SM[1] - 7 * 2.54) / 2
SM_HDR = (
    2.54,
    8 * 2.54,
    2.5,
    6.0,
)  # strip width, length, plastic height, pin past the plastic
SM_USB = (8.94, 7.35, 3.2)  # NOMINAL
SM_USB_OVERHANG = 0.5
USB_WALL = 0.8
USB_HOLE = (9.6, 4.0)
USB_FRONT_CLR = 0.15
# Component side, USB-C away: power row on the right, IO row on the left (v4/#123).
SM_COL_A = ["5V", "GND", "3V3", "IO4", "IO3", "IO2", "IO1", "IO0"]
SM_COL_B = ["IO5", "IO6", "IO7", "IO8", "IO9", "IO10", "IO20", "IO21"]

# Printed carrier: rounded channels on the back for enamelled copper wire, as v4.
C_T = 2.0
C_INSET = 0.8
HOLE_D = 1.2
WIRE_D = 0.3  # wire diameter (stand-in)
WIRE_CH = (
    0.8,
    0.7,
)  # channel width, depth: room for two wires side by side where runs share it
WIRE_BEND_R = 1.5  # channel centre-line radius at every bend (a sharp corner kinks the wire)
WIRE_CROSS = (4.0, 1.3)  # where two runs cross, the lower channel dips: length, depth
KAPTON = (
    2.0,
    0.06,
)  # Kapton square over the lower wire at each crossing: side, thickness. It
# lies in a pocket as deep as the channel plus the tape, so the upper
# wire runs on over it at its own channel depth
WIRE_RIDGE_MIN = 0.7  # plastic between two channels, or a channel and a hole, pin row or the edge
WRAP_TURNS = 4  # turns round each header pin (1.2 mm of pin)
WRAP_OD = 1.5  # the turns round a 0.64 mm square pin, outside
PIN_ROW = (
    2.4,
    0.9,
)  # room kept clear for the turns along each wrapped header row: width, past
# the end pins (no other channel runs there)
LEG_POCKET = (
    1.4,
    3.0,
)  # pocket the folded switch leg lies in, over the wire end: width, length
BTN_GPIO = [
    "IO8",
    "IO9",
    "IO10",
    "IO20",
]  # back-left, front-left, back-right, front-right: B2 B1 B3 B4

# Assembly: M2 x 12 self-tapping screws from under the floor, up through a floor post and the
# printed PCB into a post hanging from the sub-deck: floor, printed PCB and shell clamped together.
# The posts' spigots locate the printed PCB, so the gear centre distance holds.
SCREWS = [
    (-42.0, 42.0),
    (42.0, 42.0),
    (-42.0, -42.0),
    (42.0, -42.0),
]  # independent case-screw pillars in the corners
POST_D = 6.5
# v4 floor detents: hidden wall ribs, pull-release ramps, screws still clamp the case.
CASE_HOOKS = [(side, y) for side in (-1, 1) for y in (-24.0, 24.0)]
CASE_HOOK_W, CASE_HOOK_T, CASE_HOOK_L = 8.0, 1.4, 10.0
CASE_HOOK_BARB, CASE_HOOK_GAP = 1.0, 1.2
CASE_RIB_DEPTH, CASE_RIB_H = 1.0, 1.5
SPIGOT_D, SPIGOT_HOLE_D = 3.4, 3.9
SPIGOT_INTO_FLOOR = 1.5
SPIGOT_SOCKET_CLR = 0.3
SCREW_PILOT, SCREW_CLR, SCREW_HEAD_D, SCREW_L = 1.6, 2.4, 4.4, 12.0
SCREW_IN = 5.8  # pilot depth into the corner post (the tile leg's hole is above it)
DOT_D, DOT_T = 10.0, 1.5
DOT_PEG_D, DOT_SOCKET_D, DOT_PEG_L = 4.4, 4.3, 1.0
RST_HOLE_D, RST_GUIDE_D, RST_GUIDE_BORE = 1.0, 3.0, 1.4
SM_BTN_FROM_USB, SM_BTN_DX = (
    9.5,
    4.5,
)  # NOMINAL: check RST position on the actual SuperMini
SM_BTN = (3.0, 4.0, 1.5)

FONT = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
TEXT_D = 0.4
DOC_NAME = "KivoriCaseExperiment"
EXPORT = True

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


def place(shape, cx, cy, ang=0.0):
    s = shape.copy()
    if ang:
        s.rotate(V(0, 0, 0), V(0, 0, 1), ang)
    s.translate(V(cx, cy, 0))
    return s


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


# ---------------------------------------------------------------------------------------------
# Derived layout (deck frame)
# ---------------------------------------------------------------------------------------------

BODY, BODY_R = S - 2 * WALL_INSET, S_R - WALL_INSET  # walls and floor
INNER = BODY / 2 - WALL  # inner wall face

# the stack, from the encoder up
enc_board_front = ENC_BOARD_FRONT
enc_board_back = enc_board_front - HW040[2]
enc_body_top = enc_board_front + EC11_BODY_H
bush_top = enc_body_top + EC11_BUSH_L
C_BOT = enc_body_top + EC11_TOP_GAP  # carrier underside (wire channels)
C_TOP = C_BOT + C_T
enc_shaft_top = C_TOP + EC11_SHAFT_ABOVE_CARRIER
GEAR_Z = (bush_top, bush_top + GEAR_H)  # ring teeth band
PINION_Z = (
    bush_top + PINION_GAP,
    bush_top + PINION_GAP + PINION_TEETH_H,
)  # pinion teeth
PINION_TOP = enc_shaft_top - PINION_TIP_GAP
sw_base = C_TOP
SW_PLUNGER_TOP = sw_base + SW_TOTAL_H  # switch tip top = socket ceiling
DECK_TOP = SW_PLUNGER_TOP + SOCK_STEM + TILE_T  # top face of the tiles
tile_under = DECK_TOP - TILE_T
SUBDECK_TOP = tile_under - TILE_PRESS_GAP
subdeck_under = SUBDECK_TOP - SUBDECK_T
ret_top = subdeck_under - RET_GAP
TILE_SKIRT_Z = tile_under - 1.6  # lip depth independent of the thicker tile face
LEG_BOTTOM = tile_under - 9.0  # 7.6 mm guide engagement without reaching the carrier

# Display on its sockets; DISP_GAP leaves air under the moving bezel.
dpcb_c = DISP_C
glass_c = (1.6+DISP_GLASS_D/2-DISP_ACTIVE_TOP,0.0)
socket_top = C_TOP + SOCKETS * SOCKET_H
dpcb_z0 = socket_top + DISP_HDR_H  # board back: its pins fully in the sockets
dpcb_z1 = dpcb_z0 + DISP_PCB_T
glass_z0, glass_z1 = dpcb_z1, dpcb_z1 + DISP_GLASS_T
cap_under = glass_z1 + DISP_GAP
CAP_TOP = cap_under + CAP_FACE_T
CAP_BOTTOM = cap_under - CAP_SKIRT_H
RING_TOP = CAP_TOP + RING_ABOVE_CAP
RING_BEARING_Z = RING_TOP - RING_LIP_T
FLUTE_Z = (DECK_TOP + 1.0, RING_TOP - 3.0)
disp_hdr_x = DISP_HDR_X
disp_pins = {n:(disp_hdr_x,(3-i)*2.54) for i,n in enumerate(DISP_PINS)}
disp_holes = [(DISP_HOLE_X,sy*DISP_HOLE_PITCH/2) for sy in (-1,1)]

c_btn = BTN_D / math.sqrt(2)
BTNS = [(sx * c_btn, sy * c_btn) for (sx, sy) in ((-1, 1), (1, 1), (-1, -1), (1, -1))]


def btn_pin(i, k):
    return (BTNS[i][0] + SW_PADS[k][0], BTNS[i][1] + SW_PADS[k][1])


def switch_pins():
    return [btn_pin(i, k) for i in range(4) for k in range(4)] + list(SW_PADS)


SM_USB_END = BODY / 2 - USB_WALL - USB_FRONT_CLR - SM_USB_OVERHANG  # board end, in the back wall pocket
sm_c = (SM_X, SM_USB_END - SM[1] / 2)
sm_lo = C_TOP + SM_HDR[2]  # board underside, on its header strips
sm_hi = sm_lo + SM[2]
rst_xy = (SM_X + SM_BTN_DX, SM_USB_END - SM_BTN_FROM_USB)
rst_top = sm_hi + SM_BTN[2]
usb_zc = sm_hi + SM_USB[2] / 2
sm_rows = (SM_X + SM_ROW / 2, SM_X - SM_ROW / 2)
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


_bc = (-HW040_SHAFT[0], -HW040_SHAFT[1])  # board centre relative to the shaft
_hdr_local = _bc[0] + HW040_HDR_X  # header row from the shaft, module x
enc_pins = {n: hw_xy((_hdr_local, (2 - i) * 2.54)) for i, n in enumerate(HW040_PINS)}


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
    hole = Part.Face(Part.Wire(Part.makeCircle(RING_R + RING_TILE_GAP)))
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
# Wire routing
# ---------------------------------------------------------------------------------------------


def wire_layout():
    """Pin-to-pin runs matching v4/main firmware, plus B4 on the free GPIO20."""
    sm, dp, ep, leg = sm_pins, disp_pins, enc_pins, btn_pin
    pw, io = sm_rows
    ye = ep["SW"][1]
    west = [-20.32, -17.78, -15.24, -11.5]
    bottom = [ye - 2.8 - 1.6 * k for k in range(4)]
    edge = 44.1
    runs = [
        ("GND", [sm["GND"], (18.5, sm["GND"][1]), (18.5, dp["GND"][1]), dp["GND"]]),
        ("GND", [sm["GND"], (20.0, sm["GND"][1]), (20.0, 38.0), (edge, 38.0), (edge, leg(1, 1)[1]), leg(1, 1)]),
        ("GND", [leg(1, 0), (leg(1, 0)[0], 21.5), (edge, 21.5), (edge, leg(3, 1)[1]), leg(3, 1)]),
        ("GND", [leg(3, 0), (leg(3, 0)[0], -edge), (leg(2, 1)[0], -edge), leg(2, 1)]),
        ("GND", [leg(2, 0), (-edge, leg(2, 0)[1]), (-edge, leg(0, 0)[1]), leg(0, 0)]),
        ("GND", [leg(0, 1), (leg(0, 1)[0], -25.3), (ep["GND"][0], -25.3), ep["GND"]]),
        (
            "3V3",
            [
                sm["3V3"],
                (5.5, sm["3V3"][1]),
                (5.5, 9.1),
                (-9.5, 9.1),
                (-9.5, bottom[0]),
                (ep["+"][0], bottom[0]),
                ep["+"],
            ],
        ),
        (
            "3V3",
            [
                ep["+"],
                (ep["+"][0], ye + 3.67),
                (7.62, ye + 3.67),
                (7.62, leg(3, 0)[1] - 4.4),
                (34.0, leg(3, 0)[1] - 4.4),
                (34.0, dp["VCC"][1]),
                dp["VCC"],
            ],
        ),
    ]
    for k, (net, pin) in enumerate([("SW", "IO5"), ("DT", "IO6"), ("CLK", "IO7")]):
        start = (
            [sm[pin], (io, sm[pin][1] - 0.68), (west[k], sm[pin][1] - 0.68)]
            if net == "SW"
            else [sm[pin], (west[k], sm[pin][1])]
        )
        if net == "SW":
            runs.append((net, start + [(west[k], 6.5), (-3.0, 6.5), (-3.0, 2.5), SW_PADS[2]]))
        else:
            runs.append((net, start + [(west[k], bottom[k + 1]), (ep[net][0], bottom[k + 1]), ep[net]]))
    runs.append(("GND", [ep["GND"], (ep["GND"][0], -2.5), SW_PADS[0]]))
    runs += [
        ("B1", [sm["IO9"], (west[3], sm["IO9"][1]), (west[3], leg(2, 3)[1]), leg(2, 3)]),
        ("B2", [sm["IO8"], (leg(0, 3)[0], sm["IO8"][1]), leg(0, 3)]),
        (
            "B3",
            [sm["IO10"], (0.0, sm["IO10"][1]), (0.0, 24.4), (BTNS[1][0], 24.4), (BTNS[1][0], leg(1, 2)[1]), leg(1, 2)],
        ),
        (
            "B4",
            [
                sm["IO20"],
                (-2.54, sm["IO20"][1]),
                (-2.54, 14.1),
                (BTNS[3][0], 14.1),
                (BTNS[3][0], leg(3, 2)[1]),
                leg(3, 2),
            ],
        ),
    ]
    # The GC9A01 header is farther right, leaving room for five widely spaced bus lanes.
    for k, (net, pin) in enumerate([("SCL", "IO4"), ("SDA", "IO3"), ("RST", "IO2"), ("DC", "IO1"), ("CS", "IO0")]):
        x = 20.5 - 3.5 * k if net != "CS" else 3.0
        runs.append((net, [sm[pin], (x, sm[pin][1]), (x, dp[net][1]), dp[net]]))
    # Tile wires stay below the display bus at every crossing, avoiding alternating tape layers.
    return sorted(runs,key=lambda run:run[0] in ('B3','B4'))


def wire_end_counts():
    counts = {}
    for _, points in wire_layout():
        for x, y in (points[0], points[-1]):
            key = (round(x, 4), round(y, 4))
            counts[key] = counts.get(key, 0) + 1
    return counts


def header_rows():
    return [(x, SM_USB_END - SM_PIN_MARGIN - 3.5 * 2.54, 8) for x in sm_rows] + [(disp_hdr_x, DISP_C[1], 7)]


def row_len(n):
    return (n - 1) * 2.54 + 2 * PIN_ROW[1]


def leg_ends():
    """Switch legs a run ends on: (leg, the unit direction the run arrives from)."""
    out = []
    for _, pts in wire_layout():
        for a, b in ((pts[0], pts[1]), (pts[-1], pts[-2])):
            for point in switch_pins():
                if math.hypot(a[0] - point[0], a[1] - point[1]) < 1e-6:
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
        u.normalize()
        v.normalize()
        th = math.acos(max(-1.0, min(1.0, u.dot(v))))
        if th < 1e-6:
            continue
        t = min(
            r * math.tan(th / 2),
            lu / 2 if i > 1 else lu,
            lv / 2 if i < len(P) - 2 else lv,
        )
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
        f = Part.Face(
            Part.makePolygon(
                [
                    V(0, -w / 2, z),
                    V(L, -w / 2, z),
                    V(L, w / 2, z),
                    V(0, w / 2, z),
                    V(0, -w / 2, z),
                ]
            )
        )
        f = f.fuse([Part.Face(Part.Wire(Part.makeCircle(w / 2, V(t, 0, z)))) for t in (0, L)]).removeSplitter().Faces[0]
        f.rotate(V(0, 0, z), V(0, 0, 1), math.degrees(math.atan2(by - ay, bx - ax)))
        f.translate(V(ax, ay, 0))
        return f
    return Part.Face(Part.Wire(run_edges(pts, z)).makeOffset2D(w / 2, 0, False, False))


def band(pts, w, z0, z1):
    return band_face(pts, w, z0).extrude(V(0, 0, z1 - z0))


def wire_crossings():
    """Where two wire runs cross: (x, y, run on top, run that dips, its direction)."""
    runs = wire_layout()
    segs = [(k, a, b) for k, (_, pts) in enumerate(runs) for a, b in zip(pts, pts[1:])]
    out = []
    for i, (ka, a0, a1) in enumerate(segs):
        for kb, b0, b1 in segs[i + 1 :]:
            if ka == kb:
                continue
            d = (a1[0] - a0[0]) * (b1[1] - b0[1]) - (a1[1] - a0[1]) * (b1[0] - b0[0])
            if abs(d) < 1e-9:
                continue
            s = ((b0[0] - a0[0]) * (b1[1] - b0[1]) - (b0[1] - a0[1]) * (b1[0] - b0[0])) / d
            t = ((b0[0] - a0[0]) * (a1[1] - a0[1]) - (b0[1] - a0[1]) * (a1[0] - a0[0])) / d
            if 1e-6 < s < 1 - 1e-6 and 1e-6 < t < 1 - 1e-6:  # the later run dips under the earlier
                L = math.hypot(b1[0] - b0[0], b1[1] - b0[1])
                out.append(
                    (
                        a0[0] + s * (a1[0] - a0[0]),
                        a0[1] + s * (a1[1] - a0[1]),
                        ka,
                        kb,
                        ((b1[0] - b0[0]) / L, (b1[1] - b0[1]) / L),
                    )
                )
    return out


def wire_cuts():
    """Everything cut into the printed PCB's back for the wiring: the channels, the dips under the
    crossings and a pocket at each switch leg the wire ends on."""
    z0, w, dep = C_BOT - 1, WIRE_CH[0], WIRE_CH[1]
    cuts = [band(pts, w, z0, C_BOT + dep) for _, pts in wire_layout()]
    for x, y, _, _, (dx, dy) in wire_crossings():
        L = WIRE_CROSS[0]
        cuts.append(
            place(
                box(-L / 2, -w / 2, z0, L / 2, w / 2, C_BOT + WIRE_CROSS[1]),
                x,
                y,
                math.degrees(math.atan2(dy, dx)),
            )
        )
    cuts += kapton_squares(z0, C_BOT + dep + KAPTON[1])  # a flat seat for the Kapton square
    for (x, y), (dx, dy) in leg_ends():  # the leg folds back along the arriving wire
        a = math.degrees(math.atan2(dy, dx))
        cuts.append(
            place(
                box(
                    0,
                    -LEG_POCKET[0] / 2,
                    z0,
                    LEG_POCKET[1],
                    LEG_POCKET[0] / 2,
                    C_BOT + dep,
                ),
                x,
                y,
                a,
            )
        )
    return cuts


def check_wires():
    """Narrowest plastic left between a channel and anything that is not its own: another net's
    channel (where two runs cross, that pair is left out), a hole, pin row or leg pocket it does not
    end in, the board edge. Returns [(mm, net, what)], smallest first."""
    runs = wire_layout()
    zt = C_BOT
    chans = [band_face(pts, WIRE_CH[0], zt) for _, pts in runs]
    crossed = {(min(a, b), max(a, b)) for (_, _, a, b, _) in wire_crossings()}
    holes = [((x, y), HOLE_D) for d in (sm_pins, disp_pins, enc_pins) for (x, y) in d.values()]
    holes += [(p, SW_LEG_HOLE_D) for p in switch_pins()]
    holes += [(p, SPIGOT_HOLE_D) for p in SCREWS] + [(SHAFT_C, EC11_BUSH_D + 2 * EC11_HOLE_CLR)]
    pockets = [
        (
            p,
            place(
                Part.Face(
                    Part.makePolygon(
                        [
                            V(0, -LEG_POCKET[0] / 2, zt),
                            V(LEG_POCKET[1], -LEG_POCKET[0] / 2, zt),
                            V(LEG_POCKET[1], LEG_POCKET[0] / 2, zt),
                            V(0, LEG_POCKET[0] / 2, zt),
                            V(0, -LEG_POCKET[0] / 2, zt),
                        ]
                    )
                ),
                p[0],
                p[1],
                math.degrees(math.atan2(d[1], d[0])),
            ),
        )
        for (p, d) in leg_ends()
    ]
    board = carrier_outline()
    board = [f for f in board.Faces if abs(f.normalAt(0, 0).z + 1) < 1e-6][0]
    out = []
    for k, ((net, pts), ch) in enumerate(zip(runs, chans)):
        ends = [pts[0], pts[-1]]
        own = lambda p: any(math.hypot(p[0] - e[0], p[1] - e[1]) < 1e-6 for e in ends)
        for j in range(k + 1, len(runs)):
            if runs[j][0] != net and (k, j) not in crossed:
                out.append((ch.distToShape(chans[j])[0], net, "channel " + runs[j][0]))
        for p, dia in holes:
            if not own(p):
                out.append(
                    (
                        ch.distToShape(Part.Vertex(V(p[0], p[1], zt)))[0] - dia / 2,
                        net,
                        "hole (%.1f, %.1f)" % p,
                    )
                )
        mine = []
        for x, y, n in header_rows():
            tr = Part.Face(
                Part.makePolygon(
                    [
                        V(x + sx * PIN_ROW[0] / 2, y + sy * row_len(n) / 2, zt)
                        for (sx, sy) in ((-1, -1), (1, -1), (1, 1), (-1, 1), (-1, -1))
                    ]
                )
            )
            if any(abs(e[0] - x) < 1e-6 and abs(e[1] - y) <= n * 1.27 for e in ends):
                mine.append(tr)
            else:
                out.append((ch.distToShape(tr)[0], net, "pin row x %.1f" % x))
        for p, pk in pockets:
            if not own(p):
                out.append((ch.distToShape(pk)[0], net, "leg pocket (%.1f, %.1f)" % p))
        ch = ch.cut(mine) if mine else ch  # (a pin row may run out of the board edge)
        out.append(
            (
                ch.distToShape(board.OuterWire)[0] if ch.cut(board).Area < 1e-3 else 0.0,
                net,
                "board edge",
            )
        )
    return sorted(out, key=lambda t: t[0])


# ---------------------------------------------------------------------------------------------
# Parts (deck frame, except the floor)
# ---------------------------------------------------------------------------------------------


def carrier_outline():
    w, r = 2 * (INNER - C_INSET), BODY_R - WALL - C_INSET
    c = rr_prism(w, w, r, C_BOT, C_TOP)
    tab = box(SM_X - SM[0] / 2 - 0.5, INNER - C_INSET - 1, C_BOT, SM_X + SM[0] / 2 + 0.5, INNER - 0.4, C_TOP)
    return c.fuse(tab).removeSplitter()


def carrier_through_cuts():
    z0, z1 = C_BOT - 1, C_TOP + 1
    cuts = [cyl(EC11_BUSH_D + 2 * EC11_HOLE_CLR, *SHAFT_C, z0, z1)]
    cuts += [cyl(HOLE_D, x, y, z0, z1) for row in (sm_pins, disp_pins, enc_pins) for x, y in row.values()]
    cuts += [cyl(SW_LEG_HOLE_D, *p, z0, z1) for p in switch_pins()]
    cuts += [cyl(SPIGOT_HOLE_D, x, y, z0, z1) for x, y in SCREWS]
    return cuts


def peg(x, y, hole_d, z0, z1, clr=PEG_CLR):
    """Locating peg with a 0.3 mm lead-in chamfer at the tip."""
    d = hole_d - 2 * clr
    body = cyl(d, x, y, z0, z1 - 0.3)
    tip = Part.makeCone(d / 2, d / 2 - 0.3, 0.3, V(x, y, z1 - 0.3), V(0, 0, 1))
    return body.fuse(tip)


def case_hook_geometry(y):
    """v4 left hook in the deck frame; its root extends into the horizontal floor."""
    floor_z = (FLOOR_T - TILT.A34 - TILT.A32 * (y + CASE_HOOK_W / 2)) / TILT.A33
    z0, xa = floor_z - 0.5, -INNER + CASE_HOOK_GAP
    zt = z0 + CASE_HOOK_L
    b = CASE_HOOK_BARB
    barb = [(xa, zt - 2 * b), (xa - b, zt - b), (xa, zt)]
    xr = -INNER + CASE_RIB_DEPTH
    rib_top = zt - 2 * b + xa - xr - 0.05
    rib = [(-INNER - 0.8, rib_top - CASE_RIB_H), (xr, rib_top - CASE_RIB_H),
           (xr, rib_top), (-INNER - 0.8, rib_top + xr + INNER + 0.8)]
    return z0, zt, xa, barb, rib


def case_latch(side, y):
    """Return the floor hook and shell catch, mirrored from the v4 left-side profiles."""
    z0, zt, xa, barb, rib = case_hook_geometry(y)
    def profile(points, width):
        face = Part.Face(Part.makePolygon([V(x, y - width / 2, z) for x, z in points + [points[0]]]))
        return face.extrude(V(0, width, 0))
    hook = box(xa, y - CASE_HOOK_W / 2, z0, xa + CASE_HOOK_T, y + CASE_HOOK_W / 2, zt)
    hook = hook.fuse(profile(barb, CASE_HOOK_W))
    catch = profile(rib, CASE_HOOK_W + 0.8)
    if side > 0:
        hook = hook.mirror(V(0, 0, 0), V(1, 0, 0))
        catch = catch.mirror(V(0, 0, 0), V(1, 0, 0))
    return hook, catch


def make_shell():
    """Walls and sub-deck, printed sub-deck down. The walls run on down to the floor (cut there
    once the assembly is tipped)."""
    z_low = -30.0
    s = rr_prism(BODY, BODY, BODY_R, z_low, SUBDECK_TOP)
    s = s.cut(rr_prism(BODY - 2 * WALL, BODY - 2 * WALL, BODY_R - WALL, z_low - 1, subdeck_under))
    adds = []
    for legs in LEGS:  # guide bosses for the tile legs
        adds += [cyl(LEG_BOSS_D, x, y, LEG_BOTTOM, subdeck_under + 0.01) for (x, y) in legs]
    for (
        x,
        y,
    ) in SCREWS:  # independent case posts; their spigots locate the printed PCB
        adds.append(cyl(LEG_BOSS_D, x, y, C_TOP, subdeck_under + 0.01))
        adds.append(cyl(SPIGOT_D, x, y, C_BOT - SPIGOT_INTO_FLOOR, C_TOP + 0.01))
    rx, ry = rst_xy
    adds.append(
        cyl(RST_GUIDE_D, rx, ry, rst_top + 0.6, subdeck_under + 0.01).cut(
            cyl(RST_GUIDE_BORE, rx, ry, rst_top, subdeck_under + 1)
        )
    )
    adds += [case_latch(side, y)[1] for side, y in CASE_HOOKS]
    s = s.fuse(adds)
    cuts = [cyl(2 * RING_HOLE, 0, 0, subdeck_under - 1, SUBDECK_TOP + 1)]
    cuts += [cyl(SW_HOLE_D, x, y, subdeck_under - 1, SUBDECK_TOP + 1) for (x, y) in BTNS]
    for legs in LEGS:
        cuts += [cyl(LEG_HOLE_D, x, y, LEG_BOTTOM - 1, SUBDECK_TOP + 1) for (x, y) in legs]
    cuts += [cyl(SCREW_PILOT, x, y, C_BOT, C_BOT + 0.2 + SCREW_IN) for (x, y) in SCREWS]
    # Open-bottom relief lets the shell pass the SuperMini board end during assembly.
    yw = BODY / 2 - USB_WALL
    cuts.append(
        box(
            SM_X - SM[0] / 2 - 0.3,
            INNER - 1,
            z_low - 1,
            SM_X + SM[0] / 2 + 0.3,
            yw,
            sm_hi + SM_USB[2] + 0.3,
        )
    )
    pill = Part.Face(rr_wire(USB_HOLE[0], USB_HOLE[1], USB_HOLE[1] / 2)).extrude(V(0, 0, 3))
    pill.rotate(V(0, 0, 0), V(1, 0, 0), -90)
    pill.translate(V(SM_X, yw - 1, usb_zc))
    cuts.append(pill)
    cuts.append(cyl(RST_HOLE_D, *rst_xy, subdeck_under - 1, SUBDECK_TOP + 1))
    return s.cut(cuts).removeSplitter()


def make_ring():
    above = ring_solid(GEAR_BAND_R - 0.4, RING_R, SUBDECK_TOP + RING_PRESS_GAP, GEAR_Z[1]).fuse(
        ring_solid(RING_BAND_R, RING_R, GEAR_Z[1], RING_TOP))
    try:
        top = [e for e in above.Edges if e.Curve.TypeId == "Part::GeomCircle"
               and abs(e.BoundBox.ZMin - RING_TOP) < 1e-6 and e.Curve.Radius > RING_R - 0.1]
        above = above.makeChamfer(RING_CHAMFER, top)
    except Exception as exc:
        print("ring chamfer skipped:", exc)
    lip = ring_solid(RING_BORE, RING_BAND_R + 0.5, RING_TOP - RING_LIP_T, RING_TOP)
    neck = ring_solid(GEAR_BAND_R - 0.4, RING_NECK_R, ret_top, SUBDECK_TOP + RING_PRESS_GAP + 0.01)
    hole = gear_face(RING_TEETH, True).extrude(V(0, 0, GEAR_Z[1] - GEAR_Z[0] + 2))
    hole.translate(V(0, 0, GEAR_Z[0] - 1))
    teeth = cyl(2 * GEAR_BAND_R, 0, 0, GEAR_Z[0], GEAR_Z[1]).cut(hole)
    # a 45° shoulder from the band down onto the tooth tips, so the teeth print on it (top down)
    tip = RING_TEETH * GEAR_M / 2 - GEAR_M
    shoulder = revolve([(tip, GEAR_Z[1] - 0.01), (RING_BAND_R + 0.01, GEAR_Z[1] - 0.01),
                        (RING_BAND_R + 0.01, GEAR_Z[1] + RING_BAND_R - tip)])
    # The 45-degree web prints from the ring face down to a concealed flat capture ledge.
    catch = revolve([(RING_BAND_R - 0.01, 33.9), (31.9, RING_CATCH_Z),
                     (RING_CATCH_R, RING_CATCH_Z), (RING_CATCH_R, RING_CATCH_Z - 1.2),
                     (RING_BAND_R + 0.01, RING_CATCH_Z - 1.2)])
    r = fuse_all([above, lip, neck, teeth, shoulder, catch])
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
    for angle in RET_CLIP_ANGLES:
        cuts += [place(box(34.6, -2.4, ret_top - 0.1, 36.05, 2.4, ret_top + RET_CLIP_L), 0, 0, angle),
                 place(box(34.6, -2.4, ret_top + RET_CLIP_L,
                           36.6, 2.4, ret_top + RET_CLIP_L + 1.2), 0, 0, angle),
                 place(box(35.8, -0.6, ret_top - 0.1, 37.0, 0.6, ret_top + 3.0), 0, 0, angle)]
    return r.cut(cuts).removeSplitter()


def retainer_clips(squeezed=False):
    """Upward tongues catch flat pocket floors; no part projects below the retainer."""
    lip = ret_top + RET_CLIP_L
    profile = [(35.1, ret_top - 0.1), (35.9, ret_top - 0.1),
               (35.9, lip), (36.35, lip), (36.35, lip + 0.1),
               (35.8, lip + 0.9), (35.1, lip + 0.9)]
    face = Part.Face(Part.makePolygon([V(r, -2.0, z) for r, z in profile + [profile[0]]]))
    finger = face.extrude(V(0, 4.0, 0))
    if squeezed:
        finger.translate(V(-RET_CLIP_FLEX, 0, 0))
    return fuse_all([place(finger, 0, 0, angle) for angle in RET_CLIP_ANGLES])


def make_retainer():
    plate = ring_solid(RET_R[0], RET_R[1], ret_top - RET_T, ret_top)
    # Tool slots expose the outer side of each tongue from below; keep both root side webs.
    slots = [place(box(36.0, -0.6, ret_top - RET_T - 0.1, RET_R[1] + 1, 0.6, ret_top),
                   0, 0, angle) for angle in RET_CLIP_ANGLES]
    return plate.fuse(retainer_clips()).cut(slots).removeSplitter()


def make_floor():
    """Flat on the desk, its edge under the (tipped) walls; four posts under the printed PCB with
    the screws and locating sockets, plus four v4-style detent hooks. The encoder is carrier-mounted."""
    slab = box(-80, -80, 0, 80, 80, FLOOR_T)
    p = tilted(rr_prism(BODY, BODY, BODY_R, -40, 40)).common(slab)
    above = box(-80, -80, 0, 80, 80, 80)
    adds = [tilted(cyl(POST_D, x, y, -30, C_BOT)) for (x, y) in SCREWS]
    adds += [tilted(case_latch(side, y)[0]) for side, y in CASE_HOOKS]
    p = p.fuse([a.common(above) for a in adds])
    seat = C_BOT + 0.2 + SCREW_IN - 0.5 - SCREW_L  # every screw head seats here (deck frame, along the screw)
    cuts = []
    for x, y in SCREWS:
        cuts.append(tilted(cyl(SCREW_CLR, x, y, -40, C_BOT + 1)))
        cuts.append(tilted(cyl(SCREW_HEAD_D, x, y, -40, seat)))
        cuts.append(
            tilted(
                Part.makeCone(
                    SCREW_HEAD_D / 2 + 0.01,
                    SCREW_CLR / 2,
                    1.6,
                    V(x, y, seat - 0.01),
                    V(0, 0, 1),
                )
            )
        )
    for x, y in SCREWS:
        cuts.append(
            tilted(
                cyl(
                    SPIGOT_HOLE_D,
                    x,
                    y,
                    C_BOT - SPIGOT_INTO_FLOOR - SPIGOT_SOCKET_CLR,
                    C_BOT + 1,
                )
            )
        )
    for x, y in dot_positions():
        cuts.append(cyl(DOT_SOCKET_D, x, y, -1, DOT_PEG_L + 0.2))
    t = text_solid("KIVORI", 0.0, 0.0, 7.0, -0.1, TEXT_D + 0.1, mirror=True)
    if t:
        cuts.append(t)
    return p.cut(cuts).removeSplitter()


def dot_positions():
    a = math.radians(WEDGE_DEG)
    lift = TILT.A34
    return [(x, y / math.cos(a) + lift * math.tan(a)) for x, y in SCREWS]


def make_rubber_dots():
    return Part.makeCompound(
        [cyl(DOT_D, x, y, -DOT_T, 0).fuse(cyl(DOT_PEG_D, x, y, -0.01, DOT_PEG_L)) for x, y in dot_positions()]
    )



def split_snap_peg(x, y, root, lip, stem=2.2, head=3.0, slot=0.8):
    """Split retaining pillar; a separate seating shoulder carries the pressing load."""
    direction = 1 if lip > root else -1
    tip = lip + direction * 0.8
    peg = cyl(stem, x, y, min(root, lip) - 0.01, max(root, lip) + 0.01)
    barb = Part.makeCone(head / 2, (stem - 0.4) / 2, 0.8,
                         V(x, y, lip), V(0, 0, direction))
    barb = barb.common(box(x - head, y - stem * 0.3, min(lip, tip) - 0.1,
                           x + head, y + stem * 0.3, max(lip, tip) + 0.1))
    peg = peg.fuse(barb)
    z0, z1 = sorted((root + direction * 0.5, tip + direction * 0.1))
    return peg.cut(box(x - slot / 2, y - head, z0, x + slot / 2, y + head, z1))


def joint_pegs(joint, compressed=False):
    """Assembly clearance surrogate: translate the split halves 0.4 mm; no elastic prediction."""
    if joint == "carrier":
        points, root, lip, stem, head, slot = PRESS_ANCHORS, C_TOP, FLEXURE_TOP, 2.2, 3.0, 0.8
    elif joint == "bridge":
        points, root, lip, stem, head, slot = PRESS_NODES, 22.3, FLEXURE_TOP - FLEXURE_PAD_T, 2.2, 3.0, 0.8
    else:
        assert joint == "cap"
        points, root, lip, stem, head, slot = PRESS_NODES, 34.0, 27.0, 3.0, 4.2, 1.2
    shapes = []
    for x, y in points:
        peg = split_snap_peg(x, y, root, lip, stem, head, slot)
        if compressed:
            for side in (-1, 1):
                half = peg.common(box(x - 5 if side < 0 else x, y - 5, 10,
                                      x if side < 0 else x + 5, y + 5, 45))
                half.translate(V(-side * 0.4, 0, 0))
                shapes.append(half)
        else:
            shapes.append(peg)
    return fuse_all(shapes)


def squeezed_ring_hooks():
    shapes = []
    hooks = cap_ring_hooks()
    for angle in RING_HOOK_ANGLES:
        hook = hooks.common(place(box(28, -2, 31, 33, 2, 40), 0, 0, angle))
        dx, dy = polar(0.7, angle)
        hook.translate(V(-dx, -dy, 0))
        shapes.append(hook)
    return fuse_all(shapes)


def cap_ring_hooks():
    """Four concealed downward tongues; inward deflection releases the ring's continuous ledge."""
    lip = RING_CATCH_Z + RING_CATCH_GAP
    profile = [(29.1, RING_HOOK_ROOT + 0.1), (29.9, RING_HOOK_ROOT + 0.1),
               (29.9, lip + 1.6), (31.4, lip + 0.1), (31.4, lip), (29.1, lip)]
    face = Part.Face(Part.makePolygon([V(r, -1.6, z) for r, z in profile + [profile[0]]]))
    tongue = face.extrude(V(0, 3.2, 0))
    return fuse_all([place(tongue, 0, 0, angle) for angle in RING_HOOK_ANGLES])


def make_carrier():
    """Fixed display, central click switch, suspension anchors and centred crash stops."""
    c = carrier_outline().cut(carrier_through_cuts()).cut(wire_cuts())
    adds = []
    for x, y in disp_holes:
        adds.append(cyl(DISP_STANDOFF_D, x, y, C_TOP - 0.01, dpcb_z0))
        adds.append(peg(x, y, DISP_HOLE_D, dpcb_z0 - 0.01, dpcb_z1 + PEG_PROUD))
    for x, y in DISP_RESTS:
        adds.append(cyl(3.0, x, y, C_TOP - 0.01, dpcb_z0))
    for x, y in PRESS_ANCHORS:
        post = cyl(5.0, x, y, C_TOP - 0.01, FLEXURE_TOP - FLEXURE_PAD_T)
        post = post.cut(cyl(3.6, x, y, C_TOP + 0.5, FLEXURE_TOP))
        adds.append(post)
    adds.append(joint_pegs("carrier"))
    for x, y in PRESS_STOPS:
        adds.append(cyl(PRESS_STOP_D, x, y, C_TOP - 0.01, BRIDGE_Z - BEZEL_STOP_TRAVEL))
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
    """Clean-faced moving bezel with concealed bridge pegs and integral ring hooks."""
    face = cyl(2 * CAP_R, 0, 0, cap_under, CAP_TOP)
    try:
        face = face.makeChamfer(0.6, [e for e in face.Edges if abs(e.BoundBox.ZMin - CAP_TOP) < 1e-6])
    except Exception as exc:
        print("cap chamfer skipped:", exc)
    skirt = ring_solid(CAP_R - CAP_SKIRT_T, CAP_R, CAP_BOTTOM, cap_under + 0.01)
    # Flat bearing top, 45-degree underside: prints face down without a cantilevered shelf.
    rf = CAP_FLANGE_R
    flange = revolve([
        (CAP_R - 0.5, RING_BEARING_Z), (rf, RING_BEARING_Z),
        (rf, RING_BEARING_Z - 1.0),
        (CAP_R - 0.5, RING_BEARING_Z - 1.0 - (rf - CAP_R + 0.5)),
    ])
    columns = [cyl(6.0, x, y, BRIDGE_TOP, cap_under + 0.01) for x, y in PRESS_NODES]
    c = face.fuse([skirt, flange] + columns)
    for x, y in PRESS_NODES:
        c = c.cut(cyl(4.7, x, y, BRIDGE_TOP - 0.1, 33.5))
    c = c.fuse(joint_pegs("cap"))
    # Clearance on both sides and behind each free tongue, below its root.
    for angle in RING_HOOK_ANGLES:
        c = c.cut(place(box(27.8, -2.0, 30.5, 30.4, 2.0, RING_HOOK_ROOT), 0, 0, angle))
    c = c.fuse(cap_ring_hooks())
    cuts = [cyl(WINDOW, 0, 0, cap_under - 1, CAP_TOP + 1)]
    cuts.append(Part.makeCone(WINDOW / 2, WINDOW / 2 + WINDOW_CHAMFER,
                             WINDOW_CHAMFER + 0.01, V(0, 0, CAP_TOP - WINDOW_CHAMFER)))
    cuts.append(box(19.0, -DISP_TAB_W / 2 - 0.6, CAP_BOTTOM - 2,
                    CAP_FLANGE_R + 1, DISP_TAB_W / 2 + 0.6, cap_under + DISP_FPC_RELIEF[0]))
    return c.cut(cuts).removeSplitter()


def make_press_bridge():
    """Top-down printed X bridge; the central nub operates only the separate tactile switch."""
    arms = [band([(-18.0, -18.0), (18.0, 18.0)], 5.0, BRIDGE_Z, BRIDGE_TOP),
            band([(-18.0, 18.0), (18.0, -18.0)], 5.0, BRIDGE_Z, BRIDGE_TOP)]
    for x, y in PRESS_NODES:
        arms.append(cyl(6.0, x, y, FLEXURE_TOP, BRIDGE_TOP))
    arms.append(cyl(3.0, 0, 0, SW_PLUNGER_TOP, BRIDGE_Z + 0.01))
    bridge = fuse_all(arms)
    cuts = []
    for x, y in PRESS_NODES:
        cuts += [cyl(3.8, x, y, 26.0, BRIDGE_TOP + 1),
                 cyl(4.7, x, y, 26.0, 27.0),
                 box(x - 4, y - 0.8, 26.0, x + 4, y + 0.8, 27.0),
                 cyl(3.6, x, y, FLEXURE_TOP - 0.1, 21.8)]
    bridge = bridge.cut(cuts)
    bridge = bridge.fuse(joint_pegs("bridge"))
    return bridge.removeSplitter()


def make_press_flexure(i):
    """Flat-printable replaceable leaf; stiffness and fatigue require a physical trial."""
    beam = band(FLEXURE_PATH, FLEXURE_W, FLEXURE_TOP - FLEXURE_T, FLEXURE_TOP)
    pads = [cyl(6.0 if k == 0 else 5.0, x, y, FLEXURE_TOP - FLEXURE_PAD_T, FLEXURE_TOP)
            for k, (x, y) in enumerate((FLEXURE_PATH[0], FLEXURE_PATH[-1]))]
    leaf = beam.fuse(pads)
    for k, (x, y) in enumerate((FLEXURE_PATH[0], FLEXURE_PATH[-1])):
        leaf = leaf.cut(cyl(2.4, x, y, FLEXURE_TOP - FLEXURE_PAD_T - 1, FLEXURE_TOP + 1))
    leaf.rotate(V(), V(0, 0, 1), i * 90)
    return leaf.removeSplitter()


def dummy_press_switch(travel=0.0):
    body = cbox(0, 0, SW_BODY, SW_BODY, C_TOP, C_TOP + SW_BODY_H)
    boss = cyl(SW_BOSS_D, 0, 0, C_TOP + SW_BODY_H - 0.01, C_TOP + SW_BOSS_H)
    neck = cbox(0, 0, SW_NECK, SW_NECK, C_TOP + SW_BOSS_H - travel - 0.01,
                SW_PLUNGER_TOP - SW_HEAD_H - travel + 0.01)
    head = cbox(0, 0, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H - travel,
                SW_PLUNGER_TOP - travel)
    legs = [cbox(x, y, SW_LEG[0], SW_LEG[1], C_TOP - SW_LEG[2], C_TOP + 0.01) for x, y in SW_PADS]
    return fuse_all([body, boss, neck, head] + legs)


def cap_grip(z_ceil, r_wall):
    """The grip round the stem head, round the origin, a "+" of ribs out to the cap wall at r_wall:
    two wide snap plates (long sides, along x, ribbed at the root), two short snap plates (along y,
    ribbed down half their length), and the stop post. Every plate's lip snaps in under the head."""
    z_k = SW_PLUNGER_TOP - SW_HEAD_H - 0.05 - CAP_SNAP_DROP  # catch face, just under the head
    land = 0.1

    def plate(w, t, clr, reach):
        """A snap plate on the +x side: free below its rib, a lip under the head, a 45° lead-in below
        it, ending 0.1 above the switch's round platform."""
        r_i = SW_HEAD / 2 + clr
        r_o, r_l = r_i + t, SW_HEAD / 2 - reach
        z_b = max(z_k - land - (r_i - r_l), C_TOP + SW_BOSS_H + 0.1)
        prof = [
            (r_o, z_ceil + 0.01),
            (r_i, z_ceil + 0.01),
            (r_i, z_k),
            (r_l, z_k),
            (r_l, z_k - land),
            (r_i, z_b),
            (r_o, z_b),
        ]
        f = Part.Face(Part.makePolygon([V(px, 0, pz) for (px, pz) in prof + [prof[0]]])).extrude(V(0, w, 0))
        f.translate(V(0, -w / 2, 0))
        return f, r_o, z_b

    w, t = CAP_FINGER
    f, r_o, _ = plate(w, t, CAP_SNAP_CLR, CAP_SNAP)
    rt, rh = CAP_FINGER_RIB
    f = f.fuse(box(r_o - 0.01, -rt / 2, z_ceil - rh, r_wall + 0.3, rt / 2, z_ceil + 0.01))
    parts = [place(f, 0, 0, k * 180.0) for k in range(2)]
    sw, st, sc = CAP_SIDE
    f, r_o, z_b = plate(sw, st, sc, CAP_SIDE_SNAP)
    rt, share = CAP_SIDE_RIB
    f = f.fuse(
        box(
            r_o - 0.01,
            -rt / 2,
            z_ceil - share * (z_ceil - z_b),
            r_wall + 0.3,
            rt / 2,
            z_ceil + 0.01,
        )
    )
    parts += [place(f, 0, 0, k * 90.0) for k in (1, 3)]
    parts.append(cyl(CAP_STOP_D, 0, 0, SW_PLUNGER_TOP, z_ceil + 0.01))
    rd, rh = CAP_STOP_ROOT  # the post's foot: a cone into the cap top
    parts.append(Part.makeCone(CAP_STOP_D / 2, rd / 2, rh + 0.01, V(0, 0, z_ceil - rh), V(0, 0, 1)))
    return fuse_all(parts)


def make_tile(i):
    """A quarter of the top: plate, lip over the wall top, three legs, switch socket.
    Printed face down, so all four faces have the same bed finish."""
    plate = tile_prism(i, tile_under, DECK_TOP)
    try:
        top = [
            e for e in plate.Edges if abs(e.BoundBox.ZMin - DECK_TOP) < 1e-6 and abs(e.BoundBox.ZMax - DECK_TOP) < 1e-6
        ]
        plate = plate.makeChamfer(TILE_CHAMFER, top)
    except Exception as exc:
        print("tile chamfer skipped:", exc)
    lip = rr_prism(S, S, S_R, TILE_SKIRT_Z, tile_under + 0.01).cut(
        rr_prism(
            S - 2 * TILE_SKIRT_T,
            S - 2 * TILE_SKIRT_T,
            S_R - TILE_SKIRT_T,
            TILE_SKIRT_Z - 1,
            DECK_TOP,
        )
    )
    lip = lip.common(tile_prism(i, TILE_SKIRT_Z - 1, DECK_TOP))
    legs = [cyl(LEG_D, x, y, LEG_BOTTOM, tile_under + 0.01) for (x, y) in LEGS[i]]
    x, y = BTNS[i]
    grip = cap_grip(tile_under, 3.7)
    grip.translate(V(x, y, 0))
    t = fuse_all([plate, lip, grip] + legs)
    if i == 1:
        t = t.cut(cyl(1.8, *rst_xy, tile_under - 1, DECK_TOP + 1))
    return t.removeSplitter()


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
    """Round board and glass keepouts from the 7-pin drawing, with two stacked sockets."""
    pcb = cyl(DISP_PCB_D, 0, 0, dpcb_z0, dpcb_z1).fuse(
        cbox((DISP_PCB_L - DISP_ACTIVE_TOP) / 2, 0, DISP_PCB_L - DISP_ACTIVE_TOP, DISP_TAB_W, dpcb_z0, dpcb_z1)
    )
    pcb = pcb.cut([cyl(DISP_HOLE_D, x, y, dpcb_z0 - 1, dpcb_z1 + 1) for x, y in disp_holes])
    glass = cyl(DISP_GLASS_D, *glass_c, glass_z0, glass_z1)
    # The glass assembly has a narrow flex tail, contained by the drawing's overall LCD envelope.
    tail_x = 1.6 + DISP_GLASS_L - DISP_ACTIVE_TOP
    glass = glass.fuse(cbox(tail_x / 2, 0, tail_x, DISP_FLEX_W, glass_z0, glass_z1))
    smd = cyl(33.0, 0, 0, dpcb_z0 - DISP_SMD_H, dpcb_z0)
    hdr = cbox(disp_hdr_x, 0, 2.54, 7 * 2.54, socket_top, dpcb_z0)
    sockets = cbox(disp_hdr_x, 0, 2.54, 7 * 2.54, C_TOP, socket_top)
    tails = [cbox(x, y, 0.64, 0.64, C_TOP - DISP_SOCKET_TAIL, C_TOP) for x, y in disp_pins.values()]
    return fuse_all([pcb,glass,smd,hdr,sockets]+tails)


def dummy_switch(i):
    x, y = BTNS[i]
    body = cbox(x, y, SW_BODY, SW_BODY, sw_base, sw_base + SW_BODY_H)
    boss = cyl(SW_BOSS_D, x, y, sw_base + SW_BODY_H - 0.01, sw_base + SW_BOSS_H)
    neck = cbox(x, y, SW_NECK, SW_NECK, sw_base + SW_BOSS_H - 0.01, SW_PLUNGER_TOP - SW_HEAD_H + 0.01)
    head = cbox(x, y, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H, SW_PLUNGER_TOP)
    legs = [cbox(x + dx, y + dy, SW_LEG[0], SW_LEG[1], sw_base - SW_LEG[2], sw_base + 0.01)
            for dx, dy in SW_PADS]
    return fuse_all([body, boss, neck, head] + legs)


def dummy_supermini():
    hdr_y = SM_USB_END - SM_PIN_MARGIN - 3.5 * 2.54
    strips = [cbox(x, hdr_y, SM_HDR[0], SM_HDR[1], C_TOP, sm_lo) for x in sm_rows]
    pins = fuse_all([cbox(x, y, 0.64, 0.64, sm_lo - SM_HDR[2] - SM_HDR[3], sm_lo) for (x, y) in sm_pins.values()])
    pcb = cbox(sm_c[0], sm_c[1], SM[0], SM[1], sm_lo, sm_hi)
    usb = cbox(
        SM_X,
        SM_USB_END + SM_USB_OVERHANG - SM_USB[1] / 2,
        SM_USB[0],
        SM_USB[1],
        sm_hi,
        sm_hi + SM_USB[2],
    )
    chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_hi, sm_hi + 0.9)
    rx, ry = rst_xy
    rst = cbox(rx, ry, *SM_BTN[:2], sm_hi, rst_top)
    return fuse_all(strips + [pins, pcb, usb, chip, rst])


def dummy_encoder_nut():
    """The EC11's washer and M7 nut on the bushing, on the printed PCB's front."""
    af, t, wt = EC11_NUT
    washer = cyl(af + 1.0, SHAFT_C[0], SHAFT_C[1], C_TOP, C_TOP + wt)
    nut = Part.Face(
        Part.makePolygon(
            [
                V(
                    SHAFT_C[0] + af / math.sqrt(3) * math.cos(math.radians(60 * k)),
                    SHAFT_C[1] + af / math.sqrt(3) * math.sin(math.radians(60 * k)),
                    C_TOP + wt,
                )
                for k in range(7)
            ]
        )
    ).extrude(V(0, 0, t))
    return washer.fuse(nut).cut(cyl(EC11_BUSH_D, SHAFT_C[0], SHAFT_C[1], C_TOP - 1, C_TOP + wt + t + 1))


def shared_offsets():
    """Side-by-side offsets for runs of one net that share a channel: {(run, segment): mm}, one
    wire to each side of the channel's centre line."""
    runs = wire_layout()
    segs = [(k, i, a, b) for k, (_, pts) in enumerate(runs) for i, (a, b) in enumerate(zip(pts, pts[1:]))]
    off = {}
    for n, (k1, i1, a1, b1) in enumerate(segs):
        for k2, i2, a2, b2 in segs[n + 1 :]:
            if k1 == k2 or runs[k1][0] != runs[k2][0]:
                continue
            d1 = (b1[0] - a1[0], b1[1] - a1[1])
            L1 = math.hypot(*d1)
            u = (d1[0] / L1, d1[1] / L1)
            cross = lambda p: (p[0] - a1[0]) * u[1] - (p[1] - a1[1]) * u[0]
            if abs(cross(a2)) > 1e-6 or abs(cross(b2)) > 1e-6:
                continue  # not on the same line
            t = sorted(
                [
                    (a2[0] - a1[0]) * u[0] + (a2[1] - a1[1]) * u[1],
                    (b2[0] - a1[0]) * u[0] + (b2[1] - a1[1]) * u[1],
                ]
            )
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
    zc = C_BOT + WIRE_CH[1] - WIRE_D / 2
    zd = C_BOT + WIRE_CROSS[1] - WIRE_D / 2  # in the dip under a crossing
    r = WIRE_D / 2
    off = shared_offsets()
    dips = {}
    for x, y, _, kb, d in wire_crossings():
        dips.setdefault(kb, []).append((x, y, d))
    # Nearby crossings share one deep run so the wire cannot rise into the Kapton between them.
    for k, crossings in dips.items():
        lines = {}
        for x, y, (dx, dy) in crossings:
            key = (round(dx, 6), round(dy, 6), round(x * dy - y * dx, 4))
            t = x * dx + y * dy
            lines.setdefault(key, []).append((t - WIRE_CROSS[0] / 2, t + WIRE_CROSS[0] / 2))
        merged = []
        for (dx, dy, normal), intervals in lines.items():
            spans = []
            for lo, hi in sorted(intervals):
                if spans and lo <= spans[-1][1]:
                    spans[-1][1] = max(spans[-1][1], hi)
                else:
                    spans.append([lo, hi])
            for lo, hi in spans:
                t = (lo + hi) / 2
                merged.append((dx * t + dy * normal, dy * t - dx * normal, (dx, dy), (hi - lo) / 2))
        dips[k] = merged
    parts = []
    for k, (_, pts) in enumerate(wire_layout()):
        P = offset_run(pts, k, off)
        path = Part.Wire(run_edges(P, zc))
        e0 = path.OrderedEdges[0]
        prof = Part.Wire(Part.makeCircle(r, e0.Vertexes[0].Point, e0.tangentAt(e0.FirstParameter)))
        # it leaves the channel at the pin (down onto the turns), not through it
        w = path.makePipeShell([prof], True, True).cut([cyl(1.0, x, y, zc - 1, zc + 1) for (x, y) in (pts[0], pts[-1])])
        pieces = []
        for x, y, (dx, dy), h in dips.get(k, []):
            # the segment's sideways offset, so the dip lines up with the wire
            j = min(
                range(len(pts) - 1),
                key=lambda i: (
                    Part.LineSegment(V(*pts[i], 0), V(*pts[i + 1], 0)).toShape().distToShape(Part.Vertex(V(x, y, 0)))[0]
                ),
            )
            px, py = x + (P[j][0] - pts[j][0]), y + (P[j][1] - pts[j][1])
            w = w.cut(
                place(
                    box(-h, -1, zc - 1, h, 1, zc + 1),
                    px,
                    py,
                    math.degrees(math.atan2(dy, dx)),
                )
            )
            pp = [V(px + dx * s, py + dy * s, z) for (s, z) in ((-h, zc), (-h + 0.8, zd), (h - 0.8, zd), (h, zc))]
            for a, b in zip(pp, pp[1:]):
                pieces.append(Part.makeCylinder(r, (b - a).Length, a, b - a))
            pieces.extend(Part.makeSphere(r, q) for q in pp)
        if pieces:
            # Crossing ramps near a terminal use the same pin exit clearance as the channel.
            pieces = [Part.makeCompound(pieces).cut(
                [cyl(1.0, x, y, C_BOT - 2, C_TOP + 1) for x, y in (pts[0], pts[-1])])]
        wire = Part.makeCompound([w] + pieces)
        assert wire.isValid(), ("wire solid", k)
        parts.append(wire)
    ring = lambda x, y, z0, z1: cyl(WRAP_OD, x, y, z0, z1).cut(cyl(0.95, x, y, z0 - 1, z1 + 1))
    tall = WRAP_TURNS * WIRE_D
    counts = wire_end_counts()
    for x, y in list(sm_pins.values()) + list(disp_pins.values()):
        count = counts.get((round(x, 4), round(y, 4)), 0)
        if count:
            parts.append(ring(x, y, C_BOT - 0.3 - count * tall, C_BOT - 0.3))
    for x, y in enc_pins.values():
        count = counts.get((round(x, 4), round(y, 4)), 0)
        if count:
            parts.append(ring(x, y, C_BOT - 0.5 - count * tall, C_BOT - 0.5))
    return Part.makeCompound(parts)


def kapton_squares(z0, z1):
    """A Kapton square at each crossing, square to the lower run, from z0 to z1."""
    s = KAPTON[0] / 2
    return [
        place(box(-s, -s, z0, s, s, z1), x, y, math.degrees(math.atan2(dy, dx)))
        for (x, y, _, _, (dx, dy)) in wire_crossings()
    ]


def dummy_kapton():
    """The Kapton squares, on the floor of their pockets, between the lower and the upper wire."""
    z = C_BOT + WIRE_CH[1]
    return Part.makeCompound(kapton_squares(z, z + KAPTON[1]))


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

LOCAL = {}  # printed parts in the deck frame (how they print)
PRINTED = {}  # printed parts on the desk
DUMMIES = {}
BUTTON_NAMES = ["TileBackLeft", "TileBackRight", "TileFrontLeft", "TileFrontRight"]
FLEXURE_NAMES = ["FlexureBackRight", "FlexureBackLeft", "FlexureFrontLeft", "FlexureFrontRight"]
COLORS = {
    "PressBridge": (0.85, 0.65, 0.15),
    "Flexure": (0.55, 0.25, 0.70),
    "Ring": (0.80, 0.80, 0.82),
    "Shell": (0.13, 0.13, 0.15),
    "Floor": (0.13, 0.13, 0.15),
    "Retainer": (0.12, 0.45, 0.25),
    "RubberDots": (0.08, 0.08, 0.08),
    "Cap": (0.04, 0.04, 0.05),
    "Carrier": (0.12, 0.45, 0.25),
    "Pinion": (0.95, 0.50, 0.18),
    "Dummy": (0.30, 0.45, 0.85),
    "Wire": (0.85, 0.50, 0.20),
    "Kapton": (0.95, 0.70, 0.15),
}
FLIP = ("Ring", "Shell", "Cap", "PressBridge") + tuple(BUTTON_NAMES) + tuple(FLEXURE_NAMES)  # printed upside down


def build():
    LOCAL["Shell"] = make_shell()
    LOCAL["Ring"] = make_ring()
    LOCAL["Retainer"] = make_retainer()
    LOCAL["Carrier"] = make_carrier()
    LOCAL["Pinion"] = make_pinion()
    LOCAL["Cap"] = make_cap()
    LOCAL["PressBridge"] = make_press_bridge()
    for i, name in enumerate(FLEXURE_NAMES):
        LOCAL[name] = make_press_flexure(i)
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
    DUMMIES["Dummy_PressSwitch"] = tilted(dummy_press_switch())
    DUMMIES["Dummy_Display"] = tilted(dummy_display())
    DUMMIES["Dummy_SuperMini"] = tilted(dummy_supermini())
    PRINTED["RubberDots"] = make_rubber_dots()
    DUMMIES["Dummy_EncoderNut"] = tilted(dummy_encoder_nut())
    DUMMIES["Dummy_Wire"] = tilted(dummy_wire())
    DUMMIES["Dummy_Kapton"] = tilted(dummy_kapton())


def check_interference():
    items = list(PRINTED.items()) + list(DUMMIES.items())
    press_fits = {(n, "Dummy_" + n.replace("Tile", "Switch")) for n in BUTTON_NAMES}
    clashes, fits = [], []
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            (na, a), (nb, b) = items[i], items[j]
            if not a.BoundBox.intersect(b.BoundBox):
                continue
            v = a.common(b).Volume
            if v > 0.01:
                # The v4 fingers intentionally squeeze the head; larger overlaps are still errors.
                limit = 1.0 if (na, nb) in press_fits else (4.0 if (na, nb) == ("Floor", "RubberDots") else 0.0)
                target = fits if v < limit else clashes
                target.append((na, nb, round(v, 3)))
    if fits:
        print("press fits (intended):", fits)
    return clashes


def color_key(name):
    if name.startswith("Flexure"):
        return "Flexure"
    if name == "Dummy_Wire":
        return "Wire"
    if name == "Dummy_Kapton":
        return "Kapton"
    if name.startswith("Dummy"):
        return "Dummy"
    return "Shell" if name.startswith("Tile") else name


PIN_LABEL_FONT = 14


def pin_labels():
    """Pin, modeled net, anchor and text position, in the component-side deck frame."""
    nets = {}
    for net, pts in wire_layout():
        for point in (pts[0], pts[-1]):
            key = tuple(round(v, 3) for v in point)
            assert key not in nets or nets[key] == net, ("conflicting pin nets", point)
            nets[key] = net

    def net(point):
        return nets.get(tuple(round(v, 3) for v in point), "n/c")

    out = []
    gap = PIN_ROW[0] / 2 + 0.6
    for name, point in sm_pins.items():
        side = 1 if point[0] > SM_X else -1
        out.append((f"{name}: {net(point)}", point, (point[0] + side * gap, point[1]), side))
    for name, point in disp_pins.items():
        out.append((f"Disp {name}: {net(point)}", point, (point[0] + gap, point[1]), 1))
    # Fan out the horizontal encoder row so five labels do not overlap one another.
    for i, (name, point) in enumerate(enc_pins.items()):
        out.append((f"Enc {name}: {net(point)}", point, (12.0, point[1] - 5.0 - i * 3.0), 1))
    names = ["B2", "B3", "B1", "B4"]
    legs = ["FL", "FR", "BL", "BR"]  # front/back and left/right, seen from the component side
    for i, name in enumerate(names):
        side = 1 if BTNS[i][0] > 0 else -1
        for k, leg in enumerate(legs):
            point = btn_pin(i, k)
            out.append((f"{name}.{leg}: {net(point)}", point, (side * 53.0, BTNS[i][1] + (1.5 - k) * 3.0), side))
    for k, leg in enumerate(legs):
        point = SW_PADS[k]
        out.append((f"Ring SW.{leg}: {net(point)}", point, (-15.0, point[1] + (k - 1.5) * 3.0), -1))
    assert len(out) == 48
    assert len({point for _, point, _, _ in out}) == 48
    return out


def add_pin_labels(doc):
    """v4-style GUI annotations and leader lines; excluded from meshes and interference checks."""
    if doc.getObject("PinLabels"):
        for obj in list(doc.getObject("PinLabels").Group):
            doc.removeObject(obj.Name)
        doc.removeObject("PinLabels")
    group = doc.addObject("App::DocumentObjectGroup", "PinLabels")
    z = C_BOT - 2.5
    for k, (text, point, label, side) in enumerate(pin_labels()):
        a = doc.addObject("App::Annotation", f"PinLabel{k:02d}")
        a.LabelText = [text]
        a.Position = TILT.multVec(V(*label, z))
        group.addObject(a)
        line = doc.addObject("Part::Feature", f"PinLeader{k:02d}")
        line.Shape = Part.makeLine(TILT.multVec(V(*point, z)), a.Position)
        group.addObject(line)
        if App.GuiUp:
            a.ViewObject.FontSize = PIN_LABEL_FONT
            a.ViewObject.TextColor = (1.0, 0.85, 0.3)
            a.ViewObject.Justification = {1: "Left", -1: "Right"}[side]
            line.ViewObject.LineColor = (1.0, 0.85, 0.3)
            line.ViewObject.LineWidth = 1.0
    doc.recompute()
    return group


def to_doc():
    if DOC_NAME in App.listDocuments():
        App.closeDocument(DOC_NAME)
    doc = App.newDocument(DOC_NAME)
    for name, shp in list(PRINTED.items()) + list(DUMMIES.items()):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shp
        color = COLORS[color_key(name)]
        if App.GuiUp and obj.ViewObject:
            obj.ViewObject.ShapeColor = color
            obj.ViewObject.Visibility = True
    add_pin_labels(doc)
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


def check_wiring():
    """Every wire ends on its assigned pin, with each net connected through pins or switch pairs."""
    expected = {
        "GND": [sm_pins["GND"], enc_pins["GND"], disp_pins["GND"]] + [btn_pin(i, k) for i in range(4) for k in (0, 1)],
        "3V3": [sm_pins["3V3"], enc_pins["+"], disp_pins["VCC"]],
    }
    for net, pin in [("CLK", "IO7"), ("DT", "IO6")]:
        expected[net] = [sm_pins[pin], enc_pins[net]]
    expected["SW"] = [sm_pins["IO5"], SW_PADS[2]]
    expected["GND"].append(SW_PADS[0])
    for net, pin in [
        ("SCL", "IO4"),
        ("SDA", "IO3"),
        ("RST", "IO2"),
        ("DC", "IO1"),
        ("CS", "IO0"),
    ]:
        expected[net] = [sm_pins[pin], disp_pins[net]]
    for net, pin, i, k in [
        ("B1", "IO9", 2, 3),
        ("B2", "IO8", 0, 3),
        ("B3", "IO10", 1, 2),
        ("B4", "IO20", 3, 2),
    ]:
        expected[net] = [sm_pins[pin], btn_pin(i, k)]
    runs = wire_layout()
    pin_labels()
    assert {n for n, _ in runs} == set(expected)
    for net, pins in expected.items():
        edges = [(pts[0], pts[-1]) for n, pts in runs if n == net]
        assert {p for edge in edges for p in edge} == set(pins), net + " wire endpoints"
        if net == "GND":
            edges += [(btn_pin(i, 0), btn_pin(i, 1)) for i in range(4)]
        seen = {pins[0]}
        while True:
            reached = seen | {b for a, b in edges + [(b, a) for a, b in edges] if a in seen}
            if reached == seen:
                break
            seen = reached
        assert seen == set(pins), net + " disconnected"
    assert WINDOW >= 32.4 and DISP_HOLE_PITCH == 19.26
    assert abs(DISP_HDR_X - 24.74) < 1e-6 and abs(DISP_HOLE_X - 20.67) < 1e-6
    assert abs((1.6 + DISP_GLASS_L - DISP_ACTIVE_TOP) - (glass_c[0] - DISP_GLASS_D / 2) - DISP_GLASS_L) < 1e-6
    assert disp_pins["VCC"][1] > disp_pins["GND"][1] > disp_pins["SCL"][1]
    assert "BLK" not in disp_pins and len(disp_holes) == 2
    assert math.hypot(*rst_xy)-RST_GUIDE_D/2 > RET_R[1]+0.5, 'reset guide / rotating retainer'
    counts = wire_end_counts()
    for pins, exposed in ((sm_pins, SM_HDR[3] - C_T), (disp_pins, DISP_SOCKET_TAIL - C_T),
                          (enc_pins, C_BOT - enc_board_front - 2.5)):
        for x, y in pins.values():
            count = counts.get((round(x, 4), round(y, 4)), 0)
            assert exposed >= count * WRAP_TURNS * WIRE_D + 0.5, ("wrap exposure", (x, y), count)
    assert C_T - WIRE_CROSS[1] >= 0.7 - 1e-6
    print("wire endpoints, net connectivity, wrap length and crossing thickness: ok")


def check_wire_separation():
    """Check the actual 3D runs, including merged crossing dips, for unintended contacts."""
    runs = wire_layout()
    wires = DUMMIES['Dummy_Wire'].childShapes()[:len(runs)]
    assert len(wires)==len(runs) and all(wire.isValid() for wire in wires)
    for i,a in enumerate(wires):
        for j,b in enumerate(wires[i+1:],i+1):
            if runs[i][0]!=runs[j][0]:
                assert a.distToShape(b)[0]>=0.05, ('wire separation',i,runs[i][0],j,runs[j][0])
    print('different-net 3D wire separation: ok')


def check_case_latches():
    """Each detent clears at rest, catches a pull, and passes its rib when flexed inward."""
    for side, y in CASE_HOOKS:
        hook, catch = case_latch(side, y)
        assert hook.common(catch).Volume < 0.01, ("latch rest", side, y)
        assert 0.02 < hook.distToShape(catch)[0] < 0.1, ("latch catch clearance", side, y)
        pulled = hook.copy()
        pulled.translate(V(0, 0, -0.8))
        assert pulled.common(catch).Volume > 0.1, ("latch does not retain", side, y)
        # Rigid shift approximates tip clearance; it does not simulate bending or force.
        flexed = hook.copy()
        flex = CASE_HOOK_BARB + CASE_RIB_DEPTH - CASE_HOOK_GAP + 0.05
        flexed.translate(V(-side * flex, 0, 0))
        for travel in (0.0, 0.8, 1.6, 2.4, 3.2):
            moving = flexed.copy()
            moving.translate(V(0, 0, -travel))
            assert moving.common(catch).Volume < 0.01, ("latch release path", side, y, travel)
        top = tilted(hook).BoundBox.ZMax
        assert top > FLOOR_T + 5, ("latch free length", side, y)
    print("four v4-style case latches: rest, retention and flexed release paths ok", flush=True)


def check_module_measurements():
    """Check actual reference shapes against v4 interfaces and the user's mounted shaft datum."""
    assert abs(enc_shaft_top - C_TOP - 17.0) < 1e-6, "mounted shaft projection changed"
    assert abs(HW040_HDR_X - 10.25) < 1e-6, "HW040 row differs from v4 measured location"
    assert bush_top - C_TOP >= sum(EC11_NUT[1:]) + 1.0, "insufficient nut thread"
    for switch in (dummy_switch(0), dummy_press_switch()):
        assert abs(switch.BoundBox.ZMin - (C_TOP - 3.5)) < 1e-6, "v4 switch leg length"
        assert abs(switch.BoundBox.ZMax - (C_TOP + 7.2)) < 1e-6, "v4 switch actuator height"
    sm = tilted(DUMMIES["Dummy_SuperMini"], UNTILT)
    assert abs(C_TOP - sm.BoundBox.ZMin - 6.0) < 1e-6, "header tail must be 6 mm past plastic"
    encoder = tilted(DUMMIES["Dummy_Encoder"], UNTILT)
    section = encoder.common(box(SHAFT_C[0] - 4, SHAFT_C[1] - 4, enc_shaft_top - 0.2,
                                 SHAFT_C[0] + 4, SHAFT_C[1] + 4, enc_shaft_top - 0.1))
    section.rotate(V(*SHAFT_C, 0), V(0, 0, 1), -HW040_ROT)
    assert abs(section.BoundBox.YLength - 4.5) < 1e-6, "v4 CAD D-flat fit changed"
    print("measurement datums: 17 mm mounted shaft; v4 header, D-flat, switches and pin tails ok", flush=True)


def check_display_sightlines():
    """Geometric viewing cone, including optical-depth and active-circle centring allowances."""
    blockers = (LOCAL["Cap"],)
    z0 = glass_z1 - DISPLAY_VIEW_DEPTH
    z1 = RING_TOP + 2.0
    edge = 32.4 / 2 + DISPLAY_VIEW_MARGIN
    for direction in range(0, 360, 45):
        a = math.radians(direction)
        dx, dy = math.cos(a), math.sin(a)
        for view in (0.0, 10.0, DISPLAY_VIEW_ANGLE):
            reach = edge + (z1 - z0) * math.tan(math.radians(view))
            ray = Part.makeLine(V(edge * dx, edge * dy, z0), V(reach * dx, reach * dy, z1))
            for shape in blockers:
                assert ray.common(shape).Length < 1e-6, ("display edge obscured", direction, view)
    print("display sight lines: eight directions through 20 degrees, 0.5 mm depth and 0.15 mm centring allowance ok", flush=True)


def check_snap_joints():
    """Verify positive lands and squeezed insertion space, not force or allowable strain."""
    face = ring_solid(20.0, 26.0, CAP_TOP - 0.1, CAP_TOP - 0.05)
    assert abs(face.common(LOCAL["Cap"]).Volume - face.Volume) < 1e-5, "Cap face has unwanted holes"
    for joint, points, hole, lip in (("carrier", PRESS_ANCHORS, 2.4, FLEXURE_TOP),
                                      ("bridge", PRESS_NODES, 2.4, FLEXURE_TOP - FLEXURE_PAD_T),
                                      ("cap", PRESS_NODES, 3.8, 27.0)):
        normal, squeezed = joint_pegs(joint), joint_pegs(joint, True)
        for x, y in points:
            zone = cyl(6.0, x, y, lip - 0.81, lip + 0.81)
            outside = zone.cut(cyl(hole, x, y, lip - 1, lip + 1))
            assert normal.common(outside).Volume > 0.01, (joint, "retention lip missing")
            assert squeezed.common(outside).Volume < 0.01, (joint, "squeezed barb does not fit")
    free_bridge = LOCAL["PressBridge"].cut(joint_pegs("bridge")).fuse(joint_pegs("bridge", True))
    for i, name in enumerate(FLEXURE_NAMES):
        for drop in (1.5, 0.8, 0.3, 0.0):
            pad = LOCAL[name].copy()
            pad.translate(V(0, 0, -drop))
            obstacle = free_bridge if drop > 0 else LOCAL["PressBridge"]
            assert pad.common(obstacle).Volume < 0.01, ("flexure to bridge insertion", i, drop)
    for angle in RING_HOOK_ANGLES:
        x, y = polar(31.4, angle)
        assert RING_CATCH_R < math.hypot(x, y) - 0.3
        x, y = polar(31.4 - 0.7, angle)
        assert math.hypot(x, y) < RING_CATCH_R - 0.1
    print("clean Cap face; twelve positive snap pillars; four hidden ring hooks; squeezed fit envelopes ok", flush=True)


def check_retainer_clips():
    """Nominal seated retention and a squeezed insertion envelope; no force prediction."""
    ring, retainer = LOCAL["Ring"], LOCAL["Retainer"]
    assert abs(retainer.BoundBox.ZMin - (ret_top - RET_T)) < 1e-6, "retainer protrudes below old plane"
    assert ring.common(retainer).Volume < 0.01, "seated retainer clips collide"
    pulled = retainer.copy()
    pulled.translate(V(0, 0, -0.1))
    assert pulled.common(ring).Volume > 0.01, "retainer clips do not catch on separation"
    squeezed = retainer.cut(retainer_clips()).fuse(retainer_clips(True))
    for drop in (8.0, 6.0, 4.0, 2.0, 1.0, 0.3, 0.0):
        candidate = (squeezed if drop > 0 else retainer).copy()
        candidate.translate(V(0, 0, -drop))
        assert candidate.common(ring).Volume < 0.01, ("retainer clip insertion", drop)
    # A thin angled probe can enter each release window from below while this joint is accessible.
    for angle in RET_CLIP_ANGLES:
        start = V(*polar(37.0, angle), ret_top - RET_T - 1)
        end = V(*polar(36.18, angle), ret_top + 2.0)
        probe = Part.makeCylinder(0.25, (end - start).Length, start, (end - start))
        assert probe.common(ring).Volume < 0.01, ("retainer release opening", angle, "ring")
        assert probe.common(retainer).Volume < 0.01, ("retainer release opening", angle, "retainer")
    print("three retainer clips: seated fit, positive catch, squeezed insertion and local release-probe access ok", flush=True)


def check_assembly_paths():
    """Sample the documented vertical insertion order; each step excludes not-yet-fitted parts."""
    pads = [LOCAL[name] for name in FLEXURE_NAMES]
    steps = [
        ("bridge and suspension", [LOCAL["PressBridge"], *pads],
         ["Carrier", "Pinion", "Dummy_Encoder", "Dummy_SuperMini", "Dummy_PressSwitch"]),
        ("display", [tilted(DUMMIES["Dummy_Display"], UNTILT)],
         ["Carrier", "PressBridge", "Pinion", *FLEXURE_NAMES]),
        ("Cap", [LOCAL["Cap"]],
         ["Carrier", "PressBridge", "Pinion", "Dummy_Display", "Dummy_SuperMini", *FLEXURE_NAMES]),
        ("shell and ring", [LOCAL["Shell"], LOCAL["Ring"], LOCAL["Retainer"]],
         ["Carrier", "Cap", "PressBridge", "Pinion", "Dummy_Display", "Dummy_SuperMini", *FLEXURE_NAMES]),
    ]
    for label, pieces, names in steps:
        obstacles = {name: LOCAL[name] if name in LOCAL else tilted(DUMMIES[name], UNTILT) for name in names}
        if "Carrier" in obstacles:
            low = min(shape.BoundBox.ZMin for shape in pieces) - 0.1
            obstacles["Carrier"] = obstacles["Carrier"].common(box(-80, -80, low, 80, 80, CAP_TOP + 10))
        squeezed = dict(obstacles)
        insertion_pieces = pieces
        if label == "bridge and suspension":
            squeezed["Carrier"] = obstacles["Carrier"].cut(joint_pegs("carrier")).fuse(joint_pegs("carrier", True))
        elif label == "Cap":
            insertion_pieces = [pieces[0].cut(joint_pegs("cap")).fuse(joint_pegs("cap", True))]
        elif label == "shell and ring":
            squeezed["Cap"] = obstacles["Cap"].cut(cap_ring_hooks()).fuse(squeezed_ring_hooks())
        for lift in (30.0, 20.0, 12.0, 6.0, 3.0, 1.5, 0.8, 0.0):
            for piece in insertion_pieces if lift > 0 else pieces:
                moving = piece.copy()
                moving.translate(V(0, 0, lift))
                for name, obstacle in (squeezed if lift > 0 else obstacles).items():
                    if moving.BoundBox.intersect(obstacle.BoundBox):
                        assert moving.common(obstacle).Volume < 0.01, ("assembly insertion", label, lift, name)
        print("assembly insertion:", label, "ok", flush=True)


def check_motion():
    """Central switch moves with the bridge; the encoder and its pinion stay stationary."""
    fixed = {k: LOCAL[k] for k in ("Carrier", "Shell", "Pinion", *BUTTON_NAMES)}
    fixed.update({k: tilted(DUMMIES[k], UNTILT) for k in
                  ("Dummy_Display", "Dummy_SuperMini", "Dummy_Encoder", "Dummy_EncoderNut",
                   "Dummy_Wire", "Dummy_Kapton")})
    moving_names = ("Cap", "PressBridge", "Ring", "Retainer")
    low = FLEXURE_TOP - FLEXURE_PAD_T - BEZEL_STOP_TRAVEL - 0.2
    fixed["Carrier"] = fixed["Carrier"].common(box(-80, -80, low, 80, 80, CAP_TOP + 5))
    for travel in (0.0, PRESS_CLICK / 2, PRESS_CLICK, BEZEL_STOP_TRAVEL):
        print("checking central ring press at %.3f mm" % travel, flush=True)
        moving = {k: LOCAL[k].copy() for k in moving_names}
        for shape in moving.values():
            shape.translate(V(0, 0, -travel))
        fixed["Dummy_PressSwitch"] = dummy_press_switch(travel)
        for name, shape in moving.items():
            for other, obstacle in fixed.items():
                if shape.BoundBox.intersect(obstacle.BoundBox):
                    assert shape.common(obstacle).Volume < 0.01, (name, travel, other)
        for i, name in enumerate(moving_names):
            for other in moving_names[i + 1:]:
                assert moving[name].common(moving[other]).Volume < 0.01, (name, travel, other)
        assert moving["PressBridge"].distToShape(Part.Vertex(V(0, 0, SW_PLUNGER_TOP - travel)))[0] < 1e-6
        assert moving["Cap"].distToShape(moving["Ring"])[0] < 1e-6
    bridge = LOCAL["PressBridge"].copy()
    bridge.translate(V(0, 0, -BEZEL_STOP_TRAVEL - 0.05))
    assert bridge.common(fixed["Carrier"]).Volume > 0.01, "central press stop missing"
    assert RING_PRESS_GAP - BEZEL_STOP_TRAVEL >= 0.5
    assert PINION_GAP >= 1.0  # no intentional axial gear movement
    print("central click contact, stationary encoder, capture and 0.45 mm stop: ok", flush=True)


def check_pinion_axial_float():
    """Rotation-only encoder: full ring depression must clear a stationary pinion."""
    assert GEAR_Z[1] - BEZEL_STOP_TRAVEL - PINION_Z[1] >= 0.5
    for step in range(8):
        phase = step * 360 / RING_TEETH / 8
        ring, pinion = LOCAL["Ring"].copy(), LOCAL["Pinion"].copy()
        ring.rotate(V(), V(0, 0, 1), phase)
        ring.translate(V(0, 0, -BEZEL_STOP_TRAVEL))
        pinion.rotate(V(*SHAFT_C, 0), V(0, 0, 1), phase * RING_TEETH / PINION_TEETH)
        assert ring.common(pinion).Volume < 0.01, ("pressed mesh", phase)
    print("stationary pinion clears pressed ring at eight mesh phases", flush=True)


def check_flexure_envelopes():
    """Bound the vertical beam sweep; this does not simulate its elastic force or fatigue."""
    obstacles = {k: LOCAL[k] for k in ("Carrier", "Pinion", "Shell")}
    obstacles["Carrier"] = obstacles["Carrier"].common(box(-80, -80, C_TOP, 80, 80, CAP_TOP))
    obstacles.update({k: tilted(DUMMIES[k], UNTILT) for k in
                      ("Dummy_Display", "Dummy_SuperMini", "Dummy_EncoderNut", "Dummy_PressSwitch")})
    envelope = band(FLEXURE_PATH, FLEXURE_W + 0.2, FLEXURE_TOP - FLEXURE_T - 0.6, FLEXURE_TOP)
    for x, y in (FLEXURE_PATH[0], FLEXURE_PATH[-1]):
        envelope = envelope.cut(cyl(2.4, x, y, FLEXURE_TOP - 2, FLEXURE_TOP + 1))
    angle = math.radians(0.25)
    pivot_z = SW_PLUNGER_TOP - BEZEL_STOP_TRAVEL
    tooth_low = pivot_z + (GEAR_Z[0] - BEZEL_STOP_TRAVEL - pivot_z) * math.cos(angle) - GEAR_BAND_R * math.sin(angle)
    assert tooth_low - (FLEXURE_TOP + 0.8) > 0.05, "carrier snap head / fully pressed tilted teeth"
    # Conservative full-turn envelope of the plate and its new upward clips.
    heads = ring_solid(RET_R[0], RET_R[1], ret_top - RET_T - BEZEL_STOP_TRAVEL, ret_top).fuse(
        ring_solid(35.1, 36.5, ret_top - BEZEL_STOP_TRAVEL, ret_top + RET_CLIP_L + 0.9))
    for name, obstacle in obstacles.items():
        assert heads.common(obstacle).Volume < 0.01, ("retainer full-turn envelope", name)
    for i in range(4):
        assert LOCAL[FLEXURE_NAMES[i]].common(heads).Volume < 0.01, ("rotating retainer clips", i)
        beam = envelope.copy()
        beam.rotate(V(), V(0, 0, 1), i * 90)
        for name, obstacle in obstacles.items():
            if beam.BoundBox.intersect(obstacle.BoundBox):
                assert beam.common(obstacle).Volume < 0.01, ("flexure sweep", i, name)
        pad = cyl(6.2, *PRESS_NODES[i], FLEXURE_TOP - FLEXURE_PAD_T - 0.6, FLEXURE_TOP)
        for name, obstacle in obstacles.items():
            if pad.BoundBox.intersect(obstacle.BoundBox):
                assert pad.common(obstacle).Volume < 0.01, ("moving flexure pad", i, name)
        # End pads are clamped contacts; check the conservative free-span envelope separately.
        free = beam.cut([cyl(6.4, *PRESS_NODES[i], 15, 25), cyl(5.4, *PRESS_ANCHORS[i], 15, 25)])
        for stroke in (0.0, BEZEL_STOP_TRAVEL):
            bridge = LOCAL["PressBridge"].copy()
            bridge.translate(V(0, 0, -stroke))
            for name, obstacle in (("bridge", bridge),):
                if free.BoundBox.intersect(obstacle.BoundBox):
                    assert free.common(obstacle).Volume < 0.01, ("unclamped flexure span", i, stroke, name)
    print("four flexure beam envelopes clear over 0.6 mm vertical sweep; elastic response unverified", flush=True)


def check_corner_presses():
    """Clearance under small imposed tilts; this is not a force or friction simulation."""
    fixed = {k: LOCAL[k] for k in ("Shell", "Ring", "Cap", *BUTTON_NAMES)}
    low = LEG_BOTTOM - 1.0
    fixed["Carrier"] = LOCAL["Carrier"].common(box(-80, -80, low, 80, 80, CAP_TOP + 1))
    fixed.update({k: tilted(DUMMIES[k], UNTILT) for k in ("Dummy_Display", "Dummy_SuperMini")})
    corners = [(S / 2 - 6, S / 2 - 6), (S / 2 - 5, 4.0), (4.0, S / 2 - 5)]
    for i, name in enumerate(BUTTON_NAMES):
        bx, by = BTNS[i]
        for x, y in turned(corners, i):
            tile = LOCAL[name].copy()
            tile.translate(V(0, 0, -0.5))
            tile.rotate(V(bx, by, SW_PLUNGER_TOP - 0.5), V(-(y - by), x - bx, 0), 0.5)
            for other, obstacle in fixed.items():
                if other != name and tile.BoundBox.intersect(obstacle.BoundBox):
                    assert tile.common(obstacle).Volume < 0.01, ("tile corner", name, (x, y), other)
    print("tile corner clearance: 12 positions, 0.5 mm stroke with 0.5 degree tilt ok", flush=True)
    # At full stop the shoulders intentionally arrest tilt; test skew at the nominal click.
    shaft = V(0, 0, SW_PLUNGER_TOP - PRESS_CLICK)
    obstacles = {k: fixed[k] for k in ("Shell", "Carrier", "Dummy_Display", "Dummy_SuperMini", *BUTTON_NAMES)}
    pinion = LOCAL["Pinion"].copy()
    obstacles["Pinion"] = pinion
    for angle in range(0, 360, 45):
        x, y = RING_R * math.cos(math.radians(angle)), RING_R * math.sin(math.radians(angle))
        axis = V(-y, x, 0)
        moving_parts = {name: LOCAL[name] for name in ("Cap", "PressBridge", "Ring", "Retainer")}
        for i, node in enumerate(PRESS_NODES):
            moving_parts["flexure pad %d" % i] = LOCAL[FLEXURE_NAMES[i]].common(
                cyl(6.0, *node, FLEXURE_TOP - FLEXURE_PAD_T - 0.1, FLEXURE_TOP + 0.1))
        for name, source in moving_parts.items():
            part = source.copy()
            part.translate(V(0, 0, -PRESS_CLICK))
            part.rotate(shaft, axis, 0.25)
            for other, obstacle in obstacles.items():
                if part.BoundBox.intersect(obstacle.BoundBox):
                    assert part.common(obstacle).Volume < 0.01, ("ring edge", (x, y), name, other)
    print("ring edge clearance: 8 positions, nominal 0.35 mm stroke with 0.25 degree tilt ok", flush=True)


def check_ring_rocking():
    """Continuous hidden ring ledge lifts into the four integral Cap hooks."""
    bearing = Part.makeCompound([f for f in LOCAL["Cap"].Faces
                                if abs(f.BoundBox.ZMin - RING_BEARING_Z) < 1e-6
                                and abs(f.BoundBox.ZMax - RING_BEARING_Z) < 1e-6])
    assert not bearing.isNull(), "ring bearing face missing"
    # This full annulus is contained in the real ledge, so capture is independent of ring phase.
    land = ring_solid(RING_CATCH_R, 31.8, RING_CATCH_Z - 0.5, RING_CATCH_Z)
    assert abs(land.common(LOCAL["Ring"]).Volume - land.Volume) < 1e-5
    hooks = LOCAL["Cap"].common(cap_ring_hooks())
    hooks.translate(V(0, 0, -PRESS_CLICK))
    land.translate(V(0, 0, -PRESS_CLICK))
    raised = land.copy()
    raised.translate(V(0, 0, RING_CATCH_GAP + 0.05))
    assert raised.common(hooks).Volume > 0.01, "hidden hooks do not prevent ring lift"
    obstacles = {k: LOCAL[k] for k in ("Shell", *BUTTON_NAMES)}
    obstacles["Carrier"] = LOCAL["Carrier"].common(box(-80, -80, 19, 80, 80, CAP_TOP + 1))
    obstacles["Dummy_Display"] = tilted(DUMMIES["Dummy_Display"], UNTILT)
    cap = LOCAL["Cap"].copy()
    cap.translate(V(0, 0, -PRESS_CLICK))
    for angle in range(0, 360, 45):
        a = math.radians(angle)
        dx, dy = math.cos(a), math.sin(a)
        support = bearing.copy()
        support.rotate(V(), V(0, 0, 1), -angle)
        pivot = V(support.BoundBox.XMax * dx, support.BoundBox.XMax * dy,
                  RING_BEARING_Z - PRESS_CLICK)
        lo, hi = 0.1, 0.3
        for _ in range(10):
            mid = (lo + hi) / 2
            probe = land.copy()
            probe.rotate(pivot, V(-dy, dx, 0), mid)
            if probe.common(hooks).Volume > 0.00001:
                hi = mid
            else:
                lo = mid
        assert hi < 0.3, ("hidden ring capture missing", angle)
        for name in ("Ring", "Retainer"):
            part = LOCAL[name].copy()
            part.translate(V(0, 0, -PRESS_CLICK))
            part.rotate(pivot, V(-dy, dx, 0), lo)
            for other, obstacle in {**obstacles, "Cap": cap, "Pinion": LOCAL["Pinion"]}.items():
                if part.BoundBox.intersect(obstacle.BoundBox):
                    assert part.common(obstacle).Volume < 0.01, ("before hidden capture", angle, name, other)
        print("hidden hooks first contact at %.3f degrees, direction %d" % (hi, angle), flush=True)
    print("hidden ring ledge retained through full rotation; eight rocking directions clear until capture", flush=True)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    check_wiring()
    build()
    doc = to_doc()
    check_module_measurements()
    check_display_sightlines()
    check_snap_joints()
    check_retainer_clips()
    check_assembly_paths()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    tight = [r for r in check_wires() if r[0] < WIRE_RIDGE_MIN - 1e-6]
    print(
        "wire channels: %d runs, %d crossing(s), plastic under %.1f mm: %s"
        % (len(wire_layout()), len(wire_crossings()), WIRE_RIDGE_MIN, tight or "none")
    )
    assert not clashes, "assembly interference"
    assert not tight, "wire-channel clearance"
    assert all(s.isValid() and len(s.Solids) == 1 for n, s in PRINTED.items() if n != "RubberDots")
    assert PRINTED["RubberDots"].isValid() and len(PRINTED["RubberDots"].Solids) == 4
    check_wire_separation()
    check_case_latches()
    check_motion()
    check_pinion_axial_float()
    check_corner_presses()
    check_ring_rocking()
    check_flexure_envelopes()
    assert CAP_FACE_T - DISP_FPC_RELIEF[0] >= 0.7
    assert PINION_GAP - BEZEL_STOP_TRAVEL >= 0.5
    assert LEG_BOTTOM - 0.5 - C_TOP >= 1.0
    assert TILE_PRESS_GAP - 0.5 >= 0.8
    print("height: front %.1f, back %.1f, ring top %.1f mm (wedge %.1f°)" % (heights() + (WEDGE_DEG,)))
    for name, shp in PRINTED.items():
        bb = shp.BoundBox
        print(
            "%-16s vol %8.0f mm3  bbox %.1f x %.1f x %.1f  valid=%s solids=%d"
            % (
                name,
                shp.Volume,
                bb.XLength,
                bb.YLength,
                bb.ZLength,
                shp.isValid(),
                len(shp.Solids),
            )
        )
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):
    main()
