"""Who lived in a house: the few telling things a resident left, visual only
(no colliders), dressed in by `world::clutter` (its `Resident`s) into the
slots its valley's things leave free: the partition, the hook by the door,
the sills, a corner of the front room's floor. Origins as clutter.py's: on
the surface under the thing's centre; things for a wall have theirs on the
wall's face at their fixing, standing out towards -Z.

Teacher: globe, exercise_books, slate_board.
A family with small children: cot, rocking_horse, small_boots.
Hunter: traps_hook, hide_frame.
An old couple: two_cups, reading_glasses, walking_stick.
Seamstress: sewing_machine, dress_form, cloth_pinned.
Radio ham: radio_set, logbook.
Railwayman: timetable, cap_hook.

Materials as clutter.py's.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import UP, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

_kit.AO_DISTANCE = 0.2
_kit.AO_SKY = "indoors"


# --- Teacher ----------------------------------------------------------------------------------

def globe():
    """A school globe on its stand, a little dusty: 0.3 wide, 0.45 tall."""
    p = Part("static")
    p.lathe([(0.0, 0.0), (0.1, 0.0), (0.1, 0.015), (0.03, 0.03), (0.015, 0.1), (0.0, 0.1)],
            "Black", segs=16, xf=UP)
    # The meridian ring, tilted with the axis.
    tilt = 0.41
    c = (0.0, 0.27, 0.0)
    p.lathe([(0.155, -0.008), (0.165, -0.008), (0.165, 0.008), (0.155, 0.008)], "Brass",
            segs=24, closed=True, xf=T(*c) @ RZ(tilt) @ RY(math.pi / 2))
    p.sphere(0.15, c, "Blue", segs=20, rings=12)
    # Continents: a few low lumps on the sea.
    for (a, b, s) in ((0.4, 0.3, 0.07), (2.1, -0.2, 0.06), (3.6, 0.5, 0.05), (4.8, -0.4, 0.045)):
        x = c[0] + 0.148 * math.cos(b) * math.cos(a)
        y = c[1] + 0.148 * math.sin(b)
        z = c[2] + 0.148 * math.cos(b) * math.sin(a)
        p.sphere(1.0, (0, 0, 0), "Olive", segs=8, rings=5,
                 xf=T(x, y, z) @ S(s, s * 0.35, s * 0.8))
    rod(p, (0.0, 0.1, 0.0), (0.0, 0.12, 0.0), 0.012, "Brass", segs=6)
    return [p]


def exercise_books():
    """A pile of children's exercise books, the top one open, a pencil on
    it: 0.36 x 0.05 x 0.24."""
    p = Part("static")
    w = Wear(61)
    y = 0.0
    for k in range(6):
        t = 0.006
        board(p, (0.17, t, 0.21), (-0.08 + w.j(0.01), y + t / 2, w.j(0.01)),
              ("Blue", "Cream", "Red")[k % 3], wear=w, bevel=0.001, yaw=w.j(0.12))
        y += t
    for sx in (-1, 1):
        p.box((0.085, 0.003, 0.21), (0.11 + sx * 0.043, 0.0015, 0.0), "Paper",
              xf=T(0.11, 0, 0) @ RZ(sx * -0.05) @ T(-0.11, 0, 0))
    for k in range(7):
        p.box((0.15, 0.0005, 0.002), (0.11, 0.0035 + 0.003 * (k % 2), -0.08 + k * 0.026),
              "Blue")
    rod(p, (0.05, 0.008, 0.06), (0.18, 0.008, 0.02), 0.004, "Yellow", segs=6)
    return [p]


def slate_board():
    """A child's slate in its frame hung on a nail, a sum chalked on it: the
    wall's face at z 0, 0.32 x 0.24."""
    p = Part("static")
    W, H = 0.32, 0.24
    for sy in (-1, 1):
        p.box((W, 0.025, 0.012), (0, sy * (H / 2 - 0.0125), -0.006), "Timber")
    for sx in (-1, 1):
        p.box((0.025, H - 0.05, 0.012), (sx * (W / 2 - 0.0125), 0, -0.006), "Timber")
    p.box((W - 0.05, H - 0.05, 0.006), (0, 0, -0.005), "Black")
    # Chalk: two lines of marks.
    for r, n in ((0.04, 5), (-0.03, 4)):
        for k in range(n):
            p.box((0.022, 0.004, 0.001), (-0.09 + k * 0.04, r, -0.0085), "Cream",
                  xf=T(-0.09 + k * 0.04, r, 0) @ RZ(0.3 * ((k % 3) - 1)) @
                  T(0.09 - k * 0.04, -r, 0))
    rod(p, (0, H / 2 + 0.03, 0.0), (0, H / 2 + 0.03, -0.02), 0.003, "Iron", segs=5)
    rod(p, (-0.1, H / 2 - 0.01, -0.012), (0.0, H / 2 + 0.03, -0.018), 0.0015, "Rope", segs=4)
    rod(p, (0.1, H / 2 - 0.01, -0.012), (0.0, H / 2 + 0.03, -0.018), 0.0015, "Rope", segs=4)
    return [p]


# --- A family with small children -------------------------------------------------------------

def cot():
    """A child's wooden cot, a blanket over its rail: 1.0 x 0.9 x 0.55."""
    p = Part("static")
    L, D, H = 1.0, 0.55, 0.9
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.04, H, 0.04), (sx * (L / 2 - 0.02), H / 2, sz * (D / 2 - 0.02)), "Timber",
                  bevel=0.004, segs=1)
    for sz in (-1, 1):
        for y in (0.3, H - 0.03):
            p.box((L - 0.08, 0.03, 0.025), (0, y, sz * (D / 2 - 0.02)), "Timber")
        for k in range(11):
            x = -L / 2 + 0.08 + k * (L - 0.16) / 10
            p.box((0.015, H - 0.36, 0.015), (x, 0.3 + (H - 0.36) / 2, sz * (D / 2 - 0.02)),
                  "Timber")
    for sx in (-1, 1):
        p.box((0.025, 0.45, D - 0.08), (sx * (L / 2 - 0.02), 0.55, 0), "Timber")
    p.box((L - 0.08, 0.1, D - 0.08), (0, 0.35, 0), "Ticking", bevel=0.02, segs=1)
    # The blanket over the near rail, hanging.
    p.box((0.4, 0.012, 0.3), (0.15, H - 0.005, -D / 2 + 0.11), "Wool")
    p.box((0.4, 0.32, 0.012), (0.15, H - 0.17, -D / 2 - 0.01), "Wool")
    return [p]


def rocking_horse():
    """A small wooden rocking horse, its paint worn: 0.75 x 0.6 x 0.25."""
    p = Part("static")
    # The rockers: two curved runners, in short straight pieces.
    n = 8
    for sz in (-1, 1):
        for k in range(n):
            a0 = -0.6 + 1.2 * k / n
            a1 = -0.6 + 1.2 * (k + 1) / n
            R = 0.9
            p0 = (R * math.sin(a0), R * (1 - math.cos(a0)), sz * 0.1)
            p1 = (R * math.sin(a1), R * (1 - math.cos(a1)), sz * 0.1)
            rod(p, p0, p1, 0.015, "Timber", segs=5)
        for x in (-0.22, 0.22):
            rod(p, (x, 0.03, sz * 0.1), (x * 0.8, 0.3, sz * 0.04), 0.014, "Timber", segs=5)
    p.box((0.42, 0.14, 0.12), (0, 0.36, 0), "Red", bevel=0.03, segs=1)
    p.box((0.1, 0.22, 0.08), (0.24, 0.48, 0), "Red", bevel=0.02, segs=1,
          xf=T(0.24, 0.48, 0) @ RZ(-0.5) @ T(-0.24, -0.48, 0))
    p.box((0.16, 0.08, 0.075), (0.33, 0.58, 0), "Cream", bevel=0.02, segs=1)
    p.box((0.04, 0.2, 0.02), (0.18, 0.5, 0), "Black",
          xf=T(0.18, 0.5, 0) @ RZ(-0.5) @ T(-0.18, -0.5, 0))
    p.box((0.18, 0.02, 0.13), (-0.02, 0.44, 0), "Leather")
    rod(p, (-0.2, 0.38, 0), (-0.3, 0.3, 0), 0.02, "Black", segs=5)
    return [p]


def small_boots():
    """A child's pair of boots by the door: 0.2 x 0.12."""
    p = Part("static")
    for (x, z, yaw) in ((-0.045, 0.0, 0.1), (0.05, 0.02, -0.25)):
        xf = T(x, 0, z) @ RY(yaw)
        p.box((0.06, 0.025, 0.16), (0, 0.0125, -0.01), "Black", bevel=0.006, segs=1, xf=xf)
        p.box((0.058, 0.05, 0.12), (0, 0.05, -0.03), "Red", bevel=0.012, segs=1, xf=xf)
        p.lathe([(0.0, 0.0), (0.03, 0.0), (0.03, 0.08), (0.0, 0.08)], "Red", segs=10,
                xf=xf @ T(0, 0.03, 0.035) @ UP)
    return [p]


# --- Hunter -----------------------------------------------------------------------------------

def traps_hook():
    """Two gin traps and a coil of snare wire hung on a peg board: the
    wall's face at z 0."""
    p = Part("static")
    p.box((0.5, 0.08, 0.02), (0, 0, -0.01), "Timber", bevel=0.004, segs=1)
    for x in (-0.15, 0.15):
        rod(p, (x, 0.0, -0.02), (x, 0.03, -0.08), 0.008, "Iron", segs=6)
        # The trap hangs by its chain, jaws closed: a flat ring and a plate.
        c = (x, -0.25, -0.05)
        rod(p, (x, 0.02, -0.07), (x, c[1] + 0.1, -0.05), 0.003, "Rusty", segs=4)
        p.lathe([(0.075, -0.004), (0.085, -0.004), (0.085, 0.004), (0.075, 0.004)], "Rusty",
                segs=16, closed=True, xf=T(*c) @ RX(0.12))
        p.box((0.06, 0.012, 0.004), c, "Rusty")
        p.box((0.17, 0.008, 0.01), (c[0], c[1] + 0.005, c[2] - 0.004), "Rusty",
              xf=T(*c) @ RZ(0.05) @ T(*(-v for v in c)))
    p.lathe([(0.05, -0.006), (0.06, -0.006), (0.06, 0.006), (0.05, 0.006)], "Steel", segs=14,
            closed=True, xf=T(0.0, -0.1, -0.03) @ RX(0.2))
    return [p]


def hide_frame():
    """A fox's pelt stretched on a wooden frame, leaning on its nail: the
    wall's face at z 0, 0.4 x 0.75."""
    p = Part("static")
    # The frame: a long narrow board, tapered.
    p.prism([(-0.12, -0.38), (0.12, -0.38), (0.06, 0.38), (-0.06, 0.38)], -0.02, -0.005,
            "Timber", axis="z")
    # The pelt over it: a flat tapered shape, the tail hanging.
    p.prism([(-0.15, -0.3), (0.15, -0.3), (0.08, 0.3), (0.0, 0.36), (-0.08, 0.3)], -0.03,
            -0.02, "Leather", axis="z")
    p.prism([(-0.03, -0.3), (0.03, -0.3), (0.02, -0.62), (0.0, -0.66), (-0.02, -0.62)],
            -0.028, -0.02, "Red", axis="z")
    rod(p, (0, 0.37, 0.0), (0, 0.37, -0.025), 0.003, "Iron", segs=5)
    return [p]


# --- An old couple ----------------------------------------------------------------------------

def two_cups():
    """Two enamel cups on a cloth, one with its spoon, set for two: 0.3 x
    0.09 x 0.2."""
    p = Part("static")
    p.box((0.3, 0.003, 0.22), (0, 0.0015, 0), "Cream")
    for (x, z) in ((-0.07, 0.02), (0.07, -0.02)):
        p.lathe([(0.0, 0.0), (0.035, 0.0), (0.04, 0.08), (0.037, 0.08), (0.032, 0.004),
                 (0.0, 0.004)], "Enamel", segs=14, xf=T(x, 0.003, z) @ UP)
        p.lathe([(0.03, -0.004), (0.036, -0.004), (0.036, 0.004), (0.03, 0.004)], "Enamel",
                segs=10, closed=True, xf=T(x + 0.05, 0.045, z) @ RX(math.pi / 2))
    rod(p, (0.07, 0.07, -0.02), (0.1, 0.005, -0.07), 0.003, "Steel", segs=4)
    return [p]


def reading_glasses():
    """Wire spectacles folded on a book: 0.2 x 0.05 x 0.15."""
    p = Part("static")
    board(p, (0.15, 0.035, 0.21), (0.0, 0.0175, 0.0), "Olive", bevel=0.004)
    y = 0.038
    for sx in (-1, 1):
        p.lathe([(0.018, -0.0008), (0.021, -0.0008), (0.021, 0.0008), (0.018, 0.0008)], "Brass",
                segs=14, closed=True, xf=T(sx * 0.025, y, 0.02) @ RX(math.pi / 2))
        p.cyl(0.018, -0.0005, 0.0005, "Glass", segs=12,
              xf=T(sx * 0.025, y, 0.02) @ RX(math.pi / 2))
        rod(p, (sx * 0.045, y, 0.02), (sx * 0.04, y, -0.05), 0.0008, "Brass", segs=3)
    rod(p, (-0.006, y, 0.02), (0.006, y, 0.02), 0.0008, "Brass", segs=3)
    return [p]


def walking_stick():
    """A crook-handled walking stick leaning on the wall by the door: the
    wall's face at z 0, its foot 0.2 out on the floor 1.7 below."""
    p = Part("static")
    foot = (0.0, -1.7, -0.2)
    top = (0.0, -0.85, -0.03)
    rod(p, foot, top, 0.012, "Timber", segs=6)
    # The crook: a short arc over the top.
    n = 5
    for k in range(n):
        a0 = math.pi * k / n
        a1 = math.pi * (k + 1) / n
        q0 = (0.05 - 0.05 * math.cos(a0), top[1] + 0.05 * math.sin(a0), top[2])
        q1 = (0.05 - 0.05 * math.cos(a1), top[1] + 0.05 * math.sin(a1), top[2])
        rod(p, q0, q1, 0.012, "Timber", segs=6)
    p.cyl(0.013, 0.0, 0.025, "Black", segs=8, xf=T(*foot) @ RX(-math.atan2(0.17, 0.85)) @ UP)
    return [p]


# --- Seamstress -------------------------------------------------------------------------------

def sewing_machine():
    """A treadle sewing machine on its iron stand, under a window: 0.8 x
    0.95 x 0.45, its front -Z."""
    p = Part("static")
    p.box((0.8, 0.03, 0.45), (0, 0.74, 0), "Timber", bevel=0.005, segs=1)
    p.box((0.7, 0.2, 0.06), (0, 0.6, -0.2), "Timber")
    for sx in (-1, 1):
        p.prism([(-0.2, 0.0), (0.2, 0.0), (0.1, 0.72), (-0.1, 0.72)], sx * 0.36 - 0.015,
                sx * 0.36 + 0.015, "Iron", axis="x")
    p.box((0.7, 0.03, 0.03), (0, 0.12, 0.15), "Iron")
    p.box((0.4, 0.02, 0.25), (0, 0.08, -0.05), "Iron",
          xf=T(0, 0.08, -0.05) @ RX(0.25) @ T(0, -0.08, 0.05))
    p.lathe([(0.21, -0.012), (0.23, -0.012), (0.23, 0.012), (0.21, 0.012)], "Iron", segs=20,
            closed=True, xf=T(0.33, 0.35, 0.0) @ RY(math.pi / 2))
    # The machine head: the bed, the arm, the wheel.
    p.box((0.34, 0.04, 0.16), (0.0, 0.775, 0.02), "Black", bevel=0.01, segs=1)
    p.box((0.07, 0.16, 0.08), (0.14, 0.86, 0.02), "Black", bevel=0.015, segs=1)
    p.box((0.3, 0.06, 0.07), (0.0, 0.93, 0.02), "Black", bevel=0.02, segs=1)
    p.box((0.05, 0.12, 0.06), (-0.15, 0.88, 0.02), "Black", bevel=0.01, segs=1)
    p.cyl(0.06, -0.012, 0.012, "Steel", segs=14,
          xf=T(0.18, 0.9, 0.02) @ RY(math.pi / 2))
    p.box((0.12, 0.004, 0.08), (0.0, 0.94, 0.02), "Yellow")
    p.box((0.3, 0.004, 0.25), (-0.12, 0.7565, -0.05), "Ticking")
    return [p]


def dress_form():
    """A dressmaker's dummy on its tripod, a half-made dress pinned on it:
    0.4 wide, 1.5 tall."""
    p = Part("static")
    for k in range(3):
        a = k * 2 * math.pi / 3
        rod(p, (0, 0.5, 0), (0.2 * math.cos(a), 0.0, 0.2 * math.sin(a)), 0.012, "Timber",
            segs=5)
    rod(p, (0, 0.5, 0), (0, 0.95, 0), 0.015, "Timber", segs=6)
    p.lathe([(0.0, 0.95), (0.13, 0.96), (0.15, 1.05), (0.12, 1.18), (0.15, 1.33),
             (0.13, 1.42), (0.05, 1.47), (0.03, 1.52), (0.0, 1.52)], "Cream", segs=16,
            xf=S(1.0, 1.0, 0.7) @ UP)
    p.lathe([(0.0, 0.7), (0.21, 0.72), (0.16, 1.0), (0.13, 1.12), (0.16, 1.32),
             (0.14, 1.4), (0.0, 1.4)], "Blue", segs=16, xf=S(1.0, 1.0, 0.75) @ UP)
    return [p]


def cloth_pinned():
    """A length of cloth folded on a sill, a paper pattern pinned to it and a
    pincushion: 0.5 x 0.05 x 0.25."""
    p = Part("static")
    w = Wear(66)
    for k in range(4):
        board(p, (0.46, 0.008, 0.22), (0.0, 0.004 + k * 0.008, w.j(0.01)), "Blue",
              bevel=0.002, yaw=w.j(0.04))
    p.box((0.22, 0.002, 0.14), (-0.08, 0.034, 0.0), "Paper",
          xf=T(-0.08, 0.034, 0) @ RY(0.15) @ T(0.08, -0.034, 0))
    p.sphere(1.0, (0, 0, 0), "Red", segs=10, rings=6, xf=T(0.15, 0.05, 0.02) @ S(0.04, 0.025, 0.04))
    for k in range(5):
        a = k * 1.3
        rod(p, (0.15, 0.06, 0.02), (0.15 + 0.03 * math.cos(a), 0.085, 0.02 + 0.03 * math.sin(a)),
            0.0012, "Steel", segs=3)
    return [p]


# --- Radio ham --------------------------------------------------------------------------------

def radio_set():
    """A home-built wireless on a sill, its headphones on it and its aerial
    lead out of the window behind it: 0.42 x 0.25 x 0.22, its dials -Z."""
    p = Part("static")
    p.box((0.42, 0.22, 0.22), (0, 0.11, 0), "Timber", bevel=0.006, segs=1)
    p.box((0.38, 0.17, 0.004), (0, 0.12, -0.111), "Black")
    for (x, r) in ((-0.12, 0.022), (0.0, 0.03), (0.12, 0.022)):
        p.cyl(r, 0.0, 0.018, "Black", segs=12, xf=T(x, 0.1, -0.113) @ RX(math.pi))
        p.box((0.004, 0.012, 0.004), (x, 0.1 + r * 0.6, -0.133), "Cream")
    p.cyl(0.045, 0.0, 0.002, "Cream", segs=16, xf=T(0.0, 0.17, -0.114) @ RX(math.pi))
    # Headphones on top: two cups and the band.
    for sx in (-1, 1):
        p.cyl(0.035, 0.0, 0.02, "Black", segs=12, xf=T(sx * 0.07, 0.22, 0.02) @ UP)
    n = 5
    for k in range(n):
        a0, a1 = math.pi * k / n, math.pi * (k + 1) / n
        rod(p, (-0.07 * math.cos(a0), 0.24 + 0.05 * math.sin(a0), 0.05),
            (-0.07 * math.cos(a1), 0.24 + 0.05 * math.sin(a1), 0.05), 0.004, "Steel", segs=4)
    # The aerial lead out of its back to the window frame, sagging.
    rod(p, (0.18, 0.18, 0.11), (0.3, 0.25, 0.2), 0.002, "Rope", segs=3)
    rod(p, (0.3, 0.25, 0.2), (0.45, 0.4, 0.22), 0.002, "Rope", segs=3)
    return [p]


def logbook():
    """A station log open beside a Morse key: 0.36 x 0.04 x 0.24."""
    p = Part("static")
    for sx in (-1, 1):
        p.box((0.12, 0.012, 0.2), (sx * 0.062 - 0.06, 0.006, 0), "Leather", bevel=0.002,
              segs=1)
        p.box((0.11, 0.005, 0.19), (sx * 0.06 - 0.06, 0.0145, 0), "Paper")
        for k in range(9):
            p.box((0.09, 0.0006, 0.002), (sx * 0.06 - 0.06, 0.0172, -0.08 + k * 0.02), "Black")
    p.box((0.08, 0.012, 0.05), (0.13, 0.006, 0.02), "Black", bevel=0.003, segs=1)
    p.box((0.07, 0.006, 0.012), (0.13, 0.022, 0.02), "Brass",
          xf=T(0.13, 0.022, 0.02) @ RZ(0.08) @ T(-0.13, -0.022, -0.02))
    p.cyl(0.01, 0.0, 0.008, "Black", segs=8, xf=T(0.165, 0.026, 0.02) @ UP)
    return [p]


# --- Railwayman -------------------------------------------------------------------------------

def timetable():
    """The working timetable pinned on the wall, its corners curled: the
    wall's face at z 0, 0.36 x 0.5. (Its print stands a few millimetres
    proud: out of the paper's plane.)"""
    p = Part("static")
    # The print set into the card, its face 3.5 mm before the card's.
    p.box((0.36, 0.5, 0.004), (0, 0, -0.002), "Cream")
    t = 0.0055
    p.box((0.3, 0.035, t), (0, 0.205, -0.002 - t / 2), "Black")
    for k in range(14):
        y = 0.16 - k * 0.026
        for c in range(4):
            p.box((0.05, 0.004, t), (-0.12 + c * 0.08, y, -0.002 - t / 2), "Black")
    for sx in (-1, 1):
        p.box((0.05, 0.05, 0.001), (sx * 0.16, -0.225, -0.012), "Cream",
              xf=T(sx * 0.155, -0.22, -0.004) @ RX(-0.5) @ T(-sx * 0.155, 0.22, 0.004))
        p.sphere(0.006, (sx * 0.165, 0.235, -0.004), "Red", segs=6, rings=4)
    return [p]


def cap_hook():
    """A railwayman's peaked cap and a hand lamp on two pegs by the door: the
    wall's face at z 0."""
    p = Part("static")
    p.box((0.4, 0.08, 0.02), (0, 0, -0.01), "Timber", bevel=0.004, segs=1)
    for x in (-0.11, 0.11):
        rod(p, (x, 0.0, -0.02), (x, 0.03, -0.08), 0.008, "Timber", segs=6)
    # The cap: crown and peak, hung on the left peg.
    c = (-0.11, -0.03, -0.09)
    p.lathe([(0.0, 0.0), (0.1, 0.0), (0.11, 0.05), (0.0, 0.06)], "Black", segs=16,
            xf=T(*c) @ RX(1.2) @ UP)
    p.box((0.16, 0.006, 0.07), (c[0], c[1] - 0.06, c[2] - 0.03), "Black",
          xf=T(c[0], c[1] - 0.06, c[2] - 0.03) @ RX(0.9) @ T(-c[0], -c[1] + 0.06, -c[2] + 0.03))
    p.box((0.06, 0.012, 0.004), (c[0], c[1] - 0.02, c[2] - 0.045), "Brass")
    # The lamp on the right peg: a box body, its lens, the handle.
    q = (0.11, -0.13, -0.08)
    p.box((0.1, 0.14, 0.09), q, "Rusty", bevel=0.01, segs=1)
    p.cyl(0.035, 0.0, 0.01, "Glass", segs=12, xf=T(q[0], q[1], q[2] - 0.045) @ RX(math.pi))
    rod(p, (q[0] - 0.04, q[1] + 0.07, q[2]), (q[0], q[1] + 0.13, q[2]), 0.005, "Rusty", segs=4)
    rod(p, (q[0], q[1] + 0.13, q[2]), (q[0] + 0.04, q[1] + 0.07, q[2]), 0.005, "Rusty", segs=4)
    return [p]


for name, build in (("globe", globe), ("exercise_books", exercise_books),
                    ("slate_board", slate_board), ("cot", cot),
                    ("rocking_horse", rocking_horse), ("small_boots", small_boots),
                    ("traps_hook", traps_hook), ("hide_frame", hide_frame),
                    ("two_cups", two_cups), ("reading_glasses", reading_glasses),
                    ("walking_stick", walking_stick), ("sewing_machine", sewing_machine),
                    ("dress_form", dress_form), ("cloth_pinned", cloth_pinned),
                    ("radio_set", radio_set), ("logbook", logbook),
                    ("timetable", timetable), ("cap_hook", cap_hook)):
    prop(name, build)
