"""The player's gloves: worn work/tactical gloves, a left and a right.
Writes assets/models/hands/glove_{left,right}.glb.

Built in the glove's own frame, the one `spawn_hand` in
src/render/props.rs builds its joints in: palm down, fingers along -Z, the
back of the hand +Y, the palm's centre at the origin, the thumb on -X for a
right hand (+X for a left). The game rolls the whole glove onto the hand's
frame.

Objects:

- `palm`: the back of the hand and the palm, the thenar pad at the
  thumb's root, the knuckle guard, seams and the cuff with its strap.
- `finger_F_J` (F 0..3 index to little, 4 the thumb; J 0..2 knuckle
  outwards): one segment, its origin on its joint and its bone along its
  own -Z, curling about its X toward -Y. Each is parented to the one
  before (the first to `palm`), at its rest pose, so the node transforms
  read the same as the joints `spawn_hand` makes (a test checks). The
  game hangs each segment's mesh on its own joint.

Segments are tapered capsules whose round ends are centred on the joints,
so neighbours share a sphere there and a curled finger stays one smooth
shape over the knuckle. The last segment ends exactly at the fingertip
(the index's straight tip is `interact::poke::FINGERTIP`).

Materials: `Glove` (the fabric back), `Leather` (palm and finger
undersides), `Padding` (knuckle guard, finger pads, strap). The game gives
each glove its own copies, to tint when the hand is in reach of something.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RY, RZ, S, T, TO_BLENDER, export, main_guard, make, parent, report)
from mathutils import Vector

# Kept equal to src/render/props.rs (FINGER_BONES, finger_root, finger_aim).
FINGER_BONES = [
    [0.040, 0.025, 0.020],
    [0.044, 0.028, 0.022],
    [0.041, 0.026, 0.020],
    [0.033, 0.020, 0.018],
    [0.036, 0.030, 0.0],
]
FAN = 0.04
THUMB_AIM = (0.75, -0.6)

# Radius at each joint and at the tip, per finger.
RADII = [
    [0.0094, 0.0087, 0.0080, 0.0073],
    [0.0096, 0.0089, 0.0082, 0.0075],
    [0.0092, 0.0085, 0.0078, 0.0072],
    [0.0084, 0.0077, 0.0071, 0.0066],
    [0.0112, 0.0103, 0.0094],
]
SIDES = 10


def finger_root(f, ts):
    if f == 4:
        return (0.038 * ts, -0.008, -0.005)
    return ((0.03 - f * 0.02) * ts, 0.0, -0.048)


def finger_aim(f, ts):
    """The first joint's rotation with the hand open."""
    if f == 4:
        return RY(THUMB_AIM[0] * ts) @ RZ(THUMB_AIM[1] * ts)
    return RY(f * FAN * ts)


def by_normal(p, faces, mat, pred):
    """Repaint the faces whose normal passes `pred`."""
    mi = p._mi(mat)
    for f in faces:
        if pred(f.normal):
            f.material_index = mi


# --- Fingers ---------------------------------------------------------------

def segment(f, j):
    bones = [b for b in FINGER_BONES[f] if b > 0]
    L = bones[j]
    last = j == len(bones) - 1
    r0, r1 = RADII[f][j], RADII[f][j + 1]
    c1 = -(L - r1) if last else -L
    p = Part(f"finger_{f}_{j}")
    n = 3
    prof = [(r0 * math.sin(math.pi / 2 * k / n), r0 * math.cos(math.pi / 2 * k / n))
            for k in range(n + 1)]
    # A glove's finger is a touch fuller over the middle of a segment.
    prof.append(((r0 + r1) * 0.5 + 0.0004, c1 * 0.5))
    prof += [(r1 * math.cos(math.pi / 2 * k / n), c1 - r1 * math.sin(math.pi / 2 * k / n))
             for k in range(n + 1)]
    _, faces = p.lathe(prof, "Glove", segs=SIDES, start=360.0 / SIDES / 2)
    by_normal(p, faces, "Leather", lambda nm: nm.y < -0.25)
    # Pads over the back of the first two segments (the thumb's first).
    if j < 2 and not (f == 4 and j == 1):
        rm = (r0 + r1) * 0.5
        big = j == 0
        p.sphere(1.0, (0, 0, 0), "Padding", segs=10, rings=5,
                 xf=T(0.0, rm - 0.0004, -L * 0.5)
                 @ S(rm * (0.66 if big else 0.56), 0.0026, L * (0.3 if big else 0.24)))
    return p


# --- Palm ------------------------------------------------------------------

Z_WRIST, Z_FRONT = 0.056, -0.058


def palm_shape(z):
    """(half width, centre y, half height) of the hand's cut at z."""
    t = (z - Z_FRONT) / (Z_WRIST - Z_FRONT)  # 0 at the knuckles, 1 at the wrist
    w = 0.0425 - 0.0115 * t ** 1.4
    top = 0.0105 + 0.0055 * math.sin(math.pi * min(1.0, t * 1.1)) - 0.001 * t
    bot = -0.0105 - 0.0058 * min(1.0, t * 1.6)
    # The knuckle end rounds off past the fingers' first joints.
    nose = -0.045
    if z < nose:
        k = math.sqrt(max(0.0, 1.0 - ((z - nose) / (Z_FRONT - nose)) ** 2))
        k = 0.35 + 0.65 * k
        w *= 0.8 + 0.2 * k
        top, bot = top * k, bot * k
    return w, (top + bot) / 2, (top - bot) / 2


POWER = 2.2


def palm_point(z, th, grow=0.0):
    w, yc, h = palm_shape(z)
    c, s = math.cos(th), math.sin(th)
    e = 2.0 / POWER
    ww = w * (0.97 + 0.02 * (s + 1) / 2) + grow
    return Vector((math.copysign(abs(c) ** e, c) * ww, yc + math.copysign(abs(s) ** e, s) * (h + grow), z))


def palm_top(x, z):
    """The back of the hand's height over (x, z), and its normal there."""
    w, yc, h = palm_shape(z)
    ww = w * 0.99
    u = min(abs(x) / ww, 0.999)
    y = yc + h * (1 - u ** POWER) ** (1 / POWER)
    dx = 1e-4
    u2 = min((abs(x) + dx) / ww, 0.999)
    y2 = yc + h * (1 - u2 ** POWER) ** (1 / POWER)
    slope = (y2 - y) / dx * math.copysign(1, x if x != 0 else 1)
    nrm = Vector((-slope, 1.0, 0.0)).normalized()
    return y, nrm


def tube(p, pts, r, mat, sides=6):
    rings = []
    n = len(pts)
    for i in range(n):
        a, b = Vector(pts[max(i - 1, 0)]), Vector(pts[min(i + 1, n - 1)])
        t = (b - a).normalized()
        up = Vector((0, 1, 0)) if abs(t.y) < 0.9 else Vector((1, 0, 0))
        u = t.cross(up).normalized()
        w = t.cross(u).normalized()
        rings.append([Vector(pts[i]) + (u * math.cos(2 * math.pi * k / sides)
                                        + w * math.sin(2 * math.pi * k / sides)) * r
                      for k in range(sides)])
    return p.loft(rings, mat)


def palm(ts):
    p = Part("palm")
    segs = 20
    stations = 16
    rings = []
    for i in range(stations + 1):
        # Bunched toward the rounded knuckle end.
        t = i / stations
        z = Z_WRIST + (Z_FRONT - Z_WRIST) * (1 - (1 - t) ** 1.5)
        rings.append([palm_point(z, 2 * math.pi * k / segs) for k in range(segs)])
    _, faces = p.loft(rings, "Glove")
    by_normal(p, faces, "Leather", lambda nm: nm.y < -0.3)

    # The thenar pad: the thumb's root, on the palm's thumb side.
    p.sphere(1.0, (0, 0, 0), "Leather", segs=12, rings=6,
             xf=T(0.026 * ts, -0.0105, 0.010) @ RY(-0.35 * ts) @ RZ(0.35 * ts)
             @ S(0.0165, 0.0092, 0.030))
    # A heel pad at the outer edge of the palm.
    p.sphere(1.0, (0, 0, 0), "Leather", segs=10, rings=5,
             xf=T(-0.022 * ts, -0.0125, 0.030) @ S(0.015, 0.006, 0.020))

    # Knuckle guard: a padded bar across the back over the first joints.
    zg = -0.036
    bar = []
    xs = [(-0.038 + 0.076 * k / 12) for k in range(13)]
    for x in xs:
        y, nrm = palm_top(x, zg)
        end = 1.0 - (abs(x) / 0.038) ** 6
        th, half = 0.0032 + 0.0018 * end, 0.0075 * (0.6 + 0.4 * end)
        base = Vector((x, y, zg)) - nrm * 0.0015
        ring = []
        for a in range(8):
            ang = 2 * math.pi * a / 8
            ring.append(base + nrm * (th * (1 + math.sin(ang)) * 0.5 + 0.0002)
                        + Vector((0, 0, 1)) * half * math.cos(ang))
        bar.append(ring)
    p.loft(bar, "Padding")
    # Seams down the back of the hand, from the guard to the cuff, and the
    # welt along each side where the leather palm meets the fabric back.
    for x in (-0.013, 0.013):
        pts = []
        for k in range(10):
            z = zg + 0.008 + (0.042 - zg - 0.008) * k / 9
            y, nrm = palm_top(x, z)
            pts.append(Vector((x, y, z)) + nrm * 0.0002)
        tube(p, pts, 0.0011, "Glove", sides=5)
    for sx in (-1, 1):
        pts = []
        for k in range(12):
            z = 0.042 + (-0.047 - 0.042) * k / 11
            w, yc, h = palm_shape(z)
            pts.append((sx * (w * 0.98 + 0.0004), yc - h * 0.12, z))
        tube(p, pts, 0.0012, "Glove", sides=5)

    # The cuff: a short gauntlet over the wrist, with a padded strap and
    # its tab, open at the arm's end.
    prof = [  # (z, grow over the wrist's shape, material)
        (0.030, 0.0010), (0.040, 0.0030), (0.052, 0.0042), (0.060, 0.0046),
        (0.0605, 0.0068), (0.0625, 0.0074), (0.0835, 0.0076), (0.0855, 0.0070),
        (0.086, 0.0048), (0.104, 0.0054), (0.108, 0.0050), (0.110, 0.0036),
        (0.1095, 0.0018), (0.100, 0.0016),
    ]
    zw = Z_WRIST - 0.001
    rings = [[palm_point(zw, 2 * math.pi * k / segs, g) + Vector((0, 0, z - zw))
              for k in range(segs)] for z, g in prof]
    _, faces = p.loft(rings, "Glove")
    for f in faces:
        c = f.calc_center_median()
        if 0.0603 < c.z < 0.0857 and f.normal.z * f.normal.z < 0.5:
            f.material_index = p._mi("Padding")
    # The strap's tab on the little finger's side, sewn down at its end.
    w, yc, h = palm_shape(zw)
    x = -ts * (w * 0.97 + 0.0072)
    p.box((0.0055, 0.016, 0.022), (x - ts * 0.0012, yc + 0.003, 0.0735), "Padding",
          bevel=0.0018, segs=2)
    return p


# --- Assembly --------------------------------------------------------------

def place(obj, game):
    """Put an object built at the origin at `game` (a game-space matrix)."""
    obj.matrix_world = TO_BLENDER @ game @ TO_BLENDER.inverted()


def glove(side):
    main_guard()
    ts = 1.0 if side == "left" else -1.0
    palm_obj = make(palm(ts))
    for f in range(5):
        prev, chain = palm_obj, T(*finger_root(f, ts)) @ finger_aim(f, ts)
        bones = [b for b in FINGER_BONES[f] if b > 0]
        for j in range(len(bones)):
            obj = make(segment(f, j), sharp_angle=60.0)
            place(obj, chain)
            parent(obj, prev)
            prev = obj
            chain = chain @ T(0.0, 0.0, -bones[j])
    report()
    export(f"assets/models/hands/glove_{side}.glb")


for s in ("right", "left"):
    glove(s)
