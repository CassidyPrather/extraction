"""Lev, the driver, standing: a man in his fifties, 1.76 m, in a long dark
wool coat, a flat cap and a scarf, worn boots, his hands at his sides.
Writes assets/models/props/lev_body.glb (one node, `lev_body`).

Prop space: origin on the ground between his feet, facing -Z, +X his
right. Story walks him into the Depot and sways him in code.

His right forearm and hand are left off: Hands' `lev_hand` hangs from the
right sleeve, which ends at the elbow. Empties:

- `pt_right_elbow`  (0.25, 1.1, 0.0), the sleeve's open end
- `pt_right_wrist`  (0.25, 0.85, 0.0), where the wrist goes with the
                    forearm hanging straight down: its axes as the body's
                    (+Y up the forearm toward the elbow, -X toward his
                    side, which the palm faces, -Z forward)

Materials: DarkWool (coat), Wool (trousers, cap), Brick (scarf), Skin,
Leather (boots), Black, Dark (eyes), Ticking (moustache).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, S, T, Part, empty, export, make, report, reset  # noqa: E402
from mathutils import Vector  # noqa: E402

UP = RX(-math.pi / 2)


def ring(c, rx, rz, n=20, dz=0.0):
    """An ellipse round a vertical axis at centre c (x, y, z)."""
    cx, cy, cz = c
    return [Vector((cx + rx * math.cos(2 * math.pi * k / n), cy,
                    cz + dz + rz * math.sin(2 * math.pi * k / n))) for k in range(n)]


def limb(p, pts, radii, mat, n=12):
    """A tube through 3D points with a radius at each, its rings square to
    the path."""
    pts = [Vector(q) for q in pts]
    rings = []
    for i, c in enumerate(pts):
        t = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        a = Vector((1, 0, 0)) if abs(t.x) < 0.9 else Vector((0, 0, 1))
        u = t.cross(a).normalized()
        v = t.cross(u)
        r = radii[i]
        rings.append([c + (u * math.cos(2 * math.pi * k / n) + v * math.sin(2 * math.pi * k / n))
                      * r for k in range(n)])
    p.loft(rings, mat)


def lev():
    reset()
    p = Part("lev_body")
    # Boots, scuffed leather, and the trousers above them.
    for sx in (-1, 1):
        x = sx * 0.1
        p.box((0.1, 0.1, 0.27), (x, 0.062, -0.04), "Leather", bevel=0.03, segs=2)
        p.box((0.104, 0.02, 0.28), (x, 0.01, -0.04), "Black", bevel=0.006, segs=1)
        limb(p, [(x, 0.09, 0.0), (x, 0.3, 0.005), (x * 1.05, 0.5, 0.0)],
             [0.058, 0.062, 0.07], "Wool")
    # The coat: long, from mid-calf to the shoulders, buttoned.
    stations = [(0.40, 0.25, 0.17, -0.005), (0.6, 0.235, 0.16, 0.0), (0.8, 0.215, 0.145, 0.0),
                (0.95, 0.2, 0.14, 0.0), (1.05, 0.19, 0.13, 0.0), (1.2, 0.2, 0.135, -0.005),
                (1.32, 0.205, 0.13, -0.005), (1.41, 0.2, 0.115, 0.0), (1.46, 0.12, 0.09, 0.005),
                (1.48, 0.075, 0.07, 0.01)]
    p.loft([ring((0.0, y, 0.0), rx, rz, n=24, dz=dz) for y, rx, rz, dz in stations], "DarkWool")
    # The collar turned up round the neck, the front edge, buttons, a belt.
    p.lathe([(0.072, 1.44), (0.08, 1.44), (0.085, 1.53), (0.078, 1.53)], "DarkWool", segs=20,
            xf=S(1.0, 1.0, 0.9) @ UP, closed=True)
    for k in range(4):
        y = 0.75 + 0.15 * k
        rz = [s for s in stations if s[0] >= y][0][2]
        p.sphere(0.011, (0.035, y, -rz - 0.002), "Black", segs=8, rings=4)
    p.box((0.012, 1.0, 0.006), (0.005, 0.92, 0.0), "DarkWool", segs=1,
          xf=T(0, 0, -0.142))
    p.lathe([(0.193, 1.0), (0.197, 1.005), (0.197, 1.045), (0.193, 1.05)], "DarkWool",
            segs=24, xf=S(1.0, 1.0, 0.68) @ UP, closed=True)
    # The scarf: wound round the neck, an end hanging down the front.
    p.lathe([(0.06, 1.47), (0.085, 1.48), (0.088, 1.52), (0.06, 1.53)], "Brick", segs=16,
            xf=S(1.0, 1.0, 0.9) @ UP, closed=True)
    p.box((0.07, 0.3, 0.012), (0.0, 0.0, 0.0), "Brick", bevel=0.004, segs=1,
          xf=T(-0.05, 1.33, -0.138) @ RX(-0.08))
    # Neck and head, the face turned a little down; ears, nose, moustache.
    p.cyl(0.05, 1.5, 1.57, "Skin", segs=12, xf=UP)
    p.sphere(0.1, (0.0, 1.645, -0.005), "Skin", segs=16, rings=10, scale=(0.82, 1.05, 0.95))
    p.box((0.024, 0.04, 0.03), (0.0, 1.635, -0.1), "Skin", bevel=0.01, segs=2)
    for sx in (-1, 1):
        p.sphere(0.022, (sx * 0.083, 1.645, 0.0), "Skin", segs=8, rings=4, scale=(0.4, 1.2, 0.8))
    p.box((0.05, 0.012, 0.01), (0.0, 1.608, -0.096), "Ticking", bevel=0.004, segs=1)
    for sx in (-1, 1):
        p.sphere(0.009, (sx * 0.032, 1.665, -0.088), "Dark", segs=8, rings=4,
                 scale=(1.0, 0.6, 0.5))
    # The flat cap: a soft crown pulled forward, its peak over the eyes.
    p.lathe([(0.0, 0.0), (0.1, 0.0), (0.112, 0.012), (0.105, 0.03), (0.07, 0.045),
             (0.0, 0.05)], "Wool", segs=20, xf=T(0.0, 1.705, -0.01) @ RX(0.12) @ S(0.92, 1, 1)
            @ UP)
    p.box((0.16, 0.012, 0.07), (0.0, 1.708, -0.1), "Wool", bevel=0.005, segs=1,
          xf=RX(0.1, about=(0, 1.708, -0.07)))
    # The arms in their sleeves, at his sides. The left down to its hand;
    # the right ends at the elbow (Hands' lev_hand hangs there).
    for sx in (-1, 1):
        shoulder = (sx * 0.2, 1.4, 0.0)
        elbow = (sx * 0.25, 1.1, 0.0)
        limb(p, [shoulder, (sx * 0.235, 1.25, 0.0), elbow], [0.065, 0.06, 0.055], "DarkWool")
        # A round shoulder over the sleeve's top.
        p.sphere(0.072, (sx * 0.19, 1.395, 0.0), "DarkWool", segs=12, rings=6,
                 scale=(1.0, 0.75, 1.0))
        if sx < 0:
            wrist = (sx * 0.25, 0.85, 0.0)
            limb(p, [elbow, (sx * 0.252, 0.97, -0.005), wrist], [0.055, 0.052, 0.05],
                 "DarkWool")
            # The hand, hanging relaxed, fingers curled, thumb forward.
            p.box((0.035, 0.09, 0.085), (sx * 0.25, 0.79, 0.0), "Skin", bevel=0.015, segs=2)
            p.box((0.03, 0.06, 0.08), (sx * 0.245, 0.72, 0.008), "Skin", bevel=0.013, segs=2)
            p.box((0.022, 0.05, 0.024), (sx * 0.24, 0.8, -0.048), "Skin", bevel=0.009, segs=2)
    make(p)
    empty("pt_right_elbow", (0.25, 1.1, 0.0))
    empty("pt_right_wrist", (0.25, 0.85, 0.0))
    print("lev_body:")
    report()
    export("assets/models/props/lev_body.glb")


if __name__ == "__main__":
    lev()
