"""Baikal MP-133 (IZh-81 family) pump-action 12 gauge, birch stock and
forend. Writes assets/models/guns/mp133.glb.

Gun space as in src/weapons/layout.rs: origin at the palm on the stock's
straight wrist, -Z down the bore (bore axis y = 0.045), +Y up, metres.
The milled receiver runs z -0.235..-0.025; the 560 mm barrel to the muzzle
at z -0.795, with a ventilated rib and a front bead; the 4-shell tube
magazine (axis y = 0.016) under it to the cap at z -0.62.

`bolt` is the forend, its twin action bars and the bolt, one object with
its origin at (0, 0.045, -0.13): pumping slides it back +Z by 0.09, the
forend along the tube to just short of the receiver and the bolt back
inside the receiver, clear of the ejection port.

`crossbolt` is the safety through the guard's rear web, its origin on its
axis: pushed through to the right it's safe; pushed left 8 mm to fire, it
bares a red band on the left.
"""

import math
import os
import sys

from mathutils import Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, T, catmull, empty, export, export_part, fillet, join, main_guard, make,
                    report, section_rect)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

BORE = 0.045
TUBE = 0.016             # magazine tube axis
RX_HW = 0.0175           # receiver half width
RX_Z0, RX_Z1 = -0.235, -0.025
RX_Y0, RX_Y1 = -0.006, 0.066
MUZZLE = -0.795
BREECH = -0.158          # barrel's rear face, inside the receiver
TRAVEL = 0.09


def lerp_table(t, table, smooth=True):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            if smooth:
                k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


def ring(z, lo, hi, wb, wt, power, segs=24, cx=0.0):
    """A superellipse cut across a piece of furniture (as `Part.stock`)."""
    yc, h = (lo + hi) / 2, (hi - lo) / 2
    e = 2.0 / power
    out = []
    for k in range(segs):
        th = 2 * math.pi * k / segs
        c, sn = math.cos(th), math.sin(th)
        w = wb + (wt - wb) * (sn + 1) / 2
        out.append(Vector((cx + math.copysign(abs(c) ** e, c) * w,
                           yc + math.copysign(abs(sn) ** e, sn) * h, z)))
    return out


# --- Receiver (milled steel, flat sides, rounded top) ----------------------------------
recv = Part("receiver")
rsec = fillet([(-RX_HW, RX_Y0), (RX_HW, RX_Y0), (RX_HW, RX_Y1), (-RX_HW, RX_Y1)],
              [0.0025, 0.0025, 0.013, 0.013], 5)
recv.prism(rsec, RX_Z0, RX_Z1, "Blued", axis="z", bevel=0.0018, segs=2)
rc = Part("cut")
# Ejection port, right side; the bolt shows in it when closed.
rc.box((0.024, 0.026, 0.06), (0.018, 0.050, -0.12), "Black", bevel=0.002, segs=1)
# Loading port underneath, the shell lifter recessed in it.
rc.box((0.022, 0.046, 0.072), (0.0, 0.003, -0.13), "Black", bevel=0.002, segs=1)
# Stampings on the left: the model and the chambering.
LEFT = T(-RX_HW + 0.0004, 0, 0) @ RY(-math.pi / 2)
text(rc, "МР-133", 0.0055, "Blued", LEFT @ T(-0.15, 0.040, 0.0), depth=0.0008)
text(rc, "12x76", 0.0034, "Blued", LEFT @ T(-0.15, 0.030, 0.0), depth=0.0008)
recv.cut(rc, paint=True)

misc = Part("static_misc")
# Trigger-group pins through the receiver, both sides.
for sx in (-1, 1):
    for z, y in ((-0.083, 0.004), (-0.034, 0.004)):
        misc.sphere(0.0026, (sx * RX_HW, y, z), "Steel", segs=12, rings=6, scale=(0.4, 1, 1))
# Shell lifter (carrier): a trough under the bolt, seen up the loading port.
trough = ([(0.0105 * math.cos(math.radians(a)), 0.024 + 0.0105 * math.sin(math.radians(a)))
           for a in [200 + 14 * i for i in range(11)]]
          + [(0.009 * math.cos(math.radians(a)), 0.024 + 0.009 * math.sin(math.radians(a)))
             for a in [340 - 14 * i for i in range(11)]])
misc.prism(trough, -0.163, -0.099, "Steel", axis="z")
misc.box((0.006, 0.006, 0.008), (0.0, 0.017, -0.097), "Steel", bevel=0.001)  # lifter arm

# --- Trigger housing, guard, safety, action release ---------------------------------------
# The guard bow under the receiver.
guard_path = catmull([(-0.090, -0.003), (-0.0915, -0.018), (-0.085, -0.031), (-0.068, -0.038),
                      (-0.042, -0.0385), (-0.024, -0.034), (-0.012, -0.022), (-0.009, -0.003)],
                     4, closed=False)
misc.sweep(guard_path, section_rect(0.0105, 0.0052, 0.0016), "Blued")
# Its rear web, where the crossbolt safety goes through.
web = fillet([(-0.032, -0.003), (-0.009, -0.003), (-0.011, -0.022), (-0.019, -0.026),
              (-0.030, -0.012)], [0.0, 0.0, 0.004, 0.004, 0.006], 3)
misc.prism(web, -0.0055, 0.0055, "Blued", axis="x", bevel=0.001, segs=1)
# Crossbolt safety, pushed through to the right (safe): a part of its own
# (`crossbolt`), pushed left to fire, which bares its red band on the left.
CROSSBOLT = (0.0, -0.0125, -0.0215)
crossbolt = Part("crossbolt", origin=CROSSBOLT)
crossbolt.cyl(0.0042, -0.0095, 0.0115, "Steel", segs=16,
              xf=T(0, CROSSBOLT[1], CROSSBOLT[2]) @ RY(math.pi / 2), chamfer=0.0008)
crossbolt.cyl(0.0043, -0.0135, -0.0095, "Red", segs=16,
              xf=T(0, CROSSBOLT[1], CROSSBOLT[2]) @ RY(math.pi / 2))
# Action (slide) release lever, front left inside the guard.
rel = fillet([(-0.0745, -0.003), (-0.0675, -0.003), (-0.0685, -0.016), (-0.072, -0.0205),
              (-0.0765, -0.0185)], [0.0, 0.0, 0.0015, 0.0015, 0.0015], 2)
misc.prism(rel, -0.0047, -0.0017, "Blued", axis="x", bevel=0.0006, segs=1)

# --- Barrel with ventilated rib --------------------------------------------------------------
# Its origin at the breech: a cut barrel (and the tube under it) is
# drawn as this scaled back along the bore toward it.
barrel = Part("barrel", origin=(0.0, BORE, BREECH))
barrel.lathe([(0.0093, BREECH), (0.0128, BREECH), (0.0128, -0.262), (0.0119, -0.30),
              (0.0110, -0.45), (0.0106, MUZZLE + 0.004), (0.0101, MUZZLE), (0.0093, MUZZLE)],
             "Blued", segs=24, xf=T(0, BORE, 0), closed=True)
# The rib: a flat strip on posts, its top level with the receiver's.
# The bead's top on the eye line just over the receiver (`pt_sight`,
# `capture::sightline`): the rib's front was 2 mm lower.
RIB_R, RIB_F = 0.0635, 0.0632
barrel.prism([(RX_Z0 + 0.002, RIB_R - 0.003), (MUZZLE + 0.002, RIB_F - 0.003),
              (MUZZLE + 0.002, RIB_F), (RX_Z0 + 0.002, RIB_R)],
             -0.004, 0.004, "Blued", axis="x", bevel=0.0007, segs=1)


def barrel_r(z):
    return lerp_table(z, [(MUZZLE, 0.0106), (-0.45, 0.0110), (-0.30, 0.0119), (-0.262, 0.0128)],
                      smooth=False)


def rib_y(z):
    return RIB_R + (z - RX_Z0) / (MUZZLE - RX_Z0) * (RIB_F - RIB_R)


for i in range(10):
    z = -0.262 - i * 0.0575
    top = rib_y(z) - 0.0025
    bot = BORE + barrel_r(z) - 0.0015
    barrel.box((0.0042, top - bot, 0.007), (0.0, (top + bot) / 2, z), "Blued", bevel=0.0008,
               segs=1)
barrel.box((0.0045, 0.006, 0.008), (0.0, 0.0575, MUZZLE + 0.006), "Blued", bevel=0.0008, segs=1)
# Front bead.
barrel.sphere(0.0021, (0.0, RIB_F + 0.0012, MUZZLE + 0.006), "Brass", segs=12, rings=6)
# Barrel lug: a ring round the tube just behind the magazine cap.
LUG0, LUG1 = -0.616, -0.596
barrel.lathe([(0.0, LUG0), (0.0128, LUG0), (0.0138, LUG0 + 0.002), (0.0138, LUG1 - 0.002),
              (0.0128, LUG1), (0.0, LUG1)], "Blued", segs=24, xf=T(0, TUBE, 0))
barrel.prism(fillet([(LUG0 + 0.001, 0.022), (LUG1 - 0.001, 0.022), (LUG1 + 0.004, BORE - 0.004),
                     (LUG0 - 0.004, BORE - 0.004)], [0.001, 0.001, 0.002, 0.002], 2),
             -0.0075, 0.0075, "Blued", axis="x", bevel=0.001, segs=1)

# --- Magazine tube and cap ------------------------------------------------------------------
tube = Part("tube")
tube.cyl(0.0118, RX_Z0 + 0.006, LUG0 - 0.002, "Blued", segs=20, xf=T(0, TUBE, 0))
CAP0, CAP1 = -0.616, -0.650
tube.lathe([(0.0, CAP0), (0.0130, CAP0), (0.0135, CAP0 - 0.0015), (0.0135, -0.627),
            (0.0128, -0.6285), (0.0135, -0.630), (0.0135, -0.641), (0.0118, -0.647),
            (0.008, -0.6495), (0.0, CAP1)], "Blued", segs=24, xf=T(0, TUBE, 0))
# Sling swivel under the cap.
SW_F = -0.636
tube.box((0.006, 0.006, 0.01), (0.0, TUBE - 0.0135, SW_F), "Steel", bevel=0.0012)
loop = [(SW_F + 0.0095 * math.cos(a), TUBE - 0.0165 + 0.0095 * math.sin(a))
        for a in [math.pi + math.pi * i / 10 for i in range(11)]]
tube.sweep(loop, section_rect(0.0028, 0.0028, 0.0008), "Steel")

# --- Buttstock: straight wrist, birch, rubber pad -------------------------------------------
TOP = catmull([(-0.026, 0.0645), (-0.012, 0.0615), (0.015, 0.050), (0.06, 0.037), (0.12, 0.030),
               (0.20, 0.0195), (0.31, 0.0085)], 4, closed=False)
BOT = catmull([(-0.026, -0.006), (-0.006, -0.0075), (0.025, -0.015), (0.07, -0.030),
               (0.20, -0.082), (0.31, -0.127)], 4, closed=False)
WIDTH = [(-0.026, 0.0179), (-0.006, 0.0172), (0.02, 0.0158), (0.06, 0.0162), (0.13, 0.0192),
         (0.31, 0.0205)]
POWER = [(-0.026, 3.6), (0.0, 3.0), (0.03, 2.3), (0.13, 2.5), (0.31, 2.9)]


def line_y(pts, z):
    return lerp_table(z, pts, smooth=False)


stock = Part("stock")
zs = [-0.026 + (0.31 + 0.026) * (i / 44) ** 1.15 for i in range(45)]
rings = []
for z in zs:
    w = lerp_table(z, WIDTH)
    rings.append(ring(z, line_y(BOT, z), line_y(TOP, z), w, w * 0.86, lerp_table(z, POWER)))
stock.loft(rings, "Wood")
# The rubber recoil pad, a little proud of the wood, rounded at its heel.
pad = Part("pad")
w_end = lerp_table(0.31, WIDTH)
lo_e, hi_e, p_e = line_y(BOT, 0.31), line_y(TOP, 0.31), lerp_table(0.31, POWER)
prings = []
for z, grow in ((0.3095, 0.0006), (0.326, 0.0006), (0.3305, -0.0012), (0.332, -0.004)):
    prings.append(ring(z, lo_e - grow, hi_e + grow, w_end + grow, w_end * 0.86 + grow, p_e))
pad.loft(prings, "Rubber")
# Rear sling swivel under the butt.
SW_R = 0.25
yb = line_y(BOT, SW_R)
slope = math.atan2(line_y(BOT, SW_R + 0.01) - line_y(BOT, SW_R - 0.01), 0.02)
sw = Part("swivel")
sw.box((0.006, 0.008, 0.012), (0.0, yb - 0.001, SW_R), "Steel", bevel=0.0012)
loop = [(SW_R + 0.0095 * math.cos(a), yb - 0.004 + 0.0095 * math.sin(a))
        for a in [math.pi + math.pi * i / 10 for i in range(11)]]
sw.sweep(loop, section_rect(0.0028, 0.0028, 0.0008), "Steel")

# --- Moving: forend, action bars and bolt ------------------------------------------------
BOLT_O = (0.0, BORE, -0.13)
bolt = Part("bolt", origin=BOLT_O)
# Bolt: the side of it fills the ejection port when closed.
BZ0, BZ1 = BREECH + 0.001, -0.113
bolt.prism(fillet([(-0.006, 0.039), (0.014, 0.039), (0.014, 0.061), (-0.006, 0.061)], 0.002, 2),
           BZ0, BZ1, "Steel", axis="z", bevel=0.001, segs=1)
# Extractor claw on the right of the bolt face, and the bolt's lightening flat.
bolt.box((0.003, 0.007, 0.012), (0.0150, 0.047, BZ0 + 0.0115), "Steel", bevel=0.0008, segs=1)
# The action bars, both sides of the tube, from inside the forend back to
# the slide under the bolt that they drive (they stopped 75 mm short:
# `capture::attach`).
for sx in (-1, 1):
    bolt.box((0.0018, 0.008, 0.375), (sx * 0.0131, TUBE, -0.3125), "Blued", bevel=0.0005, segs=1)
bolt.box((0.028, 0.025, 0.02), (0.0, 0.0275, -0.135), "Blued", bevel=0.001, segs=1)
# The forend: ribbed birch round the tube, rounded at the nose.
FZ0, FZ1 = -0.515, -0.330
F_TOP = 0.031


def fe_bot(z):
    return lerp_table(z, [(FZ0, -0.0085), (-0.495, -0.0115), (-0.36, -0.0125), (-0.34, -0.011),
                          (FZ1, -0.0095)])


def fe_w(z):
    return lerp_table(z, [(FZ0, 0.0195), (-0.49, 0.0208), (-0.36, 0.021), (FZ1, 0.0215)])


GROOVES = [-0.474 + 0.0118 * k for k in range(10)]
DEPTH = 0.0017
stations = [(FZ0, 0.84), (FZ0 + 0.002, 0.93), (FZ0 + 0.006, 0.985), (FZ0 + 0.012, 1.0)]
for z in (-0.495, -0.485):
    stations.append((z, 1.0))
for g in GROOVES:
    stations += [(g - 0.0021, 1.0), (g - 0.0012, -1.0), (g + 0.0012, -1.0), (g + 0.0021, 1.0)]
for z in (-0.35, -0.338, FZ1 - 0.002):
    stations.append((z, 1.0))
stations.append((FZ1, 0.96))
frings = []
for z, k in stations:
    lo, hi, w = fe_bot(z), F_TOP, fe_w(z)
    if k < 0:
        lo, w = lo + DEPTH, w - DEPTH
    elif k < 1:
        yc = (lo + hi) / 2
        lo, hi, w = yc + (lo - yc) * k, yc + (hi - yc) * k, w * k
    frings.append(ring(z, lo, hi, w, w * 0.9, 2.7))
bolt.loft(frings, "Wood")

# --- Trigger --------------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, 0.0, -0.047))
trig.sweep(catmull([(-0.0465, 0.001), (-0.0485, -0.010), (-0.0505, -0.019), (-0.0495, -0.026),
                    (-0.0455, -0.031)], 3, closed=False), section_rect(0.0045, 0.0032, 0.0012),
           "Blued")

# --- Build -------------------------------------------------------------------------------------
join("static", recv, misc, worn={})
join("static_barrel", barrel, tube, worn={})
join("static_butt", stock, pad, sw, worn={})
make(bolt, worn={})
make(trig, worn={})
make(crossbolt, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.0665, -0.03))
empty("pt_port", (0.022, 0.05, -0.12))
empty("pt_load_port", (0.0, 0.0, -0.13))
empty("pt_support", (0.0, 0.012, -0.42))
empty("pt_handle", (0.0, 0.012, -0.42))
empty("pt_sling_front", (0.0, TUBE - 0.026, SW_F))
empty("pt_sling_rear", (0.0, yb - 0.0135, SW_R))
report()
export("assets/models/guns/mp133.glb")
export_part("static_butt", "assets/models/items/stock_mp133.glb")
