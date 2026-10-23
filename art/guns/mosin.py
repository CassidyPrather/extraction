"""Mosin-Nagant 91/30: 7.62x54R bolt-action rifle, 5-round internal
magazine. Writes assets/models/guns/mosin.glb.

Gun space as in src/weapons/layout.rs (`mosin()`): origin at the palm on the
wrist, -Z down the bore (bore axis y = 0.045), +Y up, metres. The bolt is
coaxial with the bore; its object origin sits on the bore axis so the game
turns it about that axis and slides it back.

`bayonet` is the 91/30's socket bayonet, fixed as the rifle was carried:
a socket round the barrel behind the front sight, a cranked neck, and a
cruciform spike under the barrel, its point at the empty `pt_bayonet_tip`.
It's its own node so the game can leave it off.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, export_part, fillet, join,
                    main_guard, make, report, section_rect, stock_surface)
from _kit import text  # noqa: E402

main_guard()

BORE = 0.045
MUZZLE = -0.98

# --- Stock ------------------------------------------------------------------------
stock_prof = [
    (-0.866, 0.037), (-0.20, 0.037),          # forend top (barrel channel edge)
    (-0.19, 0.031), (-0.012, 0.031),          # under the receiver
    (0.004, 0.030), (0.035, 0.0215),          # wrist top, down to the comb
    (0.326, 0.0145),                          # heel
    (0.326, -0.107),                          # toe
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


def screw(p, cutter, r, pos, xf, turn=0.6, mat="Blued", segs=12):
    """A domed slotted screw head of radius r at `pos`, its axis +Y turned
    by `xf`, its rim sunk 0.6 mm; `cutter` gets the slot, turned `turn`."""
    h = r * 0.55
    prof = [(0.0, -0.0006), (r, -0.0006)]
    prof += [(r * math.cos(math.radians(d)), h * math.sin(math.radians(d)))
             for d in (0, 30, 55, 75)]
    prof.append((0.0, h))
    p.lathe(prof, mat, segs=segs, xf=T(*pos) @ xf @ RX(-math.pi / 2))
    cutter.box((r * 2.4, h * 1.4, r * 0.32), (0.0, h, 0.0), mat, xf=T(*pos) @ xf @ RY(turn))


WIDTH = [(-0.9, 0.0155), (-0.6, 0.0185), (-0.25, 0.0205), (-0.19, 0.022), (-0.06, 0.022),
         (-0.02, 0.0175), (0.03, 0.0165), (0.09, 0.0195), (0.33, 0.0215)]


def stock_w(z, y):
    k = lerp_table(z, WIDTH)
    # Round the belly of the butt a little narrower.
    if z > 0.05 and y < -0.06:
        k -= (-0.06 - y) * 0.06
    return (-k, k)


def stock_wt(z, yc, h):
    k = lerp_table(z, WIDTH)
    # The butt's belly a little narrower than its comb; the forend's top
    # as wide as the channel the barrel lies in.
    belly = k - (0.003 if z > 0.06 else 0.0)
    return (belly, k * (0.96 if z < -0.19 else 0.9))


def stock_power(z):
    # Square-shouldered under the barrel and receiver, oval through the
    # wrist and the butt.
    return 3.6 if z < -0.03 else 2.5


# The stock is made in three, for the vice (`weapons::gunsmith`): the
# forend goes with the barrel (cut, it's cut with it), the middle round the
# receiver and the wrist stays, and the butt comes off at its screw.
FORE_Z = -0.25
BUTT_Z = 0.07


def stock_piece(name, lo, hi):
    piece = Part(name)
    piece.stock(stock_prof, -0.8675, 0.3285, "Wood", stock_wt, stations=120, segs=24,
                power=stock_power)
    sc = Part("cut")
    # Sling slots through the butt and the forend.
    sc.prism(fillet([(0.15, -0.037), (0.19, -0.044), (0.19, -0.052), (0.15, -0.045)], 0.004, 2),
             -0.04, 0.04, "Wood", axis="x")
    sc.prism(fillet([(-0.66, -0.006), (-0.62, -0.006), (-0.62, -0.012), (-0.66, -0.012)], 0.003,
                    2), -0.04, 0.04, "Wood", axis="x")
    # What isn't this piece.
    if lo is not None:
        sc.box((0.2, 0.4, 1.0), (0.0, 0.0, lo - 0.5), "Wood")
    if hi is not None:
        sc.box((0.2, 0.4, 1.0), (0.0, 0.0, hi + 0.5), "Wood")
    piece.cut(sc)
    return piece


stock_fore = stock_piece("stock_fore", None, FORE_Z)
stock = stock_piece("stock", FORE_Z, BUTT_Z)
stock_butt = stock_piece("stock_butt", BUTT_Z, None)
surf = stock_surface(stock_prof, stock_wt, stock_power)

# Steel butt plate.
butt_furn = Part("butt_furniture")
bp = fillet([(0.322, 0.0175), (0.3335, 0.0145), (0.3335, -0.107), (0.3275, -0.1095),
             (0.322, -0.105)], [0.0, 0.002, 0.003, 0.002, 0.0], 2)
butt_furn.prism(bp, -0.0215, 0.0215, "Blued", axis="x", bevel=0.0015, segs=1)
furn = Part("furniture")
screws = Part("screws")
slots = Part("cut")
butt_screws = Part("butt_screws")
butt_slots = Part("cut")
# Its two screws, into the heel and the toe.
for y in (0.002, -0.088):
    screw(butt_screws, butt_slots, 0.0036, (0.0, y, 0.3335), RX(math.pi / 2), turn=0.3 + y * 9)

# Upper handguard over the barrel.
hg_sec = fillet([(-0.0145, 0.0365), (0.0145, 0.0365), (0.0145, 0.052), (0.0, 0.0615),
                 (-0.0145, 0.052)], [0.0042, 0.0042, 0.006, 0.011, 0.006], 6)
furn.prism(hg_sec, -0.47, -0.245, "Wood", axis="z", bevel=0.004, segs=3)

# Barrel bands, with sling swivels on the left.
for z, top in ((-0.70, 0.0585), (-0.49, 0.0655)):
    band = fillet([(-0.0215, -0.025), (0.0215, -0.025), (0.0215, top - 0.012), (0.0, top),
                   (-0.0215, top - 0.012)], [0.012, 0.012, 0.009, 0.014, 0.009], 6)
    furn.prism(band, z - 0.007, z + 0.007, "Blued", axis="z", bevel=0.0012, segs=1)
    loop = [(z + 0.012 * math.cos(a), -0.006 + 0.012 * math.sin(a))
            for a in [math.pi * 0.5 + math.pi * i / 14 for i in range(15)]]
    furn.sweep(loop, section_rect(0.003, 0.003, 0.0008, 3), "Blued", xf=T(-0.0235, 0, 0))
    # The band spring on the right that holds it, let into the wood
    # behind the band, with the stud that catches the band's edge.
    x = surf(z + 0.016, 0.0095)
    furn.prism(fillet([(z + 0.0068, 0.0071), (z + 0.03, 0.0071), (z + 0.03, 0.0119),
                       (z + 0.0068, 0.0119)], [0.0005, 0.0022, 0.0022, 0.0005], 3),
               x - 0.0008, x + 0.0007, "Steel", axis="x", bevel=0.0003, segs=1)
    furn.cyl(0.0016, x, x + 0.0019, "Steel", segs=12, chamfer=0.0005,
             xf=T(0, 0.0095, z + 0.0105) @ RY(math.pi / 2))
# Nose cap.
nose = fillet([(-0.0168, -0.013), (0.0168, -0.013), (0.0168, 0.039), (-0.0168, 0.039)],
              [0.008, 0.008, 0.004, 0.004], 5)
furn.prism(nose, -0.874, -0.858, "Blued", axis="z", bevel=0.0012, segs=1)

# --- Barrel and sights ---------------------------------------------------------------------
# Its origin at the breech face, on the bore: a cut barrel is drawn as
# this scaled back along the bore toward it.
barrel = Part("barrel", origin=(0.0, BORE, -0.208))
barrel.lathe([(0.0042, MUZZLE + 0.002), (0.0095, MUZZLE), (0.0105, MUZZLE + 0.003),
              (0.0105, -0.60), (0.0118, -0.45), (0.0128, -0.26), (0.0138, -0.245),
              (0.0145, -0.205), (0.0, -0.205)], "Blued", segs=28, xf=T(0, BORE, 0))
# Front sight: a ring base, the post and its hood.
barrel.cyl(0.0122, -0.972, -0.952, "Blued", segs=32, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.prism(fillet([(-0.968, 0.054), (-0.956, 0.054), (-0.958, 0.0805), (-0.964, 0.0805)],
                    0.001, 1), -0.0012, 0.0012, "Blued", axis="x", bevel=0.0003, segs=1)
barrel.tube(0.0118, 0.0104, -0.971, -0.951, "Blued", segs=32, xf=T(0, 0.0645, 0))
# Cleaning rod, ahead of the nose cap.
barrel.cyl(0.0029, -0.94, -0.85, "Steel", segs=12, xf=T(0, 0.026, 0))
barrel.lathe([(0.0, -0.952), (0.0045, -0.952), (0.0045, -0.938), (0.003, -0.936), (0.0, -0.936)],
             "Steel", segs=14, xf=T(0, 0.026, 0))
# Rear sight: the ramp base on the barrel and the tangent leaf.
rs = fillet([(-0.252, BORE + 0.009), (-0.162, BORE + 0.009), (-0.162, 0.0705), (-0.180, 0.0735),
             (-0.245, 0.067), (-0.252, 0.062)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(rs, -0.0145, 0.0145, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.247, 0.0705), (-0.166, 0.0790), (-0.166, 0.0825), (-0.247, 0.0735)],
             -0.0065, 0.0065, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.019, 0.0075, 0.009), (0, 0.0775, -0.205), "Blued", bevel=0.0012)  # slider
# The slider's spring catches, a knurled button each side.
for sx in (-1, 1):
    barrel.cyl(0.0024, 0.0092, 0.0118, "Steel", segs=14, chamfer=0.0005,
               xf=RY(sx * math.pi / 2, about=(0, 0.0775, -0.205)) @ T(0, 0.0775, -0.205))
    for i in range(8):
        a = 2 * math.pi * i / 8
        c = (sx * 0.0108, 0.0775 + 0.0024 * math.cos(a), -0.205 + 0.0024 * math.sin(a))
        barrel.box((0.0018, 0.0006, 0.0006), c, "Steel", xf=RX(a, about=c))
# The leaf's hinge pin across the back of the base, its ends peened.
barrel.cyl(0.0021, -0.0158, 0.0158, "Blued", segs=14, chamfer=0.0006,
           xf=T(0, 0.0655, -0.2465) @ RY(math.pi / 2))
# The base's flat spring under the leaf's front end.
barrel.box((0.008, 0.0016, 0.03), (0, 0.0738, -0.19), "Steel", bevel=0.0005,
           xf=RX(-math.atan2(0.0085, 0.081), about=(0, 0.0738, -0.19)))
# Range marks along the leaf, every other one long.
for k in range(12):
    z = -0.243 + k * 0.0064
    y = 0.0735 + (z + 0.247) / 0.081 * 0.009
    if abs(z + 0.205) < 0.007:
        continue  # under the slider
    w = 0.0075 if k % 2 == 0 else 0.004
    barrel.box((w, 0.0003, 0.0006), (-0.0065 + w / 2 + 0.0004, y + 0.00015, z), "Worn",
               xf=RX(-math.atan2(0.009, 0.081), about=(0, y, z)))
barrel.prism([(-0.170, 0.081), (-0.164, 0.081), (-0.164, 0.0855), (-0.170, 0.0855)],
             -0.006, 0.006, "Blued", axis="x", bevel=0.0004, segs=1)  # notch plate
nc = Part("cut")
nc.box((0.0022, 0.004, 0.02), (0, 0.0855, -0.167), "Blued")
barrel.cut(nc)

# --- Receiver -------------------------------------------------------------------------
recv = Part("receiver")
# Hexagonal receiver ring.
hexa = [(0.0172 * math.cos(math.radians(60 * i)), BORE + 0.0172 * math.sin(math.radians(60 * i)))
        for i in range(6)]
recv.prism(fillet(hexa, 0.0015, 2), -0.208, -0.168, "Blued", axis="z", bevel=0.0012, segs=2)
# The receiver body: a round tube around the bolt, flattened underneath.
recv.tube(0.0148, 0.0094, -0.170, -0.014, "Blued", segs=32, xf=T(0, BORE, 0))
recv.box((0.025, 0.018, 0.156), (0, BORE - 0.012, -0.092), "Blued", bevel=0.002)
# Rear tang running down onto the wrist.
tang = fillet([(-0.016, BORE - 0.014), (0.004, BORE - 0.014), (0.045, 0.024), (0.05, 0.0275),
               (0.04, 0.0305), (-0.016, 0.034)], [0.0014, 0.003, 0.003, 0.003, 0.003, 0.0014], 3)
recv.prism(tang, -0.0075, 0.0075, "Blued", axis="x", bevel=0.0012, segs=2)
# The tang screw through the wrist, and the ejector screw on the left.
screw(screws, slots, 0.0033, (0.0, 0.0311, 0.027), RX(math.atan2(0.0035, 0.056)), turn=1.1)
ey = BORE - 0.005
screw(screws, slots, 0.0028, (-math.sqrt(0.0148 ** 2 - 0.005 ** 2) + 0.0002, ey, -0.052),
      RZ(math.pi / 2), turn=0.4)
rcut = Part("cut")
# Loading/ejection opening on top, and the bolt-handle slot on the right.
rcut.box((0.0135, 0.02, 0.090), (0, BORE + 0.014, -0.090), "Blued", bevel=0.002, segs=1)
rcut.box((0.02, 0.022, 0.016), (0.012, BORE + 0.006, -0.031), "Blued")
rcut.box((0.02, 0.0085, 0.115), (0.012, BORE + 0.0005, -0.075), "Blued")
# Serial number and year, stamped along the left of the receiver.
LEFT = T(-0.0148 + 0.0006, BORE, 0) @ RY(-math.pi / 2)
text(rcut, "ГБ 2817", 0.0034, "Blued", LEFT @ T(-0.115, 0.0, 0.0), depth=0.0012)
text(rcut, "1942", 0.0034, "Blued", LEFT @ T(-0.085, 0.0, 0.0), depth=0.0012)
recv.cut(rcut)

# Magazine box with its floorplate, and the trigger guard behind it.
mag = Part("magbox")
mag.prism(fillet([(-0.168, -0.022), (-0.074, -0.022), (-0.072, -0.040), (-0.082, -0.0465),
                  (-0.150, -0.0465), (-0.170, -0.034)], [0.004, 0.002, 0.004, 0.006, 0.008, 0.006],
                 3), -0.0145, 0.0145, "Blued", axis="x", bevel=0.0015, segs=2)
mag.box((0.032, 0.003, 0.098), (0, -0.0475, -0.120), "Blued", bevel=0.0012)  # floorplate
mag.box((0.006, 0.006, 0.009), (0, -0.048, -0.081), "Blued", bevel=0.0015)   # its latch
# Grip ridges across the latch's thumb face.
for i in range(3):
    mag.box((0.0064, 0.0008, 0.0009), (0, -0.0505, -0.0835 + i * 0.0025), "Blued",
            bevel=0.0003, segs=1)
# The floorplate's hinge pin at the front, its ends showing.
mag.cyl(0.0016, -0.0168, 0.0168, "Steel", segs=12, chamfer=0.0004,
        xf=T(0, -0.0472, -0.1655) @ RY(math.pi / 2))
guard_path = catmull([(-0.074, -0.034), (-0.062, -0.046), (-0.040, -0.0485), (-0.018, -0.044),
                      (-0.006, -0.034), (0.006, -0.0315)], 5, closed=False)
mag.sweep(guard_path, section_rect(0.009, 0.003, 0.001, 3), "Blued")
mag.box((0.012, 0.004, 0.08), (0, -0.030, -0.035), "Blued", bevel=0.0012)  # guard tang
# The guard screws: the rear one up through the tang, the front one
# ahead of the magazine box.
def belly(z):
    """The stock's underside at z (its outline's lowest crossing)."""
    ys = []
    for (za, ya), (zb, yb) in zip(stock_prof, stock_prof[1:] + stock_prof[:1]):
        if (za - z) * (zb - z) <= 0 and za != zb:
            ys.append(ya + (z - za) / (zb - za) * (yb - ya))
    return min(ys)


for z, turn in ((0.014, 0.9), (-0.178, 2.0)):
    screw(screws, slots, 0.0034, (0.0, belly(z) + 0.0004, z), RX(math.pi), turn=turn)

# --- Bolt (turns about the bore, then slides) -------------------------------------------
HANDLE_Z = -0.034
bolt = Part("turnbolt", origin=(0.0, BORE, HANDLE_Z))
# (The groove ahead of the lugs is where the separate bolt head joins the
# body.)
bolt.lathe([(0.0, -0.162), (0.0075, -0.162), (0.0086, -0.159), (0.0086, -0.1405),
            (0.0079, -0.1398), (0.0079, -0.1382), (0.0086, -0.1375), (0.0086, -0.026),
            (0.0074, -0.022), (0.0, -0.022)], "Steel", segs=28, xf=T(0, BORE, 0))
# Locking lugs at the bolt head.
for sx in (-1, 1):
    bolt.box((0.004, 0.006, 0.016), (sx * 0.0088, BORE, -0.152), "Steel", bevel=0.001, segs=2)
# Extractor along the right.
bolt.box((0.0025, 0.004, 0.06), (0.0088, BORE + 0.0035, -0.13), "Steel", bevel=0.0008, segs=2)
# Cocking piece knob at the rear: round and knurled. Its own node: it
# stands back from the bolt when cocked and sits in when not, the tell
# you look for (`weapons::visuals`, `CockingPiece`).
# Its rear face is dished, with the firing pin's end in the middle.
cock = Part("cocking_piece", origin=(0.0, BORE, -0.022))
cock.lathe([(0.0, -0.022), (0.0055, -0.022), (0.0055, -0.012), (0.0102, -0.009), (0.0108, -0.002),
            (0.0108, 0.006), (0.0098, 0.0105), (0.0082, 0.0118), (0.0064, 0.0112),
            (0.0028, 0.0106), (0.0024, 0.0118), (0.0, 0.0121)], "Steel", segs=32,
           xf=T(0, BORE, 0))
for i in range(18):
    a = 2 * math.pi * i / 18
    cock.box((0.0013, 0.0012, 0.012), (0.0108 * math.cos(a), BORE + 0.0108 * math.sin(a), 0.002),
             "Steel", xf=RZ(a, about=(0.0108 * math.cos(a), BORE + 0.0108 * math.sin(a), 0.002)))
# The straight handle with its ball knob.
knob = (0.061, 0.031, HANDLE_Z)
root = (0.008, BORE, HANDLE_Z)
dx, dy = knob[0] - root[0], knob[1] - root[1]
ln = math.hypot(dx, dy)
ang = math.atan2(dy, dx)
# The handle tapers a little out to a collar under the knob.
bolt.lathe([(0.0, 0.0), (0.0036, 0.0), (0.0032, ln - 0.016), (0.0039, ln - 0.0125),
            (0.0039, ln - 0.0105), (0.0034, ln - 0.008), (0.0, ln - 0.004)], "Steel", segs=16,
           xf=T(*root) @ RZ(ang) @ RY(math.pi / 2))
bolt.box((0.008, 0.009, 0.012), (0.009, BORE, HANDLE_Z), "Steel", bevel=0.0015)
bolt.sphere(0.0092, knob, "Steel", segs=24, rings=14)

# --- Trigger ----------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, -0.005, -0.04))
trig.sweep(catmull([(-0.044, -0.020), (-0.0445, -0.030), (-0.042, -0.039), (-0.037, -0.044)], 5,
                   closed=False), section_rect(0.005, 0.0035, 0.0012, 3), "Blued")

# --- Socket bayonet ----------------------------------------------------------------------
SPIKE_Y = BORE - 0.021
SPIKE_Z0, SPIKE_TIP = -0.99, -1.425
bayonet = Part("bayonet", origin=(0.0, BORE, -0.92))
bayonet.tube(0.0128, 0.0108, -0.948, -0.888, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.001)
# The locking ring round the socket's back end.
bayonet.tube(0.0138, 0.0127, -0.9, -0.89, "Blued", segs=20, xf=T(0, BORE, 0))
# The neck: down from the socket's front and forward to the spike.
bayonet.sweep([(-0.93, BORE - 0.011), (-0.948, BORE - 0.015), (-0.962, SPIKE_Y + 0.001),
               (-0.98, SPIKE_Y), (SPIKE_Z0 + 0.002, SPIKE_Y)],
              section_rect(0.0065, 0.006, 0.0015), "Blued")
# The spike: four fluted fins, tapering to the point.
rings = []
n = 14
for i in range(n + 1):
    u = i / n
    z = SPIKE_Z0 + (SPIKE_TIP - SPIKE_Z0) * u
    w = 0.0046 * (1.0 - u) ** 0.85 + 0.0002
    c = w * 0.32
    rings.append([(x, SPIKE_Y + y, z) for x, y in
                  ((w, 0.0), (c, c), (0.0, w), (-c, c), (-w, 0.0), (-c, -c), (0.0, -w),
                   (c, -c))])
bayonet.loft(rings, "Steel")

screws.cut(slots)
butt_screws.cut(butt_slots)
join("static", stock, recv, mag, screws, worn={})
join("static_barrel", barrel, stock_fore, furn, worn={})
join("static_butt", stock_butt, butt_furn, butt_screws, worn={})
for p in (bolt, cock, trig, bayonet):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.085, -0.02))
empty("pt_load_port", (0.0, 0.07, -0.09))
empty("pt_bayonet_tip", (0.0, SPIKE_Y, SPIKE_TIP))

# Where a sling attaches: the slots through the forend and the butt (the 91/30's sling runs through them).
empty("pt_sling_front", (0.0, -0.009, -0.64))
empty("pt_sling_rear", (0.0, -0.045, 0.17))
report()
export("assets/models/guns/mosin.glb")


export_part("static_butt", "assets/models/items/stock_mosin.glb")
export_part("turnbolt", "assets/models/items/bolt_mosin.glb")
