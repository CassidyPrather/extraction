"""RPK-74: 5.45x39 light machine gun. The AK-74's stamped receiver (with the
RPK's heavier trunnion), a long heavy barrel with the AK-74 brake, a tall
front sight at the muzzle, a folding bipod clamped behind it, wooden
handguards and the RPK's club-foot buttstock, the plum polymer AK-74 grip.
Writes assets/models/guns/rpk74.glb.

Gun space as for the AK-74: origin at the palm on the pistol grip, -Z down
the bore (bore axis y = 0.058), +Y up, metres.

The bipod (`bipod`) is modelled folded, legs forward under the barrel; its
origin is the hinge pin, and turning it about its local X by BIPOD_DEPLOY
(negative: forward-pointing legs swing down) stands it on its feet. The
legs are built at their deployed splay, which they keep folded.
"""

import math
import os
import sys

from mathutils import Matrix, Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, export_part, fillet, join, main_guard,
                    make, report, section_rect)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

BORE = 0.058
RX_HW = 0.0165          # receiver half width
RX_Z0, RX_Z1 = -0.236, 0.021
RX_Y0, RX_Y1 = 0.022, 0.078
MUZZLE = -0.82          # end of the brake
BRAKE_Z = MUZZLE + 0.08  # where the brake starts (barrel ends 2 mm into it)
FS_Z = BRAKE_Z + 0.004  # front sight block, behind the brake
PIVOT = (0.0, 0.038, -0.60)   # bipod hinge pin
LEG_SPLAY = 0.047        # leg axis x at the foot (each side)
LEG_FOOT = (0.0, 0.0275, -0.822)  # foot centre (on the leg axis), folded, x set per side
RAKE = math.radians(14)  # deployed legs lean forward this far from vertical


def along(a, b):
    """Matrix taking +Z (a cylinder's axis) onto the segment a->b, from a."""
    a, b = Vector(a), Vector(b)
    q = (b - a).normalized().to_track_quat("Z", "Y")
    return Matrix.Translation(a) @ q.to_matrix().to_4x4()


# --- Receiver (stamped, with the ejection/charging opening on the right) -----
recv = Part("receiver")
recv_prof = fillet([(RX_Z0, RX_Y0 + 0.006), (RX_Z0 + 0.012, RX_Y0), (RX_Z1 - 0.004, RX_Y0),
                    (RX_Z1, RX_Y0 + 0.004), (RX_Z1, RX_Y1), (RX_Z0, RX_Y1)],
                   [0.002, 0.003, 0.002, 0.002, 0.001, 0.001], 2)
recv.prism(recv_prof, -RX_HW, RX_HW, "Blued", axis="x", bevel=0.0012, segs=2)
for sx in (-1, 1):
    recv.box((0.0025, 0.003, RX_Z1 - RX_Z0 - 0.03), (sx * (RX_HW + 0.0005), RX_Y1 - 0.0045, -0.105),
             "Blued", bevel=0.0006, segs=1)
rc = Part("cut")
rc.box((0.02, 0.0175, 0.078), (RX_HW, 0.0695, -0.124), "Blued", bevel=0.002, segs=1)
rc.box((0.02, 0.0085, 0.13), (RX_HW, 0.0625, -0.115), "Blued")
for sx in (-1, 1):
    rc.sphere(0.03, (sx * (RX_HW + 0.0278), 0.040, -0.128), "Blued", segs=20, rings=10,
              scale=(1, 0.42, 1.25))
rc.box((0.026, 0.02, 0.064), (0, RX_Y0, -0.124), "Blued")
# Stampings 0.4 mm into the steel: АВ / ОД under the selector's travel on
# the right, serial and year on the left above the magazine well.
RIGHT = T(RX_HW - 0.0004, 0, 0) @ RY(math.pi / 2)
LEFT = T(-RX_HW + 0.0004, 0, 0) @ RY(-math.pi / 2)
text(rc, "АВ", 0.0042, "Blued", RIGHT @ T(0.1035, 0.037, 0.0), depth=0.0008)
text(rc, "ОД", 0.0042, "Blued", RIGHT @ T(0.083, 0.0275, 0.0), depth=0.0008)
text(rc, "ДМ 4127", 0.0034, "Blued", LEFT @ T(-0.05, 0.063, 0.0), depth=0.0008)
text(rc, "1986", 0.0034, "Blued", LEFT @ T(-0.05, 0.057, 0.0), depth=0.0008)
recv.cut(rc)

parts_static = Part("static_misc")
# The RPK's heavier front trunnion: a reinforcing plate each side of the
# receiver's front, with its extra row of rivets; the trigger-group pins.
for sx in (-1, 1):
    parts_static.prism(fillet([(-0.236, 0.024), (-0.198, 0.024), (-0.196, 0.040), (-0.205, 0.058),
                               (-0.236, 0.058)], [0.002, 0.004, 0.006, 0.006, 0.002], 2),
                       sx * RX_HW - 0.0006, sx * RX_HW + 0.0006, "Blued", axis="x",
                       bevel=0.0004, segs=1)
    for z, y in ((-0.229, 0.03), (-0.229, 0.04), (-0.229, 0.051), (-0.216, 0.03), (-0.216, 0.051),
                 (-0.204, 0.031), (-0.172, 0.029),
                 (-0.066, 0.034), (-0.042, 0.034), (-0.006, 0.034), (0.012, 0.03), (0.012, 0.05)):
        off = 0.0006 if z < -0.195 else 0.0
        parts_static.sphere(0.0022, (sx * (RX_HW + off), y, z), "Blued", segs=10, rings=5,
                            scale=(0.45, 1, 1))

# --- Dust cover --------------------------------------------------------------------
cover_sec = fillet([(-0.018, 0.0745), (0.018, 0.0745), (0.018, 0.090), (0.0, 0.0975),
                    (-0.018, 0.090)], [0.0013, 0.0013, 0.007, 0.012, 0.007], 4)
parts_static.prism(cover_sec, -0.208, 0.024, "Blued", axis="z", bevel=0.0012, segs=1)
for z in (-0.06, -0.035, -0.01, 0.012):
    rib = [(x * 1.06, 0.0745 + (y - 0.0745) * 1.09) for x, y in cover_sec]
    parts_static.prism(rib, z - 0.002, z + 0.002, "Blued", axis="z", bevel=0.0008, segs=1)
parts_static.cyl(0.0042, 0.024, 0.029, "Steel", segs=12, xf=T(0, 0.088, 0), chamfer=0.001)

# --- Pistol grip (the AK-74's) -------------------------------------------------------
grip_prof = catmull([(-0.0185, 0.0225), (-0.010, -0.025), (0.004, -0.072), (0.014, -0.0905),
                     (0.034, -0.093), (0.053, -0.087), (0.047, -0.055), (0.034, -0.012),
                     (0.025, 0.0225), (0.004, 0.024)], samples=4)


def grip_w(z, y):
    k = 0.0142 + 0.0012 * math.exp(-((y + 0.035) / 0.035) ** 2) - max(0.0, y - 0.01) * 0.05
    return (-k, k)


grip = Part("grip")
grip.prism(grip_prof, 0, 0, "Polymer", axis="x", bevel=0.004, segs=3, width=grip_w)
gc = Part("cut")
for sx in (-1, 1):
    for dz in (0.006, 0.0135, 0.021, 0.0285):
        gc.box((0.0024, 0.056, 0.0018), (sx * 0.0156, -0.036, dz), "Polymer", bevel=0.0006,
               segs=1, xf=RX(-0.274, about=(0, -0.036, dz)))
grip.cut(gc)
grip.cyl(0.004, -0.0935, -0.091, "Blued", segs=12, xf=T(0, 0, 0.034) @ RX(-math.pi / 2))

# --- Trigger guard and magazine catch ---------------------------------------------------
guard_path = catmull([(-0.090, 0.0225), (-0.0925, 0.012), (-0.086, 0.0045), (-0.072, 0.002),
                      (-0.035, 0.002), (-0.019, 0.004), (-0.013, 0.0115)], samples=4, closed=False)
parts_static.sweep(guard_path, section_rect(0.0085, 0.0028, 0.0010), "Blued")
parts_static.prism(fillet([(-0.097, 0.0225), (-0.093, 0.0225), (-0.0905, 0.008), (-0.095, 0.006),
                           (-0.098, 0.012)], 0.0012, 2), -0.0055, 0.0055, "Blued", axis="x",
                   bevel=0.0006, segs=1)

# --- Barrel, gas system, sights ---------------------------------------------------------
barrel = Part("barrel")
# The RPK's heavy barrel: thick at the chamber, tapering to the muzzle.
barrel.lathe([(0.0, BRAKE_Z - 0.002), (0.0102, BRAKE_Z - 0.002), (0.0104, -0.62),
              (0.0110, -0.47), (0.0116, -0.44), (0.0128, -0.24), (0.0, -0.24)], "Blued", segs=20,
             xf=T(0, BORE, 0))
# Tall front sight block at the muzzle: base clamp round the barrel, a
# tall tapered column, the post between protective ears.
fsb = fillet([(FS_Z, BORE - 0.0135), (FS_Z + 0.034, BORE - 0.0135), (FS_Z + 0.034, BORE + 0.010),
              (FS_Z + 0.022, 0.091), (FS_Z + 0.022, 0.095), (FS_Z + 0.004, 0.095),
              (FS_Z + 0.002, 0.084), (FS_Z, BORE + 0.012)],
             [0.002, 0.002, 0.004, 0.006, 0.0025, 0.0025, 0.006, 0.004], 2)
barrel.prism(fsb, -0.0125, 0.0125, "Blued", axis="x", bevel=0.0015, segs=2)
for sx in (-1, 1):
    barrel.prism(fillet([(FS_Z + 0.006, 0.092), (FS_Z + 0.020, 0.092), (FS_Z + 0.0185, 0.1105),
                         (FS_Z + 0.0085, 0.1105)], 0.002, 2), sx * 0.0105 - 0.002,
                 sx * 0.0105 + 0.002, "Blued", axis="x", bevel=0.0006, segs=1)
# (Its top on the eye line, `pt_sight`: `capture::sightline`.)
barrel.cyl(0.0016, 0.092, 0.1071, "Blued", segs=8, xf=T(0, 0, FS_Z + 0.013) @ RX(-math.pi / 2))
# Lightening hole through the column.
fc = Part("cut")
fc.cyl(0.0042, -0.02, 0.02, "Blued", segs=12, xf=T(0, 0.0805, FS_Z + 0.014) @ RY(math.pi / 2))
barrel.cut(fc)
# Gas block with the 90-degree gas port up to the tube.
gb = fillet([(-0.472, BORE - 0.0135), (-0.440, BORE - 0.0135), (-0.440, 0.094), (-0.448, 0.098),
             (-0.466, 0.098), (-0.472, 0.090)], [0.002, 0.002, 0.002, 0.004, 0.004, 0.002], 2)
barrel.prism(gb, -0.013, 0.013, "Blued", axis="x", bevel=0.0018, segs=2)
barrel.cyl(0.0092, -0.442, -0.236, "Blued", segs=16, xf=T(0, 0.0855, 0))
# Front handguard retainer band.
barrel.prism(fillet([(-0.025, 0.017), (0.025, 0.017), (0.0235, 0.072), (-0.0235, 0.072)],
                    [0.007, 0.007, 0.004, 0.004], 3), -0.437, -0.429, "Blued", axis="z",
             bevel=0.0008, segs=1)
# Rear sight block with the RPK's long leaf (to 1000 m) and windage.
rs = fillet([(-0.236, 0.078), (-0.236, 0.096), (-0.226, 0.1005), (-0.190, 0.0975), (-0.186, 0.090),
             (-0.186, 0.078)], [0.001, 0.003, 0.003, 0.004, 0.003, 0.001], 2)
barrel.prism(rs, -0.0135, 0.0135, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.229, 0.0985), (-0.168, 0.1015), (-0.168, 0.1045), (-0.229, 0.1005)],
             -0.0065, 0.0065, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.017, 0.006, 0.008), (0, 0.1008, -0.206), "Blued", bevel=0.001)  # slider
# The windage notch block, wider than the leaf, with its knurled screw
# heads both sides; the U-notch frames the front post.
notch = [(-0.0085, 0.104), (0.0085, 0.104), (0.0085, 0.1088), (0.0011, 0.1088), (0.0011, 0.1074),
         (0.0, 0.1066), (-0.0011, 0.1074), (-0.0011, 0.1088), (-0.0085, 0.1088)]
barrel.prism(fillet(notch, [0.0005, 0.0005, 0.0005, 0.0003, 0.0, 0.0006, 0.0, 0.0003, 0.0005], 2),
             -0.172, -0.165, "Blued", axis="z")
for sx in (-1, 1):
    barrel.cyl(0.0026, sx * 0.0085, sx * 0.0115, "Steel", segs=10,
               xf=T(0, 0.1062, -0.1685) @ RY(math.pi / 2), chamfer=0.0005)
for k in range(11):
    z = -0.2245 + k * 0.0052
    # On the leaf's top (they floated 2 mm over it: `capture::attach`).
    y = 0.1005 + (z + 0.229) / 0.061 * 0.004 - 0.0001
    w = 0.008 if k % 2 == 0 else 0.0045
    barrel.box((w, 0.0003, 0.0006), (-0.0065 + w / 2 + 0.0004, y + 0.00015, z), "Worn")
# Rear handguard retainer band and its locking lever on the right.
barrel.prism(fillet([(-0.026, 0.016), (0.026, 0.016), (0.0225, 0.072), (-0.0225, 0.072)],
                    [0.007, 0.007, 0.004, 0.004], 3), -0.2445, -0.2325, "Blued", axis="z",
             bevel=0.0008, segs=1)
barrel.box((0.0035, 0.022, 0.006), (0.0256, 0.042, -0.2405), "Blued", bevel=0.0008,
           xf=RX(-0.35, about=(0.0256, 0.042, -0.2405)))
barrel.cyl(0.0032, 0.0225, 0.028, "Blued", segs=10,
           xf=T(0, 0.0545, -0.2445) @ RY(math.pi / 2), chamfer=0.0006)
# Cleaning rod under the barrel, through the bipod clamp to its stop
# under the front sight.
barrel.cyl(0.0031, FS_Z + 0.03, -0.24, "Steel", segs=8, xf=T(0, 0.040, 0))
barrel.box((0.009, 0.008, 0.008), (0, 0.041, FS_Z + 0.026), "Blued", bevel=0.0015)

# Bipod clamp: a collar round the barrel with the hinge tab beneath, the
# hinge pin through it, and the front sling loop on its left.
clamp = Part("clamp")
clamp.tube(0.0148, 0.0104, PIVOT[2] - 0.013, PIVOT[2] + 0.013, "Blued", segs=20,
           xf=T(0, BORE, 0), chamfer=0.0012)
clamp.prism(fillet([(PIVOT[2] - 0.011, 0.050), (PIVOT[2] + 0.011, 0.050),
                    (PIVOT[2] + 0.0075, PIVOT[1] - 0.006), (PIVOT[2] - 0.0075, PIVOT[1] - 0.006)],
                   [0.002, 0.002, 0.005, 0.005], 3), -0.0042, 0.0042, "Blued", axis="x",
            bevel=0.0008, segs=1)
# Clamp screw boss on the right.
clamp.box((0.008, 0.012, 0.016), (0.016, 0.050, PIVOT[2]), "Blued", bevel=0.0018, segs=2)
clamp.cyl(0.0035, 0.019, 0.0225, "Steel", segs=10, xf=T(0, 0.050, PIVOT[2]) @ RY(math.pi / 2),
          chamfer=0.0006)
clamp.cyl(0.0021, -0.0175, 0.0175, "Steel", segs=10, xf=T(*PIVOT) @ RY(math.pi / 2),
          chamfer=0.0004)
for sx in (-1, 1):
    clamp.cyl(0.0034, sx * 0.0168, sx * 0.0192, "Steel", segs=10,
              xf=T(*PIVOT) @ RY(math.pi / 2), chamfer=0.0006)
clamp.box((0.006, 0.010, 0.014), (-0.0142, 0.0505, PIVOT[2]), "Blued", bevel=0.0015, segs=2)
sling = [(PIVOT[2] + 0.0085 * math.cos(a), 0.052 - 0.0085 * math.sin(a))
         for a in [math.pi * i / 8 for i in range(9)]]
clamp.sweep(sling, section_rect(0.0025, 0.0025, 0.0007), "Steel",
            xf=T(-0.0172, 0, 0))

# Muzzle brake: the AK-74's two-chamber brake.
brake = Part("brake")
brake.tube(0.0122, 0.0034, MUZZLE, BRAKE_Z, "Blued", segs=24, xf=T(0, BORE, 0), chamfer=0.0012)
bc = Part("cut")
bc.box((0.04, 0.0105, 0.017), (0, BORE, MUZZLE + 0.022), "Blued", bevel=0.003, segs=2)
bc.box((0.008, 0.04, 0.009), (0.0, BORE, MUZZLE + 0.046), "Blued", bevel=0.002, segs=1,
       xf=RZ(math.radians(90), about=(0, BORE, MUZZLE + 0.046)))
bc.box((0.012, 0.03, 0.004), (0.0, BORE + 0.012, MUZZLE + 0.056), "Blued")
bc.box((0.05, 0.05, 0.03), (0.0, BORE + 0.016, MUZZLE - 0.01), "Blued",
       xf=RX(math.radians(-25), about=(0, BORE, MUZZLE)))
brake.cut(bc)

# --- Wooden handguards ------------------------------------------------------------------
# The RPK's lower handguard is deeper and flares wider than the AK's.
lower_sec = fillet([(-0.019, 0.017), (0.019, 0.017), (0.0255, 0.038), (0.0235, 0.071),
                    (-0.0235, 0.071), (-0.0255, 0.038)], [0.009, 0.009, 0.007, 0.0042, 0.0042, 0.007],
                   4)
lower = Part("lower_hg")
lower.prism(lower_sec, -0.428, -0.2455, "Wood", axis="z", bevel=0.004, segs=2)
lc = Part("cut")
for sx in (-1, 1):
    lc.box((0.006, 0.006, 0.14), (sx * 0.0262, 0.050, -0.337), "Wood", bevel=0.0025, segs=2)
lower.cut(lc)

upper_sec = fillet([(-0.0155, 0.0755), (0.0155, 0.0755), (0.0155, 0.091), (0.0, 0.0985),
                    (-0.0155, 0.091)], [0.0042, 0.0042, 0.008, 0.012, 0.008], 4)
upper = Part("upper_hg")
upper.prism(upper_sec, -0.392, -0.247, "Wood", axis="z", bevel=0.004, segs=2)
upper.prism([(x * 1.05, y + (y - 0.0755) * 0.04) for x, y in upper_sec], -0.2465, -0.236, "Blued",
            axis="z", bevel=0.0008, segs=1)

# --- Club-foot buttstock ------------------------------------------------------------------
# A straight comb, the wrist line dropping from the grip, then the scoop
# for the off hand and the deep toe of the "club foot".
bottom = [(0.296, -0.116)] + catmull([(0.272, -0.115), (0.258, -0.104), (0.246, -0.082),
                  (0.226, -0.066), (0.19, -0.054), (0.10, -0.028), (0.025, 0.004)], 4,
                 closed=False)
stock_prof = [(0.019, 0.0765), (0.296, 0.041)] + bottom + [(0.019, 0.022)]
radii = [0.0, 0.0] + [0.0] * len(bottom) + [0.0]
radii[2] = 0.0
stock_prof = fillet(stock_prof, radii, 4)


def stock_w(z, yc, h):
    # Half widths at the belly (narrowing into the foot) and the comb.
    k = 0.0168 + (z - 0.02) / 0.28 * 0.0052
    return (k - max(0.0, -0.06 - (yc - h)) * 0.03, k * 0.85)


# Rounded across (as the AK's): the extruded outline read as a plank.
stock = Part("stock")
stock.stock(stock_prof, 0.0192, 0.2958, "Wood", stock_w, stations=90, segs=28, power=2.8)
butt = Part("butt")
butt_prof = fillet([(0.2955, 0.0425), (0.306, 0.0425), (0.306, -0.1175), (0.2955, -0.1175)],
                   [0.002, 0.004, 0.004, 0.002], 2)
butt.prism(butt_prof, -0.0235, 0.0235, "Blued", axis="x", bevel=0.002, segs=2)
butt.box((0.032, 0.056, 0.004), (0, -0.026, 0.307), "Blued", bevel=0.0015)  # trapdoor
butt.cyl(0.0022, 0.306, 0.3085, "Steel", segs=8, xf=T(0, 0.028, 0))
butt.cyl(0.0022, 0.306, 0.3085, "Steel", segs=8, xf=T(0, -0.104, 0))
# Sling loop on the left of the stock, ahead of the foot.
loop = catmull([(0.205, -0.054), (0.211, -0.04), (0.225, -0.04), (0.231, -0.054)], 3, closed=False)
butt.sweep(loop, section_rect(0.003, 0.003, 0.0008), "Steel", xf=T(-0.0205, -0.008, 0.0))
# Its two studs into the wood, where the rounded stock falls away under it.
for z in (0.205, 0.231):
    butt.cyl(0.0016, -0.022, -0.010, "Steel", segs=10, xf=T(0, -0.062, z) @ RY(math.pi / 2))

# --- Moving parts -------------------------------------------------------------------------
carrier = Part("bolt_carrier", origin=(0.0, 0.07, -0.14))
carrier.prism(fillet([(-0.0125, 0.059), (0.0125, 0.059), (0.0125, 0.074), (0.0, 0.078),
                      (-0.0125, 0.074)], 0.002, 2), -0.195, -0.088, "Steel", axis="z",
              bevel=0.0015, segs=1)
carrier.cyl(0.0075, -0.212, -0.195, "Steel", segs=14, xf=T(0, BORE + 0.003, 0))
carrier.box((0.026, 0.0065, 0.009), (0.026, 0.0625, -0.168), "Steel", bevel=0.0015)
carrier.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0068, 0.004), (0.0068, 0.009), (0.0055, 0.013),
               (0.0, 0.0135)], "Steel", segs=14,
              xf=T(0.0385, 0.0625, -0.170) @ RY(math.pi / 2) @ RX(math.radians(-12)))

SEL_PIVOT = (0.0188, 0.0605, -0.024)
selector = Part("selector", origin=SEL_PIVOT)
sel = fillet([(-0.025, 0.0545), (-0.020, 0.0545), (-0.110, 0.0575), (-0.131, 0.0545),
              (-0.137, 0.0505), (-0.133, 0.0485), (-0.130, 0.0535), (-0.112, 0.0545),
              (-0.112, 0.0675), (-0.024, 0.0665), (-0.018, 0.064)],
             [0.0, 0.0, 0.004, 0.002, 0.002, 0.001, 0.003, 0.003, 0.004, 0.004, 0.003], 2)
selector.prism(sel, RX_HW + 0.0006, RX_HW + 0.0032, "Blued", axis="x", bevel=0.0006, segs=1)
selector.cyl(0.0055, RX_HW + 0.0005, RX_HW + 0.0036, "Blued", segs=14,
             xf=T(0, SEL_PIVOT[1], SEL_PIVOT[2]) @ RY(math.pi / 2), chamfer=0.0008)

trig = Part("trigger", origin=(0.0, 0.02, -0.03))
trig.sweep(catmull([(-0.031, 0.024), (-0.0345, 0.016), (-0.0345, 0.010), (-0.031, 0.0045),
                    (-0.027, 0.0025)], 3, closed=False), section_rect(0.0048, 0.0032, 0.0012),
           "Blued")

# Bipod, folded: the yoke's two knuckles on the pin either side of the
# clamp's tab, arms forward to the leg sockets, the legs splayed out to
# their pointed feet, and the spring catch bridging them.
bipod = Part("bipod", origin=PIVOT)
px, py, pz = PIVOT
for sx in (-1, 1):
    knuckle = fillet([(pz + 0.0062, py), (pz, py + 0.0062), (pz - 0.024, py + 0.004),
                      (pz - 0.027, py - 0.004), (pz - 0.008, py - 0.0062)],
                     [0.004, 0.004, 0.003, 0.003, 0.004], 3)
    bipod.prism(knuckle, sx * 0.0048 if sx > 0 else -0.0148, 0.0148 if sx > 0 else -0.0048,
                "Blued", axis="x", bevel=0.0007, segs=1)
    top = (sx * 0.0105, py - 0.0005, pz - 0.022)
    foot = (sx * LEG_SPLAY, LEG_FOOT[1], LEG_FOOT[2])
    m = along(top, foot)
    length = (Vector(foot) - Vector(top)).length
    # Socket, the leg tube, and the foot: a ring then a cone to the point.
    bipod.cyl(0.0058, 0.0, 0.03, "Blued", segs=12, xf=m, chamfer=0.001)
    bipod.cyl(0.0042, 0.028, length, "Blued", segs=10, xf=m)
    bipod.lathe([(0.0, length - 0.004), (0.0058, length - 0.004), (0.0062, length),
                 (0.0058, length + 0.004), (0.0022, length + 0.016), (0.0, length + 0.017)],
                "Blued", segs=12, xf=m)
# Spring catch across between the leg sockets.
bipod.cyl(0.0022, -0.016, 0.016, "Steel", segs=8,
          xf=T(0, py - 0.0005, pz - 0.0255) @ RY(math.pi / 2))

# --- Build --------------------------------------------------------------------------------
join("static", recv, parts_static, grip, barrel, clamp, brake, lower, upper, worn={})
# The butt, off its tang screws at the vice (`weapons::gunsmith`).
join("static_butt", stock, butt, worn={})
for p in (carrier, selector, trig):
    make(p, worn={})
make(bipod, worn=None)  # thin legs: edge wear would turn them all bare steel

# Bipod deploy: turn about X at the pivot by BIPOD_DEPLOY (negative swings
# the folded legs down), so the legs lean RAKE forward of vertical.
top_r = Vector((0.0105, PIVOT[1] - 0.0005, PIVOT[2] - 0.022))
foot_r = Vector((LEG_SPLAY, LEG_FOOT[1], LEG_FOOT[2]))
tip_r = foot_r + (foot_r - top_r).normalized() * 0.017   # the right foot's point
tip = Vector((0.0, tip_r.y, tip_r.z))                     # midway between the points
leg = tip - Vector(PIVOT)
below = math.atan2(-leg.y, -leg.z)          # folded leg's angle below forward
BIPOD_DEPLOY = -((math.pi / 2 - RAKE) - below)
dep = Vector(PIVOT) + (Matrix.Rotation(BIPOD_DEPLOY, 3, "X") @ leg)
print(f"  bipod: BIPOD_DEPLOY {BIPOD_DEPLOY:+.4f} rad ({math.degrees(BIPOD_DEPLOY):+.2f} deg); "
      f"deployed feet tips at x +-{tip_r.x:.4f}, y {dep.y:+.4f}, z {dep.z:+.4f}; folded {tip_r}")

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.1071, 0.0))
empty("pt_port", (0.025, 0.07, -0.13))
empty("pt_support", (0.0, 0.03, -0.34))
empty("pt_handle", (0.045, 0.062, -0.17))
empty("magwell", (0.0, 0.0265, -0.1415))
empty("pt_bipod", PIVOT)
empty("pt_bipod_feet", tuple(tip))
empty("pt_sling_front", (-0.0172, 0.0435, PIVOT[2]))
empty("pt_sling_rear", (-0.0205, -0.048, 0.218))
report()
export("assets/models/guns/rpk74.glb")
export_part("static_butt", "assets/models/items/stock_rpk74.glb")
