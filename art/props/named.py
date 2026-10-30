"""The things of the people Story's notes name (and a few it asked to be
seen): visual only, dressed in by `world::clutter`'s `named` where each
stands. Origins as clutter.py's: on the surface under the thing's centre;
things for a wall have theirs on the wall's face at their fixing, standing
out towards -Z.

Marrow: till, chit_spike, closed_card (the shop, shut 13 Oct); axe_pegs,
whetstone (J., the logging foreman); barometer, kettle, horseshoe (Ondrej
Brabec, the pump keeper).
Kettle: signal_flags (the crossing keeper); waders, level_staff (the weir
keeper); antlers (old Fiala's lodge); bed_name_mirek, charcoal_sack (Mirek
Hájek, the charcoal burner); hymn_board (the sexton's chapel);
child_drawing (the Nováks, in Miners' Row).
Cinder: tin_empty (the scavenger off Lev's truck, with scenes.py's
bedroll); bow_saw (T. Veselý, the woodward).
Sallow: cup_one, photo_church (Alois Pech); key_hook_empty (J. Rybář's
shop); rods_rack, landing_net, stuffed_pike (the angling club);
armchair, peg_empty, slippers (Mrs Hlavač).
Old Sallow's silt, sixty years down: bedstead_silted, cart_wheel, mangle
(and clutter.py's dishes_broken).

Materials as clutter.py's. The letters (the card, the name, the hymn
numbers) are `text`'s stencils, registered in `render::legibility::ART`.
"""

import importlib.util
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from _kit import UP, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

# The items' stencil letters (their kit shares this one's module name).
_spec = importlib.util.spec_from_file_location("items_kit", os.path.join(HERE, "..", "items",
                                                                          "_kit.py"))
_items = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_items)
text = _items.text

_kit.AO_DISTANCE = 0.2
_kit.AO_SKY = "indoors"


def blob(p, size, at, mat, segs=10, rings=6, xf=None):
    x = T(*at) @ S(*size)
    if xf is not None:
        x = xf @ x
    p.sphere(1.0, (0, 0, 0), mat, segs=segs, rings=rings, xf=x)


def pegs(p, w, n):
    """A peg board on the wall, `n` pegs: the pegs' tips."""
    p.box((w, 0.08, 0.02), (0, 0, -0.01), "Timber", bevel=0.004, segs=1)
    tips = []
    for k in range(n):
        x = -w / 2 + w * (k + 0.5) / n
        rod(p, (x, 0.0, -0.02), (x, 0.03, -0.08), 0.008, "Timber", segs=6)
        tips.append((x, 0.03, -0.08))
    return tips


# --- Marrow -----------------------------------------------------------------------------------

def till():
    """A cash register on the counter, its drawer shut: 0.36 x 0.3 x 0.32."""
    p = Part("static")
    p.box((0.36, 0.12, 0.32), (0, 0.06, 0), "Brass", bevel=0.01, segs=1)
    p.prism([(-0.16, 0.12), (0.16, 0.12), (0.16, 0.22), (-0.16, 0.16)], -0.14, 0.12, "Brass",
            axis="z")
    for r in range(3):
        for c in range(5):
            p.cyl(0.012, 0.0, 0.01, "Cream", segs=8,
                  xf=T(-0.12 + c * 0.06, 0.13 + r * 0.025, -0.1 + r * 0.05) @ RX(-1.2))
    # Standing on the top's high end (it hung 1 to 4 cm over the slope).
    p.box((0.3, 0.08, 0.01), (0, 0.255, 0.1), "Black")
    p.box((0.05, 0.02, 0.02), (0.2, 0.06, -0.05), "Black")
    return [p]


def chit_spike():
    """A spike of paper chits on its wooden foot, the credit owed: 0.1
    across, 0.2 tall."""
    p = Part("static")
    w = Wear(71)
    p.cyl(0.045, 0.0, 0.02, "Timber", segs=12, xf=UP)
    rod(p, (0, 0.02, 0), (0, 0.2, 0), 0.0025, "Steel", segs=4)
    for k in range(9):
        board(p, (0.07, 0.001, 0.05), (0.0, 0.03 + k * 0.012, 0.0), "Paper", wear=w,
              bevel=0.0, yaw=k * 0.7 + w.j(0.3), tilt=w.j(0.15))
    return [p]


def closed_card():
    """A CLOSED card hung inside the shop's window on its string, read from
    the street (-Z): 0.32 x 0.14, its string's nail at the origin."""
    p = Part("static")
    p.box((0.32, 0.14, 0.004), (0, -0.12, 0), "Cream")
    # The letters set into the card, standing 3.5 mm proud of it.
    text(p, "CLOSED", 0.07, "Red", T(0.0, -0.12, 0.0) @ RY(math.pi), depth=0.0055)
    for sx in (-1, 1):
        rod(p, (sx * 0.14, -0.05, 0.0), (0.0, 0.0, 0.0), 0.0015, "Rope", segs=3)
    return [p]


def axe_pegs():
    """Two felling axes hung by their heads on pegs: the wall's face at z
    0, their helves hanging 0.75."""
    p = Part("static")
    for (x, y, z) in pegs(p, 0.5, 2):
        top = (x, y - 0.02, z + 0.02)
        p.box((0.17, 0.08, 0.025), top, "Steel", bevel=0.004, segs=1)
        p.box((0.02, 0.1, 0.03), (x + 0.085, y - 0.02, z + 0.02), "Steel")
        rod(p, (x, y - 0.06, z + 0.02), (x - 0.02, y - 0.8, z + 0.05), 0.016, "Timber", segs=6)
    return [p]


def whetstone():
    """A whetstone and its oil can on the table: 0.25 x 0.08 x 0.1."""
    p = Part("static")
    p.box((0.2, 0.03, 0.06), (-0.03, 0.015, 0), "Plaster", bevel=0.005, segs=1)
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.035, 0.05), (0.012, 0.075), (0.005, 0.11),
             (0.0, 0.11)], "Rusty", segs=12, xf=T(0.1, 0.0, 0.02) @ UP)
    return [p]


def barometer():
    """A banjo barometer on the wall, the needle at STORMY: the wall's face
    at z 0, 0.24 x 0.6."""
    p = Part("static")
    p.prism([(-0.06, -0.3), (0.06, -0.3), (0.06, 0.05), (-0.06, 0.05)], -0.03, 0.0, "Timber",
            axis="z")
    p.cyl(0.12, -0.035, 0.0, "Timber", segs=20, xf=T(0, -0.17, 0) @ RX(math.pi))
    p.cyl(0.1, -0.038, -0.034, "Cream", segs=20, xf=T(0, -0.17, 0) @ RX(math.pi))
    rod(p, (0.0, -0.17, -0.04), (-0.07, -0.12, -0.04), 0.002, "Black", segs=3)
    p.cyl(0.02, -0.036, 0.0, "Glass", segs=10, xf=T(0, 0.01, 0) @ RX(math.pi))
    rod(p, (0.0, -0.27, -0.032), (0.0, 0.03, -0.032), 0.003, "Glass", segs=4)
    return [p]


def kettle():
    """A black kettle on the table: 0.22 across, 0.25 tall."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.09, 0.0), (0.105, 0.04), (0.1, 0.11), (0.06, 0.14), (0.03, 0.15),
             (0.0, 0.15)], "Black", segs=16, xf=UP)
    rod(p, (0.08, 0.06, 0.0), (0.16, 0.13, 0.0), 0.012, "Black", segs=6)
    for k in range(5):
        a0, a1 = math.pi * k / 5, math.pi * (k + 1) / 5
        rod(p, (-0.06 * math.cos(a0), 0.15 + 0.08 * math.sin(a0), 0.0),
            (-0.06 * math.cos(a1), 0.15 + 0.08 * math.sin(a1), 0.0), 0.005, "Black", segs=4)
    return [p]


def horseshoe():
    """A horseshoe nailed over the door, points up to hold the luck: the
    wall's face at z 0."""
    p = Part("static")
    n = 9
    for k in range(n):
        a0 = -0.3 + (math.pi + 0.6) * k / n
        a1 = -0.3 + (math.pi + 0.6) * (k + 1) / n
        p0 = (0.06 * math.cos(a0), -0.06 * math.sin(a0) + 0.02, -0.008)
        p1 = (0.06 * math.cos(a1), -0.06 * math.sin(a1) + 0.02, -0.008)
        rod(p, p0, p1, 0.008, "Rusty", segs=4)
    return [p]


# --- Kettle -----------------------------------------------------------------------------------

def signal_flags():
    """The crossing keeper's red and green flags furled on their sticks,
    hung on a peg: the wall's face at z 0."""
    p = Part("static")
    p.box((0.2, 0.06, 0.02), (0, 0, -0.01), "Timber", bevel=0.003, segs=1)
    for (x, mat) in ((-0.04, "Red"), (0.04, "Olive")):
        rod(p, (x, 0.02, -0.04), (x + 0.02, -0.55, -0.05), 0.009, "Timber", segs=6)
        p.lathe([(0.0, 0.0), (0.025, 0.02), (0.03, 0.2), (0.02, 0.32), (0.0, 0.33)], mat,
                segs=8, xf=T(x + 0.005, -0.33, -0.05) @ UP)
    return [p]


def waders():
    """Rubber thigh waders hung by their straps from a nail: the wall's face
    at z 0, their feet 1.1 below."""
    p = Part("static")
    rod(p, (0, 0, 0), (0, 0, -0.03), 0.003, "Iron", segs=4)
    for sx in (-1, 1):
        rod(p, (0.0, 0.0, -0.02), (sx * 0.1, -0.2, -0.05), 0.006, "Black", segs=4)
        p.lathe([(0.0, 0.0), (0.06, 0.02), (0.07, 0.45), (0.075, 0.8), (0.0, 0.8)], "Black",
                segs=10, xf=T(sx * 0.1, -1.0, -0.07) @ UP)
        p.box((0.09, 0.06, 0.24), (sx * 0.1, -1.04, -0.13), "Black", bevel=0.02, segs=1)
    return [p]


def level_staff():
    """A surveyor's level staff, red and white, leaning on the wall: its top
    on the wall's face (z 0), its foot 0.3 out on the floor 2.0 below."""
    p = Part("static")
    foot, top = (0.0, -2.0, -0.3), (0.0, 0.0, -0.03)
    L = math.dist(foot, top)
    a = math.atan2(top[2] - foot[2], top[1] - foot[1])
    for k in range(20):
        t0 = k / 20
        c = tuple(f + (q - f) * (t0 + 0.025) for f, q in zip(foot, top))
        p.box((0.06, L / 20, 0.025), c, "Red" if k % 2 == 0 else "Cream",
              xf=T(*c) @ RX(-a) @ T(*(-v for v in c)))
    return [p]


def antlers():
    """A stag's antlers on their shield: the wall's face at z 0."""
    p = Part("static")
    p.prism([(-0.1, -0.12), (0.1, -0.12), (0.12, 0.05), (0.0, 0.14), (-0.12, 0.05)], -0.03,
            0.0, "Timber", axis="z")
    for sx in (-1, 1):
        pts = [(sx * 0.03, 0.02, -0.05), (sx * 0.15, 0.12, -0.08), (sx * 0.25, 0.3, -0.06),
               (sx * 0.28, 0.48, -0.03)]
        for a, b in zip(pts, pts[1:]):
            rod(p, a, b, 0.014, "Cream", segs=5)
        for (i, dx, dy) in ((1, 0.08, 0.1), (2, 0.06, 0.14), (2, -0.06, 0.12)):
            a = pts[i]
            rod(p, a, (a[0] + sx * dx, a[1] + dy, a[2] - 0.04), 0.01, "Cream", segs=5)
    return [p]


def bed_name_mirek():
    """The charcoal burner's name painted on his bed's foot board: a plank
    over the board, MIREK on it; its back on the frame at z 0."""
    p = Part("static")
    board(p, (0.8, 0.14, 0.02), (0.0, 0.0, -0.01), "Planks", bevel=0.003)
    text(p, "MIREK", 0.07, "Cream", T(0.0, 0.0, -0.01) @ RY(math.pi), depth=0.0135)
    return [p]


def charcoal_sack():
    """A sack of charcoal slumped by the door, a few lumps spilled: 0.5 x
    0.45 x 0.4."""
    p = Part("static")
    w = Wear(73)
    blob(p, (0.22, 0.2, 0.18), (0.0, 0.18, 0.0), "Canvas")
    blob(p, (0.12, 0.08, 0.1), (0.0, 0.38, 0.03), "Canvas")
    for k in range(6):
        p.box((0.04, 0.03, 0.035), (0.22 + w.j(0.08), 0.015, w.j(0.12)), "Black", bevel=0.008,
              segs=1, xf=T(0.22, 0, 0) @ RY(k * 1.1) @ T(-0.22, 0, 0))
    return [p]


def hymn_board():
    """The hymn board by the altar, its numbers still up from the last
    Sunday: the wall's face at z 0, 0.5 x 0.9."""
    p = Part("static")
    p.prism([(-0.25, -0.45), (0.25, -0.45), (0.25, 0.35), (0.0, 0.45), (-0.25, 0.35)], -0.03,
            0.0, "Timber", axis="z")
    for k, num in enumerate(("214", "37", "512")):
        y = 0.2 - k * 0.24
        p.box((0.4, 0.16, 0.006), (0.0, y, -0.033), "Black")
        text(p, num, 0.08, "Cream", T(0.0, y, -0.034) @ RY(math.pi), depth=0.0055)
    return [p]


def child_drawing():
    """A child's chalk drawing on the wall: a house, a sun, three people:
    the wall's face at z 0, 0.5 x 0.4."""
    p = Part("static")
    s = 0.004

    def line(a, b, mat="Cream"):
        L = math.dist(a, b)
        ang = math.atan2(b[1] - a[1], b[0] - a[0])
        c = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, -0.001)
        p.box((L + s, s, 0.002), c, mat, xf=T(*c) @ RZ(ang) @ T(*(-v for v in c)))

    # The house and its roof.
    for a, b in (((-0.2, -0.15), (0.0, -0.15)), ((0.0, -0.15), (0.0, 0.0)),
                 ((0.0, 0.0), (-0.2, 0.0)), ((-0.2, 0.0), (-0.2, -0.15)),
                 ((-0.22, 0.0), (-0.1, 0.09)), ((-0.1, 0.09), (0.02, 0.0))):
        line(a, b)
    # The sun.
    for k in range(8):
        a = k * math.pi / 4
        line((0.17 + 0.03 * math.cos(a), 0.13 + 0.03 * math.sin(a)),
             (0.17 + 0.06 * math.cos(a), 0.13 + 0.06 * math.sin(a)), "Yellow")
    # Three of them, big, middling and small, holding hands.
    for k, (x, h) in enumerate(((0.06, 0.12), (0.13, 0.1), (0.19, 0.06))):
        line((x, -0.18), (x, -0.18 + h))
        line((x - 0.02, -0.18 + h * 0.6), (x + 0.03, -0.18 + h * 0.6))
        line((x - 0.012, -0.18 + h + 0.02), (x + 0.012, -0.18 + h + 0.02))
    return [p]


# --- Cinder -----------------------------------------------------------------------------------

def tin_empty():
    """An empty tin, opened and licked clean, its lid bent back, a spoon in
    it: 0.1 across."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.04, 0.0), (0.04, 0.1), (0.037, 0.1), (0.037, 0.004),
             (0.0, 0.004)], "Steel", segs=14, xf=UP)
    p.cyl(0.039, 0.0, 0.002, "Steel", segs=14,
          xf=T(0.0, 0.1, 0.04) @ RX(-1.9) @ T(0.0, 0.0, -0.04) @ UP)
    rod(p, (0.0, 0.01, 0.0), (0.02, 0.15, -0.02), 0.003, "Steel", segs=4)
    return [p]


def bow_saw():
    """A bow saw hung on a nail: the wall's face at z 0, 0.75 across."""
    p = Part("static")
    rod(p, (0, 0, 0), (0, 0, -0.03), 0.003, "Iron", segs=4)
    n = 8
    pts = [(0.36 * math.cos(math.pi * k / n), -0.02 - 0.2 * math.sin(math.pi * k / n), -0.03)
           for k in range(n + 1)]
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, 0.012, "Red", segs=6)
    # The blade's back on the nail (2 mm under it).
    p.box((0.72, 0.03, 0.002), (0.0, -0.0175, -0.03), "Steel")
    return [p]


# --- Sallow -----------------------------------------------------------------------------------

def cup_one():
    """One cup on the table, a saucer under it: 0.14 across."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.07, 0.0), (0.072, 0.008), (0.0, 0.008)], "Enamel", segs=16,
            xf=UP)
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.042, 0.07), (0.039, 0.07), (0.032, 0.004),
             (0.0, 0.004)], "Enamel", segs=14, xf=T(0, 0.008, 0) @ UP)
    p.lathe([(0.025, -0.004), (0.031, -0.004), (0.031, 0.004), (0.025, 0.004)], "Enamel",
            segs=10, closed=True, xf=T(0.05, 0.045, 0) @ RX(math.pi / 2))
    return [p]


def photo_church():
    """A framed photograph of Old Sallow's church standing on its strut:
    0.2 x 0.25 (the print set back in its frame)."""
    p = Part("static")
    xf = T(0, 0.12, 0) @ RX(-0.1)
    for sx in (-1, 1):
        p.box((0.02, 0.25, 0.015), (sx * 0.09, 0, 0), "Timber", xf=xf)
    for sy in (-1, 1):
        p.box((0.16, 0.02, 0.015), (0, sy * 0.115, 0), "Timber", xf=xf)
    p.box((0.16, 0.21, 0.004), (0, 0, 0.0055), "Timber", xf=xf)
    p.box((0.16, 0.21, 0.002), (0, 0, 0.0025), "Cream", xf=xf)
    # The church in grey on the print: the nave and its tower, the dome.
    p.box((0.08, 0.05, 0.004), (-0.02, -0.05, 0.0), "Plaster", xf=xf)
    p.box((0.03, 0.1, 0.004), (0.04, -0.025, 0.0), "Plaster", xf=xf)
    p.cyl(0.016, -0.003, 0.0005, "Black", segs=10, xf=xf @ T(0.04, 0.035, 0.0))
    rod(p, (0.0, 0.0, 0.03), (0.0, 0.12, 0.009), 0.004, "Timber", segs=4)
    return [p]


def key_hook_empty():
    """The shop's key board by the door, its hook empty, the key's outline
    paler on the board: the wall's face at z 0."""
    p = Part("static")
    p.box((0.16, 0.22, 0.015), (0, -0.08, -0.0075), "Timber", bevel=0.003, segs=1)
    p.box((0.04, 0.12, 0.002), (0, -0.1, -0.016), "Cream")
    rod(p, (0, 0.0, -0.015), (0, 0.02, -0.05), 0.004, "Brass", segs=5)
    return [p]


def rods_rack():
    """Three fishing rods on a rack along the wall, a reel on each: the
    wall's face at z 0, 2.2 long."""
    p = Part("static")
    for x in (-0.8, 0.8):
        p.box((0.06, 0.25, 0.02), (x, 0, -0.01), "Timber")
        for k in range(3):
            rod(p, (x, 0.08 - k * 0.08, -0.02), (x, 0.1 - k * 0.08, -0.08), 0.006, "Timber",
                segs=5)
    for k in range(3):
        y = 0.1 - k * 0.08
        rod(p, (-1.1, y, -0.06), (1.1, y + 0.01, -0.06), 0.006 - 0.0015 * (k % 2), "Timber",
            segs=6)
        p.cyl(0.03, 0.0, 0.02, "Black", segs=12, xf=T(-0.6, y - 0.04, -0.06) @ RY(math.pi / 2))
    return [p]


def landing_net():
    """A landing net leaning on the wall: its rim on the wall's face (z 0),
    its handle's end 0.3 out on the floor 1.5 below."""
    p = Part("static")
    # Up into the rim (its end stopped short of it).
    rod(p, (0.0, -1.5, -0.3), (0.0, -0.27, -0.054), 0.012, "Timber", segs=6)
    c = (0.0, -0.12, -0.03)
    p.lathe([(0.17, -0.005), (0.18, -0.005), (0.18, 0.005), (0.17, 0.005)], "Steel", segs=16,
            closed=True, xf=T(*c) @ RX(0.2))
    p.lathe([(0.17, 0.0), (0.12, -0.12), (0.05, -0.2), (0.0, -0.22)], "Rope", segs=12,
            xf=T(*c) @ RX(0.2))
    return [p]


def stuffed_pike():
    """A stuffed pike in its glass case on the wall: the wall's face at z
    0, 0.9 x 0.3."""
    p = Part("static")
    p.box((0.9, 0.3, 0.02), (0, 0, -0.01), "Timber")
    p.box((0.86, 0.26, 0.01), (0, 0, -0.025), "Olive")
    blob(p, (0.34, 0.06, 0.04), (0.0, 0.0, -0.08), "Olive", segs=12, rings=6)
    p.prism([(0.3, 0.0), (0.42, 0.06), (0.42, -0.06)], -0.085, -0.075, "Olive", axis="z")
    p.box((0.88, 0.28, 0.003), (0, 0, -0.15), "Glass")
    for sx in (-1, 1):
        p.box((0.02, 0.28, 0.13), (sx * 0.44, 0, -0.085), "Timber")
    for sy in (-1, 1):
        p.box((0.86, 0.02, 0.13), (0, sy * 0.14, -0.085), "Timber")
    return [p]


def armchair():
    """An old armchair turned to the window, an antimacassar on its back:
    0.75 x 0.95 x 0.8, its front -Z."""
    p = Part("static")
    p.box((0.75, 0.42, 0.75), (0, 0.21, 0), "Wool", bevel=0.04, segs=1)
    p.box((0.72, 0.55, 0.16), (0, 0.7, 0.3), "Wool", bevel=0.05, segs=1)
    for sx in (-1, 1):
        p.box((0.13, 0.22, 0.7), (sx * 0.31, 0.53, 0.02), "Wool", bevel=0.05, segs=1)
    p.box((0.62, 0.12, 0.56), (0, 0.48, -0.05), "Wool", bevel=0.04, segs=1)
    p.box((0.4, 0.3, 0.004), (0, 0.82, 0.2195), "Cream")  # on the back's face (3 mm off)
    return [p]


def peg_empty():
    """A coat peg by the door with nothing on it: the wall's face at z 0."""
    p = Part("static")
    pegs(p, 0.3, 2)
    return [p]


def slippers():
    """A pair of felt slippers side by side: 0.22 x 0.27."""
    p = Part("static")
    for x in (-0.055, 0.055):
        p.box((0.085, 0.02, 0.25), (x, 0.01, 0), "Black", bevel=0.008, segs=1)
        p.box((0.08, 0.045, 0.13), (x, 0.04, -0.05), "Wool", bevel=0.02, segs=1)
    return [p]


# --- Old Sallow's silt ---------------------------------------------------------------------------

def bedstead_silted():
    """An iron bedstead sunk to its rails in the silt, rusted: its head
    and foot ends standing, 1.9 x 0.5 x 0.9 (its foot at y 0, the silt's
    top)."""
    p = Part("static")
    for (z, h) in ((-0.95, 0.55), (0.95, 0.4)):
        for sx in (-1, 1):
            rod(p, (sx * 0.44, -0.15, z), (sx * 0.44, h, z), 0.018, "Rusty", segs=6)
        rod(p, (-0.44, h, z), (0.44, h, z), 0.014, "Rusty", segs=6)
        for k in range(5):
            x = -0.3 + k * 0.15
            rod(p, (x, -0.1, z), (x, h - 0.02, z), 0.007, "Rusty", segs=4)
    for sx in (-1, 1):
        rod(p, (sx * 0.44, 0.04, -0.95), (sx * 0.44, 0.02, 0.95), 0.012, "Rusty", segs=5)
    return [p]


def cart_wheel():
    """A wooden cart wheel lying half in the silt, its iron tyre rusted:
    1.0 across (its hub at the silt's top)."""
    p = Part("static")
    xf = T(0, 0.02, 0) @ RX(-1.45)
    p.lathe([(0.46, -0.03), (0.5, -0.03), (0.5, 0.03), (0.46, 0.03)], "Rusty", segs=28,
            closed=True, xf=xf)
    p.lathe([(0.4, -0.035), (0.46, -0.035), (0.46, 0.035), (0.4, 0.035)], "Timber", segs=28,
            closed=True, xf=xf)
    p.cyl(0.08, -0.08, 0.08, "Timber", segs=12, xf=xf)
    for k in range(10):
        a = k * math.pi / 5
        p.box((0.33, 0.04, 0.03), (0.0, 0.0, 0.0), "Timber",
              xf=xf @ RZ(a) @ T(0.245, 0.0, 0.0))
    return [p]


def mangle():
    """A cast-iron mangle on its legs, tipped over in the silt: 0.9 x 0.6 x
    0.5 (lying)."""
    p = Part("static")
    tip = T(0, 0.25, 0) @ RZ(1.3) @ T(0, -0.25, 0)
    for sx in (-1, 1):
        p.prism([(-0.25, 0.0), (0.25, 0.0), (0.12, 0.9), (-0.12, 0.9)], sx * 0.3 - 0.02,
                sx * 0.3 + 0.02, "Rusty", axis="x", xf=tip)
    for y in (0.7, 0.8):
        p.cyl(0.05, -0.28, 0.28, "Timber", segs=12, xf=tip @ T(0, y, 0) @ RY(math.pi / 2))
    p.lathe([(0.2, -0.015), (0.23, -0.015), (0.23, 0.015), (0.2, 0.015)], "Rusty", segs=20,
            closed=True, xf=tip @ T(0.34, 0.75, 0) @ RY(math.pi / 2))
    return [p]


for name, build in (("till", till), ("chit_spike", chit_spike), ("closed_card", closed_card),
                    ("axe_pegs", axe_pegs), ("whetstone", whetstone),
                    ("barometer", barometer), ("kettle", kettle), ("horseshoe", horseshoe),
                    ("signal_flags", signal_flags), ("waders", waders),
                    ("level_staff", level_staff), ("antlers", antlers),
                    ("bed_name_mirek", bed_name_mirek), ("charcoal_sack", charcoal_sack),
                    ("hymn_board", hymn_board), ("child_drawing", child_drawing),
                    ("tin_empty", tin_empty), ("bow_saw", bow_saw),
                    ("cup_one", cup_one), ("photo_church", photo_church),
                    ("key_hook_empty", key_hook_empty), ("rods_rack", rods_rack),
                    ("landing_net", landing_net), ("stuffed_pike", stuffed_pike),
                    ("armchair", armchair), ("peg_empty", peg_empty), ("slippers", slippers),
                    ("bedstead_silted", bedstead_silted), ("cart_wheel", cart_wheel),
                    ("mangle", mangle)):
    prop(name, build)
