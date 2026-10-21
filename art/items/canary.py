"""The canary: a small brass birdcage with a bird-shaped flame-safety lamp
on a perch inside, a ring handle on top. Writes
assets/models/items/canary.glb (item space: see _kit.py; the item's box is
0.16 x 0.24 x 0.16, centred).

- `body`  the cage and the bird, worn brass with a little verdigris
- `glow`  the lamp's window in the bird's breast, 0.08 above the cage's
          floor: item space (0, -0.04, -0.022), its origin. The game hangs
          its light there and colours it green, amber, red, then dark
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, item, ring, wanted  # noqa: E402
from common import RX, RY, T, Part  # noqa: E402

H = 0.24
Y0 = -H / 2                      # the box's floor
GLOW = (0.0, 0.08 + Y0, -0.022)  # the breast window, 0.08 up from the floor


def canary():
    p = Part("body")
    # The base: a stepped brass dish, its rim patched with verdigris.
    p.lathe([(0.0, 0.0), (0.075, 0.0), (0.078, 0.006), (0.072, 0.012), (0.066, 0.016),
             (0.0, 0.016)], "Brass", segs=24, xf=T(0, Y0, 0) @ UP)
    p.lathe([(0.0778, 0.004), (0.0786, 0.006), (0.0778, 0.008), (0.077, 0.006)], "GreyGreen",
            segs=24, xf=T(0, Y0, 0) @ UP, closed=True, start=30.0, sweep=110.0)
    # Twelve bars up from the base, curving in to the dome's crown.
    n = 12
    for k in range(n):
        a = 2 * math.pi * k / n
        pts = [(0.066, 0.016), (0.066, 0.15), (0.058, 0.18), (0.035, 0.2), (0.0, 0.208)]
        for (r0, y0), (r1, y1) in zip(pts, pts[1:]):
            x0, z0 = r0 * math.cos(a), r0 * math.sin(a)
            x1, z1 = r1 * math.cos(a), r1 * math.sin(a)
            dx, dy, dz = x1 - x0, y1 - y0, z1 - z0
            L = math.sqrt(dx * dx + dy * dy + dz * dz)
            yaw = math.atan2(dx, dz)
            pitch = -math.asin(dy / L)
            p.cyl(0.0014, 0.0, L, "Brass", segs=4,
                  xf=T(x0, Y0 + y0, z0) @ RY(yaw) @ RX(pitch))
    # Two hoops round the bars, the crown boss and the ring handle.
    for y in (0.05, 0.15):
        ring(p, 0.0668, 0.0018, "Brass", xf=T(0, Y0 + y, 0) @ UP, segs=24, sides=4)
    p.sphere(0.012, (0.0, Y0 + 0.206, 0.0), "Brass", segs=12, rings=6, scale=(1, 0.7, 1))
    ring(p, 0.02, 0.0028, "Brass", xf=T(0, Y0 + 0.237 - 0.02, 0), segs=20, sides=6)
    # The perch, across the cage.
    p.cyl(0.0025, -0.064, 0.064, "Brass", segs=6, xf=T(0, Y0 + 0.055, 0.0) @ RY(math.pi / 2))
    # The bird on it, facing -Z: a plump brass body, head and beak, a tail,
    # the wings' edges, its feet round the perch.
    b = Y0 + 0.08
    p.sphere(0.026, (0.0, b, 0.0), "Brass", segs=14, rings=8, scale=(0.8, 0.9, 1.25))
    p.sphere(0.016, (0.0, b + 0.03, -0.022), "Brass", segs=12, rings=6)
    p.lathe([(0.0, 0.0), (0.005, 0.0), (0.0, 0.012)], "Brass", segs=8,
            xf=T(0, b + 0.028, -0.036) @ RX(math.pi))
    p.box((0.024, 0.004, 0.035), (0.0, b + 0.01, 0.038), "Brass", bevel=0.0015, segs=1,
          xf=RX(-0.4, about=(0.0, b + 0.005, 0.03)))
    for sx in (-1, 1):
        p.sphere(0.017, (sx * 0.018, b + 0.004, 0.006), "Brass", segs=10, rings=5,
                 scale=(0.3, 0.7, 1.3))
        p.cyl(0.002, b - 0.026, b - 0.02, "Dark", segs=6, xf=T(sx * 0.008, 0, 0.004) @ UP)
    # A dab of verdigris on its back.
    p.sphere(0.008, (0.004, b + 0.018, 0.012), "GreyGreen", segs=8, rings=4,
             scale=(1.2, 0.35, 1.4))
    # The breast window: its own node, for the game's light.
    g = Part("glow", origin=GLOW)
    g.sphere(0.011, GLOW, "Lamp", segs=10, rings=5, scale=(1.0, 1.1, 0.5))
    return [p, g]


if __name__ == "__main__":
    if wanted("canary"):
        item("canary", canary)
