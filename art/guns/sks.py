"""SKS: 7.62x39 semi-automatic carbine, a fixed ten-round magazine loaded
from stripper clips through the open action, a one-piece wooden stock, a
folding blade bayonet. Writes assets/models/guns/sks.glb.

Gun space as in src/weapons/layout.rs (`sks()`): origin at the palm on
the wrist, -Z down the bore (bore axis y = 0.048), +Y up, metres.

- `static`        stock, receiver and its cover, barrel, gas system,
                  sights, magazine box, guard, the bayonet's pivot block
- `bayonet`       the folding blade, folded back under the barrel; origin on
                  its pin, it turns +pi about X to fixed
- `bolt_carrier`  the carrier and bolt with the charging handle on the
                  right; slides back with the bolt (origin on the bore)
- `trigger`
- `selector`      the safety behind the trigger: forward is safe, turned back about
                  its pin it fires
- empties `pt_muzzle`, `pt_sight`, `pt_load_port` (the clip guide, where
  a clip seats over the open action), `pt_bayonet_tip` (the point with
  the bayonet fixed)
"""

import math
import os
import sys

import bmesh

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, export_part, fillet, join, main_guard, make,
                    report, section_rect)
from _kit import text  # noqa: E402

main_guard()

BORE = 0.048
# Laid out against a side photograph scaled to the published 1020 mm
# (`docs/craft/guns/sks/README.md`).
MUZZLE = -0.713
RX0, RX1 = -0.235, 0.035          # receiver, front and rear
FORE = -0.440                     # the forend's front, at the band
FOLDED_TIP = -0.365               # the bayonet's point, folded
RHW = 0.0155                      # receiver half width


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


# --- Stock: one piece, from the nose cap to the butt ---------------------------------
stock_prof = [
    (FORE, 0.031), (-0.240, 0.031),            # forend top, under the handguard
    (-0.236, 0.020), (RX1 + 0.001, 0.020),     # the receiver's bed
    (RX1 + 0.005, 0.031), (0.076, 0.023),      # wrist top, dipping before the comb
    (0.12, 0.031),                             # the comb
    (0.30, 0.027),                             # heel
    (0.30, -0.097),                            # toe
    (0.20, -0.073), (0.09, -0.050), (0.06, -0.030), (0.035, -0.018),  # belly, grip
    (0.0, -0.016), (-0.075, -0.010),           # behind the guard
    (-0.21, -0.010),                           # under the magazine
    (-0.26, 0.004), (FORE + 0.007, 0.004), (FORE, 0.016),
]
stock_prof = fillet(stock_prof, [0.004, 0.002, 0.002, 0.002, 0.004, 0.025, 0.04, 0.0, 0.0, 0.05,
                                 0.03, 0.012, 0.01, 0.01, 0.008, 0.02, 0.02, 0.006, 0.004], 4)
WIDTH = [(-0.45, 0.0145), (-0.3, 0.0175), (-0.25, 0.019), (-0.03, 0.019), (0.0, 0.0165),
         (0.04, 0.016), (0.1, 0.0195), (0.31, 0.0205)]


def stock_w(z, yc, h):
    k = lerp_table(z, WIDTH)
    return (k - (0.0025 if z > 0.07 else 0.0), k * (0.95 if z < -0.03 else 0.84))


def stock_power(z):
    return 3.4 if z < -0.03 else 2.5


# The stock is made in three, for the vice (`weapons::gunsmith`): the
# forend goes with the barrel, the middle round the receiver and the wrist
# stays, and the butt comes off at its screw.
FORE_Z = -0.27
BUTT_Z = 0.07


def stock_piece(name, lo, hi):
    piece = Part(name)
    piece.stock(stock_prof, FORE + 0.0005, 0.2995, "Wood", stock_w, stations=110, segs=32,
                power=stock_power)
    sc = Part("cut")
    # The groove the folded blade lies in, under the forend.
    sc.box((0.007, 0.032, 0.09), (0.0, 0.011, FOLDED_TIP - 0.04), "Wood")
    if lo is not None:
        sc.box((0.2, 0.4, 1.0), (0.0, 0.0, lo - 0.5), "Wood")
    if hi is not None:
        sc.box((0.2, 0.4, 1.0), (0.0, 0.0, hi + 0.5), "Wood")
    piece.cut(sc)
    return piece


stock_fore = stock_piece("stock_fore", None, FORE_Z)
stock = stock_piece("stock", FORE_Z, BUTT_Z)
stock_butt = stock_piece("stock_butt", BUTT_Z, None)

butt_furn = Part("butt_furniture")
# Steel butt plate with its trap door.
bp = fillet([(0.2995, 0.0285), (0.309, 0.0275), (0.309, -0.0985), (0.2995, -0.099)],
            [0.0, 0.002, 0.002, 0.0], 2)
butt_furn.prism(bp, -0.0205, 0.0205, "Blued", axis="x", bevel=0.0015, segs=1)
butt_furn.box((0.024, 0.05, 0.004), (0.0, -0.045, 0.3105), "Blued", bevel=0.0012)
# The trap door's hinge pin and thumb notch, the plate's two slotted screws.
butt_furn.cyl(0.0015, -0.011, 0.011, "Blued", segs=12, xf=T(0, -0.0198, 0.311) @ RY(math.pi / 2),
         chamfer=0.0003)
bpc = Part("cut")
bpc.box((0.01, 0.004, 0.003), (0, -0.07, 0.3122), "Blued", bevel=0.0012, segs=2)
for y in (0.016, -0.087):
    butt_furn.lathe([(0.0, 0.3105), (0.0023, 0.3103), (0.0034, 0.3097), (0.0037, 0.309), (0.0037, 0.308),
                (0.0, 0.308)], "Steel", segs=12, xf=T(0, y, 0))
    bpc.box((0.006, 0.0009, 0.0016), (0, y, 0.3108), "Steel")
butt_furn.cut(bpc)
furn = Part("furniture")
# Upper handguard over the gas tube.
hg = fillet([(-0.0145, 0.056), (0.0145, 0.056), (0.0145, 0.068), (0.0, 0.0755),
             (-0.0145, 0.068)], [0.0042, 0.0042, 0.006, 0.01, 0.006], 6)
furn.prism(hg, FORE - 0.006, -0.280, "Wood", axis="z", bevel=0.004, segs=3)
# The band round the forend's front and the handguard's.
band = fillet([(-0.0185, 0.0), (0.0185, 0.0), (0.0185, 0.0775), (-0.0185, 0.0775)],
              [0.009, 0.009, 0.008, 0.008], 6)
furn.prism(band, FORE - 0.012, FORE, "Blued", axis="z", bevel=0.001, segs=2)
# The band's spring catch on the right, pressed into the wood.
furn.box((0.0024, 0.018, 0.005), (0.0158, 0.016, FORE + 0.0055), "Blued", bevel=0.0006, segs=1)
# Rear sling swivel on the left of the butt; front one on the band.
for z0, y0, xs in ((0.13, -0.062, -0.021), (FORE - 0.006, 0.006, -0.0195)):
    loop = [(z0 + 0.011 * math.cos(a), y0 + 0.011 * math.sin(a))
            for a in [math.pi * 0.5 + math.pi * i / 16 for i in range(17)]]
    (butt_furn if z0 > 0 else furn).sweep(loop, section_rect(0.0028, 0.0028, 0.0008, segs=3), "Blued", xf=T(xs, 0, 0))
# The rear swivel's two studs, from its loop's ends into the wood (the loop
# hung 2 mm off the stock: `capture::attach`).
for y in (-0.051, -0.073):
    butt_furn.cyl(0.0017, -0.0225, -0.012, "Blued", segs=10, xf=T(0, y, 0.13) @ RY(math.pi / 2))

# --- Receiver ----------------------------------------------------------------------
recv = Part("receiver")
rprof = fillet([(RX0, 0.019), (RX1, 0.019), (RX1 + 0.004, 0.056), (RX0 + 0.01, 0.062),
                (RX0, 0.062)], [0.002, 0.003, 0.004, 0.004, 0.002], 2)
recv.prism(rprof, -RHW, RHW, "Blued", axis="x", bevel=0.0012, segs=1)
rc = Part("cut")
# The open top between the rear sight and the receiver cover, where the
# carrier shows and a clip goes in; the charging handle's slot on the right.
rc.box((0.018, 0.03, 0.068), (0.0, 0.066, -0.179), "Blued", bevel=0.002, segs=1)
rc.box((0.02, 0.009, 0.12), (RHW, 0.054, -0.105), "Blued")
# Stampings on the left: the arsenal's star in a triangle, serial, year.
LEFT = T(-RHW + 0.0006, 0, 0) @ RY(-math.pi / 2)
text(rc, "РК 1857", 0.0042, "Blued", LEFT @ T(-0.115, 0.044, 0.0), depth=0.0012)
text(rc, "1954", 0.0042, "Blued", LEFT @ T(-0.115, 0.036, 0.0), depth=0.0012)
recv.cut(rc)
# Receiver cover over the recoil spring, its takedown lever at the back.
cover = fillet([(-0.0145, 0.056), (0.0145, 0.056), (0.0145, 0.066), (0.0, 0.0735),
                (-0.0145, 0.066)], [0.0013, 0.0013, 0.005, 0.009, 0.005], 6)
recv.prism(cover, -0.146, RX1 - 0.002, "Blued", axis="z", bevel=0.0012, segs=2)
# The recoil spring guide's button standing out of the cover's back.
recv.lathe([(0.0, -0.0128), (0.0021, -0.0131), (0.0032, -0.0138), (0.0036, -0.0148),
            (0.0036, -0.02), (0.0, -0.02)], "Steel", segs=16, xf=T(0, 0.064, 0.053))
recv.box((0.006, 0.022, 0.008), (RHW + 0.003, 0.05, 0.029), "Blued", bevel=0.0015)
recv.cyl(0.0045, RHW, RHW + 0.004, "Blued", segs=20, xf=T(0, 0.058, 0.029) @ RY(math.pi / 2),
         chamfer=0.0006)
for k in range(4):
    recv.box((0.0008, 0.0016, 0.0062), (RHW + 0.006, 0.0435 + k * 0.0028, 0.029), "Blued",
             bevel=0.0003, segs=1)
# The clip guide cut in the carrier's face shows when the bolt is back;
# on the receiver, the bolt stop's tab on the left.
recv.box((0.004, 0.012, 0.018), (-RHW - 0.001, 0.036, -0.205), "Blued", bevel=0.001)

# Fixed magazine box under the receiver, hinged at the front, its latch
# just ahead of the guard; the trigger guard behind.
mag = Part("magbox")
# Its front slopes down and back to a short, shallow box.
mag.prism(fillet([(-0.214, -0.010), (-0.141, -0.010), (-0.140, -0.029), (-0.149, -0.034),
                  (-0.207, -0.016)], [0.003, 0.002, 0.004, 0.004, 0.004],
                 6), -0.0145, 0.0145, "Blued", axis="x", bevel=0.0015, segs=2)
mag.cyl(0.003, -0.016, 0.016, "Blued", segs=16, xf=T(0, -0.014, -0.208) @ RY(math.pi / 2),
        chamfer=0.0005)
mag.box((0.012, 0.008, 0.008), (0, -0.018, -0.134), "Blued", bevel=0.0015)
# The latch's thumb ridges, facing the guard.
for k in range(4):
    mag.box((0.0124, 0.0012, 0.0008), (0, -0.0205 + k * 0.0018, -0.1298), "Blued",
            bevel=0.0003, segs=1)
guard = catmull([(-0.078, -0.012), (-0.074, -0.03), (-0.06, -0.042), (-0.035, -0.044),
                 (-0.012, -0.038), (-0.002, -0.026), (0.004, -0.022)], 6, closed=False)
mag.sweep(guard, section_rect(0.0095, 0.003, 0.001, segs=3), "Blued")
mag.box((0.012, 0.005, 0.1425), (0, -0.0135, -0.069), "Blued", bevel=0.0015)
# The trigger group's pins, their heads either side of the housing.
for z in (-0.06, -0.034, -0.014):
    mag.cyl(0.0019, -0.0066, 0.0066, "Steel", segs=12, xf=T(0, -0.0145, z) @ RY(math.pi / 2),
            chamfer=0.0003)
# The safety behind the trigger, pushed forward (safe); a part of its own,
# turned back about its pin to fire (`selector`).
SAFETY_PIN = (0.0, -0.017, -0.012)
safety = Part("selector", origin=SAFETY_PIN)
# Long enough to stand down into the guard behind the trigger on safe, where
# a finger meets it, and to swing up out of the way to fire.
safety.box((0.0062, 0.021, 0.005), (0, -0.0265, -0.012), "Blued", bevel=0.0012,
           xf=RX(0.4, about=SAFETY_PIN))
for k in range(4):
    safety.box((0.0066, 0.0008, 0.0011), (0, -0.0215 - k * 0.0028, -0.0145), "Steel",
               bevel=0.0003, segs=1, xf=RX(0.4, about=SAFETY_PIN))

# --- Barrel, gas system, sights, bayonet ----------------------------------------------
# Its origin at the breech, on the bore: a cut barrel is drawn as this
# scaled back along the bore toward it.
barrel = Part("barrel", origin=(0.0, BORE, -0.206))
barrel.lathe([(0.0044, MUZZLE + 0.002), (0.0072, MUZZLE), (0.008, MUZZLE + 0.004),
              (0.008, -0.55), (0.0095, -0.42), (0.0115, -0.28), (0.0135, -0.24),
              (0.0135, -0.206), (0.0, -0.206)], "Blued", segs=36, xf=T(0, BORE, 0))
# Gas block and the tube running back to the rear sight.
# (Its crown under the eye line: it stood 2.5 mm over it, `capture::sightline`.)
GB = -0.546  # the gas block's front
GY = 0.064   # the gas cylinder's axis, on the barrel
gb = fillet([(GB, 0.040), (GB + 0.033, 0.040), (GB + 0.033, 0.072), (GB + 0.022, 0.0765),
             (GB + 0.006, 0.0765), (GB, 0.071)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(gb, -0.0125, 0.0125, "Blued", axis="x", bevel=0.0015, segs=2)
barrel.cyl(0.0085, GB + 0.018, -0.278, "Blued", segs=32, xf=T(0, GY, 0))
barrel.lathe([(0.0, GB + 0.003), (0.0062, GB + 0.0028), (0.0075, GB + 0.0032), (0.0085, GB + 0.007),
              (0.0085, GB + 0.014), (0.0, GB + 0.014)], "Blued", segs=24,
             xf=T(0, GY, 0))  # gas valve knob
# The gas cylinder's lock ring where the tube meets the block, and the
# cross pins through the gas block and the sight base.
barrel.lathe([(0.0, GB + 0.032), (0.0091, GB + 0.032), (0.0096, GB + 0.0332), (0.0096, GB + 0.0378),
              (0.0091, GB + 0.039), (0.0, GB + 0.039)], "Blued", segs=32, xf=T(0, GY, 0))
for z, y, hw in ((GB + 0.0165, 0.046, 0.0131), (-0.691, 0.04, 0.0098)):
    barrel.cyl(0.0018, -hw, hw, "Steel", segs=12, xf=T(0, y, z) @ RY(math.pi / 2), chamfer=0.0003)
# Front sight: base on the barrel, the post in a ring hood.
# (Moved back 27 mm and down 8 mm to the photograph's: the eye line with it.)
barrel.cyl(0.012, -0.703, -0.678, "Blued", segs=32, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.prism(fillet([(-0.700, 0.058), (-0.682, 0.058), (-0.684, 0.0755), (-0.695, 0.0755)],
                    0.0012, 2), -0.009, 0.009, "Blued", axis="x", bevel=0.0006, segs=1)
barrel.prism(fillet([(-0.694, 0.062), (-0.688, 0.062), (-0.689, 0.078), (-0.693, 0.078)], 0.0008, 1),
             -0.0012, 0.0012, "Blued", axis="x", bevel=0.0003, segs=1)
barrel.tube(0.0115, 0.0101, -0.700, -0.683, "Blued", segs=32, xf=T(0, 0.069, 0))
# The post's windage drum in the base, its index line on the right.
barrel.cyl(0.0034, -0.0094, 0.0094, "Blued", segs=16, xf=T(0, 0.062, -0.6905) @ RY(math.pi / 2),
           chamfer=0.0005)
barrel.box((0.0004, 0.004, 0.0005), (0.0095, 0.062, -0.6905), "Worn")
# Rear sight: the base ahead of the receiver, the tangent leaf and slider,
# the notch plate; range marks on the leaf.
# Drawn where it was, then moved back 22 mm and down 8 mm as a piece.
barrel_part = barrel
barrel = Part("rsight", origin=(0.0, BORE, -0.206))
rs = fillet([(-0.30, 0.058), (-0.255, 0.058), (-0.255, 0.075), (-0.265, 0.079), (-0.295, 0.076),
             (-0.30, 0.07)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(rs, -0.0135, 0.0135, "Blued", axis="x", bevel=0.0012, segs=2)
leaf = Part("leaf")
leaf.prism([(-0.295, 0.0765), (-0.24, 0.0805), (-0.24, 0.0835), (-0.295, 0.0795)],
           -0.0062, 0.0062, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.017, 0.006, 0.008), (0, 0.0795, -0.272), "Blued", bevel=0.001)
# The slider's spring catches either side, and the leaf's hinge pin.
for sx in (-1, 1):
    barrel.cyl(0.0021, sx * 0.0082, sx * 0.0098, "Steel", segs=14,
               xf=T(0, 0.0795, -0.272) @ RY(math.pi / 2), chamfer=0.0004)
    for k in range(3):
        barrel.box((0.0004, 0.0032, 0.0005), (sx * 0.0099, 0.0795, -0.2732 + k * 0.0012),
                   "Steel")
    barrel.cyl(0.0018, sx * 0.0128, sx * 0.0141, "Steel", segs=12,
               xf=T(0, 0.074, -0.2925) @ RY(math.pi / 2), chamfer=0.0003)
notch = [(-0.0062, 0.083), (0.0062, 0.083), (0.0062, 0.0875), (0.001, 0.0875), (0.001, 0.0862),
         (0.0, 0.0855), (-0.001, 0.0862), (-0.001, 0.0875), (-0.0062, 0.0875)]
barrel.prism(fillet(notch, [0.0005, 0.0005, 0.0005, 0.0003, 0.0, 0.0006, 0.0, 0.0003, 0.0005], 2),
             -0.245, -0.239, "Blued", axis="z")
# (Engraved half a millimetre into the leaf's top, their floors painted.)
lfc = Part("cut")
for k in range(8):
    z = -0.29 + k * 0.006
    y = 0.0765 + (z + 0.295) / 0.055 * 0.004 + 0.003
    w = 0.0075 if k % 2 == 0 else 0.004
    lfc.box((w, 0.0014, 0.0006), (-0.0062 + w / 2 + 0.0004, y - 0.0002, z), "Worn")
    # The hundreds of metres against every other mark, read from behind.
    if k % 2 == 0:
        text(lfc, str(k // 2 * 2 + 2), 0.0032, "Worn",
             T(0.0034, y - 0.0009, z) @ RX(-math.pi / 2) @ RZ(math.pi), depth=0.0014)
leaf.cut(lfc, paint=True)
rsight = barrel
barrel = barrel_part


def moved(part, dy, dz):
    bmesh.ops.translate(part.bm, verts=part.bm.verts[:], vec=(0.0, dy, dz))
    for c in part.cutters:
        moved(c, dy, dz)


for p in (rsight, leaf):
    moved(p, -0.008, 0.022)
# The folding blade bayonet, folded back under the barrel into its groove,
# its pivot block under the front sight.
BZ = -0.650  # the block, and the bayonet's pin
barrel.box((0.016, 0.024, 0.024), (0, 0.029, BZ), "Blued", bevel=0.002)
# The cleaning rod under the barrel, its head held in the block, its end
# in the forend.
barrel.cyl(0.0027, BZ - 0.014, FORE + 0.04, "Steel", segs=12, xf=T(0.003, 0.0375, 0))
barrel.lathe([(0.0, BZ - 0.021), (0.0028, BZ - 0.021), (0.0038, BZ - 0.0195), (0.0038, BZ - 0.0145),
              (0.0027, BZ - 0.013), (0.0, BZ - 0.013)], "Steel", segs=14, xf=T(0.003, 0.0375, 0))
blade = fillet([(BZ, 0.024), (FOLDED_TIP - 0.015, 0.024), (FOLDED_TIP, 0.017),
                (FOLDED_TIP - 0.015, 0.012), (BZ, 0.016)], [0.002, 0.003, 0.002, 0.003, 0.002], 2)
# The blade and its grip turn on the block's pin: +pi about X swings the
# point down and forward to fixed, well past the muzzle.
BAYONET_PIN = (0.0, 0.022, BZ)
bayonet = Part("bayonet", origin=BAYONET_PIN)
bayonet.prism(blade, -0.0022, 0.0022, "Steel", axis="x", bevel=0.0006, segs=1)
bayonet.prism(fillet([(BZ, 0.017), (BZ + 0.07, 0.017), (BZ + 0.07, 0.011), (BZ, 0.011)], 0.0025,
                     3), -0.0045, 0.0045, "Blued", axis="x", bevel=0.0008, segs=1)

# --- Bolt carrier (slides back) ----------------------------------------------------
carrier = Part("bolt_carrier", origin=(0.0, BORE, -0.2))
carrier.prism(fillet([(-0.0085, 0.040), (0.0085, 0.040), (0.0085, 0.062), (0.0, 0.066),
                      (-0.0085, 0.062)], [0.0013, 0.0013, 0.0025, 0.004, 0.0025], 2),
              -0.245, -0.135, "Steel", axis="z", bevel=0.0012, segs=2)
carrier.cyl(0.0068, -0.255, -0.24, "Steel", segs=24, xf=T(0, BORE, 0))
# The stripper clip's guide milled across the carrier's face, and two
# grooves along its top.
ccut = Part("cut")
ccut.box((0.0105, 0.012, 0.0085), (0.0, 0.066, -0.245), "Steel", bevel=0.0008, segs=1)
for sx in (-1, 1):
    ccut.box((0.0012, 0.003, 0.08), (sx * 0.0042, 0.0645, -0.19), "Steel")
carrier.cut(ccut)
# The charging handle, out of the slot on the right, a round knob.
carrier.box((0.026, 0.007, 0.01), (0.022, 0.054, -0.205), "Steel", bevel=0.0015)
carrier.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0064, 0.0015), (0.0068, 0.004), (0.0069, 0.007),
               (0.0068, 0.01), (0.0063, 0.012), (0.0055, 0.013), (0.003, 0.0138), (0.0, 0.014)],
              "Steel", segs=24, xf=T(0.035, 0.054, -0.205) @ RY(math.pi / 2))
# Drawn 50 mm forward of where it sits: its face at the breech, -0.205.
moved(carrier, 0.0, 0.05)
carrier.origin.z += 0.05

# Its origin on its pin: the game turns it about it (`Moves::Trigger`).
trig = Part("trigger", origin=(0.0, -0.006, -0.032))
trig.sweep(catmull([(-0.032, -0.012), (-0.034, -0.022), (-0.031, -0.031), (-0.026, -0.035)], 5,
                   closed=False), section_rect(0.005, 0.0035, 0.0012), "Blued")

join("static", stock, recv, mag, worn={})
join("static_barrel", barrel, rsight, leaf, stock_fore, furn, worn={})
join("static_butt", stock_butt, butt_furn, worn={})
for p in (carrier, trig, bayonet, safety):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.078, -0.02))
empty("pt_load_port", (0.0, 0.075, -0.179))
# The point with the bayonet fixed (the folded point turned through pi
# about the pin).
empty("pt_bayonet_tip", (0.0, 2 * BAYONET_PIN[1] - 0.0175, 2 * BAYONET_PIN[2] - FOLDED_TIP))

# Where a sling attaches: the swivels on the band's left and the butt's left.
empty("pt_sling_front", (-0.0195, 0.006, FORE - 0.014))
empty("pt_sling_rear", (-0.021, -0.062, 0.122))
report()
export("assets/models/guns/sks.glb")
export_part("static_butt", "assets/models/items/stock_sks.glb")
