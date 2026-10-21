"""What the people who worked in the Zone's other buildings left: visual
only (no colliders), dressed in by `world::clutter`'s `workplace` (a few
of a building's set, seeded). Origins as clutter.py's: on the surface
under the thing's centre; things for a wall have theirs on the wall's
face at their fixing, standing out towards -Z.

The shop: flour_sacks, price_board, shop_scales.
The log cabin: woodpile, hurricane_lamp, washstand, snowshoes.
The offices (the pump station, the mast hut, the guard booth, the signal
box): wall_clock, wall_telephone, gauge_board, oil_cans, toolbox.
The barn: pitchfork, milk_churn, harness_peg.
The halls (the bus depot, the rail shed, the factory): tyre_stack.
The shed: garden_tools, watering_can, wheelbarrow.

Materials as clutter.py's.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import UP, ALONG_X, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

_kit.AO_DISTANCE = 0.2
_kit.AO_SKY = "indoors"


def ring(p, r, t, at, mat, segs=16, xf=None, scale=(1.0, 1.0, 1.0)):
    """A round bar bent into a ring of radius `r` (to the bar's middle),
    `t` thick, in the XY plane about `at` (then `xf`)."""
    prof = [(r + t * 0.5 * math.cos(a), t * 0.5 * math.sin(a))
            for a in (2 * math.pi * k / 6 for k in range(6))]
    x = T(*at) @ S(*scale)
    if xf is not None:
        x = xf @ x
    p.lathe(prof, mat, segs=segs, closed=True, xf=x)


# --- The shop ---------------------------------------------------------------------------------

def flour_sacks():
    """Two sacks of flour slumped together and a third half-empty, its neck
    folded over: 0.75 x 0.5, 0.65 tall."""
    p = Part("static")
    w = Wear(81)
    for k, (x, z, h, lean) in enumerate(((-0.2, 0.04, 0.58, 0.06), (0.17, 0.0, 0.55, -0.1),
                                         (0.0, -0.12, 0.34, 0.0))):
        prof = [(0.0, 0.0), (0.15, 0.0), (0.19, 0.05), (0.2, h * 0.45), (0.18, h * 0.8),
                (0.11, h * 0.94), (0.045, h), (0.0, h + 0.005)]
        xf = T(x, 0, z) @ RY(0.4 * k + w.j(0.2)) @ RZ(lean) @ S(1.0, 1.0, 0.72)
        p.lathe(prof, "Canvas", segs=12, xf=xf @ UP)
        if k < 2:
            # The tied neck and its string.
            p.lathe([(0.0, 0.0), (0.04, 0.0), (0.03, 0.05), (0.055, 0.09), (0.0, 0.1)],
                    "Canvas", segs=8, xf=xf @ T(0, h - 0.005, 0) @ UP)
            ring(p, 0.036, 0.008, (0, 0, 0), "Rope", segs=8,
                 xf=xf @ T(0, h + 0.02, 0) @ RX(math.pi / 2))
        else:
            # Folded over, the flour showing at the fold.
            p.box((0.2, 0.04, 0.16), (0, h + 0.01, -0.02), "Canvas", bevel=0.015, segs=1,
                  xf=xf @ T(0, h, 0) @ RX(0.5) @ T(0, -h, 0))
    # A dusting of flour on the floor in front, spilt from the open one.
    p.cyl(0.1, 0.001, 0.004, "Cream", segs=12, xf=T(0.05, 0, -0.33) @ S(1.4, 1.0, 0.7) @ UP)
    return [p]


def price_board():
    """A blackboard of prices, chalked by hand, in a plain frame: the wall's
    face at z 0, 0.6 x 0.45."""
    p = Part("static")
    w = Wear(82)
    W, H = 0.6, 0.45
    p.box((W - 0.06, H - 0.06, 0.012), (0, 0, -0.006), "Black")
    for sy in (-1, 1):
        p.box((W, 0.03, 0.022), (0, sy * (H / 2 - 0.015), -0.011), "Timber", bevel=0.003,
              segs=1)
    for sx in (-1, 1):
        p.box((0.03, H - 0.06, 0.022), (sx * (W / 2 - 0.015), 0, -0.011), "Timber",
              bevel=0.003, segs=1)
    # Rows: the goods' names as strokes, a gap, the price. Read from the
    # room (-Z), the left is +X.
    for k in range(6):
        y = H / 2 - 0.07 - k * 0.055
        x = W / 2 - 0.06
        for _ in range(3 + (k * 5) % 4):
            lw = 0.02 + w.rng.random() * 0.025
            p.box((lw, 0.012, 0.003), (x - lw / 2, y + w.j(0.002), -0.0135), "Cream")
            x -= lw + 0.01
        for j in range(2 + k % 2):
            p.box((0.012, 0.022, 0.003), (-W / 2 + 0.08 - j * 0.018, y, -0.0135), "Cream")
    # A stub of chalk on the frame's bottom rail.
    rod(p, (0.12, -H / 2 + 0.034, -0.012), (0.16, -H / 2 + 0.034, -0.014), 0.004, "Cream",
        segs=5)
    return [p]


def shop_scales():
    """Counter scales, two brass pans on the beam's ends, a pile of weights
    beside: 0.45 x 0.2, 0.2 tall."""
    p = Part("static")
    p.box((0.32, 0.05, 0.16), (-0.04, 0.025, 0), "Painted", bevel=0.008, segs=1)
    p.box((0.05, 0.06, 0.05), (-0.04, 0.08, 0), "Painted", bevel=0.005, segs=1)
    p.box((0.3, 0.012, 0.03), (-0.04, 0.116, 0), "Iron", bevel=0.003, segs=1)
    for x in (-0.16, 0.08):
        rod(p, (x, 0.12, 0), (x, 0.135, 0), 0.008, "Iron", segs=6)
        p.lathe([(0.0, 0.0), (0.03, 0.0), (0.085, 0.025), (0.09, 0.03), (0.084, 0.031),
                 (0.0, 0.006)], "Brass", segs=18, xf=T(x, 0.135, 0) @ UP)
    # A pointer at the middle, standing up between the pans.
    rod(p, (-0.04, 0.122, -0.016), (-0.04, 0.2, -0.016), 0.003, "Black", segs=4)
    # A stack of weights on the counter, the smallest on top.
    y = 0.0
    for r, h in ((0.03, 0.035), (0.024, 0.028), (0.018, 0.022)):
        p.lathe([(0.0, 0.0), (r, 0.0), (r, h * 0.8), (r * 0.5, h), (0.0, h)], "Brass",
                segs=12, xf=T(0.19, y, 0.02) @ UP)
        y += h * 0.8
    p.lathe([(0.0, 0.0), (0.02, 0.0), (0.02, 0.02), (0.0, 0.025)], "Brass", segs=10,
            xf=T(0.19, 0.0, -0.05) @ UP)
    return [p]


# --- The cabin --------------------------------------------------------------------------------

def woodpile():
    """Split logs stacked by the stove, their ends to the room, a few
    sticks of kindling on top: 0.8 x 0.42, 0.45 tall."""
    p = Part("static")
    w = Wear(83)
    r = 0.06
    rows = (6, 5, 4, 2)
    # Each row in the hollows of the one under it, the logs just touching:
    # bark round them, their sawn ends pale; every third split, its face
    # pale too.
    for k, n in enumerate(rows):
        y = r + k * 0.105
        for j in range(n):
            x = (j - (n - 1) / 2) * 0.124 + w.j(0.003)
            z0, z1 = -0.2 + w.j(0.02), 0.2 + w.j(0.02)
            spin = T(x, y, 0) @ RZ(w.rng.random() * 6.0)
            if (j + 2 * k) % 3 == 0:
                arc = [(r * math.cos(a), r * math.sin(a))
                       for a in (math.pi * i / 6 for i in range(7))]
                p.prism(arc, z0, z1, "Timber", axis="z", xf=spin @ T(0, -r * 0.35, 0))
            else:
                p.cyl(r + w.j(0.002), z0, z1, "Boards", segs=9, xf=spin)
                for e in (z0, z1):
                    p.cyl(r * 0.88, e - 0.004, e + 0.004, "Timber", segs=9, xf=spin)
    top = r + (len(rows) - 1) * 0.105 + r
    for k in range(3):
        rod(p, (-0.12 + k * 0.08, top - 0.004, -0.15 + w.j(0.03)),
            (-0.08 + k * 0.07, top - 0.004, 0.17 + w.j(0.03)), 0.012, "Timber", segs=5)
    return [p]


def hurricane_lamp():
    """A hurricane lamp: its tin font, the glass in its wire guard, the
    chimney cap and the bail: 0.16 across, 0.34 tall."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.07, 0.0), (0.075, 0.015), (0.07, 0.05), (0.04, 0.065),
             (0.0, 0.065)], "Red", segs=16, xf=UP)
    p.lathe([(0.0, 0.065), (0.035, 0.065), (0.055, 0.1), (0.06, 0.15), (0.045, 0.2),
             (0.03, 0.215), (0.0, 0.215)], "Glass", segs=14, xf=UP)
    p.lathe([(0.0, 0.21), (0.045, 0.21), (0.05, 0.225), (0.03, 0.25), (0.022, 0.28),
             (0.0, 0.285)], "Red", segs=14, xf=UP)
    # The guard's wires, clear of the glass.
    for k in range(4):
        a = k * math.pi / 2 + 0.4
        c, s = math.cos(a), math.sin(a)
        rod(p, (0.06 * c, 0.06, 0.06 * s), (0.072 * c, 0.15, 0.072 * s), 0.002, "Iron", segs=3)
        rod(p, (0.072 * c, 0.15, 0.072 * s), (0.05 * c, 0.215, 0.05 * s), 0.002, "Iron", segs=3)
    # The side tubes up to the cap, and the bail over it.
    for sx in (-1, 1):
        rod(p, (sx * 0.07, 0.04, 0), (sx * 0.075, 0.24, 0), 0.005, "Red", segs=5)
        rod(p, (sx * 0.075, 0.24, 0), (sx * 0.03, 0.235, 0), 0.004, "Red", segs=4)
    for k in range(6):
        a0, a1 = math.pi * k / 6, math.pi * (k + 1) / 6
        rod(p, (0.075 * math.cos(a0), 0.24 + 0.09 * math.sin(a0), 0.0),
            (0.075 * math.cos(a1), 0.24 + 0.09 * math.sin(a1), 0.0), 0.0025, "Iron", segs=3)
    return [p]


def washstand():
    """A washstand: an enamel basin on its top, the jug stood in it, a towel
    over the rail at the side; 0.6 x 0.4, its top 0.78 up."""
    p = Part("static")
    w = Wear(84)
    top = 0.78
    board(p, (0.6, 0.025, 0.4), (0, top - 0.0125, 0), "Timber", wear=w)
    board(p, (0.56, 0.02, 0.36), (0, 0.18, 0), "Timber", wear=w)
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.035, top - 0.025, 0.035), (sx * 0.27, (top - 0.025) / 2, sz * 0.17),
                  "Timber", bevel=0.003)
    # A splash board along the back.
    board(p, (0.6, 0.14, 0.02), (0, top + 0.07, 0.19), "Timber", wear=w)
    # The basin and the jug in it.
    p.lathe([(0.0, 0.0), (0.08, 0.0), (0.16, 0.08), (0.17, 0.09), (0.165, 0.093),
             (0.08, 0.008), (0.0, 0.008)], "Enamel", segs=20, xf=T(-0.03, top, 0.0) @ UP)
    p.lathe([(0.0, 0.0), (0.06, 0.0), (0.075, 0.08), (0.065, 0.17), (0.05, 0.21),
             (0.06, 0.24), (0.0, 0.24)], "Enamel", segs=16, xf=T(-0.03, top + 0.008, 0.0) @ UP)
    rod(p, (0.035, top + 0.19, 0.0), (0.085, top + 0.15, 0.0), 0.008, "Enamel", segs=5)
    rod(p, (0.085, top + 0.15, 0.0), (0.06, top + 0.07, 0.0), 0.008, "Enamel", segs=5)
    # The towel rail on the side, and the towel over it.
    rod(p, (0.33, top - 0.12, -0.13), (0.33, top - 0.12, 0.13), 0.008, "Timber", segs=6)
    for sz in (-1, 1):
        rod(p, (0.288, top - 0.12, sz * 0.13), (0.33, top - 0.12, sz * 0.13), 0.007, "Timber",
            segs=5)
    for sx in (-1, 1):
        p.box((0.004, 0.24, 0.2), (0.33 + sx * 0.011, top - 0.23, 0.0), "Cream",
              xf=T(0.33, top - 0.12, 0) @ RZ(sx * 0.06) @ T(-0.33, -top + 0.12, 0))
    return [p]


def snowshoes():
    """A pair of bentwood snowshoes, gut-strung, hung on a nail by their
    tails: the wall's face at z 0, the nail at the origin, the shoes 0.9
    long below it."""
    p = Part("static")
    rod(p, (0, 0, 0), (0, 0, -0.07), 0.004, "Iron", segs=4)
    for k, (x, z) in enumerate(((-0.08, -0.02), (0.08, -0.05))):
        cy = -0.55 - 0.02 * k
        rx, ry = 0.17, 0.3
        # The frame: an ash bar bent round the toe, its ends lashed into a
        # tail up to the nail.
        t = 0.024
        ring(p, 1.0, t / rx, (0, 0, 0), "Timber", segs=20,
             xf=T(x, cy, z) @ S(rx, ry, 0.022 * rx / t))
        tip = (x * 0.25, -0.04, z)
        for sx in (-1, 1):
            rod(p, (x + sx * rx * 0.6, cy + ry * 0.78, z), tip, 0.011, "Timber", segs=5)
        # The gut webbing filling it, a rawhide lacing over it.
        p.cyl(1.0, -0.0015, 0.0015, "Straw", segs=16,
              xf=T(x, cy, z) @ S(rx - 0.01, ry - 0.01, 1.0))
        for j in range(-4, 5):
            y = j * 0.06
            half = (rx - 0.012) * math.sqrt(max(0.0, 1.0 - (y / ry) ** 2))
            if half > 0.02:
                p.box((2 * half, 0.008, 0.003), (x, cy + y, z - 0.004), "Leather")
        # The binding for the boot, a little up from the middle.
        p.box((0.11, 0.04, 0.014), (x, cy + 0.05, z - 0.013), "Leather", bevel=0.003, segs=1)
    return [p]


# --- The offices ------------------------------------------------------------------------------

def wall_clock():
    """A round office clock in its wooden case, stopped at twenty to four:
    the wall's face at z 0, 0.32 across."""
    p = Part("static")
    p.lathe([(0.135, 0.0), (0.16, 0.0), (0.16, -0.05), (0.15, -0.06), (0.135, -0.06)],
            "Timber", segs=28, closed=True)
    p.cyl(0.137, -0.04, 0.0, "Cream", segs=28)
    ring(p, 0.142, 0.01, (0, 0, -0.062), "Brass", segs=28)
    for k in range(12):
        a = 2 * math.pi * k / 12
        L = 0.022 if k % 3 == 0 else 0.012
        p.box((0.005, L, 0.004), (0, 0.115, -0.042),
              "Black", xf=RZ(-a))
    # The hands: twenty to four.
    # Seen from the room (-Z), clockwise turns towards -X.
    for (a, L, t, z) in ((math.radians(110), 0.07, 0.007, -0.0475),
                         (math.radians(240), 0.105, 0.005, -0.052)):
        p.box((t, L, 0.003), (0, L / 2 - 0.01, z), "Black", xf=RZ(a))
    p.cyl(0.008, -0.058, -0.04, "Black", segs=8)
    return [p]


def wall_telephone():
    """An oak wall telephone: its two bells, the mouthpiece on its arm, the
    crank, the receiver hung on its hook: the wall's face at z 0, the box's
    middle at the origin; 0.36 x 0.36."""
    p = Part("static")
    p.box((0.2, 0.3, 0.1), (0, 0, -0.05), "Timber", bevel=0.006, segs=1)
    # A cornice and a foot, their backs off the wall (not in the box's plane).
    p.box((0.22, 0.04, 0.116), (0, 0.17, -0.062), "Timber", bevel=0.004, segs=1)
    p.box((0.22, 0.03, 0.116), (0, -0.165, -0.062), "Timber", bevel=0.004, segs=1)
    # The bells, domed, on the front's top.
    for sx in (-1, 1):
        p.lathe([(0.0, 0.0), (0.034, 0.0), (0.03, 0.02), (0.015, 0.03), (0.0, 0.032)], "Brass",
                segs=14, xf=T(sx * 0.05, 0.1, -0.1) @ RX(math.pi))
    # The mouthpiece on its arm.
    rod(p, (0, -0.02, -0.1), (0, 0.0, -0.17), 0.01, "Black", segs=6)
    p.lathe([(0.0, 0.0), (0.015, 0.0), (0.035, 0.035), (0.03, 0.04), (0.0, 0.01)], "Black",
            segs=12, xf=T(0, 0.0, -0.17) @ RX(math.pi))
    # The crank on the +X side.
    rod(p, (0.1, -0.02, -0.05), (0.13, -0.02, -0.05), 0.006, "Iron", segs=5)
    rod(p, (0.13, -0.02, -0.05), (0.13, -0.08, -0.06), 0.005, "Iron", segs=5)
    rod(p, (0.13, -0.08, -0.06), (0.16, -0.08, -0.06), 0.008, "Black", segs=6)
    # The hook on the -X side and the receiver hanging on it.
    rod(p, (-0.1, 0.06, -0.05), (-0.14, 0.07, -0.05), 0.005, "Iron", segs=5)
    p.lathe([(0.0, 0.0), (0.022, 0.0), (0.022, 0.03), (0.014, 0.05), (0.014, 0.12),
             (0.02, 0.15), (0.0, 0.15)], "Black", segs=10,
            xf=T(-0.15, -0.09, -0.05) @ UP)
    # The cord from its foot back to the box.
    pts = [(-0.15, -0.09, -0.05), (-0.14, -0.15, -0.06), (-0.11, -0.16, -0.06),
           (-0.1, -0.12, -0.05)]
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, 0.004, "Black", segs=4)
    return [p]


def gauge_board():
    """A board of three pressure gauges, their pipes running down out of the
    board: the wall's face at z 0, 0.7 x 0.45."""
    p = Part("static")
    w = Wear(85)
    p.box((0.7, 0.45, 0.025), (0, 0, -0.0125), "Painted", bevel=0.004, segs=1)
    for k, x in enumerate((-0.22, 0.0, 0.22)):
        y = 0.05
        # A brass cup round a cream face, the needle in front of it.
        p.lathe([(0.06, -0.025), (0.072, -0.025), (0.072, -0.068), (0.06, -0.068)], "Brass",
                segs=20, closed=True, xf=T(x, y, 0))
        p.cyl(0.061, -0.06, -0.03, "Cream", segs=20, xf=T(x, y, 0))
        a = (-1.9, -0.4, 1.3)[k] + w.j(0.1)
        p.box((0.004, 0.05, 0.003), (0, 0.022, -0.065), "Black",
              xf=T(x, y, 0) @ RZ(a))
        p.cyl(0.006, -0.072, -0.06, "Black", segs=6, xf=T(x, y, 0))
        # Its pipe down from under it, out through the board's foot.
        rod(p, (x, y - 0.072, -0.045), (x, -0.6, -0.045), 0.009, "Copper", segs=6)
        p.lathe([(0.0, 0.0), (0.016, 0.0), (0.016, 0.03), (0.0, 0.03)], "Brass", segs=6,
                xf=T(x, -0.16, -0.045) @ UP)
    # A brass plate under the gauges.
    p.box((0.2, 0.04, 0.003), (0, -0.12, -0.0265), "Brass")
    return [p]


def oil_cans():
    """A square oil can, a pump oiler with its long spout, and a funnel on
    its side: 0.45 x 0.25, 0.35 tall."""
    p = Part("static")
    p.box((0.2, 0.28, 0.12), (-0.1, 0.14, 0.0), "Red", bevel=0.01, segs=2)
    p.cyl(0.02, 0.28, 0.31, "Iron", segs=10, xf=T(-0.15, 0, 0) @ UP)
    rod(p, (-0.07, 0.28, 0.0), (-0.07, 0.33, 0.0), 0.006, "Iron", segs=5)
    rod(p, (-0.07, 0.33, 0.0), (-0.02, 0.33, 0.0), 0.006, "Iron", segs=5)
    rod(p, (-0.02, 0.33, 0.0), (-0.02, 0.28, 0.0), 0.006, "Iron", segs=5)
    # The oiler: a squat cone, its spout reaching up and out.
    p.lathe([(0.0, 0.0), (0.06, 0.0), (0.062, 0.02), (0.04, 0.06), (0.012, 0.075),
             (0.0, 0.075)], "Brass", segs=16, xf=T(0.12, 0, -0.03) @ UP)
    rod(p, (0.12, 0.07, -0.03), (0.2, 0.26, -0.06), 0.005, "Brass", segs=5)
    # The funnel stood mouth down to drain.
    p.lathe([(0.054, 0.0), (0.06, 0.0), (0.014, 0.06), (0.014, 0.12), (0.009, 0.12),
             (0.009, 0.06)], "Zinc", segs=14, closed=True, xf=T(0.12, 0, 0.1) @ UP)
    return [p]


def toolbox():
    """A steel toolbox, shut, a spanner and a hammer left on the floor
    beside it: 0.6 x 0.3, 0.25 tall."""
    p = Part("static")
    p.box((0.42, 0.16, 0.2), (-0.06, 0.08, 0), "Red", bevel=0.006, segs=1)
    p.prism([(-0.1, 0.16), (0.1, 0.16), (0.08, 0.2), (-0.08, 0.2)], -0.27, 0.15, "Red",
            axis="x")
    for x in (-0.12, 0.0):
        rod(p, (x, 0.2, 0), (x, 0.23, 0), 0.006, "Iron", segs=5)
    rod(p, (-0.12, 0.23, 0), (0.0, 0.23, 0), 0.008, "Black", segs=6)
    for sx in (-0.2, 0.08):
        p.box((0.03, 0.03, 0.01), (sx, 0.13, -0.102), "Steel")
    # The spanner and the hammer.
    p.box((0.18, 0.004, 0.022), (0.28, 0.006, -0.06), "Steel", xf=RY(0.3))
    for sx in (-1, 1):
        p.box((0.026, 0.012, 0.04), (0.28 + sx * 0.09, 0.006, -0.06), "Steel", xf=RY(0.3))
    rod(p, (0.17, 0.014, 0.1), (0.3, 0.014, 0.06), 0.012, "Timber", segs=6)
    p.box((0.03, 0.03, 0.1), (0.31, 0.015, 0.055), "Iron", bevel=0.004, segs=1,
          xf=T(0.31, 0, 0.055) @ RY(-0.3) @ T(-0.31, 0, -0.055))
    return [p]


# --- The barn ---------------------------------------------------------------------------------

def pitchfork():
    """A hay fork leaning on the wall, its tines up: the wall's face at z
    0, its foot 0.32 out on the floor."""
    p = Part("static")
    foot, top = (0.0, 0.0, -0.32), (0.0, 1.25, -0.09)
    rod(p, foot, top, 0.016, "Timber", segs=7)
    d = [(t - f) for f, t in zip(foot, top)]
    L = math.sqrt(sum(v * v for v in d))
    u = [v / L for v in d]
    ferrule = [t + v * 0.08 for t, v in zip(top, u)]
    rod(p, top, ferrule, 0.02, "Iron", segs=7)
    for x in (-0.07, 0.0, 0.07):
        a = (x, ferrule[1], ferrule[2])
        b = (x, ferrule[1] + 0.15, ferrule[2] + 0.015)
        c = (x * 1.05, ferrule[1] + 0.3, ferrule[2] - 0.02)
        rod(p, a, b, 0.006, "Iron", segs=5)
        rod(p, b, c, 0.005, "Iron", segs=5)
    rod(p, (-0.075, ferrule[1], ferrule[2]), (0.075, ferrule[1], ferrule[2]), 0.009, "Iron",
        segs=6)
    return [p]


def milk_churn():
    """A milk churn, its lid on: 0.36 across, 0.72 tall."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.16, 0.0), (0.17, 0.02), (0.17, 0.43), (0.15, 0.47), (0.09, 0.56),
             (0.085, 0.62), (0.095, 0.63), (0.0, 0.63)], "Zinc", segs=20, xf=UP)
    p.lathe([(0.0, 0.625), (0.1, 0.625), (0.1, 0.655), (0.06, 0.68), (0.03, 0.7),
             (0.03, 0.72), (0.0, 0.72)], "Zinc", segs=18, xf=UP)
    ring(p, 0.172, 0.012, (0, 0, 0), "Zinc", segs=20, xf=T(0, 0.2, 0) @ RX(math.pi / 2))
    for sx in (-1, 1):
        rod(p, (sx * 0.12, 0.5, 0), (sx * 0.17, 0.56, 0), 0.008, "Zinc", segs=5)
        rod(p, (sx * 0.17, 0.56, 0), (sx * 0.1, 0.6, 0), 0.008, "Zinc", segs=5)
    return [p]


def harness_peg():
    """A horse's collar and its hames hung on a peg, the reins looped
    below: the wall's face at z 0, the peg at the origin."""
    p = Part("static")
    rod(p, (0, 0, 0), (0, 0.03, -0.12), 0.018, "Timber", segs=7)
    # The collar: a stuffed leather ring, narrower at its top.
    c = (0.0, -0.27, -0.07)
    ring(p, 1.0, 0.07 / 0.2, (0, 0, 0), "Leather", segs=18,
         xf=T(*c) @ S(0.17, 0.24, 0.2))
    # The hames, brass-tipped, along its sides.
    for sx in (-1, 1):
        pts = [(sx * 0.1, -0.06, -0.13), (sx * 0.21, -0.2, -0.13), (sx * 0.21, -0.36, -0.12),
               (sx * 0.13, -0.49, -0.11)]
        for a, b in zip(pts, pts[1:]):
            rod(p, a, b, 0.012, "Iron", segs=5)
        p.sphere(0.018, pts[0], "Brass", segs=8, rings=5)
    # The reins, looped down from the peg in front of the collar.
    pts = [(-0.03, 0.01, -0.12), (-0.08, -0.35, -0.16), (-0.06, -0.75, -0.17),
           (0.04, -0.78, -0.17), (0.07, -0.4, -0.16), (0.03, 0.01, -0.12)]
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, 0.008, "Leather", segs=4)
    return [p]


# --- The halls --------------------------------------------------------------------------------

def tyre_stack():
    """Three lorry tyres stacked, a little out of true, a rim in the top
    one: 0.8 across, 0.6 tall."""
    p = Part("static")
    w = Wear(86)
    prof = [(0.22, 0.03), (0.25, 0.0), (0.34, 0.0), (0.38, 0.04), (0.39, 0.1), (0.38, 0.16),
            (0.34, 0.2), (0.25, 0.2), (0.22, 0.17)]
    for k in range(3):
        x, z = w.j(0.03), w.j(0.03)
        p.lathe(prof, "Rubber", segs=18, closed=True,
                xf=T(x, 0.195 * k, z) @ RY(w.j(1.0)) @ RX(w.j(0.02)) @ UP)
    p.lathe([(0.19, 0.06), (0.235, 0.05), (0.235, 0.15), (0.19, 0.14), (0.18, 0.1)], "Rusty",
            segs=20, closed=True, xf=T(0.0, 0.39, 0.0) @ UP)
    p.cyl(0.19, 0.095, 0.105, "Rusty", segs=20, xf=T(0.0, 0.39, 0.0) @ UP)
    return [p]


# --- The shed ---------------------------------------------------------------------------------

def garden_tools():
    """A spade, a rake and a hoe hung from a rail of pegs: the wall's face
    at z 0, the rail at the origin, the tools hanging 1.35 below it."""
    p = Part("static")
    board(p, (0.9, 0.08, 0.02), (0, 0, -0.01), "Timber", bevel=0.004)
    for x in (-0.3, -0.06, 0.06, 0.3):
        rod(p, (x, 0.0, -0.02), (x, 0.03, -0.1), 0.01, "Timber", segs=6)
    # The spade: its D grip over the peg, the blade down at the floor.
    x = -0.3
    rod(p, (x - 0.05, 0.04, -0.07), (x + 0.05, 0.04, -0.07), 0.01, "Timber", segs=6)
    for sx in (-1, 1):
        rod(p, (x + sx * 0.05, 0.04, -0.07), (x, -0.08, -0.07), 0.009, "Timber", segs=6)
    rod(p, (x, -0.08, -0.07), (x, -1.0, -0.06), 0.017, "Timber", segs=7)
    p.box((0.18, 0.3, 0.01), (x, -1.14, -0.055), "Rusty", bevel=0.003, segs=1)
    p.box((0.05, 0.06, 0.03), (x, -0.98, -0.058), "Iron", bevel=0.004, segs=1)
    # The rake hung by its head across two pegs, the teeth out.
    hy = 0.02
    p.box((0.4, 0.03, 0.03), (0.0, hy, -0.09), "Iron", bevel=0.004, segs=1)
    for k in range(10):
        tx = -0.18 + k * 0.04
        rod(p, (tx, hy + 0.01, -0.1), (tx, hy + 0.07, -0.12), 0.004, "Iron", segs=4)
    rod(p, (0.0, hy - 0.01, -0.09), (0.03, -1.3, -0.08), 0.015, "Timber", segs=7)
    # The hoe on the last peg by its blade.
    x = 0.3
    p.box((0.14, 0.09, 0.006), (x, -0.01, -0.105), "Rusty", bevel=0.002, segs=1)
    rod(p, (x, -0.05, -0.1), (x - 0.02, -1.25, -0.06), 0.014, "Timber", segs=7)
    return [p]


def watering_can():
    """A galvanised watering can, its rose on the spout: 0.5 x 0.2, 0.4
    tall."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.095, 0.0), (0.1, 0.01), (0.1, 0.24), (0.09, 0.25), (0.0, 0.25)],
            "Zinc", segs=18, xf=S(1.0, 1.0, 0.85) @ UP)
    # The spout from low on the front, up and out, the rose on its end.
    a, b = (0.08, 0.04, 0.0), (0.3, 0.27, 0.0)
    rod(p, a, b, 0.016, "Zinc", segs=8)
    d = [v - u for u, v in zip(a, b)]
    yaw = math.atan2(d[1], d[0])
    p.lathe([(0.0, 0.0), (0.02, 0.0), (0.04, 0.035), (0.04, 0.045), (0.0, 0.045)], "Zinc",
            segs=12, xf=T(*b) @ RZ(yaw - math.pi / 2) @ UP)
    # The handle over the top and the one at the back.
    pts = [(-0.06, 0.25, 0.0), (-0.03, 0.34, 0.0), (0.05, 0.34, 0.0), (0.07, 0.25, 0.0)]
    for u, v in zip(pts, pts[1:]):
        rod(p, u, v, 0.009, "Zinc", segs=6)
    pts = [(-0.098, 0.2, 0.0), (-0.16, 0.18, 0.0), (-0.16, 0.08, 0.0), (-0.098, 0.06, 0.0)]
    for u, v in zip(pts, pts[1:]):
        rod(p, u, v, 0.008, "Zinc", segs=6)
    return [p]


def wheelbarrow():
    """A wheelbarrow, its steel tray rusted, the wheel to -Z: 0.62 wide,
    1.4 long, 0.6 tall."""
    p = Part("static")
    # The wheel.
    wz, wr = -0.55, 0.17
    p.lathe([(0.03, -0.035), (0.13, -0.035), (0.17, -0.02), (0.17, 0.02), (0.13, 0.035),
             (0.03, 0.035)], "Rubber", segs=20, closed=True, xf=T(0, wr, wz) @ ALONG_X)
    p.cyl(0.13, -0.025, 0.025, "Rusty", segs=16, xf=T(0, wr, wz) @ ALONG_X)
    rod(p, (-0.06, wr, wz), (0.06, wr, wz), 0.012, "Iron", segs=6)
    # The handles from the axle back, the legs under them.
    for sx in (-1, 1):
        rod(p, (sx * 0.055, wr, wz), (sx * 0.25, 0.42, 0.68), 0.02, "Timber", segs=7)
        rod(p, (sx * 0.17, 0.32, 0.25), (sx * 0.19, 0.012, 0.3), 0.015, "Iron", segs=5)
        rod(p, (sx * 0.19, 0.012, 0.3), (sx * 0.19, 0.012, 0.42), 0.012, "Iron", segs=5)
    # The tray: one pressed pan, its sides sloping out (a four-sided
    # lathe, square to the axes, stretched to its plan).
    fy, r0 = 0.296, math.sqrt(2.0)
    prof = [(0.0, 0.0), (r0, 0.0), (r0 * 1.5, 0.26), (r0 * 1.5 - 0.07, 0.26),
            (r0 - 0.07, 0.012), (0.0, 0.012)]
    p.lathe(prof, "Rusty", segs=4, start=45.0, smooth=False,
            xf=T(0, fy, -0.05) @ S(0.17, 1.0, 0.26) @ UP)
    return [p]


for name, build in (("flour_sacks", flour_sacks), ("price_board", price_board),
                    ("shop_scales", shop_scales), ("woodpile", woodpile),
                    ("hurricane_lamp", hurricane_lamp), ("washstand", washstand),
                    ("snowshoes", snowshoes), ("wall_clock", wall_clock),
                    ("wall_telephone", wall_telephone), ("gauge_board", gauge_board),
                    ("oil_cans", oil_cans), ("toolbox", toolbox), ("pitchfork", pitchfork),
                    ("milk_churn", milk_churn), ("harness_peg", harness_peg),
                    ("tyre_stack", tyre_stack), ("garden_tools", garden_tools),
                    ("watering_can", watering_can), ("wheelbarrow", wheelbarrow)):
    prop(name, build)
