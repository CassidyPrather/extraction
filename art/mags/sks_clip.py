"""SKS stripper clip: a stamped steel channel holding ten 7.62x39 rounds
by their rims, bullets up, the rounds alternately nudged aside as they
sit. Writes assets/models/mags/sks_clip.glb.

Item space: origin at the centre of the item box (0.014 x 0.06 x 0.085 m,
see assets/data/items.ron), the box fitted to a full clip; the channel runs
along Z at the bottom of the box and opens upward.

- `static_clip`  the channel, its spring and end stops
- `round_1`..`round_10`  the rounds, front to back; the game shows round N
  while the clip holds N or more
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import RX, T, Part, export, fillet, join, main_guard, make, report
from ammo import C762X39, cartridge  # noqa: E402
from _kit import transform  # noqa: E402

main_guard()

L = 0.0415
TK = 0.0005  # sheet thickness
W = 0.0066   # half width outside: the rims (11.3 mm) slide along inside
clip = Part("clip")
# Channel cross-section (x, y): a base with walls curled inward at the top.
outer = [(-W, -0.0055), (W, -0.0055), (W, 0.0035), (W - 0.0024, 0.0060),
         (W - 0.0033, 0.0054), (W - TK, 0.0032), (W - TK, -0.0055 + TK),
         (-W + TK, -0.0055 + TK), (-W + TK, 0.0032), (-W + 0.0033, 0.0054), (-W + 0.0024, 0.0060),
         (-W, 0.0035)]
clip.prism(fillet(outer, [0.001, 0.001, 0.002, 0.0, 0.0, 0.001, 0.0005, 0.0005, 0.001, 0.0, 0.0,
                          0.002], 2), -L, L, "Steel", axis="z", bevel=0.0002, segs=1)
# Stamped stiffening rib along the base.
clip.box((0.003, 0.0008, 0.072), (0, -0.0059, 0), "Steel", bevel=0.0003, segs=1)
# Leaf spring inside, and the end stops.
clip.box((0.003, 0.0005, 0.074), (0, -0.0045, 0), "Blued", bevel=0.0002, segs=1)
for sz in (-1, 1):
    clip.box((2 * W - 0.0004, 0.0035, 0.0008), (0, -0.0035, sz * (L - 0.0004)), "Steel",
             bevel=0.0002, segs=1)

# Ten rounds standing in it, rims overlapping a little, alternate ones
# nudged aside the way they sit in a real clip.
rounds = []
for n in range(10):
    r = Part(f"round_{n + 1}")
    z = -0.0369 + n * 0.0082
    cartridge(r, C762X39, T(0.0004 * (1 if n % 2 else -1), -0.0040, z) @ RX(math.pi / 2), segs=10)
    rounds.append(r)

# The whole load sits on the floor of its box.
transform([clip] + rounds, T(0.0, -0.0237, 0.0))
for r in rounds:
    make(r)
join("static_clip", clip)
report()
export("assets/models/mags/sks_clip.glb")
