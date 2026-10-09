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
T_NEAR, T_FAR = 18.0, 30.0  # case thickness at the near (y=0) and far (y=H) edge -> ~9.7° wedge
WALL = 2.0                  # side wall thickness (horizontal)
WALL_TAPER = 6.0            # degrees the walls lean in towards the face (keycap draft)
FACE_T = 2.5                # faceplate thickness (room for the visor pocket in front)
FACE_BEVEL = 2.0            # 45° bevel round the face; prints cleanly face-down
BP_T = 2.0                  # back plate thickness
BP_CLR = 0.2                # back plate to wall clearance (per side)

# Knob + encoder (EC11 dimensions from KiCad RotaryEncoder_Alps_EC11E-Switch_Vertical_H20mm)
KNOB_C = (27.5, 35.0)       # knob / shaft centre on the face
KNOB_D = 48.0
KNOB_H = 12.0
KNOB_GAP = 1.0              # air gap between knob skirt and face (push travel is ~0.5)
KNOB_FLUTES, KNOB_FLUTE_D = 60, 1.2   # fine vertical grip ribs round the skirt
KNOB_MARK = (13.0, 20.5, 135.0, 1.2)   # radial line marker: from r, to r, angle, width
KNOB_HALO = (24.7, 25.7, 0.6)  # thin groove ring in the face round the knob: r in, r out, depth
EC11_BODY = (12.0, 11.6)    # NOMINAL body footprint
EC11_BODY_H = 6.5           # NOMINAL body height above its PCB
EC11_BUSH_D, EC11_BUSH_L = 7.0, 7.0   # M7 bushing, NOMINAL length
EC11_SHAFT_D, EC11_SHAFT_H = 6.0, 19.7  # D shaft, top 21 mm above the HW-040 board underside
EC11_FLAT = 1.5             # D-flat depth
ENC_HOLE_D = 7.4            # face hole for the bushing
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

# Display: ST7789 1.3" 240x240, 7-pin module, mounted rotated 90° (header on the right)
DISP_C = (78.0, 46.0)       # centre of the active area on the face
DISP_ACTIVE = 23.4          # active area (square), several sources agree
DISP_WIN_MARGIN = 0.3       # window opening beyond the active area, per side
DISP_WIN_CHAMFER = 0.8
# Module frame: u runs along the 39.22 mm length, + towards the header end. Positions from a
# product photo scaled to the 27.78 mm board width; NOMINAL to about ±0.5 mm.
DISP_PCB = (39.22, 27.78, 1.6)    # module PCB (length, width, thickness); 3.0 total with glass
DISP_GLASS = (27.6, 26.16, 1.4)   # glass incl. the FPC ledge at the end away from the header
DISP_ACTIVE_U = -0.5              # active-area centre from the board centre, along u
DISP_GLASS_U = -1.6               # glass centre from the board centre, along u (ledge side)
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
BTN_CAP_DISH = 0.4          # keycap-style cylindrical dish in the cap top
# 12x12 projected-plunger switch, seller drawing TS-G005 (7.5 mm version; this one is 10 mm tall)
SW_BODY, SW_BODY_H, SW_TOTAL_H = 12.0, 3.5, 10.0  # housing 12 x 12 x 3.5; head top 10 mm (seller)
SW_BOSS_D, SW_BOSS_H = 7.1, 4.3   # round platform on the housing, top 4.3 mm above the PCB
SW_NECK = 3.0               # stem neck, square, NOMINAL from the drawing
SW_HEAD, SW_HEAD_H = 4.0, 1.8  # square head, wider than the neck (caps slip over it), with a centre hole
SW_HEAD_CLR = 0.15          # cap socket clearance per side; the cap is held by its flange, not the stem
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]  # pin 1,1,2,2 (land pattern Ø1.5)
SW_PEGS = [(0.0, -4.5), (0.0, 4.5)]  # clearance for locating bosses some 12x12 switches have (B3F)
SW_PEG_HOLE_D = 2.0
BTN_GPIO = ["IO0", "IO1", "IO20"]  # left, middle, right

# Printed PCB ("carrier"): same plane as a future real PCB
C_FRONT = -(FACE_T + EC11_BODY_H)  # -8.5: EC11 body sits on it and touches the face
C_T = 2.0                   # printed thickness (a real board is 1.6, it grows backwards); leaves a 0.6 mm HW-040 pocket floor
C_BACK = C_FRONT - C_T
C_INSET = 0.8               # carrier outline inset from the inner walls
HOLE_D = 1.2                # 2.54 mm header holes
SW_HOLE_D = 1.8             # switch legs
POSTS = [(7.0, 7.0), (7.0, 63.0), (104.0, 64.5), (104.5, 26.5)]  # face posts locating the carrier
POST_D, PIN_D, PIN_HOLE_D, PIN_L = 5.0, 2.4, 2.9, 2.0
PEG_CLR = 0.1               # peg diameter = hole - 2 * clearance (press fit)
PEG_PROUD = 1.0             # peg tip past the module board: press fit, or melt it (heat-stake)
PEG_RELIEF = 0.7            # face-back relief over the display peg tips (room for a staked head)

# ESP32-C3 SuperMini, behind the carrier, components facing the back plate
SM = (18.0, 22.52, 1.0)     # outline, NOMINAL thickness
SM_X = 49.0                 # board centre x: keeps the pin rows clear of the display board
SM_USB_END = H - WALL - 0.5  # y of the USB end of the board
SM_ROW = 15.24              # header row spacing
SM_PIN_MARGIN = (SM[1] - 7 * 2.54) / 2  # NOMINAL first pin from the USB end
# Columns as seen from the BACK (component side up, USB at the top). Seen from the face they
# mirror, so the "left" column sits at +x in the face frame.
SM_COL_A = ["5V", "GND", "3V3", "IO4", "IO3", "IO2", "IO1", "IO0"]    # at x = SM_X + row/2
SM_COL_B = ["IO5", "IO6", "IO7", "IO8", "IO9", "IO10", "IO20", "IO21"]  # at x = SM_X - row/2
SM_USB = (8.94, 7.35, 3.2)  # receptacle (width, depth, height), NOMINAL
SM_BTN_FROM_USB = 9.5       # NOMINAL BOOT / RST distance from the USB end
SM_BTN_DX = 4.5             # NOMINAL offset either side of the board centre line
SM_BTN = (3.0, 4.0, 1.5)
USB_CUT = (13.0, 7.0)       # far-wall slot for a moulded USB-C plug

# Back plate
PINHOLE_D = 4.0             # blueprint Ø4, paperclip / SIM pin
TUBE_OD = 7.0
TUBE_GAP = 0.6              # tube top below the lowest part around it
COLUMN_D = 5.0              # columns clamping the carrier against the face posts
HOOKS = [(-1, 18.0), (-1, 52.0), (+1, 18.0), (+1, 52.0)]  # (side, y centre)
HOOK_W, HOOK_T, HOOK_L = 8.0, 1.4, 10.0
HOOK_BARB = 1.0             # barb with 45° ramps both ways: snaps in, pulls out (a detent)
HOOK_GAP = 1.2              # arm to inner wall
RIB_DEPTH, RIB_H = 1.0, 1.5  # catch rib on the inside of the wall: no windows, nothing visible

# Floating plinth under the shell (part of the back plate, printed dark)
PLINTH_H = 3.0
PLINTH_INSET = 2.0          # inset from the face outline; the flared walls make the shadow ~4-5 mm
PLINTH_R = 6.0
FOOT_D, FOOT_DEPTH = 10.5, 1.0  # recesses for 10 mm stick-on rubber feet

# Dark visor: separate black panel standing 1 mm proud of the face (a recess would need a 60 mm
# bridge in the face-down print), carrying the display window and the button openings
VISOR_T = 1.0
VISOR_BOX = (44.0, 5.5, 106.5, 62.5)  # x0, y0, x1, y1 before the knob cut-out
VISOR_R = 6.0
VISOR_KNOB_R = 26.3         # concentric cut-out round the knob (even gap ring)
VISOR_EDGE = 0.5            # chamfer round the visor top edge
VISOR_PEGS = [(50.0, 60.0), (103.5, 60.0), (104.5, 32.0)]  # locating pegs into the face
VISOR_PEG_D, VISOR_PEG_H, VISOR_HOLE_D, VISOR_HOLE_DEPTH = 1.8, 1.0, 2.1, 1.2
UNDER_CLR = 0.3             # face openings behind the visor are this much bigger per side

FONT = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
TEXT_D = 0.4

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
    except Exception as exc:  # text is cosmetic, never fail the build on it
        print("text skipped:", s, exc)
        return None


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
hw_wire_slot = (hw_pin_tip_x + 1.0, hw_pin_tip_x + 4.0)        # wires drop to the back through this
enc_pins = {n: ((hw_wire_slot[0] + hw_wire_slot[1]) / 2, hw_hdr[1] + (2 - i) * 2.54)
            for i, n in enumerate(["CLK", "DT", "SW", "+", "GND"])}
hw_floor = C_FRONT - HW040[2] - 0.1        # HW-040 pocket floor

sm_c = (SM_X, SM_USB_END - SM[1] / 2)
sm_z_comp = C_BACK - SM[2]                 # component side; the board lies on the flat carrier back
sm_pins = {}
for i, n in enumerate(SM_COL_A):
    sm_pins[n] = (SM_X + SM_ROW / 2, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
for i, n in enumerate(SM_COL_B):
    sm_pins[n] = (SM_X - SM_ROW / 2, SM_USB_END - SM_PIN_MARGIN - i * 2.54)
sm_btns = [(SM_X - SM_BTN_DX, SM_USB_END - SM_BTN_FROM_USB), (SM_X + SM_BTN_DX, SM_USB_END - SM_BTN_FROM_USB)]
usb_z = sm_z_comp - SM_USB[2] / 2
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
    outer = Part.makeLoft([rr_wire(W + 2 * 60 * TAPER, H + 2 * 60 * TAPER, R_CORNER + 60 * TAPER, -60),
                           rr_wire(W + 2 * c * TAPER, H + 2 * c * TAPER, R_CORNER + c * TAPER, -c),
                           rr_wire(W - 2 * c, H - 2 * c, R_CORNER - c, 0)], True, True)
    outer = place(outer, W / 2, H / 2).common(above_back(0))
    cavity = tapered(W - 2 * WALL, H - 2 * WALL, R_CORNER - WALL, 0, -60)
    cavity = cavity.common(box(-50, -50, -60, W + 50, H + 50, -FACE_T))
    shell = outer.cut(cavity)

    adds = []
    for (x, y) in POSTS:
        adds.append(cyl(POST_D, x, y, C_FRONT, -FACE_T + 0.01))
        adds.append(cyl(PIN_D, x, y, C_FRONT - PIN_L, C_FRONT + 0.01))
    for (side, yc) in HOOKS:  # catch ribs for the back plate hooks
        x0, x1, z0, z1 = hook_geom(yc)[4]
        adds.append(mirror_x(box(x0, yc - HOOK_W / 2 - 0.4, z0, x1, yc + HOOK_W / 2 + 0.4, z1), side))
    shell = shell.fuse(adds)

    cuts = [cyl(ENC_HOLE_D, KNOB_C[0], KNOB_C[1], -FACE_T - 1, 1),
            cyl(2 * KNOB_HALO[1], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2], 1).cut(
                cyl(2 * KNOB_HALO[0], KNOB_C[0], KNOB_C[1], -KNOB_HALO[2] - 1, 2))]
    cuts += [cyl(VISOR_HOLE_D, x, y, -VISOR_HOLE_DEPTH, 1) for (x, y) in VISOR_PEGS]
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN + 2 * UNDER_CLR
    cuts.append(rr_prism(win, win, 1.0, DISP_C[0], DISP_C[1], -FACE_T - 1, 1))
    cuts.append(rr_prism(DISP_GLASS[0] + 0.5, DISP_GLASS[1] + 0.5, 0.5, glass_c[0], glass_c[1],
                         -FACE_T - 1, -FACE_T + DISP_POCKET))
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN + 2 * UNDER_CLR, BTN_OPEN + 2 * UNDER_CLR, 1.8, x, y, -FACE_T - 1, 1, a))
    for (x, y) in disp_holes:
        cuts.append(cyl(DISP_STANDOFF_D, x, y, -FACE_T - 1, -FACE_T + PEG_RELIEF))
    usb = Part.Face(rr_wire(USB_CUT[0], USB_CUT[1], USB_CUT[1] / 2)).extrude(V(0, 0, WALL + 10))
    usb.rotate(V(0, 0, 0), V(1, 0, 0), -90)
    usb.translate(V(SM_X, H - WALL - 2, usb_z))
    cuts.append(usb)
    return shell.cut(cuts).removeSplitter()


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
    for (x, y) in sm_btns:
        adds.append(cyl(TUBE_OD, x, y, z_back(y) + 0.5, tube_top))
    usb_y0 = max(SM_USB_END - SM_USB[1] + 1.5, sm_btns[0][1] + TUBE_OD / 2 + 0.5)
    adds.append(box(SM_X - 3.5, usb_y0, z_back(SM_USB_END) + 0.5, SM_X + 3.5, SM_USB_END - 0.5,
                    sm_z_comp - SM_USB[2] - 0.1))
    plate = plate.fuse([a.common(above_back(0)) for a in adds])

    # floating plinth, built in the desk frame (back plane = z 0)
    plinth = rr_prism(W - 2 * PLINTH_INSET, H_DESK - 2 * PLINTH_INSET, PLINTH_R, W / 2, H_DESK / 2, -PLINTH_H, 0.01)
    try:
        bottom = [e for e in plinth.Edges if abs(e.BoundBox.ZMax + PLINTH_H) < 1e-6]
        plinth = plinth.makeChamfer(0.6, bottom)
    except Exception as exc:
        print("plinth chamfer skipped:", exc)
    zb = -PLINTH_H
    under = []  # desk-side details, mirrored so they read correctly with the case turned over
    for (fx, fy) in ((10, 10), (W - 10, 10), (10, H_DESK - 10), (W - 10, H_DESK - 10)):
        under.append(cyl(FOOT_D, fx, fy, zb - 1, zb + FOOT_DEPTH))
    for t in (text_solid("KIVORI", W / 2, H_DESK / 2 + 3.0, 7.0, zb - 0.1, TEXT_D + 0.1, mirror=True),
              text_solid("DESK BUDDY   USB-C 5V", W / 2, H_DESK / 2 - 6.0, 2.4, zb - 0.1, TEXT_D + 0.1, mirror=True)):
        if t:
            under.append(t)
    sa, ca = math.sin(ALPHA), math.cos(ALPHA)
    for (x, y), label in zip(sm_btns, ["RST", "BOOT"]):  # seen from the back: BOOT left, RST right
        yd = (y + PLINTH_H * sa) / ca  # where the vertical pinhole leaves the plinth bottom
        t = text_solid(label, x, yd - 6.0, 2.4, zb - 0.1, TEXT_D + 0.1, mirror=True)
        if t:
            under.append(t)
    plinth = plinth.cut(under)
    plate = plate.fuse(desk_to_face(plinth))

    cuts = [cyl(PINHOLE_D, x, y, -60, tube_top + 1) for (x, y) in sm_btns]
    return plate.cut(cuts).removeSplitter()


def carrier_outline():
    return rr_prism(W - 2 * WALL - 2 * C_INSET, H - 2 * WALL - 2 * C_INSET, R_CORNER - WALL - C_INSET,
                    W / 2, H / 2, C_BACK, C_FRONT)


def carrier_through_cuts():
    """Everything that goes right through the board: these are what a real PCB keeps."""
    cuts = []
    for (side, yc) in HOOKS:  # clear the snap hooks
        x0, x1 = (0, WALL + 2.8) if side < 0 else (W - WALL - 2.8, W)
        cuts.append(box(x0, yc - HOOK_W / 2 - 1.2, C_BACK - 1, x1, yc + HOOK_W / 2 + 1.2, C_FRONT + 1))
    for (x, y) in POSTS:
        cuts.append(cyl(PIN_HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
    # under the HW-040: clearance for the EC11 legs and the pull-up resistors on its back
    kx0, kx1, ky0, ky1 = HW040_BACK_KEEPOUT
    cuts.append(box(KNOB_C[0] + kx0, KNOB_C[1] + ky0, C_BACK - 1, KNOB_C[0] + kx1, KNOB_C[1] + ky1, C_FRONT + 1))
    # header solder joints under the header end, and a slot past the pin tips for the wires
    cuts.append(box(hw_hdr[0] - 1.6, hw_hdr[1] - 7.0, C_BACK - 1, hw_hdr[0] + 1.6, hw_hdr[1] + 7.0, C_FRONT + 1))
    cuts.append(box(hw_wire_slot[0], hw_hdr[1] - 7.0, C_BACK - 1, hw_wire_slot[1], hw_hdr[1] + 7.0, C_FRONT + 1))
    cuts.append(cbox(disp_hdr_x, dpcb_c[1], 2.6, 7 * 2.54 + 1.0, C_BACK - 1, C_FRONT + 1))
    for i in range(3):
        for p in range(4):
            x, y = btn_pin(i, p)
            cuts.append(cyl(SW_HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
        for k in range(len(SW_PEGS)):  # the switch's locating bosses, so it sits flat
            x, y = btn_peg(i, k)
            cuts.append(cyl(SW_PEG_HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
    for (x, y) in sm_pins.values():
        cuts.append(cyl(HOLE_D, x, y, C_BACK - 1, C_FRONT + 1))
    return cuts


def peg(x, y, hole_d, z0, z1):
    """Locating peg with a 0.3 mm lead-in chamfer at the tip."""
    d = hole_d - 2 * PEG_CLR
    body = cyl(d, x, y, z0, z1 - 0.3)
    tip = Part.makeCone(d / 2, d / 2 - 0.3, 0.3, V(x, y, z1 - 0.3), V(0, 0, 1))
    return body.fuse(tip)


def make_carrier():
    """The printed PCB. Prints back side down; every holding feature points up from the front."""
    c = carrier_outline()
    cuts = carrier_through_cuts()
    # HW-040 pocket: module PCB flush with the carrier front, so the EC11 sits where a bare EC11 on
    # a real board would.
    cuts.append(cbox(hw_c[0], hw_c[1], HW040[0] + 0.6, HW040[1] + 0.6, hw_floor, C_FRONT + 1))
    c = c.cut(cuts)

    adds = []
    for (x, y) in hw_holes:  # HW-040 pegs, from the pocket floor through its two holes
        adds.append(peg(x, y, HW040_HOLE_D, hw_floor - 0.01, C_FRONT + PEG_PROUD))
    for (x, y) in disp_holes:  # display standoffs with pegs through its four corner holes
        adds.append(cyl(DISP_STANDOFF_D, x, y, C_FRONT - 0.01, dpcb_z1))
        adds.append(peg(x, y, DISP_HOLE_D, dpcb_z1 - 0.01, glass_z1 + PEG_PROUD))
    c = c.fuse(adds)
    # keep the display header slot clear of the standoffs
    c = c.cut(cbox(disp_hdr_x, dpcb_c[1], 2.6, 7 * 2.54 + 1.0, C_BACK - 1, C_FRONT + 6))

    # labels engraved on the back, mirrored so they read correctly from behind
    labels = [("KIVORI PROTO v3", 30.0, 60.0, 2.6)]
    for n, (x, y) in disp_pins.items():
        labels.append((n, x - DISP_HEADER_SIDE * 5.0, y, 1.6))
    for n, (x, y) in enc_pins.items():
        labels.append((n, x + 4.0, y, 1.6))
    for i, (x, y, a) in enumerate(BTNS):
        labels.append(("B%d %s" % (i + 1, BTN_GPIO[i]), x, y - 9.5, 1.8))
    tcuts = [text_solid(s, x, y, sz, C_BACK - 0.1, TEXT_D + 0.1, mirror=True) for (s, x, y, sz) in labels]
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
        flutes.append(cyl(KNOB_FLUTE_D, cx + r * math.cos(a), cy + r * math.sin(a), z0 + 1.2, z1 - 1.8))
    bush_top = C_FRONT + EC11_BODY_H + EC11_BUSH_L
    shaft_top = C_FRONT + EC11_SHAFT_H
    cuts = flutes + [cyl(18.0, cx, cy, z0 - 1, bush_top + 0.5)]
    bore = cyl(EC11_SHAFT_D + 0.15, cx, cy, bush_top, shaft_top + 0.5)
    flat = box(cx - 5, cy + EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, bush_top - 1, cx + 5, cy + 5, shaft_top + 1)
    cuts.append(bore.cut(flat))
    m0, m1, mang, mw = KNOB_MARK
    mark = box(m0, -mw / 2, z1 - 0.6, m1, mw / 2, z1 + 1)
    cuts.append(place(mark, cx, cy, mang))
    return k.cut(cuts).removeSplitter()


def make_cap(i):
    x, y, a = BTNS[i]
    z_flange = -FACE_T - 0.3 - BTN_FLANGE_T
    body = rr_prism(BTN_CAP, BTN_CAP, 1.2, x, y, z_flange, BTN_CAP_PROUD, a)
    try:
        top = [e for e in body.Edges if abs(e.BoundBox.ZMin - BTN_CAP_PROUD) < 1e-6 and abs(e.BoundBox.ZMax - BTN_CAP_PROUD) < 1e-6]
        body = body.makeFillet(0.8, top)
    except Exception as exc:
        print("cap fillet skipped:", exc)
    flange = rr_prism(BTN_FLANGE, BTN_FLANGE, 1.5, x, y, z_flange, z_flange + BTN_FLANGE_T, a)
    cap = body.fuse(flange)
    sock = SW_HEAD + 2 * SW_HEAD_CLR  # square socket over the head: keeps the cap from turning
    cap = cap.cut(rr_prism(sock, sock, 0.2, x, y, z_flange - 1, SW_PLUNGER_TOP, a))
    rd = 30.0  # keycap dish: a shallow cylinder across the top
    dish = Part.makeCylinder(rd, 30, V(-15, 0, BTN_CAP_PROUD + rd - BTN_CAP_DISH), V(1, 0, 0))
    cap = cap.cut(place(dish, x, y, a))
    return cap.removeSplitter()


# Stand-ins for the bought parts (interference checks and renders only, never printed)


def dummy_encoder():
    pcb = cbox(hw_c[0], hw_c[1], HW040[0], HW040[1], C_FRONT - HW040[2], C_FRONT)
    pcb = pcb.cut([cyl(HW040_HOLE_D, x, y, C_FRONT - 5, C_FRONT + 1) for (x, y) in hw_holes])
    body = cbox(KNOB_C[0], KNOB_C[1], EC11_BODY[0], EC11_BODY[1], C_FRONT, C_FRONT + EC11_BODY_H)
    bush = cyl(EC11_BUSH_D, KNOB_C[0], KNOB_C[1], C_FRONT + EC11_BODY_H, C_FRONT + EC11_BODY_H + EC11_BUSH_L)
    shaft = cyl(EC11_SHAFT_D, KNOB_C[0], KNOB_C[1], C_FRONT + EC11_BODY_H + EC11_BUSH_L, C_FRONT + EC11_SHAFT_H)
    shaft = shaft.cut(box(KNOB_C[0] - 5, KNOB_C[1] + EC11_SHAFT_D / 2 - EC11_FLAT, C_FRONT + EC11_SHAFT_H - 12,
                          KNOB_C[0] + 5, KNOB_C[1] + 5, C_FRONT + EC11_SHAFT_H + 1))
    # right-angle header: plastic on the front of the board, pins lying flat and pointing +x
    hdr = cbox(hw_hdr[0], hw_hdr[1], 2.5, 5 * 2.54, C_FRONT, C_FRONT + 2.5)
    pins = box(hw_hdr[0], hw_hdr[1] - 5.6, C_FRONT + 1.0, hw_pin_tip_x, hw_hdr[1] + 5.6, C_FRONT + 1.64)
    return fuse_all([pcb, body, bush, shaft, hdr, pins])


def dummy_display():
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], glass_z1, glass_z0)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], dpcb_z1, glass_z1)
    pcb = pcb.cut([cyl(DISP_HOLE_D, x, y, dpcb_z1 - 1, glass_z1 + 1) for (x, y) in disp_holes])
    hdr = cbox(disp_hdr_x, dpcb_c[1], 2.5, 7 * 2.54, dpcb_z1 - 2.5, dpcb_z1)
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
    usb = cbox(SM_X, SM_USB_END - SM_USB[1] / 2 + 0.5, SM_USB[0], SM_USB[1], sm_z_comp - SM_USB[2], sm_z_comp)
    btns = [cbox(x, y, SM_BTN[0], SM_BTN[1], sm_z_comp - SM_BTN[2], sm_z_comp) for (x, y) in sm_btns]
    chip = cbox(SM_X, sm_c[1] - 3.0, 5.0, 5.0, sm_z_comp - 0.9, sm_z_comp)
    return fuse_all([pcb, usb, chip] + btns)


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}

COLORS = {  # two-tone: light matte shell; black visor, knob and plinth; warm accent caps
    "Shell": (0.93, 0.93, 0.91), "Visor": (0.06, 0.06, 0.07), "BackPlate": (0.13, 0.13, 0.15),
    "Carrier": (0.12, 0.45, 0.25), "Knob": (0.10, 0.10, 0.11), "Cap": (0.95, 0.50, 0.18),
    "Dummy": (0.30, 0.45, 0.85),
}


def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["Visor"] = make_visor()
    PRINTED["BackPlate"] = make_back_plate()
    PRINTED["Carrier"] = make_carrier()
    PRINTED["Knob"] = make_knob()
    for i, n in enumerate(["CapLeft", "CapMiddle", "CapRight"]):
        PRINTED[n] = make_cap(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_SuperMini"] = dummy_supermini()
    for i, n in enumerate(["Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"]):
        DUMMIES[n] = dummy_switch(i)


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


def to_doc():
    if DOC_NAME in App.listDocuments():
        App.closeDocument(DOC_NAME)
    doc = App.newDocument(DOC_NAME)
    for name, shp in list(PRINTED.items()) + list(DUMMIES.items()):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shp
        if App.GuiUp and obj.ViewObject:
            key = "Dummy" if name.startswith("Dummy") else ("Cap" if name.startswith("Cap") else name)
            obj.ViewObject.ShapeColor = COLORS[key]
            if name == "Shell":
                obj.ViewObject.Transparency = 55
    doc.recompute()
    return doc


def print_pose(name, shp):
    """Rotate a part into its print orientation, resting on z = 0."""
    s = shp.copy()
    if name in ("Shell", "Knob", "Visor"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180)       # face / knob top / visor top on the bed
    elif name == "BackPlate":
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
    section = carrier_outline().cut(carrier_through_cuts()).slice(V(0, 0, 1), zc)
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
