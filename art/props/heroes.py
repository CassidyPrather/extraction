"""Hero landmarks: one per valley, seen from far off and unmistakable in
silhouette, each climbable in part. Writes assets/models/props/<name>.glb
and its lighter <name>_lod1.glb (the middle distance) and <name>_lod2.glb
(the far stand-in), in prefab space (origin on the ground under the
landmark's anchor, -Z the front), for the recipes of the same name in
src/world/buildings.rs, whose collider pieces they fill:

- `mill_race`     Marrow Creek: a timber sluice tower, 16 m to its deck, a
                  header tank on it, the sluice gate's frame over the tank;
                  the race, a plank trough, runs down a trestle beside a
                  walkway to an overshot wheel in a stone pit. Walk up the
                  walkway to the deck round the tank.
- `spoil_heap`    Kettle Pass: a mine's tip, a long ridge of spoil 14 m
                  high, the incline up its end on a trestle and the tipping
                  gantry over its far end; the winding house at the
                  incline's foot, brick, with its drum. Walk up the
                  heap's flanks or the incline.
- `blast_furnace` Cinder Works: the furnace stack, 26 m, its uptakes and
                  bleeders to 33 m, three hot-blast stoves, the downcomer to
                  the dust catcher, the skip bridge climbing to the top
                  (walk up its walkway to the charging platform), the
                  cast house at its foot (walk in).

Level 0 is the full model; 1 drops the small parts (bolts, boards for
planks, railings' bars, most bracing); 2 is the silhouette in big shapes
for past `lod::FAR_BUILDING`.

Materials: Timber, Planks, Iron, Rusty, Painted, Brick, Concrete,
Corrugated, Char, Steel, Pebble.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, ALONG_X, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, T  # noqa: E402

import _kit  # noqa: E402

# Big structures: their decks and galleries shade further.
_kit.AO_DISTANCE = 2.0
# Outdoors with roofs over floors (the cast house, the ropeway's shed): rain
# reaches only what's open to the sky.
_kit.AO_SKY = True


def beam(p, a, b, w, mat, d=None, bevel=0.0, xf=None):
    """A square-section beam from a to b (then `xf`)."""
    from mathutils import Vector
    a, b = Vector(a), Vector(b)
    v = b - a
    if v.normalized().y < -0.9999:
        # Straight down: turn about X, as for every other direction in
        # the YZ plane (the shortest arc's axis is arbitrary here).
        rot = RX(math.pi)
    else:
        rot = Vector((0, 1, 0)).rotation_difference(v.normalized()).to_matrix().to_4x4()
    mid = (a + b) / 2
    m = T(*mid) @ rot
    p.box((w, v.length, d or w), (0.0, 0.0, 0.0), mat, bevel=bevel, segs=1,
          xf=m if xf is None else xf @ m)


def lods(name, build):
    """The three levels of a hero landmark."""
    prop(name, lambda: build(0))
    prop(f"{name}_lod1", lambda: build(1))
    prop(f"{name}_lod2", lambda: build(2))


# --- Mill race (Marrow Creek) ---------------------------------------------------------------
#
# The tower's centre is the origin; the race runs down +Z. Mirrored in the
# recipe (`mill_race`): MR_* there.

MR_DECK = 19.0            # the deck's underside; its top 0.3 above
MR_GATE = MR_DECK + 5.0   # the sluice gate frame's head
MR_ROOF = MR_DECK + 6.0   # the eaves of the roof over the tank
MR_LEG = (2.6, 2.1)       # leg centres +-x, +-z at the foot, at the deck
MR_TANK = (4.4, 2.4)      # side, height of the header tank on the deck
MR_WALK = (0.75, 1.0)     # the walkway's centre x, width
MR_WALK_TOP = (3.2, 40.0)  # z where the walkway meets the deck, the ground
MR_TROUGH = (-0.5, 1.2)   # the trough's centre x, width
MR_LEVEL = 3.6            # the trough runs level at this floor at its foot
MR_WHEEL = (2.4, 1.0, 43.0)  # radius, axle y, axle z


def mr_walk_y(z):
    """The walkway's top at z."""
    z0, z1 = MR_WALK_TOP
    top = MR_DECK + 0.3
    return top - (top - 0.3) * (z - z0) / (z1 - z0)


def mr_trough_y(z):
    """The trough's floor at z: 0.6 over the walkway, then level."""
    return max(mr_walk_y(max(z, MR_WALK_TOP[0])) + 0.6, MR_LEVEL)


def mill_race(lv):
    p = Part("static")
    w = Wear(11)
    top = MR_DECK + 0.3
    (lf, lt) = MR_LEG
    # Concrete pads under the legs.
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.9, 0.5, 0.9), (sx * lf, 0.0, sz * lf), "Concrete", bevel=0.03, segs=1)
    # The legs, battered in.
    def leg(sx, sz, y):
        f = y / MR_DECK
        return (sx * (lf + (lt - lf) * f), y, sz * (lf + (lt - lf) * f))
    for sx in (-1, 1):
        for sz in (-1, 1):
            beam(p, leg(sx, sz, 0.25), leg(sx, sz, MR_DECK), 0.36 if lv < 2 else 0.5, "Timber")
    if lv < 2:
        # Girts every 4 m on all four faces, the x faces' 0.25 m above the
        # z faces' so no two share a plane; an X in every panel.
        levels = [MR_DECK * k / 4 for k in (1, 2, 3)]
        for y in levels:
            for s in (-1, 1):
                a, b = leg(-1, s, y), leg(1, s, y)
                beam(p, (a[0] - 0.2, y, a[2] + s * 0.05), (b[0] + 0.2, y, b[2] + s * 0.05),
                     0.2, "Timber", d=0.16)
                a, b = leg(s, -1, y + 0.25), leg(s, 1, y + 0.25)
                beam(p, (a[0] + s * 0.05, y + 0.25, a[2] - 0.2),
                     (b[0] + s * 0.05, y + 0.25, b[2] + 0.2), 0.16, "Timber", d=0.2)
        panels = [0.4] + levels + [MR_DECK - 0.2]
        step = 1 if lv == 0 else 2
        for k, (ya, yb) in enumerate(zip(panels, panels[1:])):
            if k % step:
                continue
            for s in (-1, 1):
                for f in ((-1, 1), (1, -1)) if lv == 0 else ((-1, 1),):
                    o = 0.1 * s + 0.03 * f[0]
                    a, b = leg(f[0], s, ya), leg(f[1], s, yb)
                    beam(p, (a[0], ya, a[2] + o), (b[0], yb, b[2] + o), 0.14, "Timber", d=0.1)
                    a, b = leg(s, f[0], ya + 0.25), leg(s, f[1], yb)
                    beam(p, (a[0] + o, ya + 0.25, a[2]), (b[0] + o, yb, b[2]), 0.1, "Timber",
                         d=0.14)
    # The deck: joists under planks, a railing round it (open where the
    # walkway comes up, +Z +X).
    half = 3.2
    if lv == 0:
        for k in range(7):
            x = -half + 0.3 + k * (2 * half - 0.6) / 6
            p.box((0.16, 0.22, 2 * half), (x, MR_DECK + 0.03, 0.0), "Timber")
        n = 21
        for k in range(n):
            z = -half + (k + 0.5) * 2 * half / n
            # Long boards: no turn about Y, or their ends would cross.
            board(p, (2 * half, 0.07, 2 * half / n - 0.015), (0.0, top - 0.035, z), "Planks",
                  roll=w.j(0.004))
    else:
        p.box((2 * half, 0.3, 2 * half), (0.0, MR_DECK + 0.15, 0.0), "Planks")
    if lv < 2:
        wx = MR_WALK[0]
        gap = (wx - MR_WALK[1] / 2 - 0.05, wx + MR_WALK[1] / 2 + 0.05)
        posts = [(-half + 0.06, -half + 0.06), (half - 0.06, -half + 0.06),
                 (half - 0.06, half - 0.06), (-half + 0.06, half - 0.06)]
        for x, z in posts:
            rod(p, (x, top, z), (x, top + 1.05, z), 0.05, "Timber", segs=4)
        e = half - 0.06
        # The z rails run between the x rails' ends, not into them.
        f = e - 0.04
        rails = [((-e, -e), (e, -e)), ((e, -f), (e, f)), ((-e, -f), (-e, f)),
                 ((-e, e), (gap[0], e)), ((gap[1], e), (e, e))]
        for (ax, az), (bx, bz) in rails:
            for y in (top + 1.0,) if lv else (top + 0.5, top + 1.0):
                beam(p, (ax, y, az), (bx, y, bz), 0.07, "Timber")
        for x in gap:
            rod(p, (x, top, e), (x, top + 1.05, e), 0.05, "Timber", segs=4)
    # The header tank: staves in iron hoops, open at the top.
    side, th = MR_TANK
    hs = side / 2
    if lv == 0:
        n = 18
        for face in range(4):
            rot = RY(math.pi / 2 * face)
            # The odd faces' staves fit between the even faces'.
            inset = 0.1 if face % 2 else 0.0
            for k in range(n):
                u = -hs + inset + (k + 0.5) * (side - 2 * inset) / n
                board(p, ((side - 2 * inset) / n - 0.01, th, 0.1), (u, top + th / 2, -hs + 0.05), "Planks",
                      xf=rot)
        for y in (top + 0.4, top + 1.2, top + 2.0):
            for face in range(4):
                rot = RY(math.pi / 2 * face)
                p.box((side + 0.06, 0.06, 0.02), (0.0, y + 0.012 * (face % 2), -hs - 0.01),
                      "Rusty", xf=rot)
        p.box((side - 0.2, 0.05, side - 0.2), (0.0, top + 0.12, 0.0), "Planks")
    else:
        p.box((side, th, side), (0.0, top + th / 2, 0.0), "Planks")
    # The sluice gate on the tank's +Z face, over the trough: two posts to
    # 21 m, a head beam, the gate's rack and its hand wheel.
    tx, tw = MR_TROUGH
    gy = top + th
    for x in (tx - tw / 2 - 0.12, tx + tw / 2 + 0.12):
        p.box((0.24, MR_GATE - top, 0.24), (x, (top + MR_GATE) / 2, hs + 0.14), "Timber")
    p.box((tw + 0.6, 0.3, 0.34), (tx, MR_GATE, hs + 0.14), "Timber")
    if lv < 2:
        p.box((tw + 0.06, 1.4, 0.08), (tx, gy - 0.1, hs + 0.08), "Planks")
        rod(p, (tx, gy + 0.6, hs + 0.14), (tx, MR_GATE + 0.4, hs + 0.14), 0.04, "Iron", segs=6)
        hub = (tx, MR_GATE + 0.4, hs + 0.14)
        p.lathe([(0.42, -0.03), (0.48, -0.03), (0.48, 0.03), (0.42, 0.03)], "Iron", segs=16,
                closed=True, xf=T(*hub) @ UP)
        for k in range(4 if lv == 0 else 2):
            a = math.pi * k / 4 if lv == 0 else math.pi * k / 2
            rod(p, (tx + 0.42 * math.cos(a), MR_GATE + 0.4, hs + 0.14 + 0.42 * math.sin(a)),
                (tx - 0.42 * math.cos(a), MR_GATE + 0.4, hs + 0.14 - 0.42 * math.sin(a)), 0.02,
                "Iron",
                segs=4)
    # A hipped roof over the tank on four posts at the deck's corners, its
    # shingles past the deck: the tower's cap, seen over the trees.
    rp = half - 0.2
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.22, MR_ROOF - 0.05 - top, 0.22), (sx * rp, (top + MR_ROOF - 0.05) / 2,
                                                         sz * rp), "Timber")
    for sz in (-1, 1):
        p.box((2 * rp + 0.3, 0.24, 0.2), (0.0, MR_ROOF - 0.12, sz * rp), "Timber")
        p.box((0.2, 0.24, 2 * rp - 0.2), (sz * rp, MR_ROOF - 0.12, 0.0), "Timber")
    eave = half + 0.5
    p.lathe([(0.0, MR_ROOF), (eave * math.sqrt(2), MR_ROOF), (eave * math.sqrt(2), MR_ROOF + 0.1),
             (0.15, MR_ROOF + 2.4), (0.0, MR_ROOF + 2.4)], "Shingles", segs=4, start=45.0,
            xf=UP)
    if lv < 2:
        rod(p, (0.0, MR_ROOF + 2.3, 0.0), (0.0, MR_ROOF + 3.2, 0.0), 0.04, "Iron", segs=5)
    # The race: a plank trough on the bents, beside the walkway.
    z0, z1 = MR_WALK_TOP
    wz1 = MR_WHEEL[2] - 1.2 * MR_WHEEL[0] / 2.4

    def trough(za, zb, wall=0.08):
        # The floor between the walls; the level run's walls a little
        # thinner, so where the two runs meet their sides don't share a
        # plane.
        ya, yb = mr_trough_y(za), mr_trough_y(zb)
        for x, wd, h in ((tx - tw / 2 + 0.04, wall, 0.8), (tx + tw / 2 - 0.04, wall, 0.8),
                         (tx, tw - 0.02 - 2 * wall, 0.08)):
            yo = h / 2 - 0.04 if h > 0.1 else -0.04
            beam(p, (x, ya + yo, za), (x, yb + yo, zb), wd, "Planks", d=h)
    # Its kink where the slope levels off.
    zk = z0 + (top - (MR_LEVEL - 0.6)) * (z1 - z0) / (top - 0.3)
    trough(hs + 0.1, zk)
    trough(zk, wz1, wall=0.07)
    if lv == 0:
        # Battens round the trough every 1.2 m.
        z = hs + 0.8
        while z < wz1 - 0.2:
            y = mr_trough_y(z)
            p.box((tw + 0.12, 0.08, 0.1), (tx, y - 0.12, z), "Timber")
            for sx in (-1, 1):
                p.box((0.06, 0.8, 0.1), (tx + sx * (tw / 2 + 0.03), y + 0.32, z), "Timber")
            z += 1.2
    # The walkway: planks across stringers, a handrail on its open side.
    wx, ww = MR_WALK
    for x in (wx - ww / 2 + 0.12, wx + ww / 2 - 0.12):
        beam(p, (x, mr_walk_y(z0) - 0.19, z0), (x, mr_walk_y(z1) - 0.19, z1), 0.08, "Timber",
             d=0.22)
    if lv == 0:
        n = 110
        for k in range(n):
            z = z0 + (k + 0.5) * (z1 - z0) / n
            board(p, (ww, 0.05, (z1 - z0) / n * 1.05 - 0.04), (wx, mr_walk_y(z) - 0.025, z),
                  "Planks", wear=w, tilt=math.atan((top - 0.3) / (z1 - z0)))
    else:
        beam(p, (wx, mr_walk_y(z0) - 0.04, z0), (wx, mr_walk_y(z1) - 0.04, z1), ww, "Planks",
             d=0.08)
    if lv < 2:
        rx = wx + ww / 2 + 0.05
        z = z0 + 0.5
        while z < z1 - 0.3:
            rod(p, (rx, mr_walk_y(z) - 0.1, z), (rx, mr_walk_y(z) + 1.0, z), 0.04, "Timber",
                segs=4)
            z += 2.4
        beam(p, (rx, mr_walk_y(z0) + 1.0, z0), (rx, mr_walk_y(z1) + 1.0, z1), 0.06, "Timber")
    # The bents, every 4 m: two posts from below ground, a cap under the
    # walkway, a stub up to the trough; braced where tall.
    z = 6.0
    while z < wz1 - 1.0:
        wy = mr_walk_y(min(z, z1)) - 0.27
        ty = mr_trough_y(z) - 0.08
        xa, xb = tx - tw / 2 - 0.1, wx + ww / 2 + 0.05
        cap = min(wy, ty - 0.1)
        if lv < 2 or int(z) % 8 == 6:
            for x in (xa, xb):
                p.box((0.22, cap + 0.95, 0.22), (x, (cap - 1.05) / 2, z), "Timber")
            p.box((xb - xa + 0.4, 0.2, 0.24), ((xa + xb) / 2, cap + 0.1 - 0.2, z), "Timber")
            if ty - 0.1 > cap + 0.05:
                p.box((0.2, ty - cap, 0.2), (tx, (ty + cap) / 2, z), "Timber")
            if lv == 0 and cap > 3.5:
                beam(p, (xa, 0.3, z + 0.13), (xb, cap - 0.4, z + 0.13), 0.12, "Timber", d=0.06)
            if lv == 0 and cap > 7.0:
                beam(p, (xb, 0.3, z - 0.13), (xa, cap - 0.4, z - 0.13), 0.12, "Timber", d=0.06)
        z += 4.0
    # The wheel in its stone pit at the foot.
    wr, wy, wz = MR_WHEEL
    # The pit: a floor, four walls, dark water in its bottom.
    px, pz = tw / 2 + 0.6, wr + 0.5
    p.box((2 * px + 0.8, 0.3, 2 * pz + 0.8), (tx, -1.75, wz), "Concrete", bevel=0.02, segs=1)
    for sx in (-1, 1):
        p.box((0.4, 1.9, 2 * pz + 0.8), (tx + sx * (px + 0.2), -0.65, wz), "Concrete",
              bevel=0.02, segs=1)
    for sz in (-1, 1):
        p.box((2 * px, 1.9, 0.4), (tx, -0.65, wz + sz * (pz + 0.2)), "Concrete", bevel=0.02,
              segs=1)
    p.box((2 * px, 0.04, 2 * pz), (tx, -1.0, wz), "Char")
    hub = (tx, wy, wz)
    segs = 32 if lv == 0 else (16 if lv == 1 else 10)
    for x in (tx - tw / 2 + 0.05, tx + tw / 2 - 0.05):
        p.lathe([(wr - 0.18, -0.05), (wr, -0.05), (wr, 0.05), (wr - 0.18, 0.05)], "Timber",
                segs=segs, closed=True, xf=T(x, wy, wz) @ ALONG_X)
    if lv < 2:
        p.cyl(0.22, -tw / 2 - 0.3, tw / 2 + 0.3, "Iron", segs=10, xf=T(*hub) @ ALONG_X)
        # Spokes from the hub's flanges to the rims (not across: they'd
        # cross each other in the middle).
        ns = 16 if lv == 0 else 8
        for x in (tx - tw / 2 + 0.05, tx + tw / 2 - 0.05):
            p.cyl(0.5, x - 0.08, x + 0.08, "Timber", segs=12, xf=T(0.0, wy, wz) @ ALONG_X)
            for k in range(ns):
                a = 2 * math.pi * k / ns
                beam(p, (x, wy + 0.45 * math.sin(a), wz + 0.45 * math.cos(a)),
                     (x, wy + (wr - 0.15) * math.sin(a), wz + (wr - 0.15) * math.cos(a)), 0.07,
                     "Timber", d=0.1)
        if lv == 0:
            for k in range(24):
                a = 2 * math.pi * k / 24
                c = (tx, wy + (wr - 0.1) * math.sin(a), wz + (wr - 0.1) * math.cos(a))
                p.box((tw - 0.12, 0.04, 0.2), c, "Planks", xf=T(*c) @ RX(-a) @ T(*(-x for x in c)))
    # The axle's posts.
    for x in (tx - tw / 2 - 0.3, tx + tw / 2 + 0.3):
        p.box((0.24, wy + 0.4, 0.3), (x, (wy + 0.4) / 2 - 0.2, wz), "Timber")
    # The feeder: a launder off the tank's back, broken off over nothing
    # (the hill spring's line is long gone), on one bent.
    if lv < 2:
        fy = top + 1.6
        beam(p, (tx, fy, -hs - 0.05), (tx, fy - 0.2, -hs - 5.5), 0.9, "Planks", d=0.08)
        for x in (tx - 0.48, tx + 0.48):
            beam(p, (x, fy + 0.3, -hs - 0.05), (x, fy + 0.1, -hs - 4.6), 0.06, "Planks", d=0.6)
        if lv == 0:
            beam(p, (tx + 0.48, fy + 0.1, -hs - 4.6), (tx + 0.56, fy - 1.4, -hs - 5.2), 0.06,
                 "Planks", d=0.6)
        for x in (tx - 0.6, tx + 0.6):
            beam(p, (x, 0.0, -hs - 4.0), (x, fy - 0.12, -hs - 4.0), 0.22, "Timber")
        p.box((1.6, 0.2, 0.26), (tx, fy - 0.2, -hs - 4.0), "Timber")
        # The pump's pipe up the -X face.
        rod(p, (-lf - 0.4, 0.0, -0.8), (-lt - 0.35, top + 0.6, -0.8), 0.11, "Rusty", segs=8)
        rod(p, (-lt - 0.35, top + 0.6, -0.8), (-hs + 0.2, top + 0.6, -0.8), 0.11, "Rusty", segs=8)
    return [p]


lods("mill_race", mill_race)


# --- Aerial ropeway (Kettle Pass) -----------------------------------------------------------
#
# The upper terminal (`ropeway_station`, the hero: up its stair to the
# loading deck and the shed on it) and the line's pylons across the valley
# (`ropeway_pylon` standing, `ropeway_pylon_down` fallen), the ropes
# broken and hanging. The ropes leave the station towards -Z. Mirrored in
# the recipes (RW_* there).

RW_PIER = (4.0, 3.5)        # the piers at +-x, +-z
RW_DECK = 9.0               # the deck's underside; its top 0.3 above
RW_DECK_Z = (-6.5, 4.5)     # the deck's front and back edges
RW_SHED = (8.0, 4.0, 7.0)   # width, wall height, depth (its front at z -2.5)
RW_SHED_C = (-0.5, 1.0)     # the shed's centre x, z (a strip of deck on its +X side)
RW_TOWER = (1.6, 24.0, 7.0)  # the tension tower's half side, top, centre z (behind the deck)
RW_WHEEL = (2.6, -4.6)      # the bull wheel's radius, centre z (just over the deck)
RW_STAIR_X = 5.6            # the stair's middle; its flights 1.1 wide either side
PY_TOP = 20.0               # the pylon's crossarm


def lattice(p, corners, y0, y1, panel, mat, lv, leg=0.22, brace=0.09, cz=0.0, xf=None,
            skip=()):
    """A tapering square lattice tower about (0, cz): corner legs from
    half-side corners[0] at y0 to corners[1] at y1, a girt and a diagonal
    in every panel on each face; then `xf`. Panels in `skip` are missing
    their braces (buckled out of a fallen tower)."""
    h0, h1 = corners

    def beam_(p, a, b, w, mat):
        beam(p, a, b, w, mat, xf=xf)

    def at(sx, sz, y):
        h = h0 + (h1 - h0) * (y - y0) / (y1 - y0)
        return (sx * h, y, cz + sz * h)
    for sx in (-1, 1):
        for sz in (-1, 1):
            beam_(p, at(sx, sz, y0), at(sx, sz, y1), leg if lv < 2 else leg * 1.5, mat)
    if lv == 2:
        return
    n = max(1, round((y1 - y0) / panel))
    for k in range(1, n + 1):
        y = y0 + (y1 - y0) * k / n
        # The x faces' girts 0.12 above the z faces', so no two meet in a
        # plane.
        for s in (-1, 1):
            if k == n:
                break
            a, b = at(-1, s, y), at(1, s, y)
            beam_(p, (a[0], y, a[2] + s * 0.02), (b[0], y, b[2] + s * 0.02), brace, mat)
            a, b = at(s, -1, y - 0.12), at(s, 1, y - 0.12)
            beam_(p, (a[0] + s * 0.02, y - 0.12, a[2]), (b[0] + s * 0.02, y - 0.12, b[2]), brace,
                 mat)
        if lv == 1 and k % 2 or k in skip:
            continue
        ya = y0 + (y1 - y0) * (k - 1) / n
        f = 1 if k % 2 else -1
        for s in (-1, 1):
            o = s * 0.05
            a, b = at(-f, s, ya), at(f, s, y)
            beam_(p, (a[0], ya, a[2] + o), (b[0], y, b[2] + o), brace * 0.8, mat)
            a, b = at(s, -f, ya + 0.06), at(s, f, y - 0.06)
            beam_(p, (a[0] + o, ya + 0.06, a[2]), (b[0] + o, y - 0.06, b[2]), brace * 0.8, mat)


def rope(p, a, b, sag, r, lv, n=None):
    """A rope from a to b hanging `sag` below the chord at its middle."""
    n = n or (12 if lv == 0 else 6)
    pts = []
    for k in range(n + 1):
        t = k / n
        pts.append(tuple(a[i] + (b[i] - a[i]) * t for i in range(3)))
        pts[-1] = (pts[-1][0], pts[-1][1] - sag * 4 * t * (1 - t), pts[-1][2])
    for q0, q1 in zip(pts, pts[1:]):
        rod(p, q0, q1, r, "Iron", segs=5 if lv == 0 else 4)


def bucket(p, at, tilt=0.0, lv=0):
    """An ore bucket on its hanger: a tipped steel tub under a bow."""
    x, y, z = at
    xf = T(x, y, z) @ RX(tilt)
    p.lathe([(0.0, -1.6), (0.42, -1.6), (0.55, -1.0), (0.55, -0.95), (0.0, -0.95)], "Rusty",
            segs=10 if lv == 0 else 6, xf=xf @ UP)
    if lv == 0:
        for sx in (-1, 1):
            p.box((0.05, 1.0, 0.08), (sx * 0.56, -0.55, 0.0), "Iron", xf=xf)
        p.box((1.2, 0.08, 0.1), (0.0, -0.04, 0.0), "Iron", xf=xf)
        p.cyl(0.12, -0.05, 0.05, "Iron", segs=8, xf=xf @ T(0, 0.08, 0) @ ALONG_X)


def ropeway_station(lv):
    p = Part("static")
    w = Wear(23)
    top = RW_DECK + 0.3
    px, pz = RW_PIER
    zf, zb = RW_DECK_Z
    # Piers, the deck on its beams.
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.8, RW_DECK + 0.2, 0.8), (sx * px, (RW_DECK - 0.8) / 2, sz * pz), "Concrete",
                  bevel=0.03 if lv < 2 else 0.0, segs=1)
    if lv < 2:
        for sz in (-1, 1):
            p.box((2 * px + 0.9, 0.5, 0.4), (0.0, RW_DECK - 0.25, sz * pz), "Rusty")
        for x in (-px, -px / 3, px / 3, px):
            p.box((0.3, 0.45, zb - zf), (x, RW_DECK - 0.75, (zf + zb) / 2), "Rusty")
    if lv == 0:
        n = 36
        for k in range(n):
            z = zf + (k + 0.5) * (zb - zf) / n
            board(p, (2 * px + 1.0, 0.06, (zb - zf) / n - 0.012), (0.0, top - 0.03, z), "Planks",
                  roll=w.j(0.004))
        p.box((2 * px + 1.0, 0.2, zb - zf), (0.0, top - 0.16, (zf + zb) / 2), "Planks")
    else:
        p.box((2 * px + 1.0, 0.3, zb - zf), (0.0, RW_DECK + 0.15, (zf + zb) / 2), "Planks")
    # The shed on the deck: plank walls, its front open to the loading
    # floor, a door on the +X side to the stair, a corrugated gable roof.
    sw, sh, sd = RW_SHED
    sx0, sz0 = RW_SHED_C
    hw, hd = sw / 2, sd / 2
    if lv < 2:
        p.box((sw, sh, 0.15), (sx0, top + sh / 2, sz0 + hd - 0.075), "Planks")
        p.box((0.15, sh, sd - 0.15), (sx0 - hw + 0.075, top + sh / 2, sz0 - 0.075), "Planks")
        # The +X wall, round its door (z 1.0 to 2.2 from the shed's
        # centre, 2.1 high) off the deck's strip.
        d0, d1 = sz0 + 1.0, sz0 + 2.2
        p.box((0.15, sh, d0 - (sz0 - hd)), (sx0 + hw - 0.075, top + sh / 2, (sz0 - hd + d0) / 2),
              "Planks")
        p.box((0.15, sh, sz0 + hd - 0.15 - d1), (sx0 + hw - 0.075, top + sh / 2,
                                                  (d1 + sz0 + hd - 0.15) / 2), "Planks")
        p.box((0.15, sh - 2.1, d1 - d0), (sx0 + hw - 0.075, top + 2.1 + (sh - 2.1) / 2, (d0 + d1) / 2),
              "Planks")
        # A beam over the open front, on posts.
        p.box((sw - 0.3, 0.35, 0.3), (sx0, top + sh - 0.175, sz0 - hd + 0.15), "Timber")
        p.box((0.25, sh - 0.35, 0.25), (sx0, top + (sh - 0.35) / 2, sz0 - hd + 0.15), "Timber")
    else:
        p.box((sw, sh, sd), (sx0, top + sh / 2, sz0), "Planks")
    # The roof: two slopes, eaves past the walls.
    rise = 2.2
    slope = math.atan2(rise, hw + 0.5)
    length = math.hypot(hw + 0.5, rise)
    for sx in (-1, 1):
        c = (sx0 + sx * (hw + 0.5) / 2, top + sh + rise / 2, sz0 - 0.4)
        # One slope 1 cm shorter, so their ends at the ridge don't share
        # a plane.
        p.box((length + 0.05, 0.08, sd + 1.6 - 0.005 * (1 - sx)), (0.0, 0.0, 0.0), "Corrugated",
              xf=T(*c) @ RY(0 if sx > 0 else math.pi) @ RZ(-slope))
    if lv < 2:
        # Gable ends in planks.
        for zz in (sz0 + hd - 0.075, sz0 - hd + 0.15):
            p.prism([(-hw, top + sh), (hw, top + sh), (sx0, top + sh + rise - 0.1)],
                    zz - 0.07, zz + 0.07, "Planks", axis="z")
    # The bull wheel at the deck's front, flat, on a post; its guard.
    wr, wz = RW_WHEEL
    wy = top + 1.6
    segs = 32 if lv == 0 else (16 if lv == 1 else 10)
    p.lathe([(wr - 0.2, -0.12), (wr, -0.12), (wr, 0.12), (wr - 0.2, 0.12)], "Iron", segs=segs,
            closed=True, xf=T(0.0, wy, wz) @ UP)
    p.cyl(0.35, top - 0.01, wy + 0.3, "Iron", segs=10, xf=T(0, 0, wz) @ UP)
    if lv < 2:
        for k in range(6 if lv == 0 else 3):
            a = math.pi * k / (6 if lv == 0 else 3)
            rod(p, (wr * 0.93 * math.cos(a), wy, wz + wr * 0.93 * math.sin(a)),
                (-wr * 0.93 * math.cos(a), wy, wz - wr * 0.93 * math.sin(a)), 0.06, "Iron",
                segs=5)
    # The tension tower behind the deck, its counterweight hanging in it,
    # a sheave on top.
    th, ty, tz = RW_TOWER
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.6, 0.5, 0.6), (sx * th, 0.0, tz + sz * th), "Concrete")
    p.box((2 * th + 0.4, 0.3, 2 * th + 0.4), (0.0, ty - 0.15, tz), "Rusty")
    lattice(p, (th, th * 0.75), 0.25, ty - 0.3, 2.4, "Rusty", lv, leg=0.24, brace=0.1, cz=tz)
    p.lathe([(1.0, -0.1), (1.2, -0.1), (1.2, 0.1), (1.0, 0.1)], "Iron", segs=segs, closed=True,
            xf=T(0.0, ty + 1.0, tz) @ ALONG_X)
    if lv < 2:
        p.box((0.3, 1.2, 0.3), (0.0, ty + 0.5, tz - 0.6), "Rusty")
        p.box((0.3, 1.2, 0.3), (0.0, ty + 0.5, tz + 0.6), "Rusty")
        # The counterweight: a concrete block on its rope.
        p.box((1.6, 2.2, 1.6), (0.0, 15.0, tz), "Concrete", bevel=0.03, segs=1)
        rod(p, (0.0, 16.1, tz), (0.0, ty + 1.0, tz), 0.04, "Iron", segs=5)
    # The stair up the +X side: two flights, a landing, the handrails.
    # The stair up the +X side: a flight up towards the front, a landing,
    # a flight back up to the deck's +X strip beside the shed.
    xo, xi = RW_STAIR_X + 0.9, RW_STAIR_X - 0.4
    zl = zf + 2.2
    flights = (((xo, 0.0, zb + 0.5), (xo, top / 2, zl + 0.8)),
               ((xi, top / 2, zl + 0.8), (xi, top, zb - 1.0)))
    for (a, b) in flights:
        steps = 14 if lv == 0 else 0
        for k in range(steps):
            t = (k + 0.5) / steps
            y = a[1] + (b[1] - a[1]) * (k + 1) / (steps + 1)
            z = a[2] + (b[2] - a[2]) * t
            board(p, (1.1, 0.05, 0.3), (a[0], y, z), "Planks", wear=w)
        for dx in (-0.58, 0.58):
            beam(p, (a[0] + dx, a[1] - 0.15, a[2]), (a[0] + dx, b[1] - 0.15, b[2]), 0.06,
                 "Timber", d=0.28)
        if lv == 1:
            beam(p, (a[0], a[1] - 0.04, a[2]), (a[0], b[1] - 0.04, b[2]), 1.1, "Planks", d=0.08)
        if lv < 2:
            beam(p, (a[0] + 0.5, a[1] + 0.95, a[2]), (a[0] + 0.5, b[1] + 0.95, b[2]), 0.05,
                 "Timber")
    # The landing between them, on two posts; the top flight steps off
    # onto the deck.
    p.box((2.7, 0.15, 1.6), ((xo + xi) / 2, top / 2 - 0.075, zl), "Planks")
    for x in (xi - 0.5, xo + 0.5):
        p.box((0.2, top / 2 + 0.85, 0.2), (x, (top / 2 + 0.85) / 2 - 0.5, zl - 0.4), "Timber")
    # The ropes: three leaving the wheel down the line, broken; one hangs
    # to the ground, two lie slack across it. A bucket still on one.
    if lv < 2:
        for x, sag, end in ((-1.6, 0.4, (-1.8, 0.0, -30.0)), (1.6, 0.6, (2.4, 0.0, -34.0)),
                            (0.0, 1.5, (0.4, 0.0, -18.0))):
            rope(p, (x, wy, wz - wr * 0.9), end, sag, 0.035, lv)
        bucket(p, (-1.65, 7.4, -13.0), tilt=0.15, lv=lv)
    return [p]


lods("ropeway_station", ropeway_station)


PY_BASE = 2.0      # the pylon's half side at the foot
PY_HEAD = 0.6      # and under its crossarm
PY_ARM = 3.4       # the crossarm's half length (along x); saddles at +-2.6, +-1.2


def pylon_body(p, lv, xf=None, skip=(), sunk=0.0):
    """The pylon standing at the origin (then `xf`): footings, the lattice,
    the crossarm and its saddles (moved `sunk` along z: a fallen one's,
    pressed into the ground), a ladder up one leg."""
    def bx(size, c, mat, **kw):
        p.box(size, c, mat, xf=xf, **kw)
    lattice(p, (PY_BASE, PY_HEAD), 0.25, PY_TOP, 2.5, "Rusty", lv, leg=0.2, brace=0.08, xf=xf,
            skip=skip)
    bx((2 * PY_ARM, 0.4, 0.3), (0.0, PY_TOP + 0.2, sunk), "Rusty")
    bx((1.4, 0.3, 1.4), (0.0, PY_TOP - 0.15, 0.0), "Rusty")
    for x in (-2.6, -1.2, 1.2, 2.6):
        bx((0.3, 0.25, 1.6), (x, PY_TOP + 0.525, sunk), "Iron")
    if lv < 2:
        # Struts from the crossarm's ends down to the legs.
        for sx in (-1, 1):
            for sz in (-1, 1):
                beam(p, (sx * PY_ARM * 0.9, PY_TOP + 0.05, sunk + sz * 0.1),
                     (sx * PY_HEAD * 1.25, PY_TOP - 2.5, sz * PY_HEAD * 1.25), 0.08, "Rusty",
                     xf=xf)
    if lv == 0:
        # A ladder up the -Z +X leg's face.
        h = lambda y: PY_BASE + (PY_HEAD - PY_BASE) * y / PY_TOP  # noqa: E731
        for k in range(int(PY_TOP / 0.4)):
            y = 0.5 + k * 0.4
            beam(p, (h(y) - 0.45, y, -h(y) - 0.12), (h(y) - 0.05, y, -h(y) - 0.12), 0.025,
                 "Iron", xf=xf)
        for o in (0.05, 0.45):
            beam(p, (h(0.3) - o, 0.3, -h(0.3) - 0.12), (h(PY_TOP) - o, PY_TOP - 0.2,
                                                        -h(PY_TOP) - 0.12), 0.035, "Iron", xf=xf)


def ropeway_pylon(lv):
    p = Part("static")
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.9, 0.6, 0.9), (sx * PY_BASE, 0.0, sz * PY_BASE), "Concrete",
                  bevel=0.03 if lv < 2 else 0.0, segs=1)
    pylon_body(p, lv)
    # The ropes, broken off either side: hanging from the saddles to the
    # ground, one still taut a little way to a bucket caught on it.
    if lv < 2:
        for x, sz, reach, sag in ((-2.6, -1, 13.0, 1.2), (2.6, 1, 11.0, 0.8),
                                  (-1.2, 1, 16.0, 1.6), (1.2, -1, 9.0, 0.6)):
            rope(p, (x, PY_TOP + 0.65, sz * 0.7), (x * 1.1, 0.0, sz * reach), sag, 0.03, lv)
        bucket(p, (2.74, 8.5, 6.4), tilt=0.5, lv=lv)
    return [p]


def ropeway_pylon_down(lv):
    """A pylon broken off its footings and lying along -Z, twisted a
    little, its crossarm in the grass; the sheared stumps of its legs
    still on their footings."""
    p = Part("static")
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.9, 0.6, 0.9), (sx * PY_BASE, 0.0, sz * PY_BASE), "Concrete",
                  bevel=0.03 if lv < 2 else 0.0, segs=1)
            h = PY_BASE + (PY_HEAD - PY_BASE) * 1.6 / PY_TOP
            tilt = 0.12 if sz > 0 else 0.05
            beam(p, (sx * PY_BASE, 0.25, sz * PY_BASE),
                 (sx * h, 1.6 + 0.3 * sx, sz * h - tilt * 1.6), 0.2, "Rusty")
    # Lying: the tower's +Y along -Z, its foot 2.5 m out from the stumps,
    # on its face (turned so that face is flat on the ground) and rolled
    # a few degrees.
    taper = math.atan((PY_BASE - PY_HEAD) / PY_TOP)
    xf = T(0.4, PY_BASE - 0.15, -PY_BASE - 2.5) @ RZ(0.08) @ RX(-(math.pi / 2 + taper))
    pylon_body(p, lv, xf=xf, skip=(3, 5, 6), sunk=-0.3)
    if lv < 2:
        for x, end, sag in ((-2.6, (-5.0, 0.0, -30.0), 0.0), (2.6, (6.0, 0.0, -28.0), 0.0)):
            rope(p, (x + 0.4, 0.6, -PY_TOP - 2.6), end, sag, 0.03, lv, n=4)
        bucket(p, (-3.2, 1.0, -12.0), tilt=1.4, lv=lv)
    return [p]


lods("ropeway_pylon", ropeway_pylon)
lods("ropeway_pylon_down", ropeway_pylon_down)


# --- Blast furnace (Cinder Works) -----------------------------------------------------------
#
# The furnace's axis is the origin; the cast house is in front (-Z), the
# skip bridge climbs from behind (+Z) to the charging platform, the stoves
# stand in a row along +X, the dust catcher off -X. Mirrored in the recipe
# (BF_* there).

BF_PROFILE = [(3.4, 1.2), (3.4, 6.0), (4.2, 9.0), (4.2, 9.6), (3.0, 24.0), (1.4, 26.0)]
BF_DECK = 24.0              # the charging platform's top, 12 x 12 round the stack
BF_STOVES = (3.0, (12.0, 18.5, 25.0), 4.0, 20.0)  # radius, x's, z, height to the dome
BF_CHIMNEY = (1.4, 1.0, (18.5, 11.5), 38.0)        # r at the foot, at the top, (x, z), height
BF_CATCHER = (2.8, (-14.0, 5.0), 6.0, 14.0)        # radius, (x, z), y0, y1 of its drum
BF_SKIP = ((0.0, 36.0), (0.0, 5.5))                # the bridge's foot and head (x, z)
BF_WALK_X = 1.7                                     # the walkway beside the skip rails
BF_CAST = (16.0, (-4.0, -18.0), 9.0)               # the cast house: width, z span, eaves


def frustum(p, profile, mat, segs, xf=None):
    """A solid of revolution about Y through (r, y) points, capped."""
    prof = [(0.0, profile[0][1])] + list(profile) + [(0.0, profile[-1][1])]
    p.lathe(prof, mat, segs=segs, xf=(xf or T()) @ UP)


def pipe(p, a, b, r, mat, segs=12):
    rod(p, a, b, r, mat, segs=segs)


def blast_furnace(lv):
    p = Part("static")
    segs = 32 if lv == 0 else (14 if lv == 1 else 10)
    # The foundation, the hearth, bosh and stack, the top cone.
    p.box((14.0, 1.4, 14.0), (0.0, 0.5, 0.0), "Concrete", bevel=0.04 if lv < 2 else 0.0, segs=1)
    frustum(p, BF_PROFILE, "Rusty", segs)
    if lv < 2:
        # Hoops round the hearth and stack, the mantle ring on its columns.
        for y in (2.5, 3.8, 5.0) if lv == 0 else ():
            p.lathe([(3.38, y - 0.08), (3.48, y - 0.08), (3.48, y + 0.08), (3.38, y + 0.08)],
                    "Iron", segs=segs, closed=True, xf=UP)
        p.lathe([(4.15, 9.1), (5.2, 9.1), (5.2, 9.7), (4.15, 9.7)], "Cast", segs=segs,
                closed=True, xf=UP)
        for k in range(8):
            a = 2 * math.pi * (k + 0.5) / 8
            p.box((0.5, 7.9, 0.5), (4.75 * math.cos(a), 5.15, 4.75 * math.sin(a)), "Rusty")
        for y in (13.0, 17.0, 21.0) if lv == 0 else ():
            r = 4.2 + (3.0 - 4.2) * (y - 9.6) / (24.0 - 9.6)
            p.lathe([(r - 0.02, y - 0.1), (r + 0.1, y - 0.1), (r + 0.1, y + 0.1),
                     (r - 0.02, y + 0.1)], "Iron", segs=segs, closed=True, xf=UP)
    # The bustle pipe round the bosh and the tuyere stocks down to the
    # hearth.
    if lv < 2:
        p.lathe([(5.9 + 0.55 * math.cos(2 * math.pi * k / 8), 8.0 + 0.55 * math.sin(
            2 * math.pi * k / 8)) for k in range(8)], "Rusty", segs=segs, closed=True, xf=UP)
        for k in range(10 if lv == 0 else 5):
            a = 2 * math.pi * k / (10 if lv == 0 else 5)
            c, s_ = math.cos(a), math.sin(a)
            pipe(p, (5.9 * c, 7.5, 5.9 * s_), (5.2 * c, 5.0, 5.2 * s_), 0.18, "Rusty", segs=6)
            pipe(p, (5.25 * c, 5.0, 5.25 * s_), (3.35 * c, 4.6, 3.35 * s_), 0.14, "Iron", segs=6)
    # The charging platform round the top: a deck on brackets, a railing.
    dk = BF_DECK
    hole = 3.0
    for (sx, sz, w, d) in ((0, -1, 12.0, 6.0 - hole), (0, 1, 12.0, 6.0 - hole),
                           (-1, 0, 6.0 - hole, 2 * hole), (1, 0, 6.0 - hole, 2 * hole)):
        cx = sx * (hole + (6.0 - hole) / 2)
        cz = sz * (hole + (6.0 - hole) / 2)
        p.box((w, 0.3, d), (cx, dk - 0.15, cz), "Painted" if lv < 2 else "Rusty")
    if lv < 2:
        for k in range(8):
            a = 2 * math.pi * (k + 0.5) / 8
            beam(p, (3.6 * math.cos(a), 18.5, 3.6 * math.sin(a)),
                 (5.6 * math.cos(a), dk - 0.3, 5.6 * math.sin(a)), 0.25, "Rusty")
        e = 5.95
        posts = [(-e, -e), (e, -e), (e, e), (-e, e)]
        # Open at the +Z +X corner where the walkway arrives.
        rails = [((-e, -e), (e, -e)), ((-e, -e + 0.04), (-e, e - 0.04)), ((-e, e), (0.9, e)),
                 ((e, -e + 0.04), (e, 3.4))]
        for (ax, az), (bx, bz) in rails:
            for y in ((dk + 1.05,) if lv else (dk + 0.55, dk + 1.05)):
                beam(p, (ax, y, az), (bx, y, bz), 0.06, "Painted")
        for x, z in posts + [(0.9, e), (e, 3.4)]:
            rod(p, (x, dk, z), (x, dk + 1.1, z), 0.05, "Painted", segs=5)
    # The top house over the bell, the uptakes from the top cone up and
    # over to the downcomer's head, the bleeders above.
    p.box((3.4, 3.2, 3.4), (0.0, 27.4, 0.0), "Corrugated")
    p.box((3.8, 0.12, 3.8), (0.0, 29.06, 0.0), "Rusty")
    ups = [(2.0, 0.0), (-2.0, 0.0), (0.0, 2.0), (0.0, -2.0)] if lv < 2 else [(2.0, 0.0),
                                                                             (-2.0, 0.0)]
    for x, z in ups:
        pipe(p, (x * 0.9, 24.6, z * 0.9), (x * 1.15, 31.0, z * 1.15), 0.5, "Rusty",
             segs=12 if lv < 2 else 6)
    pipe(p, (-2.3, 31.0, -2.3), (2.3, 31.0, 2.3), 0.55, "Rusty", segs=12 if lv < 2 else 6)
    # A little higher than the other, so their sides don't meet in a plane.
    pipe(p, (2.3, 31.05, -2.3), (-2.3, 31.05, 2.3), 0.55, "Rusty", segs=12 if lv < 2 else 6)
    for x, z in ((-2.3, -2.3), (2.3, 2.3), (2.3, -2.3), (-2.3, 2.3)):
        pipe(p, (x, 31.0, z), (x, 33.2, z), 0.3, "Rusty", segs=10 if lv < 2 else 5)
        if lv < 2:
            p.cyl(0.45, 33.1, 33.5, "Iron", segs=10, xf=T(x, 0.0, z) @ UP)
    # The downcomer: from the uptakes' crossing, down to the dust
    # catcher's top.
    cr, (cx, cz), cy0, cy1 = BF_CATCHER
    pipe(p, (-2.3, 31.0, -2.3), (cx + 0.6, cy1 + 2.0, cz), 0.9, "Rusty",
         segs=14 if lv < 2 else 8)
    # The dust catcher: a drum on a cone on legs, a domed top.
    frustum(p, [(0.4, 2.2), (cr, cy0), (cr, cy1), (1.0, cy1 + 2.2)], "Rusty", segs,
            xf=T(cx, 0.0, cz))
    for k in range(4):
        a = math.pi / 4 + math.pi / 2 * k
        beam(p, (cx + (cr + 0.6) * math.cos(a), 0.0, cz + (cr + 0.6) * math.sin(a)),
             (cx + (cr - 0.2) * math.cos(a), cy0 + 0.5, cz + (cr - 0.2) * math.sin(a)), 0.4,
             "Rusty")
    # The stoves: tall domed drums in a row, the hot-blast main along their
    # feet to the bustle pipe; their chimney behind.
    sr, xs, sz, sh = BF_STOVES
    for x in xs:
        frustum(p, [(sr + 0.25, 0.0), (sr + 0.25, 0.8), (sr, 0.8), (sr, sh),
                    (sr * 0.8, sh + 1.5), (sr * 0.45, sh + 2.6), (0.4, sh + 3.0)], "Rusty",
                segs, xf=T(x, 0.0, sz))
        if lv < 2:
            for y in (5.0, 10.0, 15.0) if lv == 0 else ():
                p.lathe([(sr - 0.02, y - 0.1), (sr + 0.08, y - 0.1), (sr + 0.08, y + 0.1),
                         (sr - 0.02, y + 0.1)], "Iron", segs=segs, closed=True,
                        xf=T(x, 0.0, sz) @ UP)
            pipe(p, (x, 8.0, sz - sr + 0.2), (x, 8.0, sz - sr - 1.1), 0.5, "Rusty", segs=10)
    pipe(p, (5.9, 8.0, 2.4), (xs[-1] + 1.2, 8.0, sz - sr - 1.2), 0.62, "Rusty",
         segs=14 if lv < 2 else 8)
    # The stoves' chimney, brick, banded.
    r0, r1, (hx, hz), hh = BF_CHIMNEY
    frustum(p, [(r0 + 0.4, 0.0), (r0 + 0.4, 1.0), (r0, 1.0), (r1, hh), (0.0, hh)], "Brick", segs,
            xf=T(hx, 0.0, hz))
    if lv < 2:
        for y in (12.0, 24.0, hh - 0.6) if lv == 0 else (hh - 0.6,):
            r = r0 + (r1 - r0) * (y - 1.0) / (hh - 1.0)
            p.lathe([(r - 0.02, y - 0.15), (r + 0.1, y - 0.15), (r + 0.1, y + 0.15),
                     (r - 0.02, y + 0.15)], "Iron", segs=segs, closed=True,
                    xf=T(hx, 0.0, hz) @ UP)
        pipe(p, (xs[1], 3.0, sz + sr - 0.2), (hx, 3.0, hz - r0 + 0.1), 0.7, "Brick", segs=10)
    # The skip bridge: two lattice trusses from the stockhouse pit up to
    # the top, the skip rails on them, the walkway alongside.
    (fx, fz), (tx_, tz) = BF_SKIP
    ft = (fx, 0.3, fz)
    hd = (tx_, BF_DECK + 2.5, tz)
    import mathutils
    along = mathutils.Vector(hd) - mathutils.Vector(ft)
    n = 10 if lv == 0 else 5
    for sx in (-1, 1):
        x = fx + sx * 1.0
        lo = lambda t: (x, ft[1] + along.y * t - 1.8, ft[2] + along.z * t)  # noqa: E731
        hi = lambda t: (x, ft[1] + along.y * t, ft[2] + along.z * t)  # noqa: E731
        beam(p, lo(0.0), lo(1.0), 0.32, "Rusty")
        beam(p, hi(0.0), hi(1.0), 0.32, "Rusty")
        if lv < 2:
            for k in range(n + 1):
                t = k / n
                a, b = lo(t), hi(t)
                beam(p, (a[0], a[1] + 0.15, a[2]), (b[0], b[1] - 0.15, b[2]), 0.16, "Rusty")
                if k < n:
                    t2 = (k + 1) / n
                    a2 = lo(t2)
                    beam(p, (x - sx * 0.04, b[1] - 0.15, b[2]), (x - sx * 0.04, a2[1] + 0.15,
                                                                 a2[2]), 0.12, "Rusty")
    if lv < 2:
        for k in range(n + 1):
            t = k / n
            y, z = ft[1] + along.y * t - 1.8, ft[2] + along.z * t
            p.box((2.3, 0.14, 0.14), (fx, y - 0.02, z), "Rusty")
        for x in (fx - 0.45, fx + 0.45):
            beam(p, (x, ft[1] + 0.18, ft[2]), (x, hd[1] + 0.18, hd[2]), 0.1, "Iron")
        # The skip itself, part way up.
        t = 0.62
        p.box((1.1, 1.0, 1.6), (fx, ft[1] + along.y * t + 0.75, ft[2] + along.z * t), "Rusty",
              xf=None)
    # The walkway: a sloping grating beside the bridge from the ground to
    # the charging platform, a handrail.
    wx = BF_WALK_X
    w0 = (wx, 0.3, fz - 1.0)
    w1 = (wx, BF_DECK, 5.6)
    beam(p, (wx, w0[1] - 0.06, w0[2]), (wx, w1[1] - 0.06, w1[2]), 1.0, "Painted", d=0.12)
    if lv < 2:
        rx = wx + 0.47
        k = 0
        while k <= 12:
            t = k / 12
            y = w0[1] + (w1[1] - w0[1]) * t
            z = w0[2] + (w1[2] - w0[2]) * t
            rod(p, (rx, y, z), (rx, y + 1.05, z), 0.035, "Painted", segs=5)
            k += 1
        beam(p, (rx, w0[1] + 1.05, w0[2]), (rx, w1[1] + 1.05, w1[2]), 0.05, "Painted")
    # The stockhouse pit at the bridge's foot, and the hoist house.
    p.box((4.0, 0.6, 5.0), (fx, 0.0, fz + 1.5), "Concrete", bevel=0.03 if lv < 2 else 0.0,
          segs=1)
    hx_, hz_ = 7.0, 33.0
    p.box((6.0, 4.5, 6.0), (hx_, 2.25, hz_), "Brick")
    p.prism([(hx_ - 3.4, 4.5), (hx_ + 3.4, 4.5), (hx_, 6.4)], hz_ - 3.4, hz_ + 3.4, "Corrugated",
            axis="z")
    if lv < 2:
        pipe(p, (hx_ - 3.0, 3.6, hz_), (fx + 0.2, ft[1] + 3.0, fz - 3.0), 0.03, "Iron", segs=4)
    # The cast house in front: a steel frame, a gable roof, sheeting on the
    # upper half of its sides (walk in under it), the iron runner and a
    # ladle car at its far end.
    cw, (cz0, cz1), ce = BF_CAST
    hw = cw / 2
    p.box((cw + 0.6, 0.2, cz0 - cz1), (0.0, 0.1, (cz0 + cz1) / 2), "Concrete")
    nb = 5
    for k in range(nb):
        z = cz0 + (cz1 - cz0) * k / (nb - 1)
        for sx in (-1, 1):
            p.box((0.4, ce, 0.4), (sx * hw, 0.2 + ce / 2, z), "Rusty")
        if lv < 2:
            p.box((cw + 0.5, 0.45, 0.3), (0.0, ce - 0.0, z), "Rusty")
    rise = 3.0
    slope = math.atan2(rise, hw + 0.6)
    length = math.hypot(hw + 0.6, rise)
    for sx in (-1, 1):
        c = (sx * (hw + 0.6) / 2, ce + 0.2 + rise / 2, (cz0 + cz1) / 2)
        p.box((length + 0.05, 0.1, cz0 - cz1 + 1.0 - 0.01 * (1 - sx)), (0.0, 0.0, 0.0),
              "Corrugated", xf=T(*c) @ RY(0 if sx > 0 else math.pi) @ RZ(-slope))
    for sx in (-1, 1):
        p.box((0.1, ce - 4.0, cz0 - cz1), (sx * (hw + 0.25), 4.2 + (ce - 4.0) / 2,
                                           (cz0 + cz1) / 2), "Corrugated")
    p.prism([(-hw, ce + 0.2), (hw, ce + 0.2), (0.0, ce + 0.2 + rise - 0.1)], cz1 - 0.08,
            cz1 + 0.08, "Corrugated", axis="z")
    if lv < 2:
        p.box((0.9, 0.06, cz0 - cz1 - 2.0), (0.0, 0.23, (cz0 + cz1) / 2 - 0.5), "Char")
        for x in (-0.6, 0.6):
            p.box((0.15, 0.4, cz0 - cz1 - 2.0), (x, 0.4, (cz0 + cz1) / 2 - 0.5), "Concrete")
        # The ladle car on its rails.
        lz = cz1 + 2.2
        for x in (-0.75, 0.75):
            p.box((0.12, 0.1, 6.0), (x, 0.25, lz), "Steel")
        p.box((2.4, 0.8, 3.2), (0.0, 0.9, lz), "Rusty")
        p.lathe([(0.0, 1.3), (1.3, 1.3), (1.7, 3.4), (1.6, 3.5), (0.0, 3.5)], "Cast", segs=segs,
                xf=T(0.0, 0.0, lz) @ UP)
        p.box((0.8, 0.06, 0.8), (0.0, 3.52, lz), "Char")
    return [p]


lods("blast_furnace", blast_furnace)
