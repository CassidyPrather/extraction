"""Mosin-Nagant 91/30: 7.62x54R bolt-action rifle, 5-round internal
magazine. Writes assets/models/guns/mosin.glb.

Gun space as in src/weapons/layout.rs (`mosin()`): origin at the palm on the
wrist, -Z down the bore (bore axis y = 0.045), +Y up, metres. The bolt is
coaxial with the bore; its object origin sits on the bore axis so the game
turns it about that axis and slides it back.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_rect)

main_guard()

BORE = 0.045
MUZZLE = -0.98

# --- Stock ------------------------------------------------------------------------
stock_prof = [
    (-0.866, 0.037), (-0.20, 0.037),          # forend top (barrel channel edge)
    (-0.19, 0.031), (-0.012, 0.031),          # under the receiver
    (0.004, 0.030), (0.035, 0.0215),          # wrist top, down to the comb
    (0.322, 0.0145),                          # heel
    (0.329, -0.107),                          # toe
    (0.24, -0.090), (0.11, -0.060), (0.045, -0.038), (0.0, -0.031),  # belly to wrist
    (-0.05, -0.027), (-0.20, -0.026),         # under the magazine
    (-0.60, -0.020), (-0.858, -0.011), (-0.868, 0.004),
]
stock_prof = fillet(stock_prof, [0.004, 0.002, 0.002, 0.004, 0.01, 0.03, 0.006, 0.006, 0.05,
                                 0.05, 0.03, 0.02, 0.01, 0.01, 0.01, 0.006, 0.006], 4)


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


WIDTH = [(-0.9, 0.0155), (-0.6, 0.0185), (-0.25, 0.0205), (-0.19, 0.022), (-0.06, 0.022),
         (-0.02, 0.0175), (0.03, 0.0165), (0.09, 0.0195), (0.33, 0.0215)]


def stock_w(z, y):
    k = lerp_table(z, WIDTH)
    # Round the belly of the butt a little narrower.
    if z > 0.05 and y < -0.06:
        k -= (-0.06 - y) * 0.06
    return (-k, k)


stock = Part("stock")
stock.prism(stock_prof, 0, 0, "Wood", axis="x", bevel=0.0085, segs=4, width=stock_w)
sc = Part("cut")
# Sling slots through the butt and the forend.
sc.prism(fillet([(0.15, -0.037), (0.19, -0.044), (0.19, -0.052), (0.15, -0.045)], 0.004, 2),
         -0.04, 0.04, "Wood", axis="x")
sc.prism(fillet([(-0.66, -0.006), (-0.62, -0.006), (-0.62, -0.012), (-0.66, -0.012)], 0.003, 2),
         -0.04, 0.04, "Wood", axis="x")
stock.cut(sc)

# Steel butt plate.
furn = Part("furniture")
bp = fillet([(0.318, 0.0175), (0.3245, 0.0145), (0.3315, -0.107), (0.3255, -0.1095),
             (0.322, -0.097)], [0.0, 0.002, 0.003, 0.002, 0.0], 2)
furn.prism(bp, -0.0215, 0.0215, "Blued", axis="x", bevel=0.0015, segs=1)

# Upper handguard over the barrel.
hg_sec = fillet([(-0.0145, 0.0365), (0.0145, 0.0365), (0.0145, 0.052), (0.0, 0.0615),
                 (-0.0145, 0.052)], [0.001, 0.001, 0.006, 0.011, 0.006], 4)
furn.prism(hg_sec, -0.47, -0.245, "Wood", axis="z", bevel=0.004, segs=2)

# Barrel bands, with sling swivels on the left.
for z, top in ((-0.70, 0.0585), (-0.49, 0.0655)):
    band = fillet([(-0.0215, -0.025), (0.0215, -0.025), (0.0215, top - 0.012), (0.0, top),
                   (-0.0215, top - 0.012)], [0.012, 0.012, 0.009, 0.014, 0.009], 4)
    furn.prism(band, z - 0.007, z + 0.007, "Blued", axis="z", bevel=0.0012, segs=1)
    loop = [(z + 0.012 * math.cos(a), -0.006 + 0.012 * math.sin(a))
            for a in [math.pi * 0.5 + math.pi * i / 8 for i in range(9)]]
    furn.sweep(loop, section_rect(0.003, 0.003, 0.0008), "Blued", xf=T(-0.0235, 0, 0))
# Nose cap.
nose = fillet([(-0.0168, -0.013), (0.0168, -0.013), (0.0168, 0.039), (-0.0168, 0.039)],
              [0.008, 0.008, 0.004, 0.004], 3)
furn.prism(nose, -0.874, -0.858, "Blued", axis="z", bevel=0.0012, segs=1)

# --- Barrel and sights ---------------------------------------------------------------------
barrel = Part("barrel")
barrel.lathe([(0.0042, MUZZLE + 0.002), (0.0095, MUZZLE), (0.0105, MUZZLE + 0.003),
              (0.0105, -0.60), (0.0118, -0.45), (0.0128, -0.26), (0.0138, -0.245),
              (0.0145, -0.205), (0.0, -0.205)], "Blued", segs=20, xf=T(0, BORE, 0))
# Front sight: a ring base, the post and its hood.
barrel.cyl(0.0122, -0.972, -0.952, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.prism(fillet([(-0.968, 0.054), (-0.956, 0.054), (-0.958, 0.0805), (-0.964, 0.0805)],
                    0.001, 1), -0.0012, 0.0012, "Blued", axis="x", bevel=0.0003, segs=1)
barrel.tube(0.0118, 0.0104, -0.971, -0.951, "Blued", segs=18, xf=T(0, 0.0645, 0))
# Cleaning rod, ahead of the nose cap.
barrel.cyl(0.0029, -0.94, -0.85, "Steel", segs=8, xf=T(0, 0.026, 0))
barrel.lathe([(0.0, -0.952), (0.0045, -0.952), (0.0045, -0.938), (0.003, -0.936), (0.0, -0.936)],
             "Steel", segs=10, xf=T(0, 0.026, 0))
# Rear sight: the ramp base on the barrel and the tangent leaf.
rs = fillet([(-0.252, BORE + 0.009), (-0.162, BORE + 0.009), (-0.162, 0.0705), (-0.180, 0.0735),
             (-0.245, 0.067), (-0.252, 0.062)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(rs, -0.0145, 0.0145, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.247, 0.0705), (-0.166, 0.0790), (-0.166, 0.0825), (-0.247, 0.0735)],
             -0.0065, 0.0065, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.019, 0.0075, 0.009), (0, 0.0775, -0.205), "Blued", bevel=0.0012)  # slider
barrel.prism([(-0.170, 0.081), (-0.164, 0.081), (-0.164, 0.0855), (-0.170, 0.0855)],
             -0.006, 0.006, "Blued", axis="x", bevel=0.0004, segs=1)  # notch plate
nc = Part("cut")
nc.box((0.0022, 0.004, 0.02), (0, 0.0855, -0.167), "Blued")
barrel.cut(nc)

# --- Receiver -------------------------------------------------------------------------
recv = Part("receiver")
# Hexagonal receiver ring.
hexa = [(0.0172 * math.cos(math.radians(30 + 60 * i)), BORE + 0.0172 * math.sin(math.radians(30 + 60 * i)))
        for i in range(6)]
recv.prism(fillet(hexa, 0.0015, 1), -0.208, -0.168, "Blued", axis="z", bevel=0.0012, segs=1)
# The receiver body: a round tube around the bolt, flattened underneath.
recv.tube(0.0148, 0.0094, -0.170, -0.014, "Blued", segs=24, xf=T(0, BORE, 0))
recv.box((0.025, 0.018, 0.156), (0, BORE - 0.012, -0.092), "Blued", bevel=0.002)
# Rear tang running down onto the wrist.
tang = fillet([(-0.016, BORE - 0.014), (0.004, BORE - 0.014), (0.045, 0.024), (0.05, 0.0275),
               (0.04, 0.0305), (-0.016, 0.034)], 0.003, 2)
recv.prism(tang, -0.0075, 0.0075, "Blued", axis="x", bevel=0.0012, segs=1)
rcut = Part("cut")
# Loading/ejection opening on top, and the bolt-handle slot on the right.
rcut.box((0.0135, 0.02, 0.090), (0, BORE + 0.014, -0.090), "Blued", bevel=0.002, segs=1)
rcut.box((0.02, 0.022, 0.016), (0.012, BORE + 0.006, -0.031), "Blued")
rcut.box((0.02, 0.0085, 0.115), (0.012, BORE + 0.0005, -0.075), "Blued")
recv.cut(rcut)

# Magazine box with its floorplate, and the trigger guard behind it.
mag = Part("magbox")
mag.prism(fillet([(-0.168, -0.022), (-0.074, -0.022), (-0.072, -0.040), (-0.082, -0.0465),
                  (-0.150, -0.0465), (-0.170, -0.034)], [0.004, 0.002, 0.004, 0.006, 0.008, 0.006],
                 3), -0.0145, 0.0145, "Blued", axis="x", bevel=0.0015, segs=2)
mag.box((0.032, 0.003, 0.098), (0, -0.0475, -0.120), "Blued", bevel=0.0012)  # floorplate
mag.box((0.006, 0.006, 0.009), (0, -0.048, -0.081), "Blued", bevel=0.0015)   # its latch
guard_path = catmull([(-0.074, -0.034), (-0.062, -0.046), (-0.040, -0.0485), (-0.018, -0.044),
                      (-0.006, -0.034), (0.006, -0.0315)], 4, closed=False)
mag.sweep(guard_path, section_rect(0.009, 0.003, 0.001), "Blued")
mag.box((0.012, 0.004, 0.08), (0, -0.030, -0.035), "Blued", bevel=0.0012)  # guard tang
mag.cyl(0.003, -0.0005, 0.0012, "Blued", segs=10, xf=T(0, -0.0475, 0.006) @ RX(math.pi / 2))

# --- Bolt (turns about the bore, then slides) -------------------------------------------
HANDLE_Z = -0.034
bolt = Part("turnbolt", origin=(0.0, BORE, HANDLE_Z))
bolt.lathe([(0.0, -0.162), (0.0075, -0.162), (0.0086, -0.159), (0.0086, -0.026),
            (0.0074, -0.022), (0.0, -0.022)], "Steel", segs=20, xf=T(0, BORE, 0))
# Locking lugs at the bolt head.
for sx in (-1, 1):
    bolt.box((0.004, 0.006, 0.016), (sx * 0.0088, BORE, -0.152), "Steel", bevel=0.001)
# Extractor along the right.
bolt.box((0.0025, 0.004, 0.06), (0.0088, BORE + 0.0035, -0.13), "Steel", bevel=0.0008)
# Cocking piece knob at the rear: round and knurled.
bolt.lathe([(0.0, -0.022), (0.0055, -0.022), (0.0055, -0.012), (0.0102, -0.009), (0.0108, -0.002),
            (0.0108, 0.006), (0.0095, 0.011), (0.0, 0.0125)], "Steel", segs=18, xf=T(0, BORE, 0))
for i in range(12):
    a = 2 * math.pi * i / 12
    bolt.box((0.0016, 0.0014, 0.012), (0.0108 * math.cos(a), BORE + 0.0108 * math.sin(a), 0.002),
             "Steel", xf=RZ(a, about=(0.0108 * math.cos(a), BORE + 0.0108 * math.sin(a), 0.002)))
# The straight handle with its ball knob.
knob = (0.061, 0.031, HANDLE_Z)
root = (0.008, BORE, HANDLE_Z)
dx, dy = knob[0] - root[0], knob[1] - root[1]
ln = math.hypot(dx, dy)
ang = math.atan2(dy, dx)
bolt.cyl(0.0034, 0.0, ln - 0.004, "Steel", segs=10,
         xf=T(*root) @ RZ(ang) @ RY(math.pi / 2))
bolt.box((0.008, 0.009, 0.012), (0.009, BORE, HANDLE_Z), "Steel", bevel=0.0015)
bolt.sphere(0.0092, knob, "Steel", segs=18, rings=10)

# --- Trigger ----------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, -0.005, -0.04))
trig.sweep(catmull([(-0.044, -0.020), (-0.0445, -0.030), (-0.042, -0.039), (-0.037, -0.044)], 3,
                   closed=False), section_rect(0.005, 0.0035, 0.0012), "Blued")

join("static", stock, furn, barrel, recv, mag)
for p in (bolt, trig):
    make(p)

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.085, -0.02))
empty("pt_load_port", (0.0, 0.07, -0.09))

report()
export("assets/models/guns/mosin.glb")
