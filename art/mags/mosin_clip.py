"""Mosin stripper clip: a stamped steel channel that holds five 7.62x54R
rounds by their rims. Writes assets/models/mags/mosin_clip.glb.

Item space: origin at the centre of the item box (0.012 x 0.012 x 0.055 m,
see assets/data/items.ron); the channel runs along Z and opens upward.
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, export, fillet, join, main_guard, report

main_guard()

L = 0.0275
T = 0.0005  # sheet thickness
clip = Part("clip")
# Channel cross-section (x, y): a base with walls curled inward at the top.
outer = [(-0.0058, -0.0055), (0.0058, -0.0055), (0.0058, 0.0035), (0.0035, 0.0058),
         (0.0026, 0.0052), (0.0052 - T, 0.0032), (0.0052 - T, -0.0055 + T),
         (-0.0052 + T, -0.0055 + T), (-0.0052 + T, 0.0032), (-0.0026, 0.0052), (-0.0035, 0.0058),
         (-0.0058, 0.0035)]
clip.prism(fillet(outer, [0.001, 0.001, 0.002, 0.0, 0.0, 0.001, 0.0005, 0.0005, 0.001, 0.0, 0.0,
                          0.002], 2), -L, L, "Steel", axis="z", bevel=0.0002, segs=1)
# Stamped stiffening rib along the base.
clip.box((0.003, 0.0008, 0.044), (0, -0.0059, 0), "Steel", bevel=0.0003, segs=1)
# Leaf spring inside, and the end stops.
clip.box((0.0085, 0.0005, 0.048), (0, -0.0045, 0), "Blued", bevel=0.0002, segs=1)
for sz in (-1, 1):
    clip.box((0.0108, 0.0035, 0.0008), (0, -0.0035, sz * (L - 0.0004)), "Steel", bevel=0.0002,
             segs=1)

join("static_clip", clip)
report()
export("assets/models/mags/mosin_clip.glb")
