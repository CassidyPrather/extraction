"""The Cinder furnace cellars' machinery, for Underground: a travelling
crane over the ash pit and the vent levers. Soot-black cast and wrought
iron, rust, heat-blued steel nearest the furnaces. Writes
assets/models/props/<name>.glb (prop space: metres, Y up, -Z the front).

- `cellar_crane_bridge`  the crane's bridge: a 1.0 m deep riveted I-beam,
                         8 m along X, its end carriages' wheels on the
                         runway rails (at y -0.62, z 0), origin at its
                         centre; the trolley's rails along its bottom flange
- `cellar_crane_hook`    the hook block, 0.4 x 0.6 x 0.3, origin at its top
                         (where the chain meets it): side plates round a
                         sheave, a forged hook under it; `chain` above, its
                         origin at the block's top, 1.0 m of links up +Y
                         (the game scales its Y to the drop); `pt_grip` on
                         the bar across the block's top, where a hand
                         holds on to ride
- `cellar_vent`          a cast-iron vent box on a wall, 0.5 x 0.8 x 0.25,
                         origin at the centre of its back (on the wall),
                         louvres on its front; `lever` pivots about X at
                         its base (origin on the pivot), modelled up
                         (shut), the handle forward

Materials: Iron, Rusty, Cast, Blued, Steel, Black, Warn.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, S, T, Part, empty, export, make, report, reset  # noqa: E402
from mathutils import Vector  # noqa: E402

UP = RX(-math.pi / 2)
ALONG_X = RY(math.pi / 2)


def wanted(name):
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or name in argv


def write(name, parts, points=None):
    for p in parts:
        make(p)
    for k, v in (points or {}).items():
        empty(k, v)
    print(f"{name}:")
    report()
    export(f"assets/models/props/{name}.glb")


def crane_bridge():
    reset()
    b = Part("cellar_crane_bridge")
    L, H, F = 8.0, 1.0, 0.4
    # The I-beam: flanges top and bottom, the web, stiffeners every 0.8 m.
    for y in (H / 2 - 0.02, -H / 2 + 0.02):
        b.box((L, 0.04, F), (0.0, y, 0.0), "Iron", bevel=0.004, segs=1)
    b.box((L, H - 0.08, 0.025), (0.0, 0.0, 0.0), "Iron", segs=1)
    for k in range(11):
        x = -4.0 + 0.8 * k
        for sz in (-1, 1):
            b.box((0.02, H - 0.08, F / 2 - 0.02), (x, 0.0, sz * (F / 4 + 0.006)), "Iron", segs=1)
    # Rust streaks down the web, soot along the bottom flange.
    for x in (-2.9, -0.6, 1.7, 3.1):
        for sz in (-1, 1):
            b.box((0.12, 0.5, 0.004), (x, 0.08, sz * 0.0145), "Rusty", segs=1)
    # The trolley's rails along the bottom flange's top, both sides.
    for sz in (-1, 1):
        b.box((L - 0.4, 0.03, 0.025), (0.0, -H / 2 + 0.05, sz * 0.13), "Steel", segs=1)
    # End carriages: a box girder across each end, two wheels on the runway
    # rails, a buffer facing out.
    for sx in (-1, 1):
        x = sx * (L / 2 + 0.1)
        b.box((0.3, 0.35, 1.4), (x, -H / 2 + 0.05, 0.0), "Iron", bevel=0.01, segs=1)
        for sz in (-1, 1):
            b.cyl(0.13, -0.05, 0.05, "Rusty", segs=16,
                  xf=T(x, -0.62 + 0.13 - 0.13, sz * 0.5) @ ALONG_X, chamfer=0.01)
            b.cyl(0.15, -0.012, 0.012, "Rusty", segs=16,
                  xf=T(x - sx * 0.05, -0.62, sz * 0.5) @ ALONG_X)
        b.cyl(0.08, 0.0, 0.12, "Black", segs=12,
              xf=T(x + sx * 0.15, -H / 2 + 0.05, 0.0) @ RY(sx * math.pi / 2))
    # A hazard band on each end of the beam.
    for sx in (-1, 1):
        b.box((0.2, H - 0.1, 0.03), (sx * (L / 2 - 0.15), 0.0, 0.0), "Warn", segs=1)
    write("cellar_crane_bridge", [b])


def crane_hook():
    reset()
    k = Part("cellar_crane_hook")
    # Origin at the block's top: side plates hanging below it round a sheave.
    for sz in (-1, 1):
        k.box((0.4, 0.34, 0.03), (0.0, -0.17, sz * 0.12), "Iron", bevel=0.01, segs=1)
    k.cyl(0.13, -0.09, 0.09, "Blued", segs=20, xf=T(0, -0.17, 0), chamfer=0.01)
    k.cyl(0.03, -0.15, 0.15, "Steel", segs=10, xf=T(0, -0.17, 0))
    # The grab bar across the top, the swivel and the forged hook.
    k.cyl(0.02, -0.18, 0.18, "Black", segs=10, xf=T(0, -0.015, 0) @ ALONG_X)
    k.cyl(0.05, -0.39, -0.34, "Iron", segs=14, xf=UP, chamfer=0.008)
    k.cyl(0.025, -0.43, -0.39, "Blued", segs=10, xf=UP)
    # A thick hook: down from the swivel, then round its bowl (a circle
    # beside the shank) and up to the point, tapering.
    cx, cy, r = 0.035, -0.53, 0.06
    pts = [(0.0, -0.43), (-0.012, -0.46), (-0.022, -0.5)]
    for i in range(12):
        a = math.radians(185 + 215 * i / 11)
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    rings = []
    for i, (x, y) in enumerate(pts):
        a = pts[max(i - 1, 0)]
        b_ = pts[min(i + 1, len(pts) - 1)]
        t = Vector((b_[0] - a[0], b_[1] - a[1], 0.0)).normalized()
        nrm = Vector((-t.y, t.x, 0.0))
        r = 0.032 * (1.0 - 0.6 * i / (len(pts) - 1))
        c = Vector((x, y, 0.0))
        rings.append([c + nrm * r * math.cos(2 * math.pi * j / 8)
                      + Vector((0, 0, 1)) * r * math.sin(2 * math.pi * j / 8) for j in range(8)])
    k.loft(rings, "Blued")
    # The chain: 1.0 m of links up from the block's top, alternately turned.
    ch = Part("chain")
    n = 17
    pitch = 1.0 / n
    for i in range(n):
        y = pitch * (i + 0.5)
        rot = RY(math.pi / 2) if i % 2 else T()
        ch.lathe([(0.012, -0.004), (0.016, 0.0), (0.012, 0.004), (0.008, 0.0)], "Iron", segs=10,
                 xf=T(0, y, 0) @ rot @ S(1.0, 2.3, 1.0),
                 closed=True)
    write("cellar_crane_hook", [k, ch], {"pt_grip": (0.0, -0.015, 0.0)})


def vent():
    reset()
    W, H, D = 0.5, 0.8, 0.25
    v = Part("cellar_vent")
    # Origin at the back's centre on the wall; the box out to -Z.
    # (The box runs into the wall plate, not flush with its back.)
    FZ = -(D - 0.01)
    v.box((W, H, -FZ - 0.005), (0.0, 0.0, (FZ - 0.005) / 2), "Cast", bevel=0.015, segs=1)
    v.box((W + 0.04, H + 0.04, 0.02), (0.0, 0.0, -0.01), "Cast", bevel=0.005, segs=1)
    for k in range(7):
        y = -0.24 + 0.07 * k
        v.box((W - 0.08, 0.012, 0.05), (0.0, 0.0, 0.0), "Black", segs=1,
              xf=T(0.0, y + 0.1, FZ - 0.008) @ RX(0.6))
    # Bolts at the corners, a soot stain up from the louvres, the pivot boss.
    for sx in (-1, 1):
        for sy in (-1, 1):
            v.cyl(0.012, 0.0, 0.012, "Rusty", segs=6,
                  xf=T(sx * (W / 2), sy * (H / 2), -0.02) @ RY(math.pi))
    v.box((W - 0.12, 0.25, 0.003), (0.0, 0.33, FZ - 0.001), "Black", segs=1)
    piv = (0.0, -H / 2 + 0.08, -D - 0.03)
    v.cyl(0.035, FZ + 0.005, -D - 0.03, "Iron", segs=12, xf=T(0, piv[1], 0))
    # The lever: an iron arm up from the pivot, a heat-blued handle across
    # its top.
    lever = Part("lever", origin=piv)
    lever.box((0.03, 0.55, 0.02), (0.0, piv[1] + 0.275, piv[2] - 0.012), "Iron", bevel=0.004,
              segs=1)
    lever.cyl(0.02, piv[2] - 0.01, piv[2] - 0.035, "Steel", segs=10, xf=T(0, piv[1], 0))
    lever.cyl(0.018, 0.0, 0.12, "Blued", segs=10,
              xf=T(0.0, piv[1] + 0.55, piv[2] - 0.022) @ RX(math.pi), chamfer=0.004)
    write("cellar_vent", [v, lever])


if __name__ == "__main__":
    for name, build in (("cellar_crane_bridge", crane_bridge), ("cellar_crane_hook", crane_hook),
                        ("cellar_vent", vent)):
        if wanted(name):
            build()
