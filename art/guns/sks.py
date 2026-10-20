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
- empties `pt_muzzle`, `pt_sight`, `pt_load_port` (the clip guide, where
  a clip seats over the open action), `pt_bayonet_tip` (the point with
  the bayonet fixed)
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_rect)
from _kit import text  # noqa: E402

main_guard()

BORE = 0.048
MUZZLE = -0.735
RX0, RX1 = -0.255, -0.02          # receiver, front and rear
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
    (-0.552, 0.031), (-0.262, 0.031),          # forend top, under the handguard
    (-0.258, 0.020), (-0.025, 0.020),          # the receiver's bed
    (0.0, 0.026), (0.04, 0.019),               # wrist top to the comb
    (0.30, 0.008),                             # heel
    (0.30, -0.112),                            # toe
    (0.20, -0.094), (0.09, -0.068), (0.05, -0.058), (0.035, -0.04),  # belly, grip
    (0.0, -0.028), (-0.075, -0.012),           # behind the guard
    (-0.21, -0.012),                           # under the magazine
    (-0.45, -0.012), (-0.545, -0.006), (-0.552, 0.012),
]
stock_prof = fillet(stock_prof, [0.004, 0.003, 0.003, 0.004, 0.008, 0.03, 0.0, 0.0, 0.05, 0.04,
                                 0.012, 0.01, 0.012, 0.008, 0.004, 0.01, 0.006, 0.004], 4)
WIDTH = [(-0.56, 0.0145), (-0.3, 0.0175), (-0.25, 0.019), (-0.03, 0.019), (0.0, 0.0165),
         (0.04, 0.016), (0.1, 0.0195), (0.31, 0.0205)]


def stock_w(z, yc, h):
    k = lerp_table(z, WIDTH)
    return (k - (0.0025 if z > 0.07 else 0.0), k * (0.95 if z < -0.03 else 0.84))


def stock_power(z):
    return 3.4 if z < -0.03 else 2.5


stock = Part("stock")
stock.stock(stock_prof, -0.5515, 0.2995, "Wood", stock_w, stations=110, segs=24,
            power=stock_power)
sc = Part("cut")
# The sling slot through the butt.
sc.prism(fillet([(0.14, -0.05), (0.18, -0.058), (0.18, -0.066), (0.14, -0.058)], 0.0035, 2),
         -0.04, 0.04, "Wood", axis="x")
stock.cut(sc)

furn = Part("furniture")
# Steel butt plate with its trap door.
bp = fillet([(0.2995, 0.0105), (0.309, 0.0095), (0.309, -0.113), (0.2995, -0.1135)],
            [0.0, 0.002, 0.002, 0.0], 2)
furn.prism(bp, -0.0205, 0.0205, "Blued", axis="x", bevel=0.0015, segs=1)
furn.box((0.024, 0.05, 0.004), (0.0, -0.045, 0.3105), "Blued", bevel=0.0012)
# Upper handguard over the gas tube.
hg = fillet([(-0.0145, 0.0605), (0.0145, 0.0605), (0.0145, 0.075), (0.0, 0.0845),
             (-0.0145, 0.075)], [0.0042, 0.0042, 0.006, 0.01, 0.006], 4)
furn.prism(hg, -0.47, -0.275, "Wood", axis="z", bevel=0.004, segs=2)
# The barrel band and the nose cap the bayonet lug sits in.
band = fillet([(-0.0185, -0.008), (0.0185, -0.008), (0.0185, 0.083), (-0.0185, 0.083)],
              [0.009, 0.009, 0.008, 0.008], 4)
furn.prism(band, -0.48, -0.468, "Blued", axis="z", bevel=0.001, segs=1)
nose = fillet([(-0.016, -0.007), (0.016, -0.007), (0.016, 0.04), (-0.016, 0.04)],
              [0.007, 0.007, 0.004, 0.004], 3)
furn.prism(nose, -0.562, -0.55, "Blued", axis="z", bevel=0.001, segs=1)
# Rear sling swivel on the left of the butt; front one on the band.
for z0, y0, xs in ((0.16, -0.075, -0.021), (-0.474, 0.0, -0.0195)):
    loop = [(z0 + 0.011 * math.cos(a), y0 + 0.011 * math.sin(a))
            for a in [math.pi * 0.5 + math.pi * i / 8 for i in range(9)]]
    furn.sweep(loop, section_rect(0.0028, 0.0028, 0.0008), "Blued", xf=T(xs, 0, 0))

# --- Receiver ----------------------------------------------------------------------
recv = Part("receiver")
rprof = fillet([(RX0, 0.019), (RX1, 0.019), (RX1 + 0.004, 0.056), (RX0 + 0.01, 0.062),
                (RX0, 0.062)], [0.002, 0.003, 0.004, 0.004, 0.002], 2)
recv.prism(rprof, -RHW, RHW, "Blued", axis="x", bevel=0.0012, segs=1)
rc = Part("cut")
# The open top between the rear sight and the receiver cover, where the
# carrier shows and a clip goes in; the charging handle's slot on the right.
rc.box((0.018, 0.03, 0.105), (0.0, 0.066, -0.19), "Blued", bevel=0.002, segs=1)
rc.box((0.02, 0.009, 0.09), (RHW, 0.054, -0.168), "Blued")
# Stampings on the left: the arsenal's star in a triangle, serial, year.
LEFT = T(-RHW + 0.0006, 0, 0) @ RY(-math.pi / 2)
text(rc, "РК 1857", 0.0042, "Blued", LEFT @ T(-0.115, 0.044, 0.0), depth=0.0012)
text(rc, "1954", 0.0042, "Blued", LEFT @ T(-0.115, 0.036, 0.0), depth=0.0012)
recv.cut(rc)
# Receiver cover over the recoil spring, its takedown lever at the back.
cover = fillet([(-0.0145, 0.056), (0.0145, 0.056), (0.0145, 0.066), (0.0, 0.0735),
                (-0.0145, 0.066)], [0.0013, 0.0013, 0.005, 0.009, 0.005], 3)
recv.prism(cover, -0.137, -0.018, "Blued", axis="z", bevel=0.0012, segs=1)
recv.box((0.006, 0.022, 0.008), (RHW + 0.003, 0.05, -0.022), "Blued", bevel=0.0015)
recv.cyl(0.0045, RHW, RHW + 0.004, "Blued", segs=12, xf=T(0, 0.058, -0.022) @ RY(math.pi / 2))
# The clip guide cut in the carrier's face shows when the bolt is back;
# on the receiver, the bolt stop's tab on the left.
recv.box((0.004, 0.012, 0.018), (-RHW - 0.001, 0.036, -0.205), "Blued", bevel=0.001)

# Fixed magazine box under the receiver, hinged at the front, its latch
# just ahead of the guard; the trigger guard behind.
mag = Part("magbox")
mag.prism(fillet([(-0.215, -0.012), (-0.085, -0.012), (-0.084, -0.036), (-0.1, -0.05),
                  (-0.195, -0.05), (-0.213, -0.036)], [0.003, 0.002, 0.004, 0.008, 0.008, 0.006],
                 3), -0.0145, 0.0145, "Blued", axis="x", bevel=0.0015, segs=2)
mag.cyl(0.003, -0.016, 0.016, "Blued", segs=10, xf=T(0, -0.016, -0.212) @ RY(math.pi / 2))
mag.box((0.012, 0.008, 0.008), (0, -0.04, -0.08), "Blued", bevel=0.0015)
guard = catmull([(-0.078, -0.012), (-0.074, -0.03), (-0.06, -0.042), (-0.035, -0.044),
                 (-0.012, -0.038), (-0.002, -0.026), (0.004, -0.022)], 4, closed=False)
mag.sweep(guard, section_rect(0.0095, 0.003, 0.001), "Blued")
mag.box((0.012, 0.005, 0.085), (0, -0.0145, -0.04), "Blued", bevel=0.0015)
# The safety behind the trigger, pushed forward (safe).
mag.box((0.006, 0.012, 0.005), (0, -0.022, -0.012), "Blued", bevel=0.0012,
        xf=RX(0.4, about=(0, -0.017, -0.012)))

# --- Barrel, gas system, sights, bayonet ----------------------------------------------
barrel = Part("barrel")
barrel.lathe([(0.0044, MUZZLE + 0.002), (0.0095, MUZZLE), (0.0105, MUZZLE + 0.004),
              (0.0105, -0.6), (0.0115, -0.45), (0.0128, -0.28), (0.0145, -0.262),
              (0.0, -0.262)], "Blued", segs=20, xf=T(0, BORE, 0))
# Gas block and the tube running back to the rear sight.
gb = fillet([(-0.618, 0.036), (-0.585, 0.036), (-0.585, 0.083), (-0.596, 0.088), (-0.612, 0.088),
             (-0.618, 0.082)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(gb, -0.0125, 0.0125, "Blued", axis="x", bevel=0.0015, segs=2)
barrel.cyl(0.0095, -0.6, -0.468, "Blued", segs=16, xf=T(0, 0.073, 0))
barrel.lathe([(0.0, -0.615), (0.0075, -0.615), (0.0085, -0.611), (0.0085, -0.604),
              (0.0, -0.604)], "Blued", segs=14, xf=T(0, 0.073, 0))  # gas valve knob
# Front sight: base on the barrel, the post in a ring hood.
barrel.cyl(0.012, -0.73, -0.705, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.prism(fillet([(-0.727, 0.058), (-0.709, 0.058), (-0.711, 0.0835), (-0.722, 0.0835)],
                    0.0012, 2), -0.009, 0.009, "Blued", axis="x", bevel=0.0006, segs=1)
barrel.prism(fillet([(-0.721, 0.07), (-0.715, 0.07), (-0.716, 0.083), (-0.72, 0.083)], 0.0008, 1),
             -0.0012, 0.0012, "Blued", axis="x", bevel=0.0003, segs=1)
barrel.tube(0.0115, 0.0101, -0.727, -0.71, "Blued", segs=20, xf=T(0, 0.0735, 0))
# Rear sight: the base ahead of the receiver, the tangent leaf and slider,
# the notch plate; range marks on the leaf.
rs = fillet([(-0.30, 0.058), (-0.255, 0.058), (-0.255, 0.075), (-0.265, 0.079), (-0.295, 0.076),
             (-0.30, 0.07)], [0.002, 0.002, 0.003, 0.004, 0.004, 0.003], 2)
barrel.prism(rs, -0.0135, 0.0135, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.295, 0.0765), (-0.24, 0.0805), (-0.24, 0.0835), (-0.295, 0.0795)],
             -0.0062, 0.0062, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.017, 0.006, 0.008), (0, 0.0795, -0.272), "Blued", bevel=0.001)
notch = [(-0.0062, 0.083), (0.0062, 0.083), (0.0062, 0.0875), (0.001, 0.0875), (0.001, 0.0862),
         (0.0, 0.0855), (-0.001, 0.0862), (-0.001, 0.0875), (-0.0062, 0.0875)]
barrel.prism(fillet(notch, [0.0005, 0.0005, 0.0005, 0.0003, 0.0, 0.0006, 0.0, 0.0003, 0.0005], 2),
             -0.245, -0.239, "Blued", axis="z")
for k in range(8):
    z = -0.29 + k * 0.006
    y = 0.0765 + (z + 0.295) / 0.055 * 0.004 + 0.003
    w = 0.0075 if k % 2 == 0 else 0.004
    barrel.box((w, 0.0003, 0.0006), (-0.0062 + w / 2 + 0.0004, y + 0.00015, z), "Worn")
# The folding blade bayonet, folded back under the barrel into its groove,
# its pivot block under the front sight.
barrel.box((0.016, 0.018, 0.024), (0, 0.026, -0.69), "Blued", bevel=0.002)
blade = fillet([(-0.69, 0.024), (-0.42, 0.024), (-0.405, 0.017), (-0.42, 0.012), (-0.69, 0.016)],
               [0.002, 0.003, 0.002, 0.003, 0.002], 2)
# The blade and its grip turn on the block's pin: +pi about X swings the
# point down and forward to fixed, well past the muzzle.
BAYONET_PIN = (0.0, 0.022, -0.69)
bayonet = Part("bayonet", origin=BAYONET_PIN)
bayonet.prism(blade, -0.0022, 0.0022, "Steel", axis="x", bevel=0.0006, segs=1)
bayonet.prism(fillet([(-0.69, 0.017), (-0.62, 0.017), (-0.62, 0.011), (-0.69, 0.011)], 0.0025,
                     3), -0.0045, 0.0045, "Blued", axis="x", bevel=0.0008, segs=1)

# --- Bolt carrier (slides back) ----------------------------------------------------
carrier = Part("bolt_carrier", origin=(0.0, BORE, -0.2))
carrier.prism(fillet([(-0.0085, 0.040), (0.0085, 0.040), (0.0085, 0.062), (0.0, 0.066),
                      (-0.0085, 0.062)], [0.0013, 0.0013, 0.0025, 0.004, 0.0025], 2),
              -0.245, -0.135, "Steel", axis="z", bevel=0.0012, segs=1)
carrier.cyl(0.0068, -0.255, -0.24, "Steel", segs=14, xf=T(0, BORE, 0))
# The charging handle, out of the slot on the right, a round knob.
carrier.box((0.026, 0.007, 0.01), (0.022, 0.054, -0.205), "Steel", bevel=0.0015)
carrier.lathe([(0.0, 0.0), (0.0058, 0.0), (0.0068, 0.004), (0.0068, 0.01), (0.0055, 0.013),
               (0.0, 0.0135)], "Steel", segs=14, xf=T(0.035, 0.054, -0.205) @ RY(math.pi / 2))

trig = Part("trigger", origin=(0.0, -0.005, -0.045))
trig.sweep(catmull([(-0.05, -0.012), (-0.052, -0.022), (-0.049, -0.031), (-0.044, -0.035)], 3,
                   closed=False), section_rect(0.005, 0.0035, 0.0012), "Blued")

join("static", stock, furn, recv, mag, barrel, worn={})
for p in (carrier, trig, bayonet):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.0875, -0.02))
empty("pt_load_port", (0.0, 0.075, -0.2))
# The point with the bayonet fixed (the folded point, -0.405, turned
# through pi about the pin).
empty("pt_bayonet_tip", (0.0, 2 * BAYONET_PIN[1] - 0.0175, 2 * BAYONET_PIN[2] + 0.405))

report()
export("assets/models/guns/sks.glb")
