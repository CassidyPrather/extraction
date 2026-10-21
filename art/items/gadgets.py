"""Gadgets: flashlight, power cell, compass, field radio, binoculars, EMP
grenade, EMP trap, map case, camera drone, lead-lined case, cloak
generator, deployable cover, climbing and zipline kits, matches, door
wedge. Writes assets/models/items/<id>.glb for each (item space: see
_kit.py).

Built round what src/gadgets/ hangs on them:

- flashlight: the beam leaves from z -0.06; a power cell seats standing up
  through the bay at (0, 0, 0.02).
- compass: the game draws the face (y 0.016) and the needle (y 0.019) on
  top of the case, so the case's dial seat is at y 0.015.
- map_case: the game draws the map sheet (0.28 m square) at y 0.006,
  between the backing board and the clear sleeve.
- binoculars: the eye goes at (0, 0, 0.1).
- cloak_generator: like the flashlight, a power cell seats standing up at
  (0, 0, 0.02), here in the open bay between the base and the emitter.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import (ALONG_X, UP, cell_bay, centred, cube, item, panel, ring, transform,
                  tube_path, wanted)
from common import Part, RX, RY, RZ, T, S, fillet, section_rect


# --- Flashlight ------------------------------------------------------------------------------

def flashlight():
    p = Part("body")
    # Head: a flared bell with a glass lens and a bezel ring.
    p.lathe([(0.0, -0.09), (0.0168, -0.09), (0.0175, -0.087), (0.0175, -0.068), (0.0135, -0.055),
             (0.0135, 0.0095), (0.0, 0.0095)], "Paint", segs=24)
    p.tube(0.0178, 0.0145, -0.0905, -0.086, "Steel", segs=24)
    p.cyl(0.0148, -0.0895, -0.088, "Lens", segs=20)
    # A slide switch on top, a ring of grip ridges.
    p.box((0.008, 0.004, 0.018), (0.0, 0.0145, -0.035), "Black", bevel=0.0015, segs=1)
    for k in range(3):
        p.tube(0.0142, 0.0125, -0.02 + k * 0.008, -0.016 + k * 0.008, "Paint", segs=20)
    # Tail and a lanyard loop.
    p.lathe([(0.0, 0.0305), (0.0135, 0.0305), (0.0135, 0.084), (0.012, 0.09), (0.0, 0.09)],
            "Paint", segs=24)
    p.lathe([(0.004, -0.0012), (0.0055, 0.0), (0.004, 0.0012), (0.0025, 0.0)], "Steel", segs=10,
            xf=T(0, 0, 0.094) @ ALONG_X @ S(1, 1.6, 1), closed=True)
    cell_bay(p, 0.0135, "Paint")
    return [p]


# --- Power cell ------------------------------------------------------------------------------

def power_cell():
    p = Part("body")
    p.lathe([(0.0, -0.027), (0.0085, -0.027), (0.0095, -0.026), (0.0095, 0.026), (0.0085, 0.027),
             (0.0, 0.027)], "Olive", segs=16, xf=UP)
    # Contacts at both ends, a charge window, a printed band.
    p.cyl(0.004, 0.027, 0.03, "Steel", segs=10, xf=UP, chamfer=0.001)
    p.cyl(0.006, -0.0285, -0.027, "Steel", segs=10, xf=UP)
    p.lathe([(0.0096, -0.01), (0.0099, -0.009), (0.0099, 0.012), (0.0096, 0.013)], "Glow",
            segs=6, xf=UP, start=-25, sweep=50, closed=True)
    p.tube(0.0097, 0.009, 0.018, 0.024, "Cream", segs=16, xf=UP)
    return [p]


# --- Compass --------------------------------------------------------------------------------

def compass():
    p = Part("body")
    # Case: a squat drum, the dial seat at y 0.015 (the game draws the face).
    p.lathe([(0.0, -0.006), (0.021, -0.006), (0.0235, -0.004), (0.024, 0.0), (0.024, 0.015),
             (0.0, 0.015)], "Blued", segs=28, xf=UP)
    # Rotating bezel with grip notches, and the glass over the needle.
    p.lathe([(0.0242, 0.012), (0.0272, 0.0125), (0.0275, 0.0205), (0.0262, 0.0215),
             (0.0242, 0.0215)], "Black", segs=28, xf=UP, closed=True)
    for k in range(16):
        p.box((0.0012, 0.006, 0.0015), (0.0, 0.017, 0.0), "Blued",
              xf=RY(2 * math.pi * k / 16) @ T(0.0278, 0, 0))
    p.cyl(0.0244, 0.0205, 0.0212, "Clear", segs=24, xf=UP)
    p.box((0.002, 0.0015, 0.004), (0.0, 0.0215, -0.0255), "Glow", bevel=0.0004, segs=1)
    # Strap lugs and a canvas wrist strap under it.
    for sx in (-1, 1):
        p.box((0.006, 0.006, 0.016), (sx * 0.025, 0.0, 0.0), "Blued", bevel=0.0015, segs=1)
        p.cyl(0.0012, -0.008, 0.008, "Steel", segs=6, xf=T(sx * 0.0265, 0.0, 0.0))
    strap = [(-0.032, 0.0), (-0.027, -0.009), (-0.018, -0.012), (0.018, -0.012), (0.027, -0.009),
             (0.032, 0.0)]
    p.sweep([(x, y) for x, y in strap], section_rect(0.014, 0.0025), "Olive",
            xf=RY(math.pi / 2))
    return [p]


# --- Field radio ----------------------------------------------------------------------------

def radio():
    p = Part("body")
    W, D = 0.066, 0.038
    y0, y1 = -0.085, 0.045
    body = fillet([(-W / 2, y0), (W / 2, y0), (W / 2, y1), (-W / 2, y1)], 0.006, 3)
    p.prism(body, -D / 2, D / 2, "Paint", axis="z", bevel=0.004, segs=2)
    # Speaker grille and a little tuning window on the front (+Z), the
    # window set flush into a recess cut for it.
    for k in range(6):
        p.box((0.04, 0.003, 0.002), (0.0, -0.06 + k * 0.007, D / 2 + 0.0005), "Black")
    recess = Part("recess")
    recess.box((0.046, 0.022, 0.009), (0.0, 0.02, D / 2), "Black")
    p.cut(recess)
    window = Part("window")
    panel(window, "z", -0.023, 0.023, 0.009, 0.031, D / 2 - 0.0042, D / 2 + 0.0004, "Black",
          [(-0.02, 0.02, 0.012, 0.028, "Cream")])
    window.box((0.0012, 0.014, 0.0008), (0.006, 0.02, D / 2 + 0.0008), "LampRed")
    # Knobs on top (the tuning one, `dial`, turns on its own node), the
    # rubber antenna, push-to-talk on the side.
    dial = Part("dial", origin=(-0.018, y1, 0.0))
    for q, x, r in ((dial, -0.018, 0.007), (p, 0.0, 0.0055)):
        q.cyl(r, y1, y1 + 0.012, "Black", segs=14, xf=T(x, 0, 0) @ UP, chamfer=0.0015)
        q.box((0.0015, 0.002, r * 1.6), (x, y1 + 0.0125, 0.0), "Cream")
    p.cyl(0.007, y1, y1 + 0.012, "Black", segs=12, xf=T(0.019, 0, 0) @ UP)
    p.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0045, 0.06), (0.0035, 0.07), (0.0, 0.072)], "Rubber",
            segs=10, xf=T(0.019, y1 + 0.012, 0) @ UP)
    p.box((0.004, 0.026, 0.016), (W / 2 + 0.001, 0.0, 0.0), "Black", bevel=0.0015, segs=1)
    # Belt clip on the back.
    p.box((0.02, 0.06, 0.003), (0.0, 0.0, -D / 2 - 0.0035), "Steel", bevel=0.001, segs=1)
    p.box((0.02, 0.006, 0.005), (0.0, 0.028, -D / 2 - 0.002), "Steel")
    # Its box takes in the aerial (`dims` in items.ron). Centring moves the
    # geometry only, so the dial's origin follows it by hand.
    p.bm.verts.ensure_lookup_table()
    before = p.bm.verts[0].co.copy()
    parts = centred([p, window, dial])
    dial.origin += p.bm.verts[0].co - before
    return parts


# --- Binoculars ---------------------------------------------------------------------------

def binoculars():
    p = Part("body")
    for sx in (-1, 1):
        x = sx * 0.052
        # Objective tube forward, prism housing, eyepiece behind (offset in).
        p.lathe([(0.0, -0.064), (0.021, -0.064), (0.0225, -0.06), (0.0225, -0.02), (0.0, -0.02)],
                "Black", segs=24, xf=T(x, 0.0, 0.0))
        p.cyl(0.0185, -0.0645, -0.062, "Lens", segs=20, xf=T(x, 0.0, 0.0))
        house = fillet([(-0.024, -0.026), (0.024, -0.026), (0.024, 0.026), (-0.024, 0.026)],
                       0.009, 3)
        p.prism([(x + a * 1.05 - sx * 0.006, b) for a, b in house], -0.025, 0.03, "Black",
                axis="z", bevel=0.004, segs=2)
        xe = sx * 0.034
        p.lathe([(0.0, 0.03), (0.0135, 0.03), (0.0135, 0.05), (0.0, 0.05)], "Black", segs=20,
                xf=T(xe, 0.0, 0.0))
        p.lathe([(0.0115, 0.05), (0.0155, 0.05), (0.0165, 0.065), (0.0145, 0.065),
                 (0.0115, 0.056)], "Rubber", segs=20, xf=T(xe, 0.0, 0.0), closed=True)
        p.cyl(0.01, 0.054, 0.056, "Lens", segs=16, xf=T(xe, 0.0, 0.0))
        # Hinge knuckles to the bridge.
        p.box((0.03, 0.012, 0.04), (sx * 0.02, 0.0, 0.005), "Black", bevel=0.003, segs=1)
    # Central hinge and focus wheel.
    p.cyl(0.009, -0.03, 0.045, "Black", segs=16)
    p.cyl(0.011, 0.032, 0.044, "Rubber", segs=16)
    p.cyl(0.004, -0.032, -0.03, "Steel", segs=10)
    # Neck strap lugs.
    for sx in (-1, 1):
        p.box((0.004, 0.01, 0.01), (sx * 0.078, 0.015, 0.0), "Steel", bevel=0.001, segs=1)
    return [p]


# --- EMP grenade ----------------------------------------------------------------------------

def emp_grenade():
    p = Part("body")
    # Body: a ribbed can with coil windows round its waist.
    p.lathe([(0.0, -0.055), (0.025, -0.055), (0.03, -0.05), (0.031, -0.04), (0.031, 0.02),
             (0.028, 0.028), (0.016, 0.032), (0.0, 0.032)], "Paint", segs=24, xf=UP)
    for y in (-0.042, 0.012):
        p.tube(0.0322, 0.029, y - 0.003, y + 0.003, "Black", segs=24, xf=UP)
    p.tube(0.0305, 0.026, -0.03, 0.0, "Copper", segs=24, xf=UP)
    for k in range(6):
        a = 2 * math.pi * k / 6
        p.box((0.008, 0.032, 0.005), (0.0, -0.015, 0.0), "Paint", bevel=0.001, segs=1,
              xf=RY(a) @ T(0.0305, 0, 0))
    p.lathe([(0.0312, -0.025), (0.0318, -0.024), (0.0318, -0.006), (0.0312, -0.005)], "Glow",
            segs=24, xf=UP, closed=True, start=15, sweep=30)
    p.lathe([(0.0312, -0.025), (0.0318, -0.024), (0.0318, -0.006), (0.0312, -0.005)], "Glow",
            segs=24, xf=UP, closed=True, start=195, sweep=30)
    # Fuse head: a threaded neck, the spoon down the side, the pull ring.
    p.cyl(0.011, 0.031, 0.043, "Steel", segs=16, xf=UP)
    p.box((0.016, 0.01, 0.022), (0.0, 0.048, 0.0), "Steel", bevel=0.002, segs=1)
    spoon = [(0.0, 0.052), (0.02, 0.052), (0.031, 0.046), (0.034, 0.03), (0.034, -0.02)]
    p.sweep([(z, y) for z, y in spoon], section_rect(0.012, 0.0015), "Olive",
            xf=RY(-math.pi / 2))
    p.lathe([(0.009, -0.0012), (0.0105, 0.0), (0.009, 0.0012), (0.0075, 0.0)], "Steel", segs=14,
            xf=T(0.02, 0.05, 0.0) @ ALONG_X, closed=True)
    p.cyl(0.0012, 0.008, 0.012, "Steel", segs=6, xf=T(0, 0.05, 0.0) @ ALONG_X)
    return [p]


# --- EMP trap --------------------------------------------------------------------------------

def emp_trap():
    s = cube(0.5)
    yb = -s / 2
    p = Part("body")
    # A low domed puck on a rubber foot, coil ring under a glowing seam.
    p.cyl(0.036, yb, yb + 0.004, "Rubber", segs=28, xf=UP)
    p.lathe([(0.0, yb + 0.004), (0.038, yb + 0.004), (0.039, yb + 0.012), (0.037, yb + 0.022),
             (0.03, yb + 0.03), (0.016, yb + 0.034), (0.0, yb + 0.035)], "Paint", segs=28, xf=UP)
    p.tube(0.0395, 0.035, yb + 0.012, yb + 0.018, "Copper", segs=28, xf=UP)
    p.tube(0.0398, 0.035, yb + 0.0185, yb + 0.0205, "Glow", segs=28, xf=UP)
    # Three prongs and a pressure plate on top.
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.3
        p.cyl(0.0022, yb + 0.02, yb + 0.05, "Copper", segs=8,
              xf=RY(a) @ T(0.026, 0, 0) @ UP @ RX(0.0))
        p.sphere(0.004, (0.0, 0.0, 0.0), "Copper", segs=8, rings=4,
                 xf=RY(a) @ T(0.026, yb + 0.051, 0))
    p.cyl(0.012, yb + 0.034, yb + 0.038, "Black", segs=18, xf=UP, chamfer=0.001)
    p.box((0.005, 0.002, 0.012), (0.0, yb + 0.0385, 0.0), "LampRed")
    return [p]


# --- Map case ----------------------------------------------------------------------------------

def map_case():
    p = Part("body")
    # Backing board under the map (the sheet is at y 0.006, 0.28 m square).
    board = fillet([(-0.15, -0.15), (0.15, -0.15), (0.15, 0.15), (-0.15, 0.15)], 0.012, 3)
    p.prism(board, -0.004, 0.0045, "Olive", axis="y", bevel=0.0015, segs=1)
    # Clear sleeve over it inside a stitched border round the window. The
    # sleeve stands 3 mm off the board so the two never fight.
    p.box((0.276, 0.0004, 0.276), (0.0, 0.0077, 0.0), "Clear")
    for sx in (-1, 1):
        p.box((0.012, 0.0038, 0.3), (sx * 0.144, 0.0064, 0.0), "Olive", bevel=0.001, segs=1)
        p.box((0.276, 0.0038, 0.012), (0.0, 0.0064, sx * 0.144), "Olive", bevel=0.001, segs=1)
    # A press stud flap on one edge, and a pencil in a loop.
    p.box((0.06, 0.004, 0.022), (0.0, 0.006, 0.161), "Olive", bevel=0.0015, segs=1)
    p.cyl(0.004, 0.006, 0.0095, "Steel", segs=10, xf=T(0, 0, 0.162) @ UP)
    p.cyl(0.0035, -0.12, 0.07, "Yellow", segs=6, xf=T(0.155, 0.006, 0.0))
    p.lathe([(0.0, 0.07), (0.0035, 0.07), (0.0, 0.085)], "Planks", segs=6,
            xf=T(0.155, 0.006, 0.0))
    p.box((0.01, 0.009, 0.012), (0.155, 0.006, -0.08), "Olive")
    return [p]


# --- Camera drone -----------------------------------------------------------------------------

def camera_drone():
    p = Part("body")
    # A flat body, the camera ball under its nose (-Z), four arms out to
    # guarded rotors. 0.2 x 0.06 x 0.2 box, origin at its centre.
    p.box((0.06, 0.022, 0.09), (0.0, 0.0, 0.0), "Paint", bevel=0.006, segs=2)
    p.box((0.05, 0.008, 0.06), (0.0, 0.014, 0.01), "Black", bevel=0.003, segs=1)
    p.sphere(0.012, (0.0, -0.014, -0.038), "Black", segs=14, rings=8)
    p.cyl(0.006, 0.0, 0.004, "Lens", segs=12, xf=T(0.0, -0.016, -0.049))
    p.box((0.004, 0.004, 0.004), (0.0, 0.013, 0.042), "LampRed")
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        x, z = math.cos(a) * 0.07, math.sin(a) * 0.07
        # Arm.
        p.box((0.012, 0.008, 0.07), (0.0, 0.0, 0.0), "Black", bevel=0.002, segs=1,
              xf=T(x / 2, 0.0, z / 2) @ RY(-a + math.pi / 2))
        # Motor, rotor and guard.
        p.cyl(0.008, -0.006, 0.012, "Steel", segs=12, xf=T(x, 0, z) @ UP)
        p.box((0.075, 0.0015, 0.008), (0.0, 0.0, 0.0), "Black", xf=T(x, 0.014, z) @ RY(a))
        p.tube(0.043, 0.04, 0.004, 0.016, "Paint", segs=28, xf=T(x, 0, z) @ UP)
    # Skids.
    for sx in (-1, 1):
        p.box((0.004, 0.018, 0.004), (sx * 0.022, -0.018, 0.0), "Black")
        p.box((0.004, 0.004, 0.07), (sx * 0.022, -0.028, 0.0), "Black")
    return [p]


# --- Lead-lined case --------------------------------------------------------------------------

def lead_case():
    # 0.11 x 0.07 x 0.11: a squat dull box, a lid with a seal line and two
    # latches. The artifact cradle is at its centre.
    p = Part("body")
    p.box((0.11, 0.045, 0.11), (0.0, -0.0125, 0.0), "Iron", bevel=0.006, segs=2)
    p.box((0.112, 0.022, 0.112), (0.0, 0.024, 0.0), "Iron", bevel=0.006, segs=2)
    p.box((0.113, 0.003, 0.113), (0.0, 0.0115, 0.0), "Black")
    for sx in (-1, 1):
        p.box((0.018, 0.026, 0.008), (sx * 0.03, 0.008, 0.057), "Steel", bevel=0.002, segs=1)
    recess = Part("recess")
    recess.box((0.05, 0.009, 0.03), (0.0, 0.035, 0.0), "Yellow")
    p.cut(recess)
    # The warning plate: a black trefoil let into the yellow, the two a
    # mosaic of full-thickness pieces (printed on, they'd z-fight).
    def sector(r0, r1, a0, a1, n=8):
        outer = [(r1 * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
                  r1 * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]
        inner = [(r0 * math.cos(math.radians(a1 - (a1 - a0) * i / n)),
                  r0 * math.sin(math.radians(a1 - (a1 - a0) * i / n))) for i in range(n + 1)]
        return outer + inner

    trefoil = [[(0.003 * math.cos(2 * math.pi * i / 14), 0.003 * math.sin(2 * math.pi * i / 14))
                for i in range(14)]]
    trefoil += [sector(0.0045, 0.0115, a - 30, a + 30) for a in (90, 210, 330)]
    y0, y1 = 0.0311, 0.0355
    plate = Part("plate")
    plate.box((0.05, y1 - y0, 0.03), (0.0, (y0 + y1) / 2, 0.0), "Yellow")
    holes = Part("cut")
    for poly in trefoil:
        holes.prism(poly, y0 - 0.001, y1 + 0.001, "Yellow", axis="y")
    plate.cut(holes)
    for poly in trefoil:
        plate.prism(poly, y0, y1, "Black", axis="y")
    # A folding bail handle on top, and the hinges along the back.
    for sx in (-1, 1):
        p.box((0.012, 0.008, 0.016), (sx * 0.04, 0.039, -0.035), "Steel", bevel=0.002, segs=1)
    p.sweep([(-0.04, 0.041), (-0.04, 0.047), (-0.03, 0.05), (0.03, 0.05), (0.04, 0.047),
             (0.04, 0.041)], [(0.0025 * math.cos(a), 0.0025 * math.sin(a)) for a in
                              [2 * math.pi * i / 6 for i in range(6)]], "Steel",
            xf=T(0, 0, -0.035) @ RY(math.pi / 2))
    for sx in (-1, 1):
        p.cyl(0.004, sx * 0.04 - 0.012, sx * 0.04 + 0.012, "Steel", segs=10,
              xf=T(0, 0.0115, -0.057) @ RY(math.pi / 2))
    return [p, plate]


# --- Cloak generator ---------------------------------------------------------------------------

def cloak_generator():
    yb = -0.15
    p = Part("body")
    # A squat olive field set: a heavy base with its control panel, an open
    # bay in the middle where a power cell stands between two contacts at
    # (0, 0, 0.02), and the emitter on top: stacked copper coils round a
    # mast with a glowing tip.
    plan = fillet([(-0.115, -0.115), (0.115, -0.115), (0.115, 0.115), (-0.115, 0.115)], 0.025, 3)
    for x in (-0.09, 0.09):
        for z in (-0.09, 0.09):
            p.cyl(0.012, yb, yb + 0.006, "Rubber", segs=12, xf=T(x, 0, z) @ UP)
    p.prism(plan, yb + 0.006, -0.044, "Paint", axis="y", bevel=0.008, segs=2)
    # Control panel standing off the front, its dials part of its face.
    panel(p, "z", -0.07, 0.07, -0.13, -0.07, -0.122, -0.113, "Black",
          [(-0.06, -0.01, -0.115, -0.106, "Glow"), (0.018, 0.058, -0.124, -0.078, "Cream"),
           (0.034, 0.042, -0.122, -0.092, "Black")])
    p.cyl(0.004, -0.124, -0.1215, "Steel", segs=8, xf=T(-0.04, -0.088, 0.0))
    p.box((0.003, 0.012, 0.003), (0.0, 0.0, 0.0), "Steel", xf=T(-0.04, -0.082, -0.1228) @ RX(-0.2))
    p.sphere(0.0045, (-0.015, -0.088, -0.1205), "LampRed", segs=10, rings=5)
    for sx in (-1, 1):
        tube_path(p, [(sx * 0.11, -0.075, -0.045), (sx * 0.1205, -0.075, -0.035),
                      (sx * 0.1205, -0.075, 0.035), (sx * 0.11, -0.075, 0.045)], 0.004, "Steel",
                  sides=6, closed=False)
    # The open bay: four posts, the cell's seat and the contact over it.
    for x in (-0.096, 0.096):
        for z in (-0.096, 0.096):
            p.box((0.018, 0.088, 0.018), (x, 0.0, z), "Paint", bevel=0.003, segs=1)
    p.box((0.034, 0.0115, 0.034), (0.0, -0.03825, 0.02), "Dark", bevel=0.002, segs=1)
    p.cyl(0.0075, -0.0325, -0.0302, "Steel", segs=12, xf=T(0, 0, 0.02) @ UP)
    p.box((0.034, 0.0115, 0.034), (0.0, 0.03825, 0.02), "Dark", bevel=0.002, segs=1)
    p.cyl(0.0045, 0.0302, 0.0325, "Steel", segs=12, xf=T(0, 0, 0.02) @ UP)
    for k in range(4):
        y = 0.0302 + 0.0006 * k
        ring(p, 0.004, 0.0005, "Steel", xf=T(0, y, 0.02) @ UP, segs=8, sides=3)
    for mat, pts in (("Red", [(0.017, -0.036, 0.03), (0.06, -0.041, 0.06), (0.087, -0.03, 0.087)]),
                     ("Black", [(-0.017, -0.036, 0.03), (-0.06, -0.041, 0.06),
                                (-0.087, -0.03, 0.087)]),
                     ("Copper", [(0.017, 0.036, 0.025), (0.06, 0.04, 0.06), (0.087, 0.03, 0.087)])):
        tube_path(p, pts, 0.002, mat, sides=5, closed=False)
    # Upper housing with a glowing band round it.
    band = fillet([(-0.111, -0.111), (0.111, -0.111), (0.111, 0.111), (-0.111, 0.111)], 0.022, 3)
    p.prism(plan, 0.044, 0.058, "Paint", axis="y", bevel=0.005, segs=2)
    p.prism(band, 0.058, 0.064, "Glow", axis="y")
    p.prism(plan, 0.064, 0.08, "Paint", axis="y", bevel=0.005, segs=2)
    # Emitter.
    p.lathe([(0.0, 0.08), (0.082, 0.08), (0.085, 0.083), (0.083, 0.088), (0.0, 0.088)], "Dark",
            segs=28, xf=UP)
    for R, y in ((0.074, 0.096), (0.059, 0.109), (0.044, 0.121)):
        ring(p, R, 0.0042, "Copper", xf=T(0, y, 0) @ UP, segs=24, sides=5)
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.5
        p.box((0.006, 0.042, 0.006), (0.0, 0.0, 0.0), "Dark",
              xf=RY(a) @ T(0.059, 0.105, 0.0) @ RZ(0.4))
    p.cyl(0.007, 0.088, 0.136, "Steel", segs=12, xf=UP)
    p.sphere(0.012, (0.0, 0.137, 0.0), "Glow", segs=14, rings=7)
    return [p]


# --- Deployable cover --------------------------------------------------------------------------

def deployable_cover():
    yb = -0.06
    p = Part("body")
    # A ballistic panel folded in two on a piano hinge, a vision slit in
    # its hood, the legs strapped flat on top and a carry handle.
    for k in range(2):
        p.box((0.58, 0.022, 0.48), (0.0, yb + 0.011 + k * 0.0225, -0.004), "Paint", bevel=0.004,
              segs=2)
    for k in range(7):
        x0 = -0.28 + k * 0.08
        p.cyl(0.0115, x0 + 0.002, x0 + 0.078, "Steel" if k % 2 else "Iron", segs=12,
              xf=T(0, yb + 0.0225, 0.238) @ ALONG_X @ T(0, 0, -0.0))
    p.cyl(0.004, -0.29, 0.29, "Steel", segs=8, xf=T(0, yb + 0.0225, 0.238) @ ALONG_X)
    # Vision slit: a hood standing off the top leaf, dark in its mouth.
    top = yb + 0.0445
    p.box((0.16, 0.012, 0.05), (-0.07, top + 0.006, -0.17), "Paint", bevel=0.003, segs=1)
    p.box((0.13, 0.006, 0.006), (-0.07, top + 0.0055, -0.1955), "Black")
    # Straps across, the folded legs on them, buckles.
    for x in (-0.2, 0.2):
        p.box((0.03, 0.004, 0.484), (x, top + 0.002, -0.004), "Olive")
        p.box((0.034, 0.006, 0.02), (x, top + 0.007, -0.15), "Steel", bevel=0.001, segs=1)
    for z in (-0.12, 0.13):
        p.cyl(0.0085, -0.26, 0.26, "Iron", segs=12, xf=T(0, top + 0.0125, z) @ ALONG_X,
              chamfer=0.002)
        for sx in (-1, 1):
            p.box((0.012, 0.004, 0.04), (sx * 0.262, top + 0.006, z), "Iron", bevel=0.001, segs=1)
    # Carry handle.
    for sx in (-1, 1):
        p.box((0.012, 0.03, 0.02), (sx * 0.06, top + 0.019, 0.005), "Steel", bevel=0.002, segs=1)
    p.cyl(0.009, -0.07, 0.07, "Rubber", segs=12, xf=T(0, top + 0.034, 0.005) @ ALONG_X)
    # Rubber bumpers on the front corners.
    for sx in (-1, 1):
        p.box((0.04, 0.046, 0.01), (sx * 0.27, yb + 0.023, -0.2455), "Rubber", bevel=0.003, segs=1)
    return [p]


# --- Climbing kit -------------------------------------------------------------------------------

def climbing_kit():
    s = cube(4.0)
    yb = -s / 2
    p = Part("body")
    # A coil of rope bound in three places, a webbing harness on it, and
    # the four-tined grapnel lying across the top tied to the rope's end.
    for k, (x, z) in enumerate(((0.0, 0.0), (0.003, -0.002), (-0.002, 0.003))):
        ring(p, 0.052, 0.0082, "Rope", xf=T(x, yb + 0.0082 + k * 0.0145, z) @ UP, segs=24,
             sides=7)
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.4
        p.lathe([(0.0125, -0.003), (0.0145, -0.003), (0.0145, 0.003), (0.0125, 0.003)], "Rope",
                segs=12, closed=True, xf=RY(a) @ T(0.052, yb + 0.0227, 0.0) @ S(1.0, 1.55, 1.0))
    # Harness: a waist belt and a leg loop of olive webbing, a buckle.
    p.lathe([(0.0395, -0.008), (0.0415, -0.008), (0.0415, 0.008), (0.0395, 0.008)], "Olive",
            segs=24, closed=True,
            xf=T(-0.012, yb + 0.055, 0.006) @ RX(0.12) @ S(1.15, 1.0, 0.75) @ UP)
    p.lathe([(0.0215, -0.007), (0.0235, -0.007), (0.0235, 0.007), (0.0215, 0.007)], "Olive",
            segs=18, closed=True, xf=T(0.034, yb + 0.054, -0.03) @ RZ(0.2) @ UP)
    p.box((0.012, 0.018, 0.006), (0.0, 0.0, 0.0), "Steel", bevel=0.001, segs=1,
          xf=T(-0.012, yb + 0.055, -0.0365) @ RX(0.12))
    ring(p, 0.009, 0.0016, "Steel", xf=T(0.008, yb + 0.066, -0.04) @ S(1.0, 1.6, 1.0) @ RY(0.3),
         segs=14, sides=5)
    # Grapnel: shank, eye, four tines.
    gy, gz = yb + 0.071, 0.012
    p.cyl(0.0055, -0.048, 0.036, "Iron", segs=10, xf=T(0, gy, gz) @ ALONG_X)
    ring(p, 0.0085, 0.0025, "Iron", xf=T(-0.057, gy, gz) @ ALONG_X @ RY(math.pi / 2), segs=12,
         sides=5)
    for k in range(4):
        ph = math.pi / 4 + k * math.pi / 2
        c, sn = math.cos(ph), math.sin(ph)
        pts = [(x, gy + r * c, gz + r * sn) for x, r in
               ((0.034, 0.0), (0.043, 0.008), (0.047, 0.019), (0.043, 0.028), (0.033, 0.032))]
        tube_path(p, pts, 0.0038, "Iron", sides=6, closed=False)
        tip = pts[-1]
        p.lathe([(0.0, 0.0), (0.0038, 0.0), (0.0, 0.007)], "Iron", segs=6,
                xf=T(*tip) @ RY(-math.pi / 2) @ RZ(0.0))
    p.lathe([(0.0, -0.002), (0.008, -0.002), (0.009, 0.004), (0.006, 0.008), (0.0, 0.008)],
            "Iron", segs=10, xf=T(0.036, gy, gz) @ ALONG_X)
    tube_path(p, [(-0.064, gy, gz), (-0.07, gy - 0.008, gz + 0.01), (-0.066, yb + 0.05, 0.03),
                  (-0.052, yb + 0.042, 0.036)], 0.0062, "Rope", sides=8, closed=False)
    return [p]


# --- Zipline kit --------------------------------------------------------------------------------

def zipline_kit():
    s = cube(4.5)
    yb = -s / 2
    p = Part("body")
    # A cable drum on its side with a crank, the trolley clipped on top,
    # and the two ground anchors strapped alongside.
    cy = yb + 0.058
    drum = T(0, cy, 0) @ ALONG_X
    for sx in (-1, 1):
        p.lathe([(0.0, 0.0), (0.056, 0.0), (0.058, 0.002), (0.058, 0.004), (0.056, 0.006),
                 (0.0, 0.006)], "Paint", segs=28,
                xf=drum @ T(0, 0, sx * 0.034 - (0.006 if sx < 0 else 0.0)))
    wave = [(0.0, -0.034)]
    for k in range(17):
        z = -0.034 + 0.068 * k / 16
        wave.append((0.048 if k % 2 == 0 else 0.0496, z))
    wave.append((0.0, 0.034))
    p.lathe(wave, "Steel", segs=24, xf=drum)
    p.cyl(0.006, -0.048, 0.048, "Steel", segs=10, xf=drum)
    # Crank on +X.
    p.box((0.005, 0.034, 0.008), (0.045, cy - 0.014, 0.0), "Iron", bevel=0.001, segs=1)
    p.cyl(0.0048, 0.044, 0.064, "Black", segs=10, xf=T(0, cy - 0.028, 0) @ ALONG_X)
    # Ground anchors: tapered spikes with an eye, along the -X side.
    for x in (-0.07, -0.055):
        xf = T(x, yb + 0.0065, 0.0)
        p.lathe([(0.0, -0.075), (0.002, -0.07), (0.0055, -0.02), (0.0065, 0.06), (0.0, 0.06)],
                "Iron", segs=8, xf=xf)
        ring(p, 0.008, 0.0022, "Iron", xf=xf @ T(0, -0.0043, 0.068) @ UP, segs=10, sides=5)
    for z in (-0.04, 0.03):
        p.box((0.034, 0.004, 0.012), (-0.0625, yb + 0.0145, z), "Olive")
    # Trolley on top: side plates, two wheels, a carabiner and a grip.
    ty = cy + 0.0515
    for sx in (-1, 1):
        p.box((0.004, 0.032, 0.04), (sx * 0.0095, ty + 0.016, 0.0), "Steel", bevel=0.001, segs=1)
    for z in (-0.01, 0.01):
        p.cyl(0.0095, -0.0075, 0.0075, "Black", segs=14, xf=T(0, ty + 0.022, z) @ ALONG_X)
    p.cyl(0.0022, -0.012, 0.012, "Steel", segs=6, xf=T(0, ty + 0.022, -0.01) @ ALONG_X)
    p.cyl(0.0022, -0.012, 0.012, "Steel", segs=6, xf=T(0, ty + 0.022, 0.01) @ ALONG_X)
    ring(p, 0.01, 0.002, "Steel", xf=T(0.0, ty + 0.002, 0.028) @ S(1.0, 1.0, 1.6) @ UP,
         segs=14, sides=5)
    p.cyl(0.0085, -0.035, 0.035, "Rubber", segs=12, xf=T(0, cy + 0.06, -0.045) @ ALONG_X)
    tube_path(p, [(0.0, ty + 0.002, 0.04), (0.0, cy + 0.06, -0.035)], 0.0018, "Olive", sides=4,
              closed=False)
    tube_path(p, [(0.0, cy + 0.049, 0.012), (0.0, ty + 0.012, 0.0), (0.0, ty + 0.03, -0.022)],
              0.0016, "Steel", sides=5, closed=False)
    return [p]


# --- Matches ----------------------------------------------------------------------------------

def matches():
    s = cube(0.02)
    yb = -s / 2
    p = Part("body")
    # A wood-veneer matchbox, the tray slid half out on the red heads,
    # striker strips down both sides, a printed label, one match loose.
    w, h = 0.017, 0.0085
    z0, z1 = -0.0065, 0.0135
    t = 0.0006
    zc = (z0 + z1) / 2
    p.box((w, t, z1 - z0), (0.0, yb + t / 2, zc), "Boards")
    panel(p, "y", -w / 2, w / 2, z0, z1, yb + h - t, yb + h, "Paper",
          [(-0.0068, 0.0068, -0.0045, 0.0115, "Yellow"), (-0.004, 0.004, -0.0005, 0.0075, "Red")])
    for sx in (-1, 1):
        p.box((t, h - 2 * t, z1 - z0), (sx * (w - t) / 2, yb + h / 2, zc), "Bakelite")
    # The tray: what shows of it, out of the -Z end.
    tw = w - 2 * t - 0.0004
    p.box((tw, t, 0.0085), (0.0, yb + t + t / 2, -0.0095), "Boards")
    p.box((tw, h - 2 * t - 0.0004, t), (0.0, yb + h / 2, -0.0132), "Boards")
    for sx in (-1, 1):
        p.box((t, h - 2 * t - 0.0006, 0.0085), (sx * (tw - t) / 2, yb + h / 2 - 0.0002, -0.0095),
              "Boards")
    for k in range(7):
        x = -0.0055 + k * 0.0018
        y = yb + 2 * t + 0.0008 + (0.0012 if k % 2 else 0.0)
        p.box((0.0012, 0.0012, 0.022), (x, y, 0.0), "Cream")
        p.box((0.0017, 0.0017, 0.003), (x, y, -0.011), "Red", bevel=0.0005, segs=1)
    xf = T(0.002, yb + h + 0.0007, 0.004) @ RY(0.5)
    p.box((0.0012, 0.0012, 0.02), (0.0, 0.0, 0.0), "Cream", xf=xf)
    p.box((0.0017, 0.0017, 0.003), (0.0, 0.0, -0.0105), "Bakelite", bevel=0.0005, segs=1, xf=xf)
    # Modelled small; a real box is about 5 x 3.6 x 1.7 cm.
    return centred(transform([p], S(2.1, 1.85, 2.0)))


# --- Door wedge -------------------------------------------------------------------------------

def door_wedge():
    s = cube(0.15)
    yb = -s / 2
    p = Part("body")
    # A chunky yellow plastic wedge on a black rubber sole, grip ridges up
    # its slope and a cord loop through its heel.
    sole = 0.0035
    prof = [(-0.026, 0.0), (0.026, 0.0), (0.026, 0.02), (0.022, 0.0235), (0.015, 0.024),
            (-0.026, 0.003)]
    p.prism(fillet([(z, yb + sole + y) for z, y in prof], [0.001, 0.002, 0.003, 0.003, 0.004,
                                                          0.001], 2),
            -0.019, 0.019, "Yellow", axis="x", bevel=0.0009, segs=2)
    p.box((0.038, sole, 0.052), (0.0, yb + sole / 2, 0.0), "Rubber", bevel=0.0008, segs=1)
    ang = math.atan2(0.021, 0.041)
    for k in range(6):
        z = -0.019 + k * 0.0068
        y = yb + sole + 0.003 + (z + 0.026) * math.tan(ang)
        p.box((0.034, 0.0028, 0.0022), (0.0, 0.0, 0.0), "Yellow",
              xf=T(0.0, y + 0.0006, z) @ RX(ang))
    p.cyl(0.0035, -0.0195, 0.0195, "Black", segs=10, xf=T(0, yb + sole + 0.012, 0.019) @ ALONG_X)
    tube_path(p, [(0.0195, yb + sole + 0.012, 0.019), (0.023, yb + sole + 0.016, 0.022),
                  (0.022, yb + sole + 0.024, 0.024), (0.012, yb + sole + 0.026, 0.024),
                  (0.0, yb + sole + 0.0245, 0.022), (-0.012, yb + sole + 0.026, 0.024),
                  (-0.022, yb + sole + 0.024, 0.024), (-0.023, yb + sole + 0.016, 0.022),
                  (-0.0195, yb + sole + 0.012, 0.019)], 0.0011, "Rope", sides=5, closed=False)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


for item_id, fn in (("camera_drone", camera_drone), ("lead_case", lead_case),
                    ("cloak_generator", cloak_generator), ("deployable_cover", deployable_cover),
                    ("climbing_kit", climbing_kit), ("zipline_kit", zipline_kit),
                    ("matches", matches), ("door_wedge", door_wedge)):
    if wanted(item_id):
        item(item_id, fn)

for item_id, fn in (("flashlight", flashlight), ("power_cell", power_cell),
                    ("compass", compass), ("radio", radio), ("binoculars", binoculars),
                    ("emp_grenade", emp_grenade), ("emp_trap", emp_trap),
                    ("map_case", map_case)):
    if wanted(item_id):
        item(item_id, fn)
