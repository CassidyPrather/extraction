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

# Laid out against two side photographs scaled to the published 843 mm
# (`docs/craft/guns/ppsh/README.md`).
BORE = 0.052
RX0, RX1 = -0.232, 0.012       # receiver front (the jacket's hinge) and rear
RHW = 0.0165
MUZZLE = -0.5143               # the compensator's face, on the bore
WELL_Z = -0.187                # the magazine well's centre
WOOD_F = -0.13                 # the stock's front: bare steel ahead of it


def screw(p, cutter, r, pos, xf, turn=0.6, mat="Blued", segs=12):
    """A domed slotted screw head of radius r at `pos`, its axis +Y turned
    by `xf`, its rim sunk 0.6 mm; `cutter` gets the slot, turned `turn`."""
    h = r * 0.55
    prof = [(0.0, -0.0006), (r, -0.0006)]
    prof += [(r * math.cos(math.radians(d)), h * math.sin(math.radians(d)))
             for d in (0, 30, 55, 75)]
    prof.append((0.0, h))
    p.lathe(prof, mat, segs=segs, xf=T(*pos) @ xf @ RX(-math.pi / 2))
    if cutter is not None:
        cutter.box((r * 2.4, h * 1.4, r * 0.32), (0.0, h, 0.0), mat,
                   xf=T(*pos) @ xf @ RY(turn))


def lerp_table(t, table):
    for (a, va), (b, vb) in zip(table, table[1:]):
        if a <= t <= b:
            k = (t - a) / (b - a)
            k = k * k * (3 - 2 * k)
            return va + (vb - va) * k
    return table[0][1] if t < table[0][0] else table[-1][1]


# --- Stock: wood from the magazine well back to the butt ---------------------------------
stock_prof = [
    (WOOD_F, 0.03), (0.0, 0.03),                # under the receiver
    (0.03, 0.026), (0.065, 0.013),              # wrist, down to the comb
    (0.095, 0.006), (0.145, 0.011),             # the comb
    (0.315, -0.006),                            # heel
    (0.315, -0.128),                            # toe
    (0.2, -0.095), (0.145, -0.071),             # belly
    (0.118, -0.061), (0.098, -0.066),           # the knob under the grip
    (0.075, -0.040), (0.04, -0.018),            # the grip, slim
    (0.0, -0.008), (-0.06, -0.004),             # behind the guard
    (WOOD_F + 0.008, -0.004), (WOOD_F, 0.008),
]
stock_prof = fillet(stock_prof, [0.004, 0.004, 0.012, 0.02, 0.02, 0.04, 0.0, 0.0, 0.04, 0.015,
                                 0.01, 0.012, 0.015, 0.01, 0.008, 0.006, 0.004, 0.004], 4)
WIDTH = [(WOOD_F, 0.02), (-0.02, 0.021), (0.03, 0.0165), (0.08, 0.017), (0.32, 0.021)]


def stock_w(z, yc, h):
    k = lerp_table(z, WIDTH)
    return (k - (0.0025 if z > 0.08 else 0.0), k * (0.95 if z < 0.0 else 0.84))


stock = Part("stock")
stock.stock(stock_prof, WOOD_F + 0.0005, 0.3145, "Wood", stock_w, stations=110, segs=28,
            power=lambda z: 3.4 if z < 0.0 else 2.5)
sc = Part("cut")
# The sling slot in the butt.
sc.prism(fillet([(0.15, -0.055), (0.19, -0.063), (0.19, -0.071), (0.15, -0.063)], 0.0035, 2),
         -0.04, 0.04, "Wood", axis="x")
stock.cut(sc)
furn = Part("furniture")
bp = fillet([(0.3145, -0.0045), (0.324, -0.0055), (0.324, -0.129), (0.3145, -0.1295)],
            [0.0, 0.002, 0.002, 0.0], 2)
furn.prism(bp, -0.0215, 0.0215, "Blued", axis="x", bevel=0.0015, segs=1)
furn.box((0.024, 0.05, 0.004), (0.0, -0.05, 0.3255), "Blued", bevel=0.0012)
# The trap's hinge at its foot and its latch button at the top.
furn.cyl(0.0022, -0.013, 0.013, "Blued", segs=12, chamfer=0.0005,
         xf=T(0, -0.0755, 0.3265) @ RY(math.pi / 2))
furn.lathe([(0.0, 0.0), (0.0034, 0.0), (0.0034, 0.0014), (0.0026, 0.0024), (0.0, 0.0026)],
           "Steel", segs=14, xf=T(0, -0.028, 0.327))
screws = Part("screws")
slots = Part("cut")
# The butt plate's two screws, into the heel and the toe.
for y in (-0.016, -0.114):
    screw(screws, slots, 0.0034, (0.0, y, 0.324), RX(math.pi / 2), turn=0.5 - y * 11)
# The front sling swivel on the jacket's left, under its slots.
SWIVEL = (-0.019, BORE - 0.0125, -0.35)
swivel = [(SWIVEL[2] + 0.009 * math.cos(a), SWIVEL[1] + 0.009 * math.sin(a))
          for a in [math.pi + math.pi * i / 14 for i in range(15)]]
furn.sweep(swivel, section_rect(0.0025, 0.0025, 0.0007, 3), "Blued", xf=T(SWIVEL[0], 0, 0))
furn.box((0.008, 0.006, 0.022), (-0.0155, SWIVEL[1], SWIVEL[2]), "Blued", bevel=0.001)

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
# It runs on from the jacket, the ejection port cut in it.
recv.prism(cover, RX0 + 0.006, RX1, "Blued", axis="z", bevel=0.0012, segs=1)
cc = Part("cut")
cc.box((0.018, 0.03, 0.05), (0.004, 0.06, -0.16), "Blued", bevel=0.002, segs=1)
# Serial and year stamped on the cover's upper left, where the shooter
# sees them, and the notch in the standing sight leaf. (The cover's
# rounded edges lean its sides in by up to 1.2 mm toward the front; the
# letters follow.)
SLOPE = math.atan2(0.0065, 0.0165)
for s_, z in (("ПГ 3047", -0.098), ("1943", -0.068)):
    sink = 0.0012 * (RX1 - 0.0012 - z) / (RX1 - 0.0012 + 0.135) + 0.0006  # (lean as drawn)
    at = T(-0.0088 + sink * math.sin(SLOPE), 0.0705 - 0.0088 * math.tan(SLOPE)
           - sink * math.cos(SLOPE), z) @ RZ(SLOPE) @ RY(-math.pi / 2) @ RX(-math.pi / 2)
    text(cc, s_, 0.0034, "Blued", at, depth=0.0012)
cc.box((0.0024, 0.006, 0.006), (0.0, 0.0805, -0.03), "Blued")
recv.cut(cc)
# The cover's latch at the back; the hinge at the jacket.
recv.box((0.012, 0.01, 0.008), (0.0, 0.058, RX1 + 0.003), "Blued", bevel=0.0018, segs=2)
# Its push button, ribbed for the thumb.
recv.lathe([(0.0, 0.0), (0.0034, 0.0), (0.0034, 0.0022), (0.0027, 0.003), (0.0, 0.0031)],
           "Steel", segs=16, xf=T(0, 0.058, RX1 + 0.0068))
for i in range(3):
    recv.box((0.011, 0.0008, 0.0008), (0.0, 0.0632, RX1 + 0.0004 + i * 0.0022),
             "Blued", bevel=0.0002, segs=1)
recv.cyl(0.004, -0.019, 0.019, "Blued", segs=20, xf=T(0, 0.054, RX0) @ RY(math.pi / 2))
# The hinge pin's peened heads.
for sx in (-1, 1):
    recv.lathe([(0.0, 0.0), (0.0026, 0.0), (0.0024, 0.0008), (0.0014, 0.0014), (0.0, 0.0015)],
               "Steel", segs=12, xf=T(sx * 0.0188, 0.054, RX0) @ RY(sx * math.pi / 2))
# Rivets along the foot of the receiver's sides.
for sx in (-1, 1):
    for z in (-0.215, -0.105, -0.012):
        recv.lathe([(0.0, -0.0005), (0.0019, -0.0005), (0.0019, 0.0002), (0.0013, 0.0009),
                    (0.0, 0.0011)], "Blued", segs=10,
                   xf=T(sx * RHW, 0.036, z) @ RY(sx * math.pi / 2))
# The L-shaped flip rear sight on the cover.
recv.box((0.012, 0.012, 0.004), (0.0, 0.074, -0.03), "Blued", bevel=0.001, segs=2)
recv.box((0.012, 0.002, 0.016), (0.0, 0.069, -0.022), "Blued", bevel=0.0006, segs=2)
# The leaves' pivot, and the ears pressed up either side to guard them.
recv.cyl(0.0017, -0.0075, 0.0075, "Steel", segs=12, chamfer=0.0004,
         xf=T(0, 0.0695, -0.0305) @ RY(math.pi / 2))
for sx in (-1, 1):
    recv.prism(fillet([(-0.036, 0.066), (-0.02, 0.066), (-0.022, 0.0775), (-0.034, 0.0775)],
                      [0.0, 0.0, 0.0025, 0.0025], 3),
               sx * 0.0095 - 0.001, sx * 0.0095 + 0.001, "Blued", axis="x", bevel=0.0004,
               segs=1)
# The magazine catch behind the well, and the well's housing under the
# receiver.
recv.box((0.014, 0.018, 0.006), (0.0, 0.001, WELL_Z + 0.027), "Blued", bevel=0.0015)

housing = Part("housing")
# The receiver's front, bare steel from the hinge back to the wood, and the
# magazine well's housing standing below it.
housing.prism(fillet([(RX0, 0.03), (WOOD_F + 0.004, 0.03), (WOOD_F + 0.004, 0.004),
                      (RX0 + 0.012, 0.004)], [0.002, 0.0, 0.002, 0.006], 2), -RHW, RHW,
              "Blued", axis="x", bevel=0.0012, segs=1)
housing.prism(fillet([(WELL_Z - 0.024, 0.01), (WELL_Z + 0.024, 0.01), (WELL_Z + 0.024, -0.008),
                      (WELL_Z - 0.024, -0.008)], 0.003, 2), -0.0175, 0.0175, "Blued",
              axis="x", bevel=0.0012, segs=1)
# The drum's feed tower goes up through it (it stood through solid steel:
# `capture::seated`): 21 x 38 mm, 0.6 mm clear.
hc = Part("cut")
hc.box((0.0222, 0.048, 0.0392), (0.0, 0.012, WELL_Z - 0.005), "Blued")
housing.cut(hc)
# Trigger guard.
# Long and deep, its bottom straight.
guard = catmull([(-0.078, 0.012), (-0.081, -0.012), (-0.075, -0.029), (-0.06, -0.034),
                 (-0.02, -0.034), (-0.006, -0.028), (-0.002, -0.012), (0.0, 0.0)], 6,
                closed=False)
recv.sweep(guard, section_rect(0.009, 0.003, 0.001, 3), "Blued")

# --- Barrel jacket: a perforated tube, slanted at the front ---------------------------
jacket = Part("jacket")
jacket.tube(0.0175, 0.0155, MUZZLE - 0.012, RX0 + 0.006, "Blued", segs=40, xf=T(0, BORE, 0),
            chamfer=0.0008)
# The compensator's face, closed but for the bullet's hole: the jacket's
# front cut 15 degrees off upright, its top lip forward, so the gas
# striking the lip pushes the muzzle down.
FACE = RX(math.radians(-15), about=(0, BORE, MUZZLE))
jacket.cyl(0.0182, MUZZLE, MUZZLE + 0.0016, "Blued", segs=40, xf=FACE @ T(0, BORE, 0))
jc = Part("cut")
# Four long cooling slots down each side.
for z in (-0.452, -0.395, -0.339, -0.285):
    for a in (-90, 90):
        rot = RZ(math.radians(a), about=(0, BORE, 0))
        jc.box((0.014, 0.04, 0.048), (0.0, BORE + 0.02, z), "Blued", bevel=0.0065, segs=3,
               xf=rot)
jc.box((0.1, 0.1, 0.1), (0.0, BORE, MUZZLE - 0.05), "Blued", xf=FACE)
jc.cyl(0.0045, MUZZLE - 0.01, MUZZLE + 0.01, "Blued", segs=20, xf=T(0, BORE, 0))
# A round port through either side, behind the face.
jc.cyl(0.0075, -0.03, 0.03, "Blued", segs=24, xf=T(0, BORE, -0.497) @ RY(math.pi / 2))
jacket.cut(jc)
barrel = Part("barrel")
# 269 mm, its muzzle 35 mm behind the compensator's face.
barrel.lathe([(0.0039, -0.479), (0.0075, -0.479), (0.0082, -0.45),
              (0.0095, -0.21), (0.0, -0.21)], "Blued", segs=24, xf=T(0, BORE, 0))
# The front sight in its ears on the jacket's top, near the front.
FS = 0.037  # moved back from where it was drawn, to the photographs'
barrel.prism(fillet([(-0.488 + FS, BORE + 0.015), (-0.47 + FS, BORE + 0.015),
                     (-0.474 + FS, BORE + 0.028), (-0.484 + FS, BORE + 0.028)], 0.0012, 2),
             -0.0014, 0.0014, "Blued", axis="x", bevel=0.0004, segs=1)
for sx in (-1, 1):
    barrel.prism(fillet([(-0.49 + FS, BORE + 0.014), (-0.468 + FS, BORE + 0.014),
                         (-0.472 + FS, BORE + 0.029), (-0.486 + FS, BORE + 0.029)], 0.0012, 2),
                 sx * 0.008 - 0.0015,
                 sx * 0.008 + 0.0015, "Blued", axis="x", bevel=0.0004, segs=1)
# The muzzle's bearing ring, holding it in the jacket.
barrel.tube(0.0155, 0.0042, -0.483, -0.475, "Blued", segs=40, xf=T(0, BORE, 0))

# --- Moving parts ------------------------------------------------------------------------
bolt = Part("bolt", origin=(0.0, BORE, -0.1))
bolt.prism(fillet([(-0.0125, 0.036), (0.0125, 0.036), (0.0125, 0.058), (-0.0125, 0.058)],
                  0.002, 2), -0.205, -0.08, "Steel", axis="z", bevel=0.0012, segs=1)
# The bolt's face, its firing pin and the extractor along the right, seen
# through the ejection port.
bolt.lathe([(0.0, -0.2065), (0.0012, -0.2065), (0.0016, -0.2052), (0.0016, -0.204)], "Steel",
           segs=10, xf=T(0, BORE, 0))
bolt.box((0.0025, 0.004, 0.035), (0.0118, 0.052, -0.19), "Steel", bevel=0.0006, segs=1)
bolt.box((0.022, 0.008, 0.012), (0.024, 0.046, -0.115), "Steel", bevel=0.0015, segs=2)
# The handle's knob, knurled, and the safety catch that slides in from it
# into a notch in the receiver.
bolt.lathe([(0.0, 0.0), (0.0055, 0.0), (0.0066, 0.004), (0.0066, 0.01), (0.0052, 0.013),
            (0.0, 0.0135)], "Steel", segs=22, xf=T(0.034, 0.046, -0.115) @ RY(math.pi / 2))
for i in range(14):
    a = 2 * math.pi * i / 14
    c = (0.0415, 0.046 + 0.0066 * math.cos(a), -0.115 + 0.0066 * math.sin(a))
    bolt.box((0.0056, 0.0007, 0.0007), c, "Steel", xf=RX(a + math.pi / 4, about=c))
bolt.box((0.0075, 0.0028, 0.008), (0.0245, 0.0514, -0.115), "Steel", bevel=0.0007, segs=2)
for i in range(3):
    bolt.box((0.0065, 0.0008, 0.0009), (0.0245, 0.0528, -0.1175 + i * 0.0025), "Steel",
             bevel=0.0002, segs=1)

# Its origin on its pin: the game turns it about it (`Moves::Trigger`).
trig = Part("trigger", origin=(0.0, 0.018, -0.037))
trig.sweep(catmull([(-0.037, 0.012), (-0.04, 0.0), (-0.038, -0.012), (-0.032, -0.019)], 5,
                   closed=False), section_rect(0.0048, 0.0032, 0.0011, 3), "Blued")

SEL_PIVOT = (0.0, 0.01, -0.06)
selector = Part("selector", origin=SEL_PIVOT)
selector.prism(fillet([(-0.064, 0.012), (-0.056, 0.012), (-0.057, 0.0), (-0.063, 0.0)], 0.0012,
                      3), -0.0035, 0.0035, "Blued", axis="x", bevel=0.0005, segs=2)
# Ribs across its face for the trigger finger to push it.
for i in range(3):
    selector.box((0.0062, 0.0008, 0.0008), (0.0, 0.0018 + i * 0.0022, -0.0632 + i * 0.00017),
                 "Blued", bevel=0.0002, segs=1)

screws.cut(slots)
join("static", stock, furn, recv, housing, jacket, barrel, screws, worn={})
for p in (bolt, trig, selector):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, 0.08, 0.0))
empty("magwell", (0.0, 0.026, WELL_Z))

# Where a sling attaches: the swivel on the jacket's left, and the slot through the butt.
empty("pt_sling_front", (SWIVEL[0], SWIVEL[1] - 0.009, SWIVEL[2]))
empty("pt_sling_rear", (0.0, -0.063, 0.17))
report()
export("assets/models/guns/ppsh.glb")
