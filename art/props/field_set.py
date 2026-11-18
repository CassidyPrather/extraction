"""The field set (#94, `raid::transceiver`): a fixed radio bolted to a
wall at each way out of a valley, to call the truck from. Writes
assets/models/props/field_set.glb.

Prop space (see _kit.py): metres, +Y up, its front to -Z, its origin at the
case's middle (the set's collider, 0.32 x 0.26 x 0.14). The case is bolted
to a board painted pale with a red band, held off the wall on four
standoffs (the wall at +Z 0.07 + OFF): proud of any ivy on the wall, and
something to see from the pad. A grey-green steel case on two flanges, a
rain hood over it; on its face a red call button, big enough to find in
the dark, its lamp and the frequency's meter beside it; the handset in
its cradle on the side; the cable up the wall to the roof's aerial.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import AIRBORNE, ALONG_X, bolt, prop, rod  # noqa: E402
from common import Part, T  # noqa: E402

W, H, D = 0.32, 0.26, 0.14
FRONT = -D / 2
# The board: its size, its thickness, how far the set stands off the wall
# behind the case's back (`raid::transceiver::OFF_WALL`).
BOARD_W, BOARD_H, BOARD_T = 0.56, 0.5, 0.03
OFF = 0.13


def field_set():
    p = Part("static")
    # The board on its standoffs, its red band along the top.
    back = D / 2
    p.box((BOARD_W, BOARD_H, BOARD_T), (0.0, 0.03, back + BOARD_T / 2), "Pale", bevel=0.004,
          segs=1)
    p.box((BOARD_W + 0.002, 0.06, BOARD_T + 0.002), (0.0, 0.03 + BOARD_H / 2 - 0.03,
          back + BOARD_T / 2), "Red", bevel=0.003, segs=1)
    for x in (-BOARD_W / 2 + 0.05, BOARD_W / 2 - 0.05):
        for y in (0.03 - BOARD_H / 2 + 0.05, 0.03 + BOARD_H / 2 - 0.05):
            p.cyl(0.018, back + BOARD_T, back + OFF, "Steel", segs=10,
                  xf=T(x, y, 0.0))
            bolt(p, (x, y, back - 0.001), (0, 0, -1), r=0.01, mat="Iron")
    # The case, and its two flanges bolted to the board.
    p.box((W, H, D - 0.006), (0.0, 0.0, 0.003), "GreyGreen", bevel=0.008, segs=1)
    for x in (-W / 2 - 0.025, W / 2 + 0.025):
        p.box((0.05, 0.2, 0.006), (x, 0.0, D / 2 - 0.003), "GreyGreen", bevel=0.002, segs=1)
        for y in (-0.07, 0.07):
            bolt(p, (x, y, D / 2 - 0.006), (0, 0, -1), r=0.009, mat="Iron")
    # The rain hood, sloped off the wall.
    p.box((W + 0.03, 0.008, D + 0.03), (0.0, H / 2 + 0.012, -0.012), "Painted",
          bevel=0.002, segs=1, xf=None)
    # The face: a recessed panel, the call button in its guard ring, its
    # lamp, the meter.
    p.box((W - 0.04, H - 0.04, 0.006), (0.0, 0.0, FRONT - 0.001), "Dark", bevel=0.003, segs=1)
    bx, by = -0.05, -0.03
    p.cyl(0.042, FRONT - 0.012, FRONT, "Steel", segs=24, xf=T(bx, by, 0.0))
    p.cyl(0.032, FRONT - 0.03, FRONT - 0.012, "Red", segs=24, chamfer=0.006,
          xf=T(bx, by, 0.0))
    p.cyl(0.014, FRONT - 0.014, FRONT, "LampRed", segs=14, xf=T(bx, 0.075, 0.0))
    p.cyl(0.04, FRONT - 0.006, FRONT, "Cream", segs=20, xf=T(0.07, 0.04, 0.0))
    rod(p, (0.07, 0.04, FRONT - 0.008), (0.095, 0.065, FRONT - 0.008), 0.002, "Black", segs=3)
    p.cyl(0.016, FRONT - 0.02, FRONT, "Bakelite", segs=12, xf=T(0.07, -0.06, 0.0))
    # The handset in its cradle on the right side.
    sx = W / 2
    p.box((0.02, 0.05, 0.08), (sx + 0.01, 0.06, -0.01), "Black", bevel=0.003, segs=1)
    p.box((0.03, 0.05, 0.08), (sx + 0.01, -0.07, -0.01), "Black", bevel=0.003, segs=1)
    rod(p, (sx + 0.035, -0.09, -0.01), (sx + 0.035, 0.09, -0.01), 0.013, "Bakelite", segs=10)
    for y in (-0.1, 0.1):
        p.cyl(0.026, -0.015, 0.015, "Bakelite", segs=16, xf=T(sx + 0.035, y, -0.01) @ ALONG_X)
    rod(p, (sx + 0.035, -0.115, -0.01), (sx + 0.01, -0.14, 0.02), 0.005, "Black", segs=5)
    rod(p, (sx + 0.01, -0.14, 0.02), (sx - 0.02, -H / 2, 0.02), 0.005, "Black", segs=5)
    # The cable out of the top, up the wall to the roof.
    wall = back + OFF
    rod(p, (-0.12, H / 2, back - 0.02), (-0.12, H / 2 + 0.06, back - 0.02), 0.011, "Black",
        segs=6)
    rod(p, (-0.12, H / 2 + 0.06, back - 0.02), (-0.12, 0.03 + BOARD_H / 2 + 0.05, wall - 0.015),
        0.011, "Black", segs=6)
    rod(p, (-0.12, 0.03 + BOARD_H / 2 + 0.05, wall - 0.015), (-0.12, H / 2 + 0.6, wall - 0.015),
        0.011, "Black", segs=6)
    for y in (H / 2 + 0.4, H / 2 + 0.55):
        p.box((0.03, 0.012, 0.026), (-0.12, y, wall - 0.013), "Steel")
    return [p]


AIRBORNE.add("field_set")
prop("field_set", field_set)
