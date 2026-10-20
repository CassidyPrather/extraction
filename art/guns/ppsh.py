"""PPSh-41: a 7.62x25 submachine gun firing from an open bolt, fed from a
71-round drum. Writes assets/models/guns/ppsh.glb.

Gun space as in src/weapons/layout.rs (`ppsh()`): origin at the palm on
the wrist of the stock, -Z down the bore (bore axis y = BORE), +Y up,
metres.

- `static`     stock, receiver and its hinged cover, the perforated
               barrel jacket with its slanted compensator, sights, the
               guard, the magazine catch
- `bolt`       the bolt and its cocking handle on the right, modelled
               forward (closed); an open bolt rests back, so the game
               draws it at its travel
- `trigger`
- `selector`   the fire selector ahead of the trigger, inside the guard;
               turns about X with the mode
- empties `pt_muzzle`, `pt_sight`, `magwell` (where the drum's feed
               tower seats: the drum hangs below it)
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_rect)
from _kit import text  # noqa: E402

main_guard()

BORE = 0.052
RX0, RX1 = -0.2, 0.012         # receiver front and rear
RHW = 0.0165
MUZZLE = -0.52
WELL_Z = -0.152                # the magazine well's centre


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


# --- Stock: wood from the magazine well back to the butt ---------------------------------
stock_prof = [
    (-0.205, 0.03), (0.0, 0.03),                # under the receiver
    (0.03, 0.026), (0.07, 0.016),               # wrist to comb
    (0.315, 0.004),                             # heel
    (0.315, -0.112),                            # toe
    (0.21, -0.095), (0.1, -0.072), (0.05, -0.058), (0.03, -0.038),  # belly, grip
    (-0.005, -0.022), (-0.06, -0.012),          # behind the guard
    (-0.11, -0.005), (-0.125, 0.004),           # up to the well
    (-0.18, 0.004), (-0.205, 0.012),            # the forend ahead of the well
]
stock_prof = fillet(stock_prof, [0.004, 0.004, 0.012, 0.03, 0.0, 0.0, 0.05, 0.04, 0.012, 0.01,
                                 0.01, 0.008, 0.006, 0.004, 0.004, 0.004], 4)
WIDTH = [(-0.21, 0.02), (-0.02, 0.021), (0.03, 0.0165), (0.08, 0.017), (0.32, 0.021)]


def stock_w(z, yc, h):
    k = lerp_table(z, WIDTH)
    return (k - (0.0025 if z > 0.08 else 0.0), k * (0.95 if z < 0.0 else 0.84))


stock = Part("stock")
stock.stock(stock_prof, -0.2045, 0.3145, "Wood", stock_w, stations=90, segs=24,
            power=lambda z: 3.4 if z < 0.0 else 2.5)
sc = Part("cut")
# The magazine well through the stock, and the sling slot in the butt.
sc.box((0.03, 0.06, 0.064), (0.0, 0.0, WELL_Z), "Wood", bevel=0.003, segs=1)
sc.prism(fillet([(0.15, -0.055), (0.19, -0.063), (0.19, -0.071), (0.15, -0.063)], 0.0035, 2),
         -0.04, 0.04, "Wood", axis="x")
stock.cut(sc)
furn = Part("furniture")
bp = fillet([(0.3145, 0.0055), (0.324, 0.0045), (0.324, -0.113), (0.3145, -0.1135)],
            [0.0, 0.002, 0.002, 0.0], 2)
furn.prism(bp, -0.0215, 0.0215, "Blued", axis="x", bevel=0.0015, segs=1)
furn.box((0.024, 0.05, 0.004), (0.0, -0.05, 0.3255), "Blued", bevel=0.0012)
# The front sling swivel on the stock's left, ahead of the magazine well.
swivel = [(-0.185 + 0.009 * math.cos(a), 0.015 + 0.009 * math.sin(a))
          for a in [math.pi + math.pi * i / 8 for i in range(9)]]
furn.sweep(swivel, section_rect(0.0025, 0.0025, 0.0007), "Blued", xf=T(-0.021, 0, 0))
furn.box((0.004, 0.006, 0.022), (-0.0195, 0.017, -0.185), "Blued", bevel=0.001)

# --- Receiver: the lower half, and the cover hinged at the front -------------------------
recv = Part("receiver")
lower = fillet([(RX0, 0.03), (RX1, 0.03), (RX1, 0.052), (RX0, 0.052)],
               [0.003, 0.004, 0.002, 0.002], 2)
recv.prism(lower, -RHW, RHW, "Blued", axis="x", bevel=0.0012, segs=1)
rc = Part("cut")
# The cocking handle's slot along the right; the ejection port on top.
rc.box((0.01, 0.0085, 0.13), (RHW, 0.046, -0.07), "Blued")
rc.box((0.016, 0.02, 0.05), (0.004, 0.06, -0.16), "Blued", bevel=0.002, segs=1)
LEFT = T(-RHW + 0.0006, 0, 0) @ RY(-math.pi / 2)
text(rc, "ЛХ 5520", 0.0042, "Blued", LEFT @ T(-0.06, 0.04, 0.0), depth=0.0012)
recv.cut(rc)
cover = fillet([(-0.0165, 0.052), (0.0165, 0.052), (0.0165, 0.064), (0.0, 0.0705),
                (-0.0165, 0.064)], [0.0013, 0.0013, 0.006, 0.01, 0.006], 3)
recv.prism(cover, -0.135, RX1, "Blued", axis="z", bevel=0.0012, segs=1)
cc = Part("cut")
cc.box((0.018, 0.03, 0.05), (0.004, 0.06, -0.16), "Blued", bevel=0.002, segs=1)
recv.cut(cc)
# The cover's latch at the back; the hinge at the jacket.
recv.box((0.012, 0.01, 0.008), (0.0, 0.058, RX1 + 0.003), "Blued", bevel=0.0018)
recv.cyl(0.004, -0.019, 0.019, "Blued", segs=12, xf=T(0, 0.054, -0.205) @ RY(math.pi / 2))
# The L-shaped flip rear sight on the cover.
recv.box((0.012, 0.012, 0.004), (0.0, 0.074, -0.03), "Blued", bevel=0.001)
recv.box((0.012, 0.002, 0.016), (0.0, 0.069, -0.022), "Blued", bevel=0.0006)
# The magazine catch behind the well, and the well's housing under the
# receiver.
recv.box((0.014, 0.022, 0.006), (0.0, 0.014, WELL_Z + 0.035), "Blued", bevel=0.0015)
recv.prism(fillet([(WELL_Z - 0.03, 0.03), (WELL_Z + 0.03, 0.03), (WELL_Z + 0.03, 0.012),
                   (WELL_Z - 0.03, 0.012)], 0.003, 2), -0.016, 0.016, "Blued", axis="x",
           bevel=0.0012, segs=1)
# Trigger guard.
guard = catmull([(-0.075, 0.012), (-0.078, -0.004), (-0.066, -0.018), (-0.045, -0.02),
                 (-0.024, -0.016), (-0.012, -0.006), (-0.006, 0.0)], 4, closed=False)
recv.sweep(guard, section_rect(0.009, 0.003, 0.001), "Blued")

# --- Barrel jacket: a perforated tube, slanted at the front ---------------------------
jacket = Part("jacket")
jacket.tube(0.0175, 0.0155, -0.545, -0.2, "Blued", segs=28, xf=T(0, BORE, 0), chamfer=0.0008)
jc = Part("cut")
# Rows of long cooling slots round the jacket (not along its bottom).
for k in range(7):
    z = -0.255 - k * 0.032
    for a in (90, 140, 40, 200, -20):
        rot = RZ(math.radians(a), about=(0, BORE, 0))
        jc.box((0.012, 0.04, 0.02), (0.0, BORE + 0.02, z), "Blued", bevel=0.005, segs=2,
               xf=rot)
# The compensator: the jacket's front cut on a slant, open at the top.
# (A plane at 45 degrees, from the top at z -0.505 to the bottom at -0.54:
# the gas blows up and forward and pushes the muzzle down.)
jc.box((0.1, 0.1, 0.1), (0.0, BORE, -0.5725), "Blued",
       xf=RX(math.radians(45), about=(0, BORE, -0.5225)))
jacket.cut(jc)
barrel = Part("barrel")
barrel.lathe([(0.0039, MUZZLE + 0.016), (0.0075, MUZZLE + 0.016), (0.0082, -0.47),
              (0.0095, -0.21), (0.0, -0.21)], "Blued", segs=18, xf=T(0, BORE, 0))
# The front sight in its ears on the jacket's top, near the front.
barrel.prism(fillet([(-0.488, BORE + 0.015), (-0.47, BORE + 0.015), (-0.474, BORE + 0.028),
                     (-0.484, BORE + 0.028)], 0.0012, 2), -0.0014, 0.0014, "Blued", axis="x",
             bevel=0.0004, segs=1)
for sx in (-1, 1):
    barrel.prism(fillet([(-0.49, BORE + 0.014), (-0.468, BORE + 0.014), (-0.472, BORE + 0.029),
                         (-0.486, BORE + 0.029)], 0.0012, 2), sx * 0.008 - 0.0015,
                 sx * 0.008 + 0.0015, "Blued", axis="x", bevel=0.0004, segs=1)
# The muzzle's front bearing ring inside the compensator.
barrel.tube(0.0155, 0.0042, -0.505, -0.497, "Blued", segs=24, xf=T(0, BORE, 0))

# --- Moving parts ------------------------------------------------------------------------
bolt = Part("bolt", origin=(0.0, BORE, -0.1))
bolt.prism(fillet([(-0.0125, 0.036), (0.0125, 0.036), (0.0125, 0.058), (-0.0125, 0.058)],
                  0.002, 2), -0.205, -0.08, "Steel", axis="z", bevel=0.0012, segs=1)
bolt.box((0.022, 0.008, 0.012), (0.024, 0.046, -0.115), "Steel", bevel=0.0015)
bolt.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0066, 0.004), (0.0066, 0.01), (0.0052, 0.013),
            (0.0, 0.0135)], "Steel", segs=14, xf=T(0.034, 0.046, -0.115) @ RY(math.pi / 2))

trig = Part("trigger", origin=(0.0, 0.01, -0.035))
trig.sweep(catmull([(-0.037, 0.012), (-0.04, 0.002), (-0.038, -0.006), (-0.033, -0.01)], 3,
                   closed=False), section_rect(0.0048, 0.0032, 0.0011), "Blued")

SEL_PIVOT = (0.0, 0.01, -0.06)
selector = Part("selector", origin=SEL_PIVOT)
selector.prism(fillet([(-0.064, 0.012), (-0.056, 0.012), (-0.057, 0.0), (-0.063, 0.0)], 0.0012,
                      2), -0.0035, 0.0035, "Blued", axis="x", bevel=0.0005, segs=1)

join("static", stock, furn, recv, jacket, barrel, worn={})
for p in (bolt, trig, selector):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.08, 0.0))
empty("magwell", (0.0, 0.03, WELL_Z))

# Where a sling attaches: the swivel on the stock's left ahead of the well, and the slot through the butt.
empty("pt_sling_front", (-0.021, 0.006, -0.185))
empty("pt_sling_rear", (0.0, -0.063, 0.17))
report()
export("assets/models/guns/ppsh.glb")
