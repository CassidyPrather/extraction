"""TOZ-8M: the Soviet single-shot 5.6 mm (.22 LR) small-bore bolt-action club
and hunting rifle. Writes assets/models/guns/toz8.glb.

Gun space as in src/weapons/layout.rs: origin at the palm on the wrist, -Z down
the bore (bore axis y = 0.040), +Y up, metres. No magazine: the bolt opens
onto the loading trough in the receiver's window, a round is laid in by hand
and pushed into the chamber by the bolt. The bolt is coaxial with the bore;
its object origin sits on the bore axis, so the game turns it about that
axis (the handle lifts from turned down on the right to up) and then slides
it back 0.075 m: the handle's root runs out of the slot at the receiver's
open rear end.

A plain birch sporter stock with a straight wrist, a round receiver with
an 11 mm dovetail on top for a diopter, a slim 640 mm barrel, a hooded
front sight and a tangent leaf with a notch. A club rifle: light, plain,
its bluing rubbed off on the edges.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, arc, catmull, empty, export, fillet, join, main_guard,
                    make, report, section_rect)

main_guard()

BORE = 0.040
MUZZLE = -0.84
RECV0, RECV1 = -0.20, -0.03     # receiver front and rear
BREECH = -0.170                 # the bolt face, closed
HANDLE_Z = -0.060
BOLT_ORIGIN = (0.0, BORE, -0.10)
SIGHT_Y = 0.062                 # the sight line: notch bottom and post tip

# --- Stock --------------------------------------------------------------------------------
# Side outline: forend tip, the barrel channel's edge, a step down under the
# receiver, the wrist, a low comb to the heel; then the toe and the belly
# back to the forend. A straight (English) wrist: no pistol grip.
stock_prof = [
    (-0.536, 0.036), (-0.206, 0.036),           # forend top, either side of the barrel
    (-0.199, 0.029), (-0.037, 0.029),           # under the receiver
    (-0.024, 0.0265), (0.015, 0.0242),          # down behind the receiver onto the wrist
    (0.055, 0.0228), (0.095, 0.0262),           # wrist to the comb
    (0.297, 0.0195),                            # heel
    (0.297, -0.100),                            # toe
    (0.20, -0.0775), (0.10, -0.0515), (0.04, -0.0315), (0.005, -0.0238),  # belly to the wrist
    (-0.025, -0.0192), (-0.10, -0.0148), (-0.21, -0.0135),               # under the action
    (-0.45, -0.0048), (-0.536, -0.0012), (-0.547, 0.008), (-0.547, 0.026),  # forend, its tip
]
stock_prof = fillet(stock_prof, [0.006, 0.003, 0.003, 0.004, 0.008, 0.02, 0.02, 0.02, 0.005,
                                 0.007, 0.05, 0.05, 0.03, 0.02, 0.02, 0.03, 0.05, 0.05, 0.008,
                                 0.004, 0.006], 4)


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


# Half widths along the stock: a slim forend, the action, the wrist with
# its slight swell under the palm, and the butt.
WIDTH = [(-0.55, 0.0122), (-0.45, 0.0138), (-0.25, 0.0158), (-0.20, 0.0164), (-0.04, 0.0166),
         (-0.016, 0.0150), (0.012, 0.0158), (0.036, 0.0150), (0.085, 0.0165), (0.30, 0.0198)]


def stock_wt(z, yc, h):
    k = lerp_table(z, WIDTH)
    belly = k - (0.002 if z > 0.06 else 0.0008)
    return (belly, k * (0.92 if z < -0.03 else 0.86))


def stock_power(z):
    # Squarish along the action and forend, oval through the wrist and butt.
    if z < -0.05:
        return 3.2
    return 2.4 if z < 0.07 else 2.6


stock = Part("stock")
stock.stock(stock_prof, -0.5465, 0.2968, "Wood", stock_wt, stations=110, segs=24,
            power=stock_power)

furn = Part("furniture")
# Steel butt plate with its two screws.
bp = fillet([(0.2955, 0.0205), (0.302, 0.0205), (0.302, -0.1015), (0.2955, -0.1015)],
            [0.002, 0.003, 0.004, 0.002], 2)
furn.prism(bp, -0.0184, 0.0184, "Blued", axis="x", bevel=0.0012, segs=1)
for y in (0.006, -0.085):
    furn.cyl(0.0032, 0.3015, 0.3027, "Steel", segs=12, xf=T(0, y, 0), chamfer=0.0004)
    furn.box((0.0052, 0.0007, 0.0008), (0.0, y, 0.3027), "Dark")

# Sling swivels: a stud under the forend and under the butt, a loop in each.
SWIVELS = ((-0.43, -0.0054), (0.20, -0.0772))
for z, yb in SWIVELS:
    furn.cyl(0.0032, yb + 0.003, yb - 0.0075, "Blued", segs=10,
             xf=T(0, 0, z) @ RX(-math.pi / 2))
    furn.cyl(0.0052, yb + 0.002, yb - 0.0012, "Blued", segs=12,
             xf=T(0, 0, z) @ RX(-math.pi / 2))  # the escutcheon washer
    cy = yb - 0.0075 - 0.0083
    loop = [(z + 0.0085 * math.cos(a), cy + 0.0085 * math.sin(a))
            for a in [2 * math.pi * i / 16 for i in range(16)]]
    furn.sweep(loop, section_rect(0.0034, 0.0026, 0.0009), "Blued", closed=True)

# Trigger guard: a stamped loop on a plate let into the belly, two screws.
furn.prism(fillet([(-0.103, -0.0128), (-0.103, -0.0163), (-0.006, -0.0232), (-0.006, -0.0200)],
                  0.0012, 2), -0.0055, 0.0055, "Blued", axis="x", bevel=0.0008, segs=1)
guard_path = catmull([(-0.090, -0.0135), (-0.084, -0.026), (-0.069, -0.037), (-0.049, -0.0408),
                      (-0.029, -0.0368), (-0.017, -0.0285), (-0.012, -0.0205)], 4, closed=False)
furn.sweep(guard_path, section_rect(0.0085, 0.0028, 0.0010), "Blued")
for z, y in ((-0.097, -0.0174), (-0.010, -0.0237)):
    furn.cyl(0.0028, y + 0.0012, y - 0.0007, "Steel", segs=12,
             xf=T(0, 0, z) @ RX(-math.pi / 2), chamfer=0.0003)

# --- Barrel and sights ----------------------------------------------------------------------
barrel = Part("barrel")
barrel.lathe([(0.0029, MUZZLE + 0.0022), (0.0071, MUZZLE), (0.0085, MUZZLE + 0.0012),
              (0.0085, -0.56), (0.0094, -0.36), (0.0104, -0.25), (0.0105, -0.21),
              (0.0105, RECV0 + 0.0005), (0.0, RECV0 + 0.0005)], "Blued", segs=20,
             xf=T(0, BORE, 0))

# The bore, dark in the crowned muzzle.
barrel.cyl(0.0028, MUZZLE + 0.0021, MUZZLE + 0.006, "Black", segs=12, xf=T(0, BORE, 0))
# Front sight: a ring on the barrel, a block, the post and its hood.
barrel.cyl(0.0095, -0.8365, -0.8115, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.0008)
barrel.box((0.010, 0.0082, 0.022), (0.0, 0.0498, -0.824), "Blued", bevel=0.0008, segs=1)
barrel.prism([(-0.8276, 0.053), (-0.8204, 0.053), (-0.8228, SIGHT_Y), (-0.8252, SIGHT_Y)],
             -0.0009, 0.0009, "Blued", axis="x")
HOOD_Y = 0.0565
hood = ([(0.0085, 0.0442)] + arc(0.0, HOOD_Y, 0.0085, 0, 180, 14) + [(-0.0085, 0.0442),
        (-0.0075, 0.0442)] + arc(0.0, HOOD_Y, 0.0075, 180, 0, 14) + [(0.0075, 0.0442)])
barrel.prism(hood, -0.8335, -0.8145, "Blued", axis="z")

# Rear sight: a base on the barrel, the tangent leaf hinged at its front
# lying back, the slider on it and the notch plate at its rear end.
rs = fillet([(-0.302, 0.0462), (-0.250, 0.0462), (-0.250, 0.0535), (-0.256, 0.0556),
             (-0.296, 0.0533), (-0.302, 0.0510)], [0.0012, 0.0012, 0.0015, 0.003, 0.003, 0.002],
            2)
barrel.prism(rs, -0.0065, 0.0065, "Blued", axis="x", bevel=0.0008, segs=1)
leaf = fillet([(-0.2975, 0.0538), (-0.2545, 0.0562), (-0.2545, 0.0590), (-0.2975, 0.0566)],
              0.0006, 1)
barrel.prism(leaf, -0.0045, 0.0045, "Blued", axis="x", bevel=0.0004, segs=1)
# The hinge pin through the base's front ears.
barrel.cyl(0.0019, -0.0072, 0.0072, "Blued", segs=10,
           xf=T(0, 0.0548, -0.2955) @ RY(math.pi / 2))
slope = math.atan2(0.0024, 0.043)
barrel.box((0.011, 0.0046, 0.0062), (0.0, 0.0577, -0.277), "Blued", bevel=0.0007, segs=1,
           xf=RX(-slope, about=(0.0, 0.0577, -0.277)))
# Range marks along the leaf's top.
for k in range(5):
    z = -0.292 + k * 0.0062
    if abs(z + 0.277) < 0.006:
        continue  # under the slider
    y = 0.0566 + (z + 0.2975) / 0.043 * 0.0024
    barrel.box((0.0045, 0.0003, 0.0006), (-0.0022, y + 0.00012, z), "Worn",
               xf=RX(-slope, about=(0, y, z)))
plate = fillet([(-0.2585, 0.0560), (-0.2525, 0.0560), (-0.2525, 0.0640), (-0.2585, 0.0640)],
               0.0006, 1)
barrel.prism(plate, -0.0060, 0.0060, "Blued", axis="x", bevel=0.0004, segs=1)
nc = Part("cut")
# (Its floor half a millimetre under the line: `capture::sightline`.)
nc.box((0.0022, 0.004, 0.02), (0.0, SIGHT_Y + 0.0015, -0.2555), "Blued")
barrel.cut(nc)

# --- Receiver -----------------------------------------------------------------------------
recv = Part("receiver")
# A round tube round the bolt, a solid ring at the front the barrel screws into.
recv.lathe([(0.0032, RECV0), (0.0127, RECV0), (0.0135, RECV0 + 0.0008), (0.0135, RECV1 - 0.0008),
            (0.0127, RECV1), (0.0095, RECV1), (0.0095, BREECH), (0.0032, BREECH)], "Blued",
           segs=24, xf=T(0, BORE, 0), closed=True)
# The 11 mm dovetail along the top (wider at the top, undercut below),
# interrupted by the loading window.
dove = [(-0.0045, BORE + 0.0100), (0.0045, BORE + 0.0100), (0.0045, BORE + 0.0128),
        (0.0057, BORE + 0.0165), (-0.0057, BORE + 0.0165), (-0.0045, BORE + 0.0128)]
recv.prism(dove, RECV0 + 0.0012, -0.0705, "Blued", axis="z")
rcut = Part("cut")
# The loading window over the chamber, open on the top and the right down
# to the bore line.
rcut.box((0.021, 0.026, 0.068), (0.0045, BORE + 0.013, -0.134), "Blued", bevel=0.002, segs=1)
# The notch the handle's root turns down into, and the slot along the top
# it slides back out of (open at the rear).
rcut.box((0.025, 0.035, 0.015), (0.0125, BORE + 0.011, HANDLE_Z), "Blued")
rcut.box((0.012, 0.022, 0.050), (0.0005, BORE + 0.015, -0.0425), "Blued")
recv.cut(rcut)
# The loading trough: the polished floor of the window the round is laid on
# (its own Part: the exact solver spoils a mesh with open pieces in it).
trough = Part("trough")
trough.lathe([(0.0091, -0.168), (0.0099, -0.168), (0.0099, -0.101), (0.0091, -0.101)], "Worn",
           segs=11, xf=T(0, BORE, 0), closed=True, start=215.0, sweep=110.0)
# The feed lip at the chamber mouth, under where the round's nose goes in.
trough.prism(fillet([(-0.1695, BORE - 0.0094), (-0.1600, BORE - 0.0094), (-0.1695, BORE - 0.0072)],
                  0.0004, 1), -0.0035, 0.0035, "Worn", axis="x")

# --- Bolt (turns about the bore, then slides back) ------------------------------------------
bolt = Part("turnbolt", origin=BOLT_ORIGIN)
bolt.lathe([(0.0, BREECH), (0.0074, BREECH), (0.0085, BREECH + 0.0011), (0.0085, -0.043),
            (0.0080, -0.0415), (0.0, -0.0415)], "Steel", segs=20, xf=T(0, BORE, 0))
# The extractor along the right of the head, and its claw over the face.
bolt.box((0.0022, 0.0034, 0.034), (0.0083, BORE + 0.0022, BREECH + 0.019), "Steel",
         bevel=0.0006, segs=1)
# Cocking piece: the sleeve and the round knurled knob you pull to cock.
# Its own node: it stands back when cocked (`weapons::visuals`).
cock = Part("cocking_piece", origin=(0.0, BORE, -0.0425))
cock.lathe([(0.0, -0.0425), (0.0068, -0.0425), (0.0068, -0.029), (0.0084, -0.027),
            (0.0086, -0.019), (0.0086, -0.012), (0.0075, -0.0085), (0.0, -0.0075)], "Steel",
           segs=18, xf=T(0, BORE, 0))
for i in range(12):
    a = 2 * math.pi * i / 12
    c = (0.0086 * math.cos(a), BORE + 0.0086 * math.sin(a), -0.0175)
    cock.box((0.0014, 0.0012, 0.0085), c, "Steel", xf=RZ(a, about=c))
# The straight handle, turned down to the right, its ball knob, and the
# lug at its root that bears in the receiver's notch.
KNOB = (0.050, 0.035, HANDLE_Z)
root = (0.0075, BORE, HANDLE_Z)
dx, dy = KNOB[0] - root[0], KNOB[1] - root[1]
ang = math.atan2(dy, dx)
bolt.cyl(0.0031, 0.0, math.hypot(dx, dy) - 0.004, "Steel", segs=10,
         xf=T(*root) @ RZ(ang) @ RY(math.pi / 2))
bolt.box((0.0062, 0.0074, 0.0105), (0.0098, BORE - 0.0003, HANDLE_Z), "Steel", bevel=0.0012,
         xf=RZ(ang, about=(0.0, BORE, HANDLE_Z)))
bolt.sphere(0.0074, KNOB, "Steel", segs=16, rings=10)

# --- Trigger ------------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, -0.013, -0.052))
trig.sweep(catmull([(-0.0515, -0.0125), (-0.0518, -0.0225), (-0.0492, -0.0310),
                    (-0.0440, -0.0362)], 3, closed=False),
           section_rect(0.0048, 0.0032, 0.0011), "Blued")

join("static", stock, furn, barrel, recv, trough, worn={"amount": 0.55})
for p in (bolt, cock, trig):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, SIGHT_Y, -0.03))
empty("pt_port", (0.0, 0.050, -0.150))
empty("pt_load_port", (0.0, 0.050, -0.150))
empty("pt_support", (0.0, 0.0, -0.40))
empty("pt_handle", KNOB)
for name, (z, yb) in zip(("pt_sling_front", "pt_sling_rear"), SWIVELS):
    empty(name, (0.0, yb - 0.0075 - 0.0083, z))
report()
export("assets/models/guns/toz8.glb")
