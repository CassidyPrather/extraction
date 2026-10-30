"""Sallow Reservoir's stock (src/world/dam.rs): what the Custodian has
collected and keeps in rows on the drained bed, tagged, pending review.

- `kept_bus`      a single-deck bus as `buildings::wrecked_bus` blocks it
                  out: 2.5 x 10 m, origin on the ground under its middle,
                  nose to -Z, one corner on a block where a wheel's gone.
- `kept_bicycle`  a roadster leaning in a rack, along +X, origin on the
                  ground under its middle.
- `wheelie_bin`   a 240-litre bin, lid shut, handle and wheels to -X,
                  origin on the ground under its middle.

Twelve years in the open: paint faded and streaked, rust at the seams,
tyres flat. Each carries the Custodian's tag.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _kit  # noqa: E402
from _kit import ALONG_X, UP, Part, RY, RZ, T, Wear, fillet, prop  # noqa: E402

_kit.AO_SKY = True

# The bus, as `wrecked_bus` measures it.
BUS_W, BUS_L = 2.5, 10.0
WHEEL_R = 0.48
BODY_LO = WHEEL_R + 0.15
BAND = BODY_LO + 1.05


def tag(p, centre, normal_axis="z", sign=-1.0):
    """The Custodian's tag: a white card with a black stripe, 4 mm proud."""
    x, y, z = centre
    if normal_axis == "z":
        p.box((0.22, 0.14, 0.004), (x, y, z + sign * 0.004), "Paper")
        p.box((0.18, 0.02, 0.004), (x, y + 0.03, z + sign * 0.008), "Black")
    else:
        p.box((0.004, 0.14, 0.22), (x + sign * 0.004, y, z), "Paper")
        p.box((0.004, 0.02, 0.18), (x + sign * 0.008, y + 0.03, z), "Black")


def kept_bus():
    p = Part("static")
    w = Wear(41)
    hw, hl = BUS_W * 0.5, BUS_L * 0.5
    # Lower body: the skirt and panels below the windows, rounded at the
    # corners in plan.
    plan = fillet([(-hw, -hl), (hw, -hl), (hw, hl), (-hw, hl)], 0.25, 3)
    p.prism([(x, z) for x, z in plan], BODY_LO, BAND, "Painted", axis="y", bevel=0.02, segs=1)
    # A cream band along the waist.
    p.box((BUS_W + 0.012, 0.18, BUS_L - 0.5), (0.0, BAND - 0.2, 0.0), "Cream")
    # The window band: dark glass set in, pillars between.
    p.box((BUS_W - 0.08, 0.86, BUS_L - 0.35), (0.0, BAND + 0.45, 0.0), "Glass")
    for k in range(7):
        z = -hl + 0.2 + k * (BUS_L - 0.4) / 6.0
        p.box((BUS_W, 0.9, 0.12), (0.0, BAND + 0.45, z), "Painted", bevel=0.01, segs=1)
    # The windscreen pillars and the destination board over it.
    p.box((BUS_W - 0.3, 0.22, 0.02), (0.0, BAND + 0.78, -hl - 0.005), "Black")
    p.box((1.4, 0.12, 0.004), (0.0, BAND + 0.78, -hl - 0.017), "Cream")
    # Roof, a little proud all round, its edge rounded.
    p.box((BUS_W + 0.04, 0.16, BUS_L + 0.04), (0.0, BAND + 0.98, 0.0), "Painted",
          bevel=0.06, segs=2)
    # Bumpers, headlamps, the grille.
    for sgn in (-1.0, 1.0):
        p.box((BUS_W + 0.06, 0.22, 0.14), (0.0, BODY_LO + 0.15, sgn * (hl + 0.05)), "Iron",
              bevel=0.02, segs=1)
    for sx in (-1.0, 1.0):
        p.cyl(0.11, -hl - 0.03, -hl + 0.01, "Glass", segs=14,
              xf=T(sx * (hw - 0.35), BODY_LO + 0.45, 0.0))
    p.box((1.0, 0.35, 0.02), (0.0, BODY_LO + 0.5, -hl - 0.01), "Iron")
    # The folding door on the right at the front.
    p.box((0.03, 1.85, 0.95), (hw + 0.012, BODY_LO + 0.95, -hl + 1.0), "Iron")
    p.box((0.02, 0.8, 0.4), (hw + 0.03, BODY_LO + 1.4, -hl + 0.78), "Glass")
    # Rust streaks under the windows.
    for k in range(6):
        z = -hl + 1.0 + k * 1.5 + w.j(0.3)
        for sx in (-1.0, 1.0):
            p.box((0.004, 0.45 + w.j(0.05), 0.08), (sx * (hw + 0.004), BAND - 0.65, z), "Rusty")
    # Wheels: three corners on flat tyres, the fourth on a concrete block.
    for i, (sx, sz) in enumerate([(-1, -1), (1, -1), (-1, 1), (1, 1)]):
        x, z = sx * (hw - 0.25), sz * (hl - 1.9)
        if i == 3:
            p.box((0.4, BODY_LO, 0.5), (x, BODY_LO * 0.5, z), "Concrete", bevel=0.02, segs=1)
            continue
        # A flat tyre: squashed where it meets the ground.
        p.cyl(WHEEL_R, -0.16, 0.16, "Rubber", segs=20,
              xf=T(x, WHEEL_R, z) @ ALONG_X)
        p.cyl(WHEEL_R * 0.55, -0.17, 0.17, "Iron", segs=14,
              xf=T(x, WHEEL_R, z) @ ALONG_X)
        # Wheel arch.
        p.box((0.03, 0.12, 1.25), (sx * (hw + 0.006), BODY_LO + 0.05, z), "Black")
    # The tag behind the windscreen.
    tag(p, (0.6, BAND + 0.3, -hl + 0.05), "z", 1.0)
    return [p]


def kept_bicycle():
    """A roadster: two wheels, a diamond frame, saddle and upright bars."""
    p = Part("static")
    r = 0.34
    front, rear = (0.5, r, 0.0), (-0.5, r, 0.0)
    for x in (front[0], rear[0]):
        # Tyre, rim, hub.
        p.tube(r, r - 0.035, -0.018, 0.018, "Rubber", segs=24, xf=T(x, r, 0.0))
        p.tube(r - 0.035, r - 0.05, -0.012, 0.012, "Zinc", segs=24, xf=T(x, r, 0.0))
        p.cyl(0.03, -0.04, 0.04, "Iron", segs=10, xf=T(x, r, 0.0))
    # The frame as rods: seat tube, top tube, down tube, stays, fork.
    bb = (-0.05, 0.27)
    seat = (-0.15, 0.82)
    head = (0.38, 0.8)

    def rod(a, b, rr=0.014, mat="Painted"):
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy)
        ang = math.atan2(dy, dx)
        p.cyl(rr, -length / 2, length / 2, mat, segs=8,
              xf=T((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, 0.0) @ RZ(ang) @ ALONG_X)

    rod(bb, seat)
    rod(seat, head)
    rod(bb, head, 0.016)
    rod(bb, (rear[0], rear[1]), 0.01)
    rod(seat, (rear[0], rear[1]), 0.009)
    rod(head, (front[0], front[1]), 0.012, "Zinc")
    # Saddle, bars, a chain guard, a basket's frame at the front.
    p.box((0.24, 0.06, 0.12), (seat[0] - 0.02, seat[1] + 0.09, 0.0), "Leather",
          bevel=0.02, segs=1)
    rod(seat, (seat[0] - 0.02, seat[1] + 0.06), 0.011, "Zinc")
    # The stem up to the bar (it stopped 5 cm short: bar and grips in the
    # air).
    rod(head, (head[0] - 0.08, head[1] + 0.2), 0.012, "Zinc")
    p.cyl(0.011, -0.27, 0.27, "Zinc", segs=8, xf=T(head[0] - 0.08, head[1] + 0.2, 0.0))
    for z in (-0.27, 0.27):
        p.cyl(0.016, z - 0.05, z + 0.05, "Rubber", segs=8,
              xf=T(head[0] - 0.08, head[1] + 0.2, 0.0))
    # The chain guard on the stay (it stood 2.4 cm off it).
    p.box((0.4, 0.07, 0.012), (-0.22, 0.31, 0.015), "Painted")
    # The tag on the seat tube (it hung in the air under the saddle).
    tag(p, (-0.11, 0.6, 0.011), "z", 1.0)
    return [p]


def wheelie_bin():
    """240 litres, 0.58 x 1.0 x 0.66: a tapered body, the lid and its hinge
    rail at -X with the handle, two wheels below it."""
    p = Part("static")
    lo, hi = 0.06, 0.95
    rings = []
    for y, s in [(lo, 0.9), (hi - 0.04, 1.0), (hi, 1.0)]:
        hw, hd = 0.29 * s, 0.33 * s
        rings.append([(-hw, y, -hd), (hw, y, -hd), (hw, y, hd), (-hw, y, hd)])
    p.loft(rings, "Olive")
    # Ribs down the front and back.
    for z in (-0.335, 0.335):
        for x in (-0.12, 0.12):
            p.box((0.04, 0.7, 0.012), (x, 0.5, z), "Olive")
    # Lid, a little over the rim, and the hinge rail and handle at -X.
    p.box((0.62, 0.05, 0.7), (-0.02, hi + 0.025, 0.0), "Olive", bevel=0.012, segs=1)
    p.cyl(0.02, -0.3, 0.3, "Black", segs=10, xf=T(-0.32, hi - 0.02, 0.0))
    p.box((0.06, 0.06, 0.5), (-0.34, hi + 0.01, 0.0), "Black", bevel=0.01, segs=1)
    # Wheels on their axle.
    for z in (-0.26, 0.26):
        p.cyl(0.1, z - 0.03, z + 0.03, "Rubber", segs=14, xf=T(-0.24, 0.1, 0.0))
    p.cyl(0.012, -0.29, 0.29, "Iron", segs=8, xf=T(-0.24, 0.1, 0.0))
    # A house number, and the tag on the lid.
    p.box((0.12, 0.08, 0.004), (0.12, 0.75, -0.336), "Cream")
    p.box((0.22, 0.004, 0.14), (0.05, hi + 0.054, 0.0), "Paper")
    p.box((0.18, 0.004, 0.02), (0.05, hi + 0.058, 0.03), "Black")
    return [p]


prop("kept_bus", kept_bus)
prop("kept_bicycle", kept_bicycle)
prop("wheelie_bin", wheelie_bin)
