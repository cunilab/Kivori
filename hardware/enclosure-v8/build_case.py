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
KNOB_GAP = 4.3              # air gap between knob skirt and face (push travel is ~0.5; raised 2.5 mm from 1.8 after the first
                            # print; KNOB_H is held, so the shaft enters the knob 2.5 mm less deep)
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
DISP_STANDOFF_D = 4.5      # pillar diameter
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
LIP_W, LIP_H = 1.0, 0.6     # white lip: width and height above the cowl top (v7's bezel: 2.0 x 0.8)
LIP_CHAMFER = 0.6           # 45° bevel on the lip's window edge, over its whole height: the seated eye (59° over the glass) is steeper
                            # than the bevel, so what hides the screen is the bevel's foot, at the cowl top
LIP_COLLAR = (0.4, 0.5, 0.1)  # collar on the lip's underside that locates it in the cowl window: wall, height, play per side
EYE_ELEV = 40.0             # seated eye above the desk, from the front (the concept sheet's viewpoint)
RIM_HIDDEN_MAX = 0.5        # build check: the near rim may hide at most this much of the active area from that eye

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
                            # bends as the cap snaps on: 5.9 mm of finger (2.5% strain at the old d 0.35, now ~2.9%, 3 t d / 2 L^2 with
                            # d = lip reach + the 0.05 preload of CAP_SNAP_CLR = 0.40 now) and 4.6 mm of short
                            # plate (1.7% at the old d 0.2, now ~2.1%), at PETG's limit of 2-2.5%. (A 0.75 share on both gave 15%
                            # and 7%: the cap would not click on.)
CAP_SNAP = 0.35             # lip reach in under the head (the neck is 0.4 narrower per side: 0.05 gap left; 0.40 would touch it;
                            # was 0.30, raised to cut the cap wobble: test-fit on a real switch)
CAP_SNAP_CLR = -0.05        # finger to head side: they press on it slightly (at 0.1 the printed fingers stood off)
CAP_SNAP_DROP = 0.4         # lip catch this much lower than drawn (measured: at 0.2 the cap was still loose on the head)
CAP_SIDE = (2.0, 1.2, 0.0)  # short sides (2 wide, as on the bought cap): plate width, thickness, play to the head
CAP_SIDE_SNAP = 0.25        # (was 0.20) their lips reach this far in under the head (shorter, stiffer plates: less than
                            # the long ones); the ribs make a "+" with the fingers, as on the bought cap
CAP_STOP_D = 3.0            # stop post on the head, round its centre hole
CAP_STOP_ROOT = (3.6, 1.0)  # the post widens to this Ø over this height where it meets the cap top
SW_PADS = [(-6.25, -2.5), (6.25, -2.5), (-6.25, 2.5), (6.25, 2.5)]  # pin 1,1,2,2 (land pattern Ø1.5)

# Printed PCB ("carrier"): same plane as a future real PCB
C_FRONT = -10.2             # printed PCB front (the 7.2 mm switch top ends 3.0 below the face: the caps reach down)
KNOB_H = 11.0               # held from the 1.8 mm gap (it was HW040_FRONT + EC11_SHAFT_H + KNOB_TIP_CLR + KNOB_TOP_T - KNOB_GAP)
C_T = 2.0                   # printed thickness (a real board is 1.6, it grows backwards); leaves a 0.6 mm HW-040 pocket floor
C_BACK = C_FRONT - C_T
C_INSET = 0.8               # carrier outline inset from the inner walls
HOLE_D = 1.2                # clearance for other 2.54 mm header pins
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
HALO_OD, HALO_ID, HALO_T = 42.0, 12.4, 0.6  # white trim ring on the face (glued on)

KNOB_CAP_T, KNOB_CAP_D = 0.8, 31.0
KNOB_CAP_PEGS = [(KNOB_C[0] + 10 * math.cos(math.radians(a)),
                  KNOB_C[1] + 10 * math.sin(math.radians(a))) for a in (0, 120, 240)]
UNDER_CLR = 0.3


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
SW_PLUNGER_TOP = C_FRONT + SW_TOTAL_H

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
    for (x, y, a) in BTNS:
        cuts.append(rr_prism(BTN_OPEN + 2 * UNDER_CLR, BTN_OPEN + 2 * UNDER_CLR, 1.8, x, y, -FACE_T - 1, 1, a))
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
    """Slim white lip round the window, glued on the cowl top; its collar locates it in the cowl window. 1.0 wide, 0.6 high,
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
    cuts += [cyl(SW_HOLE_D, *btn_pin(i, k), z0, z1) for i in range(3) for k in range(4)]
    cuts += [cyl(VIA_D, x, y, z0, z1) for _, x, y in wire_vias()]
    return cuts


RP_PIN = {net: name for name, net in RP_NETS.items()}   # net -> RP2040 pin name
LEG_NET = {(0, 1): 'GND', (1, 0): 'GND', (1, 1): 'GND', (2, 0): 'GND', (2, 1): 'GND',
           (0, 3): 'B1', (1, 3): 'B2', (2, 3): 'B3'}      # switch leg (key, pad) -> net; the rest is n/c
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
            leg = btn_pin(i, 3)
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
    # key ground: east of everything, down to the right key's free lower leg
    leg = btn_pin(2, 1)
    xk = 75.0
    via_k = (xk, leg[1])
    R.append(('GND', [('F', [D['GND'], (X0 + 1.9, D['GND'][1] + 0.8), (xk, D['GND'][1] + 0.8), via_k]),
                      ('B', [via_k, leg])], (('disp', D['GND']), ('leg', leg))))
    for a, b in (((0, 1), (1, 0)), ((1, 1), (2, 0))):   # key to key ground chain
        pa, pb = btn_pin(*a), btn_pin(*b)
        R.append(('GND', [('B', [pa, pb])], (('leg', pa), ('leg', pb))))
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
    F += [(LEG_NET.get((i, k), 'n/c %d.%d' % (i, k)), "leg hole %d.%d" % (i, k), _circle(*btn_pin(i, k), SW_HOLE_D))
          for i in range(3) for k in range(4)]
    F += [('solid', "spigot hole", _circle(x, y, SPIGOT_HOLE_D)) for x, y in POSTS]
    F += [('solid', "encoder bushing hole", _circle(*KNOB_C, EC11_BUSH_D + 2 * EC11_HOLE_CLR))]
    if face == 'F':
        F += [('solid', "post", _circle(x, y, POST_D)) for x, y in POSTS]
        F += [('solid', "standoff", _circle(x, y, DISP_STANDOFF_D)) for x, y in disp_standoffs]
        F += [(net, "pocket %s" % n, _circle(x, y, DPIN_POCKET[0])) for (n, (x, y)), net in
              zip(disp_pins.items(), ('GND', '3V3', 'SCL', 'SDA', 'RES', 'DC', 'BLK'))]
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


def make_carrier():
    """The printed PCB. Prints back side down; every holding feature points up from the front."""
    c = carrier_outline()
    cuts = carrier_through_cuts()
    c = c.cut(cuts)

    c = c.fuse(display_pillars() + [wire_cradle()])
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
        pillar = cyl(DISP_STANDOFF_D, x, ys, C_FRONT - 0.01, DISP_Z + 8).cut(above_plane(W_PCB1)).cut(header)
        out += [pillar, tilt(peg(xh, yh, DISP_HOLE_D, W_PCB1 - 0.3, W_GLASS1 + PEG_PROUD))]
    return out


def wire_cradle():
    """Strain relief for the display wires: a low rib under the header whose top is parallel to the board, DISP_PIN_TIP_CLR under
    the pin tips (the pins stand free). Each wire has a funnel, a straight bore and, under it, its pocket in the carrier."""
    xs, ys = [p[0] for p in disp_pins.values()], [p[1] for p in disp_pins.values()]
    blk = box(xs[0] - CRADLE_W / 2, min(ys) - CRADLE_END, C_FRONT - 0.01, xs[0] + CRADLE_W / 2, max(ys) + CRADLE_END, DISP_Z + 8)
    return blk.cut(above_plane(W_CRADLE))


def cradle_cuts():
    """Bore and funnel for every display wire (the pocket under the carrier's front is part of the bore)."""
    cuts = []
    for x, y in disp_pins.values():
        zt = plane_z(y, W_CRADLE)
        cuts.append(cyl(DPIN_POCKET[0], x, y, C_FRONT - DPIN_POCKET[1], zt + 1))
        cuts.append(Part.makeCone(DPIN_POCKET[0] / 2, DPIN_POCKET[0] / 2 + 1.0, 1.0, V(x, y, zt - (CRADLE_FUNNEL_R - DPIN_POCKET[0] / 2)), V(0, 0, 1)))
    return cuts


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


def check_cap_grip():
    """Measure the printed grip (not the constants): each lip's reach under the stem head must equal
    CAP_SNAP / CAP_SIDE_SNAP and stay >= 0.05 mm clear of the stem neck. Returns (long, short) reach and
    (long, short) neck clearance, raising if a rule fails."""
    z_ceil = BTN_CAP_PROUD - CAP_TOP_T
    grip = cap_grip(z_ceil, 6.0)
    z_k = SW_PLUNGER_TOP - SW_HEAD_H - 0.05 - CAP_SNAP_DROP
    z_lo, z_hi = z_k - 0.09, z_k - 0.01  # inside the lip's 0.1 land
    long_lip = grip.common(box(0.5, -0.5, z_lo, 5, 0.5, z_hi))
    short_lip = grip.common(box(-0.5, 0.5, z_lo, 0.5, 5, z_hi))
    reach = (SW_HEAD / 2 - long_lip.BoundBox.XMin, SW_HEAD / 2 - short_lip.BoundBox.YMin)
    neck = box(-SW_NECK / 2, -SW_NECK / 2, SW_PLUNGER_TOP - SW_HEAD_H - SW_NECK_H, SW_NECK / 2, SW_NECK / 2,
               SW_PLUNGER_TOP - SW_HEAD_H)
    clear = (long_lip.distToShape(neck)[0], short_lip.distToShape(neck)[0])
    if abs(reach[0] - CAP_SNAP) > 1e-4 or abs(reach[1] - CAP_SIDE_SNAP) > 1e-4:
        raise ValueError("cap lip reach %s differs from CAP_SNAP/CAP_SIDE_SNAP" % (reach,))
    if min(clear) < 0.05 - 1e-6:
        raise ValueError("cap lip to stem neck clearance %s below 0.05 mm" % (clear,))
    return reach, clear


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
    ring = lambda x, y, z0: cyl(WRAP[0], x, y, z0, z0 + WRAP[2]).cut(cyl(WRAP[1], x, y, z0 - 1, z0 + WRAP[2] + 1))
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
                    parts.append(ring(pin[0], pin[1], z0))
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
    for i, n in enumerate(["Dummy_SwitchLeft", "Dummy_SwitchMiddle", "Dummy_SwitchRight"]):
        DUMMIES[n] = dummy_switch(i)


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
    gnd = {(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)}    # lower legs: the key ground chain
    net = lambda i, k: "GND" if (i, k) in gnd else "B%d" % (i + 1) if k == 3 else "n/c"
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
    # switch legs: <switch L/M/R>-<leg: LL lower left, LR, UL, UR>, below the lower legs, above the upper
    legs = ["LL", "LR", "UL", "UR"]
    for i, sw in enumerate(["L", "M", "R"]):
        for k in range(4):
            p = btn_pin(i, k)
            out.append(("%s-%s %s" % (sw, legs[k], net(i, k)), p[0], p[1] + (-1.9 if k < 2 else 1.9), 0))
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


def report_new_features():
    """Numbers for the second-print changes: knob, cowl pegs, RP2040 drop."""
    shaft_top = HW040_FRONT + EC11_SHAFT_H
    hub_z0 = max(HW040_FRONT + EC11_BODY_H + EC11_BUSH_L + 0.5, KNOB_GAP)
    print("knob: z %.1f..%.1f (H %.1f), shaft enters %.2f mm (D flat %.1f mm long, all inside), tip clearance %.1f" % (
        KNOB_GAP, KNOB_GAP + KNOB_H, KNOB_H, shaft_top - hub_z0, min(EC11_FLAT_L, shaft_top - hub_z0), KNOB_TIP_CLR))
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
    print("wrap room: RP2040 pins %.1f mm above the carrier, encoder pins %.1f mm bare behind it, key legs %.1f mm; a wrap (%.1f mm) fits" % (
        tip - C_FRONT, C_BACK - (HW040_FRONT + 2.5), C_BACK - (C_FRONT - SW_LEG[2]), WRAP[2]))
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
        pressed.translate(V(0, 0, -0.5))
        others = [shape for n, shape in PRINTED.items() if n != name]
        others += [board, DUMMIES["Dummy_Display"]]
        if any(pressed.common(shape).Volume > 0.01 for shape in others):
            raise ValueError(name + " collides during 0.5 mm travel")
    pressed_knob = PRINTED["Knob"].fuse(PRINTED["KnobCap"])
    pressed_knob.translate(V(0, 0, -0.5))
    if any(pressed_knob.common(shape).Volume > 0.01 for shape in
           (PRINTED["Shell"], PRINTED["DisplayCowl"], PRINTED["HaloDiffuser"], DUMMIES["Dummy_Encoder"])):
        raise ValueError("knob collides during 0.5 mm travel")
    dp = check_display()
    print("display problems:", "; ".join(dp) if dp else "none")
    if dp:
        raise ValueError("display protection checks failed; inspect diagnostics above")
    if any(len(shape.Solids) != 1 for name, shape in PRINTED.items()):
        raise ValueError("printed part contains disconnected islands")
    if KNOB_GAP - 0.5 - HALO_T < 0.3-1e-6:
        raise ValueError("pressed knob to halo ring clearance below 0.3 mm")
    print("all keys and knob: 0.5 mm travel clear")
    reach, clear = check_cap_grip()
    print("cap grip: lip under the head long %.2f / short %.2f mm; clearance to stem neck long %.2f / short %.2f mm (rule >= 0.05): ok" % (
        reach + clear))
    report_new_features()
    if EXPORT:
        export(doc, here)
        print("exported to", os.path.join(here, "export"))
    return doc


if __name__ in ("__main__", "build_case"):  # freecadcmd imports the file as module "build_case"
    main()
