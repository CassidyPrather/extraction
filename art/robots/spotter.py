"""The spotter: the Custodian's tethered observation balloon. A gored
envelope with a hazard band and three tail fins, a camera pod slung under
it on short rigging, a beacon under the pod, and the winch on the ground
that holds its cable. Writes assets/models/robots/spotter.glb.

World scale, metres, +Y up, -Z forward. Objects and their origins (each
the spawn offset of its part or piece):

- `envelope` (0, 0, 0)          the gas envelope, centred on its origin:
                                2.8 m across, 3.2 m tall, twelve gores with
                                dark seam ribs, a dark band round its waist
                                with a hazard stripe, a valve cap on top, a
                                load collar underneath, three fins at the
                                back (+Z)
- `gondola`  (0, -2.0, 0)       the camera pod, 0.5 x 0.4 x 0.5, centred on its
                                origin, with its hanger and rigging up to
                                the envelope's load collar and skin
- `eye`      (0, -2.0, -0.27)   the camera on the pod's front: a hooded bezel,
                                its lens looking -Z
- `lamp`     (0, -2.25, 0)      the beacon under the pod (the game hangs a
                                light here)
- `winch`    (6, 0, 0)          the ground winch, its skid on the ground
                                (y 0) round its origin: 1.2 x 0.9 x 0.8, the
                                drum across X, a motor box, the cable guide
                                on top at y 0.9 straight above the origin
                                (apart in the file, as in the game)

Every object has `<name>_lod1` and `<name>_lod2` stand-ins (fewer sides,
no small detail). There is no tether: the game stretches the sentry's
cable from the winch's guide to the gondola.
"""

import math
import os
import sys

from mathutils import Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, export, fillet, make, report, reset,
                    section_rect)

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
SEGS = (48, 20, 10)

# The envelope: a superellipse of revolution about Y.
R, H, POW = 1.4, 1.6, 2.2
GONDOLA = (0.0, -2.0, 0.0)
EYE = (0.0, -2.0, -0.27)
LAMP = (0.0, -2.25, 0.0)
WINCH = (6.0, 0.0, 0.0)


def suffix(lod):
    return "" if lod == 0 else f"_lod{lod}"


def radius(y):
    """The envelope's radius at height y."""
    t = min(1.0, abs(y) / H)
    return R * max(0.0, 1.0 - t ** POW) ** (1.0 / POW)


def rod(p, a, b, r, mat, segs=8):
    """A round rod from game-space point a to b."""
    a, b = Vector(a), Vector(b)
    d = b - a
    rot = d.to_track_quat("Z", "Y").to_matrix().to_4x4()
    p.cyl(r, 0.0, d.length, mat, segs=segs, xf=T(*a) @ rot)


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    out = []

    # --- Envelope ------------------------------------------------------------------
    env = Part("envelope" + suffix(lod))
    rings = 40 if full else (20 if lod == 1 else 12)
    prof = []
    for i in range(rings + 1):
        y = H * math.sin(-math.pi / 2 + math.pi * i / rings)
        prof.append((radius(y), y))
    prof[0] = (0.0, -H)
    prof[-1] = (0.0, H)
    env.lathe(prof, "Shell", segs=n, xf=UP)
    # Seam ribs down the twelve gores, dark and proud.
    if lod < 2:
        path = []
        steps = 36 if full else 16
        for i in range(steps + 1):
            y = 1.58 * math.sin(-math.pi / 2 + math.pi * i / steps)
            path.append((radius(y), y))
        for k in range(12):
            env.sweep(path, section_rect(0.05, 0.03), "Dark",
                      xf=RY(2 * math.pi * k / 12))
    # The waist band and its hazard stripe.
    ys = [-0.12 + 0.06 * i for i in range(5)]
    band = [(radius(y) + 0.025, y) for y in ys] + [(radius(y) - 0.03, y) for y in reversed(ys)]
    env.lathe(band, "Dark", segs=n, xf=UP, closed=True)
    ys = [-0.045 + 0.0225 * i for i in range(5)]
    stripe = [(radius(y) + 0.036, y) for y in ys] + [(radius(y), y) for y in reversed(ys)]
    env.lathe(stripe, "Warn", segs=n, xf=UP, closed=True)
    # Valve cap on top, load collar underneath (the ribs end under both).
    ch = 0.012 if lod < 2 else 0.0
    env.cyl(0.33, 1.52, 1.63, "Dark", segs=n, xf=UP, chamfer=ch)
    env.cyl(0.33, -1.66, -1.52, "Dark", segs=n, xf=UP, chamfer=ch)
    if full:
        env.cyl(0.08, 1.62, 1.68, "Steel", segs=12, xf=UP, chamfer=0.01)
    # Three stabiliser fins at the back, their roots buried in the skin.
    fin = fillet([(0.8, 0.06), (1.6, 0.16), (1.98, 0.86), (1.76, 0.95), (0.8, 0.6)],
                 [0.0, 0.05, 0.05, 0.05, 0.0], 3 if full else 1)
    for a in (0.0, 120.0, 240.0):
        env.prism(fin, -0.014, 0.014, "Shell", axis="x", bevel=0.008 if lod < 2 else 0.0,
                  segs=1, xf=RZ(math.radians(a)))
        if lod < 2:
            # A dark trailing edge.
            edge = [(1.56, 0.15), (1.66, 0.17), (2.02, 0.86), (1.93, 0.9)]
            env.prism(edge, -0.02, 0.02, "Dark", axis="x", segs=1, xf=RZ(math.radians(a)))
    out.append(env)

    # --- Gondola ---------------------------------------------------------------------
    gx, gy, gz = GONDOLA
    gon = Part("gondola" + suffix(lod), origin=GONDOLA)
    gon.box((0.5, 0.4, 0.5), (gx, gy, gz), "Shell", bevel=0.03 if lod < 2 else 0.0,
            segs=2 if full else 1)
    # A dark skirt round its belly and a top plate under the hanger.
    gon.box((0.52, 0.1, 0.52), (gx, gy - 0.13, gz), "Dark", bevel=0.015 if lod < 2 else 0.0,
            segs=1)
    gon.box((0.36, 0.03, 0.36), (gx, gy + 0.205, gz), "Dark", segs=1)
    # The hanger post up into the load collar.
    gon.cyl(0.035, gy + 0.21, -1.6, "Steel", segs=max(6, n // 3), xf=UP)
    if lod < 2:
        gon.cyl(0.06, gy + 0.215, gy + 0.26, "Dark", segs=max(8, n // 3), xf=UP, chamfer=0.008)
        # Rigging from the pod's top corners out to the skin.
        for sx in (-1, 1):
            for sz in (-1, 1):
                rod(gon, (sx * 0.17, gy + 0.2, sz * 0.17), (sx * 0.47, -1.36, sz * 0.47), 0.008,
                    "Steel", segs=6)
    if full:
        # Hazard stripes down its flanks, a lit status strip, an aerial.
        for sx in (-1, 1):
            gon.box((0.006, 0.05, 0.3), (sx * 0.253, gy + 0.06, gz), "Warn", segs=1)
            gon.box((0.006, 0.016, 0.16), (sx * 0.253, gy - 0.02, gz - 0.05), "Glow", segs=1)
        gon.cyl(0.006, gy + 0.2, gy + 0.42, "Dark", segs=6, xf=T(0.15, 0, 0.15) @ UP)
    out.append(gon)

    # --- Eye ---------------------------------------------------------------------------
    ex, ey, ez = EYE
    eye = Part("eye" + suffix(lod), origin=EYE)
    aim = T(ex, ey, 0) @ RY(math.pi)  # cyl axis +Z -> -Z
    eye.cyl(0.11, 0.23, 0.29, "Dark", segs=max(8, n // 2), xf=aim,
            chamfer=0.01 if lod < 2 else 0.0)
    eye.cyl(0.075, 0.28, 0.296, "Eye", segs=max(8, n // 2), xf=aim)
    if lod < 2:
        # A hood over the lens against the sun.
        eye.box((0.26, 0.02, 0.09), (ex, ey + 0.12, ez - 0.02), "Dark", segs=1)
    out.append(eye)

    # --- Beacon ------------------------------------------------------------------------
    lx, ly, lz = LAMP
    lamp = Part("lamp" + suffix(lod), origin=LAMP)
    lamp.cyl(0.06, ly + 0.035, ly + 0.06, "Dark", segs=max(8, n // 2), xf=UP)
    lamp.lathe([(0.0, ly + 0.036), (0.045, ly + 0.036), (0.045, ly + 0.0),
                (0.03, ly - 0.03), (0.0, ly - 0.04)], "Lamp", segs=max(8, n // 2), xf=UP)
    if full:
        # A wire guard over it.
        for a in (0.0, math.pi / 2):
            lamp.box((0.11, 0.008, 0.008), (0.0, ly - 0.042, 0.0), "Dark", segs=1, xf=RY(a))
    out.append(lamp)

    # --- Winch -------------------------------------------------------------------------
    wx, wy, wz = WINCH
    w = Part("winch" + suffix(lod), origin=WINCH)
    wb = 0.006 if lod < 2 else 0.0
    # The skid: two runners with upturned ends, a deck across them.
    for sz in (-1, 1):
        runner = fillet([(wx - 0.6, 0.06), (wx - 0.56, 0.0), (wx + 0.56, 0.0), (wx + 0.6, 0.06),
                         (wx + 0.6, 0.08), (wx - 0.6, 0.08)], [0, 0.03, 0.03, 0, 0, 0],
                        2 if lod < 2 else 1)
        w.prism(runner, wz + sz * 0.355 - 0.03, wz + sz * 0.355 + 0.03, "Steel", axis="z")
    w.box((1.12, 0.04, 0.8), (wx, 0.095, wz), "Dark", bevel=wb, segs=1)
    # The drum across X, between two cheek plates, on its axle.
    DX, DY = wx - 0.07, 0.4
    cheek = fillet([(-0.17, 0.11), (0.17, 0.11), (0.12, DY + 0.06), (-0.12, DY + 0.06)],
                   [0, 0, 0.05, 0.05], 2 if lod < 2 else 1)
    for x in (DX - 0.34, DX + 0.34):
        w.prism(cheek, x - 0.02, x + 0.02, "Shell", axis="x", bevel=wb, segs=1)
    w.cyl(0.035, DX - 0.38, DX + 0.38, "Steel", segs=max(8, n // 3), xf=T(0, DY, wz) @ SIDE)
    w.cyl(0.16, DX - 0.28, DX + 0.28, "Dark", segs=n, xf=T(0, DY, wz) @ SIDE)
    w.cyl(0.21, DX - 0.26, DX + 0.26, "Rubber", segs=n, xf=T(0, DY, wz) @ SIDE)
    for x in (DX - 0.3, DX + 0.3):
        w.cyl(0.27, x - 0.02, x + 0.02, "Steel", segs=n, xf=T(0, DY, wz) @ SIDE,
              chamfer=0.006 if lod < 2 else 0.0)
    # The motor box off the drum's right end, its shaft into the cheek.
    w.box((0.24, 0.32, 0.4), (wx + 0.46, 0.27, wz), "Shell", bevel=0.02 if lod < 2 else 0.0,
          segs=2 if full else 1)
    w.box((0.26, 0.05, 0.42), (wx + 0.46, 0.16, wz), "Dark", segs=1)
    w.cyl(0.06, DX + 0.3, wx + 0.34, "Dark", segs=max(8, n // 3), xf=T(0, DY, wz) @ SIDE)
    # The guide: a post at the back, an arm over the drum, a fairlead
    # ring straight above the origin, the cable down from it to the drum.
    w.box((0.08, 0.86, 0.08), (wx, 0.53, wz + 0.33), "Dark", bevel=wb, segs=1)
    w.box((0.065, 0.07, 0.4), (wx, 0.9, wz + 0.16), "Dark", bevel=wb, segs=1)
    w.tube(0.065, 0.025, -0.045, 0.045, "Steel", segs=max(8, n // 2), xf=T(wx, 0.9, wz) @ UP,
           chamfer=0.008 if lod < 2 else 0.0)
    w.cyl(0.018, 0.55, 0.92, "Rubber", segs=8, xf=T(wx, 0, wz) @ UP)
    if lod < 2:
        # Rollers either side of the fairlead's throat.
        for sx in (-1, 1):
            w.cyl(0.018, -0.05, 0.05, "Steel", segs=8, xf=T(wx + sx * 0.045, 0.85, wz))
        # A diagonal brace from the deck to the post.
        rod(w, (wx, 0.11, wz + 0.05), (wx, 0.5, wz + 0.3), 0.02, "Dark", segs=8)
    if full:
        w.box((0.006, 0.06, 0.25), (wx + 0.583, 0.33, wz), "Warn", segs=1)
        w.box((0.006, 0.03, 0.03), (wx + 0.583, 0.23, wz - 0.1), "Glow", segs=1)
        # Lifting eyes at the deck corners.
        for sx in (-1, 1):
            for sz in (-1, 1):
                w.tube(0.03, 0.016, -0.008, 0.008, "Steel", segs=10,
                       xf=T(wx + sx * 0.5, 0.14, wz + sz * 0.34) @ SIDE)
    out.append(w)
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        make(p)
report()
export("assets/models/robots/spotter.glb")
