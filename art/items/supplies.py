"""Medical and maintenance supplies: field dressing, regen injector,
cleaning kit, gun oil, spare parts, multitool, fuel can. Writes
assets/models/items/<id>.glb for each (item space: see _kit.py). Things
without `dims` are a cube of side cbrt(bulk / 1000); each model rests on
that cube's floor.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, centred, cube, item, ring, text, transform, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, fillet, section_rect


def pillow(p, hx, hy, hz, y, mat, r=0.006):
    """A soft packet: a rounded slab, its edges pinched."""
    prof = fillet([(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)], r, 3)
    p.prism(prof, y - hy, y + hy, mat, axis="y", bevel=hy * 0.9, segs=3)


# --- Field dressing ---------------------------------------------------------------------------

def field_dressing():
    s = cube(0.1)
    p = Part("body")
    # A rubberised packet with a paper label and a red tear-strip. The
    # label sits in a recess cut for it, flush, not on the rubber.
    y = -s / 2 + 0.012
    pillow(p, s / 2 - 0.001, 0.012, s / 2 - 0.003, y, "Olive", r=0.008)
    p.box((s - 0.01, 0.003, 0.004), (0.0, y, -s / 2 + 0.003), "Red")
    recess = Part("recess")
    recess.box((0.026, 0.009, 0.019), (0.0, y + 0.012, 0.0005), "Paper")
    p.cut(recess)
    label = Part("label")
    label.box((0.026, 0.0042, 0.019), (0.0, y + 0.0101, 0.0005), "Paper")
    label.box((0.016, 0.0009, 0.003), (-0.003, y + 0.0124, -0.005), "Red")
    label.box((0.016, 0.0009, 0.002), (-0.003, y + 0.0124, 0.0), "Red")
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p, label])


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
    # Modelled stubby; a real auto-injector is a 13 cm pen.
    return centred(transform([p], S(1.35, 1.35, 2.6)))


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
    p.prism(tin, yb, yb + 0.014, "Blued", axis="y", bevel=0.0015, segs=1)
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
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


# --- Multitool --------------------------------------------------------------------------------

def multitool():
    s = cube(0.1)
    yb = -s / 2
    p = Part("body")
    # A folding pliers tool lying closed: two steel handle channels with
    # black grip inlays, the pliers' jaws out at the front, the knife blade
    # swung half open.
    for sx in (-1, 1):
        p.box((0.0072, 0.006, 0.03), (sx * 0.0038, yb + 0.003, 0.007), "Steel", bevel=0.0008,
              segs=1)
        p.box((0.0012, 0.0036, 0.022), (sx * 0.0078, yb + 0.003, 0.008), "Black", bevel=0.0003,
              segs=1)
        jaw = [(0.0, -0.007), (0.0055, -0.007), (0.0035, -0.0175), (0.0014, -0.0222),
               (0.0, -0.0222)]
        p.prism([(sx * x, z) for x, z in jaw], yb + 0.001, yb + 0.005, "Steel", axis="y",
                xf=T(0.0, 0.0, -0.007) @ RY(sx * 0.07) @ T(0.0, 0.0, 0.007))
        for k in range(3):
            p.box((0.0016, 0.0042, 0.0008), (sx * 0.0012, yb + 0.003, -0.012 - k * 0.0025), "Dark",
                  xf=RY(sx * 0.07))
    p.cyl(0.0024, yb + 0.0002, yb + 0.0058, "Steel", segs=12, xf=T(0, 0, -0.0072) @ UP)
    for z in (0.0, 0.019):
        p.cyl(0.0014, yb + 0.0058, yb + 0.0064, "Steel", segs=8, xf=T(0.0038, 0, z) @ UP)
        p.cyl(0.0014, yb + 0.0058, yb + 0.0064, "Steel", segs=8, xf=T(-0.0038, 0, z) @ UP)
    blade = [(0.0, -0.0026), (0.0155, -0.0026), (0.0192, 0.0004), (0.0, 0.0028)]
    p.prism(blade, yb + 0.0062, yb + 0.007, "Steel", axis="y", xf=T(0.0038, 0, 0.019) @ RY(0.25))
    ring(p, 0.003, 0.0007, "Steel", xf=T(0.0, yb + 0.003, 0.0195) @ RX(math.pi / 2), segs=10,
         sides=4)
    # Modelled small; a real folding pliers tool is about 10 cm closed.
    return centred(transform([p], S(2.2, 2.2, 2.2)))


# --- Fuel can ---------------------------------------------------------------------------------

def fuel_can():
    p = Part("body")
    # A ten-litre olive jerrycan: pressed X ribs on both flanks, a welded
    # seam round its middle, three handles on top, a cam-lever spout cap on
    # the cut-off front corner, the fuel stencilled front and back.
    yb = -0.175
    prof = [(-0.145, yb), (0.145, yb), (0.145, yb + 0.33), (-0.1, yb + 0.33), (-0.145, yb + 0.29)]
    p.prism(fillet(prof, 0.015, 3), -0.075, 0.075, "Paint", axis="x", bevel=0.01, segs=2)
    seam = [(-0.1485, yb + 0.0035), (0.1485, yb + 0.0035), (0.1485, yb + 0.3335),
            (-0.1015, yb + 0.3335), (-0.1485, yb + 0.2915)]
    p.prism(fillet(seam, 0.017, 3), -0.002, 0.002, "Paint", axis="x")
    # X ribs: one bar corner to corner, the other in two halves either side.
    ang = math.atan2(0.25, 0.22)
    L = math.hypot(0.25, 0.22)
    yc = yb + 0.16
    for sx in (-1, 1):
        x = sx * 0.0775
        p.box((0.0055, 0.024, L), (x, 0.0, 0.0), "Paint", bevel=0.002, segs=1,
              xf=T(0, yc, 0) @ RX(ang))
        for sz in (-1, 1):
            p.box((0.0055, 0.024, L / 2 - 0.022), (x, 0.0, sz * (L / 4 + 0.011)), "Paint",
                  bevel=0.002, segs=1, xf=T(0, yc, 0) @ RX(-ang))
    # Handles on top.
    top = yb + 0.33
    for x in (-0.035, 0.0, 0.035):
        tube_path(p, [(x, top - 0.002, -0.005), (x, top + 0.011, 0.008), (x, top + 0.011, 0.1),
                      (x, top - 0.002, 0.113)], 0.0055, "Paint", sides=6, closed=False)
    # Spout and cap, square to the cut-off corner.
    xf = T(0.0, yb + 0.3078, -0.1205) @ RX(-0.835) @ UP
    p.cyl(0.016, -0.004, 0.009, "Steel", segs=16, xf=xf)
    p.lathe([(0.0, 0.009), (0.02, 0.009), (0.021, 0.011), (0.021, 0.019), (0.018, 0.022),
             (0.0, 0.022)], "Paint", segs=18, xf=xf)
    p.box((0.008, 0.006, 0.04), (0.0, 0.0, 0.0), "Steel", bevel=0.0015, segs=1,
          xf=xf @ T(0.0, 0.0, 0.025) @ RX(math.pi / 2) @ T(0.0, 0.0, 0.004))
    for sz, z in ((-1, -0.145), (1, 0.145)):
        text(p, "ДТ", 0.045, "Cream", T(0.0, yb + 0.13, z) @ (RY(math.pi) if sz < 0 else T()),
             depth=0.0035)
    return [p]


for item_id, fn in (("field_dressing", field_dressing), ("regen_injector", regen_injector),
                    ("cleaning_kit", cleaning_kit), ("gun_oil", gun_oil),
                    ("spare_parts", spare_parts), ("multitool", multitool),
                    ("fuel_can", fuel_can)):
    if wanted(item_id):
        item(item_id, fn)
