"""The works gas holder (gasometer): Cinder's Ash Row, the valley's first
landmark seen from the truck. Writes assets/models/props/gas_holder.glb
and its lighter gas_holder_lod1.glb (the middle distance) and
gas_holder_lod2.glb (the far stand-in), in prefab space (origin on the
ground at the tank's middle, +Y up, -Z the front), for the recipe of the
same name in src/world/buildings.rs, whose collider pieces it fills.

A dented, rusted tank inside a ring guide-frame of eight columns, braced
in two tiers between them, a girder ring at mid-height and round the
top: the frame is what reads against the sky from the gate, so its
columns and braces are thick enough to show at 100 m (a column 0.4 m, a
brace 0.16 m). The fire went through the Row: soot runs down the tank,
one column is bent at its top tier and drags the top ring out with it.
13.9 m to the top ring's highest point, a lamp post on the front-right
column's head to 14.2 m (the game's landmark lamp stands on it: the one
highest point), never over 14.5; 13.4 m across the columns, 13.8 across
the plinth. No lamp or glow of its own.

Level 0 is the full model; 1 drops the guide rollers, the soot and the
dents' detail; 2 is the silhouette in big shapes (tank, columns, top
ring) for past `lod::FAR_BUILDING`.

Materials: Painted (the tank, its paint rust-blotched), Rusty (the
frame), Char (soot), Concrete (the plinth).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, Wear, prop  # noqa: E402
from common import Part, RX, T  # noqa: E402
from mathutils import Vector  # noqa: E402

import _kit  # noqa: E402

# A big open frame: its tank's foot shades further.
_kit.AO_DISTANCE = 2.0

# Columns: how many, their ring (centre lines), their section, height.
N = 8
RING = 6.35
COL = 0.4
TOP = 13.6
MID = 6.8
# The tank: radius, its wall's top, its crown's top.
TANK_R = 5.4
TANK_TOP = 8.6
CROWN = 9.5
# The column the lamp post stands on (front right), and the post's height
# over its top: 14.2 m, the highest of it, under the 14.5 m cap.
LAMP_COLUMN = 0
LAMP_POST = 0.6
# The bent column, and how far its top is dragged out (m) and down.
BENT = 5
BENT_OUT = 0.55
BENT_DROP = 0.25


def beam(p, a, b, w, mat, d=None):
    """A square-section beam from a to b."""
    a, b = Vector(a), Vector(b)
    v = b - a
    if v.normalized().y < -0.9999:
        rot = RX(math.pi)
    else:
        rot = Vector((0, 1, 0)).rotation_difference(v.normalized()).to_matrix().to_4x4()
    mid = (a + b) / 2
    p.box((w, v.length, d or w), (0.0, 0.0, 0.0), mat, segs=1, xf=T(*mid) @ rot)


def column_at(k, y):
    """Column k's centre line at height y: straight, but for the bent one,
    which kinks out above the mid ring."""
    a = math.radians(22.5 + 45.0 * k)
    out, drop = 0.0, 0.0
    if k == BENT and y > MID:
        t = (y - MID) / (TOP - MID)
        out = BENT_OUT * t * t
        drop = BENT_DROP * t
    r = RING + out
    return Vector((r * math.sin(a), y - drop, -r * math.cos(a)))


def tank(p, lod):
    """The tank: a dented wall, a shallow crown, a ring round its rim."""
    segs = (40, 20, 12)[lod]
    rings = (12, 4, 1)[lod]
    w = Wear(29)
    dents = [(w.rng.uniform(0, 2 * math.pi), w.rng.uniform(1.5, 7.5),
              w.rng.uniform(0.12, 0.3), w.rng.uniform(0.7, 1.4)) for _ in range(5)]
    import bmesh
    bm = p.bm
    rows = []
    for j in range(rings + 1):
        y = 0.25 + (TANK_TOP - 0.25) * j / rings
        row = []
        for i in range(segs):
            a = 2 * math.pi * i / segs
            r = TANK_R
            if lod < 2:
                for da, dy, depth, size in dents:
                    d = (a - da + math.pi) % (2 * math.pi) - math.pi
                    arc = d * TANK_R
                    r -= depth * math.exp(-(arc / size) ** 2 - ((y - dy) / size) ** 2)
            row.append(bm.verts.new((r * math.sin(a), y, -r * math.cos(a))))
        rows.append(row)
    faces = []
    for r0, r1 in zip(rows, rows[1:]):
        for i in range(segs):
            faces.append(bm.faces.new((r0[i], r0[(i + 1) % segs], r1[(i + 1) % segs], r1[i])))
    foot = bm.verts.new((0.0, 0.25, 0.0))
    for i in range(segs):
        faces.append(bm.faces.new((rows[0][(i + 1) % segs], rows[0][i], foot)))
    # The crown: a shallow dome from the wall's top.
    crown = []
    for j in range(1, 4):
        t = j / 4
        y = TANK_TOP + (CROWN - TANK_TOP) * math.sin(t * math.pi / 2)
        r = TANK_R * math.cos(t * math.pi / 2 * 0.95)
        crown.append([bm.verts.new((r * math.sin(2 * math.pi * i / segs), y,
                                    -r * math.cos(2 * math.pi * i / segs)))
                      for i in range(segs)])
    prev = rows[-1]
    for row in crown:
        for i in range(segs):
            faces.append(bm.faces.new((prev[i], prev[(i + 1) % segs], row[(i + 1) % segs],
                                       row[i])))
        prev = row
    top = bm.verts.new((0.0, CROWN, 0.0))
    for i in range(segs):
        faces.append(bm.faces.new((prev[i], prev[(i + 1) % segs], top)))
    verts = [v for row in rows + crown for v in row] + [foot, top]
    p._finish_piece(verts, faces, "Painted")
    if lod == 0:
        # The rim's angle ring, and the guide rollers' arms out to the
        # columns (the tank rose and fell between them).
        p.tube(TANK_R + 0.12, TANK_R - 0.02, TANK_TOP - 0.15, TANK_TOP + 0.05, "Rusty", segs=segs,
               xf=UP)
        for k in range(N):
            c = column_at(k, TANK_TOP - 0.05)
            a = math.radians(22.5 + 45.0 * k)
            rim = Vector((TANK_R * math.sin(a), TANK_TOP - 0.05, -TANK_R * math.cos(a)))
            beam(p, rim, c - (c - rim).normalized() * (COL / 2), 0.14, "Rusty")
        # Soot run down from the crown, where the fire licked it: dark
        # strips stood off the wall (4 mm, so the two don't fight).
        for a0, length in ((0.3, 4.5), (1.4, 2.8), (2.6, 5.5), (3.9, 3.6), (5.1, 4.8)):
            for s in (-1, 1):
                a = a0 + s * 0.04
                r = TANK_R + 0.006
                p.box((0.35, length, 0.004), (0.0, 0.0, 0.0), "Char", segs=1,
                      xf=T(r * math.sin(a), TANK_TOP - length / 2, -r * math.cos(a))
                      @ _ry(-a))


def _ry(a):
    from common import RY
    return RY(a)


def frame(p, lod):
    """The guide frame: eight columns, a girder ring at mid-height and round
    the top, cross-bracing in both tiers between them."""
    for k in range(N):
        # (Its foot inside the plinth, not on its underside.)
        pts = [column_at(k, y) for y in (-0.15, MID, (MID + TOP) / 2, TOP)]
        for a, b in zip(pts, pts[1:]):
            beam(p, a, b, COL, "Rusty")
    for k in range(N):
        j = (k + 1) % N
        # (Narrower than the columns, so no face of a ring lies in a
        # column's.)
        for y, w in ((MID, 0.32), (TOP, 0.36)):
            if lod == 2 and y == MID:
                continue
            beam(p, column_at(k, y), column_at(j, y), w, "Rusty")
        if lod == 2:
            continue
        # Cross-bracing, both tiers: what the sky shows through.
        for y0, y1 in ((0.3, MID), (MID, TOP - 0.1)):
            # One brace of each X 3 cm further out than the other, so the
            # two don't lie in one plane where they cross.
            a0, a1 = column_at(k, y0), column_at(j, y1)
            b0, b1 = column_at(j, y0), column_at(k, y1)
            mid = (a0 + b0) / 2
            out = Vector((mid.x, 0.0, mid.z)).normalized() * 0.03
            beam(p, a0, a1, 0.16, "Rusty")
            beam(p, b0 + out, b1 + out, 0.16, "Rusty")


def lamp_post(p):
    """A short post on the front-right column's head, the frame's one
    highest point: the game's landmark lamp stands on it (on a ring of
    level tops it would hang over the tank's middle)."""
    top = column_at(LAMP_COLUMN, TOP)
    beam(p, top, top + Vector((0.0, LAMP_POST, 0.0)), 0.16, "Rusty")


def gas_holder(lod):
    p = Part("static")
    # A concrete plinth the tank stands in.
    p.lathe([(0.0, -0.2), (RING + 0.55, -0.2), (RING + 0.55, 0.25), (0.0, 0.25)], "Concrete",
            segs=(32, 16, 12)[lod], xf=UP)
    tank(p, lod)
    frame(p, lod)
    lamp_post(p)
    return [p]


def lods(name, build):
    prop(name, lambda: build(0))
    prop(f"{name}_lod1", lambda: build(1))
    prop(f"{name}_lod2", lambda: build(2))


lods("gas_holder", gas_holder)
