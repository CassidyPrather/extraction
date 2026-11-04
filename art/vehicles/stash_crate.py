"""The stash crate bolted to the truck bed: an open-topped plank crate with
corner posts, steel corner plates and rope handles. Writes
assets/models/vehicles/stash_crate.glb.

Crate space: origin at the stash's centre (truck space (-0.55, FLOOR +
0.35, -1.0)), so the bed floor is at y -0.35. The stash holds things in a
0.9 x 0.6 x 1.2 box; the crate is 0.95 x 0.65 x 1.25 outside.

- `crate`  the whole crate (static)

Materials: Boards, Iron, Rope.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RY, T, export, main_guard, make, report
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

W, D = 0.95, 1.25
Y0, Y1 = -0.35, 0.3
c = Part("crate")
# Floor boards inside, raised on two runners across them all (along the
# crate, the runners missed the middle board), a hair into them.
for z in (-0.45, 0.45):
    c.box((W - 0.06, 0.04, 0.08), (0.0, Y0 + 0.02, z), "Boards", bevel=0.004, segs=1)
for k in range(5):
    w = (W - 0.06) / 5
    c.box((w - 0.006, 0.022, D - 0.06), (-W / 2 + 0.03 + w * (k + 0.5), Y0 + 0.0505, 0.0),
          "Boards", bevel=0.003, segs=1)
# Walls: three boards a side, ends and sides.
h = (Y1 - Y0 - 0.04) / 3
for k in range(3):
    y = Y0 + 0.04 + h * (k + 0.5)
    for sx in (-1, 1):
        c.box((0.022, h - 0.008, D - 0.04), (sx * (W / 2 - 0.011), y, 0.0), "Boards", bevel=0.003,
              segs=1)
    for sz in (-1, 1):
        c.box((W - 0.04, h - 0.008, 0.022), (0.0, y, sz * (D / 2 - 0.011)), "Boards", bevel=0.003,
              segs=1)
# Corner posts inside, battens outside across the boards.
for sx in (-1, 1):
    for sz in (-1, 1):
        # (Down 6 mm past the floor boards' undersides: flush faces z-fight.)
        c.box((0.05, Y1 - Y0 - 0.034, 0.05), (sx * (W / 2 - 0.047), (Y0 + Y1) / 2 + 0.017,
                                               sz * (D / 2 - 0.047)), "Boards", bevel=0.004, segs=1)
        # Steel corner plates top and bottom, with bolt heads.
        for y in (Y0 + 0.07, Y1 - 0.05):
            c.box((0.006, 0.1, 0.1), (sx * (W / 2 + 0.003), y, sz * (D / 2 - 0.05)), "Iron")
            c.box((0.1, 0.1, 0.006), (sx * (W / 2 - 0.05), y, sz * (D / 2 + 0.003)), "Iron")
            c.cyl(0.008, 0.0, 0.006, "Iron", segs=6,
                  xf=T(sx * (W / 2 + 0.006), y, sz * (D / 2 - 0.05)) @ RY(math.pi / 2))
for sx in (-1, 1):
    c.box((0.02, Y1 - Y0, 0.08), (sx * (W / 2 + 0.01), (Y0 + Y1) / 2, 0.0), "Boards", bevel=0.004,
          segs=1)
# A top rim.
for sx in (-1, 1):
    c.box((0.04, 0.025, D), (sx * (W / 2 - 0.02), Y1 + 0.0125, 0.0), "Boards", bevel=0.004, segs=1)
for sz in (-1, 1):
    c.box((W - 0.08, 0.025, 0.04), (0.0, Y1 + 0.0125, sz * (D / 2 - 0.02)), "Boards", bevel=0.004,
          segs=1)
# Rope handles on the ends, through wooden cleats.
for sz in (-1, 1):
    z = sz * (D / 2 + 0.03)
    for x in (-0.15, 0.15):
        c.box((0.05, 0.05, 0.035), (x, 0.08, sz * (D / 2 + 0.017)), "Boards", bevel=0.005, segs=1)
    path = [(-0.17, 0.08), (-0.12, 0.03), (-0.05, 0.005), (0.05, 0.005), (0.12, 0.03), (0.17, 0.08)]
    c.sweep([(x, y) for x, y in path],
            [(0.012 * math.cos(a), 0.012 * math.sin(a)) for a in
             [2 * math.pi * i / 6 for i in range(6)]], "Rope", xf=T(0, 0, z) @ RY(math.pi / 2))
# Bolted down: two angle brackets to the bed at each side.
for sx in (-1, 1):
    for z in (-0.4, 0.4):
        c.box((0.06, 0.006, 0.08), (sx * (W / 2 + 0.03), Y0 + 0.003, z), "Iron")
        c.box((0.006, 0.06, 0.08), (sx * (W / 2 + 0.003), Y0 + 0.03, z), "Iron")
# The unit's number stencilled on the aisle side, either side of the
# batten (raised 5 mm: flush paint z-fights the boards).
for z, label in ((0.3, "ВЧ"), (-0.3, "41812")):
    text(c, label, 0.05, "Enamel", T(W / 2, 0.02, z) @ RY(math.pi / 2), depth=0.005)
make(c)
report()
export("assets/models/vehicles/stash_crate.glb")
