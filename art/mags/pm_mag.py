"""PM magazine: 8-round single stack 9x18. Writes assets/models/mags/pm_mag.glb.

Item space: origin at the centre of the item box (0.016 x 0.1 x 0.03 m,
see assets/data/items.ron), +Y up (feed lips on top), -Z forward (bullet
noses point that way).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, fillet, join, main_guard, report

main_guard()

body = Part("static_mag")
plan = fillet([(-0.0062, -0.0145), (0.0062, -0.0145), (0.0062, 0.0142), (-0.0062, 0.0142)],
              [0.0055, 0.0055, 0.0018, 0.0018], 4)
# Profile is (x, z) for a prism along Y.
body.prism(plan, -0.046, 0.0435, "Blued", axis="y", bevel=0.0006, segs=1)
# Feed lips: the top narrows and steps down towards the front.
lips = fillet([(-0.0045, -0.0105), (0.0045, -0.0105), (0.0045, 0.0138), (-0.0045, 0.0138)],
              [0.003, 0.003, 0.0012, 0.0012], 3)
body.prism(lips, 0.043, 0.0495, "Blued", axis="y", bevel=0.0005, segs=1)
cut = Part("cut")
# Lip shape: cut the front of the lips down at an angle.
cut.box((0.02, 0.012, 0.02), (0, 0.0525, -0.012), "Blued",
        xf=RX(0.35, about=(0, 0.0495, -0.002)))
# Round-count holes on both sides.
for i in range(4):
    cut.cyl(0.0016, -0.01, 0.01, "Blued", segs=10, xf=T(0, -0.025 + i * 0.016, 0.004) @ RY(math.pi / 2))
body.cut(cut)

rest = Part("rest")
# Follower showing between the lips.
rest.box((0.0062, 0.003, 0.02), (0, 0.0435, 0.0005), "Black", bevel=0.0008, segs=1)
# Base plate with the heel lip the catch bites on.
base = fillet([(-0.0076, -0.0158), (0.0076, -0.0158), (0.0076, 0.0162), (-0.0076, 0.0162)],
              [0.006, 0.006, 0.0025, 0.0025], 4)
rest.prism(base, -0.05, -0.046, "Blued", axis="y", bevel=0.0009, segs=2)
rest.box((0.0085, 0.006, 0.0035), (0, -0.046, 0.0162), "Blued", bevel=0.0009, segs=1)

join("static_mag", body, rest)
report()
export("assets/models/mags/pm_mag.glb")
