"""Gadgets: flashlight, power cell, compass, field radio, binoculars, EMP
grenade, EMP trap, map case. Writes assets/models/items/<id>.glb for each
(item space: see _kit.py).

Built round what src/gadgets/ hangs on them:

- flashlight: the beam leaves from z -0.06; a power cell seats standing up
  through the bay at (0, 0, 0.02).
- compass: the game draws the face (y 0.016) and the needle (y 0.019) on
  top of the case, so the case's dial seat is at y 0.015.
- map_case: the game draws the map sheet (0.28 m square) at y 0.006,
  between the backing board and the clear sleeve.
- binoculars: the eye goes at (0, 0, 0.1).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, cell_bay, cube, item, wanted
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
    # Speaker grille and a little tuning window on the front (+Z).
    for k in range(6):
        p.box((0.04, 0.003, 0.002), (0.0, -0.06 + k * 0.007, D / 2 + 0.0005), "Black")
    p.box((0.046, 0.022, 0.002), (0.0, 0.02, D / 2), "Black", bevel=0.001, segs=1)
    p.box((0.04, 0.016, 0.001), (0.0, 0.02, D / 2 + 0.0012), "Cream")
    p.box((0.0012, 0.014, 0.0008), (0.006, 0.02, D / 2 + 0.002), "LampRed")
    # Knobs on top, the rubber antenna, push-to-talk on the side.
    for x, r in ((-0.018, 0.007), (0.0, 0.0055)):
        p.cyl(r, y1, y1 + 0.012, "Black", segs=14, xf=T(x, 0, 0) @ UP, chamfer=0.0015)
        p.box((0.0015, 0.002, r * 1.6), (x, y1 + 0.0125, 0.0), "Cream")
    p.cyl(0.007, y1, y1 + 0.012, "Black", segs=12, xf=T(0.019, 0, 0) @ UP)
    p.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0045, 0.06), (0.0035, 0.07), (0.0, 0.072)], "Rubber",
            segs=10, xf=T(0.019, y1 + 0.012, 0) @ UP)
    p.box((0.004, 0.026, 0.016), (W / 2 + 0.001, 0.0, 0.0), "Black", bevel=0.0015, segs=1)
    # Belt clip on the back.
    p.box((0.02, 0.06, 0.003), (0.0, 0.0, -D / 2 - 0.0035), "Steel", bevel=0.001, segs=1)
    p.box((0.02, 0.006, 0.005), (0.0, 0.028, -D / 2 - 0.002), "Steel")
    return [p]


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
    # Clear sleeve over it, a stitched border round the window.
    p.prism(fillet([(-0.142, -0.142), (0.142, -0.142), (0.142, 0.142), (-0.142, 0.142)], 0.006,
                   2), 0.0068, 0.0073, "Clear", axis="y")
    for sx in (-1, 1):
        p.box((0.012, 0.0035, 0.3), (sx * 0.144, 0.0062, 0.0), "Olive", bevel=0.001, segs=1)
        p.box((0.276, 0.0035, 0.012), (0.0, 0.0062, sx * 0.144), "Olive", bevel=0.001, segs=1)
    # A press stud flap on one edge, and a pencil in a loop.
    p.box((0.06, 0.004, 0.03), (0.0, 0.006, 0.158), "Olive", bevel=0.0015, segs=1)
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
    p.box((0.05, 0.002, 0.03), (0.0, 0.0355, 0.0), "Yellow")
    return [p]


for item_id, fn in (("camera_drone", camera_drone), ("lead_case", lead_case)):
    if wanted(item_id):
        item(item_id, fn)

for item_id, fn in (("flashlight", flashlight), ("power_cell", power_cell),
                    ("compass", compass), ("radio", radio), ("binoculars", binoculars),
                    ("emp_grenade", emp_grenade), ("emp_trap", emp_trap),
                    ("map_case", map_case)):
    if wanted(item_id):
        item(item_id, fn)
