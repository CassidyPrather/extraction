"""Lived-in clutter for the houses' rooms, visual only (no colliders): what
people left, by valley, and what a fire or a shell left of it. Dressed in
by `world::clutter` into slots on each house's plan. Origins: on the
surface under the thing's centre; things for a wall (`*_hook`, boards,
racks) have theirs on the wall's face at their fixing, the thing standing
out towards -Z.

Marrow (evacuated, half-packed): suitcase_open, suitcase_shut, bundle,
belongings_box, coat_hook, picture_frame.
Kettle (miners' rows): boots, helmet_lamp, carbide_lamp, lunch_pail,
jacket_hook, tally_board.
Anywhere: dishes, jars, books, rug, broom.
Ruins and Cinder's soot: dishes_broken, books_charred, frame_charred,
ash_film (a sill's grey film, a unit metre long).
For Story: dog_bowl, lead_hook and dog_collar (Hana's), pigeonholes (the
post office), ledger_open (the mine office).

Materials: Canvas, Leather, Wool, Ticking, Enamel, Brass, Paper, Cream,
Glass, Iron, Rusty, Timber, Planks, Char, Red, Olive, Black, Rope, Steel,
Yellow, Blue, Plaster.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import UP, ALONG_X, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

# Small things: their occlusion is their own folds and the floor's.
_kit.AO_DISTANCE = 0.2
# Indoors, always under a roof (but a burnt-out ruin's: the rain reaches
# those, and they're the props the game dresses there anyway).
_kit.AO_SKY = "indoors"


def blob(p, size, at, mat, segs=10, rings=6, xf=None):
    """A soft lump (a cloth bundle, a rolled blanket): a squashed sphere."""
    x = T(*at) @ S(*size)
    if xf is not None:
        x = xf @ x
    p.sphere(1.0, (0, 0, 0), mat, segs=segs, rings=rings, xf=x)


# --- Marrow -----------------------------------------------------------------------------------

def suitcase_open():
    """Lying open on a bed, clothes half in: 0.66 x 0.18 x 0.42, the lid
    up behind (+Z)."""
    p = Part("static")
    w = Wear(41)
    L, D, H = 0.66, 0.42, 0.12
    p.box((L, 0.012, D), (0, 0.006, 0), "Leather", bevel=0.004, segs=1)
    # The sides on the bottom board, not beside it.
    sh, sy = H - 0.012, 0.012 + (H - 0.012) / 2
    for sx in (-1, 1):
        p.box((0.012, sh, D), (sx * (L / 2 - 0.006), sy, 0), "Leather", bevel=0.003, segs=1)
    for sz in (-1, 1):
        p.box((L - 0.024, sh, 0.012), (0, sy, sz * (D / 2 - 0.006)), "Leather", bevel=0.003,
              segs=1)
    # The lid, hinged at the back edge, open past upright.
    hinge = (0.0, H, D / 2)
    lid = T(*hinge) @ RX(1.95) @ T(*(-v for v in hinge))
    p.box((L, 0.05, D), (0, H + 0.025, 0), "Leather", bevel=0.004, segs=1, xf=lid)
    p.box((L - 0.04, 0.004, D - 0.04), (0, H + 0.052, 0), "Ticking", xf=lid)
    # Folded clothes in it, a shirt sleeve over the edge.
    for k, (mat, h) in enumerate((("Wool", 0.04), ("Canvas", 0.03), ("Cream", 0.025))):
        y = 0.012 + 0.035 * k + h / 2
        board(p, (0.28, h, 0.3), (-0.15 + 0.12 * (k % 2) + w.j(0.02), y, w.j(0.02)), mat,
              bevel=0.01)
    blob(p, (0.12, 0.05, 0.16), (0.2, 0.06, 0.0), "Wool")
    p.box((0.08, 0.01, 0.22), (L / 2 + 0.03, 0.08, -0.05), "Cream", bevel=0.003, segs=1,
          xf=T(L / 2, 0.11, 0) @ RZ(-0.9) @ T(-L / 2, -0.11, 0))
    for x in (-0.18, 0.18):
        # On the front's face (2 mm off it).
        p.box((0.05, 0.03, 0.02), (x, 0.07, -D / 2 - 0.009), "Brass")
    return [p]


def suitcase_shut():
    """Packed and strapped, left by the door: 0.62 x 0.2 x 0.42, flat."""
    p = Part("static")
    p.box((0.62, 0.2, 0.42), (0, 0.1, 0), "Leather", bevel=0.02, segs=2)
    for x in (-0.17, 0.17):
        p.box((0.04, 0.21, 0.43), (x, 0.1, 0), "Canvas", bevel=0.004, segs=1)
    # The handle on its lugs, the lugs on the face below its rounded edge
    # (they stood 2 mm off it).
    p.box((0.16, 0.025, 0.03), (0, 0.185, -0.235), "Leather", bevel=0.008, segs=1)
    for x in (-0.06, 0.06):
        p.box((0.02, 0.03, 0.02), (x, 0.175, -0.219), "Brass")
    p.box((0.06, 0.05, 0.004), (0.25, 0.12, -0.212), "Cream")
    return [p]


def bundle():
    """A blanket tied up round what mattered, knot on top: 0.5 across."""
    p = Part("static")
    blob(p, (0.24, 0.17, 0.2), (0, 0.17, 0), "Wool", segs=12, rings=8)
    blob(p, (0.07, 0.05, 0.06), (0, 0.33, 0), "Wool")
    for sx in (-1, 1):
        blob(p, (0.06, 0.03, 0.03), (sx * 0.07, 0.35, 0), "Wool")
    p.lathe([(0.2, -0.01), (0.215, -0.01), (0.215, 0.01), (0.2, 0.01)], "Rope", segs=14,
            closed=True, xf=T(0, 0.17, 0) @ S(1.15, 0.85, 1.0))
    return [p]


def belongings_box():
    """A wooden box packed and not taken: a pot, a rolled blanket, a lamp,
    a framed photograph on top; 0.55 x 0.45 x 0.4."""
    p = Part("static")
    w = Wear(42)
    for sz in (-1, 1):
        board(p, (0.55, 0.3, 0.015), (0, 0.15, sz * 0.19), "Planks", wear=w)
    for sx in (-1, 1):
        board(p, (0.015, 0.3, 0.365), (sx * 0.267, 0.15, 0), "Planks", wear=w)
    p.box((0.52, 0.012, 0.365), (0, 0.006, 0), "Planks")
    blob(p, (0.22, 0.08, 0.13), (-0.08, 0.3, 0.03), "Wool")
    p.cyl(0.09, 0.0, 0.13, "Enamel", segs=12, xf=T(0.15, 0.2, -0.05) @ UP)
    rod(p, (0.05, 0.33, -0.05), (0.26, 0.33, -0.05), 0.008, "Iron", segs=5)
    # The lamp stood up on the box's bottom (it hung beside the pot).
    p.cyl(0.035, 0.0, 0.16, "Glass", segs=8, xf=T(0.17, 0.012, 0.1) @ UP)
    p.box((0.18, 0.02, 0.14), (-0.1, 0.39, -0.05), "Timber", bevel=0.004, segs=1,
          xf=T(-0.1, 0.39, -0.05) @ RX(-0.2) @ T(0.1, -0.39, 0.05))
    return [p]


def coat_hook():
    """A board with three pegs on the wall, a coat left on one: the wall's
    face at z 0, the coat hanging down to 1.0 m below the pegs."""
    p = Part("static")
    p.box((0.6, 0.1, 0.02), (0, 0, -0.01), "Timber", bevel=0.004, segs=1)
    for x in (-0.2, 0.0, 0.2):
        rod(p, (x, 0.0, -0.02), (x, 0.04, -0.09), 0.01, "Timber", segs=6)
    # The coat: shoulders off the middle peg, falling in folds.
    c = (0.0, -0.45, -0.08)
    p.lathe([(0.0, 0.45), (0.14, 0.4), (0.22, 0.2), (0.24, -0.1), (0.27, -0.5), (0.0, -0.5)],
            "Wool", segs=10, xf=T(*c) @ S(1.0, 1.0, 0.35) @ UP)
    # The sleeves hanging down from the shoulders (not out through the
    # wall behind).
    for sx in (-1, 1):
        p.lathe([(0.0, 0.0), (0.05, 0.02), (0.045, 0.55), (0.0, 0.56)], "Wool", segs=8,
                xf=T(sx * 0.2, -0.1, -0.09) @ RZ(sx * 0.12) @ RX(math.pi) @ UP)
    return [p]


def picture_frame():
    """A photograph in its frame, face down where it fell: 0.3 x 0.02 x 0.24."""
    p = Part("static")
    p.box((0.3, 0.02, 0.24), (0, 0.01, 0), "Timber", bevel=0.004, segs=1)
    p.box((0.24, 0.004, 0.18), (0, 0.022, 0), "Cream")
    p.box((0.04, 0.003, 0.12), (0.05, 0.025, 0.0), "Black")
    return [p]


# --- Kettle -----------------------------------------------------------------------------------

def boots():
    """A pair of work boots by the door, one fallen over: 0.35 x 0.28."""
    p = Part("static")
    # (The fallen one lifted onto its side: tipped, it dipped 4 cm under
    # the floor it lies on.)
    for (x, z, tip, lift) in ((-0.07, 0.0, 0.0, 0.0), (0.1, 0.05, 1.45, 0.0425)):
        xf = T(x, lift, z) @ RZ(tip) @ T(0, 0.0 if tip == 0 else 0.05, 0)
        p.box((0.1, 0.04, 0.29), (0, 0.02, -0.02), "Black", bevel=0.01, segs=1, xf=xf)
        p.box((0.095, 0.08, 0.21), (0, 0.08, -0.06), "Leather", bevel=0.02, segs=1, xf=xf)
        p.lathe([(0.0, 0.0), (0.05, 0.0), (0.05, 0.16), (0.0, 0.16)], "Leather", segs=10,
                xf=xf @ T(0, 0.05, 0.07) @ UP)
    return [p]


def helmet_lamp():
    """A miner's helmet, its cap lamp on the front and the lamp's cable
    trailing: 0.3 across."""
    p = Part("static")
    prof = [(0.0, 0.17), (0.07, 0.165), (0.12, 0.13), (0.14, 0.07), (0.14, 0.03), (0.17, 0.02),
            (0.17, 0.0), (0.0, 0.0)]
    p.lathe(prof, "Yellow", segs=16, xf=S(1.0, 1.0, 1.2) @ UP)
    p.box((0.05, 0.03, 0.03), (0, 0.11, -0.16), "Black")
    p.cyl(0.035, -0.04, 0.0, "Steel", segs=10, xf=T(0, 0.11, -0.17))
    p.cyl(0.028, -0.042, -0.038, "Glass", segs=10, xf=T(0, 0.11, -0.17))
    for k in range(5):
        a, b = 0.6 + k * 0.35, 0.6 + (k + 1) * 0.35
        rod(p, (0.16 * math.cos(a), 0.012, 0.16 * math.sin(a) + 0.05),
            (0.18 * math.cos(b) + 0.03 * k, 0.012, 0.18 * math.sin(b) + 0.05), 0.006, "Black",
            segs=4)
    return [p]


def carbide_lamp():
    """A brass carbide lamp: the water chamber over the carbide, the
    reflector and burner; 0.1 across, 0.2 high."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.04, 0.0), (0.04, 0.07), (0.045, 0.075), (0.045, 0.14), (0.02, 0.16),
             (0.0, 0.16)], "Brass", segs=14, xf=UP)
    p.lathe([(0.0, 0.0), (0.05, 0.01), (0.055, 0.02), (0.0, 0.02)], "Steel", segs=14,
            xf=T(0, 0.11, -0.05) @ RX(math.pi / 2) @ UP)
    rod(p, (0, 0.11, -0.045), (0, 0.11, -0.06), 0.005, "Brass", segs=5)
    rod(p, (0, 0.16, 0), (0, 0.2, 0.03), 0.003, "Iron", segs=4)
    return [p]


def lunch_pail():
    """A tin dinner pail with its lid and bail handle: 0.2 across."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.085, 0.0), (0.095, 0.16), (0.0, 0.16)], "Steel", segs=16, xf=UP)
    p.lathe([(0.0, 0.16), (0.1, 0.16), (0.1, 0.175), (0.03, 0.19), (0.0, 0.19)], "Steel",
            segs=16, xf=UP)
    for sx in (-1, 1):
        rod(p, (sx * 0.096, 0.15, 0), (sx * 0.06, 0.26, 0), 0.004, "Iron", segs=4)
    rod(p, (-0.06, 0.26, 0), (0.06, 0.26, 0), 0.004, "Iron", segs=4)
    return [p]


def jacket_hook():
    """A peg board on the wall, a work jacket and a carbide lamp's strap on
    the pegs (the wall's face at z 0)."""
    p = coat_hook()[0]
    rod(p, (0.2, 0.04, -0.09), (0.21, -0.25, -0.1), 0.008, "Leather", segs=4)
    p.box((0.18, 0.08, 0.03), (-0.0, -0.12, -0.165), "Canvas", bevel=0.005, segs=1)
    return [p]


def tally_board():
    """The shot-firer's tally board on a nail: a board ruled in rows,
    tallies chalked, a pencil on a string (the wall's face at z 0, the
    nail at the board's top)."""
    p = Part("static")
    p.box((0.4, 0.5, 0.015), (0, -0.25, -0.0075), "Timber", bevel=0.004, segs=1)
    for k in range(8):
        p.box((0.36, 0.003, 0.002), (0, -0.07 - 0.055 * k, -0.016), "Cream")
        for t in range(1 + (k * 3) % 5):
            p.box((0.004, 0.035, 0.002), (-0.12 + t * 0.02, -0.09 - 0.055 * k, -0.016), "Cream")
    rod(p, (0, 0.0, 0.0), (0, 0.0, -0.04), 0.003, "Iron", segs=4)
    rod(p, (0.18, -0.02, -0.017), (0.19, -0.3, -0.02), 0.0015, "Rope", segs=3)
    rod(p, (0.19, -0.3, -0.02), (0.2, -0.38, -0.022), 0.004, "Timber", segs=5)
    return [p]


# --- Anywhere ---------------------------------------------------------------------------------

def dishes():
    """Plates stacked, two cups, a pot on the table's corner: 0.45 x 0.3."""
    p = Part("static")
    for k in range(5):
        p.lathe([(0.0, 0.0), (0.06, 0.0), (0.11, 0.015), (0.11, 0.018), (0.0, 0.01)], "Enamel",
                segs=16, xf=T(-0.1, 0.012 * k, 0.0) @ UP)
    for x, z in ((0.08, -0.08), (0.14, 0.04)):
        p.lathe([(0.0, 0.0), (0.035, 0.0), (0.042, 0.075), (0.038, 0.075), (0.03, 0.005),
                 (0.0, 0.005)], "Enamel", segs=12, xf=T(x, 0, z) @ UP)
    p.lathe([(0.0, 0.0), (0.07, 0.0), (0.075, 0.1), (0.0, 0.1)], "Iron", segs=14,
            xf=T(0.05, 0, 0.15) @ UP)
    rod(p, (0.12, 0.08, 0.15), (0.24, 0.085, 0.15), 0.008, "Iron", segs=5)
    return [p]


def jars():
    """A row of preserving jars, one empty: 0.45 x 0.2 x 0.12."""
    p = Part("static")
    w = Wear(43)
    for k in range(4):
        x = -0.17 + k * 0.115 + w.j(0.01)
        h = 0.16 + w.j(0.015)
        p.lathe([(0.0, 0.0), (0.045, 0.0), (0.048, h * 0.85), (0.035, h), (0.0, h)], "Glass",
                segs=12, xf=T(x, 0, 0) @ UP)
        if k != 2:
            p.lathe([(0.0, 0.004), (0.042, 0.004), (0.044, h * 0.6), (0.0, h * 0.6)],
                    ("Red", "Olive", "Yellow")[k % 3], segs=10, xf=T(x, 0, 0) @ UP)
        p.cyl(0.04, h, h + 0.015, "Rusty", segs=10, xf=T(x, 0, 0) @ UP)
    return [p]


def books():
    """A stack of books, one lying open beside it: 0.4 x 0.15 x 0.25."""
    p = Part("static")
    w = Wear(44)
    y = 0.0
    for k, mat in enumerate(("Red", "Olive", "Blue", "Leather")):
        t = 0.03 + w.j(0.006)
        board(p, (0.17 + w.j(0.02), t, 0.24 + w.j(0.02)), (-0.08 + w.j(0.01), y + t / 2, 0.0),
              mat, wear=w, bevel=0.004, yaw=w.j(0.15))
        y += t
    for sx in (-1, 1):
        p.box((0.13, 0.012, 0.2), (0.17 + sx * 0.066, 0.006, 0.0), "Paper",
              xf=T(0.17, 0.0, 0) @ RZ(sx * -0.08) @ T(-0.17, 0, 0))
    return [p]


def rug():
    """A rag rug in stripes, a corner turned up: 1.4 x 0.9."""
    p = Part("static")
    w = Wear(48)
    # Stripes of torn rag woven across, each its own width and a little
    # thicker or thinner (braided, not cut from one sheet), their edges
    # softened; a knotted fringe at both ends.
    mats = ("Red", "Cream", "Olive", "Wool", "Blue", "Cream", "Red", "Olive", "Red")
    ws = [1.0 + w.j(0.35) for _ in mats]
    total = sum(ws)
    x = -0.7
    for m, f in zip(mats, ws):
        sw = 1.4 * f / total
        t = 0.008 + w.j(0.002)
        # Each a hair into the last (side by side they'd only touch).
        p.box((sw + 0.002, t, 0.9), (x + sw / 2, t / 2, 0.0), m, bevel=0.002, segs=1)
        x += sw
    for sx in (-1, 1):
        for k in range(18):
            z = -0.425 + k * 0.05 + w.j(0.006)
            L = 0.05 + w.j(0.012)
            p.box((L, 0.004, 0.012), (sx * (0.7 + L / 2 - 0.003), 0.002, z), "Cream",
                  xf=T(sx * 0.7, 0, z) @ RY(w.j(0.25)) @ T(-sx * 0.7, 0, -z))
    p.box((0.22, 0.008, 0.3), (0.62, 0.03, 0.33), "Red",
          xf=T(0.6, 0.0, 0.33) @ RZ(0.35) @ T(-0.6, 0.0, -0.33))
    return [p]


def broom():
    """A broom leaning on the wall by the door (the wall's face at z 0)."""
    p = Part("static")
    lean = 0.12
    w = Wear(49)
    rod(p, (0.0, 0.25, -0.02 - lean), (0.0, 1.35, -0.025), 0.013, "Timber", segs=8)
    # A birch-twig head: a splayed core, bunches of twigs standing out of
    # it to a ragged worn end, two bindings round its neck.
    p.lathe([(0.0, 0.07), (0.07, 0.07), (0.05, 0.25), (0.0, 0.27)], "Straw", segs=14,
            xf=T(0, 0.0, -0.03 - lean) @ S(1.0, 1.0, 0.5) @ UP)
    for k in range(18):
        a = 2 * math.pi * k / 18 + w.j(0.15)
        r0, r1 = 0.03, 0.095 + w.j(0.012)
        top = (r0 * math.cos(a), 0.2, -0.03 - lean + 0.5 * r0 * math.sin(a))
        foot = (r1 * math.cos(a), 0.005 + abs(w.j(0.01)), -0.03 - lean + 0.5 * r1 * math.sin(a))
        rod(p, top, foot, 0.006, "Straw", segs=4)
    for y in (0.2, 0.245):
        p.cyl(0.03, y, y + 0.018, "Rope", segs=10, xf=T(0, 0, -0.02 - lean * 0.85) @ UP)
    p.cyl(0.017, 1.3, 1.36, "Iron", segs=8, xf=T(0, 0, -0.025) @ UP)
    return [p]


# --- Ruins and soot ---------------------------------------------------------------------------

def dishes_broken():
    """Plates in pieces on the floor, a cup on its side: 0.5 across."""
    p = Part("static")
    w = Wear(45)
    laid = []
    for k in range(9):
        a = w.rng.random() * 2 * math.pi
        r = 0.05 + w.rng.random() * 0.2
        size = 0.03 + w.rng.random() * 0.05
        # On the floor, or on a shard it overlaps, a little into it (in one
        # plane they'd fight; stepped up 4 mm each, the last stood 3 cm
        # over the floor).
        x, z = r * math.cos(a), r * math.sin(a)
        y = max([yj + 0.0055 for xj, zj, sj, yj in laid
                 if math.hypot(x - xj, z - zj) < size + sj], default=0.0)
        laid.append((x, z, size, y))
        p.prism([(-size, -size * 0.6), (size, -size * 0.4), (size * 0.2, size)], 0.0, 0.006,
                "Enamel", axis="y", xf=T(x, y, z) @ RY(a))
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.042, 0.075), (0.038, 0.075), (0.03, 0.005),
             (0.0, 0.005)], "Enamel", segs=12, xf=T(0.12, 0.04, 0.05) @ RZ(1.57) @ UP)
    return [p]


def books_charred():
    """A stack of books burnt to their spines, ash round them."""
    p = Part("static")
    w = Wear(46)
    y = 0.0
    for k in range(3):
        t = 0.025 + w.j(0.005)
        board(p, (0.16 + w.j(0.02), t, 0.22 + w.j(0.02)), (w.j(0.02), y + t / 2, w.j(0.02)),
              "Char", wear=w, bevel=0.006, yaw=w.j(0.3))
        y += t
    # Ash flakes on the floor round them (21 cm apart: they never overlap,
    # and stepped up 4 mm each, the last stood 2 cm over the floor).
    for k in range(6):
        a = k * 1.1
        p.box((0.06, 0.003, 0.05), (0.2 * math.cos(a), 0.0015, 0.2 * math.sin(a)),
              "Paper" if k % 3 == 0 else "Char", xf=RY(a))
    return [p]


def frame_charred():
    """A picture frame burnt half away, the glass cracked out of it."""
    p = Part("static")
    p.box((0.3, 0.02, 0.03), (0, 0.01, -0.105), "Char", bevel=0.004, segs=1)
    p.box((0.03, 0.02, 0.21), (-0.135, 0.01, 0.015), "Char", bevel=0.004, segs=1)
    p.box((0.14, 0.02, 0.03), (-0.05, 0.01, 0.105), "Char", bevel=0.004, segs=1)
    p.prism([(-0.08, -0.06), (0.06, -0.08), (0.0, 0.05)], 0.0, 0.003, "Glass", axis="y",
            xf=T(0.06, 0.0, 0.03))
    return [p]


def ash_film():
    """A grey film of ash on a sill: a metre along X (scaled to the sill),
    0.2 deep, a few millimetres thick, thicker in drifts."""
    p = Part("static")
    w = Wear(47)
    p.box((1.0, 0.002, 0.18), (0, 0.001, 0), "Plaster")
    for k in range(5):
        x = -0.4 + k * 0.2 + w.j(0.04)
        blob(p, (0.07, 0.006, 0.05), (x, 0.002, w.j(0.04)), "Plaster", segs=8, rings=4)
    return [p]


# --- For Story --------------------------------------------------------------------------------

def dog_bowl():
    """An enamel dog's bowl, empty, by the door: 0.22 across."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.09, 0.0), (0.11, 0.06), (0.105, 0.065), (0.08, 0.012),
             (0.0, 0.012)], "Enamel", segs=18, xf=UP)
    p.lathe([(0.105, 0.059), (0.113, 0.059), (0.113, 0.067), (0.105, 0.067)], "Blue", segs=18,
            closed=True, xf=UP)
    return [p]


def lead_hook():
    """A coat hook by the door with a dog's lead hanging on it, the clip
    at its end (the wall's face at z 0, the hook at y 0)."""
    p = Part("static")
    p.box((0.1, 0.14, 0.02), (0, 0, -0.01), "Timber", bevel=0.004, segs=1)
    rod(p, (0, 0.02, -0.02), (0, 0.06, -0.09), 0.008, "Iron", segs=6)
    # The lead: a loop over the hook, falling to the clip.
    pts = [(0.0, 0.05, -0.08), (0.02, -0.1, -0.06), (0.01, -0.3, -0.05), (-0.01, -0.5, -0.05)]
    for a, b in zip(pts, pts[1:]):
        p.box((0.018, math.dist(a, b), 0.004), (0, 0, 0), "Leather",
              xf=T(*[(u + v) / 2 for u, v in zip(a, b)])
              @ RZ(math.atan2(b[0] - a[0], a[1] - b[1])))
    p.box((0.012, 0.03, 0.008), (-0.01, -0.515, -0.05), "Brass", bevel=0.002, segs=1)
    return [p]


def dog_collar():
    """The dog's collar on the lead's clip, placed with `lead_hook` (the
    same origin): hung there once Lev has been home."""
    p = Part("static")
    p.lathe([(0.055, -0.009), (0.065, -0.009), (0.065, 0.009), (0.055, 0.009)], "Leather",
            segs=14, closed=True, xf=T(-0.01, -0.585, -0.05) @ RX(0.2))
    p.cyl(0.012, -0.004, 0.004, "Brass", segs=8, xf=T(-0.01, -0.645, -0.06))
    return [p]


def pigeonholes():
    """The post office's sorting rack on the wall: 3 x 4 pigeonholes, a
    few letters in them, one sticking out (the wall's face at z 0, the
    rack's foot at y 0)."""
    p = Part("static")
    W, H, D = 0.9, 0.7, 0.28
    p.box((W, H, 0.012), (0, H / 2, -0.006), "Timber")
    for k in range(5):
        p.box((W - 0.024, 0.012, D), (0, k * H / 4, -D / 2 - 0.012), "Timber")
    for k in range(4):
        p.box((0.012, H, D), (-W / 2 + 0.006 + k * (W - 0.012) / 3, H / 2, -D / 2 - 0.012),
              "Timber")
    w = Wear(48)
    for (c, r, n) in ((0, 1, 3), (1, 1, 1), (2, 2, 2), (1, 3, 4), (0, 2, 1), (2, 0, 2)):
        x = -W / 2 + (c + 0.5) * W / 3
        for k in range(n):
            board(p, (0.2, 0.006, 0.14), (x + w.j(0.02), r * H / 4 + 0.012 + 0.008 * k + 0.003,
                                           -0.1 - 0.03 * (k == n - 1 and c == 1)),
                  "Paper" if k % 2 == 0 else "Cream", wear=w, bevel=0.0)
    p.box((0.12, 0.003, 0.09), (0.0, 3 * H / 4 + 0.05, -D - 0.03), "Paper",
          xf=T(0, 3 * H / 4, -D) @ RX(-0.5) @ T(0, -3 * H / 4, D))
    return [p]


def ledger_open():
    """A ledger lying open, a pen across it: 0.46 x 0.04 x 0.32."""
    p = Part("static")
    for sx in (-1, 1):
        p.box((0.22, 0.02, 0.32), (sx * 0.115, 0.01, 0), "Leather", bevel=0.003, segs=1,
              xf=T(0, 0, 0) @ RZ(sx * -0.03))
        p.box((0.21, 0.008, 0.3), (sx * 0.112, 0.024, 0), "Paper",
              xf=RZ(sx * -0.03))
        for k in range(10):
            p.box((0.18, 0.001, 0.002), (sx * 0.112, 0.0285, -0.12 + k * 0.026), "Blue",
                  xf=RZ(sx * -0.03))
    rod(p, (-0.15, 0.033, 0.08), (0.05, 0.033, 0.13), 0.004, "Black", segs=5)
    return [p]


for name, build in (("suitcase_open", suitcase_open), ("suitcase_shut", suitcase_shut),
                    ("bundle", bundle), ("belongings_box", belongings_box),
                    ("coat_hook", coat_hook), ("picture_frame", picture_frame),
                    ("boots", boots), ("helmet_lamp", helmet_lamp),
                    ("carbide_lamp", carbide_lamp), ("lunch_pail", lunch_pail),
                    ("jacket_hook", jacket_hook), ("tally_board", tally_board),
                    ("dishes", dishes), ("jars", jars), ("books", books), ("rug", rug),
                    ("broom", broom), ("dishes_broken", dishes_broken),
                    ("books_charred", books_charred), ("frame_charred", frame_charred),
                    ("ash_film", ash_film), ("dog_bowl", dog_bowl), ("lead_hook", lead_hook),
                    ("dog_collar", dog_collar),
                    ("pigeonholes", pigeonholes), ("ledger_open", ledger_open)):
    prop(name, build)
