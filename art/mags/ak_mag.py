"""AK-74 magazine: 30-round 5.45x39, plum polymer. Writes
assets/models/mags/ak_mag.glb.

Item space: origin at the centre of the item box (0.026 x 0.2 x 0.09 m,
see assets/data/items.ron), +Y up (feed lips on top), -Z forward. The
banana curve runs forward toward the bottom; the top face is level, so the
gun's model seat (`magwell` in ak74.glb) needs no tilt.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, T, catmull, export, fillet, join, main_guard, report

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
# Feed lips (steel) and the follower between them.
lips = fillet([(-0.0075, -0.012), (0.0075, -0.012), (0.0075, 0.038), (-0.0075, 0.038)],
              [0.003, 0.003, 0.002, 0.002], 3)
body.prism(lips, TOP - 0.004, TOP, "Steel", axis="y", bevel=0.0006, segs=1)
body.box((0.006, 0.004, 0.032), (0.0, TOP - 0.004, 0.012), "Black", bevel=0.0008, segs=1)
# Floor plate with its tab at the back.
fp = fillet([(-0.056, -0.096), (0.006, -0.1), (0.008, -0.094), (-0.054, -0.088)], 0.0015, 2)
body.prism(fp, -0.0128, 0.0128, "Polymer", axis="x", bevel=0.0012, segs=1)

join("static_mag", body)
report()
export("assets/models/mags/ak_mag.glb")
