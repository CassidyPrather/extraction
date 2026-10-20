"""Truck upgrades (`TruckUpgradeProp` in src/render/upgrades.rs). Writes
assets/models/props/<name>.glb, one node `static` each, in truck space
axes (x across, y up, -Z forward) about the origin noted:

- `stash_rack`    a timber rack over the wall-side 0.4 m of the stash crate
                  (x -0.45 to -0.05), the rest left open to reach into;
                  origin at the crate's centre (truck (-0.55, 1.75, -1.0)):
                  posts on the crate's rim, a slatted shelf at +0.465
- `stash_rack_2`  a shelf 1.0 along Z by 0.3 deep hung on the left canvas
                  wall; origin at the shelf's centre (truck (-0.95, 1.55,
                  -1.0)), the wall at x -0.15
- `bench_vice`    an engineer's vice; origin on the bench top under its
                  body (truck (0.72, 1.9, -0.45)), jaws toward -Z
- `aerial`        a whip aerial; origin at its base on the cab roof (truck
                  (0.95, 2.6, -2.1)), coax back over the roof's rear edge
                  (z +0.2) and down
- `med_cabinet`   a wall cabinet 0.40 x 0.48 x 0.14; origin at its centre
                  (truck (0.60, 2.05, -1.74)), its back on the wall, open to
                  +Z, its door standing open, the floor inside at y -0.21

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
    # Over the wall-side 0.4 m of the stash crate only (local x -0.45 to
    # -0.05), so a hand still goes straight down into the rest of it: four
    # posts on the crate's rim (its top at y +0.325), a slatted shelf at
    # +0.465, a brace on the wall side.
    hz = 0.605
    x0, x1 = -0.43, -0.07
    rim, y1 = 0.325, 0.62
    for x in (x0, x1):
        for sz in (-1, 1):
            z = sz * hz
            board(p, (0.04, y1 - rim - 0.005, 0.04), (x, (rim + 0.005 + y1) / 2, z), "Planks",
                  bevel=0.003)
            # A plate screwed to the rim at its foot.
            p.box((0.09, 0.005, 0.07), (x, rim + 0.0025, z), "Iron")
            bolt(p, (x + 0.03, rim + 0.005, z), (0, 1, 0), r=0.007, h=0.004)
            bolt(p, (x - 0.03, rim + 0.005, z), (0, 1, 0), r=0.007, h=0.004)
    # Rails between the posts along Z, under the slats.
    for x in (x0, x1):
        board(p, (0.025, 0.05, 2 * hz - 0.04), (x, 0.43, 0.0), "Planks", bevel=0.003)
    for k in range(7):
        z = -hz + 0.07 + k * (2 * hz - 0.14) / 6
        board(p, (0.42, 0.02, 0.1), ((x0 + x1) / 2, 0.465, z), "Planks", w, bevel=0.003)
    # The brace on the wall side, corner to corner, and a top rail at each
    # end.
    L = math.hypot(2 * hz - 0.04, y1 - rim - 0.08)
    board(p, (0.02, L, 0.05), (x0 - 0.03, (rim + 0.04 + y1 - 0.04) / 2, 0.0), "Planks",
          tilt=math.atan2(2 * hz - 0.04, y1 - rim - 0.08), bevel=0.003)
    for sz in (-1, 1):
        board(p, (x1 - x0 + 0.04, 0.04, 0.025), ((x0 + x1) / 2, y1 - 0.025, sz * (hz + 0.0325)),
              "Planks", bevel=0.003)
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
    t = 0.015
    back = -D / 2 + 0.01
    # A hollow carcass: back, sides, top, and a floor whose top is at
    # y -0.21 (what's kept in it lies there), its door standing open on
    # hinges at -X.
    p.box((W, H, t), (0.0, 0.0, back + t / 2), "Enamel", bevel=0.003, segs=1)
    for sx in (-1, 1):
        p.box((t, H, D - 0.01 - t), (sx * (W / 2 - t / 2), 0.0, back + t + (D - 0.01 - t) / 2),
              "Enamel", bevel=0.003, segs=1)
    for y, h in ((H / 2 - t / 2, t), (-0.21 - 0.015, 0.03)):
        p.box((W - 2 * t, h, D - 0.01 - t), (0.0, y, back + t + (D - 0.01 - t) / 2), "Enamel")
    # The door, swung open about 110 degrees, the cross on its outside.
    from common import RY
    front = back + D - 0.01
    hinge = (-W / 2, front)
    a = math.radians(-110)  # out into the room (+Z), not into the wall
    door = T(hinge[0], 0.0, hinge[1]) @ RY(a)
    p.box((W - 0.01, H - 0.01, 0.016), (W / 2, 0.0, 0.008), "Enamel", bevel=0.004, segs=1,
          xf=door)
    p.box((0.16, 0.05, 0.004), (W / 2, 0.03, 0.018), "Red", xf=door)
    p.box((0.05, 0.158, 0.004), (W / 2, 0.03, 0.022), "Red", xf=door)
    p.box((0.02, 0.06, 0.012), (W - 0.03, -0.05, 0.022), "Zinc", bevel=0.002, segs=1, xf=door)
    for y in (-0.16, 0.16):
        rod(p, (hinge[0], y - 0.03, hinge[1]), (hinge[0], y + 0.03, hinge[1]), 0.006, "Zinc",
            segs=6)
    return [p]


prop("stash_rack", stash_rack)
prop("stash_rack_2", stash_rack_2)
prop("bench_vice", bench_vice)
prop("aerial", aerial)
prop("med_cabinet", med_cabinet)
