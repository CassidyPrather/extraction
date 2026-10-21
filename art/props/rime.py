"""Rime Fell, the fifth valley: Coldharbour, the mine company's settlement
on the bench above the trees, snowed in half the year. Board-and-batten
miners' houses of the fifties to the seventies, a stove in every room and
the winter's things round it. Visual only (no colliders), dressed in by
`world::clutter`. Origins as clutter.py's: on the surface under the
thing's centre, its front to -Z; things for a wall have theirs on the
wall's face (z 0), standing out towards -Z, their back (+Z) to it.

Indoors:
- `felt_boots`       a pair of grey felt valenki, knee-high, by the door
- `snow_shovel`      a broad board shovel, tin-edged, leant on the wall
                     (the origin on the floor at the wall's face)
- `skis_wall`        old wooden skis, leather bindings, bamboo poles,
                     leant upright on the wall (likewise)
- `winter_stores`    two flour sacks, a stack of tins, a crock
- `drying_line`      a cord across the corner by the stove, socks,
                     mittens and a scarf on it (the origin on the wall's
                     face at the cord's height, its pegs into the wall)
- `ice_axe`          an ice axe and a pair of strap crampons, for a table
- `firewood_basket`  a wicker basket of split logs, for beside the stove
- `stove_pipe_rack`  an iron rack on the wall over the stove's pipe (the
                     pipe rises between its rails, 0.15 out), gloves and a
                     cap drying on it (the origin on the wall's face at the
                     rails' height, about 1.7 m up)

Outdoors (baked with the sky):
- `sledge`           a hand sledge, iron-shod runners, a sack lashed on
- `snow_shovel_out`  `snow_shovel` for an outside wall

Materials as clutter.py's and yards.py's.
"""

import math
import os
import sys

from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import ALONG_X, UP, Wear, board, prop, rod  # noqa: E402,F401
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

# Small things: their occlusion is their own folds and the floor's.
_kit.AO_DISTANCE = 0.2
# Indoors, under a roof (the outdoor ones switch, see `outdoor`).
_kit.AO_SKY = "indoors"
# Hung on a wall, not standing on the floor: no floor in their bake.
_kit.AIRBORNE.update({"drying_line", "stove_pipe_rack"})


def outdoor(name, build):
    """Build a prop baked as yards.py's are: out in the weather."""
    dist, sky = _kit.AO_DISTANCE, _kit.AO_SKY
    _kit.AO_DISTANCE, _kit.AO_SKY = 0.35, True
    try:
        prop(name, build)
    finally:
        _kit.AO_DISTANCE, _kit.AO_SKY = dist, sky


def blob(p, size, at, mat, segs=10, rings=6, xf=None):
    """A soft lump: a squashed sphere."""
    x = T(*at) @ S(*size)
    if xf is not None:
        x = xf @ x
    p.sphere(1.0, (0, 0, 0), mat, segs=segs, rings=rings, xf=x)


def ring(p, r, t, at, mat, segs=16, xf=None, sweep=360.0, start=0.0, sides=6):
    """A round bar bent into a ring of radius `r` (to the bar's middle),
    `t` thick, in the XY plane about `at` (then `xf`); `sweep` degrees of
    it for an arc, `sides` round its bar."""
    prof = [(r + t * 0.5 * math.cos(a), t * 0.5 * math.sin(a))
            for a in (2 * math.pi * (k + 0.5) / sides for k in range(sides))]
    x = T(*at)
    if xf is not None:
        x = xf @ x
    p.lathe(prof, mat, segs=segs, closed=True, xf=x, sweep=sweep, start=start)


def along(a, b):
    """A frame at `a` whose Z runs towards `b`."""
    a, b = Vector(a), Vector(b)
    rot = Vector((0, 0, 1)).rotation_difference((b - a).normalized()).to_matrix().to_4x4()
    return T(*a) @ rot


def path(p, pts, r, mat, segs=4):
    """A cord through a list of points."""
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, r, mat, segs=segs)


# --- Felt boots -------------------------------------------------------------------------------

# The foot's stations heel to toe: (z, centre height, half width, half height).
FOOT = ((0.08, 0.07, 0.034, 0.064), (0.064, 0.076, 0.048, 0.074), (0.02, 0.072, 0.052, 0.07),
        (-0.04, 0.056, 0.051, 0.052), (-0.1, 0.046, 0.048, 0.042), (-0.15, 0.04, 0.042, 0.036),
        (-0.18, 0.035, 0.031, 0.03), (-0.196, 0.03, 0.012, 0.018))


def felt_boot(p, xf, w):
    # The foot in one piece of felt, its sole flattened 4 mm up, on a thin
    # leather sole stitched on under it.
    n = 10
    rings = []
    for (z, cy, rx, ry) in FOOT:
        rings.append([xf @ Vector((rx * math.cos(a), max(cy + ry * math.sin(a), 0.004), z))
                      for a in (2 * math.pi * k / n for k in range(n))])
    p.loft(rings, "Wool")
    outline = [(rx * 0.93, z) for (z, cy, rx, ry) in FOOT[1:-1]]
    outline = outline + [(-x, z) for (x, z) in reversed(outline)]
    p.prism(outline, 0.0, 0.008, "Leather", axis="y", xf=xf)
    # The shaft, a felt tube to the knee, a little slack and leaning.
    # It grows out of the foot: its foot narrower than the instep, buried.
    prof = [(0.036, 0.05), (0.046, 0.05), (0.06, 0.17), (0.06 + w.j(0.002), 0.31),
            (0.064, 0.42), (0.055, 0.42), (0.051, 0.31), (0.051, 0.17)]
    lean = T(0, 0.1, 0.03) @ RX(0.04 + w.j(0.03)) @ RZ(w.j(0.04)) @ T(0, -0.1, -0.03)
    p.lathe(prof, "Wool", segs=10, closed=True,
            xf=xf @ lean @ T(0, 0, 0.022) @ S(1.0, 1.0, 1.15) @ UP)


def felt_boots():
    """A pair of grey felt winter boots (valenki), knee-high, by the door:
    0.25 x 0.42 x 0.28, the toes to -Z."""
    p = Part("static")
    w = Wear(501)
    felt_boot(p, T(-0.065, 0, 0.0) @ RY(0.06), w)
    felt_boot(p, T(0.068, 0, 0.02) @ RY(-0.1), w)
    return [p]


# --- Snow shovel ------------------------------------------------------------------------------

SHOVEL_LEAN = 0.3
SHOVEL_FOOT = 0.348


def arc_slab(half, z0, z1, curve, n=8):
    """A plan-view profile (x, z) of a blade dished by `curve` at its edges,
    from z0 to z1 in front of its centre line."""
    xs = [-half + 2 * half * k / n for k in range(n + 1)]
    c = [curve * (x / half) ** 2 for x in xs]
    return [(x, cz + z0) for x, cz in zip(xs, c)] + \
        [(x, cz + z1) for x, cz in reversed(list(zip(xs, c)))]


def shovel():
    """The board shovel, built standing up (its blade's edge at y 0, the
    blade dished towards -Z), then leant back with its grip on the wall."""
    p = Part("static")
    X = T(0, 0.003, -SHOVEL_FOOT) @ RX(SHOVEL_LEAN)
    cv = -0.05
    # The blade of three boards (a single dished slab here, the joints as
    # two dark cracks), a tin strip folded over its edge.
    p.prism(arc_slab(0.23, -0.006, 0.006, cv), 0.006, 0.44, "Planks", axis="y", xf=X)
    for x in (-0.077, 0.077):
        cz = cv * (x / 0.23) ** 2
        p.box((0.004, 0.36, 0.004), (x, 0.24, cz - 0.0065), "Black", xf=X)
    p.prism(arc_slab(0.236, -0.01, 0.01, cv), 0.0, 0.05, "Zinc", axis="y", xf=X)
    # Low sides along its edges and the cleat across its head.
    for sx in (-1, 1):
        cz = cv * (0.222 / 0.23) ** 2
        p.box((0.012, 0.38, 0.04), (sx * 0.222, 0.25, cz - 0.022), "Timber", bevel=0.003,
              segs=1, xf=X)
    p.prism(arc_slab(0.23, -0.034, -0.004, cv), 0.4, 0.452, "Timber", axis="y", xf=X)
    # The handle along the blade's middle, strapped where it crosses the
    # cleat, a T-grip at its top.
    h0, h1 = (0.0, 0.1, -0.022), (0.0, 1.3, -0.06)
    rod(p, X @ Vector(h0), X @ Vector(h1), 0.016, "Timber", segs=8)
    for y in (0.2, 0.426):
        t = (y - h0[1]) / (h1[1] - h0[1])
        z = h0[2] + t * (h1[2] - h0[2])
        p.box((0.05, 0.03, 0.044), (0.0, y, z + 0.002), "Zinc", xf=X)
    rod(p, X @ Vector((-0.07, 1.315, -0.061)), X @ Vector((0.07, 1.315, -0.061)), 0.017,
        "Timber", segs=8)
    p.box((0.04, 0.035, 0.036), (0.0, 1.3, -0.061), "Timber", bevel=0.004, segs=1, xf=X)
    return [p]


def snow_shovel():
    """A broad wooden snow shovel leant on the wall by the door, 1.35 long:
    its blade's edge on the floor 0.34 out, its grip on the wall's face."""
    return shovel()


def snow_shovel_out():
    """`snow_shovel` for an outside wall: baked with the sky."""
    return shovel()


# --- Skis -------------------------------------------------------------------------------------

SKI_LEAN = 0.12
SKI_FOOT = 0.213
SKI_STRAIGHT = 1.66


def ski(p, x, w):
    X = T(x, 0.0015, -SKI_FOOT) @ RX(SKI_LEAN + w.j(0.01))
    # Its tail on the floor, its tip curled away from the wall at the top.
    R, n = 0.3, 6
    pts = [(0.0, 0.0), (0.0, 0.6), (0.0, 1.2), (0.0, SKI_STRAIGHT)]
    for k in range(1, n + 1):
        f = 0.95 * k / n
        pts.append((-R + R * math.cos(f), SKI_STRAIGHT + R * math.sin(f)))
    sec = [(-0.035, -0.011), (0.035, -0.011), (0.035, 0.011), (-0.035, 0.011)]
    p.sweep(pts, sec, "Boards", xf=X)
    # The binding: toe irons either side, a leather toe strap over them,
    # the heel strap hanging down the ski's face.
    s = 0.86
    for sx in (-1, 1):
        p.box((0.008, 0.07, 0.03), (sx * 0.037, s, -0.012), "Iron", xf=X)
    p.box((0.09, 0.05, 0.02), (0.0, s + 0.005, -0.02), "Leather", bevel=0.004, segs=1, xf=X)
    p.box((0.022, 0.24, 0.004), (0.0, s - 0.15, -0.0175), "Leather", xf=X)
    p.box((0.03, 0.02, 0.006), (0.0, s - 0.27, -0.019), "Iron", xf=X)
    return X


def pole(p, foot, top, w):
    rod(p, foot, top, 0.011, "Straw", segs=6)
    F = along(foot, top)
    L = (Vector(top) - Vector(foot)).length
    # The knuckles of the cane.
    for k in range(3):
        z = 0.3 + k * 0.33 + w.j(0.02)
        p.cyl(0.013, z, z + 0.008, "Straw", segs=6, xf=F)
    # The spike and its ferrule, the basket a hand above it.
    p.cyl(0.0145, -0.02, 0.05, "Iron", segs=6, xf=F)
    p.lathe([(0.0, -0.06), (0.007, -0.02), (0.0, -0.02)], "Iron", segs=5, xf=F)
    ring(p, 0.06, 0.008, (0, 0, 0.1), "Leather", segs=10, xf=F, sides=4)
    for k in range(4):
        a = k * math.pi / 2 + 0.4
        rod(p, F @ Vector((0.0, 0.0, 0.09)),
            F @ Vector((0.056 * math.cos(a), 0.056 * math.sin(a), 0.1)), 0.003, "Leather",
            segs=4)
    # The leather grip and its wrist loop.
    p.cyl(0.015, L - 0.14, L, "Leather", segs=8, xf=F)
    ring(p, 0.045, 0.006, (0.0, 0.0, 0.0), "Leather", segs=8, sides=4,
         xf=F @ T(0.0, 0.0, L - 0.115) @ RX(math.pi / 2))
    return F


def skis_wall():
    """A pair of old wooden skis with leather bindings and two bamboo poles,
    leant upright on the wall, 1.9 tall: their tails on the floor 0.21 out,
    their backs on the wall's face."""
    p = Part("static")
    w = Wear(503)
    for x in (-0.045, 0.045):
        ski(p, x, w)
    for (foot, top) in (((0.17, 0.0, -0.27), (0.145, 1.31, -0.016)),
                        ((0.28, 0.0, -0.24), (0.215, 1.32, -0.016))):
        pole(p, (foot[0], 0.06, foot[2] + 0.012), top, w)
    return [p]


# --- Winter stores ----------------------------------------------------------------------------

def sack(p, x, z, h, r, yaw, lean, mat):
    prof = [(0.0, 0.0), (r * 0.78, 0.0), (r * 0.97, 0.05), (r, h * 0.45), (r * 0.9, h * 0.8),
            (r * 0.55, h * 0.94), (r * 0.22, h), (0.0, h + 0.005)]
    # Leant on its neighbour about the near edge of its foot.
    xf = T(x, 0, z) @ RY(yaw) @ RZ(lean, about=(-math.copysign(r * 0.78, lean), 0, 0)) @ \
        S(1.0, 1.0, 0.72)
    p.lathe(prof, mat, segs=11, xf=xf @ UP)
    p.lathe([(0.0, 0.0), (0.035, 0.0), (0.026, 0.045), (0.048, 0.08), (0.0, 0.09)], mat,
            segs=8, xf=xf @ T(0, h - 0.005, 0) @ UP)
    ring(p, 0.031, 0.008, (0, 0, 0), "Rope", segs=8, sides=4,
         xf=xf @ T(0, h + 0.02, 0) @ RX(math.pi / 2))


def tin(p, x, y, z, r, h, mat, label, w):
    xf = T(x, y, z) @ RY(w.rng.random() * 6.0) @ UP
    p.cyl(r, 0.0, h, mat, segs=8, xf=xf)
    if label:
        p.cyl(r + 0.004, h * 0.2, h * 0.8, label, segs=8, xf=xf)


def winter_stores():
    """The winter's stores on the floor of the back room: two flour sacks
    slumped together, a stack of tins, a lidded crock with its stone:
    0.7 x 0.45, 0.55 tall."""
    p = Part("static")
    w = Wear(507)
    sack(p, -0.19, 0.07, 0.44, 0.15, 0.3, 0.05, "Canvas")
    sack(p, 0.07, 0.09, 0.4, 0.14, -0.5, -0.1, "Canvas")
    # The tins: three on the floor, one on them, a small one on top.
    r, h = 0.048, 0.11
    for k, (x, z) in enumerate(((0.21, -0.1), (0.307, -0.1), (0.258, -0.017))):
        tin(p, x, 0.0, z, r, h, "Zinc", ("Paper", "Olive", None)[k], w)
    tin(p, 0.258, h - 0.004, -0.072, r, h, "Rusty", "Cream", w)
    tin(p, 0.262, 2 * h - 0.008, -0.07, 0.04, 0.08, "Zinc", "Paper", w)
    # The crock of salted cabbage: stoneware, its shoulder glazed brown, a
    # board lid with a stone on it.
    c = (-0.07, 0.0, -0.13)
    p.lathe([(0.0, 0.0), (0.085, 0.0), (0.095, 0.02), (0.1, 0.12), (0.092, 0.2),
             (0.085, 0.21), (0.0, 0.21)], "Pale", segs=14, xf=T(*c) @ UP)
    p.lathe([(0.104, 0.135), (0.097, 0.2), (0.084, 0.214), (0.096, 0.13)], "Hide", segs=14,
            closed=True, xf=T(*c) @ UP)
    p.cyl(0.08, 0.205, 0.225, "Timber", segs=12, xf=T(*c) @ UP)
    blob(p, (0.05, 0.03, 0.04), (c[0] + 0.01, 0.245, c[2]), "Concrete", segs=7, rings=4)
    return [p]


# --- Drying line ------------------------------------------------------------------------------

LINE_HX = 0.6
LINE_Z = -0.07
LINE_SAG = 0.06


def cord_y(x):
    return -LINE_SAG * (1.0 - (x / LINE_HX) ** 2)


def on_cord(x, dz=0.0):
    """A frame at the cord's top at x, turned with its slope."""
    slope = 2 * LINE_SAG * x / LINE_HX ** 2
    return T(x, cord_y(x) + 0.003, LINE_Z + dz) @ RZ(math.atan(slope))


def clothes_peg(p, F, x=0.0):
    p.box((0.012, 0.06, 0.032), (x, -0.012, 0.0), "Timber", xf=F)


def sock(p, x, mat, heel_to, w):
    F = on_cord(x) @ RZ(w.j(0.05))
    leg = 0.24 + w.j(0.02)
    p.box((0.085, leg, 0.012), (0.0, -leg / 2, 0.0), mat, bevel=0.004, segs=1, xf=F)
    # The foot, turned at the heel.
    H = F @ T(0.0, -leg + 0.03, 0.0) @ RZ(heel_to * 1.25)
    p.box((0.08, 0.16, 0.02), (0.0, -0.08, 0.0), mat, bevel=0.004, segs=1, xf=H)
    p.box((0.09, 0.03, 0.02), (0.0, -0.025, 0.0), mat, xf=F)
    clothes_peg(p, F)


def mitten(p, x, mat, cuff, w, flip=1):
    F = on_cord(x) @ RZ(w.j(0.08))
    p.box((0.1, 0.07, 0.024), (0.0, -0.035, 0.0), cuff, xf=F)
    p.box((0.095, 0.17, 0.016), (0.0, -0.15, 0.0), mat, bevel=0.007, segs=1, xf=F)
    T_ = F @ T(flip * 0.045, -0.12, 0.0) @ RZ(flip * 0.5)
    p.box((0.032, 0.075, 0.024), (0.0, -0.035, 0.0), mat, xf=T_)
    clothes_peg(p, F)


def drying_line():
    """A cord across the corner by the stove, two pegs into the wall at its
    ends, wool socks, a pair of mittens and a scarf hung to dry: 1.2 along
    X, the cord 0.07 out from the wall, things hanging to 0.45 below it."""
    p = Part("static")
    w = Wear(509)
    for sx in (-1, 1):
        rod(p, (sx * LINE_HX, 0.004, 0.015), (sx * LINE_HX, 0.012, -0.09), 0.009, "Timber",
            segs=6)
        rod(p, (sx * LINE_HX, 0.016, -0.088), (sx * LINE_HX, 0.03, -0.095), 0.012, "Timber",
            segs=6)
    n = 8
    xs = [-LINE_HX + 2 * LINE_HX * k / n for k in range(n + 1)]
    path(p, [(x, cord_y(x), LINE_Z) for x in xs], 0.003, "Rope")
    sock(p, -0.45, "Cream", 1, w)
    sock(p, -0.32, "Cream", 1, w)
    mitten(p, -0.15, "DarkWool", "Ticking", w, flip=-1)
    mitten(p, 0.0, "DarkWool", "Ticking", w, flip=1)
    # The scarf over the cord, its two ends hanging, fringed.
    xs0 = 0.3
    F = on_cord(xs0)
    for dz, L in ((-0.009, 0.38), (0.009, 0.33)):
        p.box((0.15, L, 0.006), (0.0, -L / 2 - 0.006, dz), "Olive", xf=F)
        for k in range(5):
            p.box((0.012, 0.03, 0.004), (-0.06 + k * 0.03, -L - 0.02, dz), "Olive", xf=F)
    p.box((0.15, 0.014, 0.024), (0.0, -0.002, 0.0), "Olive", bevel=0.005, segs=1, xf=F)
    return [p]


# --- Ice axe ----------------------------------------------------------------------------------

def crampon(p, cx, cz, yaw, w):
    F = T(cx, 0.0, cz) @ RY(yaw)
    L, half, y0 = 0.27, 0.045, 0.03
    # The frame: two side bars, joined at toe and heel, ten points down.
    for sz in (-1, 1):
        p.box((L, 0.014, 0.006), (0.0, y0 + 0.007, sz * half), "Iron", xf=F)
    for x in (-L / 2 + 0.006, L / 2 - 0.006):
        p.box((0.008, 0.01, 2 * half + 0.006), (x, y0 + 0.012, 0.0), "Iron", xf=F)
    for sz in (-1, 1):
        for k in range(4):
            x = -L / 2 + 0.03 + k * 0.07
            p.lathe([(0.0, -y0), (0.005, 0.0), (0.0, 0.0)], "Iron", segs=3,
                    xf=F @ T(x, y0 + 0.002, sz * half) @ UP)
        # The two front points, raked forward.
        p.lathe([(0.0, -0.032), (0.005, 0.0), (0.0, 0.0)], "Iron", segs=3,
                xf=F @ T(L / 2 - 0.01, y0 + 0.008, sz * (half - 0.012)) @ RZ(-0.6) @ UP)
    # The straps: one through each side's rings, loose on the table, a
    # buckle on the long one.
    for sz, (dx, dz) in ((-1, (0.05, -0.11)), (1, (-0.04, 0.1))):
        a = F @ Vector((0.02, y0 + 0.004, sz * (half + 0.003)))
        b = F @ Vector((0.02 + dx * 0.4, 0.005, sz * (half + 0.035)))
        c = F @ Vector((0.02 + dx, 0.0025, sz * (half + 0.035) + dz))
        for (u, v) in ((a, b), (b, c)):
            d = v - u
            ang = math.atan2(-d.z, d.x)
            pitch = math.atan2(d.y, math.hypot(d.x, d.z))
            p.box((d.length + 0.004, 0.003, 0.018), (0, 0, 0), "Leather",
                  xf=T(*((u + v) / 2)) @ RY(ang) @ RZ(pitch))
    q = F @ Vector((0.07, 0.0, half + 0.035 + 0.06))
    p.box((0.024, 0.006, 0.026), (0, 0, 0), "Iron", xf=T(q.x, 0.004, q.z) @ RY(yaw - 0.7))


def ice_axe():
    """An ice axe lying on the table with a pair of strap crampons by it:
    0.78 x 0.05 x 0.5, the axe's head to +X."""
    p = Part("static")
    w = Wear(511)
    # The shaft rests on its spike and the adze: a little off the table.
    a, b = Vector((-0.33, 0.016, -0.1)), Vector((0.35, 0.026, -0.1))
    rod(p, a, b, 0.015, "Timber", segs=8)
    F = along(a, b)
    p.cyl(0.0155, -0.035, 0.0, "Iron", segs=8, xf=F)
    p.lathe([(0.0, -0.075), (0.008, -0.04), (0.0155, -0.035), (0.0, -0.035)], "Iron", segs=8,
            xf=F)
    hx, hy, hz = 0.365, 0.026, -0.1
    # The head, lying flat: the pick to -Z, the adze to +Z, its socket on
    # the shaft's end.
    prof = [(0.016, 0.03), (0.016, -0.03), (0.011, -0.08), (0.006, -0.13), (0.0, -0.165),
            (-0.006, -0.15), (-0.009, -0.1), (-0.012, -0.04), (-0.016, -0.03), (-0.016, 0.03),
            (-0.008, 0.045), (0.008, 0.045)]
    p.prism([(x + hx, z + hz) for x, z in prof], hy - 0.011, hy + 0.011, "Steel", axis="y")
    p.prism([(0.04, -0.011), (0.115, -0.023), (0.115, 0.023), (0.04, 0.011)], -0.006, 0.006,
            "Steel", axis="x", xf=T(hx, hy, hz))
    p.box((0.05, 0.03, 0.04), (hx - 0.018, hy, hz), "Iron", bevel=0.003, segs=1)
    # The sliding wrist loop, its sling flat on the table.
    ring(p, 0.019, 0.006, (0.0, 0.0, 0.38), "Iron", segs=8, xf=F)
    ring(p, 0.06, 0.012, (0.0, 0.0, 0.0), "Leather", segs=12, sides=4,
         xf=T(0.02, 0.0016, -0.19) @ S(1.4, 0.25, 1.0) @ UP)
    p.box((0.016, 0.004, 0.05), (0.05, 0.008, -0.135), "Leather",
          xf=T(0.05, 0.008, -0.135) @ RX(0.3) @ T(-0.05, -0.008, 0.135))
    crampon(p, -0.1, 0.07, 0.08, w)
    crampon(p, -0.06, 0.21, -0.12, w)
    return [p]


# --- Firewood basket --------------------------------------------------------------------------

def firewood_basket():
    """A wicker log basket for beside the stove, split birch and pine in it,
    handles either side: 0.5 across, 0.45 tall."""
    p = Part("static")
    w = Wear(513)
    top = 0.33
    prof = [(0.0, 0.0), (0.19, 0.0), (0.205, 0.02)]
    for k in range(4):
        y = 0.07 + k * 0.075
        r = 0.208 + 0.04 * y / top
        prof += [(r + 0.006, y - 0.015), (r, y + 0.015)]
    prof += [(0.262, top + 0.005), (0.258, top + 0.022), (0.244, top + 0.022),
             (0.238, top + 0.005), (0.2, 0.04), (0.0, 0.035)]
    p.lathe(prof, "Straw", segs=12, xf=UP)
    # The handles: bent cane loops over the rim either side.
    for sx in (-1, 1):
        ring(p, 0.065, 0.016, (0, 0, 0), "Straw", segs=6, sweep=180.0, sides=4,
             xf=T(sx * 0.25, top + 0.01, 0.0) @ RY(math.pi / 2))
    # The logs on end: round ones with their bark, split ones pale.
    logs = [(0.0, 0.0, 0.0, 0.05, 0.43)]
    for k in range(6):
        logs.append((0.13 + w.j(0.01), k * math.pi / 3 + 0.3 + w.j(0.15), 0.12, 0.047,
                     0.4 + w.j(0.04)))
    for k, (R, phi, tilt, r, h) in enumerate(logs):
        xf = T(R * math.cos(phi), 0.0, -R * math.sin(phi)) @ RY(phi) @ RZ(-tilt) @ RY(-phi)
        if k % 3 == 1:
            arc = [(r * math.cos(a), r * math.sin(a)) for a in
                   (math.pi * i / 5 for i in range(6))]
            p.prism(arc, 0.036, h, "Timber", axis="y", xf=xf @ RY(phi + w.j(1.0)))
        else:
            p.cyl(r + w.j(0.003), 0.036, h, "Boards" if k % 2 else "Pale", segs=6,
                  xf=xf @ UP)
            p.lathe([(0.0, h + 0.004), (r * 0.85, h + 0.004), (r * 0.85, h - 0.004)], "Timber",
                    segs=6, xf=xf @ UP)
    return [p]


# --- Sledge -----------------------------------------------------------------------------------

def sledge():
    """A wooden hand sledge, its runners iron-shod and curled up at the
    front, a sack lashed on its deck: 0.46 x 0.35 x 1.2, the front -Z."""
    p = Part("static")
    w = Wear(517)
    run = [(0.56, 0.045), (0.48, 0.033), (0.3, 0.031), (0.0, 0.031), (-0.3, 0.031),
           (-0.4, 0.034), (-0.48, 0.05), (-0.54, 0.09), (-0.575, 0.15), (-0.58, 0.21),
           (-0.565, 0.26), (-0.535, 0.29)]
    for sx in (-1, 1):
        X = T(sx * 0.2, 0, 0)
        p.sweep(run, [(-0.016, -0.025), (0.016, -0.025), (0.016, 0.025), (-0.016, 0.025)],
                "Timber", xf=X)
        p.sweep(run, [(-0.011, -0.031), (0.011, -0.031), (0.011, -0.023), (-0.011, -0.023)],
                "Iron", xf=X)
        for z in (0.42, 0.05, -0.3):
            board(p, (0.046, 0.09, 0.035), (sx * 0.2, 0.095, z), "Timber", wear=w, bevel=0.0)
    for z in (0.42, 0.05, -0.3):
        board(p, (0.47, 0.03, 0.05), (0.0, 0.145, z), "Timber", wear=w, bevel=0.004)
    for k in range(4):
        x = -0.165 + k * 0.11
        board(p, (0.085, 0.018, 0.86), (x + w.j(0.003), 0.168, 0.06), "Planks", wear=w,
              bevel=0.003)
    # The bow: a bar across the curls, the pull rope's ends on it.
    rod(p, (-0.215, 0.285, -0.54), (0.215, 0.285, -0.54), 0.015, "Timber", segs=6)
    for sx in (-1, 1):
        rod(p, (sx * 0.2, 0.15, -0.33), (sx * 0.2, 0.27, -0.53), 0.012, "Timber", segs=6)
    path(p, [(-0.12, 0.285, -0.555), (-0.07, 0.2, -0.6), (0.0, 0.17, -0.62),
             (0.07, 0.2, -0.6), (0.12, 0.285, -0.555)], 0.007, "Rope", segs=5)
    # The sack, and the rope over it twice, tied off to the runners.
    sy, sxr, szr, cy, cz = 0.09, 0.19, 0.3, 0.172 + 0.083, 0.1
    blob(p, (sxr, sy, szr), (0.0, cy, cz), "Canvas", segs=9, rings=6)
    blob(p, (0.06, 0.05, 0.05), (0.0, cy + 0.02, cz - szr + 0.01), "Canvas", segs=6, rings=4)
    for z in (cz - 0.13, cz + 0.14):
        f = math.sqrt(max(0.0, 1.0 - ((z - cz) / szr) ** 2))
        pts = [(-0.215, 0.12, z)]
        for k in range(7):
            th = math.pi * (1 - k / 6)
            pts.append((sxr * f * math.cos(th) * 1.02, cy + sy * f * math.sin(th) * 1.02 + 0.004,
                        z))
        pts.append((0.215, 0.12, z))
        path(p, pts, 0.006, "Rope", segs=3)
    return [p]


# --- Stove-pipe rack --------------------------------------------------------------------------

def glove(p, x, w):
    """A leather work glove folded over the front rail, its cuff hanging."""
    zr = -0.29
    for dz, L in ((-0.016, 0.17), (0.016, 0.12)):
        F = T(x, 0.0, zr + dz) @ RZ(w.j(0.06))
        p.box((0.09, L, 0.012), (0.0, -L / 2, 0.0), "Glove", bevel=0.005, segs=1, xf=F)
    p.box((0.09, 0.024, 0.052), (x, 0.012, zr), "Glove", bevel=0.008, segs=1)
    p.box((0.028, 0.07, 0.012), (0.0, -0.035, 0.0), "Glove",
          xf=T(x + 0.045, -0.1, zr - 0.024) @ RZ(0.45))


def stove_pipe_rack():
    """An iron drying rack on the wall over the stove, its pipe rising
    between the two rails (0.15 out): gloves over the front rail, a felt
    cap on top. The wall's face at z 0, the rails' height y 0, 0.6 wide,
    0.32 out."""
    p = Part("static")
    w = Wear(521)
    for sx in (-1, 1):
        x = sx * 0.28
        # The bracket: a plate screwed to the wall, an arm, a brace.
        p.box((0.03, 0.28, 0.006), (x, -0.12, -0.003), "Iron")
        p.box((0.012, 0.012, 0.32), (x, -0.018, -0.165), "Iron")
        rod(p, (x, -0.25, -0.008), (x, -0.024, -0.25), 0.006, "Iron", segs=5)
        for y in (-0.22, -0.03):
            p.cyl(0.007, -0.009, -0.006, "Cast", segs=6, xf=T(x, y, 0.0))
    for z in (-0.05, -0.29):
        rod(p, (-0.31, 0.0, z), (0.31, 0.0, z), 0.012, "Iron", segs=6)
    glove(p, -0.205, w)
    glove(p, -0.11, w)
    # The cap, crown up across both rails: felt, a band of fur.
    c = (0.185, 0.006, -0.17)
    xf = T(*c) @ RY(0.3) @ S(1.0, 1.0, 1.12) @ UP
    p.lathe([(0.0, 0.0), (0.112, 0.0), (0.118, 0.03), (0.11, 0.075), (0.085, 0.11),
             (0.045, 0.128), (0.0, 0.132)], "DarkWool", segs=12, xf=xf)
    p.lathe([(0.112, 0.0), (0.124, 0.002), (0.126, 0.05), (0.113, 0.052)], "Hide", segs=12,
            closed=True, xf=xf)
    return [p]


for name, build in (("felt_boots", felt_boots), ("snow_shovel", snow_shovel),
                    ("skis_wall", skis_wall), ("winter_stores", winter_stores),
                    ("drying_line", drying_line), ("ice_axe", ice_axe),
                    ("firewood_basket", firewood_basket),
                    ("stove_pipe_rack", stove_pipe_rack)):
    prop(name, build)

for name, build in (("sledge", sledge), ("snow_shovel_out", snow_shovel_out)):
    outdoor(name, build)
