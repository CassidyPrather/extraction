"""Melee: a fixed-blade field knife (the bayonets are on their guns, see
art/guns/sks.py and mosin.py). Writes assets/models/items/knife.glb (item
space: see _kit.py).

The knife points down -Z, its edge down (-Y), the blade's flats facing
+-X. One node, `body`, and the points Weapon feel's melee reads:

- `pt_grip`    the middle of the handle, where the palm closes
- `pt_tip`     the point
- `pt_edge_a`, `pt_edge_b`  two points along the edge (a near the guard,
               b toward the tip)
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, wanted  # noqa: E402
from common import S, T, Part, reset  # noqa: E402
from grenades import write  # noqa: E402

TIP_Z = -0.135


def spine(z):
    """The blade's back: level, then dropping to the point."""
    if z > -0.07:
        return 0.0115
    k = (z + 0.07) / (TIP_Z + 0.07)
    return 0.0115 - 0.0095 * math.sin(k * math.pi / 2) ** 1.5


def edge(z):
    """The edge: level from the ricasso, then sweeping up to the point."""
    if z > -0.06:
        return -0.0125
    k = (z + 0.06) / (TIP_Z + 0.06)
    return -0.0125 + 0.0145 * k ** 2.2


def knife():
    reset()
    p = Part("body")
    # The blade, lofted from the ricasso to the point: full thickness at
    # the spine down to the grind line, then flat-ground to the edge.
    rings = []
    n = 22
    for i in range(n + 1):
        u = i / n
        z = -0.004 + (TIP_Z + 0.0006 + 0.004) * u ** 0.6
        top, bot = spine(z), edge(z)
        if i == n:
            top, bot = 0.0021, 0.0019
        ricasso = z > -0.012
        g = bot + (top - bot) * (0.92 if ricasso else 0.55)
        t = 0.0022 * (1.0 - 0.6 * u)
        e = 0.0005 if ricasso else 0.00025
        rings.append([(t, top, z), (t, g, z), (e, bot, z), (-e, bot, z), (-t, g, z),
                      (-t, top, z)])
    p.loft(rings, "Steel")
    # A fuller along the flats, just under the spine.
    for sx in (-1, 1):
        p.box((0.0006, 0.0026, 0.06), (sx * 0.0021, 0.0068, -0.048), "Blued", bevel=0.0002,
              segs=1)
    # Guard: a steel plate with rounded ends, a little forward of the handle.
    p.box((0.016, 0.042, 0.006), (0.0, -0.0015, -0.001), "Steel", bevel=0.002, segs=2)
    # Handle: stacked leather washers, oval, with a swell for the palm; a
    # spacer each end and a steel pommel nut.
    prof = [(0.0, 0.002)]
    k = 16
    for i in range(k + 1):
        z = 0.002 + 0.106 * i / k
        r = 0.0115 + 0.0018 * math.sin(math.pi * i / k) - (0.0004 if i % 2 else 0.0)
        prof.append((r, z))
    prof.append((0.0, 0.108))
    p.lathe(prof, "Leather", segs=20, xf=T(0.0, -0.001, 0.0) @ S(0.78, 1.0, 1.0))
    for z0 in (0.002, 0.104):
        p.lathe([(0.0, z0), (0.0118, z0), (0.0118, z0 + 0.003), (0.0, z0 + 0.003)], "Brass",
                segs=20, xf=T(0.0, -0.001, 0.0) @ S(0.78, 1.0, 1.0))
    p.lathe([(0.0, 0.107), (0.0115, 0.107), (0.012, 0.111), (0.0095, 0.118), (0.004, 0.121),
             (0.0, 0.121)], "Steel", segs=20, xf=T(0.0, -0.001, 0.0) @ S(0.78, 1.0, 1.0))
    # A lanyard hole through the pommel.
    p.cyl(0.0024, -0.006, 0.006, "Dark", segs=10,
          xf=T(0.0, -0.001, 0.114) @ ALONG_X)
    write("knife", [p], {
        "pt_grip": (0.0, -0.001, 0.055),
        "pt_tip": (0.0, 0.002, TIP_Z),
        "pt_edge_a": (0.0, edge(-0.03), -0.03),
        "pt_edge_b": (0.0, edge(-0.1), -0.1),
    })


if __name__ == "__main__":
    if wanted("knife"):
        knife()
