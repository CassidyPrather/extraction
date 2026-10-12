"""Medical and maintenance supplies: field dressing, regen injector,
cleaning kit, gun oil, spare parts. Writes assets/models/items/<id>.glb
for each (item space: see _kit.py). Things without `dims` are a cube of
side cbrt(bulk / 1000); each model rests on that cube's floor.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, cube, item, wanted
from common import Part, RX, RY, RZ, T, S, fillet, section_rect


def pillow(p, hx, hy, hz, y, mat, r=0.006):
    """A soft packet: a rounded slab, its edges pinched."""
    prof = fillet([(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)], r, 3)
    p.prism(prof, y - hy, y + hy, mat, axis="y", bevel=hy * 0.9, segs=3)


# --- Field dressing ---------------------------------------------------------------------------

def field_dressing():
    s = cube(0.1)
    p = Part("body")
    # A rubberised packet with a paper label and a red tear-strip.
    y = -s / 2 + 0.012
    pillow(p, s / 2 - 0.001, 0.012, s / 2 - 0.003, y, "Olive", r=0.008)
    p.box((0.03, 0.0008, 0.026), (0.0, y + 0.012, 0.002), "Paper")
    p.box((0.016, 0.0009, 0.003), (-0.003, y + 0.0125, -0.005), "Red")
    p.box((0.016, 0.0009, 0.002), (-0.003, y + 0.0125, 0.0), "Red")
    p.box((s - 0.01, 0.003, 0.004), (0.0, y, -s / 2 + 0.003), "Red")
    return [p]


# --- Regen injector ---------------------------------------------------------------------------

def regen_injector():
    s = cube(0.08)
    y = -s / 2 + 0.0085
    p = Part("body")
    ax = T(0, y, 0)
    # A stubby auto-injector lying along Z: grip, a window on the fluid, cap.
    p.lathe([(0.0, -0.022), (0.006, -0.022), (0.0085, -0.019), (0.0085, 0.016), (0.0075, 0.019),
             (0.0, 0.019)], "Polymer", segs=16, xf=ax)
    p.tube(0.0088, 0.008, -0.012, 0.006, "Clear", segs=16, xf=ax)
    p.cyl(0.0078, -0.011, 0.005, "Glow", segs=12, xf=ax)
    for z in (-0.017, 0.011):
        p.tube(0.0089, 0.0082, z - 0.0015, z + 0.0015, "Black", segs=16, xf=ax)
    p.lathe([(0.0, 0.019), (0.0072, 0.019), (0.0072, 0.026), (0.006, 0.028), (0.0, 0.028)],
            "Yellow", segs=16, xf=ax)
    p.box((0.002, 0.006, 0.014), (0.0, y + 0.008, -0.004), "Black", bevel=0.0005, segs=1)
    return [p]


# --- Cleaning kit -----------------------------------------------------------------------------

def cleaning_kit():
    p = Part("body")
    # A canvas roll with two leather straps, the rod poking out one end,
    # a brush on it, and a little oiler tucked under the straps.
    roll = fillet([(-0.036, -0.022), (0.036, -0.022), (0.036, 0.016), (-0.036, 0.016)],
                  [0.012, 0.012, 0.02, 0.02], 4)
    p.prism(roll, -0.1, 0.1, "Canvas", axis="z", bevel=0.006, segs=2)
    p.box((0.07, 0.003, 0.16), (0.0, 0.0175, 0.0), "Canvas", bevel=0.0012, segs=1,
          xf=RX(0.02))
    for z in (-0.06, 0.055):
        strap = fillet([(-0.038, -0.024), (0.038, -0.024), (0.038, 0.019), (-0.038, 0.019)],
                       [0.013, 0.013, 0.021, 0.021], 4)
        p.prism(strap, z - 0.009, z + 0.009, "Bakelite", axis="z")
        p.box((0.012, 0.004, 0.014), (0.03, 0.019, z), "Steel", bevel=0.001, segs=1)
    p.cyl(0.0035, 0.08, 0.124, "Steel", segs=8, xf=T(0.018, 0.0, 0.0))
    p.cyl(0.0055, 0.112, 0.125, "Brass", segs=10, xf=T(0.018, 0.0, 0.0))
    p.cyl(0.003, -0.124, -0.09, "Steel", segs=8, xf=T(-0.012, -0.004, 0.0))
    p.lathe([(0.0, -0.125), (0.007, -0.122), (0.007, -0.114), (0.0, -0.114)], "Bakelite",
            segs=10, xf=T(-0.012, -0.004, 0.0))
    return [p]


# --- Gun oil --------------------------------------------------------------------------------

def gun_oil():
    s = cube(0.12)
    yb = -s / 2
    p = Part("body")
    # A small oil can: a round tin, a stepped shoulder, a spout and cap.
    p.lathe([(0.0, yb), (0.018, yb), (0.0195, yb + 0.002), (0.0195, yb + 0.026),
             (0.018, yb + 0.028), (0.009, yb + 0.033), (0.0045, yb + 0.034), (0.0, yb + 0.034)],
            "Paint", segs=20, xf=UP)
    p.tube(0.0198, 0.017, yb + 0.006, yb + 0.02, "Cream", segs=20, xf=UP)
    p.tube(0.0199, 0.017, yb + 0.011, yb + 0.015, "Red", segs=20, xf=UP)
    p.lathe([(0.0, yb + 0.033), (0.0035, yb + 0.033), (0.0022, yb + 0.044), (0.0, yb + 0.044)],
            "Brass", segs=10, xf=UP)
    p.cyl(0.003, yb + 0.044, yb + 0.049, "Black", segs=10, xf=UP, chamfer=0.0008)
    return [p]


# --- Spare parts -----------------------------------------------------------------------------

def spare_parts():
    s = cube(0.2)
    yb = -s / 2
    p = Part("body")
    # A small tin with its lid on, and on it a recoil spring, an extractor
    # and a few pins, tied with wire.
    tin = fillet([(-0.026, -0.02), (0.026, -0.02), (0.026, 0.02), (-0.026, 0.02)], 0.005, 2)
    p.prism(tin, yb, yb + 0.018, "Blued", axis="y", bevel=0.0015, segs=1)
    lid = fillet([(-0.0265, -0.0205), (0.0265, -0.0205), (0.0265, 0.0205), (-0.0265, 0.0205)],
                 0.0055, 2)
    p.prism(lid, yb + 0.014, yb + 0.02, "Blued", axis="y", bevel=0.0015, segs=1)
    p.box((0.03, 0.0005, 0.018), (0.0, yb + 0.0203, 0.0), "Paper")
    # Coiled spring lying across the lid.
    ys = yb + 0.0255
    for k in range(9):
        z = -0.016 + k * 0.004
        p.lathe([(0.0042, -0.0007), (0.0049, 0.0), (0.0042, 0.0007), (0.0035, 0.0)], "Steel",
                segs=10, xf=T(-0.008, ys, z) @ RX(0.25), closed=True)
    # Extractor: a little hooked bar.
    ext = [(0.0, 0.0), (0.016, 0.0), (0.018, 0.003), (0.016, 0.004), (0.004, 0.004), (0.0, 0.003)]
    p.prism([(x + 0.003, z - 0.01) for x, z in ext], yb + 0.0205, yb + 0.0235, "Blued", axis="y")
    for k in range(3):
        p.cyl(0.0012, -0.008, 0.008, "Steel", segs=6,
              xf=T(0.013 + k * 0.004, yb + 0.0215, 0.01) @ RY(0.3 * k))
    # Binding wire round the lot.
    for x in (-0.018, 0.018):
        p.box((0.0012, 0.0012, 0.042), (x, yb + 0.0207, 0.0), "Steel")
    return [p]


for item_id, fn in (("field_dressing", field_dressing), ("regen_injector", regen_injector),
                    ("cleaning_kit", cleaning_kit), ("gun_oil", gun_oil),
                    ("spare_parts", spare_parts)):
    if wanted(item_id):
        item(item_id, fn)
