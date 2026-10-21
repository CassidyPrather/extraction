"""Landmarks: tall things each valley can be told apart by from a
distance. Writes assets/models/props/<name>.glb, in prefab space (origin
on the ground under the centre, -Z the front), for the recipes of the same
name in src/world/buildings.rs, whose collider pieces they fill:

- `water_tower`   a brick shaft tapering from r 1.8 to 1.55 over 12 m on a
                  concrete ring, a steel gallery, a riveted tank (r 2.5,
                  12.5 to 16 m) under a cone roof; a ladder up the back,
                  a door at the foot
- `headframe`     a quarry's steel headframe, 18 m: four legs and the back
                  struts braced in lattice, a platform, two sheave wheels
- `signal_box`    a railway signal box: a brick lower floor, a glazed
                  timber cab above, a hipped roof, a stair up the side
- `steeple`       a chapel bell tower: a square brick base, an octagonal
                  belfry with arched openings, an onion dome and a cross

They are seen from far off: their silhouettes and big shapes matter,
small parts are kept few.

Materials: Brick, Concrete, Painted, Rusty, Iron, Timber, Planks, Glass,
Shingles, Corrugated, Zinc.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, board, prop, rod  # noqa: E402
from common import Part, RY, T, fillet  # noqa: E402

import _kit  # noqa: E402

# Big structures: their eaves and galleries shade further.
_kit.AO_DISTANCE = 1.5


def ring(p, r_in, r_out, y0, y1, mat, segs=32):
    """A flat annulus (a washer) from y0 to y1."""
    p.lathe([(r_in, y0), (r_out, y0), (r_out, y1), (r_in, y1)], mat, segs=segs, closed=True,
            xf=UP)


# --- Water tower ------------------------------------------------------------------------------

SHAFT = (1.8, 1.55, 0.4, 12.0)  # r bottom, r top, y bottom, y top
TANK = (2.5, 12.5, 16.0)


def shaft_r(y):
    r0, r1, y0, y1 = SHAFT
    return r0 + (r1 - r0) * (y - y0) / (y1 - y0)


def water_tower():
    p = Part("static")
    r0, r1, y0, y1 = SHAFT
    ring(p, 0.0, 2.05, -0.3, y0, "Concrete", segs=24)
    p.lathe([(0.0, y0 - 0.02), (r0, y0 - 0.02), (r1, y1), (0.0, y1)], "Brick", segs=24, xf=UP)
    # A corbelled ring of concrete under the gallery.
    p.lathe([(0.0, y1 - 0.02), (1.7, y1 - 0.02), (1.95, y1 + 0.25), (1.95, y1 + 0.4),
             (0.0, y1 + 0.4)], "Concrete", segs=24, xf=UP)
    # The gallery deck, its railing on posts.
    gy = y1 + 0.4
    ring(p, 1.8, 2.95, gy, gy + 0.1, "Painted", segs=32)
    top = gy + 0.1
    for k in range(16):
        a = 2 * math.pi * k / 16
        rod(p, (2.88 * math.cos(a), top, 2.88 * math.sin(a)),
            (2.88 * math.cos(a), top + 1.0, 2.88 * math.sin(a)), 0.025, "Painted", segs=5)
    for y in (top + 0.5, top + 1.0):
        ring(p, 2.86, 2.9, y - 0.02, y + 0.02, "Painted", segs=32)
    # The tank: riveted courses, a cone roof, a finial.
    tr, ty0, ty1 = TANK
    p.lathe([(0.0, top - 0.01), (tr, top - 0.01), (tr, ty1), (0.0, ty1)], "Painted", segs=32,
            xf=UP)
    for y in (13.4, 14.3, 15.2):
        ring(p, tr - 0.01, tr + 0.02, y - 0.03, y + 0.03, "Rusty", segs=32)
    p.lathe([(0.0, ty1 - 0.01), (tr + 0.12, ty1 - 0.01), (tr + 0.12, ty1 + 0.06),
             (0.25, ty1 + 1.35), (0.0, ty1 + 1.4)], "Rusty", segs=24, xf=UP)
    rod(p, (0.0, ty1 + 1.3, 0.0), (0.0, ty1 + 2.1, 0.0), 0.04, "Iron", segs=6)
    p.sphere(0.08, (0.0, ty1 + 2.1, 0.0), "Iron", segs=8, rings=5)
    # A door at the foot (front, -Z) in a concrete surround, and slit
    # windows up the shaft.
    z = -shaft_r(1.4)
    p.box((1.3, 2.5, 0.3), (0.0, y0 + 1.25, z + 0.05), "Concrete", bevel=0.02, segs=1)
    p.box((0.9, 2.0, 0.06), (0.0, y0 + 1.0, z - 0.13), "Painted", bevel=0.006, segs=1)
    for y in (4.0, 7.0, 10.0):
        zz = -shaft_r(y)
        p.box((0.22, 0.9, 0.2), (0.0, y, zz + 0.02), "Glass")
        p.box((0.42, 0.08, 0.3), (0.0, y - 0.49, zz + 0.0), "Concrete", bevel=0.01, segs=1)
    # A ladder up the back, from the ground to the gallery and on up the
    # tank to the roof, on brackets.
    lz = 1.95
    for x in (-0.2, 0.2):
        rod(p, (x, 0.0, lz), (x, gy, lz), 0.025, "Iron", segs=6)
        rod(p, (x, top, tr + 0.25), (x, ty1 + 0.4, tr + 0.25), 0.025, "Iron", segs=6)
    for k in range(int(gy / 0.35)):
        y = 0.3 + k * 0.35
        rod(p, (-0.2, y, lz), (0.2, y, lz), 0.014, "Iron", segs=5)
    for k in range(int((ty1 + 0.3 - top) / 0.35)):
        y = top + 0.3 + k * 0.35
        rod(p, (-0.2, y, tr + 0.25), (0.2, y, tr + 0.25), 0.014, "Iron", segs=5)
    for y in (2.0, 5.0, 8.0, 11.0):
        for x in (-0.2, 0.2):
            rod(p, (x, y, shaft_r(y) - 0.05), (x, y, lz), 0.018, "Iron", segs=5)
    for y in (13.0, 15.5):
        for x in (-0.2, 0.2):
            rod(p, (x, y, tr - 0.02), (x, y, tr + 0.25), 0.018, "Iron", segs=5)
    # The overflow pipe down the side.
    px = shaft_r(6.0) + 0.15
    rod(p, (px * 0.7, top + 0.37, -px * 0.7), (px * 0.7, 0.0, -px * 0.7), 0.07, "Rusty", segs=8)
    rod(p, (tr * 0.68, top + 0.3, -tr * 0.68), (px * 0.7, top + 0.3, -px * 0.7), 0.07, "Rusty",
        segs=8)
    return [p]


prop("water_tower", water_tower)

# --- Headframe --------------------------------------------------------------------------------

HF_LEG = 1.4      # legs at (+-1.4, y, +-1.4)
HF_TOP = 16.0
HF_STRUT = ((1.4, 15.0, 1.4), (2.3, 0.0, 7.0))  # back struts, mirrored in x


def beam(p, a, b, w, mat, d=None):
    """A square-section beam from a to b."""
    from mathutils import Vector
    a, b = Vector(a), Vector(b)
    v = b - a
    rot = Vector((0, 1, 0)).rotation_difference(v.normalized()).to_matrix().to_4x4()
    mid = (a + b) / 2
    p.box((w, v.length, d or w), (0.0, 0.0, 0.0), mat, xf=T(*mid) @ rot)


def headframe():
    p = Part("static")
    L = HF_LEG
    # A concrete collar round the shaft, its mouth dark.
    p.box((4.4, 0.4, 4.4), (0.0, 0.0, 0.0), "Concrete", bevel=0.03, segs=1)
    p.box((2.0, 0.006, 2.0), (0.0, 0.203, 0.0), "Char")
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.3, HF_TOP - 0.2, 0.3), (sx * L, 0.2 + (HF_TOP - 0.2) / 2, sz * L), "Rusty",
                  bevel=0.01, segs=1)
    # Girts every 3 m (the x faces' 0.3 m above the z faces', so no two
    # share a plane), a Warren diagonal in every panel.
    levels = [1.5 + 3.0 * k for k in range(5)]
    for k, y in enumerate(levels):
        for s_ in (-1, 1):
            p.box((2 * L, 0.22, 0.18), (0.0, y, s_ * L), "Rusty")
            p.box((0.18, 0.22, 2 * L), (s_ * L, y + 0.3, 0.0), "Rusty")
    for k, (ya, yb) in enumerate(zip(levels, levels[1:])):
        f = 1 if k % 2 == 0 else -1
        for s_ in (-1, 1):
            # Panels alternate 3 cm in and out, so where two diagonals meet
            # at a girt their faces don't share a plane.
            o = 0.02 * f
            beam(p, (-f * L, ya, s_ * L + o), (f * L, yb, s_ * L + o), 0.14, "Rusty", d=0.1)
            beam(p, (s_ * L + o, ya + 0.3, -f * L), (s_ * L + o, yb + 0.3, f * L), 0.1, "Rusty",
                 d=0.14)
    # Back struts to the ground behind, with ties to the legs.
    (ax, ay, az), (bx, by, bz) = HF_STRUT
    for sx in (-1, 1):
        beam(p, (sx * ax, ay, az), (sx * bx, by, bz), 0.32, "Rusty")
        for t in (0.35, 0.65):
            q = (sx * (ax + (bx - ax) * t), ay + (by - ay) * t, az + (bz - az) * t)
            beam(p, (sx * L, q[1], L), q, 0.12, "Rusty")
        p.box((0.9, 0.3, 0.9), (sx * bx, 0.0, bz), "Concrete", bevel=0.02, segs=1)
    # The head: a platform, a railing, the sheave wheels on their beams.
    p.box((3.6, 0.15, 3.6), (0.0, HF_TOP + 0.075, 0.0), "Painted", bevel=0.01, segs=1)
    for sx in (-1, 1):
        rod(p, (sx * 1.75, HF_TOP + 0.15, -1.75), (sx * 1.75, HF_TOP + 1.1, -1.75), 0.03, "Iron",
            segs=5)
        rod(p, (sx * 1.75, HF_TOP + 0.15, 1.75), (sx * 1.75, HF_TOP + 1.1, 1.75), 0.03, "Iron",
            segs=5)
    for z in (-1.75, 1.75):
        rod(p, (-1.75, HF_TOP + 1.1, z), (1.75, HF_TOP + 1.1, z), 0.025, "Iron", segs=5)
    for x in (-1.75, 1.75):
        rod(p, (x, HF_TOP + 1.05, -1.75), (x, HF_TOP + 1.05, 1.75), 0.025, "Iron", segs=5)
    for x in (-1.0, 1.0):
        p.box((0.3, 0.5, 3.4), (x, HF_TOP + 0.4, 0.0), "Rusty")
    for x in (-0.55, 0.55):
        hub = (x, HF_TOP + 1.9, 0.0)
        p.lathe([(1.25, -0.08), (1.4, -0.08), (1.4, 0.08), (1.25, 0.08)], "Iron", segs=32,
                closed=True, xf=T(*hub) @ RY(math.pi / 2))
        p.cyl(0.18, -0.12, 0.12, "Iron", segs=10, xf=T(*hub) @ RY(math.pi / 2))
        for k in range(6):
            a = math.pi * k / 3
            # From inside the hub (not its centre: there the spokes would
            # overlap) to inside the rim.
            rod(p, (x, hub[1] + 0.15 * math.sin(a), 0.15 * math.cos(a)),
                (x, hub[1] + 1.3 * math.sin(a), 1.3 * math.cos(a)), 0.04, "Iron", segs=5)
    # Axle trestles under the wheels.
    for z in (-0.5, 0.5):
        p.box((1.6, 1.5, 0.2), (0.0, HF_TOP + 1.1, z), "Rusty")
    rod(p, (-0.9, HF_TOP + 1.9, 0.0), (0.9, HF_TOP + 1.9, 0.0), 0.07, "Steel", segs=8)
    return [p]


prop("headframe", headframe)

# --- Signal box -------------------------------------------------------------------------------
#
# The recipe (`signal_box` in src/world/buildings.rs) builds the brick
# ground floor as a building, with its door, windows and steps; this is
# what stands on it: the glazed cab on the floor slab, its roof and
# chimney, the outside stair to a landing and the cab's door.

SB = (6.0, 3.6)        # the walls' centre lines, x and z
SB_FLOOR = 3.15        # the cab floor (the slab's top)
SB_EAVE = 5.4


def signal_box():
    p = Part("static")
    W, D = SB
    hw, hd = W / 2, D / 2
    # The station's name board on the brick front, over the door.
    p.box((1.6, 0.3, 0.04), (0.5, 2.62, -hd - 0.15 - 0.02), "Cream")
    # Panelled timber walls with a band of glass, a sill round it.
    y0 = SB_FLOOR
    cw, cd = W, D
    p.box((cw, 0.75, cd), (0.0, y0 + 0.375, 0.0), "Planks")
    p.box((cw - 0.04, 1.16, cd - 0.04), (0.0, y0 + 1.35, 0.0), "Glass")
    p.box((cw, SB_EAVE - (y0 + 1.95), cd), (0.0, (y0 + 1.95 + SB_EAVE) / 2, 0.0), "Planks")
    p.box((cw + 0.08, 0.05, cd + 0.08), (0.0, y0 + 0.775, 0.0), "Timber")
    # Glazing bars round the band: posts proud of the glass.
    for k in range(9):
        x = -cw / 2 + cw * k / 8
        for sz in (-1, 1):
            p.box((0.07, 1.1, 0.05), (x, y0 + 1.35, sz * (cd / 2 + 0.01)), "Timber")
    for k in range(1, 5):
        z = -cd / 2 + cd * k / 5
        for sx in (-1, 1):
            p.box((0.05, 1.1, 0.07), (sx * (cw / 2 + 0.01), y0 + 1.35, z), "Timber")
    # A pyramid roof of iron sheet over the cab, a chimney through it.
    r = (D + 0.8) / math.sqrt(2)
    from common import S
    p.lathe([(0.0, SB_EAVE), (r, SB_EAVE), (r, SB_EAVE + 0.08), (0.12, SB_EAVE + 1.4),
             (0.0, SB_EAVE + 1.4)], "Corrugated", segs=4, start=45.0,
            xf=S((W + 0.8) / (D + 0.8), 1.0, 1.0) @ UP)
    p.box((0.45, 1.9, 0.45), (1.6, SB_EAVE + 0.85, 0.6), "Brick")
    p.box((0.55, 0.08, 0.55), (1.6, SB_EAVE + 1.84, 0.6), "Concrete")
    # The stair up the +X end, past the brick (its face at hw + 0.15), to
    # a landing at the cab floor and the cab's door.
    sx0, sx1 = hw + 0.2, hw + 1.1
    z0, z1 = -hd + 0.1, 0.5
    top = SB_FLOOR - 0.15
    n = 16
    for k in range(n):
        y = top * (k + 1) / (n + 1)
        z = z0 + (z1 - z0) * (k + 0.5) / n
        board(p, (0.86, 0.04, 0.24), ((sx0 + sx1) / 2, y, z), "Planks", bevel=0.004)
    for x in (sx0 + 0.03, sx1 - 0.03):
        beam(p, (x, 0.0, z0 - 0.1), (x, top, z1 + 0.1), 0.05, "Timber", d=0.22)
    beam(p, (sx1 - 0.03, 1.0, z0 - 0.1), (sx1 - 0.03, top + 1.0, z1 + 0.1), 0.04, "Timber")
    p.box((0.9, 0.1, 1.2), ((sx0 + sx1) / 2, top - 0.01, z1 + 0.6), "Planks", bevel=0.005, segs=1)
    p.box((0.2, 0.1, 1.2), (hw + 0.1, top - 0.01, z1 + 0.6), "Timber")
    # The landing's posts, set into the ground as deep as the stair's
    # blockout ramp goes.
    for z in (z1 + 0.05, z1 + 1.15):
        rod(p, (sx1 - 0.03, -0.13, z), (sx1 - 0.03, top + 1.0, z), 0.04, "Timber", segs=4)
    rod(p, (sx1 - 0.03, top + 1.0, z1 + 0.05), (sx1 - 0.03, top + 1.0, z1 + 1.15), 0.03, "Timber",
        segs=4)
    p.box((0.05, 2.0, 0.85), (hw + 0.025, y0 + 1.0, z1 + 0.6), "Painted", bevel=0.005, segs=1)
    return [p]


def lever_frame():
    """The signal box's lever frame, 2.4 x 1.0 x 0.5 (origin on the floor
    under its centre, the signalman's side -Z): a cast frame, twelve levers
    in their quadrants, some pulled, number plates, a shelf of block
    instruments over it."""
    p = Part("static")
    p.box((2.4, 0.35, 0.45), (0.0, 0.175, 0.04), "Cast", bevel=0.01, segs=1)
    p.box((2.44, 0.06, 0.5), (0.0, 0.38, 0.04), "Cast", bevel=0.006, segs=1)
    for k in range(12):
        x = -1.1 + k * 0.2
        pulled = k in (2, 3, 7, 10)
        a = -0.35 if pulled else 0.3
        top = (x, 0.41 + 0.6 * math.cos(a), 0.04 + 0.6 * math.sin(a))
        rod(p, (x, 0.41, 0.04), top, 0.015, "Steel", segs=6)
        p.box((0.05, 0.1, 0.05), top, "Red" if k % 3 == 0 else "Cream", bevel=0.005, segs=1)
        p.box((0.12, 0.05, 0.004), (x, 0.3, -0.18), "Cream")
    # The block instruments on a shelf behind.
    p.box((2.4, 0.03, 0.25), (0.0, 1.3, 0.2), "Timber", bevel=0.004, segs=1)
    for x in (-0.8, 0.0, 0.8):
        p.box((0.3, 0.4, 0.2), (x, 1.515, 0.2), "Timber", bevel=0.01, segs=1)
        p.cyl(0.07, 0.096, 0.104, "Cream", segs=12, xf=T(x, 1.55, 0.0) @ RY(math.pi))
    for x in (-1.15, 1.15):
        p.box((0.05, 1.32, 0.05), (x, 0.66, 0.29), "Timber")
    return [p]


prop("signal_box", signal_box)
prop("lever_frame", lever_frame)

# --- Steeple ----------------------------------------------------------------------------------

ST_BASE = (4.0, 8.0)      # square side, height
ST_BELFRY = (1.9, 8.1, 12.0)  # circumradius of the octagon, y0, y1
ST_DOME = (1.55, 13.5, 17.0)  # widest radius, y0, y1


def steeple():
    p = Part("static")
    side, bh = ST_BASE
    h = side / 2
    p.box((side + 0.2, 0.5, side + 0.2), (0.0, 0.25, 0.0), "Concrete", bevel=0.02, segs=1)
    p.box((side, bh - 0.45, side), (0.0, 0.45 + (bh - 0.45) / 2, 0.0), "Brick")
    # Whitewashed pilasters at the corners, a cornice over them.
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.5, bh - 0.6, 0.5), (sx * (h - 0.2), 0.5 + (bh - 0.6) / 2, sz * (h - 0.2)),
                  "Plaster")
    p.box((side + 0.35, 0.3, side + 0.35), (0.0, bh + 0.05, 0.0), "Concrete", bevel=0.02, segs=1)
    # Blind arches on three sides, the door on the front (-Z).
    for k in range(4):
        rot = RY(math.pi / 2 * k)
        if k == 0:
            p.box((1.2, 2.18, 0.08), (0.0, 1.71, -h - 0.02), "Painted", bevel=0.01, segs=1)
            p.cyl(0.6, -h - 0.05, -h + 0.01, "Painted", segs=12, xf=T(0.0, 2.8, 0.0))
            p.box((1.5, 0.12, 0.3), (0.0, 0.56, -h - 0.15), "Concrete", bevel=0.01, segs=1)
        else:
            p.box((0.7, 1.5, 0.04), (0.0, 4.8, -h - 0.01), "Char", xf=rot)
            p.cyl(0.35, -h - 0.025, -h + 0.005, "Char", segs=10, xf=rot @ T(0.0, 5.55, 0.0))
    # The belfry: an octagon, open arches on four faces, a cornice.
    r, y0, y1 = ST_BELFRY
    p.lathe([(0.0, y0 - 0.06), (r, y0 - 0.06), (r, y1), (0.0, y1)], "Plaster", segs=8,
            start=22.5, xf=UP)
    ap = r * math.cos(math.pi / 8)
    for k in range(4):
        rot = RY(math.pi / 2 * k)
        p.box((0.8, 1.7, 0.06), (0.0, 9.85, -ap - 0.01), "Char", xf=rot)
        p.cyl(0.4, -ap - 0.035, -ap + 0.015, "Char", segs=10, xf=rot @ T(0.0, 10.7, 0.0))
    p.lathe([(0.0, y1 - 0.02), (r + 0.15, y1 - 0.02), (r + 0.15, y1 + 0.25), (0.0, y1 + 0.25)],
            "Concrete", segs=8, start=22.5, xf=UP)
    # The drum and the onion dome, a slanted-bar cross on top.
    dr, d0, d1 = ST_DOME
    p.lathe([(0.0, y1 + 0.23), (1.2, y1 + 0.23), (1.2, d0), (0.0, d0)], "Plaster", segs=16,
            xf=UP)
    prof = [(0.0, d0 - 0.02), (1.25, d0 - 0.02), (dr, d0 + 0.7), (1.5, d0 + 1.35),
            (1.15, d0 + 2.0), (0.6, d0 + 2.6), (0.25, d0 + 3.05), (0.12, d1), (0.0, d1)]
    p.lathe(prof, "Painted", segs=16, xf=UP)
    rod(p, (0.0, d1 - 0.05, 0.0), (0.0, d1 + 2.0, 0.0), 0.05, "Rusty", segs=6)
    for y, w in ((d1 + 1.4, 0.8), (d1 + 1.8, 0.4)):
        rod(p, (-w / 2, y, 0.0), (w / 2, y, 0.0), 0.045, "Rusty", segs=6)
    a = 0.35
    rod(p, (-0.3 * math.cos(a), d1 + 0.7 - 0.3 * math.sin(a), 0.0),
        (0.3 * math.cos(a), d1 + 0.7 + 0.3 * math.sin(a), 0.0), 0.04, "Rusty", segs=6)
    return [p]


prop("steeple", steeple)


# --- Survey mast ------------------------------------------------------------------------------

def survey_mast():
    """The survey post's mast on the open platform of its tower (the
    ranger tower's legs and stair, no cabin): a red-and-white banded pole
    5.4 m up from the platform's boards, guyed to its corners, and a
    surveyor's beacon on top, an open pyramid of boards in red and white,
    1.4 across: the post's silhouette at 300 m, unlike the ranger tower's
    roofed cabin. Origin on the platform's boards under the pole."""
    p = Part("static")
    H = 5.4
    n = 9
    for k in range(n):
        y0, y1 = H * k / n, H * (k + 1) / n
        p.cyl(0.12, y0, y1, "Red" if k % 2 == 0 else "Cream", segs=12, xf=UP)
    p.cyl(0.18, 0.0, 0.06, "Iron", segs=12, xf=UP)
    # Guys to the platform's corners from two thirds up.
    for sx in (-1, 1):
        for sz in (-1, 1):
            rod(p, (0.0, H * 0.66, 0.0), (sx * 2.0, 0.05, sz * 2.0), 0.006, "Steel", segs=4)
    # The beacon: three legs of an open pyramid over the top, boards
    # across them in bands.
    top = H + 1.6
    base = H - 0.1
    legs = []
    for k in range(3):
        a = k * 2 * math.pi / 3
        legs.append((0.7 * math.cos(a), base, 0.7 * math.sin(a)))
    apex = (0.0, top, 0.0)
    for f in legs:
        rod(p, f, apex, 0.03, "Timber", segs=5)
        rod(p, f, (0.0, base + 0.2, 0.0), 0.02, "Timber", segs=4)
    for j, t in enumerate((0.15, 0.4, 0.65)):
        for k in range(3):
            a, b = legs[k], legs[(k + 1) % 3]
            pa = tuple(u + (v - u) * t for u, v in zip(a, apex))
            pb = tuple(u + (v - u) * t for u, v in zip(b, apex))
            mid = tuple((u + v) / 2 for u, v in zip(pa, pb))
            L = math.dist(pa, pb)
            ang = math.atan2(pb[2] - pa[2], pb[0] - pa[0])
            # Each board a little outside its legs, facing out.
            out = (mid[0] * 0.08, 0.0, mid[2] * 0.08)
            c = (mid[0] + out[0], mid[1], mid[2] + out[2])
            p.box((L * 0.95, 0.18, 0.025), c, "Red" if j % 2 == 0 else "Cream",
                  xf=T(*c) @ RY(-ang) @ T(*(-v for v in c)))
    p.sphere(0.06, apex, "Red", segs=10, rings=6)
    return [p]


prop("survey_mast", survey_mast)
