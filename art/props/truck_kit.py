"""Truck upgrades (`TruckUpgradeProp` in src/render/upgrades.rs). Writes
assets/models/props/<name>.glb, one node `static` each, in truck space
axes (x across, y up, -Z forward) about the origin noted:

- `stash_rack`    a timber rack over the stash crate; origin at the crate's
                  centre (truck (-0.55, 1.75, -1.0)): posts bolted to the
                  crate's corners up to y +0.6, a slatted shelf at +0.45,
                  a cross brace on the -X side
- `stash_rack_2`  a shelf 1.0 along Z by 0.3 deep hung on the left canvas
                  wall; origin at the shelf's centre (truck (-0.95, 1.55,
                  -1.0)), the wall at x -0.15
- `bench_vice`    an engineer's vice; origin on the bench top under its
                  body (truck (0.72, 1.9, -0.45)), jaws toward -Z
- `aerial`        a whip aerial; origin at its base on the cab roof (truck
                  (0.95, 2.6, -2.1)), coax back over the roof's rear edge
                  (z +0.2) and down
- `med_cabinet`   a wall cabinet 0.40 x 0.48 x 0.14; origin at its centre
                  (truck (0.55, 2.05, -1.74)), its back on the wall, door +Z

Materials: Planks, Timber, Iron, Painted, Steel, Rubber, Enamel, Red, Zinc.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, Wear, angle_iron, board, bolt, prop, rod  # noqa: E402
from common import Part, T  # noqa: E402


def stash_rack():
    w = Wear(91)
    p = Part("static")
    # The stash crate is 0.95 x 1.25 outside, its rim's top at y +0.325.
    hx, hz = 0.475, 0.625
    y0, y1 = 0.0, 0.6
    for sx in (-1, 1):
        for sz in (-1, 1):
            x, z = sx * (hx + 0.02), sz * (hz + 0.02)
            board(p, (0.04, y1 - y0, 0.04), (x, (y0 + y1) / 2, z), "Planks", bevel=0.003)
            # An angle bracket bolting it to the crate's corner.
            p.box((0.006, 0.12, 0.05), (x - sx * 0.023, 0.06, z), "Iron")
            bolt(p, (x + sx * 0.02, 0.03, z), (sx, 0, 0), r=0.007, h=0.004)
            bolt(p, (x + sx * 0.02, 0.09, z), (sx, 0, 0), r=0.007, h=0.004)
    # Rails under the shelf, the slats across them.
    for sx in (-1, 1):
        board(p, (0.025, 0.05, 2 * hz + 0.06), (sx * (hx + 0.005), 0.42, 0.0), "Planks",
              bevel=0.003)
    for k in range(6):
        z = -hz + 0.1 + k * (2 * hz - 0.2) / 5
        board(p, (2 * hx + 0.1, 0.02, 0.1), (0.0, 0.455, z), "Planks", w, bevel=0.003)
    # The brace on the -X side, corner to corner, and a top rail round.
    L = math.hypot(2 * hz + 0.04, y1 - 0.1)
    board(p, (0.02, L, 0.05), (-(hx + 0.045), (y1 + 0.1) / 2, 0.0), "Planks",
          tilt=math.atan2(2 * hz + 0.04, y1 - 0.1), bevel=0.003)
    for sz in (-1, 1):
        board(p, (2 * hx + 0.08, 0.04, 0.025), (0.0, y1 - 0.025, sz * (hz + 0.045)), "Planks",
              bevel=0.003)
    return [p]


def stash_rack_2():
    p = Part("static")
    # A board on two angle brackets screwed to a batten on the wall.
    board(p, (0.3, 0.025, 1.0), (0.0, 0.0, 0.0), "Planks", bevel=0.003)
    board(p, (0.03, 0.08, 1.0), (-0.13, -0.05, 0.0), "Planks", bevel=0.003)
    for z in (-0.38, 0.38):
        p.box((0.2, 0.005, 0.03), (-0.04, -0.015, z), "Iron")
        p.box((0.005, 0.2, 0.03), (-0.1475, -0.1125, z), "Iron")
        rod(p, (-0.14, -0.0175, z), (0.05, -0.2, z), 0.005, "Iron", segs=5)
        bolt(p, (-0.04, -0.0175, z), (0, -1, 0), r=0.006, h=0.003)
    # A lip along the front, so things don't roll off on the road.
    board(p, (0.02, 0.05, 1.0), (0.135, 0.0365, 0.0), "Planks", bevel=0.003)
    return [p]


def bench_vice():
    p = Part("static")
    # A swivel base on the bench, the body, two jaws with steel faces, the
    # screw and its tommy bar, all in worn green paint over iron.
    p.cyl(0.07, 0.0, 0.02, "Iron", segs=16, xf=UP, chamfer=0.004)
    p.box((0.12, 0.07, 0.2), (0.0, 0.055, 0.0), "Painted", bevel=0.008, segs=1)
    p.box((0.2, 0.09, 0.035), (0.0, 0.135, -0.09), "Painted", bevel=0.006, segs=1)
    p.box((0.2, 0.09, 0.035), (0.0, 0.135, -0.17), "Painted", bevel=0.006, segs=1)
    p.box((0.08, 0.05, 0.14), (0.0, 0.09, -0.15), "Painted", bevel=0.006, segs=1)
    for z in (-0.1095, -0.1505):
        p.box((0.19, 0.04, 0.004), (0.0, 0.16, z), "Steel")
    rod(p, (0.0, 0.11, -0.04), (0.0, 0.11, -0.27), 0.01, "Steel", segs=8)
    rod(p, (-0.11, 0.11, -0.26), (0.11, 0.11, -0.26), 0.007, "Steel", segs=6)
    for sx in (-1, 1):
        p.sphere(0.012, (sx * 0.115, 0.11, -0.26), "Steel", segs=8, rings=5)
    return [p]


def aerial():
    p = Part("static")
    p.cyl(0.04, 0.0, 0.03, "Rubber", segs=12, xf=UP, chamfer=0.004)
    p.cyl(0.02, 0.03, 0.06, "Zinc", segs=10, xf=UP)
    # The spring, a coil of short rods.
    pts = [(0.012 * math.cos(t * 0.6), 0.06 + t * 0.006, 0.012 * math.sin(t * 0.6))
           for t in range(15)]
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, 0.003, "Steel", segs=4)
    # A whip tapering 8 mm to 3 mm over 2.4 m.
    p.lathe([(0.0, 0.14), (0.004, 0.14), (0.0015, 2.54), (0.0, 2.545)], "Steel", segs=6, xf=UP)
    p.cyl(0.006, 0.14, 0.16, "Zinc", segs=8, xf=UP)
    # The coax: off the base, back to the roof's edge and over it.
    path = [(0.03, 0.01, 0.0), (0.05, 0.004, 0.08), (0.04, 0.004, 0.19), (0.04, -0.02, 0.22),
            (0.04, -0.3, 0.23)]
    for a, b in zip(path, path[1:]):
        rod(p, a, b, 0.004, "Rubber", segs=5)
    return [p]


def med_cabinet():
    p = Part("static")
    W, H, D = 0.40, 0.48, 0.14
    p.box((W, H, D - 0.02), (0.0, 0.0, -0.01), "Enamel", bevel=0.006, segs=1)
    p.box((W - 0.01, H - 0.01, 0.016), (0.0, 0.0, D / 2 - 0.012), "Enamel", bevel=0.004,
          segs=1)
    # A red cross on the door, 4 mm proud.
    dz = D / 2 - 0.004 + 0.002
    p.box((0.16, 0.05, 0.004), (0.0, 0.03, dz), "Red")
    p.box((0.05, 0.16 - 0.002, 0.004), (0.0, 0.03, dz + 0.004), "Red")
    # A latch and two hinges.
    p.box((0.02, 0.06, 0.012), (W / 2 - 0.03, -0.05, dz + 0.002), "Zinc", bevel=0.002, segs=1)
    for y in (-0.16, 0.16):
        rod(p, (-W / 2 - 0.004, y - 0.03, D / 2 - 0.01), (-W / 2 - 0.004, y + 0.03, D / 2 - 0.01),
            0.006, "Zinc", segs=6)
    return [p]


prop("stash_rack", stash_rack)
prop("stash_rack_2", stash_rack_2)
prop("bench_vice", bench_vice)
prop("aerial", aerial)
prop("med_cabinet", med_cabinet)
