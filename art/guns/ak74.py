"""AK-74: 5.45x39 rifle with wooden furniture and a plum polymer grip.
Writes assets/models/guns/ak74.glb.

Gun space as in src/weapons/layout.rs (`ak74()`): origin at the palm on the
pistol grip, -Z down the bore (bore axis y = 0.058), +Y up, metres.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, S, catmull, empty, export, fillet, join, main_guard,
                    make, report, section_rect, section_circle)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

BORE = 0.058
RX_HW = 0.0165          # receiver half width
RX_Z0, RX_Z1 = -0.236, 0.021
RX_Y0, RX_Y1 = 0.022, 0.078

# --- Receiver (stamped, with the ejection/charging opening on the right) -----
recv = Part("receiver")
recv_prof = fillet([(RX_Z0, RX_Y0 + 0.006), (RX_Z0 + 0.012, RX_Y0), (RX_Z1 - 0.004, RX_Y0),
                    (RX_Z1, RX_Y0 + 0.004), (RX_Z1, RX_Y1), (RX_Z0, RX_Y1)],
                   [0.002, 0.003, 0.002, 0.002, 0.001, 0.001], 2)
recv.prism(recv_prof, -RX_HW, RX_HW, "Blued", axis="x", bevel=0.0012, segs=2)
# Top rails the dust cover rides on.
for sx in (-1, 1):
    recv.box((0.0025, 0.003, RX_Z1 - RX_Z0 - 0.03), (sx * (RX_HW + 0.0005), RX_Y1 - 0.0045, -0.105),
             "Blued", bevel=0.0006, segs=1)
rc = Part("cut")
# Ejection port and charging slot (right side).
rc.box((0.02, 0.0175, 0.078), (RX_HW, 0.0695, -0.124), "Blued", bevel=0.002, segs=1)
rc.box((0.02, 0.0085, 0.13), (RX_HW, 0.0625, -0.115), "Blued")
# Magazine-well dimples, both sides.
for sx in (-1, 1):
    rc.sphere(0.03, (sx * (RX_HW + 0.0278), 0.040, -0.128), "Blued", segs=20, rings=10,
              scale=(1, 0.42, 1.25))
# Magazine opening underneath.
rc.box((0.026, 0.02, 0.064), (0, RX_Y0, -0.124), "Blued")
# Stampings cut 0.4 mm into the steel: the selector's АВ (auto) and ОД
# (single) on the right, under its lever's travel, and the serial and
# year on the left above the magazine well.
RIGHT = T(RX_HW - 0.0004, 0, 0) @ RY(math.pi / 2)
LEFT = T(-RX_HW + 0.0004, 0, 0) @ RY(-math.pi / 2)
text(rc, "АВ", 0.0042, "Blued", RIGHT @ T(0.1035, 0.037, 0.0), depth=0.0008)
text(rc, "ОД", 0.0042, "Blued", RIGHT @ T(0.083, 0.0275, 0.0), depth=0.0008)
text(rc, "ВЛ 3317", 0.0034, "Blued", LEFT @ T(-0.05, 0.063, 0.0), depth=0.0008)
text(rc, "1984", 0.0034, "Blued", LEFT @ T(-0.05, 0.057, 0.0), depth=0.0008)
recv.cut(rc)

parts_static = Part("static_misc")
# Rivets: trunnion and trigger-group pins.
for sx in (-1, 1):
    for z, y in ((-0.226, 0.03), (-0.226, 0.048), (-0.212, 0.03), (-0.212, 0.048), (-0.172, 0.029),
                 (-0.066, 0.034), (-0.042, 0.034), (-0.006, 0.034), (0.012, 0.03), (0.012, 0.05)):
        parts_static.sphere(0.0022, (sx * RX_HW, y, z), "Blued", segs=10, rings=5,
                            scale=(0.45, 1, 1))

# --- Dust cover --------------------------------------------------------------------
cover_sec = fillet([(-0.018, 0.0745), (0.018, 0.0745), (0.018, 0.090), (0.0, 0.0975),
                    (-0.018, 0.090)], [0.0013, 0.0013, 0.007, 0.012, 0.007], 4)
parts_static.prism(cover_sec, -0.208, 0.024, "Blued", axis="z", bevel=0.0012, segs=1)
# AK-74 stiffening ribs.
for z in (-0.06, -0.035, -0.01, 0.012):
    rib = [(x * 1.06, 0.0745 + (y - 0.0745) * 1.09) for x, y in cover_sec]
    parts_static.prism(rib, z - 0.002, z + 0.002, "Blued", axis="z", bevel=0.0008, segs=1)
# Recoil spring guide button at the rear.
parts_static.cyl(0.0042, 0.024, 0.029, "Steel", segs=12, xf=T(0, 0.088, 0), chamfer=0.001)

# --- Pistol grip -----------------------------------------------------------------------
grip_prof = catmull([(-0.0185, 0.0225), (-0.010, -0.025), (0.004, -0.072), (0.014, -0.0905),
                     (0.034, -0.093), (0.053, -0.087), (0.047, -0.055), (0.034, -0.012),
                     (0.025, 0.0225), (0.004, 0.024)], samples=4)


def grip_w(z, y):
    k = 0.0142 + 0.0012 * math.exp(-((y + 0.035) / 0.035) ** 2) - max(0.0, y - 0.01) * 0.05
    return (-k, k)


grip = Part("grip")
grip.prism(grip_prof, 0, 0, "Polymer", axis="x", bevel=0.004, segs=3, width=grip_w)
# Moulded grooves down both sides, along the grip's rake.
gc = Part("cut")
for sx in (-1, 1):
    for dz in (0.006, 0.0135, 0.021, 0.0285):
        gc.box((0.0024, 0.056, 0.0018), (sx * 0.0156, -0.036, dz), "Polymer", bevel=0.0006,
               segs=1, xf=RX(-0.274, about=(0, -0.036, dz)))
grip.cut(gc)
# Grip screw head at the base.
grip.cyl(0.004, -0.0935, -0.091, "Blued", segs=12, xf=T(0, 0, 0.034) @ RX(-math.pi / 2))

# --- Trigger guard and magazine catch ---------------------------------------------------
guard_path = catmull([(-0.090, 0.0225), (-0.0925, 0.012), (-0.086, 0.0045), (-0.072, 0.002),
                      (-0.035, 0.002), (-0.019, 0.004), (-0.013, 0.0115)], samples=4, closed=False)
parts_static.sweep(guard_path, section_rect(0.0085, 0.0028, 0.0010), "Blued")
# Magazine catch paddle, just ahead of the guard.
parts_static.prism(fillet([(-0.097, 0.0225), (-0.093, 0.0225), (-0.0905, 0.008), (-0.095, 0.006),
                           (-0.098, 0.012)], 0.0012, 2), -0.0055, 0.0055, "Blued", axis="x",
                   bevel=0.0006, segs=1)

# --- Barrel, gas system, sights, muzzle brake --------------------------------------------
barrel = Part("barrel")
barrel.lathe([(0.0, -0.616), (0.0098, -0.616), (0.0098, -0.47), (0.0104, -0.44), (0.0118, -0.24),
              (0.0, -0.24)], "Blued", segs=20, xf=T(0, BORE, 0))
# Bore at the muzzle (seen through the brake).
# Front sight block: base clamp, post with ears.
fsb = fillet([(-0.618, BORE - 0.012), (-0.585, BORE - 0.012), (-0.585, 0.086), (-0.592, 0.093),
              (-0.611, 0.093), (-0.618, 0.086)], [0.002, 0.002, 0.002, 0.003, 0.003, 0.002], 2)
barrel.prism(fsb, -0.0125, 0.0125, "Blued", axis="x", bevel=0.0015, segs=2)
# Protective ears around the post.
for sx in (-1, 1):
    barrel.prism(fillet([(-0.609, 0.090), (-0.594, 0.090), (-0.596, 0.1075), (-0.606, 0.1075)],
                        0.002, 2), sx * 0.0105 - 0.002, sx * 0.0105 + 0.002, "Blued", axis="x",
                 bevel=0.0006, segs=1)
barrel.cyl(0.0016, 0.090, 0.1045, "Blued", segs=8, xf=T(0, 0, -0.601) @ RX(-math.pi / 2))
# Bayonet lug and cleaning-rod stop under the front sight block.
barrel.box((0.008, 0.012, 0.03), (0, BORE - 0.016, -0.6), "Blued", bevel=0.0015)
# Gas block with the 90-degree gas port up to the tube.
gb = fillet([(-0.472, BORE - 0.013), (-0.440, BORE - 0.013), (-0.440, 0.094), (-0.448, 0.098),
             (-0.466, 0.098), (-0.472, 0.090)], [0.002, 0.002, 0.002, 0.004, 0.004, 0.002], 2)
barrel.prism(gb, -0.0128, 0.0128, "Blued", axis="x", bevel=0.0018, segs=2)
# Gas tube, the front part shows ahead of the upper handguard.
barrel.cyl(0.0092, -0.442, -0.236, "Blued", segs=16, xf=T(0, 0.0855, 0))
# Front handguard retainer band.
barrel.prism(fillet([(-0.0215, 0.020), (0.0215, 0.020), (0.0215, 0.072), (-0.0215, 0.072)],
                    [0.006, 0.006, 0.004, 0.004], 3), -0.437, -0.429, "Blued", axis="z",
             bevel=0.0008, segs=1)
# Rear sight block on the barrel, with its tangent leaf.
rs = fillet([(-0.236, 0.078), (-0.236, 0.096), (-0.226, 0.1005), (-0.190, 0.0975), (-0.186, 0.090),
             (-0.186, 0.078)], [0.001, 0.003, 0.003, 0.004, 0.003, 0.001], 2)
barrel.prism(rs, -0.0135, 0.0135, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.229, 0.0985), (-0.176, 0.1015), (-0.176, 0.1045), (-0.229, 0.1005)],
             -0.0065, 0.0065, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.017, 0.006, 0.008), (0, 0.1008, -0.206), "Blued", bevel=0.001)  # slider
# The notch plate, its U-notch framing the front post; range marks
# graduated along the leaf.
notch = [(-0.0065, 0.104), (0.0065, 0.104), (0.0065, 0.1088), (0.0011, 0.1088), (0.0011, 0.1074),
         (0.0, 0.1066), (-0.0011, 0.1074), (-0.0011, 0.1088), (-0.0065, 0.1088)]
barrel.prism(fillet(notch, [0.0005, 0.0005, 0.0005, 0.0003, 0.0, 0.0006, 0.0, 0.0003, 0.0005], 2),
             -0.179, -0.173, "Blued", axis="z")
for k in range(9):
    z = -0.2245 + k * 0.0052
    y = 0.0985 + (z + 0.229) / 0.053 * 0.003 + 0.0045
    w = 0.008 if k % 2 == 0 else 0.0045
    barrel.box((w, 0.0003, 0.0006), (-0.0065 + w / 2 + 0.0004, y + 0.00015, z), "Worn")
# The rear handguard retainer band and its locking lever on the right.
barrel.prism(fillet([(-0.0225, 0.020), (0.0225, 0.020), (0.0225, 0.072), (-0.0225, 0.072)],
                    [0.006, 0.006, 0.004, 0.004], 3), -0.2445, -0.2325, "Blued", axis="z",
             bevel=0.0008, segs=1)
barrel.box((0.0035, 0.022, 0.006), (0.0238, 0.044, -0.2405), "Blued", bevel=0.0008,
           xf=RX(-0.35, about=(0.0238, 0.044, -0.2405)))
barrel.cyl(0.0032, 0.0225, 0.027, "Blued", segs=10,
           xf=T(0, 0.0545, -0.2445) @ RY(math.pi / 2), chamfer=0.0006)
# Front sling swivel on the left of the gas block.
sling = [(-0.461 + 0.0075 * math.cos(a), 0.045 - 0.0075 * math.sin(a))
         for a in [math.pi * i / 8 for i in range(9)]]
barrel.sweep(sling, section_rect(0.0025, 0.0025, 0.0007), "Steel", xf=T(-0.0145, 0, 0))
# Cleaning rod under the barrel.
barrel.cyl(0.0031, -0.62, -0.24, "Steel", segs=8, xf=T(0, 0.040, 0))
barrel.box((0.009, 0.007, 0.006), (0, 0.040, -0.623), "Steel", bevel=0.0015)

# Muzzle brake: two-chamber AK-74 brake with side windows and a slanted face.
brake = Part("brake")
brake.tube(0.0122, 0.0034, -0.692, -0.612, "Blued", segs=24, xf=T(0, BORE, 0), chamfer=0.0012)
bc = Part("cut")
bc.box((0.04, 0.0105, 0.017), (0, BORE, -0.67), "Blued", bevel=0.003, segs=2)
bc.box((0.008, 0.04, 0.009), (0.0, BORE, -0.646), "Blued", bevel=0.002, segs=1,
       xf=RZ(math.radians(90), about=(0, BORE, -0.646)))
bc.box((0.012, 0.03, 0.004), (0.0, BORE + 0.012, -0.636), "Blued")
bc.box((0.05, 0.05, 0.03), (0.0, BORE + 0.016, -0.702), "Blued",
       xf=RX(math.radians(-25), about=(0, BORE, -0.692)))
brake.cut(bc)

# --- Wooden handguards ------------------------------------------------------------------
lower_sec = fillet([(-0.0175, 0.023), (0.0175, 0.023), (0.0235, 0.040), (0.0225, 0.071),
                    (-0.0225, 0.071), (-0.0235, 0.040)], [0.008, 0.008, 0.006, 0.0042, 0.0042, 0.006],
                   4)
lower = Part("lower_hg")
lower.prism(lower_sec, -0.428, -0.237, "Wood", axis="z", bevel=0.004, segs=2)
lc = Part("cut")
# Palm-swell grooves along each side.
for sx in (-1, 1):
    lc.box((0.006, 0.006, 0.15), (sx * 0.0245, 0.052, -0.333), "Wood", bevel=0.0025, segs=2)
lower.cut(lc)

upper_sec = fillet([(-0.0155, 0.0755), (0.0155, 0.0755), (0.0155, 0.091), (0.0, 0.0985),
                    (-0.0155, 0.091)], [0.0042, 0.0042, 0.008, 0.012, 0.008], 4)
upper = Part("upper_hg")
upper.prism(upper_sec, -0.392, -0.238, "Wood", axis="z", bevel=0.004, segs=2)
upper.prism([(x * 1.05, y + (y - 0.0755) * 0.04) for x, y in upper_sec], -0.244, -0.236, "Blued",
            axis="z", bevel=0.0008, segs=1)

# --- Stock ------------------------------------------------------------------------------
# (Corners hidden in the receiver and the butt plate stay sharp: a rounding
# tighter than the 8 mm bevel folds the bevelled side's outline over itself,
# and the folded side z-fights.)
stock_prof = fillet([(0.019, 0.0765), (0.30, 0.036), (0.30, -0.086), (0.255, -0.080),
                     (0.025, 0.004), (0.019, 0.022)], [0.0, 0.0, 0.0, 0.02, 0.01, 0.0], 4)


def stock_w(z, y):
    k = 0.0165 + (z - 0.02) / 0.28 * 0.0045
    return (-k, k)


stock = Part("stock")
stock.prism(stock_prof, 0, 0, "Wood", axis="x", bevel=0.008, segs=4, width=stock_w)
sc = Part("cut")
# The AK-74's lightening grooves along both sides of the stock.
for sx in (-1, 1):
    slot = fillet([(0.075, 0.034), (0.235, 0.008), (0.235, -0.008), (0.075, 0.020)],
                  [0.007, 0.007, 0.007, 0.007], 3)
    lo = sx * 0.024 - 0.006
    sc.prism(slot, lo, lo + 0.012, "Wood", axis="x", xf=T(0, 0.0, 0.0))
stock.cut(sc)
butt = Part("butt")
butt_prof = fillet([(0.296, 0.0375), (0.306, 0.0375), (0.306, -0.0875), (0.296, -0.0875)],
                   [0.002, 0.004, 0.004, 0.002], 2)
butt.prism(butt_prof, -0.0225, 0.0225, "Black", axis="x", bevel=0.002, segs=2)
butt.box((0.032, 0.05, 0.004), (0, -0.016, 0.307), "Black", bevel=0.0015)  # trapdoor
# Sling loop on the left of the stock.
loop = catmull([(0.255, -0.06), (0.262, -0.046), (0.276, -0.046), (0.283, -0.06)], 3, closed=False)
butt.sweep(loop, section_rect(0.003, 0.003, 0.0008), "Steel", xf=T(-0.022, -0.012, 0.0))

# --- Moving parts -------------------------------------------------------------------------
carrier = Part("bolt_carrier", origin=(0.0, 0.07, -0.14))
carrier.prism(fillet([(-0.0125, 0.059), (0.0125, 0.059), (0.0125, 0.074), (0.0, 0.078),
                      (-0.0125, 0.074)], 0.002, 2), -0.195, -0.088, "Steel", axis="z",
              bevel=0.0015, segs=1)
# Bolt face lugs and the extractor showing at the port.
carrier.cyl(0.0075, -0.212, -0.195, "Steel", segs=14, xf=T(0, BORE + 0.003, 0))
# Charging handle: arm out of the slot and an upturned knob.
carrier.box((0.026, 0.0065, 0.009), (0.026, 0.0625, -0.168), "Steel", bevel=0.0015)
carrier.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0068, 0.004), (0.0068, 0.009), (0.0055, 0.013),
               (0.0, 0.0135)], "Steel", segs=14,
              xf=T(0.0385, 0.0625, -0.170) @ RY(math.pi / 2) @ RX(math.radians(-12)))

PIVOT = (0.0188, 0.0605, -0.024)
selector = Part("selector", origin=PIVOT)
sel = fillet([(-0.025, 0.0545), (-0.020, 0.0545), (-0.110, 0.0575), (-0.131, 0.0545),
              (-0.137, 0.0505), (-0.133, 0.0485), (-0.130, 0.0535), (-0.112, 0.0545),
              (-0.112, 0.0675), (-0.024, 0.0665), (-0.018, 0.064)],
             [0.0, 0.0, 0.004, 0.002, 0.002, 0.001, 0.003, 0.003, 0.004, 0.004, 0.003], 2)
sel = [(z, y) for z, y in sel]
selector.prism(sel, RX_HW + 0.0006, RX_HW + 0.0032, "Blued", axis="x", bevel=0.0006, segs=1)
selector.cyl(0.0055, RX_HW + 0.0005, RX_HW + 0.0036, "Blued", segs=14,
             xf=T(0, PIVOT[1], PIVOT[2]) @ RY(math.pi / 2), chamfer=0.0008)

trig = Part("trigger", origin=(0.0, 0.02, -0.03))
trig.sweep(catmull([(-0.031, 0.024), (-0.0345, 0.016), (-0.0345, 0.010), (-0.031, 0.0045),
                    (-0.027, 0.0025)], 3, closed=False), section_rect(0.0048, 0.0032, 0.0012),
           "Blued")

# --- Build --------------------------------------------------------------------------------
join("static", recv, parts_static, grip, barrel, brake, lower, upper, stock, butt, worn={})
for p in (carrier, selector, trig):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, -0.69))
empty("pt_sight", (0.0, 0.104, 0.0))
empty("magwell", (0.0, 0.031, -0.139))

report()
export("assets/models/guns/ak74.glb")
