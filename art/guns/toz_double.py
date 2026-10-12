"""TOZ side-by-side double-barrel 12 gauge, boxlock, double triggers.
Writes assets/models/guns/toz_double.glb.

Gun space as in src/weapons/layout.rs (`toz()`): origin at the palm on the
wrist, -Z down the bores (axes at x = +-0.0128, y = 0.035), +Y up, metres.
The `barrels` object (barrels, rib, lumps, forend) has its origin on the
hinge pin, which the game tips it around.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_rect)

main_guard()

HINGE = (0.0, 0.012, -0.11)
BX, BY = 0.0128, 0.035
BREECH, MUZZLE = -0.1165, -0.82

# --- Barrels (tip at the hinge) -----------------------------------------------------
bar = Part("barrels", origin=HINGE)
for sx in (-1, 1):
    bar.lathe([(0.0093, BREECH), (0.0124, BREECH), (0.0124, BREECH - 0.085),
               (0.0114, BREECH - 0.16), (0.0109, MUZZLE + 0.004), (0.0103, MUZZLE),
               (0.0093, MUZZLE)], "Blued", segs=22, xf=T(sx * BX, BY, 0), closed=True)
# Top rib, a slightly concave strip that tapers towards the muzzle.
rib = [(BREECH - 0.002, 0.0455), (MUZZLE + 0.001, 0.0425), (MUZZLE + 0.001, 0.0478),
       (BREECH - 0.002, 0.0515)]
bar.prism(rib, -0.0052, 0.0052, "Blued", axis="x", bevel=0.0008, segs=1)
# Bottom rib between the barrels.
bar.prism([(BREECH - 0.05, 0.0225), (MUZZLE + 0.001, 0.0255), (MUZZLE + 0.001, 0.031),
           (BREECH - 0.05, 0.028)], -0.004, 0.004, "Blued", axis="x", bevel=0.0006, segs=1)
# Front bead.
bar.sphere(0.0019, (0.0, 0.049, MUZZLE + 0.008), "Steel", segs=10, rings=6)
# Lumps under the breech, hooking onto the hinge pin.
lump = fillet([(BREECH, 0.024), (BREECH - 0.058, 0.024), (BREECH - 0.058, 0.014),
               (-0.122, 0.002), (-0.110, 0.0005), (-0.1005, 0.008), (BREECH, 0.016)],
              [0.0, 0.002, 0.004, 0.006, 0.004, 0.003, 0.0], 3)
bar.prism(lump, -0.011, 0.011, "Blued", axis="x", bevel=0.0012, segs=1)
# Extractor across the breech face, between the chambers.
bar.box((0.012, 0.012, 0.004), (0.0, BY + 0.0005, BREECH - 0.0025), "Steel", bevel=0.001)
# Forend iron and the wooden forend.
fi = fillet([(-0.0205, 0.004), (0.0205, 0.004), (0.023, 0.020), (-0.023, 0.020)],
            [0.006, 0.006, 0.002, 0.002], 3)
bar.prism(fi, -0.215, -0.165, "Steel", axis="z", bevel=0.002, segs=2)
fe = fillet([(-0.017, -0.016), (0.017, -0.016), (0.0255, 0.006), (0.0245, 0.029),
             (-0.0245, 0.029), (-0.0255, 0.006)], [0.010, 0.010, 0.006, 0.002, 0.002, 0.006], 4)
bar.prism(fe, -0.43, -0.212, "Wood", axis="z", bevel=0.005, segs=2)
bar.box((0.008, 0.005, 0.014), (0.0, 0.0045, -0.205), "Steel", bevel=0.0015)  # forend latch
# Front sling swivel on the bottom rib.
loop = [(-0.62 + 0.009 * math.cos(a), 0.014 + 0.009 * math.sin(a))
        for a in [math.pi + math.pi * i / 8 for i in range(9)]]
bar.sweep(loop, section_rect(0.0025, 0.0025, 0.0007), "Steel")
bar.box((0.006, 0.006, 0.02), (0.0, 0.0205, -0.62), "Steel", bevel=0.0015)

# --- Action (boxlock) ---------------------------------------------------------------------
act = Part("action")
act_prof = fillet([
    (-0.1185, 0.019), (-0.1185, 0.0525),   # standing breech face
    (-0.106, 0.0535), (-0.094, 0.050),     # fences down to the top strap
    (-0.06, 0.0455), (-0.028, 0.040),
    (-0.028, -0.0115), (-0.06, -0.0135),   # trigger plate
    (-0.088, -0.008), (-0.104, -0.0005), (-0.1185, 0.008),  # knuckle round the hinge pin
], [0.0, 0.003, 0.004, 0.01, 0.01, 0.002, 0.004, 0.012, 0.012, 0.01, 0.004], 4)


def act_w(z, y):
    k = 0.0245 - max(0.0, z + 0.06) * 0.06
    return (-k, k)


act.prism(act_prof, 0, 0, "Steel", axis="x", bevel=0.003, segs=2, width=act_w)
# Fences: rounded shoulders behind each barrel.
for sx in (-1, 1):
    act.sphere(0.0135, (sx * BX, BY + 0.002, -0.1135), "Steel", segs=20, rings=10,
               scale=(1.0, 1.0, 0.55))
# Hinge pin heads.
for sx in (-1, 1):
    act.cyl(0.0045, sx * 0.0225, sx * 0.0258, "Steel", segs=14,
            xf=T(0, HINGE[1], HINGE[2]) @ RY(math.pi / 2), chamfer=0.0007)
# Firing pin bushings on the breech face.
for sx in (-1, 1):
    act.cyl(0.0028, BREECH - 0.0022, BREECH + 0.0002, "Steel", segs=12, xf=T(sx * BX, BY, 0))
# Top lever on the tang, thumb piece swung to the right; safety slide behind.
tl = fillet([(-0.006, -0.050), (0.006, -0.050), (0.005, -0.030), (0.016, -0.018), (0.012, -0.012),
             (-0.004, -0.024)], [0.004, 0.004, 0.004, 0.003, 0.003, 0.004], 3)
act.prism(tl, 0.0, 0.0, "Blued", axis="y", bevel=0.0008, segs=1,
          width=lambda x, z: (0.042 - (z + 0.05) * 0.06, 0.0475 - (z + 0.05) * 0.06))
act.box((0.006, 0.004, 0.009), (0.0, 0.0405, -0.004), "Blued", bevel=0.0012)
# Top tang onto the wrist.
act.prism(fillet([(-0.03, 0.0355), (0.025, 0.0275), (0.03, 0.0305), (-0.03, 0.041)], 0.003, 2),
          -0.0065, 0.0065, "Steel", axis="x", bevel=0.001, segs=1)

# Trigger guard with its long tang, and the rear trigger (the front one moves).
guard_path = catmull([(-0.072, -0.0175), (-0.070, -0.032), (-0.058, -0.0405), (-0.036, -0.0425),
                      (-0.016, -0.038), (-0.004, -0.032), (0.018, -0.032), (0.04, -0.034)], 4,
                     closed=False)
act.sweep(guard_path, section_rect(0.0095, 0.003, 0.001), "Blued")
act.sweep(catmull([(-0.033, -0.017), (-0.0345, -0.025), (-0.032, -0.032), (-0.027, -0.0355)], 3,
                  closed=False), section_rect(0.0045, 0.003, 0.0011), "Blued")

# --- Stock --------------------------------------------------------------------------------
stock_prof = [
    (-0.029, 0.0385), (0.025, 0.0265), (0.06, 0.0215), (0.14, 0.019), (0.358, 0.006),
    (0.366, -0.118),
    (0.27, -0.098), (0.14, -0.066), (0.085, -0.058), (0.062, -0.054),   # semi-pistol grip
    (0.035, -0.032), (0.0, -0.024), (-0.029, -0.0155),
]
stock_prof = fillet(stock_prof, [0.002, 0.012, 0.02, 0.02, 0.005, 0.005, 0.05, 0.03, 0.01, 0.008,
                                 0.012, 0.008, 0.002], 4)
WIDTH = [(-0.03, 0.0175), (0.0, 0.0165), (0.05, 0.0172), (0.13, 0.0205), (0.37, 0.0215)]


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


stock = Part("stock")
stock.prism(stock_prof, 0, 0, "Wood", axis="x", bevel=0.0095, segs=4,
            width=lambda z, y: (-lerp_table(z, WIDTH), lerp_table(z, WIDTH)))
# Rubber butt pad.
pad = fillet([(0.3545, 0.0075), (0.371, 0.0055), (0.381, -0.120), (0.364, -0.1205)],
             [0.001, 0.004, 0.004, 0.001], 3)
stock.prism(pad, -0.0215, 0.0215, "Black", axis="x", bevel=0.003, segs=2)
# Rear sling swivel under the butt.
loop = [(0.25 + 0.009 * math.cos(a), -0.104 + 0.009 * math.sin(a))
        for a in [math.pi + math.pi * i / 8 for i in range(9)]]
stock.sweep(loop, section_rect(0.0025, 0.0025, 0.0007), "Steel", xf=RX(0.0))
stock.box((0.006, 0.006, 0.02), (0.0, -0.097, 0.25), "Steel", bevel=0.0015,
          xf=RX(math.atan2(0.032, 0.13), about=(0, -0.097, 0.25)))

# --- Front trigger ---------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, -0.012, -0.05))
trig.sweep(catmull([(-0.049, -0.017), (-0.0505, -0.025), (-0.048, -0.032), (-0.043, -0.036)], 3,
                   closed=False), section_rect(0.0045, 0.003, 0.0011), "Blued")

join("static", act, stock)
make(bar)
make(trig)

empty("pt_muzzle", (0.0, 0.035, -0.82))
empty("pt_sight", (0.0, 0.07, -0.05))
empty("pt_hinge", HINGE)

report()
export("assets/models/guns/toz_double.glb")
