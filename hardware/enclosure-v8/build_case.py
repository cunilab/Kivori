"""Kivori enclosure v8, "Terrace" with the display raised and tilted 12° towards the user (approved concept B): parametric build script
(FreeCAD 1.1, Part workbench, no GUI needed).

Run it inside FreeCAD (GUI Python console, the FreeCAD MCP, or freecadcmd):

    p = "<repo>/hardware/enclosure-v8/build_case.py"
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
KNOB_D = 36.0
KNOB_GAP = 4.3              # owner-approved: skirt and hub flush, with 3.0 mm shaft engagement
KNOB_ENGAGE = 3.0           # how far the shaft enters the knob's bore (all of it on the D flat); the hub hangs this far below the shaft tip
KNOB_TOP_T = 4.5            # knob top above the shaft-tip clearance
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

# Display: ST7789 1.3" 240x240, 7-pin module, mounted rotated 90° (header on the right), RAISED and TILTED 12° towards the
# user (v8, concept B). It is still carried by its PCB on four pillars at the four corner holes; the pillars are of
# different heights (the far pair is 4.8 mm taller), their tops are cut square to the tilted board and the pegs lean with it.
DISP_C = (43.0, 77.4)      # centre of the window on the face: the active area's centre, which is also the tilt pivot (y was 78.0: the
                            # tilted header's pocket row moves with the board, this keeps the centre pocket on its v7 y)
DISP_TILT = 12.0            # degrees towards the user: the front edge (nearest the knob, y low) is low, the back edge high.
                            # With the case wedge (ALPHA, 7.2°) the screen is 19.2° to the desk
DISP_Z = 5.8                # glass front above the face at the window centre (the pivot)
DISP_ACTIVE = 23.4          # active area (square), several sources agree
DISP_WIN_MARGIN = 0.4       # the white lip's window beyond the active area, per side
# Module frame: u runs along the 39.22 mm length, + towards the header end. Glass and visible
# area measured on the real module (photo next to a ruler, about ±0.5 mm).
DISP_PCB = (39.22, 27.78, 1.6)    # module PCB (length, width, thickness); 3.0 total with glass
DISP_GLASS = (28.0, 26.3, 1.4)    # glass incl. the FPC ledge at the end away from the header (measured)
DISP_ACTIVE_U = 1.85              # visible-area centre from the board centre, along u (measured)
DISP_GLASS_U = 0.8                # glass centre from the board centre, along u (measured)
DISP_HDR_U = 2.2                  # 7-pin header row, from the header end
DISP_HDR_V = 0.1                  # header row centre past the board centre, towards the far (USB) edge (measured)
DISP_GLASS_AIR = 0.6        # air between the glass front and the cowl's roof underneath: the glass carries no load of any kind
                            # (it is not clamped: the module is held by its PCB, on the pillars)
DISP_ROOF_T = 0.6           # the cowl's roof over the glass (3 layers); its top is DISP_GLASS_AIR + DISP_ROOF_T = 1.2 above the glass
                            # front. The whole cavity ceiling is that one plane, so the LCD flex cable has 2.0 mm over the board
DISP_FPC = (18.0, 1.4)      # ASSUMED flex-cable keep-out: width (centred on the glass) and height above the module board. It
                            # leaves the glass ledge at the end away from the header and lies on the board; v7's face pinched it
                            # at 1.3 mm of air. Here it may rise to the glass front (1.4) and still has DISP_GLASS_AIR to spare
DISP_FPC_AIR_MIN = 0.5      # build check: keep-out to cowl, lip and everything printed
DISP_HEADER_SIDE = +1       # +1 header on the right end, -1 on the left end
DISP_HOLE_D = 2.0          # four corner holes, NOMINAL ("mounting holes: 2 mm")
DISP_HOLE_INSET = 2.5      # hole centres from both edges at each corner (measured: 34.2 x 22.8 apart)
DISP_STANDOFF_D = 4.5      # other pillars and the far-east pillar's wider foot
DISP_EAST_D = 3.2           # approved smaller far-east pillar, beside the GND pin
DISP_EAST_FOOT_H = 2.0      # retain a wider foot below the pins/wires
DISP_EAST_TAPER_H = 0.65    # 45 degree reduction from radius 2.25 to 1.60
DISP_EAST_FILLET = 0.25     # soften both joins of the tapered foot
CRADLE_END_TRIM = 0.7       # shave the far end to remove the old 0.14 mm slit beside the wide foot
DISP_PINS = ["GND", "VCC", "SCL", "SDA", "RES", "DC", "BLK"]  # top to bottom as mounted
PEG_PROUD = 1.0            # peg tip past the module board (it was 2.0 to heat-stake; now the cowl's pads hold the board down, so the
                            # peg only locates it and is not melted)
DISP_PAD = (3.6, 3.0)       # cowl hold-down pad on each corner hole of the board: outside D, bore D round the peg tip (the cowl
                            # sits on 0.25-play pegs, so the bore leaves 0.6 per side)
DISP_PAD_GAP = 0.2          # air between a pad and the board: it only stops the board lifting off the pegs, it never presses on it
DISP_PIN_TIP_CLR = 0.8      # header pin tips to the wire cradle's top plane (the pins stand free: nothing forces them, nothing bends them)
DISP_WIRE_DX = -0.7         # the wires leave a display pin's root this far to its west (clear of the pin) and drop to the cradle there
PIN_PILLAR_CLR = 0.8          # slot in the far pillars round the leaning header pins (and their wire-wrap rings)
DISP_HDR_BODY = (2.54, 7 * 2.54, 2.5)   # header plastic: width, length, height under the board
LIP_W, LIP_H = 1.4, 0.6     # white lip: width and height above the cowl top (v7's bezel: 2.0 x 0.8)
LIP_CHAMFER = 0.4           # 45° bevel on the lip's window edge, over 0.4 mm (0.2 mm vertical remains): the seated eye (59° over the glass) is steeper
                            # than the bevel, so what hides the screen is the bevel's foot, at the cowl top
LIP_COLLAR = (0.4, 0.5, 0.1)  # collar on the lip's underside that locates it in the cowl window: wall, height, play per side
EYE_ELEV = 40.0             # seated eye above the desk, from the front (the concept sheet's viewpoint)
RIM_HIDDEN_MAX = 0.5        # build check: the near rim may hide at most this much of the active area from that eye

# Buttons: Outemu MX-style, full height, 3-pin. Every switch number is from the Outemu drawing the owner
# supplied (top housing 15.60 square, bottom housing 13.95 square and 15.03 over the clips on +-Y, bottom to
# housing top 11.00, plate level 5.00, flange top 6.00, clip 1.90, stem 4.10 above the housing, stem cross
# 4.00 with arms 1.10 / 1.30, centre post dia 3.85, pins 3.80 / 2.54 / 5.08 from the post and 2.85 below the
# bottom). Keycap numbers are from the owner's 1u keycap drawing (18 x 18 base, side walls 80 deg, front 70 deg,
# 9.4 back / 9.68 front tall, wall 0.91, stem tube dia 5.50, cross 4 x 1.26, socket 1.30 above the skirt bottom).
BTN_MID = (43.0, 11.5)      # middle button centre (the 18 mm cap skirt needs 11.5 to keep its opening clear of the near wall)
KEY_STYLE = 'well'          # 'well': a square opening per key, the cap skirt rides in it (ref: case well)
                            # 'plate': one slot over the whole row, the switches stand on the plate frames below it
BTN_PITCH = 20.0            # the 18 mm cap skirt rides in a face opening, so the webs between openings need 1.3 mm
BTN_OPEN = 18.7             # face opening round the 18.0 cap base: 0.35 per side
BTN_CAP = 18.0              # keycap base (drawing)
CAP_H = 9.5                 # keycap height, skirt bottom to the flat top (drawing: 9.4 back / 9.68 front, the top is flat here)
CAP_SIDE_DEG, CAP_FRONT_DEG, CAP_BACK_DEG = 80.0, 70.0, 80.0   # wall angles from the horizontal (drawing; the back wall is ASSUMED 80)
CAP_WALL = 0.91             # skirt wall (drawing)
CAP_TUBE_D = 5.50           # stem tube (drawing)
CAP_SOCKET_UP = 1.30        # socket mouth above the skirt bottom (drawing)
CAP_TOP_T = 1.5             # top above the ceiling (ASSUMED); the cap prints top down on its flat top
CAP_RIB_T, CAP_RIB_UP = 0.8, 5.0   # ASSUMED support ribs: thickness, bottom above the skirt bottom (so they clear the housing at full press)
CAP_CLR = 0.35              # skirt bottom over the housing flange at full press
# Switch, heights above the switch bottom (= the PCB plane).
SW_PLATE_T, SW_PLATE_Z = 1.5, 5.0   # plate level 5.00 (drawing); plate thickness 1.5 (Cherry plate, ASSUMED)
SW_CUTOUT = 14.1            # plate cutout: the drawing's 14.00 plus 0.1 for FDM at most (14.3 let the clips slip out)
SW_BASE = 15.60             # top housing flange, square
SW_LOWER, SW_CLIPS = 13.95, 15.03   # bottom housing, and over the clips (clips on +-Y)
SW_HOUSING_H = 11.00        # bottom to housing top
SW_FLANGE_Z = 6.00          # flange top
SW_CLIP_H = 1.90            # clip height
SW_UPPER = (13.0, 11.6)     # ASSUMED upper housing width at the flange top and at the top (tapers, read off the front view)
SW_STEM_H = 4.10            # stem above the housing, at rest
SW_STEM_L = 4.00            # stem cross length (both ways)
SW_STEM_ARMS = (1.10, 1.30)  # cross arm thickness: of the Y bar (along x), of the X bar (along y)
SW_TRAVEL = 4.0
SW_PIN_L = 2.85             # pins and post below the bottom
SW_PIN = (0.85, 0.78)       # pin section, from the 0.78 - 0.85 callouts
SW_POST_D, SW_POST_L = 3.85, 2.85  # post dia (drawing); its length is ASSUMED equal to the pins
SW_WELL = (8.0, 5.0)        # ASSUMED stem well in the housing top: width, depth. The stem tube travels in it
SW_CLIP_HALF = 2.3          # clip pocket half width at the plate underside (the hook is 4.0 wide): the flat catch strip is 4.6 x OVH_LEDGE
SW_CLIP_LEAN = 0.4          # the pocket's ends and back wall lean in at 45 deg over this height under the plate
SW_SINK = 0.8               # switch seat sunk into a pocket in the carrier front: the pins then stand 1.65 mm behind it (a 0.9 mm wrap fits)
# Pins relative to the centre post: (-3.81, 2.54) and (2.54, 5.08) (drawing 3.80 / 2.54 / 5.08; KiCad
# SW_Cherry_MX_1.00u_PCB has the same, Y flipped for this frame, whose +Y is away from the user).
# Pad 0 is the ground pin (daisy chain); pad 1 is B1/B2/B3.
SW_PADS = [(-3.81, 2.54), (2.54, 5.08)]
# Keycap socket: the stem plus 0.10 FDM allowance (cross 4.00 -> 4.10, arms 1.10 / 1.30 -> 1.20 / 1.40). The drawing's 1.26
# slot would pinch the 1.30 arm.
CAP_SLOT_L, CAP_SLOT_ARMS = SW_STEM_L + 0.10, (SW_STEM_ARMS[0] + 0.10, SW_STEM_ARMS[1] + 0.10)

# Printed PCB ("carrier"): same plane as a future real PCB
C_FRONT = -10.2             # printed PCB front (the 7.2 mm switch top ends 3.0 below the face: the caps reach down)
KNOB_H = 11.0               # held from the 1.8 mm gap (it was HW040_FRONT + EC11_SHAFT_H + KNOB_TIP_CLR + KNOB_TOP_T - KNOB_GAP)
C_T = 2.0                   # flat printed carrier, no back-side component pockets
C_BACK = C_FRONT - C_T
SW_Z0 = C_FRONT - SW_SINK   # switch bottom (the PCB plane of the drawing)
BTN_CAP_PROUD = SW_Z0 + SW_FLANGE_Z + CAP_CLR + SW_TRAVEL + CAP_H   # cap top above the face
C_INSET = 0.8               # carrier outline inset from the inner walls
HOLE_D = 1.2                # clearance for other 2.54 mm header pins
RP_PIN_FIT_D = 0.90          # provisional PETG press fit; choose after coupon test
SW_HOLE_D = 1.70            # printed hole for the Ø1.50 pin drill (FDM prints small; test-fit)
SW_POST_HOLE = 4.20         # printed hole for the Ø3.99 centre post
POSTS = [(6.0, 7.0), (6.0, H - 7.5), (W - 6.0, H - 7.5), (W - 6.0, 7.0)]  # four corner screws; the back plate pillars carry the loads
# Back plate pillars: they reach up to the carrier back (PILLAR_GAP short of it) right under the load points, so a press on a key or on
# the encoder goes straight into the back plate and does not bend the carrier. Under each key: its east side, under the pocket floor (the
# pins, the post and the ground chain are on the west and south); the others stand under the frame walls between keys. Under the
# encoder: round the HW-040 window, which the pillars keep PILLAR_WIN clear of.
PILLAR_D, PILLAR_GAP, PILLAR_WIN = 3.2, 0.2, 0.5
PILLARS = [(29.5, 12.0), (49.5, 12.0), (69.5, 12.0), (15.2, 12.0), (33.0, 17.0), (53.0, 17.0),
           (36.0, 29.0), (50.0, 29.0), (36.0, 54.2), (50.0, 54.2)]

POST_D = 4.2               # slim: the front-right post sits just outside the right cap
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

# Waveshare RP2040-Zero sits on the carrier underside, components facing the bottom cover.
SM = (18.0, 23.5, 1.1)      # drawing outline; thickness from official STEP
SM_X = W / 2  # underside board, on the case's centre line (it was 46.0, 3 mm to the right)
SM_USB = (9.6, 7.71, 3.25)  # official STEP: width, depth, height above board top
SM_USB_OVERHANG = 1.21      # official STEP, receptacle front past the board end
# USB-C port: a tight pill-shaped hole. A hidden pocket inside the far wall thins it to USB_WALL
# round the receptacle, and the board sits right up against that pocket, so a cable's plug
# reaches fully into the receptacle without its moulded grip entering the wall.
USB_WALL = 0.8              # wall left in front of the receptacle (measured square to the wall)
USB_HOLE = (10.4, 4.8)      # clearance round the STEP's 9.6 x 4.16 mm connector envelope
USB_HOLE_DZ = -1.5          # the shell's hole sits this much lower than the modelled receptacle (towards the back plate): on the
                            # print of the first v7 the hole came out 1.5 mm too high against the real connector (measured
                            # on the print), so the real connector is taken to sit 1.5 mm lower than modelled
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

# 0.3 mm copper wire (possibly bare), wire-wrapped on every pin and laid in its own groove. No two wires cross or
# share a groove or hole, and different nets keep WIRE_RIDGE_MIN of plastic between them (pin holes included). A wire
# changes face only through its own via. The routes are in wire_routes().
WIRE_D = 0.3                # wire diameter
GROOVE_W, GROOVE_D = 0.6, 0.5   # groove width, depth
VIA_D = 0.8                 # a wire's own through-hole where it changes face
WIRE_BEND_R = 1.5           # groove centre-line radius at every bend (less where a stretch is too short for it)
WIRE_ATT = 0.5              # modelled wire ends this far from a pin centre (the wrap closes the gap)
COL_PITCH = 1.5             # groove to groove in the west corridor (a via sits beside each column: 0.7 plastic to its neighbour)
PITCH_MIN = 1.3             # groove to groove elsewhere: the groove plus WIRE_RIDGE_MIN
WIRE_RIDGE_MIN = 0.7        # plastic kept between different nets (grooves, vias, pin holes), or a hole and the edge
WRAP = (1.3, 1.0, 0.9)      # wire-wrap ring modelled on each pin: outside, inside, height (3 turns of 0.3)
DPIN_POCKET = (1.2, 1.0)    # entry pocket in the carrier under each display pin: diameter, depth
DISP_PIN_L = 6.0            # display header: standard 2.54 mm header, pin past the plastic (NOMINAL). The pins stand free, tilted
                            # with the board, their tips DISP_PIN_TIP_CLR over the wire cradle; the wires are wrapped at the pin
                            # root and each tail drops straight to a funnel in the cradle, a bore and the pocket under it
WIRE_Z_FRONT = C_FRONT - GROOVE_D + WIRE_D / 2 + 0.03   # wire centre in a front groove (0.03 off the floor)
WIRE_Z_BACK = C_BACK + GROOVE_D - WIRE_D / 2 - 0.03      # ... in a back groove
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
# No printed feet: the plinth's bed face is flat (a flat roof over air printed badly) and the owner sticks rubber
# bumpers over the screw counterbores (SCREW_HEAD_D).

# Separate black display cowl and white trim: every visible face can lie on the print bed.
VISOR_T = 0.0               # key mechanism references the bare main face in v7
COWL_BOX = (14.0, 56.0, 72.0, 93.0)
COWL_PEGS = [(18.5, 60.0), (66.5, 60.0), (17.4, 76.0), (66.7, 76.0)]  # in the solid strips round the display pocket,
                            # clear of the module, standoffs, wires and the knob curve
COWL_PEG_D, COWL_PEG_HOLE_D, COWL_PEG_L = 3.0, 3.5, 3.0  # peg, shell hole (0.25 per side), peg length (the shell face
                            # is 2.5: the tip comes out 0.5 behind it; glue or friction fit)
COWL_EDGE = 0.4             # 45 deg chamfer round the cowl's seating edge
COWL_WIN = DISP_ACTIVE + 2 * DISP_WIN_MARGIN + 2 * (LIP_COLLAR[0] + LIP_COLLAR[2])   # the cowl's window: the lip's collar sits in it
COWL_WIN_R = 1.0 + LIP_COLLAR[0] + LIP_COLLAR[2]
COWL_CURVE_R = 22.0  # follows the halo, with clearance at rest and during knob press
HALO_OD, HALO_ID, HALO_T = 38.0, 12.4, 0.6  # white trim ring on the face (glued on); 38 keeps it 0.5 clear of the key caps

KNOB_CAP_T, KNOB_CAP_D = 0.8, 31.0
KNOB_CAP_PEGS = [(KNOB_C[0] + 10 * math.cos(math.radians(a)),
                  KNOB_C[1] + 10 * math.sin(math.radians(a))) for a in (0, 120, 240)]
OVH_MAX = 45.0              # steepest overhang that prints without support: degrees from vertical
BRIDGE_MAX = 10.0           # longest bridge accepted (mm)
OVH_LEDGE = 0.5             # a flat ledge this narrow (about a line width) needs no support either

DOC_NAME = "KivoriCaseV8"
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
SW_STEM_TOP = SW_Z0 + SW_HOUSING_H + SW_STEM_H   # stem top at rest, face frame

# Display. The module is built in its own untilted frame (x = u along the face x, y = the face y with DISP_C[1] at the pivot, z = w
# along the board normal: glass front w = 0, glass back = board front w = -1.4, board back w = -3.0) and tilt() carries a shape
# into the face frame: rotated DISP_TILT about the x axis through the pivot (the +y side rises), then lifted DISP_Z.
_TS, _TC = math.sin(math.radians(DISP_TILT)), math.cos(math.radians(DISP_TILT))


def tilt(shape):
    s = shape.copy()
    s.rotate(V(0, DISP_C[1], 0), V(1, 0, 0), DISP_TILT)
    s.translate(V(0, 0, DISP_Z))
    return s


def tp(x, y, w):
    """Module-frame point (x, y, w) -> face frame."""
    v = y - DISP_C[1]
    return V(x, DISP_C[1] + v * _TC - w * _TS, DISP_Z + v * _TS + w * _TC)


def plane_z(y, w):
    """Face-frame z of the plane at module height w, at face y."""
    return DISP_Z + w * _TC + (y - DISP_C[1] + w * _TS) / _TC * _TS


def above_plane(w0, h=60.0):
    """Everything of the module frame above the board-parallel plane w = w0, in the face frame (to cut things off)."""
    return tilt(box(-100, DISP_C[1] - 100, w0, 200, DISP_C[1] + 100, w0 + h))


def below_plane(w0, h=60.0):
    return tilt(box(-100, DISP_C[1] - 100, w0 - h, 200, DISP_C[1] + 100, w0))


W_GLASS0, W_GLASS1 = 0.0, -DISP_GLASS[2]       # glass front, glass back = board front
W_PCB1 = W_GLASS1 - DISP_PCB[2]                # board back (the pillar seats)
W_ROOF0 = DISP_GLASS_AIR                       # cowl cavity ceiling over the glass
W_TOP = DISP_GLASS_AIR + DISP_ROOF_T           # cowl top (the lip stands on it)
W_HDR1 = W_PCB1 - DISP_HDR_BODY[2]             # header plastic bottom: the pins start here
W_PIN_TIP = W_HDR1 - DISP_PIN_L
W_ROOT = W_HDR1 - 0.2                          # first wire-wrap ring top, just under the header plastic
W_CRADLE = W_PIN_TIP - DISP_PIN_TIP_CLR        # the wire cradle's top plane: parallel to the board, under the pin tips

dpcb_c = (DISP_C[0] - DISP_HEADER_SIDE * DISP_ACTIVE_U, DISP_C[1])
glass_c = (dpcb_c[0] + DISP_HEADER_SIDE * DISP_GLASS_U, DISP_C[1])
disp_hdr_x = dpcb_c[0] + DISP_HEADER_SIDE * (DISP_PCB[0] / 2 - DISP_HDR_U)
disp_hdr_y = dpcb_c[1] + DISP_HDR_V
disp_pins_m = {n: (disp_hdr_x, disp_hdr_y + (3 - i) * 2.54) for i, n in enumerate(DISP_PINS)}   # pin axes, module frame
disp_holes = [(dpcb_c[0] + su * (DISP_PCB[0] / 2 - DISP_HOLE_INSET), dpcb_c[1] + sv * (DISP_PCB[1] / 2 - DISP_HOLE_INSET))
              for su in (-1, 1) for sv in (-1, 1)]
# Pillars: vertical, centred under their seat point (the hole's centre on the board's back), top cut square to the board
disp_seat = [(x, tp(x, y, W_PCB1).y) for x, y in disp_holes]
disp_standoffs = disp_seat
# The wire pockets: where the wires meet the carrier. A wire drops from its pin root (a wrap ring, west of the pin by DISP_WIRE_DX
# so it clears the tilted pin) to the funnel in the cradle top, straight, and the bore under it ends in the pocket. The pocket
# sits at the pin's plan position at the board's back, so the wire leans a little the other way from the pin (clear of it).
disp_pins = {n: (x + DISP_WIRE_DX, tp(x, y, W_PCB1).y) for n, (x, y) in disp_pins_m.items()}


def disp_root_z(n):
    """Face-frame z of the first wrap ring's top on display pin n."""
    x, y = disp_pins_m[n]
    return tp(x, y, W_ROOT).z


# The opening in the face and the cowl's cavity: the tilted board's plan outline and the pillars, with 0.7 in x and 0.5 in y.
_cav_y = ([tp(0, dpcb_c[1] + sv * DISP_PCB[1] / 2, w).y for sv in (-1, 1) for w in (W_PCB1, W_GLASS1)]
          + [y + sg * DISP_STANDOFF_D / 2 for _, y in disp_seat for sg in (-1, 1)])
CAV = (dpcb_c[0] - DISP_PCB[0] / 2 - 0.7, min(_cav_y) - 0.5, dpcb_c[0] + DISP_PCB[0] / 2 + 0.7, max(_cav_y) + 0.5)


def cav_prism(z0, z1, r=1.0):
    x0, y0, x1, y1 = CAV
    return rr_prism(x1 - x0, y1 - y0, r, (x0 + x1) / 2, (y0 + y1) / 2, z0, z1)


CRADLE_W, CRADLE_END = 2.6, 1.2   # wire cradle under the header: width, and past the end pockets
CRADLE_FUNNEL_R = 0.85            # funnel radius at the cradle's top plane (45°), bore radius DPIN_POCKET[0] / 2 at the bottom


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
# The wires are wrapped on the RP2040-Zero pins where they stand out of the carrier front.
RP_NETS = {'IO3': 'SW', 'IO4': 'DT', 'IO5': 'CLK', 'IO6': 'B1', 'IO7': 'B2', 'IO8': 'B3',
           'IO26': 'SCL', 'IO15': 'SDA', 'IO14': 'RES', 'IO13': 'DC', 'IO12': 'BLK',
           '3V3': '3V3', 'GND': 'GND'}


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
    cuts.append(cav_prism(-FACE_T - 1, 1))
    if KEY_STYLE == 'plate':   # one slot over the row: the plate frames on the carrier show as the switch plate
        cuts.append(rr_prism(BTNS[-1][0] - BTNS[0][0] + BTN_OPEN, BTN_OPEN, 1.8, BTN_MID[0], BTN_MID[1], -FACE_T - 1, 1))
    else:
        for (x, y, a) in BTNS:
            cuts.append(rr_prism(BTN_OPEN, BTN_OPEN, 1.8, x, y, -FACE_T - 1, 1, a))
    for (x, y) in POSTS:
        cuts.append(cyl(SCREW_PILOT, x, y, C_BACK - SPIGOT_INTO_COL - 1, -FACE_T - 1.5))
    cuts += usb_port_cuts()
    cuts += [cyl(COWL_PEG_HOLE_D, x, y, -FACE_T - 1, 1) for x, y in COWL_PEGS]  # the cowl's pegs
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
    uz = usb_z + USB_HOLE_DZ
    port.translate(V(SM_X, H - uz * TAPER, uz))

    # pocket: from the inside up to USB_WALL short of the outer surface, over the board width and
    # from just behind the carrier down past the receptacle. Its far-from-face side slopes 45°,
    # so the shell still prints face down without an overhang.
    u = USB_WALL / math.cos(math.radians(WALL_TAPER))
    face_ = tapered(W - 2 * u, H - 2 * u, R_CORNER - u, 0, -60)
    z_top, z_bot = max(sm_hi, usb_hi) + 0.3, min(sm_lo, usb_lo + USB_HOLE_DZ) - 0.4   # (down to the real connector)
    yf = H - u - z_bot * TAPER        # pocket face at the bottom of the pocket
    prof = [(H - 15, z_top), (H + 5, z_top), (H + 5, z_bot + (H + 5 - yf)), (H - 15, z_bot - (yf - H + 15))]
    pocket = Part.Face(Part.makePolygon([V(0, py, pz) for (py, pz) in prof + [prof[0]]]))
    pocket = pocket.extrude(V(SM[0] + 1.2, 0, 0))
    pocket.translate(V(SM_X - SM[0] / 2 - 0.6, 0, 0))
    return [port, pocket.common(face_)]


def cowl_pegs():
    pegs = [peg(x, y, COWL_PEG_HOLE_D, -0.01, COWL_PEG_L, (COWL_PEG_HOLE_D - COWL_PEG_D) / 2) for x, y in COWL_PEGS]
    return [q.mirror(V(0, 0, 0), V(0, 0, 1)) for q in pegs]  # peg() grows up; these hang down


def make_cowl():
    """Black removable hood, prints with its tilted top plane on the bed. Over the glass it is a 0.6 roof 0.6 above the glass
    (the glass is never touched); four pads press down on the board's corner holes with an air gap, the pegs of the pillars
    reach into their bores. Nothing of it is near the flex cable."""
    x0, y0, x1, y1 = COWL_BOX
    c = rr_prism(x1 - x0, y1 - y0, 4.0, (x0 + x1) / 2, (y0 + y1) / 2, 0, 30).cut(above_plane(W_TOP))
    c = c.cut(Part.makeCone(COWL_CURVE_R, COWL_CURVE_R + 0.2, 30.02, V(*KNOB_C, -0.01), V(0, 0, 1)))
    c = c.cut(cav_prism(-1, 30, 0.5).cut(above_plane(W_ROOF0)))
    c = c.cut(tilt(rr_prism(COWL_WIN, COWL_WIN, COWL_WIN_R, *DISP_C, W_ROOF0 - 1, W_TOP + 6)))
    pads = []
    for x, y in disp_holes:
        pad = cyl(DISP_PAD[0], x, y, W_GLASS1 + DISP_PAD_GAP, W_ROOF0 + 0.3)
        pads.append(tilt(pad.cut(cyl(DISP_PAD[1], x, y, W_GLASS1 - 1, W_GLASS1 + PEG_PROUD + 0.4))))
    c = c.fuse(pads)
    # Plain pegs on the underside: they go through the shell face, 0.25 per side in their holes.
    c = c.fuse(cowl_pegs()).removeSplitter()
    # Chamfer the seating edge so the first layer's squish never shows as a gap against the face.
    seat = [f for f in c.Faces if f.Surface.TypeId == "Part::GeomPlane" and f.BoundBox.ZMax < 1e-6 and f.BoundBox.ZMin > -1e-6
            and f.Area > 100][0]
    return c.makeChamfer(COWL_EDGE, [e for e in seat.OuterWire.Edges]).removeSplitter()


def make_lip():
    """Slim white lip round the window, glued on the cowl top; its collar locates it in the cowl window. 1.4 wide, 0.6 high,
    a 45° chamfer on the window edge; prints with its top on the bed."""
    win = DISP_ACTIVE + 2 * DISP_WIN_MARGIN
    out = win + 2 * LIP_W
    lip = rr_prism(out, out, 1.5, *DISP_C, W_TOP, W_TOP + LIP_H)
    cw = win + 2 * LIP_COLLAR[0]
    lip = lip.fuse(rr_prism(cw, cw, 1.0 + LIP_COLLAR[0], *DISP_C, W_TOP - LIP_COLLAR[1], W_TOP + 0.01))
    lip = lip.cut(rr_prism(win, win, 1.0, *DISP_C, W_TOP - 1, W_TOP + LIP_H + 1))
    c = LIP_CHAMFER
    lip = lip.cut(place(Part.makeLoft([rr_wire(win, win, 1.0, W_TOP + LIP_H - c), rr_wire(win + 2 * c, win + 2 * c, 1.0 + c, W_TOP + LIP_H + 0.001)],
                                      True, True), *DISP_C))
    return tilt(lip.removeSplitter())


def make_halo():
    """White trim ring on the face, glued on."""
    cx, cy = KNOB_C
    return cyl(HALO_OD, cx, cy, 0, HALO_T).cut(cyl(HALO_ID, cx, cy, -1, 1))


def make_pin_coupon():
    """Five nine-pin rows permit testing a real header strip."""
    coupon=box(0,0,0,31,16,2)
    sizes=(0.80,0.85,0.90,0.95,1.00)
    coupon = coupon.cut([cyl(d,2.34+2.54*i,2.92+2.54*j,-1,3)
                         for j,d in enumerate(sizes) for i in range(9)]).removeSplitter()
    # One through five dot marks identify 0.80 through 1.00 mm rows.
    coupon = coupon.cut([cyl(0.8,24.6+i,2.92+2.54*j,1.5,3)
                         for j in range(5) for i in range(j+1)])
    coupon.translate(V(-40, 0, 0))
    return coupon



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
    for (x, y) in PILLARS:   # load pillars: up to PILLAR_GAP short of the carrier back
        adds.append(cyl(PILLAR_D, x, y, z_back(y) + 0.5, C_BACK - PILLAR_GAP))
    plate = plate.fuse([a.common(above_back(0)) for a in adds])

    # White base: 1.2 mm visible vertical edge and a 45-degree underside chamfer.
    # The flat bottom remains bed-facing; no cosmetic stripe is left as an unsupported shelf.
    plinth = Part.Face(plinth_outline()).extrude(V(0, 0, -PLINTH_H))
    try:
        plinth = plinth.makeChamfer(PLINTH_H - 1.2, [e for e in plinth.Edges
                                                     if abs(e.BoundBox.ZMax + PLINTH_H) < 1e-6])
    except Exception as exc:
        print("plinth chamfer skipped:", exc)
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


def bumper_room():
    """Largest round bumper centred on each screw axis that stays on the plinth's flat bottom (it stops where the
    underside chamfer starts), in POSTS order."""
    flat = plinth_flat().OuterWire
    return [2 * flat.distToShape(Part.Vertex(V(x, y, -PLINTH_H)))[0] for (x, y) in screw_axes()]


def check_underside():
    """Nothing but the plinth reaches the desk side. Returns problems."""
    problems = []
    for n, shp in PRINTED.items():
        sd = shp.copy()
        sd.translate(V(0, 0, T_NEAR))
        sd.rotate(V(0, 0, 0), V(1, 0, 0), math.degrees(ALPHA))
        z = sd.optimalBoundingBox().ZMin
        if z < -PLINTH_H - 1e-3:
            problems.append("%s below the plinth bottom (z %.2f)" % (n, z))
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
    cuts += [cyl(HOLE_D, x, y, z0, z1) for (x, y) in enc_pins.values()]
    cuts += [cyl(RP_PIN_FIT_D, x, y, z0, z1) for x, y in sm_pins.values()]
    cuts += [cyl(SW_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(3) for k in range(2)]
    cuts += [cyl(SW_POST_HOLE, x, y, z0, z1) for (x, y, _) in BTNS]
    cuts += [cyl(VIA_D, x, y, z0, z1) for _, x, y in wire_vias()]
    return cuts


RP_PIN = {net: name for name, net in RP_NETS.items()}   # net -> RP2040 pin name
LEG_NET = {(0, 0): 'GND', (1, 0): 'GND', (2, 0): 'GND',
           (0, 1): 'B1', (1, 1): 'B2', (2, 1): 'B3'}       # switch pin (key, pad) -> net
ENC_NET = {'CLK': 'CLK', 'DT': 'DT', 'SW': 'SW', '+': '3V3', 'GND': 'GND'}


def wire_routes():
    """Every wire as (net, segs, ends). segs = [(face, [(x, y), ...]), ...]: the groove centre lines, 'F' on the carrier
    front and 'B' on its back; two segments meet at a via. ends = (start, end) as (kind, pin xy): 'rp' RP2040 pin
    (front), 'disp' display pin (front, its pocket), 'enc' encoder pin or 'leg' switch leg (back). No run crosses another:
      * display: RP2040 right column and bottom row to the display pins' pockets (GND and 3V3 almost straight)
      * RP2040 left column: a groove on the front west to the via at the top of that wire's column, the column
        straight down the west corridor on the back, then to the pin or leg; the columns west to east are the three
        keys (they turn east above the keys) then CLK, DT, SW (they turn east into the encoder pins)
      * encoder 3V3 and GND, key GND: from the display's VCC and GND pins (a second and third wrap there), on the
        front round the east and south of the display, a via east of the encoder pins and a short hop west."""
    S, D, E = sm_pins, disp_pins, enc_pins
    R = []
    # GND and 3V3: out to the display's pocket, a little steeper than a straight line, round the corner standoff
    for n, p in (('GND', 'GND'), ('3V3', 'VCC')):
        a_, d_ = S[RP_PIN[n]], D[p]
        R.append((n, [('F', [a_, (a_[0] + 3.2, d_[1] + 0.7), d_])], (('rp', a_), ('disp', d_))))
    for n, p in (('SCL', 'SCL'), ('SDA', 'SDA'), ('RES', 'RES')):
        R.append((n, [('F', [S[RP_PIN[n]], D[p]])], (('rp', S[RP_PIN[n]]), ('disp', D[p]))))
    s13, s12, s14 = S[RP_PIN['DC']], S[RP_PIN['BLK']], S[RP_PIN['RES']]
    R.append(('DC', [('F', [s13, (s13[0] + 0.6, s13[1] - 1.45), (s14[0] + 1.9, s13[1] - 1.45), D['DC']])],
              (('rp', s13), ('disp', D['DC']))))
    R.append(('BLK', [('F', [s12, (s12[0] + 0.7, s12[1] - 2.75), (s14[0] + 2.4, s12[1] - 2.75), D['BLK']])],
              (('rp', s12), ('disp', D['BLK']))))
    # RP2040 left column: wires and the columns they take (west to east), see above
    order = ['B1', 'B2', 'B3', 'CLK', 'DT', 'SW']
    col = {n: enc_x - 1.95 - COL_PITCH * (5 - i) for i, n in enumerate(order)}
    for n in order:
        p = S[RP_PIN[n]]
        top = (col[n], p[1])                       # the via: the column starts here
        if n in ('CLK', 'DT', 'SW'):
            e = E[n]
            back, end = [top, (col[n], e[1]), e], ('enc', e)
        else:
            i = int(n[1]) - 1                      # key 0..2
            leg = btn_pin(i, 1)                    # the signal pin
            yh = 21.9 + COL_PITCH * i              # turns east above its key; the rightmost key turns first (highest)
            back, end = [top, (col[n], yh), (leg[0], yh), leg], ('leg', leg)
        R.append((n, [('F', [p, top]), ('B', back)], (('rp', p), end)))
    # power from the display's VCC and GND pins
    xp = enc_x + 2.4
    xg = xp + 1.5
    X0 = disp_hdr_x     # the display pins' x (the pockets are DISP_WIRE_DX west of it)
    ex1 = X0 + 3.04
    ex2 = ex1 + PITCH_MIN
    yp = 62.0
    via_p, via_g = (xp, E['+'][1] + 1.7), (xg, E['GND'][1] + 2.4)
    R.append(('3V3', [('F', [D['VCC'], (ex1, D['VCC'][1]), (ex1, yp), (xp, yp), via_p]), ('B', [via_p, E['+']])],
              (('disp', D['VCC']), ('enc', E['+']))))
    R.append(('GND', [('F', [D['GND'], (X0 + 1.4, D['GND'][1] - 0.8), (ex2, D['GND'][1] - 0.8),
                             (ex2, yp - PITCH_MIN), (xg, yp - PITCH_MIN), via_g]),
                      ('B', [via_g, E['GND']])], (('disp', D['GND']), ('enc', E['GND']))))
    # key ground, south of the centre posts. A straight run at the pin's y passes 0.14 mm
    # from the Ø4.2 post hole. The feed reaches the right pin from the south; each link to the
    # next pin steps west of that pin (away from its post) before dropping to its own y.
    ys, yb = BTN_MID[1] - 4.5, BTN_MID[1] - 5.9   # 6.5 and 5.1: both clear the post hole, and each other
    xk = 75.0
    via_k = (xk, ys)
    g = [btn_pin(i, 0) for i in range(3)]
    R.append(('GND', [('F', [D['GND'], (X0 + 1.9, D['GND'][1] + 0.8), (xk, D['GND'][1] + 0.8), via_k]),
                      ('B', [via_k, (g[2][0], ys), g[2]])], (('disp', D['GND']), ('leg', g[2]))))
    R.append(('GND', [('B', [g[2], (g[2][0] - 3.0, g[2][1]), (g[2][0] - 3.0, yb), (g[1][0], yb), g[1]])],
              (('leg', g[2]), ('leg', g[1]))))
    R.append(('GND', [('B', [g[1], (g[1][0] - 3.0, g[1][1]), (g[1][0] - 3.0, ys), (g[0][0], ys), g[0]])],
              (('leg', g[1]), ('leg', g[0]))))
    return R


def wire_vias():
    """Where a wire changes face: (net, x, y)."""
    return [(net, *p1[-1]) for net, segs, _ in wire_routes() for (_, p1), _s in zip(segs, segs[1:])]


def disp_pockets():
    return list(disp_pins.values())


def groove_cuts():
    """Grooves, vias and display pockets, ready to cut from the carrier."""
    cuts = []
    for net, segs, _ in wire_routes():
        for face, pts in segs:
            if face == 'F':
                cuts.append(band(pts, GROOVE_W, C_FRONT - GROOVE_D, C_FRONT + 1))
            else:
                cuts.append(band(pts, GROOVE_W, C_BACK - 1, C_BACK + GROOVE_D))
    cuts += [cyl(VIA_D, x, y, C_BACK - 1, C_FRONT + 1) for _, x, y in wire_vias()]
    cuts += cradle_cuts()
    return cuts


def _circle(x, y, d):
    return Part.Face(Part.Wire(Part.makeCircle(d / 2, V(x, y, 0))))


def wire_features(face):
    """(net, label, 2D shape) of everything on one face of the carrier that belongs to a net, plus the solids and holes
    that have to be kept clear (net 'solid'). The 0.7 mm rule is checked between different nets."""
    F = []
    for net, segs, _ in wire_routes():
        for fc, pts in segs:
            if fc == face:
                F.append((net, "groove %s %s" % (net, face), band_face(pts, GROOVE_W, 0)))
    F += [(net, "via %s" % net, _circle(x, y, VIA_D)) for net, x, y in wire_vias()]
    F += [(RP_NETS.get(n, 'n/c ' + n), "RP hole " + n, _circle(x, y, RP_PIN_FIT_D)) for n, (x, y) in sm_pins.items()]
    F += [(ENC_NET[n], "encoder hole " + n, _circle(x, y, HOLE_D)) for n, (x, y) in enc_pins.items()]
    F += [(LEG_NET[(i, k)], "switch pin %d.%d" % (i, k), _circle(*btn_pin(i, k), SW_HOLE_D))
          for i in range(3) for k in range(2)]
    F += [('solid', "switch post", _circle(x, y, SW_POST_HOLE)) for (x, y, _) in BTNS]
    F += [('solid', "spigot hole", _circle(x, y, SPIGOT_HOLE_D)) for x, y in POSTS]
    F += [('solid', "encoder bushing hole", _circle(*KNOB_C, EC11_BUSH_D + 2 * EC11_HOLE_CLR))]
    if face == 'F':
        F += [('solid', "post", _circle(x, y, POST_D)) for x, y in POSTS]
        F += [('solid', "standoff", _circle(x, y, DISP_STANDOFF_D)) for x, y in disp_standoffs]
        F += [(net, "pocket %s" % n, _circle(x, y, DPIN_POCKET[0])) for (n, (x, y)), net in
              zip(disp_pins.items(), ('GND', '3V3', 'SCL', 'SDA', 'RES', 'DC', 'BLK'))]
    else:
        F += [('solid', "pillar %d" % i, _circle(x, y, PILLAR_D)) for i, (x, y) in enumerate(PILLARS)]
    return F


def _near(a, b, m):
    A, B = a.BoundBox, b.BoundBox
    return not (A.XMax + m < B.XMin or B.XMax + m < A.XMin or A.YMax + m < B.YMin or B.YMax + m < A.YMin)


def check_wires():
    """Plastic between different nets (and the board edge) on both faces. Returns (tightest mm, what, problems)."""
    board = carrier_outline().cut(carrier_through_cuts()[:len(HOOKS)])
    board = [f for f in board.Faces if abs(f.normalAt(0, 0).z + 1) < 1e-6][0].OuterWire
    tight, problems = (9e9, ""), []
    for face in ('F', 'B'):
        F = wire_features(face)
        for i, (na, la, sa) in enumerate(F):
            if la.startswith(("groove", "via", "pocket")):
                d = sa.distToShape(board)[0]
                if d < WIRE_RIDGE_MIN - 1e-6:
                    problems.append("%.2f %s / board edge" % (d, la))
            for nb, lb, sb in F[i + 1:]:
                if na == nb or (na == 'solid' and nb == 'solid') or not _near(sa, sb, WIRE_RIDGE_MIN + 0.5):
                    continue
                d = sa.distToShape(sb)[0]
                if d < tight[0]:
                    tight = (d, "%s: %s / %s" % (face, la, lb))
                if d < WIRE_RIDGE_MIN - 1e-6:
                    problems.append("%.2f %s: %s / %s" % (d, face, la, lb))
    # one groove, one wire: grooves of the same net may only meet inside the 2 mm round a shared pin or pocket
    for face in ('F', 'B'):
        gs = [(net, pts) for net, segs, _ in wire_routes() for fc, pts in segs if fc == face]
        for i, (na, pa) in enumerate(gs):
            for nb, pb in gs[i + 1:]:
                if na != nb:
                    continue
                ends = Part.makeCompound([_circle(*q, 2.0) for q in (pa[0], pa[-1], pb[0], pb[-1])])
                sa, sb = band_face(pa, GROOVE_W, 0).cut(ends), band_face(pb, GROOVE_W, 0).cut(ends)
                if sa.BoundBox.intersect(sb.BoundBox) and sa.common(sb).Area > 1e-3:
                    problems.append("%s grooves of net %s overlap" % (face, na))
    return tight, problems


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
    """A groove's outline at height z: the run's centre line grown to width w (round ends)."""
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


def peg(x, y, hole_d, z0, z1, clr=PEG_CLR):
    """Locating peg with a 0.3 mm lead-in chamfer at the tip."""
    d = hole_d - 2 * clr
    body = cyl(d, x, y, z0, z1 - 0.3)
    tip = Part.makeCone(d / 2, d / 2 - 0.3, 0.3, V(x, y, z1 - 0.3), V(0, 0, 1))
    return body.fuse(tip)


def switch_plate(cx, cy):
    """MX plate for one key, grown up from the carrier front: 1.5 thick, its top SW_PLATE_Z above the switch seat
    (the pocket floor), the cutout SW_CUTOUT (corners rounded), continuous all round.

    The 15.6 flange rests on the plate top and the clips snap in under it. Clips are on +-Y. The wall under
    the plate is flush with the cutout everywhere (no overhang), except for a blind pocket on the inside of
    each clip side: the plate overhangs it by OVH_LEDGE (0.5, a line width) and that crisp flat underside is
    the catch. The pocket's side walls and back wall lean in at 45 deg over the last 0.4 mm below the plate
    (they print without support and the internal corners are 135 deg, not 90), and there is no window
    through the wall. The east and west walls run the full height, so the plate is part of the carrier."""
    z0 = C_FRONT
    zb = SW_Z0 + SW_PLATE_Z - SW_PLATE_T
    zt = SW_Z0 + SW_PLATE_Z
    h = SW_CUTOUT / 2
    x_out = SW_BASE / 2 + 0.8
    y_s, y_n = -(SW_BASE / 2 + 0.9), SW_BASE / 2 + 1.6   # the south wall stops at the carrier's edge
    body = rr_prism(2 * x_out, y_n - y_s, 1.0, 0, (y_n + y_s) / 2, z0, zt)
    body = body.cut(rr_prism(SW_CUTOUT, SW_CUTOUT, 0.5, 0, 0, z0 - 1, zt + 1))   # cutout, rounded corners (no sharp inside corner)
    lean, strip = SW_CLIP_LEAN, OVH_LEDGE
    wide, deep = SW_CLIP_HALF + lean, h + strip + lean                    # the pocket at its floor
    for sg in (-1, 1):
        # plan: a rounded slot (r 0.5, its inner end lost in the cutout); the last `lean` under the plate is a 45 deg
        # chamfer all round, so the only flat ceiling is the catch strip: 2 * SW_CLIP_HALF wide, OVH_LEDGE deep
        y_in = sg * (h - 1.0)
        straight = rr_prism(2 * wide, abs(sg * deep - y_in), 0.5, 0, (y_in + sg * deep) / 2, z0 - 0.01, zb - lean)
        w0, w1 = 2 * wide, 2 * SW_CLIP_HALF
        d0, d1 = abs(sg * deep - y_in), abs(sg * (h + strip) - y_in)
        top = Part.makeLoft([_cap_section(w0, d0, 0.5, zb - lean, (y_in + sg * deep) / 2),
                             _cap_section(w1, d1, 0.4, zb, (y_in + sg * (h + strip)) / 2)], True, True)
        body = body.cut(straight).cut(top)
    body.translate(V(cx, cy, 0))
    return body


def make_carrier():
    """The printed PCB. Prints back side down; every holding feature points up from the front."""
    c = carrier_outline()
    cuts = carrier_through_cuts()
    c = c.cut(cuts)
    c = c.cut([box(x - SW_CUTOUT / 2, y - SW_CUTOUT / 2, SW_Z0, x + SW_CUTOUT / 2, y + SW_CUTOUT / 2, C_FRONT + 1)
               for (x, y, _) in BTNS])   # seat pocket: the bottom housing sinks in, the pins and post still pass the floor

    c = c.fuse(display_pillars() + [wire_cradle()] + [switch_plate(x, y) for (x, y, _) in BTNS])
    c = c.cut(groove_cuts())
    return c.removeSplitter()


def display_pillars():
    """The display's four pillars: vertical, of different heights (the far pair is taller), each topped by a seat cut square to the
    tilted board and a peg that leans with it. The board's header, which hangs under it, is kept clear of them."""
    hx, hy = disp_hdr_x, disp_hdr_y
    # the header's plastic and the pins' sweep (the pins lean towards the far pillar) with 0.3 / 0.6 of air round them
    header = tilt(box(hx - DISP_HDR_BODY[0] / 2 - 0.3, hy - DISP_HDR_BODY[1] / 2 - 0.3, W_HDR1 - 0.3,
                      hx + DISP_HDR_BODY[0] / 2 + 0.3, hy + DISP_HDR_BODY[1] / 2 + 0.3, W_PCB1 + 0.01))
    header = header.fuse(tilt(box(hx - 0.32 - PIN_PILLAR_CLR, hy - 3 * 2.54 - 0.32 - PIN_PILLAR_CLR, W_PIN_TIP - PIN_PILLAR_CLR,
                                  hx + 0.32 + PIN_PILLAR_CLR, hy + 3 * 2.54 + 0.32 + PIN_PILLAR_CLR, W_HDR1)))
    out = []
    for (x, ys), (xh, yh) in zip(disp_seat, disp_holes):
        if (x, ys) == max(disp_seat):
            z0 = C_FRONT + DISP_EAST_FOOT_H
            z1 = z0 + DISP_EAST_TAPER_H
            pillar = cyl(DISP_STANDOFF_D, x, ys, C_FRONT - 0.01, z0)
            pillar = pillar.fuse(Part.makeCone(DISP_STANDOFF_D / 2, DISP_EAST_D / 2, DISP_EAST_TAPER_H, V(x, ys, z0)))
            pillar = pillar.fuse(cyl(DISP_EAST_D, x, ys, z1, DISP_Z + 8)).removeSplitter()
            joins = [e for e in pillar.Edges if e.Curve.TypeId == "Part::GeomCircle"
                     and any(abs(e.CenterOfMass.z - z) < 1e-6 for z in (z0, z1))]
            pillar = pillar.makeFillet(DISP_EAST_FILLET, joins)
        else:
            pillar = cyl(DISP_STANDOFF_D, x, ys, C_FRONT - 0.01, DISP_Z + 8)
        pillar = pillar.cut(above_plane(W_PCB1)).cut(header)
        out += [pillar, tilt(peg(xh, yh, DISP_HOLE_D, W_PCB1 - 0.3, W_GLASS1 + PEG_PROUD))]
    return out


def wire_cradle():
    """Strain relief for the display wires: a low rib under the header whose top is parallel to the board, DISP_PIN_TIP_CLR under
    the pin tips (the pins stand free). Each wire has a funnel, a straight bore and, under it, its pocket in the carrier."""
    xs, ys = [p[0] for p in disp_pins.values()], [p[1] for p in disp_pins.values()]
    blk = box(xs[0] - CRADLE_W / 2, min(ys) - CRADLE_END, C_FRONT - 0.01, xs[0] + CRADLE_W / 2, max(ys) + CRADLE_END - CRADLE_END_TRIM, DISP_Z + 8)
    return blk.cut(above_plane(W_CRADLE))


def cradle_cuts():
    """Bore and funnel for every display wire (the pocket under the carrier's front is part of the bore)."""
    cuts = []
    for x, y in disp_pins.values():
        zt = plane_z(y, W_CRADLE)
        cuts.append(cyl(DPIN_POCKET[0], x, y, C_FRONT - DPIN_POCKET[1], zt + 1))
        cuts.append(Part.makeCone(DPIN_POCKET[0] / 2, DPIN_POCKET[0] / 2 + 1.0, 1.0, V(x, y, zt - (CRADLE_FUNNEL_R - DPIN_POCKET[0] / 2)), V(0, 0, 1)))
    return cuts


def knob_hub_z0():
    """z of the hub's underside (the bore's mouth): the shaft tip minus KNOB_ENGAGE."""
    return HW040_FRONT + EC11_SHAFT_H - KNOB_ENGAGE


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
    hub_z0 = knob_hub_z0()            # hub / bore entry: KNOB_ENGAGE under the shaft tip (below the skirt rim when the gap is large)
    if hub_z0 < bush_top + 0.5:
        raise ValueError("knob hub would reach the bushing tip")
    # hollow: skirt, top, and a hub round the shaft tied to the skirt by ribs. Prints top down,
    # so all of it grows up from the top with no overhang.
    inside = cyl(KNOB_D - 2 * KNOB_WALL, cx, cy, z0 - 1, z1 - KNOB_TOP_T)
    keep = [cyl(KNOB_HUB_D, cx, cy, hub_z0, z1)]
    for i in range(KNOB_RIBS):
        rib = box(0, -KNOB_RIB_T / 2, max(hub_z0, z0), KNOB_D / 2 - KNOB_WALL + 0.1, KNOB_RIB_T / 2, z1)
        keep.append(place(rib, cx, cy, 45.0 + i * 360.0 / KNOB_RIBS))
    if hub_z0 < z0:                   # the hub hangs below the skirt rim; the ribs stay flush with it
        k = k.fuse(cyl(KNOB_HUB_D, cx, cy, hub_z0, z0 + 0.5))
    cuts = flutes + [inside.cut(keep)]
    # bore: round over the shaft's round part, D-shaped over its flat
    bore = cyl(EC11_SHAFT_D + 0.25, cx, cy, hub_z0 - 1, shaft_top + KNOB_TIP_CLR)  # snug once FDM shrinks it
    flat = box(cx - 5, cy + EC11_SHAFT_D / 2 - EC11_FLAT + 0.1, shaft_top - EC11_FLAT_L + 0.5, cx + 5, cy + 5, shaft_top + 1)
    cuts.append(bore.cut(flat))
    cuts += [cyl(2.0, x, y, z1 - 1.2, z1 + 1) for x, y in KNOB_CAP_PEGS]
    return k.cut(cuts).removeSplitter()


def _mx_cross(length, arms, z0, z1):
    """A + centred on the origin: the bar along x is arms[1] thick (in y), the bar along y arms[0] (in x)."""
    return box(-length / 2, -arms[1] / 2, z0, length / 2, arms[1] / 2, z1).fuse(
        box(-arms[0] / 2, -length / 2, z0, arms[0] / 2, length / 2, z1))


def _cap_section(w, h, r, z, dy):
    wire = rr_wire(w, h, r, z)
    wire.translate(V(0, dy, 0))
    return wire


def _cap_loft(base, z0, z1, r):
    """Cap body from the skirt bottom z0 up to z1: `base` wide square, side walls CAP_SIDE_DEG, front (-y) wall
    CAP_FRONT_DEG, back wall CAP_BACK_DEG. The top stays flat, shifted back where the front leans more."""
    t = z1 - z0
    i_s, i_f, i_b = (t / math.tan(math.radians(d)) for d in (CAP_SIDE_DEG, CAP_FRONT_DEG, CAP_BACK_DEG))
    return Part.makeLoft([_cap_section(base, base, r, z0, 0.0),
                          _cap_section(base - 2 * i_s, base - i_f - i_b, r, z1, (i_f - i_b) / 2)], True, True)


def make_cap(i):
    """1u keycap on an MX cross socket, from the keycap drawing: tapered walls (80 deg sides, 70 deg front), flat top,
    a 5.5 mm stem tube with the cross socket, four ribs. No rounding on the top. Prints top down on the flat top.

    The skirt bottom sits SW_TRAVEL + CAP_CLR over the flange top, so at a full press it stops 0.35 mm above it;
    the skirt then rides inside the face opening, round the upper housing."""
    x, y, a = BTNS[i]
    z_top = BTN_CAP_PROUD
    z_bot = SW_Z0 + SW_FLANGE_Z + CAP_CLR + SW_TRAVEL
    z_ceil = z_top - CAP_TOP_T
    z_mouth = z_bot + CAP_SOCKET_UP
    outer = _cap_loft(BTN_CAP, z_bot, z_top, 1.5)
    hollow = _cap_loft(BTN_CAP - 2 * CAP_WALL, z_bot, z_ceil, 0.6).fuse(
        rr_prism(BTN_CAP - 2 * CAP_WALL, BTN_CAP - 2 * CAP_WALL, 0.6, 0, 0, z_bot - 1, z_bot + 0.01))
    tube = cyl(CAP_TUBE_D, 0, 0, z_mouth, z_ceil + 0.2)
    zr = z_bot + CAP_RIB_UP
    ribs = box(-BTN_CAP / 2, -CAP_RIB_T / 2, zr, BTN_CAP / 2, CAP_RIB_T / 2, z_ceil + 0.2).fuse(
        box(-CAP_RIB_T / 2, -BTN_CAP / 2, zr, CAP_RIB_T / 2, BTN_CAP / 2, z_ceil + 0.2)).common(outer)
    cap = outer.cut(hollow).fuse(tube).fuse(ribs)
    # the cross stops on the stem top, so the cap is located there as well as by the cross
    cap = cap.cut(_mx_cross(CAP_SLOT_L, CAP_SLOT_ARMS, z_mouth - 0.2, SW_STEM_TOP))
    return place(cap, x, y, a).removeSplitter()


def _void_half(cap, origin, step, axis, limit):
    """How far `axis` runs from origin through empty space before it hits the cap."""
    d = 0.0
    while d < limit:
        p = origin + axis * d
        if cap.isInside(p, 1e-7, True):
            return d
        d += step
    return limit


def check_pillars():
    """Measure the back plate pillars on the built solids: the gap from each pillar top to the carrier back, its plan
    clearance to every groove, via, pin hole and post on the back (and to the HW-040 window), and its 3D clearance to the wires,
    switch pins, encoder and RP2040. Returns (largest gap, smallest gap, tightest plan clearance, tightest 3D clearance)."""
    plate, car = PRINTED["BackPlate"], PRINTED["Carrier"]
    F = [f for f in wire_features('B') if not f[1].startswith("pillar")]
    ex, ey = HW040[0] / 2 + ENC_RELIEF, HW040[1] / 2 + ENC_RELIEF
    window = Part.Face(Part.makePolygon([V(hw_c[0] + sx * ex, hw_c[1] + sy * ey, 0) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1), (-1, -1))]))
    parts = [DUMMIES[n] for n in ("Dummy_Wire", "Dummy_Encoder", "Dummy_EncoderNut", "Dummy_RP2040_Zero",
                                  "Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight")]
    gaps, plan, near3 = [], 9e9, 9e9
    for i, (x, y) in enumerate(PILLARS):
        top = max(z for z in (C_BACK - 3 + k * 0.01 for k in range(300)) if plate.isInside(V(x, y, z), 1e-7, True))
        if abs(plate.distToShape(Part.Vertex(V(x, y, top)))[0]) > 1e-6:
            raise ValueError("pillar %d at (%.1f, %.1f) is not on the back plate" % (i, x, y))
        gap = C_BACK - top
        gaps.append(gap)
        circle = _circle(x, y, PILLAR_D)
        p_win = 0.0 if window.isInside(V(x, y, 0), 1e-7, True) else circle.distToShape(window)[0]
        p_plan = min([circle.distToShape(f[2])[0] for f in F if _near(circle, f[2], 3.0)] + [9e9])
        pillar = cyl(PILLAR_D, x, y, top - 4.0, top)
        p3 = min(pillar.distToShape(s)[0] for s in parts)
        c_gap = pillar.distToShape(car)[0]
        if abs(c_gap - gap) > 0.02 or gap > PILLAR_GAP + 0.02 or gap < 0.0:
            raise ValueError("pillar %d: gap to the carrier back %.2f (want %.1f)" % (i, c_gap, PILLAR_GAP))
        if p_plan < WIRE_RIDGE_MIN - 1e-6 or p3 < 0.5 or p_win < PILLAR_WIN - 1e-6:
            raise ValueError("pillar %d at (%.1f, %.1f): plan clearance %.2f, to the HW-040 window %.2f, 3D clearance %.2f to the wires, pins, encoder or RP2040" % (
                i, x, y, p_plan, p_win, p3))
        plan, near3 = min(plan, p_plan), min(near3, p3)
    return max(gaps), min(gaps), plan, near3


def check_switch_plate():
    """Measure the plate on the built solids, not the constants: top over the seat (the pocket floor), thickness, cutout,
    and that each clip hook of the dummy switch reaches under the plate's underside. Returns the figures."""
    car, sw = PRINTED["Carrier"], DUMMIES["Dummy_SwitchMiddle"]
    x, y, _ = BTNS[1]

    def span(shape, p, axis, lo, hi, step=0.01):
        """First and last coordinate along `axis` that are solid, between lo and hi."""
        hit = [v for v in (lo + k * step for k in range(int((hi - lo) / step) + 1))
               if shape.isInside(V(*[p[i] if i != axis else v for i in range(3)]), 1e-7, True)]
        return (hit[0], hit[-1]) if hit else (None, None)

    floor = span(car, (x + 5.5, y - 5.0, 0), 2, C_BACK - 0.5, C_FRONT + 1)[1]          # pocket floor top
    gy = y + SW_CUTOUT / 2 + 0.25                                                  # inside the clip pocket, only the plate is above it
    p0, p1 = span(car, (x, gy, 0), 2, C_FRONT + 0.05, SW_Z0 + 8)
    cut_w = 2 * (span(car, (0, y, SW_Z0 + SW_PLATE_Z - SW_PLATE_T / 2), 0, x, x + 9)[0] - x)
    top, thick = p1 - floor, p1 - p0
    hook = span(sw, (x, y + SW_CUTOUT / 2 + 0.3, 0), 2, SW_Z0, p0 - 0.001)[1]          # hook top, just under the plate
    reach = (y + SW_CLIPS / 2) - (y + SW_CUTOUT / 2)
    if abs(top - SW_PLATE_Z) > 0.05 or abs(thick - SW_PLATE_T) > 0.05 or cut_w > 14.1 + 0.02 or cut_w < SW_LOWER:
        raise ValueError("switch plate: top %.2f over the seat (want %.1f), thickness %.2f (want %.1f), cutout %.2f (want <= 14.1)" % (
            top, SW_PLATE_Z, thick, SW_PLATE_T, cut_w))
    if hook is None or abs(hook - p0) > 0.05 or reach < 0.3:
        raise ValueError("switch clip does not reach under the plate: hook top %s, plate underside %.2f, reach %.2f" % (hook, p0, reach))
    return top, thick, cut_w, reach


def check_cap_socket():
    """Measure the middle cap's cross socket, and that a full travel still clears the housing, the plate and the face."""
    cap = PRINTED["CapMiddle"]
    x, y, _ = BTNS[1]
    z = SW_STEM_TOP - 1.0
    # out on each bar, past the other bar, so the thickness is the one being measured
    arm_y = _void_half(cap, V(x + CAP_SLOT_L * 0.35, y, z), 0.01, V(0, 1, 0), 2.0)
    arm_x = _void_half(cap, V(x, y + CAP_SLOT_L * 0.35, z), 0.01, V(1, 0, 0), 2.0)
    length = _void_half(cap, V(x, y, z), 0.01, V(1, 0, 0), 4.0)
    measured = (2 * arm_x, 2 * arm_y, 2 * length)
    expect = CAP_SLOT_ARMS + (CAP_SLOT_L,)
    if any(abs(m - e) > 0.04 for m, e in zip(measured, expect)):
        raise ValueError("MX socket measures %s, wanted %s (arm x, arm y, length)" % (measured, expect))
    extra = (CAP_SLOT_ARMS[0] - SW_STEM_ARMS[0], CAP_SLOT_ARMS[1] - SW_STEM_ARMS[1], CAP_SLOT_L - SW_STEM_L)
    if min(extra) < 0.05 - 1e-6 or max(extra) > 0.15 + 1e-6:
        raise ValueError("socket extra over the stem %s is outside 0.05-0.15 mm" % (extra,))
    pressed = cap.copy()
    pressed.translate(V(0, 0, -SW_TRAVEL))
    # the stem rides with the cap, so test against the switch body without it (its stem well takes the tube)
    gaps = {n: pressed.distToShape(s)[0] for n, s in (("housing", DUMMIES["Dummy_SwitchMiddle"]),
                                                      ("plate", PRINTED["Carrier"]), ("face", PRINTED["Shell"]))}
    if any(pressed.common(s).Volume > 0.01 for s in (DUMMIES["Dummy_SwitchMiddle"], PRINTED["Carrier"], PRINTED["Shell"])):
        raise ValueError("cap hits the housing, the plate or the face before the switch bottoms")
    if min(gaps.values()) < 0.3 - 1e-6:
        raise ValueError("cap clearance at full travel below 0.3 mm: %s" % gaps)
    return measured, extra


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


def display_parts():
    """The module's glass, board, header plastic and pins, in the face frame (tilted)."""
    glass = cbox(glass_c[0], glass_c[1], DISP_GLASS[0], DISP_GLASS[1], W_GLASS1, W_GLASS0)
    pcb = cbox(dpcb_c[0], dpcb_c[1], DISP_PCB[0], DISP_PCB[1], W_PCB1, W_GLASS1)
    pcb = pcb.cut([cyl(DISP_HOLE_D, x, y, W_PCB1 - 1, W_GLASS1 + 1) for (x, y) in disp_holes])
    hdr = cbox(disp_hdr_x, disp_hdr_y, DISP_HDR_BODY[0], DISP_HDR_BODY[1], W_HDR1, W_PCB1)
    pins = fuse_all([cbox(x, y, 0.64, 0.64, W_PIN_TIP, W_HDR1) for (x, y) in disp_pins_m.values()])
    return {n: tilt(shp) for n, shp in (("glass", glass), ("pcb", pcb), ("header", hdr), ("pins", pins))}


def dummy_display():
    return fuse_all(list(display_parts().values()))


def dummy_fpc():
    """The flex cable's keep-out (assumed, see DISP_FPC): from the glass's end away from the header to the board's end."""
    xg = glass_c[0] - DISP_HEADER_SIDE * DISP_GLASS[0] / 2
    xb = dpcb_c[0] - DISP_HEADER_SIDE * DISP_PCB[0] / 2
    fpc = box(min(xg, xb), glass_c[1] - DISP_FPC[0] / 2, W_GLASS1, max(xg, xb), glass_c[1] + DISP_FPC[0] / 2, W_GLASS1 + DISP_FPC[1])
    return tilt(fpc)


def dummy_switch(i):
    """Outemu MX stand-in (without the stem, which is its own part so it can have its own colour): bottom housing
    with clips, 15.6 flange on the plate, tapered upper housing with the stem well, centre post and two pins."""
    x, y, a = BTNS[i]
    z_p = SW_Z0 + SW_PLATE_Z
    low = rr_prism(SW_LOWER, SW_LOWER, 0.4, x, y, SW_Z0, z_p + 0.1, a)
    flange = rr_prism(SW_BASE, SW_BASE, 0.6, x, y, z_p, SW_Z0 + SW_FLANGE_Z, a)
    upper = place(Part.makeLoft([rr_wire(SW_UPPER[0], SW_UPPER[0], 0.6, SW_Z0 + SW_FLANGE_Z - 0.01),
                                 rr_wire(SW_UPPER[1], SW_UPPER[1], 0.6, SW_Z0 + SW_HOUSING_H)], True, True), x, y, a)
    well = rr_prism(SW_WELL[0], SW_WELL[0], 0.4, x, y, SW_Z0 + SW_HOUSING_H - SW_WELL[1], SW_Z0 + SW_HOUSING_H + 0.2, a)
    post = cyl(SW_POST_D, x, y, SW_Z0 - SW_POST_L, SW_Z0 + 0.01)
    pins = [cbox(*btn_pin(i, k), SW_PIN[0], SW_PIN[1], SW_Z0 - SW_PIN_L, SW_Z0 + 0.01, a) for k in range(2)]
    # clips on +-Y: hooks out to 15.03 over the clips, under the plate's underside (they run from 1.9 below the plate top)
    clips = [place(box(-2.0, s * (SW_LOWER / 2 - 0.1), z_p - SW_CLIP_H, 2.0, s * SW_CLIPS / 2, z_p - SW_PLATE_T), x, y, a)
             for s in (-1, 1)]
    return fuse_all([low, flange, upper, post] + pins + clips).cut(well)


def dummy_stem(i):
    x, y, a = BTNS[i]
    return place(_mx_cross(SW_STEM_L, SW_STEM_ARMS, SW_Z0 + SW_HOUSING_H - 1.0, SW_STEM_TOP), x, y, a)


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


def _pipe(pts):
    path = Part.Wire(run_edges(pts, None))
    e = path.OrderedEdges[0]
    profile = Part.Wire(Part.makeCircle(WIRE_D / 2, e.Vertexes[0].Point, e.tangentAt(e.FirstParameter)))
    return path.makePipeShell([profile], True, True)


def _toward(p, q, d):
    L = math.hypot(q[0] - p[0], q[1] - p[1])
    return (p[0] + (q[0] - p[0]) / L * d, p[1] + (q[1] - p[1]) / L * d)


def dummy_wire():
    """Every wire of wire_routes() as a swept circle in its groove, with its wire-wrap rings and the tails down the display
    pins; grouped by net in WIRE_BY_NET."""
    groups, wraps = {}, {}
    DISP_TAILS.clear()
    ring = lambda x, y, z0, g=0.0: cyl(WRAP[0] + g, x, y, z0, z0 + WRAP[2]).cut(cyl(WRAP[1] + g, x, y, z0 - 1, z0 + WRAP[2] + 1))
    name_at = {xy: n for n, xy in disp_pins.items()}   # pocket position -> display pin
    for net, segs, ends in wire_routes():
        parts = []
        for k, (face, pts) in enumerate(segs):
            pts = list(pts)
            for end, at in ((0, ends[0]), (-1, ends[1])):
                if (end == 0 and k != 0) or (end == -1 and k != len(segs) - 1):
                    continue
                kind, pin = at
                nxt = pts[1] if end == 0 else pts[-2]
                if kind == 'disp':     # ring at the pin root (along the tilted pin), then a straight tail to the cradle's funnel,
                    #                     down its bore to the pocket: the wire never leans on the pin
                    n = name_at[pin]
                    i = wraps.setdefault(pin, [])
                    i.append(1)
                    top = W_ROOT - WRAP[2] * (len(i) - 1)
                    px, py = disp_pins_m[n]
                    parts.append(tilt(cyl(WRAP[0], px, py, top - WRAP[2], top).cut(cyl(WRAP[1], px, py, top - WRAP[2] - 1, top + 1))))
                    p0 = tp(pin[0], py, top - WRAP[2] / 2)
                    zt = plane_z(pin[1], W_CRADLE)
                    tail = _pipe([(p0.x, p0.y, p0.z), (pin[0], pin[1], zt + 0.3), (pin[0], pin[1], WIRE_Z_FRONT)])
                    DISP_TAILS.append(tail)
                    parts.append(tail)
                else:
                    pts[end] = _toward(pin, nxt, WIRE_ATT)
                    z0 = {'rp': C_FRONT + 0.05, 'enc': C_BACK - 0.05 - WRAP[2], 'leg': C_BACK - 0.05 - WRAP[2]}[kind]
                    parts.append(ring(pin[0], pin[1], z0, 0.2 if kind == 'leg' else 0.0))   # a key pin is 0.85 x 0.78: its diagonal needs a wider bore
            if face == 'B':
                parts.append(_pipe([(x, y, WIRE_Z_BACK) for x, y in pts]))
                continue
            parts.append(_pipe([(x, y, WIRE_Z_FRONT) for x, y in pts]))
        for (_, p1), _s in zip(segs, segs[1:]):
            parts.append(cyl(WIRE_D, p1[-1][0], p1[-1][1], WIRE_Z_BACK, WIRE_Z_FRONT))
        groups.setdefault(net, []).extend(parts)
    WIRE_BY_NET.clear()
    WIRE_BY_NET.update({net: Part.makeCompound(shapes) for net, shapes in groups.items()})
    return Part.makeCompound(list(WIRE_BY_NET.values()))


# ---------------------------------------------------------------------------------------------
# Build, check, export
# ---------------------------------------------------------------------------------------------

PRINTED = {}
DUMMIES = {}
WIRE_BY_NET = {}
DISP_TAILS = []

COLORS = {
    "Shell": (0.07, 0.07, 0.08), "DisplayCowl": (0.07, 0.07, 0.08),
    "DisplayLip": (0.90, 0.90, 0.88), "KnobCap": (0.90, 0.90, 0.88),
    "HaloDiffuser": (0.90, 0.90, 0.88), "PinFitCoupon": (0.15, 0.15, 0.16),
    "BackPlate": (0.90, 0.90, 0.88),
    "Carrier": (0.15, 0.15, 0.16), "Knob": (0.07, 0.07, 0.08),
    "CapLeft": (0.90, 0.90, 0.88), "CapMiddle": (0.90, 0.90, 0.88), "CapRight": (0.90, 0.90, 0.88),
    "Dummy": (0.30, 0.45, 0.85), "Dummy_Wire": (0.85, 0.45, 0.20), "Dummy_FPC": (0.95, 0.75, 0.10),
}



def build():
    PRINTED["Shell"] = make_shell()
    PRINTED["DisplayCowl"] = make_cowl()
    PRINTED["DisplayLip"] = make_lip()
    PRINTED["KnobCap"] = make_knob_cap()
    PRINTED["HaloDiffuser"] = make_halo()
    PRINTED["PinFitCoupon"] = make_pin_coupon()
    PRINTED["BackPlate"] = make_back_plate()
    PRINTED["Carrier"] = make_carrier()
    PRINTED["Knob"] = make_knob()
    for i, n in enumerate(["CapLeft", "CapMiddle", "CapRight"]):
        PRINTED[n] = make_cap(i)
    DUMMIES["Dummy_Encoder"] = dummy_encoder()
    DUMMIES["Dummy_EncoderNut"] = dummy_encoder_nut()
    DUMMIES["Dummy_Display"] = dummy_display()
    DUMMIES["Dummy_FPC"] = dummy_fpc()
    DUMMIES["Dummy_RP2040_Zero"] = dummy_rp2040_zero()
    DUMMIES["Dummy_Wire"] = dummy_wire()
    for i, n in enumerate(["Left", "Middle", "Right"]):
        DUMMIES["Dummy_Switch" + n] = dummy_switch(i)
        DUMMIES["Dummy_Stem" + n] = dummy_stem(i)


PRESS_FITS = set()  # the MX socket is a clearance fit; it does not bite the stem in the model



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



def check_wire_shorts():
    """Reject modeled overlaps between insulated wires carrying different nets. Wires may cross, but only
    stacked (none do in the current routes)."""
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
            raise RuntimeError("Save or close the modified V8 document before rebuilding")
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
    gap = 1.5                                         # from the pin to the text
    out = []
    for n, p in sm_pins.items():
        side = 1 if p[0] > SM_X else -1               # outside each row
        t = n if n in ("GND", "3V3") else "%s: %s" % (n, RP_NETS.get(n, "n/c"))
        out.append((t, p[0] + side * gap, p[1], side))
    for n, p in disp_pins.items():
        out.append(("Disp " + n, p[0] + gap, p[1], 1))
    for n, p in enc_pins.items():
        out.append(("Enc " + n, p[0] - 1.2, p[1], -1))
    # switch pins: pad 0 ground (toward the left), pad 1 the key signal
    for i, sw in enumerate(["L", "M", "R"]):
        for k, tag in ((0, "GND"), (1, "B%d" % (i + 1))):
            p = btn_pin(i, k)
            out.append(("%s %s" % (sw, tag), p[0], p[1] + (1.6 if k else -1.6), 0))
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
    if name in ("DisplayCowl", "DisplayLip"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180 - DISP_TILT)   # the tilted top plane on the bed
    elif name in ("Shell", "Knob", "KnobCap", "HaloDiffuser") or name.startswith("Cap"):
        s.rotate(V(0, 0, 0), V(1, 0, 0), 180)       # face / knob top / cap top on the bed
    elif name == "BackPlate":
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
    # Include trimmed-face tessellation points: a UV grid alone can miss a narrow face
    # completely and return zero reach without having sampled its interior.
    mesh, triangles = f.tessellate(0.05)
    probes = list(mesh) + [sum((mesh[k] for k in t), V()) / 3 for t in triangles]
    reach = max((sc.distToShape(Part.Vertex(p))[0] for p in probes), default=0.0)
    u0, u1, v0, v1 = f.ParameterRange
    for i in range(11):
        for j in range(11):
            u, v = u0 + (u1 - u0) * i / 10, v0 + (v1 - v0) * j / 10
            if f.isPartOfDomain(u, v):
                reach = max(reach, sc.distToShape(Part.Vertex(f.valueAt(u, v)))[0])
    return ("bridge", 2 * reach) if around else ("ledge", reach)


def face_samples(f):
    """Points and exact surface normals on every trimmed face, including tiny faces.

    Tessellation guarantees an interior sample even when a face misses the UV grid.
    Boundary normals also catch steep ends of a curved face; no coarse-pass early exit.
    """
    if f.Surface.TypeId == "Part::GeomPlane":
        return [(f.CenterOfMass, f.normalAt(0, 0))]
    vertices, triangles = f.tessellate(0.05)
    points = list(vertices) + [sum((vertices[k] for k in t), V()) / 3 for t in triangles]
    samples = []
    for p in points:
        u, v = f.Surface.parameter(p)
        samples.append((f.valueAt(u, v), f.normalAt(u, v)))
    u0, u1, v0, v1 = f.ParameterRange
    for i in range(17):
        for j in range(17):
            u, v = u0 + (u1 - u0) * i / 16, v0 + (v1 - v0) * j / 16
            if f.isPartOfDomain(u, v):
                samples.append((f.valueAt(u, v), f.normalAt(u, v)))
    if not samples:
        raise ValueError("overhang audit could not sample a face")
    return samples


OVERHANG_REPORT = {}


def check_overhangs():
    """Audit every face in exported print pose, with no small-face exclusion.

    Downward faces over OVH_MAX require supported edges on both sides of a bridge
    <=BRIDGE_MAX, or a ledge <=OVH_LEDGE (one extrusion width). Curved USB ends
    widen their roof's bridge; standalone curved groove roofs need their own supported
    edges. Bed faces are counted separately. Report both the self-supporting and absolute worst surface angles.
    """
    # OCCT's approximated BSpline chamfers differ from 45 degrees by a few 1e-5
    # degrees at trim boundaries. This is numerical tolerance, not a print allowance.
    lim = math.sin(math.radians(OVH_MAX + 0.001))
    problems, summary = [], []
    OVERHANG_REPORT.clear()
    for name, shp in PRINTED.items():
        s = print_pose(name, shp)
        roofs, steep = [], []
        worst, supported, ledge, bed_faces = 0.0, 0.0, 0.0, 0
        for f in s.Faces:
            samples = face_samples(f)
            if f.BoundBox.ZMax < 1e-6:
                bed_faces += 1
                continue
            angle = max(math.degrees(math.asin(max(0.0, min(1.0, -n.z)))) for p, n in samples)
            worst = max(worst, angle)
            bad = [p for p, n in samples if -n.z > lim]
            if not bad:
                supported = max(supported, angle)
                continue
            if f.Surface.TypeId == "Part::GeomPlane":
                roofs.append([f, flat_roof(s, f), 0.0])
            else:
                steep.append((f, bad))
        for f, bad in steep:  # a curved face running on from a roof belongs to its bridge
            xs, ys = [p.x for p in bad], [p.y for p in bad]
            host = [r for r in roofs if any(e.isSame(e2) for e in f.Edges for e2 in r[0].Edges)]
            if host:
                host[0][2] += max(max(xs) - min(xs), max(ys) - min(ys))
            else:
                # A curved groove roof may bridge between its own supported edges even
                # without an adjacent planar roof. Audit those edges rather than skip it.
                roofs.append([f, flat_roof(s, f), 0.0])
        bridges = []
        for f, (kind, w), extra in roofs:
            c = f.CenterOfMass
            if kind == "bridge" and w + extra <= BRIDGE_MAX + 1e-6:
                bridges.append(w + extra)
            elif kind == "ledge" and w <= OVH_LEDGE + 1e-6:
                ledge = max(ledge, w)
                continue
            else:
                problems.append("%s: %s %.1f mm at (%.1f, %.1f), z %.1f" % (name, kind, w + extra, c.x, c.y, c.z))
        OVERHANG_REPORT[name] = dict(faces=len(s.Faces), bed_faces=bed_faces,
                                    worst_angle=worst, self_support_angle=supported,
                                    longest_bridge=max(bridges, default=0.0), ledge=ledge)
        summary.append("%s: %d faces, self-support %.2f deg, worst %.2f deg incl. bridges/line-width ledges; "
                       "bridge %.2f mm; ledge %.2f mm" % (name, len(s.Faces), supported, worst,
                                                         max(bridges, default=0.0), ledge))
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
    doc.saveAs(os.path.join(here, "kivori-case-v8.FCStd"))


def check_usb_hole():
    """The real connector sits USB_HOLE_DZ lower than modelled (see there): it has to clear the hole all round. Returns the
    smallest margin (mm) between the shifted connector envelope and the hole, in width and height."""
    uz = usb_z + USB_HOLE_DZ
    return min(USB_HOLE[0] / 2 - SM_USB[0] / 2,
               (uz + USB_HOLE[1] / 2) - (usb_hi + USB_HOLE_DZ), (usb_lo + USB_HOLE_DZ) - (uz - USB_HOLE[1] / 2))


def eye_hidden():
    """How much of the active area the near rim (the lip, or the cowl under it) hides from a seated eye EYE_ELEV above the desk,
    in front of the case, measured along the glass from its near edge: rays from points on the glass towards the eye, tested against
    the cowl and the lip. Returns (hidden mm, ray elevation over the glass plane)."""
    E = math.radians(EYE_ELEV) + ALPHA                       # eye elevation over the face plane (the case wedge adds ALPHA)
    e = V(0, -math.cos(E), math.sin(E))
    occ = [PRINTED["DisplayCowl"], PRINTED["DisplayLip"]]
    n = tp(0, DISP_C[1], 1.0) - tp(0, DISP_C[1], 0.0)        # glass normal

    def blocked(v):
        p = tp(DISP_C[0], DISP_C[1] + v, 0.0) + n * 0.01
        return any(o.isInside(p + e * (0.1 * k), 1e-6, True) for k in range(1, 130) for o in occ)

    v0 = -DISP_ACTIVE / 2
    v = v0
    while v < DISP_ACTIVE / 2 and blocked(v):
        v += 0.02
    return v - v0, math.degrees(math.acos(max(-1.0, min(1.0, e.dot(V(0, math.cos(math.radians(DISP_TILT)), math.sin(math.radians(DISP_TILT)))))))) \
        if False else math.degrees(math.asin(e.dot(n)))


def check_display():
    """Everything that protects the tilted display. Prints a summary and returns the problems."""
    problems = []
    parts = display_parts()
    glass, pcb, pins = parts["glass"], parts["pcb"], parts["pins"]
    dists = {nm: glass.distToShape(sh)[0] for nm, sh in PRINTED.items()}
    nm, g = min(dists.items(), key=lambda kv: kv[1])
    roof = glass.distToShape(PRINTED["DisplayCowl"].common(tilt(box(-100, DISP_C[1] - 100, W_ROOF0 - 0.01, 200, DISP_C[1] + 100, W_TOP + 1))))[0]
    print("glass: nothing printed within %.2f mm (nearest: %s, beside the header-end pads); air under the cowl roof %.2f, under the lip's collar %.2f" % (
        g, nm, roof, glass.distToShape(PRINTED["DisplayLip"])[0]))
    if g < 0.3 - 1e-6:
        problems.append("glass air gap %.2f < 0.3 (%s)" % (g, nm))
    pil = display_pillars()
    seats = [pcb.distToShape(q)[0] for q in pil[0::2]]
    hts = [tp(x, y, W_PCB1).z - C_FRONT for x, y in disp_holes]
    print("board: rests on %d pillar seats (gaps %s mm), pillar heights near %.1f / far %.1f mm (far pair %.1f taller), board and pegs tilted %.0f°" % (
        len(seats), ", ".join("%.2f" % d for d in seats), min(hts), max(hts), max(hts) - min(hts), DISP_TILT))
    if max(seats) > 1e-3:
        problems.append("board does not touch every pillar seat: %s" % seats)
    pad = pcb.distToShape(PRINTED["DisplayCowl"])[0]
    pegs = pcb.distToShape(PRINTED["DisplayCowl"])[0]
    peg_pad = min(q.distToShape(PRINTED["DisplayCowl"])[0] for q in pil[1::2])
    print("board: the cowl's pads stand %.2f mm over it (hold-down, never a load); the pegs' tips are %.2f from the pad bores" % (pad, peg_pad))
    if abs(pad - DISP_PAD_GAP) > 0.05 or peg_pad < 0.3:
        problems.append("pad gap %.2f / peg to pad %.2f" % (pad, peg_pad))
    fpc = DUMMIES["Dummy_FPC"]
    fd = {nm: fpc.distToShape(sh)[0] for nm, sh in PRINTED.items()}
    nm, f = min(fd.items(), key=lambda kv: kv[1])
    print("flex cable keep-out (%.0f wide, up to %.1f over the board): nearest printed part %.2f mm (%s); roof over the board %.1f mm" % (
        DISP_FPC[0], DISP_FPC[1], f, nm, DISP_GLASS_AIR + DISP_GLASS[2]))
    if f < DISP_FPC_AIR_MIN - 1e-6:
        problems.append("flex keep-out %.2f mm from %s" % (f, nm))
    cr = wire_cradle().cut(cradle_cuts())
    pt = min(pins.distToShape(q)[0] for q in [cr] + [display_pillars()[i] for i in (0, 2, 4, 6)])
    pw = min(pins.distToShape(t)[0] for t in DISP_TAILS)
    print("header pins (stand free): tips %.2f mm from the cradle top, nearest pillar slot / anything printed %.2f mm; wire tails %.2f mm from the pins" % (
        pins.distToShape(cr)[0], pins.distToShape(PRINTED["Carrier"])[0], pw))
    if pt < DISP_PIN_TIP_CLR - 1e-3 or pw < 0.2:
        problems.append("header pins %.2f mm from the carrier (< %.1f) / tails %.2f from the pins" % (pt, DISP_PIN_TIP_CLR, pw))
    spans = []
    for nme, (x, y) in disp_pins.items():
        px, py = disp_pins_m[nme]
        p0 = tp(x, py, W_ROOT - WRAP[2] / 2)
        spans.append(math.dist((p0.x, p0.y, p0.z), (x, y, plane_z(y, W_CRADLE) + 0.3)))
    lead = math.degrees(math.atan2(abs(disp_pins[DISP_PINS[3]][1] - tp(*disp_pins_m[DISP_PINS[3]], W_ROOT).y),
                                   tp(*disp_pins_m[DISP_PINS[3]], W_ROOT).z - plane_z(disp_pins[DISP_PINS[3]][1], W_CRADLE)))
    print("wires: free span from the pin root to the cradle funnel %.1f..%.1f mm, straight (leans %.0f° the other way from the pins), funnel and "
          "%.1f bore, foot bend R %.1f mm; wrap ring R %.2f" % (min(spans), max(spans), lead, DPIN_POCKET[0], DPIN_POCKET[0] / 2, WRAP[0] / 2))
    hid, ang = eye_hidden()
    print("rim: hides %.2f mm of the active area for a seated eye (%.0f° over the desk, %.1f° over the glass); limit %.1f. Lip covers the glass edge by %.2f (near/far), %.2f (header end), %.2f (flex end) mm" % (
        hid, EYE_ELEV, ang, RIM_HIDDEN_MAX, DISP_GLASS[1] / 2 - DISP_ACTIVE / 2 - DISP_WIN_MARGIN,
        glass_c[0] + DISP_GLASS[0] / 2 - (DISP_C[0] + DISP_ACTIVE / 2 + DISP_WIN_MARGIN),
        (DISP_C[0] - DISP_ACTIVE / 2 - DISP_WIN_MARGIN) - (glass_c[0] - DISP_GLASS[0] / 2)))
    if hid > RIM_HIDDEN_MAX:
        problems.append("rim hides %.2f mm of the active area" % hid)
    return problems


def check_pillar_lip():
    """Measure the approved far-east support and the lip in its exported print orientation."""
    problems = []
    x, y = max(disp_seat)
    idx = disp_seat.index((x, y))
    pillar = display_pillars()[2 * idx]
    pins = fuse_all([tilt(cbox(px, py, 0.64, 0.64, W_PIN_TIP, W_HDR1))
                     for px, py in disp_pins_m.values()])
    pin_gap = pillar.distToShape(pins)[0]
    wire_gap = min(pillar.distToShape(t)[0] for t in DISP_TAILS)
    wrap_gap = pillar.distToShape(DUMMIES["Dummy_Wire"])[0]
    cradle_gap = pillar.distToShape(wire_cradle())[0]
    section = Part.Face(pillar.slice(V(0, 0, 1), -2.0)[0])
    diameter = section.BoundBox.XLength
    moments = []
    for z in (-3.8, 0.0, 3.0):
        f = Part.Face(pillar.slice(V(0, 0, 1), z)[0])
        m = f.MatrixOfInertia
        moments.append((m.A11 + m.A22 - math.sqrt((m.A11 - m.A22)**2 + 4 * m.A12**2)) / 2)
    full_i = math.pi * DISP_STANDOFF_D**4 / 64
    weak = min(moments)
    seat = pillar.common(above_plane(W_PCB1 - 0.001)).Volume / 0.001
    print("far-east pillar: axis %.2f, %.2f; upper diameter %.2f, foot diameter %.2f x %.2f high, "
          "45 degree taper %.2f high / fillets R%.2f; pin %.3f, wires %.3f, cradle %.3f mm; "
          "weakest section I %.3f mm4 (%.1f %% of plain 4.5), seat %.2f mm2" % (
              x, y, diameter, DISP_STANDOFF_D, DISP_EAST_FOOT_H, DISP_EAST_TAPER_H, DISP_EAST_FILLET,
              pin_gap, wire_gap, cradle_gap, weak, 100 * weak / full_i, seat))
    print("pillar to all modeled wire including wrap rings: %.3f mm (tail-only clearance above matches the concept analysis)" % wrap_gap)
    if wrap_gap < 0.3 - 1e-6:
        problems.append("pillar to wrap rings below 0.3 mm")
    if abs(diameter - DISP_EAST_D) > 1e-6 or not pillar.isValid():
        problems.append("far-east pillar diameter / validity")
    if pin_gap < 0.8 - 1e-6 or wire_gap < 0.95 or cradle_gap < 0.79 - 1e-6:
        problems.append("far-east pillar clearance pin %.3f wire %.3f cradle %.3f" % (pin_gap, wire_gap, cradle_gap))
    lip = print_pose("DisplayLip", PRINTED["DisplayLip"])
    bed = sum(f.Area for f in lip.Faces if f.BoundBox.ZMax < 1e-6 and f.normalAt(0, 0).z < -0.99)
    hidden, _ = eye_hidden()
    print("lip: width %.2f, height %.2f, first-layer face %.2f mm2 (>=100), 45 degree overhang %.2f mm (<=0.4), "
          "rim hides %.3f mm (<=0.5); prints top face down" % (LIP_W, LIP_H, bed, LIP_CHAMFER, hidden))
    if bed < 100 or LIP_CHAMFER > 0.4 + 1e-6 or hidden > RIM_HIDDEN_MAX:
        problems.append("lip first layer / overhang / rim")
    return problems


def check_knob_engagement():
    """Measured on the knob solid, not from the parameters: how far the shaft tip sits above the hub's underside (read on the round
    side of the bore wall, where the D flat does not interfere), and the knob's gap to the bushing and to the nut, at rest and pressed
    0.5 mm. Raises when the engagement is under KNOB_ENGAGE or the knob comes within 0.5 mm of the bushing tip or the nut.
    Returns (engagement, to bushing at rest, to bushing pressed, to nut pressed)."""
    cx, cy = KNOB_C
    shaft_top = HW040_FRONT + EC11_SHAFT_H
    knob = PRINTED["Knob"]
    wall = Part.makeLine(V(cx, cy - (EC11_SHAFT_D + 0.25) / 2 - 0.1, shaft_top - 10), V(cx, cy - (EC11_SHAFT_D + 0.25) / 2 - 0.1, KNOB_GAP + KNOB_H + 1))
    hit = wall.common(knob)
    eng = shaft_top - min(e.BoundBox.ZMin for e in hit.Edges)
    bush = cyl(EC11_BUSH_D, cx, cy, HW040_FRONT + EC11_BODY_H, HW040_FRONT + EC11_BODY_H + EC11_BUSH_L)
    pressed = knob.copy()
    pressed.translate(V(0, 0, -0.5))
    d_rest, d_press = knob.distToShape(bush)[0], pressed.distToShape(bush)[0]
    d_nut = pressed.distToShape(DUMMIES["Dummy_EncoderNut"])[0]
    if eng < KNOB_ENGAGE - 1e-6:
        raise ValueError("knob shaft engagement %.2f mm is under %.1f mm" % (eng, KNOB_ENGAGE))
    if abs(knob.BoundBox.ZMin - KNOB_GAP) > 1e-6 or abs(knob_hub_z0() - KNOB_GAP) > 1e-6:
        raise ValueError("knob hub must be flush with the skirt rim")
    if min(d_press, d_nut) < 0.5 - 1e-6:
        raise ValueError("pressed knob within 0.5 mm of the encoder bushing / nut")
    return eng, d_rest, d_press, d_nut


def report_new_features():
    """Numbers for the second-print changes: knob, cowl pegs, RP2040 drop."""
    shaft_top = HW040_FRONT + EC11_SHAFT_H
    hub_z0 = knob_hub_z0()
    print("knob: skirt z %.1f..%.1f (H %.1f), hub from z %.1f, shaft enters %.2f mm (D flat %.1f mm long, all inside), tip clearance %.1f" % (
        KNOB_GAP, KNOB_GAP + KNOB_H, KNOB_H, hub_z0, shaft_top - hub_z0, min(EC11_FLAT_L, shaft_top - hub_z0), KNOB_TIP_CLR))
    eng, bush_rest, bush_pressed, nut_pressed = check_knob_engagement()
    print("knob measured on the solid: shaft enters %.2f mm (rule >= %.1f); hub underside to the bushing tip %.2f mm at rest, %.2f pressed "
          "(0.5 mm travel; rule >= 0.5), to the nut %.2f pressed" % (eng, KNOB_ENGAGE, bush_rest, bush_pressed, nut_pressed))
    knob = PRINTED["Knob"]
    pressed = knob.copy()
    pressed.translate(V(0, 0, -0.5))
    print("knob to face (pressed 0.5): %.2f mm; to cowl %.2f (rest) / %.2f (pressed); to halo ring %.2f (pressed)" % (
        pressed.distToShape(PRINTED["Shell"])[0], knob.distToShape(PRINTED["DisplayCowl"])[0],
        pressed.distToShape(PRINTED["DisplayCowl"])[0], pressed.distToShape(PRINTED["HaloDiffuser"])[0]))
    print("cowl to halo ring %.2f mm" % PRINTED["DisplayCowl"].distToShape(PRINTED["HaloDiffuser"])[0])
    pegs = cowl_pegs()
    peg_shell = min(q.distToShape(PRINTED["Shell"])[0] for q in pegs)
    peg_other = min(q.distToShape(DUMMIES[n])[0] for q in pegs for n in ("Dummy_Display", "Dummy_Wire", "Dummy_Encoder"))
    peg_car = min(q.distToShape(PRINTED["Carrier"])[0] for q in pegs)
    print("cowl pegs (%d, D%.1f x %.1f) at %s: to shell holes %.2f, to display/wires %.1f, to carrier %.1f mm" % (
        len(pegs), COWL_PEG_D, COWL_PEG_L, COWL_PEGS, peg_shell, peg_other, peg_car))
    print("cowl pegs poke %.2f mm behind the shell face" % (COWL_PEG_L - FACE_T))
    tip = 2 * RP_FLIP_Z - (C_FRONT - SM_HDR[3])
    print("RP2040: x %.2f (case centre %.2f), pins stand %.2f mm out of the carrier front; USB hole z %.2f (modelled receptacle %.2f)" % (
        SM_X, W / 2, tip - C_FRONT, usb_z + USB_HOLE_DZ, usb_z))
    margin = check_usb_hole()
    print("USB hole vs the connector (shifted %.1f mm to its real position): margin %.2f mm" % (USB_HOLE_DZ, margin))
    if margin < 0.2:
        raise ValueError("USB hole does not clear the real connector")
    print("wrap room: RP2040 pins %.1f mm above the carrier, encoder pins %.1f mm bare behind it, key pins %.1f mm; a wrap (%.1f mm) fits" % (
        tip - C_FRONT, C_BACK - (HW040_FRONT + 2.5), SW_PIN_L - C_T + SW_SINK, WRAP[2]))
    board = DUMMIES["Dummy_RP2040_Zero"]
    rx, ry = RESET_C
    plate = PRINTED["BackPlate"].cut(cyl(4.2, rx, ry, z_back(ry) - 1, RESET_SURFACE_Z))  # without the reset guide
    print("board to back plate %.2f mm (guide aside), to encoder %.2f mm" % (
        board.distToShape(plate)[0], board.distToShape(DUMMIES["Dummy_Encoder"])[0]))
    print("reset guide: %.1f mm tall, ends %.1f mm short of the button" % (RESET_SURFACE_Z - 0.6 - (z_back(ry) + 1.0), 0.6))
    lens = [sum(math.dist(a, b) for _, pts in segs for a, b in zip(pts, pts[1:])) for _, segs, _ in wire_routes()]
    print("wires: %d, %d vias, %d display pockets, total %.0f mm of groove, longest %.0f mm" % (
        len(lens), len(wire_vias()), len(disp_pockets()), sum(lens), max(lens)))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    build()
    doc = to_doc()
    clashes = check_interference()
    print("interference:", clashes if clashes else "none")
    tightest, tight = check_wires()
    print("net to net plastic: tightest %.2f mm (%s); rule %.1f mm: %s" % (
        tightest[0], tightest[1], WIRE_RIDGE_MIN, "ok" if not tight else "; ".join(tight)))
    shorts = check_wire_shorts()
    print("wire net overlaps:", shorts if shorts else "none")
    low = check_underside()
    print("underside problems:", ", ".join(low) if low else "none")
    print("screw counterbore D%.1f (clearance hole D%.1f); biggest bumper that stays on the flat, per screw: %s mm" % (
        SCREW_HEAD_D, SCREW_CLR, ", ".join("%.1f" % d for d in bumper_room())))
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
    if clashes or tight or shorts or low or ovh or any(not shape.isValid() or shape.Volume <= 0 for shape in PRINTED.values()):
        raise ValueError("design checks failed; inspect diagnostics above")
    if min(clearances.values()) < 0.5 - 1e-6:
        raise ValueError("RP2040-Zero clearance below 0.5 mm")
    for name in ("CapLeft", "CapMiddle", "CapRight"):
        pressed = PRINTED[name].copy()
        pressed.translate(V(0, 0, -SW_TRAVEL))
        others = [shape for n, shape in PRINTED.items() if n != name]
        others += [board, DUMMIES["Dummy_Display"]]
        if any(pressed.common(shape).Volume > 0.01 for shape in others):
            raise ValueError(name + " collides during %.1f mm travel" % SW_TRAVEL)
    pressed_knob = PRINTED["Knob"].fuse(PRINTED["KnobCap"])
    pressed_knob.translate(V(0, 0, -0.5))
    if any(pressed_knob.common(shape).Volume > 0.01 for shape in
           (PRINTED["Shell"], PRINTED["DisplayCowl"], PRINTED["HaloDiffuser"], DUMMIES["Dummy_Encoder"])):
        raise ValueError("knob collides during 0.5 mm travel")
    pl = check_pillar_lip()
    print("pillar/lip problems:", "; ".join(pl) if pl else "none")
    if pl:
        raise ValueError("pillar/lip checks failed")
    dp = check_display()
    print("display problems:", "; ".join(dp) if dp else "none")
    if dp:
        raise ValueError("display protection checks failed; inspect diagnostics above")
    if any(len(shape.Solids) != 1 for name, shape in PRINTED.items()):
        raise ValueError("printed part contains disconnected islands")
    if KNOB_GAP - 0.5 - HALO_T < 0.3-1e-6:
        raise ValueError("pressed knob to halo ring clearance below 0.3 mm")
    print("all keys and knob: %.1f mm key travel and 0.5 mm knob travel clear" % SW_TRAVEL)
    g_hi, g_lo, c_plan, c_3d = check_pillars()
    print("pillars: %d, gap to the carrier back %.2f - %.2f mm; clearance to grooves, vias, pin holes and the HW-040 window %.2f mm (rule %.1f), to wires, pins, encoder and RP2040 %.2f mm: ok" % (
        len(PILLARS), g_lo, g_hi, c_plan, WIRE_RIDGE_MIN, c_3d))
    top, thick, cut_w, reach = check_switch_plate()
    print("switch plate: top %.2f over the seat, %.2f thick, cutout %.2f; clips reach %.2f mm under it, hook touching the underside: ok" % (top, thick, cut_w, reach))
    slot, extra = check_cap_socket()
    print("MX socket: arms %.2f / %.2f mm, length %.2f mm; extra over the stem %.2f / %.2f / %.2f mm: ok" % (slot + extra))
    report_new_features()
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):  # freecadcmd imports the file as module "build_case"
    main()
