"""Ammunition: loose rounds of each calibre, the boxes they come in, and the
robots' linked belt. Writes assets/models/items/<id>.glb for each (item
space: see _kit.py).

Loose rounds lie along Z, bullet to -Z, in their `dims` box. The boxes
are told apart at a glance by packing as much as by their stencilled
calibre: 9x18 in a kraft card box torn open on the noses, 5.45x39 in an
olive paper packet tied with string, 7.62x54R in a white carton torn on
the rims, 12 gauge in a yellow carton showing the brass heads. The belt
has no `dims`, so it lies folded in a cube of side cbrt(bulk / 1000).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ON_TOP, UP, cube, item, panel, ragged, text, wanted
from common import Part, RX, RY, RZ, T, S, fillet

# Cartridge profiles, (r, z) with the base at z 0 and the bullet to -z:
# (case, bullet, overall length).
C9 = ([(0.0, 0.0), (0.0043, 0.0), (0.00475, -0.0005), (0.00475, -0.0011), (0.0041, -0.0014),
       (0.0041, -0.0021), (0.00475, -0.0026), (0.0047, -0.0181), (0.0, -0.0181)],
      [(0.0, -0.0175), (0.00455, -0.0175), (0.00455, -0.0195), (0.0041, -0.0218),
       (0.0031, -0.0236), (0.0016, -0.0247), (0.0, -0.025)], 0.025)
C545 = ([(0.0, 0.0), (0.0045, 0.0), (0.005, -0.0005), (0.005, -0.0012), (0.0041, -0.0015),
         (0.0041, -0.0024), (0.0049, -0.003), (0.0046, -0.0315), (0.0032, -0.0345),
         (0.0031, -0.0398), (0.0, -0.0398)],
        [(0.0, -0.039), (0.0028, -0.039), (0.0028, -0.0445), (0.0025, -0.048), (0.0018, -0.052),
         (0.0009, -0.0553), (0.0, -0.057)], 0.057)
C762 = ([(0.0, 0.0), (0.0058, 0.0), (0.00625, -0.0004), (0.00625, -0.0016), (0.0057, -0.0019),
         (0.0059, -0.004), (0.0055, -0.04), (0.0042, -0.0445), (0.0041, -0.0535), (0.0, -0.0535)],
        [(0.0, -0.052), (0.0039, -0.052), (0.0039, -0.06), (0.0034, -0.066), (0.0024, -0.071),
         (0.0012, -0.075), (0.0, -0.077)], 0.077)


def split(prof, zc):
    """Cut a lathe profile (axis to axis, base to point, z falling) at z
    `zc` into the part behind and the part ahead."""
    i = next(k for k, (r, z) in enumerate(prof) if z < zc)
    (r0, z0), (r1, z1) = prof[i - 1], prof[i]
    r = r0 + (r1 - r0) * (zc - z0) / (z1 - z0)
    return prof[:i] + [(r, zc), (0.0, zc)], [(0.0, zc), (r, zc)] + prof[i:]


# Tip paint by type: the point painted back this far, a band behind it.
TIP, BAND = 0.0048, 0.0022
TIPS = {
    "ap": [(TIP, "Black")],                         # 7N22: black
    "api": [(TIP + BAND, "Red"), (TIP, "Black")],    # BZ, B-32: black over red
    "tracer": [(TIP, "Leaf")],                       # 7T3, T-45, T-46: green
}


def bullet(p, c, xf, segs, tip=None, seat=0.0):
    """The bullet, copper, its point painted for its `tip` type; `seat`
    moves it along the case (a hand-loaded round's odd seating)."""
    prof = c[1]
    xf = xf @ T(0, 0, seat)
    if not tip:
        p.lathe(prof, "Copper", segs=segs, xf=xf)
        return
    end = prof[-1][1]
    mat = "Copper"
    for back, m in TIPS[tip]:
        behind, prof = split(prof, end + back)
        p.lathe(behind, mat, segs=segs, xf=xf)
        mat = m
    p.lathe(prof, mat, segs=segs, xf=xf)


def cartridge(p, c, xf, segs=14, base=True, tip=None, brass="Brass", seat=0.0):
    """A brass case and copper bullet, base at z 0 (then `xf`); with
    `base`, the primer and its red lacquer seal; `tip` paints the point
    (ap, api, tracer); `brass` and `seat` for re-packed rounds."""
    case, _, _ = c
    p.lathe(case, brass, segs=segs, xf=xf)
    bullet(p, c, xf, segs, tip, seat)
    if base:
        p.cyl(0.0017, -0.0004, 0.00003, "Copper", segs=10, xf=xf)
        p.tube(0.0023, 0.0017, -0.0004, 0.00002, "Red", segs=10, xf=xf)


def nose(p, c, z_from, xf, segs=10, tip=None, brass="Brass", seat=0.0):
    """Only the front of a cartridge from `z_from` on (what shows out of a
    box), base at z 0 (then `xf`)."""
    case = c[0]
    front = [(r, z) for r, z in case if z <= z_from and r > 0]
    p.lathe([(0.0, z_from), (front[0][0], z_from)] + front + [(0.0, front[-1][1])], brass,
            segs=segs, xf=xf)
    bullet(p, c, xf, segs, tip, seat)


def carton(p, sx, sy, sz, y0, mat, top=None, wall=0.0012, sides=True):
    """An open-topped card box on the floor y0: floor and four walls (the
    two at +-X left out without `sides`). With `top` (z0, z1) a lid panel
    over that stretch."""
    p.box((sx, wall, sz), (0.0, y0 + wall / 2, 0.0), mat, bevel=0.0004, segs=1)
    for sx_ in (-1, 1) if sides else ():
        p.box((wall, sy, sz), (sx_ * (sx - wall) / 2, y0 + sy / 2, 0.0), mat, bevel=0.0004, segs=1)
    for sz_ in (-1, 1):
        p.box((sx - 2 * wall, sy, wall), (0.0, y0 + sy / 2, sz_ * (sz - wall) / 2), mat,
              bevel=0.0004, segs=1)
    if top:
        z0, z1 = top
        p.box((sx, wall, z1 - z0), (0.0, y0 + sy - wall / 2, (z0 + z1) / 2), mat, bevel=0.0004,
              segs=1)


# --- Loose rounds ----------------------------------------------------------------------------
C762X39 = ([(0.0, 0.0), (0.0053, 0.0), (0.0056, -0.0005), (0.0056, -0.0013), (0.0047, -0.0016),
            (0.0047, -0.0026), (0.0056, -0.0032), (0.0052, -0.0295), (0.0043, -0.0325),
            (0.0042, -0.0387), (0.0, -0.0387)],
           [(0.0, -0.037), (0.0039, -0.037), (0.0039, -0.044), (0.0035, -0.049), (0.0024, -0.053),
            (0.0011, -0.0555), (0.0, -0.056)], 0.056)

# The pistol and submachine-gun round: a short bottlenecked case.
C762X25 = ([(0.0, 0.0), (0.0049, 0.0), (0.0049, -0.0011), (0.0042, -0.0014), (0.0042, -0.0022),
            (0.0049, -0.0026), (0.0048, -0.0205), (0.0043, -0.0215), (0.0042, -0.025),
            (0.0, -0.025)],
           [(0.0, -0.0235), (0.004, -0.0235), (0.004, -0.028), (0.0033, -0.0315), (0.0019, -0.0338),
            (0.0, -0.0345)], 0.0345)

# The Nagant's gas-seal round: the bullet sits wholly inside the case, the
# crimped mouth just showing its flat nose.
C762X38R = ([(0.0, 0.0), (0.0049, 0.0), (0.0049, -0.0011), (0.0039, -0.0013), (0.0039, -0.0355),
             (0.0033, -0.0385), (0.0, -0.0385)],
            [(0.0, -0.0383), (0.0026, -0.0383), (0.0021, -0.0389), (0.0, -0.039)], 0.039)


def round_9x18():
    p = Part("body")
    # A stubby pistol round: straight brass case, round-nosed copper bullet.
    cartridge(p, C9, T(0, 0, C9[2] / 2), segs=14)
    return [p]


def round_545():
    p = Part("body")
    # A slim bottlenecked rifle round with a long pointed bullet.
    cartridge(p, C545, T(0, 0, C545[2] / 2), segs=14)
    return [p]


def round_762r():
    p = Part("body")
    # The big rimmed rifle round: tapered case, a rim at the base, long bullet.
    cartridge(p, C762, T(0, 0, C762[2] / 2), segs=16)
    return [p]


def shell_12ga(hull="Red", head="Brass"):
    p = Part("body")
    # A red plastic hull on a brass head, the star crimp closing the front.
    xf = T(0, 0, 0.035)
    p.lathe([(0.0, 0.0), (0.0099, 0.0), (0.01, -0.0004), (0.01, -0.0013), (0.0093, -0.0016),
             (0.0093, -0.012), (0.0091, -0.0125), (0.0, -0.0125)], head, segs=20, xf=xf)
    p.lathe([(0.0, -0.011), (0.0091, -0.011), (0.0091, -0.066), (0.0085, -0.0685),
             (0.006, -0.0697), (0.0, -0.07)], hull, segs=20, xf=xf)
    for k in range(6):
        p.box((0.0006, 0.0055, 0.0008), (0.0, 0.003, -0.0693), "Dark",
              xf=xf @ RZ(math.pi * k / 3))
    p.cyl(0.0028, -0.0004, 0.00003, "Steel", segs=12, xf=xf)
    # Head stamp ring.
    p.tube(0.0068, 0.0062, -0.0002, 0.00002, "Copper", segs=16, xf=xf)
    return [p]


# --- Boxes -----------------------------------------------------------------------------------

def ammo_9x18():
    p = Part("body")
    # A small kraft card box of pistol rounds standing nose up, the front
    # of the lid torn off; a printed label on what is left.
    sx, sy, sz = 0.04, 0.03, 0.07
    y0 = -sy / 2
    yl = y0 + sy - 0.001
    carton(p, sx, sy - 0.0022, sz, y0, "Rope")
    # What is left of the lid, its label printed on it.
    panel(p, "y", -sx / 2, sx / 2, -0.004, sz / 2, yl - 0.0012, yl, "Rope",
          [(-0.016, 0.016, 0.001, 0.031, "Paper"), (-0.016, 0.016, 0.024, 0.028, "Red")])
    ragged(p, sx - 0.001, -0.004, 0.006, yl - 0.0012, yl, "Rope", seed=2)
    p.box((sx - 0.003, 0.001, 0.034), (0.0, y0 + 0.0125, -0.0175), "Black")
    for i in range(4):
        for j in range(3):
            x = -0.0143 + i * 0.00953
            z = -0.0295 + j * 0.0095
            nose(p, C9, -0.012, T(x, y0 + 0.0013, z) @ RX(math.pi / 2), segs=8)
    text(p, "9x18", 0.0065, "Black", T(0.0, yl, 0.012) @ ON_TOP,
         depth=0.0004)
    # The calibre stamped on both sides too.
    for sx_ in (-1, 1):
        text(p, "9x18", 0.008, "Black",
             T(sx_ * (sx / 2), 0.0, 0.0) @ RY(sx_ * math.pi / 2), depth=0.0004)
    return [p]


def ammo_545(label="5.45", tip_of=lambda k: None, stripe=None):
    p = Part("body")
    # A packet of rifle rounds wrapped in olive paper and tied with string,
    # one end torn open on the slim pointed noses.
    sx, sy, sz = 0.05, 0.035, 0.1
    zt = -0.03
    body = fillet([(-0.0245, -0.017), (0.0245, -0.017), (0.0245, 0.017), (-0.0245, 0.017)],
                  0.003, 2)
    p.prism([(x, y) for x, y in body], zt + 0.0035, sz / 2 - 0.0005, "Olive", axis="z",
            bevel=0.002, segs=1)
    # Torn paper round the opening, the dark between the rounds inside.
    zp = zt + 0.0035
    ragged(p, 0.047, zp, 0.0115, 0.0162, 0.017, "Olive", seed=1)
    ragged(p, 0.047, zp, 0.0105, -0.017, -0.0162, "Olive", seed=4)
    for sx_ in (-1, 1):
        ragged(p, 0.032, zp, 0.0105, -0.0004, 0.0004, "Olive", seed=3 + sx_,
               xf=T(sx_ * 0.0241, 0.0, 0.0) @ RZ(math.pi / 2))
    p.box((0.046, 0.031, 0.0035), (0.0, 0.0, zt + 0.00175), "Black")
    for i in range(4):
        for j in range(3):
            x = -0.0158 + i * 0.0105
            y = -0.0105 + j * 0.0105
            nose(p, C545, -0.028, T(x, y, -sz / 2 + C545[2]), segs=8, tip=tip_of(i * 3 + j))
    # String round it, both ways.
    for z in (-0.005, 0.03):
        p.box((0.0505, 0.0015, 0.0015), (0.0, 0.0172, z), "Rope")
        p.box((0.0505, 0.0015, 0.0015), (0.0, -0.0172, z), "Rope")
        for sx_ in (-1, 1):
            p.box((0.0015, 0.0345, 0.0015), (sx_ * 0.0248, 0.0, z), "Rope")
    p.box((0.0016, 0.0015, 0.0365), (0.0, 0.0174, 0.0125), "Rope")
    # A paper slip under the string with the calibre on it.
    p.box((0.03, 0.0004, 0.022), (-0.008, 0.0171, 0.0125), "Paper", xf=RY(0.04))
    text(p, label, 0.006, "Black",
         T(-0.008, 0.0174, 0.0125) @ ON_TOP, depth=0.0004)
    if stripe:
        # The type's stripe across the slip's foot.
        p.box((0.03, 0.0004, 0.004), (-0.008, 0.0175, 0.003), stripe, xf=RY(0.04))
    return [p]


def round_762x39():
    p = Part("body")
    # The intermediate round: a short tapered case, a pointed bullet.
    cartridge(p, C762X39, T(0, 0, C762X39[2] / 2), segs=14)
    return [p]


def ammo_762x39(label="7.62x39", tip=None, band="Olive"):
    p = Part("body")
    # A cream card box of twenty standing on end, two rows of ten, its lid
    # torn off over the bullets; a green band and the calibre on the sides.
    sx, sy, sz = 0.06, 0.03, 0.026
    y0 = -0.0235
    wall = 0.0012
    carton(p, sx, sy, sz, y0, "Cream", wall=wall, sides=False)
    for sx_ in (-1, 1):
        panel(p, "x", -sz / 2, sz / 2, y0, y0 + sy, sx_ * (sx / 2 - wall), sx_ * sx / 2,
              "Cream", [(-sz / 2, sz / 2, y0 + 0.007, y0 + 0.013, band)])
    ragged(p, sx - 0.003, -sz / 2 + wall, 0.004, y0 + sy - 0.0012, y0 + sy, "Cream", seed=7)
    # The dark between the rounds, under the tops of the cases.
    p.box((sx - 0.004, 0.003, sz - 0.004), (0.0, y0 + sy - 0.006, 0.0), "Black")
    for i in range(10):
        for j in range(2):
            x = -0.0255 + i * 0.00567
            z = -0.0058 + j * 0.0116
            base = y0 + sy - 0.006 - 0.034
            nose(p, C762X39, -0.034, T(x, base, z) @ RX(math.pi / 2), segs=8, tip=tip)
    for sz_ in (-1, 1):
        rot = RY(math.pi) if sz_ < 0 else T()
        text(p, label, 0.0075, "Black", T(0.0, y0 + 0.012, sz_ * (sz / 2 + 0.0001)) @ rot,
             depth=0.0004)
    return [p]


def round_762x38r():
    p = Part("body")
    # The Nagant round: a long straight case, the bullet hidden inside it.
    cartridge(p, C762X38R, T(0, 0, C762X38R[2] / 2), segs=12)
    return [p]


def ammo_762x38r():
    p = Part("body")
    # A brown paper packet of fourteen, two rows of seven lying flat, one
    # end torn open on the crimped mouths; the calibre in black.
    sx, sy, sz = 0.072, 0.022, 0.042
    zt = -0.017
    body = fillet([(-sx / 2, -sy / 2), (sx / 2, -sy / 2), (sx / 2, sy / 2), (-sx / 2, sy / 2)],
                  0.003, 2)
    p.prism(body, zt + 0.003, sz / 2 - 0.0005, "Rope", axis="z", bevel=0.002, segs=1)
    zp = zt + 0.003
    ragged(p, sx - 0.003, zp, 0.009, sy / 2 - 0.0008, sy / 2, "Rope", seed=2)
    ragged(p, sx - 0.003, zp, 0.008, -sy / 2, -sy / 2 + 0.0008, "Rope", seed=5)
    p.box((sx - 0.004, sy - 0.003, 0.003), (0.0, 0.0, zt + 0.0015), "Black")
    for i in range(7):
        for j in range(2):
            x = -0.03 + i * 0.01
            y = -0.0049 + j * 0.0098
            nose(p, C762X38R, -0.031, T(x, y, -sz / 2 + C762X38R[2] - 0.004), segs=8)
    p.box((0.05, 0.0004, 0.02), (0.0, sy / 2 + 0.0001, 0.006), "Paper")
    text(p, "7.62 НАГАН", 0.005, "Black", T(0.0, sy / 2 + 0.0003, 0.006) @ ON_TOP,
         depth=0.0004)
    return [p]


def round_762x25():
    p = Part("body")
    # The Tokarev round: a short bottlenecked case, a round-nosed bullet.
    cartridge(p, C762X25, T(0, 0, C762X25[2] / 2), segs=12)
    return [p]


def ammo_762x25():
    p = Part("body")
    # A grey card box of seventy, ten by seven on end, half its lid torn off;
    # the calibre stamped on the sides.
    sx, sy, sz = 0.1, 0.03, 0.07
    y0 = -0.0245
    wall = 0.0012
    carton(p, sx, sy, sz, y0, "Pebble", wall=wall, sides=False, top=None)
    # Half the lid still on, over the rounds you haven't taken.
    p.box((sx / 2, wall, sz), (sx / 4, y0 + sy - wall / 2 + 0.0004, 0.0), "Pebble", bevel=0.0004,
          segs=1)
    for sx_ in (-1, 1):
        panel(p, "x", -sz / 2, sz / 2, y0, y0 + sy, sx_ * (sx / 2 - wall), sx_ * sx / 2,
              "Pebble", [(-sz / 2, sz / 2, y0 + 0.006, y0 + 0.011, "Red")])
    ragged(p, sx / 2 - 0.003, -sz / 2 + wall, 0.004, y0 + sy - 0.0012, y0 + sy, "Pebble", seed=9,
           xf=T(-sx / 4, 0, 0))
    p.box((sx / 2 - 0.004, 0.003, sz - 0.004), (-sx / 4, y0 + sy - 0.006, 0.0), "Black")
    for i in range(5):
        for j in range(7):
            x = -0.045 + i * 0.01
            z = -0.03 + j * 0.01
            base = y0 + sy - 0.006 - 0.022
            nose(p, C762X25, -0.022, T(x, base, z) @ RX(math.pi / 2), segs=7)
    for sz_ in (-1, 1):
        rot = RY(math.pi) if sz_ < 0 else T()
        text(p, "7.62x25", 0.0075, "Black", T(0.0, y0 + 0.017, sz_ * (sz / 2 + 0.0001)) @ rot,
             depth=0.0004)
    return [p]


def ammo_762r(label="54R", band="Red"):
    p = Part("body")
    # A white card carton of the big rimmed rounds lying in two layers,
    # its back end torn off on the brass rims; a red band and the calibre.
    sx, sy, sz = 0.045, 0.03, 0.09
    zt = 0.03
    ze = zt - 0.0035
    for z0, z1, mat in ((-sz / 2, -0.018, "Paper"), (-0.018, -0.006, band), (-0.006, ze, "Paper")):
        p.box((sx - 0.001, sy - 0.0005, z1 - z0), (0.0, -0.00025, (z0 + z1) / 2), mat,
              bevel=0.0006, segs=1)
    for y0, y1 in ((sy / 2 - 0.0013, sy / 2 - 0.0005), (-sy / 2, -sy / 2 + 0.0008)):
        ragged(p, sx - 0.001, ze, -0.0105, y0, y1, "Paper", seed=2)
    for sx_ in (-1, 1):
        ragged(p, sy - 0.003, ze, -0.0105, -0.0004, 0.0004, "Paper", seed=5 + sx_,
               xf=T(sx_ * (sx / 2 - 0.0009), -0.00025, 0.0) @ RZ(math.pi / 2))
    p.box((sx - 0.003, sy - 0.003, 0.0035), (0.0, -0.00025, ze + 0.00175), "Black")
    for i in range(3):
        for j in range(2):
            x = -0.0138 + i * 0.0138
            y = -0.0072 + j * 0.0136
            xf = T(x, y, sz / 2 - 0.0004)
            p.lathe([(0.0, -0.016), (0.0057, -0.016)]
                    + [(r, z) for r, z in C762[0] if -0.016 < z and r > 0][::-1]
                    + [(0.0, 0.0)], "Brass", segs=14, xf=xf)
            p.cyl(0.0017, -0.0004, 0.00003, "Copper", segs=10, xf=xf)
            p.tube(0.0023, 0.0017, -0.0004, 0.00002, "Red", segs=10, xf=xf)
    text(p, "7.62", 0.008, "Black", T(0.0, sy / 2 - 0.0005, -0.033) @ ON_TOP,
         depth=0.0004)
    for sx_ in (-1, 1):
        text(p, label, 0.0055, "Black",
             T(sx_ * (sx / 2 - 0.0005), 0.0, -0.032) @ RY(sx_ * math.pi / 2), depth=0.0004)
    return [p]


def ammo_12ga(hull="Red", label=None):
    p = Part("body")
    # A yellow carton of shells standing on end, the lid torn back off the
    # front rows: brass heads with their primers, red hulls between.
    sx, sy, sz = 0.07, 0.07, 0.12
    y0 = -sy / 2
    wall = 0.0015
    yl = y0 + sy - 0.001
    carton(p, sx, sy - 0.001 - wall, sz, y0, "Yellow", wall=wall, sides=False)
    # The sides and what is left of the lid, the printing part of the card.
    for sx_ in (-1, 1):
        panel(p, "x", -sz / 2, sz / 2, y0, yl - wall, sx_ * (sx / 2 - wall), sx_ * sx / 2, "Yellow",
              [(-0.05, 0.05, -0.025, 0.025, "Paper"), (-0.05, 0.05, -0.022, -0.014, "Red")])
    panel(p, "y", -sx / 2, sx / 2, 0.0, sz / 2, yl - wall, yl, "Yellow",
          [(-0.03, 0.03, 0.006, 0.056, "Paper"), (-0.03, 0.03, 0.046, 0.054, "Red"),
           (0.014, 0.022, 0.013, 0.036, "Red"), (0.0135, 0.0225, 0.036, 0.042, "Brass")])
    ragged(p, sx - 0.001, 0.0, 0.01, yl - wall, yl, "Yellow", seed=3)
    p.box((sx - 0.004, 0.001, 0.058), (0.0, y0 + 0.052, -0.03), "Black")
    yt = y0 + sy - 0.0035
    for i in range(3):
        for j in range(3):
            x = -0.021 + i * 0.021
            z = -0.0475 + j * 0.0205
            xf = T(x, yt, z) @ UP
            p.lathe([(0.0, -0.009), (0.0092, -0.009), (0.0092, -0.0016), (0.0099, -0.0013),
                     (0.0099, 0.0), (0.0, 0.0)], "Brass", segs=16, xf=xf)
            p.lathe([(0.0, -0.022), (0.009, -0.022), (0.009, -0.0088), (0.0, -0.0088)], hull,
                    segs=12, xf=xf)
            p.cyl(0.0028, -0.0004, 0.0003, "Steel", segs=10, xf=xf)
    # The gauge on the lid by a drawn shell, and on the sides.
    text(p, "12", 0.018, "Black", T(-0.01, yl, 0.026) @ ON_TOP, depth=0.0005)
    for sx_ in (-1, 1):
        text(p, "12", 0.022, "Black", T(sx_ * (sx / 2), 0.004, 0.0) @ RY(sx_ * math.pi / 2),
             depth=0.0005)
        if label:
            text(p, label, 0.009, "Black",
                 T(sx_ * (sx / 2), -0.02, 0.0) @ RY(sx_ * math.pi / 2), depth=0.0005)
    return [p]


# --- Typed rounds and boxes (Weapon feel's ammunition types) --------------------------------

def typed_round(c, tip, segs):
    def build():
        p = Part("body")
        cartridge(p, c, T(0, 0, c[2] / 2), segs=segs, tip=tip)
        return [p]
    return build


def slug_12ga():
    p = Part("body")
    # A blue hull on a brass head, roll-crimped over a lead slug whose
    # rifled nose shows at the mouth.
    xf = T(0, 0, 0.035)
    p.lathe([(0.0, 0.0), (0.0099, 0.0), (0.01, -0.0004), (0.01, -0.0013), (0.0093, -0.0016),
             (0.0093, -0.012), (0.0091, -0.0125), (0.0, -0.0125)], "Brass", segs=20, xf=xf)
    p.lathe([(0.0068, -0.011), (0.0091, -0.011), (0.0091, -0.0655), (0.0086, -0.0685),
             (0.0072, -0.069), (0.0068, -0.0665)], "Blue", segs=20, xf=xf, closed=True)
    p.lathe([(0.0, -0.045), (0.0072, -0.045), (0.0072, -0.064), (0.0062, -0.0675),
             (0.0035, -0.0695), (0.0, -0.07)], "Zinc", segs=16, xf=xf)
    for k in range(8):
        p.box((0.0007, 0.0004, 0.004), (0.0, 0.0071, -0.066), "Dark",
              xf=xf @ RZ(math.pi * k / 4))
    p.cyl(0.0028, -0.0004, 0.00003, "Steel", segs=12, xf=xf)
    p.tube(0.0068, 0.0062, -0.0002, 0.00002, "Copper", segs=16, xf=xf)
    return [p]


TYPED = {
    "round_545_ap": typed_round(C545, "ap", 14),
    "round_545_tracer": typed_round(C545, "tracer", 14),
    "round_762x39_ap": typed_round(C762X39, "api", 14),
    "round_762x39_tracer": typed_round(C762X39, "tracer", 14),
    "round_762r_ap": typed_round(C762, "api", 16),
    "round_762r_tracer": typed_round(C762, "tracer", 16),
    "slug_12ga": slug_12ga,
    # Boxes: the ball boxes with their type's tips, stripes and labels.
    "ammo_545_ap": lambda: ammo_545("7N22", lambda k: "ap", "Black"),
    "ammo_545_tracer": lambda: ammo_545("7T3", lambda k: "tracer", "Leaf"),
    "ammo_545_t4": lambda: ammo_545("5.45 T", lambda k: "tracer" if k % 4 == 3 else None,
                                    "Leaf"),
    "ammo_762x39_ap": lambda: ammo_762x39("BZ", "api", "Black"),
    "ammo_762x39_tracer": lambda: ammo_762x39("T-45", "tracer", "Leaf"),
    "ammo_762r_ap": lambda: ammo_762r("B-32", "Black"),
    "ammo_762r_tracer": lambda: ammo_762r("T-46", "Leaf"),
    "ammo_12ga_slug": lambda: ammo_12ga("Blue", "ПУЛЯ"),
}


# --- Re-packed (made at the bench from scavenged brass and powder) ---------------------------

# Each round's seating, a little off and never the same: offsets (m) along
# the case, cycled through.
SEATS = (0.0006, -0.0004, 0.0011, 0.0, -0.0008, 0.0003, 0.0009, -0.0002)


def repack_round(c, segs, seat=0.0007):
    def build():
        p = Part("body")
        cartridge(p, c, T(0, 0, c[2] / 2), segs=segs, brass="Tarnish", seat=seat)
        return [p]
    return build


def repack_box(dims, c, calibre, shells=False):
    """A plain kraft-card box the factory box's size, its lid flap folded
    back, the re-packed rounds standing in it noses up (or shells heads
    up), a pencilled paper label on the front."""
    def build():
        sx, sy, sz = dims
        p = Part("body")
        y0 = -sy / 2
        wall = 0.0012
        carton(p, sx, sy - 0.002, sz, y0, "Straw", wall=wall)
        # What's left of the lid: a flap hanging down the back from its fold
        # (torn short, so it stays inside the item's box).
        flap = sy - 0.006
        # (Hanging out past the string, clear of the wall's plane.)
        p.box((sx, flap, wall), (0.0, y0 + sy - 0.002 - flap / 2, sz / 2 + 0.0034 + wall / 2),
              "Straw", bevel=0.0004, segs=1)
        top = y0 + sy - 0.006
        p.box((sx - 0.004, 0.002, sz - 0.004), (0.0, top - 0.003, 0.0), "Black", segs=1)
        r = 0.0099 if shells else max(rr for rr, _ in c[0])
        pitch = 2 * r + 0.0012
        cols = max(1, int((sx - 0.004) // pitch))
        rows = max(1, int((sz - 0.004) // pitch))
        # A home-made box is seldom full: at most 20 rounds, from the front
        # corner, the rest of the floor bare.
        k = 0
        for i in range(cols):
            for j in range(rows):
                if k >= 20:
                    break
                x = (i - (cols - 1) / 2) * pitch
                z = (j - (rows - 1) / 2) * pitch
                if shells:
                    xf = T(x, top + 0.002, z) @ UP
                    p.lathe([(0.0, -0.009), (0.0092, -0.009), (0.0092, -0.0016), (0.0099, -0.0013),
                             (0.0099, 0.0), (0.0, 0.0)], "Tarnish", segs=12, xf=xf)
                    p.cyl(0.0028, -0.0004, 0.0003, "Steel", segs=8, xf=xf)
                else:
                    # The case's top `show` stands above the dark fill, the
                    # bullet tips no higher than just over the box's top.
                    show = 0.012
                    case_end = min(z_ for _, z_ in c[0])
                    zf = case_end + show
                    base = min(top - 0.003 + case_end + show, top + 0.004 - c[2])
                    # ...and never through the box's floor (a long round in a
                    # shallow box stands a little proud instead).
                    base = max(base, y0 + 0.002 + zf)
                    nose(p, c, zf, T(x, base, z) @ RX(math.pi / 2), segs=7, brass="Tarnish",
                         seat=SEATS[k % len(SEATS)])
                k += 1
        # The label: a torn paper slip, pencilled, a little askew.
        lw, lh = min(0.06, sx - 0.006), min(0.026, sy * 0.6)
        # A slip tucked under a string round the box (it stands off the card
        # by the string's thickness, clear of both the wall's faces).
        lab = T(0.0, -sy * 0.08, -sz / 2 - 0.0025) @ RZ(0.04)
        p.box((lw, lh, 0.0012), (0.0, 0.0, 0.0), "Paper", xf=lab)
        ys = -sy * 0.08
        p.box((sx + 0.003, 0.0016, 0.0016), (0.0, ys, -sz / 2 - 0.0036), "Rope", segs=1)
        p.box((sx + 0.003, 0.0016, 0.0016), (0.0, ys, sz / 2 + 0.0009), "Rope", segs=1)
        for sx_ in (-1, 1):
            p.box((0.0016, 0.0016, sz + 0.009), (sx_ * (sx / 2 + 0.0009), ys, -0.0014), "Rope",
                  segs=1)
        text(p, "REPACK", 0.006, "Dark", lab @ T(0.0, lh * 0.2, -0.0006) @ RY(math.pi),
             depth=0.0003)
        text(p, calibre, 0.0048, "Dark", lab @ T(0.0, -lh * 0.25, -0.0006) @ RY(math.pi),
             depth=0.0003)
        return [p]
    return build


def cleaning_rod():
    p = Part("body")
    # A steel rod along Z, 8 mm at its thickest: a slotted jag at the -Z
    # end, a knurled handle with a brass ferrule at +Z.
    p.cyl(0.0033, -0.27, 0.25, "Steel", segs=10)
    p.lathe([(0.0, -0.3), (0.0025, -0.3), (0.0035, -0.297), (0.0035, -0.272), (0.0033, -0.27),
             (0.0, -0.27)], "Steel", segs=10)
    p.box((0.008, 0.0012, 0.02), (0.0, 0.0, -0.288), "Dark", segs=1)
    p.cyl(0.004, 0.25, 0.258, "Brass", segs=12, chamfer=0.0008)
    p.cyl(0.004, 0.258, 0.3, "Iron", segs=12, chamfer=0.001)
    for k in range(5):
        p.tube(0.0041, 0.0039, 0.262 + 0.007 * k, 0.264 + 0.007 * k, "Dark", segs=12)
    return [p]


REPACK = {
    "cleaning_rod": cleaning_rod,
    "round_9x18_repack": repack_round(C9, 14),
    "round_545_repack": repack_round(C545, 14),
    "round_762x39_repack": repack_round(C762X39, 14),
    "round_762x25_repack": repack_round(C762X25, 14),
    "round_762r_repack": repack_round(C762, 16),
    "shell_12ga_repack": lambda: shell_12ga(hull="Brick", head="Tarnish"),
    "ammo_9x18_repack": repack_box((0.04, 0.03, 0.07), C9, "9x18"),
    "ammo_545_repack": repack_box((0.05, 0.035, 0.1), C545, "5.45"),
    "ammo_762x39_repack": repack_box((0.06, 0.046, 0.03), C762X39, "7.62x39"),
    "ammo_762x25_repack": repack_box((0.1, 0.036, 0.073), C762X25, "7.62x25"),
    "ammo_762r_repack": repack_box((0.045, 0.03, 0.09), C762, "54R"),
    "ammo_12ga_repack": repack_box((0.07, 0.07, 0.12), None, "12", shells=True),
}


# --- Robot belt ------------------------------------------------------------------------------

def robot_ammo_belt():
    s = cube(0.5)
    yb = -s / 2
    p = Part("body")
    # Caseless slugs (a propellant block, a steel penetrator) clipped in
    # dark polymer links, the belt folded flat on itself three times; a pale
    # starter tab on the free end.
    n, pitch = 7, 0.0085
    x0 = -(n - 1) * pitch / 2
    rr = 0.0042
    for layer in range(3):
        y = yb + 0.0062 + layer * 0.0118
        for k in range(n):
            x = x0 + k * pitch
            p.box((0.0078, 0.0078, 0.024), (x, y, 0.003), "Bakelite")
            p.lathe([(0.0, -0.009), (0.0032, -0.009), (0.0032, -0.016), (0.0018, -0.0205),
                     (0.0, -0.022)], "Steel", segs=6, xf=T(x, y, 0.0))
            for z in (-0.003, 0.011):
                p.box((0.0088, 0.0096, 0.004), (x, y, z), "Dark")
        # Link strips along both edges of the layer.
        for z in (-0.003, 0.011):
            p.box((n * pitch, 0.0014, 0.004), (0.0, y - 0.0055, z), "Dark")
        # Folds: half-loops of the link strip round the ends.
        if layer < 2:
            xe = x0 + (n - 0.5) * pitch if layer % 2 == 0 else x0 - 0.5 * pitch
            a0 = -90.0 if layer % 2 == 0 else 90.0
            for z in (-0.003, 0.011):
                p.lathe([(0.0053, z - 0.002), (0.0067, z - 0.002), (0.0067, z + 0.002),
                         (0.0053, z + 0.002)], "Dark", segs=8,
                        xf=T(xe, y + 0.0004, 0.0), closed=True, start=a0, sweep=180.0)
    yt = yb + 0.0062 + 2 * 0.0118 + 0.0048
    xt = x0 + (n - 1) * pitch - 0.003
    p.box((0.016, 0.0016, 0.03), (xt, yt + 0.0008, 0.004), "Shell", bevel=0.0006, segs=1)
    p.box((0.004, 0.0006, 0.022), (xt + 0.003, yt + 0.0018, 0.004), "Glow")
    return [p]


# The magazine scripts import the cartridges from here; build only when run.
if __name__ == "__main__":
    for item_id, fn in ((("round_9x18", round_9x18), ("round_545", round_545),
                        ("round_762r", round_762r), ("shell_12ga", shell_12ga),
                        ("round_762x39", round_762x39), ("ammo_762x39", ammo_762x39),
                        ("round_762x38r", round_762x38r), ("ammo_762x38r", ammo_762x38r),
                        ("round_762x25", round_762x25), ("ammo_762x25", ammo_762x25),
                        ("ammo_9x18", ammo_9x18), ("ammo_545", ammo_545),
                        ("ammo_762r", ammo_762r), ("ammo_12ga", ammo_12ga),
                        ("robot_ammo_belt", robot_ammo_belt)) + tuple(TYPED.items())
                       + tuple(REPACK.items())):
        if wanted(item_id):
            item(item_id, fn)
