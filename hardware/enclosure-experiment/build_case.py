"""Kivori experimental enclosure: square body, a turning ring round a static screen, 4 buttons.

FreeCAD 1.1, Part workbench, no GUI needed. Run it inside FreeCAD (GUI Python console, the FreeCAD
MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-experiment/build_case.py"
    exec(open(p).read(), {"__file__": p, "__name__": "__main__"})

How it works with the standard parts (ST7789 1.3" square LCD, HW-040 / EC11 encoder):
  * The screen sits still in the middle, under a square window in a round black cap.
  * The ring turns round it. Teeth on the inside of the ring drive a small gear on the encoder
    shaft, which sits off-centre under the screen (an internal gear pair: both turn the same way).
  * The cap is hinged at the back. Pressing it tips it onto the top of the encoder shaft, so the
    HW-040's own push switch is the click (GPIO10, unchanged).
  * The whole top is four quadrant tiles round the ring: those are the buttons.

Frame: the desk is z = 0, the ring axis is the z axis, +x is right and +y is away from the user.

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
# seems to float over a shadow line.
S = 100.0                   # outer size (tile outline)
S_R = 10.0                  # corner radius
WALL_INSET = 1.5
WALL = 2.0
FLOOR_T = 2.0
DECK_TOP = 24.0             # top face of the tiles
SUBDECK_TOP, SUBDECK_T = 21.2, 2.0

# Ring (turns): rests on the sub-deck, a screwed-on retainer under it keeps it in
RING_TOP = 33.0
RING_R = 34.0               # outer radius above the sub-deck
RING_HOLE = 33.0            # sub-deck hole radius
RING_NECK_R = 32.8          # ring radius through the sub-deck hole
RING_BORE = 26.2            # top lip round the cap
RING_LIP_T = 2.0
RING_BAND_R = 27.0          # inner radius between the lip and the gear teeth
RING_CHAMFER = 2.0
FLUTES, FLUTE_D = 72, 1.4
FLUTE_Z = (DECK_TOP + 1.0, RING_TOP - 3.0)
RET_GAP = 0.2               # retainer top below the sub-deck underside
RET_T = 1.5
RET_R = (30.6, 34.6)
RET_SCREWS = [90.0, 210.0, 330.0]  # M2 x 6 into the ring, at radius 31.7
RET_SCREW_R = 31.7

# Gear pair, module 1: internal teeth on the ring, pinion on the encoder shaft
GEAR_M = 1.0
RING_TEETH, PINION_TEETH = 56, 20   # 2.8 encoder turns per ring turn
GEAR_Z = (19.3, 24.9)       # ring teeth band
GEAR_BAND_R = 30.4          # outer radius of the toothed band below the deck
PINION_Z = (20.0, 24.6)
BACKLASH_CD = 0.35          # pinion moved this much towards the axis: tooth play for printed gears
                            # (an internal pair gets play from a SMALLER centre distance)
PINION_PHASE = 0.5          # pinion turned by this many tooth pitches so it meshes
SHAFT_C = (-((RING_TEETH - PINION_TEETH) * GEAR_M / 2 - BACKLASH_CD), 0.0)

# HW-040 + EC11, hanging from a bracket by its own bushing nut (no board hole positions needed)
EC11_BODY = (12.0, 11.6)    # NOMINAL
EC11_BODY_H = 6.5           # NOMINAL
EC11_BUSH_D, EC11_BUSH_L = 7.0, 7.0   # NOMINAL
EC11_SHAFT_D, EC11_SHAFT_H = 6.0, 19.7  # shaft top above the board front, NOMINAL
EC11_FLAT = 1.5
EC11_PUSH = 0.5             # push-switch travel, NOMINAL
NUT = (11.0, 2.0)
HW040 = (26.0, 18.5, 1.3)
HW040_SHAFT = (-2.3, 1.8)   # measured: shaft from board centre (+x header end, -y holes)
HW040_HDR_X = 10.35
HW040_ROT = -90.0           # board turned so the header points to the front
BRACKET_Z0 = 11.8           # EC11 body top = bracket underside
BRACKET_T = 2.0
BRACKET_LEGS = [(-6.0, 4.0), (-18.0, 14.2)]
BRACKET_LEG_D = 6.0
ENC_HOLE_D = 7.4

# Cap (static, hinged at the back) carrying the display under a round window
CAP_TOP = 32.0
CAP_R = 25.9
CAP_FACE_T = 1.6
CAP_SKIRT_T = 1.2
CAP_BOTTOM = 25.6
CAP_FLANGE_R = 26.7         # under the ring lip: keeps the cap in
WINDOW = 24.0               # square window over the 23.4 mm active area
WINDOW_R = 1.5
WINDOW_CHAMFER = 0.8

# Display: ST7789 1.3" 240x240 (GMT130), header on the right (same as v1: firmware rotation stays)
DISP_C = (0.0, 0.0)
DISP_PCB = (39.22, 27.78, 1.6)
DISP_GLASS = (27.6, 26.16, 1.4)  # NOMINAL
DISP_ACTIVE_U = -0.5        # NOMINAL
DISP_GLASS_U = -1.6         # NOMINAL
DISP_HDR_U = 2.2
DISP_POCKET = 0.6
DISP_POCKET_CLR = 0.3
CLAMP_T = 2.0
CLAMP_LIP = 1.5
CLAMP_BOSSES = [(-22.5, 8.0), (-22.5, -8.0), (3.0, 19.5), (7.0, -19.5)]
CLAMP_BOSS_D = 5.0
CLAMP_PRELOAD = 0.2
CLAMP_PILOT, CLAMP_CLR = 1.7, 2.4  # M2 x 6 self-tapping
NUB_D = 5.0                 # under the clamp: presses the encoder shaft

# Hinge at the back: as far from the shaft as from the cap centre, so a press in the middle
# needs the same force as the encoder's own push switch
HINGE_Y, HINGE_Z = 19.0, 26.8
HINGE_PIN_D = 2.5
HINGE_LUG_X = (11.5, 13.5)
HINGE_POST_X = (7.5, 11.1)
HINGE_POST_Y = (16.5, 21.0)
HINGE_HOLE_D, HINGE_SLOT = 2.65, 2.0

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
TILE_SKIRT_T, TILE_SKIRT_Z = 1.2, 20.4  # lip over the wall top: thickness, bottom
TILE_LEGS = [(44.0, 6.0), (6.0, 44.0), (42.0, 42.0)]  # for the +x +y tile
LEG_D, LEG_HOLE_D, LEG_BOSS_D, LEG_BOTTOM = 4.0, 4.3, 7.5, 15.0
SOCK_CLR = 0.025            # socket = switch tip + this, per side (a light grip)
SOCK_WALL = 0.7             # flexing walls round the socket
SOCK_HOOK = (0.3, 0.6)      # hooks under the tip: depth, height (45° lead-in below, flat on top)
SOCK_SLIT = 0.5             # slits at the socket corners free the walls ...
SOCK_SLIT_TOP = DECK_TOP - 1.0  # ... up into the tile, leaving a 1 mm face
SW_HOLE_D = 9.0             # socket through the sub-deck (room for the walls to flex)
SW_BODY, SW_BODY_H, SW_TOTAL_H = 12.0, 3.5, 10.0
SW_BOSS_D, SW_BOSS_H = 7.1, 4.3
SW_NECK, SW_HEAD, SW_HEAD_H = 3.0, 4.0, 1.8  # stem: neck, square tip, tip height (NOMINAL)
SW_HEAD_CLR = 0.15
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]
SW_PLUNGER_TOP = 21.0       # switch tip top = socket ceiling (tile resting on its switch)

# ESP32-C3 SuperMini on the floor, components up, USB-C out of the right wall
SM = (18.0, 22.52, 1.0)     # NOMINAL
SM_USB_END = S / 2 - WALL_INSET - WALL - 0.5
SM_Z = FLOOR_T + 1.3        # board underside
SM_USB = (8.94, 7.35, 3.2)  # NOMINAL
USB_SLOT = (13.0, 7.5, 3.0)  # width, height, 45° corner chamfer

# Assembly screws: M3 x 8 self-tapping, through the floor into bosses in the walls
BOSSES = [(0.0, 44.5), (0.0, -44.5), (-44.5, 0.0), (44.5, 17.0)]
BOSS_D, BOSS_TOP = 6.0, 10.0
SCREW_PILOT, SCREW_CLR, SCREW_CSK = 2.5, 3.4, 6.4
FOOT_RING = (10.5, 0.8, 0.4)  # marker rings for 10 mm stick-on feet: diameter, width, depth

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


def rr_prism(w, h, r, z0, z1):
    return Part.Face(rr_wire(w, h, r, z0)).extrude(V(0, 0, z1 - z0))


def box(x0, y0, z0, x1, y1, z1):
    return Part.makeBox(abs(x1 - x0), abs(y1 - y0), abs(z1 - z0), V(min(x0, x1), min(y0, y1), min(z0, z1)))


def cbox(cx, cy, w, h, z0, z1):
    return box(cx - w / 2, cy - h / 2, z0, cx + w / 2, cy + h / 2, z1)


def cyl(d, cx, cy, z0, z1):
    return Part.makeCylinder(d / 2.0, z1 - z0, V(cx, cy, z0), V(0, 0, 1))


def xcyl(d, y, z, x0, x1):
    return Part.makeCylinder(d / 2.0, x1 - x0, V(x0, y, z), V(1, 0, 0))


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


def octagon_slot_x(w, h, c, zc, x0, x1):
    """Slot through a wall along x, w along y, h along z, 45° corners (no overhang printed)."""
    a, b = w / 2, h / 2
    pts = [(-a + c, -b), (a - c, -b), (a, -b + c), (a, b - c), (a - c, b), (-a + c, b), (-a, b - c), (-a, -b + c)]
    f = Part.Face(Part.makePolygon([V(x0, py, zc + pz) for (py, pz) in pts] + [V(x0, pts[0][0], zc + pts[0][1])]))
    return f.extrude(V(x1 - x0, 0, 0))


# ---------------------------------------------------------------------------------------------
# Derived layout
# ---------------------------------------------------------------------------------------------

BODY, BODY_R = S - 2 * WALL_INSET, S_R - WALL_INSET  # walls and floor
subdeck_under = SUBDECK_TOP - SUBDECK_T
tile_under = DECK_TOP - TILE_T
ret_top = subdeck_under - RET_GAP
enc_board_front = BRACKET_Z0 - EC11_BODY_H
enc_shaft_top = enc_board_front + EC11_SHAFT_H
bush_top = BRACKET_Z0 + EC11_BUSH_L

dpcb_c = (DISP_C[0] - DISP_ACTIVE_U, DISP_C[1])
glass_c = (dpcb_c[0] + DISP_GLASS_U, DISP_C[1])
cap_under = CAP_TOP - CAP_FACE_T
glass_z1 = cap_under + DISP_POCKET
glass_z0 = glass_z1 - DISP_GLASS[2]
dpcb_z0 = glass_z0 - DISP_PCB[2]             # board back
clamp_z0 = dpcb_z0 - CLAMP_T
disp_hdr_x = dpcb_c[0] + DISP_PCB[0] / 2 - DISP_HDR_U

c_btn = BTN_D / math.sqrt(2)
BTNS = [(sx * c_btn, sy * c_btn) for (sx, sy) in ((-1, 1), (1, 1), (-1, -1), (1, -1))]
sw_base = SW_PLUNGER_TOP - SW_TOTAL_H

sm_c = (SM_USB_END - SM[1] / 2, 0.0)         # board long axis along x
sm_usb_zc = SM_Z + SM[2] + SM_USB[2] / 2


def hw_place(shape):
    """HW-040 local frame (shaft at the origin) -> world."""
    s = shape.copy()
    s.rotate(V(0, 0, 0), V(0, 0, 1), HW040_ROT)
    s.translate(V(SHAFT_C[0], SHAFT_C[1], 0))
    return s


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


# ---------------------------------------------------------------------------------------------
# Parts
# ---------------------------------------------------------------------------------------------


def make_shell():
    """Walls and sub-deck, printed sub-deck down."""
    s = rr_prism(BODY, BODY, BODY_R, FLOOR_T, SUBDECK_TOP)
    s = s.cut(rr_prism(BODY - 2 * WALL, BODY - 2 * WALL, BODY_R - WALL, FLOOR_T - 1, subdeck_under))
    adds = []
    for (x, y) in BOSSES:  # screw bosses with a 45° top so they print upside down
        boss = cyl(BOSS_D, x, y, FLOOR_T, BOSS_TOP)
        boss = boss.fuse(Part.makeCone(BOSS_D / 2, 0.01, BOSS_D / 2, V(x, y, BOSS_TOP), V(0, 0, 1)))
        adds.append(boss.common(rr_prism(BODY - 1, BODY - 1, BODY_R, 0, SUBDECK_TOP)))
    for legs in LEGS:  # guide bosses for the tile legs
        adds += [cyl(LEG_BOSS_D, x, y, LEG_BOTTOM, subdeck_under + 0.01) for (x, y) in legs]
    s = s.fuse(adds)
    cuts = [cyl(2 * RING_HOLE, 0, 0, subdeck_under - 1, SUBDECK_TOP + 1)]
    cuts += [cyl(SW_HOLE_D, x, y, subdeck_under - 1, SUBDECK_TOP + 1) for (x, y) in BTNS]
    for legs in LEGS:
        cuts += [cyl(LEG_HOLE_D, x, y, LEG_BOTTOM - 1, SUBDECK_TOP + 1) for (x, y) in legs]
    cuts += [cyl(SCREW_PILOT, x, y, FLOOR_T - 1, BOSS_TOP - 1) for (x, y) in BOSSES]
    xw = BODY / 2
    cuts.append(octagon_slot_x(USB_SLOT[0], USB_SLOT[1], USB_SLOT[2], sm_usb_zc, xw - WALL - 2, xw + 1))
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
    r = fuse_all([above, lip, neck, teeth])
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
    """Floor with everything static: hinge posts, switch pedestals, SuperMini seat, bracket sockets."""
    p = rr_prism(BODY, BODY, BODY_R, 0, FLOOR_T)
    try:
        p = p.makeChamfer(0.6, [e for e in p.Edges if abs(e.BoundBox.ZMax) < 1e-6])
    except Exception as exc:
        print("floor chamfer skipped:", exc)
    adds = []
    for s in (-1, 1):  # hinge posts with a snap-in C hole for the cap's pins
        x0, x1 = sorted((s * HINGE_POST_X[0], s * HINGE_POST_X[1]))
        post = box(x0, HINGE_POST_Y[0], FLOOR_T - 0.01, x1, HINGE_POST_Y[1], HINGE_Z + 1.8)
        post = post.cut(xcyl(HINGE_HOLE_D, HINGE_Y, HINGE_Z, x0 - 1, x1 + 1))
        post = post.cut(box(x0 - 1, HINGE_Y - HINGE_SLOT / 2, HINGE_Z, x1 + 1, HINGE_Y + HINGE_SLOT / 2, HINGE_Z + 5))
        adds.append(post)
    for (x, y) in BTNS:  # switch pedestals: pocket, leg holes, wire exit
        ped = cbox(x, y, 14.0, 14.0, FLOOR_T - 0.01, sw_base + 0.8)
        ped = ped.cut(cbox(x, y, SW_BODY + 0.3, SW_BODY + 0.3, sw_base, sw_base + 2))
        ped = ped.cut([cyl(1.8, x + dx, y + dy, sw_base - 5, sw_base + 1) for (dx, dy) in SW_PADS])
        ped = ped.cut(cbox(x, y, 16.0, 4.0, sw_base - 4.0, sw_base - 1.5))
        adds.append(ped.common(rr_prism(BODY - 2 * WALL - 0.4, BODY - 2 * WALL - 0.4, BODY_R - WALL, 0, DECK_TOP)))
    # SuperMini seat: blocks under both ends, side guides, a stop wall that takes the plug force
    x_f = SM_USB_END - SM[1]
    adds.append(box(x_f, -6.0, FLOOR_T - 0.01, x_f + 3.0, 6.0, SM_Z))
    adds.append(box(SM_USB_END - 3.0, -4.0, FLOOR_T - 0.01, SM_USB_END - 0.5, 4.0, SM_Z))
    adds.append(box(x_f - 2.0, -6.0, FLOOR_T - 0.01, x_f - 0.15, 6.0, SM_Z + SM[2] + 1.5))
    for s in (-1, 1):
        y0 = s * (SM[0] / 2 + 0.15)
        adds.append(box(x_f - 2.0, y0, FLOOR_T - 0.01, x_f + 6.0, y0 + s * 1.5, SM_Z + SM[2] + 1.0))
    p = p.fuse(adds)
    cuts = []
    for (x, y) in BOSSES:
        cuts.append(cyl(SCREW_CLR, x, y, -1, FLOOR_T + 1))
        cuts.append(Part.makeCone(SCREW_CSK / 2, SCREW_CLR / 2, (SCREW_CSK - SCREW_CLR) / 2, V(x, y, -0.01), V(0, 0, 1)))
    cuts += [cyl(3.0 + 0.2, x, y, FLOOR_T - 1.5, FLOOR_T + 1) for (x, y) in BRACKET_LEGS]
    d, wd, dp = FOOT_RING
    for sx in (-1, 1):
        for sy in (-1, 1):
            fx, fy = sx * (BODY / 2 - 9), sy * (BODY / 2 - 9)
            cuts.append(cyl(d, fx, fy, -1, dp).cut(cyl(d - 2 * wd, fx, fy, -2, dp + 1)))
    t = text_solid("KIVORI", 0.0, 0.0, 7.0, -0.1, TEXT_D + 0.1, mirror=True)
    if t:
        cuts.append(t)
    return p.cut(cuts).removeSplitter()


def make_bracket():
    """Holds the EC11 by its bushing: the nut clamps it, so the gear centre distance is exact."""
    z0, z1 = BRACKET_Z0, BRACKET_Z0 + BRACKET_T
    plate = box(-24.0, -7.0, z0, -3.0, 16.0, z1).common(cyl(2 * 29.8, 0, 0, z0 - 1, z1 + 1))
    legs = [cyl(BRACKET_LEG_D, x, y, FLOOR_T, z0 + 0.01) for (x, y) in BRACKET_LEGS]
    pegs = [cyl(3.0, x, y, FLOOR_T - 1.4, FLOOR_T + 0.01) for (x, y) in BRACKET_LEGS]
    b = fuse_all([plate] + legs + pegs)
    return b.cut(cyl(ENC_HOLE_D, SHAFT_C[0], SHAFT_C[1], z0 - 1, z1 + 1)).removeSplitter()


def make_pinion():
    g = gear_face(PINION_TEETH, False).extrude(V(0, 0, PINION_Z[1] - PINION_Z[0]))
    g.rotate(V(0, 0, 0), V(0, 0, 1), PINION_PHASE * 360.0 / PINION_TEETH)
    g.translate(V(SHAFT_C[0], SHAFT_C[1], PINION_Z[0]))
    # D bore built in the encoder's own frame, so its flat matches the turned shaft
    bore = cyl(EC11_SHAFT_D + 0.15, 0, 0, PINION_Z[0] - 1, PINION_Z[1] + 1)
    flat = box(-5, EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, PINION_Z[0] - 2, 5, 5, PINION_Z[1] + 2)
    return g.cut(hw_place(bore.cut(flat))).removeSplitter()


def make_cap():
    face = cyl(2 * CAP_R, 0, 0, cap_under, CAP_TOP)
    try:
        face = face.makeChamfer(0.6, [e for e in face.Edges if abs(e.BoundBox.ZMin - CAP_TOP) < 1e-6])
    except Exception as exc:
        print("cap chamfer skipped:", exc)
    skirt = ring_solid(CAP_R - CAP_SKIRT_T, CAP_R, CAP_BOTTOM, cap_under + 0.01)
    rf = CAP_FLANGE_R  # flange under the ring lip; 45° top so the cap prints face down
    flange = revolve([(CAP_R - 0.5, cap_under + 0.3), (CAP_R - 0.01, cap_under + 0.3),
                      (rf, cap_under + 0.3 - (rf - CAP_R)), (rf, CAP_BOTTOM + 3.0), (CAP_R - 0.5, CAP_BOTTOM + 3.0)])
    adds = [skirt, flange]
    adds += [cyl(CLAMP_BOSS_D, x, y, dpcb_z0 + CLAMP_PRELOAD, cap_under + 0.01) for (x, y) in CLAMP_BOSSES]
    for s in (-1, 1):
        x0, x1 = sorted((s * HINGE_LUG_X[0], s * HINGE_LUG_X[1]))
        lug = box(x0, HINGE_Y - 1.75, CAP_BOTTOM, x1, HINGE_Y + 1.75, cap_under + 0.01)
        pin = xcyl(HINGE_PIN_D, HINGE_Y, HINGE_Z, *sorted((s * HINGE_LUG_X[0], s * (HINGE_POST_X[0] + 0.4))))
        adds.append(lug.fuse(pin).common(cyl(2 * (CAP_R - 0.3), 0, 0, 0, 40)).fuse(pin))
    c = face.fuse(adds)
    win = rr_prism(WINDOW, WINDOW, WINDOW_R, cap_under - 1, CAP_TOP + 1)
    cham = Part.makeLoft([rr_wire(WINDOW, WINDOW, WINDOW_R, CAP_TOP - WINDOW_CHAMFER),
                          rr_wire(WINDOW + 2 * WINDOW_CHAMFER, WINDOW + 2 * WINDOW_CHAMFER,
                                  WINDOW_R + WINDOW_CHAMFER, CAP_TOP + 0.01)], True)
    win = win.fuse(cham)
    win.translate(V(DISP_C[0], DISP_C[1], 0))
    cuts = [win]
    k = 2 * DISP_POCKET_CLR
    cuts.append(cbox(glass_c[0], glass_c[1], DISP_GLASS[0] + k, DISP_GLASS[1] + k, cap_under - 5, glass_z1))
    cuts += [cyl(CLAMP_PILOT, x, y, dpcb_z0 - 1, cap_under - 0.6) for (x, y) in CLAMP_BOSSES]
    return c.cut(cuts).removeSplitter()


def make_clamp():
    """Presses the display board against the cap (open round the header) and carries the nub
    that pushes the encoder shaft."""
    z0, z1 = clamp_z0, dpcb_z0
    hl, hw = DISP_PCB[0] / 2, DISP_PCB[1] / 2
    cx, cy = dpcb_c
    f = box(-24.5, -22.0, z0, 12.0, 22.0, z1).common(cyl(2 * (CAP_R - CAP_SKIRT_T - 0.3), 0, 0, z0 - 1, z1 + 1))
    f = f.cut(box(cx - hl + CLAMP_LIP, cy - hw + CLAMP_LIP, z0 - 1, cx + hl + 3.0, cy + hw - CLAMP_LIP, z1 + 1))
    f = f.cut(box(HINGE_POST_X[0] - 0.5, HINGE_POST_Y[0] - 0.5, z0 - 1, 30, 30, z1 + 1))
    f = f.cut(box(-30, HINGE_POST_Y[0] - 0.5, z0 - 1, -HINGE_POST_X[0] + 0.5, 30, z1 + 1))
    pad = cbox(SHAFT_C[0], SHAFT_C[1], NUB_D + 2.0, NUB_D + 2.0, z0, z1)
    nub = cyl(NUB_D, SHAFT_C[0], SHAFT_C[1], enc_shaft_top, z0 + 0.01)
    f = f.fuse([pad, nub])
    f = f.cut([cyl(CLAMP_CLR, x, y, z0 - 1, z1 + 1) for (x, y) in CLAMP_BOSSES])
    return f.removeSplitter()


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
    boss = cbox(x, y, sock + 2 * SOCK_WALL, sock + 2 * SOCK_WALL, z_bot, tile_under + 0.01)
    t = fuse_all([plate, lip, boss] + legs)
    cuts = [cbox(x, y, sock, sock, z_bot - 1, SW_PLUNGER_TOP)]
    o = sock / 2 + SOCK_WALL / 2  # slits at the four corners, up into the tile
    cuts += [cbox(x + sx * o, y + sy * o, SOCK_SLIT + SOCK_WALL, SOCK_SLIT + SOCK_WALL, z_bot - 1, SOCK_SLIT_TOP)
             for sx in (-1, 1) for sy in (-1, 1)]
    # a gap round the socket up into the tile, so the walls flex from 1 mm under the face
    bo = sock + 2 * SOCK_WALL
    cuts.append(cbox(x, y, bo + 1.2, bo + 1.2, tile_under - 0.01, SOCK_SLIT_TOP).cut(
        cbox(x, y, bo, bo, tile_under - 1, SOCK_SLIT_TOP + 1)))
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


# Stand-ins for the bought parts


def dummy_encoder():
    bc = (-HW040_SHAFT[0], -HW040_SHAFT[1])  # board centre relative to the shaft
    t = enc_board_front
    pcb = cbox(bc[0], bc[1], HW040[0], HW040[1], t - HW040[2], t)
    body = cbox(0, 0, EC11_BODY[0], EC11_BODY[1], t, BRACKET_Z0)
    bush = cyl(EC11_BUSH_D, 0, 0, BRACKET_Z0, bush_top)
    shaft = cyl(EC11_SHAFT_D, 0, 0, bush_top, enc_shaft_top)
    shaft = shaft.cut(box(-5, EC11_SHAFT_D / 2 - EC11_FLAT, enc_shaft_top - 12, 5, 5, enc_shaft_top + 1))
    nut = cyl(NUT[0], 0, 0, BRACKET_Z0 + BRACKET_T, BRACKET_Z0 + BRACKET_T + NUT[1])
    hx = bc[0] + HW040_HDR_X
    hdr = cbox(hx, 0, 2.5, 5 * 2.54, t, t + 2.5)
    pins = box(hx, -5.6, t + 1.0, bc[0] + HW040[0] / 2 + 6.0, 5.6, t + 1.64)
    legs = cbox(0, 0, 14.0, 8.0, t - HW040[2] - 1.5, t - HW040[2])
    return hw_place(fuse_all([pcb, body, bush, shaft, nut, hdr, pins, legs]))


def dummy_display():
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], glass_z0, glass_z1)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], dpcb_z0, glass_z0)
    hdr = cbox(disp_hdr_x, dpcb_c[1], 2.5, 7 * 2.54, dpcb_z0 - 2.5, dpcb_z0)
    pins = cbox(disp_hdr_x, dpcb_c[1], 0.64, 7 * 2.54 - 1.9, dpcb_z0 - 8.5, dpcb_z0 - 2.5)
    return fuse_all([glass, pcb, hdr, pins])


def dummy_switch(i):
    x, y = BTNS[i]
    body = cbox(x, y, SW_BODY, SW_BODY, sw_base, sw_base + SW_BODY_H)
    boss = cyl(SW_BOSS_D, x, y, sw_base + SW_BODY_H - 0.01, sw_base + SW_BOSS_H)
    neck = cbox(x, y, SW_NECK, SW_NECK, sw_base + SW_BOSS_H - 0.01, SW_PLUNGER_TOP - SW_HEAD_H + 0.01)
    head = cbox(x, y, SW_HEAD, SW_HEAD, SW_PLUNGER_TOP - SW_HEAD_H, SW_PLUNGER_TOP)
    legs = [cyl(0.7, x + dx, y + dy, sw_base - 3.5, sw_base) for (dx, dy) in SW_PADS]
    return fuse_all([body, boss, neck, head] + legs)


def dummy_supermini():
    z = SM_Z
    pcb = cbox(sm_c[0], sm_c[1], SM[1], SM[0], z, z + SM[2])
    usb = cbox(SM_USB_END - SM_USB[1] / 2 + 0.5, 0, SM_USB[1], SM_USB[0], z + SM[2], z + SM[2] + SM_USB[2])
    chip = cbox(sm_c[0] - 3.0, 0, 5.0, 5.0, z + SM[2], z + SM[2] + 0.9)
    return fuse_all([pcb, usb, chip])


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}
BUTTON_NAMES = ["TileBackLeft", "TileBackRight", "TileFrontLeft", "TileFrontRight"]
COLORS = {
    "Ring": (0.80, 0.80, 0.82), "Shell": (0.13, 0.13, 0.15), "Floor": (0.13, 0.13, 0.15),
    "Retainer": (0.12, 0.45, 0.25), "Cap": (0.04, 0.04, 0.05), "Clamp": (0.12, 0.45, 0.25),
    "Bracket": (0.12, 0.45, 0.25), "Pinion": (0.95, 0.50, 0.18), "Button": (0.95, 0.50, 0.18),
    "Dummy": (0.30, 0.45, 0.85),
}
FLIP = ("Ring", "Shell", "Cap", "Bracket", "Clamp") + tuple(BUTTON_NAMES)  # printed upside down


def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["Ring"] = make_ring()
    PRINTED["Retainer"] = make_retainer()
    PRINTED["Floor"] = make_floor()
    PRINTED["Bracket"] = make_bracket()
    PRINTED["Pinion"] = make_pinion()
    PRINTED["Cap"] = make_cap()
    PRINTED["Clamp"] = make_clamp()
    for i, n in enumerate(BUTTON_NAMES):
        PRINTED[n] = make_tile(i)
        DUMMIES["Dummy_" + n.replace("Tile", "Switch")] = dummy_switch(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_SuperMini"] = dummy_supermini()


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

    for name, shp in PRINTED.items():
        mesh = MeshPart.meshFromShape(Shape=print_pose(name, shp), LinearDeflection=0.02,
                                      AngularDeflection=math.radians(10), Relative=False)
        mesh.write(os.path.join(exp, "%s.stl" % name))
    doc.saveAs(os.path.join(here, "kivori-case-experiment.FCStd"))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
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
