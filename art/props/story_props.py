"""Story's props. Writes assets/models/props/<name>.glb, each in the frame
its code gives it (claude/polish-story):

- `cork_board`       the notes board in the truck (src/raid/board.rs): a
                     1.20 x 0.70 cork face, 20 mm thick, centred on the
                     origin, its face at z +0.01 facing +Z (cards pin at
                     z 0.014); a timber frame round the edge, standing
                     12 mm proud of the cork; hung from two screws at the
                     top corners on short straps
- `cab_hatch_panel`  the sliding cover over Lev's hatch (src/raid/lev.rs):
                     olive steel 0.32 x 0.28 x 0.015, origin at its centre,
                     facing +Z (into the bed), a finger pull at its +X edge.
                     It runs in the rails on truck.glb's front wall
- `terminal`         a CRT terminal (map terminals, src/world/notes.rs, and
                     the scenes'): origin on the desk under its base; a
                     deep case 0.44 x 0.38 x 0.40 centred (0, 0.22, -0.06),
                     its screen recessed behind the bezel (the bezel's
                     face at z 0.14, the glass at 0.128, under the game's
                     screen panel at 0.145); a base 0.20 x 0.03 x 0.20; a
                     keyboard 0.42 x 0.03 x 0.15 at (0, 0.015, 0.25) tilted
                     0.08 rad toward the user

Materials: Leather (cork), Timber, Iron, Painted, Olive, Cream, Black,
Glass.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, T, Part, empty, export, fillet, make, report, reset  # noqa: E402


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


def cork_board():
    reset()
    W, H = 1.2, 0.7
    b = Part("cork_board")
    b.box((W, H, 0.02), (0.0, 0.0, 0.0), "Leather", segs=1)
    # The frame: four timber rails round the cork, their backs at z -0.008
    # behind the cork's back, their fronts 12 mm proud of its face.
    F, z0, z1 = 0.035, -0.012, 0.022
    for sy in (-1, 1):
        b.box((W + 2 * F, F, z1 - z0), (0.0, sy * (H / 2 + F / 2), (z0 + z1) / 2), "Timber",
              bevel=0.003, segs=1)
    for sx in (-1, 1):
        b.box((F, H, z1 - z0), (sx * (W / 2 + F / 2), 0.0, (z0 + z1) / 2), "Timber",
              bevel=0.003, segs=1)
    # Hung from two screws above the top corners on short straps.
    for sx in (-1, 1):
        x = sx * (W / 2 - 0.05)
        b.box((0.02, 0.09, 0.004), (x, H / 2 + F + 0.03, -0.006), "Leather", segs=1)
        b.cyl(0.006, -0.012, 0.0, "Iron", segs=8, xf=T(x, H / 2 + F + 0.07, 0.0))
    write("cork_board", [b])


def cab_hatch_panel():
    reset()
    W, H, D = 0.32, 0.28, 0.015
    p = Part("cab_hatch_panel")
    p.box((W, H, D), (0.0, 0.0, 0.0), "Olive", bevel=0.002, segs=1)
    # Two pressed stiffening ribs, and the finger pull at the +X edge.
    for y in (-0.07, 0.07):
        p.box((W - 0.06, 0.012, 0.004), (0.0, y, D / 2 + 0.0015), "Olive", segs=1)
    p.box((0.018, 0.07, 0.016), (W / 2 - 0.025, 0.0, D / 2 + 0.008), "Iron", bevel=0.003, segs=1)
    write("cab_hatch_panel", [p])


def terminal():
    reset()
    t = Part("terminal")
    # The case: a deep box tapering to the back, in old beige plastic.
    side = [(0.14, 0.03), (0.14, 0.41), (-0.06, 0.41), (-0.26, 0.36), (-0.26, 0.08),
            (-0.06, 0.03)]
    t.prism(fillet(side, [0.01, 0.01, 0.02, 0.03, 0.03, 0.02], 2), -0.22, 0.22, "Cream",
            axis="x", bevel=0.006, segs=1)
    # The bezel's recess and the curved glass in it (the game's panel sits
    # in front at z 0.145).
    rc = Part("cut")
    rc.box((0.34, 0.26, 0.04), (0.0, 0.23, 0.14), "Black", segs=1)
    t.cut(rc, paint=True)
    t.box((0.336, 0.256, 0.004), (0.0, 0.23, 0.124), "Glass", segs=1)
    # Vents on the top, a power switch and lamp on the front below the screen.
    # On the flat of the top (they stood over the slope behind it).
    for k in range(5):
        t.box((0.24, 0.003, 0.012), (0.0, 0.4115, 0.05 - 0.022 * k), "Black", segs=1)
    t.box((0.02, 0.012, 0.008), (0.17, 0.06, 0.142), "Black", segs=1)
    t.cyl(0.004, 0.0, 0.003, "Warn", segs=8, xf=T(0.14, 0.06, 0.14))
    # The base under it.
    t.box((0.2, 0.03, 0.2), (0.0, 0.015, -0.06), "Cream", bevel=0.004, segs=1)
    # The keyboard, tilted toward the user, its keys in rows.
    kb = T(0.0, 0.015, 0.25) @ RX(0.08)
    t.box((0.42, 0.03, 0.15), (0.0, 0.0, 0.0), "Cream", bevel=0.004, segs=1, xf=kb)
    for row in range(4):
        for k in range(12 - (row == 3) * 4):
            x = -0.165 + k * 0.03 + (0.06 if row == 3 else 0.0)
            t.box((0.024, 0.008, 0.022), (x, 0.018, -0.045 + row * 0.03), "Black", segs=1, xf=kb)
    t.box((0.15, 0.008, 0.022), (0.0, 0.018, 0.072), "Black", segs=1, xf=kb)   # space bar
    write("terminal", [t])


if __name__ == "__main__":
    for name, build in (("cork_board", cork_board), ("cab_hatch_panel", cab_hatch_panel),
                        ("terminal", terminal)):
        if wanted(name):
            build()
