"""SVD magazine: 10-round 7.62x54R, stamped steel with pressed ribs.
Writes assets/models/mags/svd_mag.glb.

Item space: origin at the centre of the item box (0.03 x 0.124 x 0.10 m),
+Y up (feed lips on top), -Z forward. The box is a little slimmer than
the rimmed round is long plus the curve, so the top's middle (the lips)
is at z +0.004, and the gun's `magwell` (svd.glb) is 4 mm ahead of where
the lips seat. The top face is level: no tilt at the seat.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RY, T, catmull, export, fillet, join, main_guard, make, report

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from ammo import C762, cartridge  # noqa: E402

main_guard()

TOP = 0.062
HW = 0.0122             # body half width
FRONT = [(-0.0375, TOP - 0.004), (-0.0385, 0.02), (-0.041, -0.02), (-0.0455, -0.055)]
REAR = [(0.046, TOP - 0.006), (0.0455, 0.02), (0.0425, -0.02), (0.038, -0.055)]
spine = catmull(REAR, 4, closed=False)
belly = catmull(list(reversed(FRONT)), 4, closed=False)
outline = [(-0.0375, TOP - 0.0015), (0.0445, TOP - 0.0015)] + spine + belly
outline = fillet(outline, [0.0015, 0.003] + [0.0] * (len(spine) - 1) + [0.003]
                 + [0.003] + [0.0] * (len(belly) - 2) + [0.0015], 3)


def edge_z(pts, y):
    """z of the front or back edge at height y."""
    for (za, ya), (zb, yb) in zip(pts, pts[1:]):
        if min(ya, yb) <= y <= max(ya, yb):
            return za + (y - ya) / (yb - ya) * (zb - za)
    return pts[-1][0]


body = Part("body")
body.prism(outline, -HW, HW, "Blued", axis="x", bevel=0.0015, segs=2)
# Front locking lug, the rear catch lug.
body.prism(fillet([(-0.0448, TOP - 0.011), (-0.036, TOP - 0.013), (-0.036, TOP - 0.0015),
                   (-0.0435, TOP - 0.0035)], 0.0012, 2), -0.0065, 0.0065, "Blued", axis="x",
           bevel=0.0006, segs=1)
body.box((0.012, 0.007, 0.006), (0, TOP - 0.024, 0.0465), "Blued", bevel=0.0012, segs=1)
# Steel feed lips curled in over the rounds; the follower under them.
for sx in (-1, 1):
    lip = fillet([(sx * 0.0044, TOP - 0.0055), (sx * 0.0082, TOP - 0.0055), (sx * 0.0082, TOP),
                  (sx * 0.0062, TOP), (sx * 0.0044, TOP - 0.0022)], 0.0007, 2)
    body.prism(lip, -0.016, 0.042, "Blued", axis="z")
# (Its own node, uncut by the mouth: seen between the lips once empty.)
follower = Part("static_follower")
follower.box((0.0088, 0.006, 0.05), (0.0, TOP - 0.0135, 0.012), "Black", bevel=0.0008, segs=1)
# Floor plate, slid on from the front, its catch at the back.
fp = fillet([(-0.049, -0.062), (0.041, -0.062), (0.0415, -0.0545), (-0.048, -0.0545)], 0.0015, 2)
body.prism(fp, -HW - 0.0012, HW + 0.0012, "Blued", axis="x", bevel=0.0012, segs=1)
body.box((0.008, 0.006, 0.004), (0, -0.0583, 0.0425), "Blued", bevel=0.001, segs=1)

cut = Part("cut")
# The mouth between the lips, down to the follower.
cut.box((0.0088, 0.03, 0.058), (0.0, TOP - 0.0135 + 0.0155, 0.013), "Blued")
# The pressed ribs: the panels between them sunk 0.8 mm, both sides.
RIBS = [-0.040, -0.020, 0.0, 0.020, 0.040]
for sx in (-1, 1):
    for y0, y1 in zip(RIBS, RIBS[1:]):
        a, b = y0 + 0.0022, y1 - 0.0022
        quad = [(edge_z(FRONT, a) + 0.007, a), (edge_z(REAR, a) - 0.007, a),
                (edge_z(REAR, b) - 0.007, b), (edge_z(FRONT, b) + 0.007, b)]
        lo = HW - 0.0008 if sx > 0 else -HW - 0.01
        cut.prism(fillet(quad, 0.0025, 3), lo, lo + 0.0108, "Blued", axis="x")
body.cut(cut)

# The top two rounds, staggered, bullets forward, rims at the back. The game
# shows round N only while the magazine holds N or more.
for n, (x, y) in enumerate(((0.0043, TOP - 0.0042), (-0.0043, TOP - 0.0098)), 1):
    rnd = Part(f"round_{n}")
    cartridge(rnd, C762, T(x, y, 0.0435), segs=12, base=False)
    make(rnd, worn=None)

join("static_mag", body, worn={})
make(follower, worn={})
report()
export("assets/models/mags/svd_mag.glb")
