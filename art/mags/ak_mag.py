"""AK-74 magazine: 30-round 5.45x39, plum polymer. Writes
assets/models/mags/ak_mag.glb.

Item space: origin at the centre of the item box (0.026 x 0.2 x 0.09 m,
see assets/data/items.ron), +Y up (feed lips on top), -Z forward. The
banana curve runs forward toward the bottom; the top face is level, so the
gun's model seat (`magwell` in ak74.glb) needs no tilt.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, catmull, export, fillet, join, main_guard, make, report

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from ammo import C545, cartridge  # noqa: E402
from _kit import text  # noqa: E402

main_guard()

TOP = 0.1
rear = [(0.044, TOP - 0.006), (0.041, 0.05), (0.031, -0.01), (0.015, -0.064), (0.003, -0.094)]
front = [(-0.052, -0.090), (-0.042, -0.06), (-0.026, -0.005), (-0.017, 0.05), (-0.0155, TOP - 0.008)]
spine = catmull(rear, 4, closed=False)
belly = catmull(front, 4, closed=False)
outline = [(-0.0165, TOP - 0.002), (0.040, TOP - 0.002)] + spine + belly
outline = fillet(outline, [0.002, 0.004] + [0.0] * (len(spine) - 1) + [0.004]
                 + [0.004] + [0.0] * (len(belly) - 2) + [0.002], 3)

body = Part("body")
body.prism(outline, -0.0118, 0.0118, "Polymer", axis="x", bevel=0.0022, segs=2)
# Reinforcing ribs: along the spine, and the two raised bands near the top.
spine_rib = [(z + 0.0012, y) for z, y in spine]
rib = spine_rib + [(z - 0.008, y) for z, y in reversed(spine_rib)]
body.prism(rib, -0.0128, 0.0128, "Polymer", axis="x", bevel=0.0012, segs=1)
for y0 in (0.058, 0.074):
    band = fillet([(-0.018, y0), (0.042, y0), (0.042, y0 + 0.006), (-0.018, y0 + 0.006)], 0.0015, 2)
    body.prism(band, -0.0125, 0.0125, "Polymer", axis="x", bevel=0.0008, segs=1)
# Front locking lug and the rear catch lug.
body.prism(fillet([(-0.0215, TOP - 0.010), (-0.014, TOP - 0.012), (-0.014, TOP - 0.002),
                   (-0.0205, TOP - 0.0035)], 0.0012, 2), -0.006, 0.006, "Polymer", axis="x",
           bevel=0.0006, segs=1)
body.box((0.012, 0.006, 0.006), (0, TOP - 0.022, 0.045), "Polymer", bevel=0.0012, segs=1)
# Steel feed lips, curled in over the rounds, open between; the follower
# deep in the gap shows when it's empty.
for sx in (-1, 1):
    lip = fillet([(sx * 0.0042, TOP - 0.0055), (sx * 0.0078, TOP - 0.0055), (sx * 0.0078, TOP),
                  (sx * 0.0058, TOP), (sx * 0.0042, TOP - 0.0022)], 0.0007, 2)
    body.prism(lip, -0.010, 0.038, "Steel", axis="z")
# (Its own node, uncut by the mouth: seen between the lips once empty.)
follower = Part("static_follower")
follower.box((0.0084, 0.006, 0.042), (0.0, TOP - 0.0125, 0.013), "Black", bevel=0.0008, segs=1)
# Floor plate with its tab at the back.
fp = fillet([(-0.056, -0.096), (0.006, -0.1), (0.008, -0.094), (-0.054, -0.088)], 0.0015, 2)
body.prism(fp, -0.0128, 0.0128, "Polymer", axis="x", bevel=0.0012, segs=1)

# The mouth between the lips, down to the follower, and the moulding
# mark on the right side, 0.6 mm deep.
mouth = Part("cut")
mouth.box((0.0084, 0.03, 0.05), (0.0, TOP - 0.0125 + 0.015, 0.013), "Polymer")
side = T(0.0118 - 0.0006, 0, 0) @ RY(math.pi / 2)
text(mouth, "6Л23", 0.0052, "Polymer", side @ T(0.013, -0.062, 0.0), depth=0.0012)
body.cut(mouth)

# The top two rounds, staggered, bullets forward. The game shows round N
# only while the magazine holds N or more.
for n, (x, y) in enumerate(((0.0024, TOP - 0.0032), (-0.0024, TOP - 0.0072)), 1):
    rnd = Part(f"round_{n}")
    cartridge(rnd, C545, T(x, y, 0.0375), segs=12, base=False)
    make(rnd, worn=None)

join("static_mag", body, worn={})
make(follower, worn={})
report()
export("assets/models/mags/ak_mag.glb")
