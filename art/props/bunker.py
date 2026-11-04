"""Fittings for the underground sites (src/world/underground.rs, `prop` and
`dressing`). Writes assets/models/props/<name>.glb for each; prop space as
in _kit.py: origin on the floor under the piece's centre, -Z its front.
Each fills the collider box the Rust recipe uses:

- `filter_unit`    a filter-ventilation unit (`PropKind::Machine`): steel
                   cabinet 1.4 x 1.6 x 1.0, a hand-crank wheel and a gauge
                   on its front, a duct up from its top to 2.5
- `switch_cabinet` a switchgear cabinet (`PropKind::Cabinet`), 0.8 x 1.4 x
                   0.4, its door on -Z with a handle and a warning plate
- `mine_tub`       a mine tub on its wheels (`PropKind::Cart`): body 0.9 x
                   0.7 x 1.4 from 0.25 to 0.95, four flanged wheels
- `bulkhead_lamp`  the lamp fitting on a ceiling, origin at the ceiling,
                   hanging down 0.15 (the light is the game's)
- `rubble`         fallen ceiling (`PropKind::Rubble`): broken slabs and
                   lumps heaped over the 1.2 x 0.45 x 1.0 collider, a
                   few bent rebars out of them, spill round its foot

Materials: Painted, Iron, Rusty, Zinc, Cast, Glass, Yellow, Black, Concrete.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, ALONG_X, Wear, board, bolt, prop, rod  # noqa: E402
from common import Part, T, fillet  # noqa: E402


# --- Filter-ventilation unit ------------------------------------------------------------------

def filter_unit():
    w = Wear(41)
    p = Part("static")
    # The cabinet on a plinth; a seam down the front between two panels.
    p.box((1.4, 0.08, 1.0), (0.0, 0.04, 0.0), "Cast", bevel=0.005)
    p.box((1.36, 1.5, 0.96), (0.0, 0.83, 0.0), "Painted", bevel=0.012)
    for x in (-0.34, 0.34):
        board(p, (0.6, 1.3, 0.012), (x + w.j(0.002), 0.85, -0.484), "Painted", bevel=0.004)
    # Bolts round each panel.
    for x in (-0.34, 0.34):
        for dx in (-0.27, 0.27):
            for y in (0.24, 1.46):
                bolt(p, (x + dx, y, -0.49), (0, 0, -1), r=0.008, h=0.006)
    # The crank wheel on its boss, a handle on its rim.
    cx, cy, z = -0.34, 0.95, -0.49
    # The boss out to the spokes (they stood 11 mm past it), the handle
    # from the rim (1 cm off it).
    p.cyl(0.05, 0.0, 0.072, "Iron", segs=12, xf=T(cx, cy, z - 0.072))
    p.tube(0.2, 0.18, 0.0, 0.025, "Iron", segs=24, xf=T(cx, cy, z - 0.08))
    for k in range(4):
        a = k * math.pi / 2 + 0.3
        rod(p, (cx, cy, z - 0.07), (cx + math.cos(a) * 0.185, cy + math.sin(a) * 0.185, z - 0.07),
            0.009, "Iron", segs=6)
    rod(p, (cx + 0.19, cy, z - 0.075), (cx + 0.19, cy, z - 0.17), 0.012, "Black", segs=8)
    # A gauge: bezel, dark face.
    gx, gy = 0.34, 1.2
    p.cyl(0.07, 0.0, 0.03, "Zinc", segs=16, xf=T(gx, gy, z - 0.03))
    p.cyl(0.058, 0.0, 0.006, "Black", segs=16, xf=T(gx, gy, z - 0.036))
    # Duct from the top, a flange where it leaves the cabinet.
    p.cyl(0.3, 0.0, 0.04, "Iron", segs=20, xf=T(0.0, 1.58, 0.0) @ UP)
    p.cyl(0.25, 0.0, 0.88, "Rusty", segs=20, xf=T(0.0, 1.62, 0.0) @ UP)
    return [p]


# --- Switch cabinet ---------------------------------------------------------------------------

def switch_cabinet():
    w = Wear(43)
    p = Part("static")
    p.box((0.8, 1.4, 0.38), (0.0, 0.7, 0.01), "Painted", bevel=0.008)
    board(p, (0.72, 1.28, 0.012), (w.j(0.002), 0.72, -0.185), "Painted", bevel=0.004)
    # Handle, hinges, a yellow plate.
    rod(p, (0.28, 0.6, -0.198), (0.28, 0.8, -0.198), 0.01, "Zinc", segs=8)
    for y in (0.25, 1.2):
        p.cyl(0.012, 0.0, 0.08, "Iron", segs=8, xf=T(-0.365, y - 0.04, -0.19) @ UP)
    board(p, (0.2, 0.14, 0.004), (0.0, 1.15, -0.1935), "Yellow", bevel=0.0)
    # Conduit up from the top.
    for x in (-0.2, 0.0, 0.2):
        p.cyl(0.022, 0.0, 0.6, "Zinc", segs=8, xf=T(x, 1.4, 0.05) @ UP)
    return [p]


# --- Mine tub ---------------------------------------------------------------------------------

def mine_tub():
    p = Part("static")
    # A riveted steel body, open on top: floor, sides and ends.
    p.box((0.86, 0.04, 1.33), (0.0, 0.27, 0.0), "Rusty", bevel=0.004)
    for sx in (-1, 1):
        p.box((0.03, 0.66, 1.36), (sx * 0.43, 0.61, 0.0), "Rusty", bevel=0.004)
    for sz in (-1, 1):
        p.box((0.8, 0.66, 0.03), (0.0, 0.61, sz * 0.665), "Rusty", bevel=0.004)
    # A band round the top lip.
    for sx in (-1, 1):
        p.box((0.05, 0.05, 1.4), (sx * 0.43, 0.925, 0.0), "Iron", bevel=0.004)
    for sz in (-1, 1):
        p.box((0.81, 0.05, 0.055), (0.0, 0.925, sz * 0.6725), "Iron", bevel=0.004)
    # Axles and flanged wheels, under the body.
    for z in (-0.45, 0.45):
        p.cyl(0.03, -0.42, 0.42, "Iron", segs=8, xf=T(0.0, 0.16, z) @ ALONG_X)
        for sx in (-1, 1):
            x = sx * 0.33
            p.cyl(0.16, -0.03, 0.03, "Cast", segs=16, xf=T(x, 0.16, z) @ ALONG_X)
            p.cyl(0.18, -0.008, 0.008, "Cast", segs=16, xf=T(x - sx * 0.035, 0.16, z) @ ALONG_X)
    # Couplings at the ends.
    for sz in (-1, 1):
        rod(p, (0.0, 0.35, sz * 0.68), (0.0, 0.35, sz * 0.8), 0.025, "Iron", segs=8)
    return [p]


# --- Bulkhead lamp ----------------------------------------------------------------------------

def bulkhead_lamp():
    p = Part("static")
    # The body under the ceiling, a glass dome, a cage over it.
    prof = fillet([(-0.16, -0.12), (0.16, -0.12), (0.16, 0.0), (-0.16, 0.0)], 0.02, 2)
    p.prism(prof, -0.09, 0.09, "Cast", axis="z", bevel=0.006, segs=2)
    p.sphere(0.07, (0.0, -0.12, 0.0), "Glass", segs=16, rings=8, scale=(1.4, 0.4, 0.9))
    for x in (-0.06, 0.0, 0.06):
        rod(p, (x, -0.12, -0.075), (x, -0.155, 0.0), 0.004, "Iron", segs=5)
        rod(p, (x, -0.155, 0.0), (x, -0.12, 0.075), 0.004, "Iron", segs=5)
    return [p]


# --- Rubble ----------------------------------------------------------------------------------

def rubble():
    w = Wear(47)
    p = Part("static")
    # A mound of fines and dust under it all.
    p.lathe([(0.0, 0.0), (0.6, 0.0), (0.45, 0.16), (0.22, 0.28), (0.0, 0.31)], "Concrete",
            segs=14, xf=UP, smooth=True)
    # Slabs leaning on the heap, lumps on and round it: each a box turned
    # every way, no two at the same height or angle.
    slabs = [
        ((0.7, 0.09, 0.5), (-0.2, 0.32, -0.1), 0.25, 0.4, -0.3),
        ((0.55, 0.08, 0.45), (0.3, 0.22, 0.2), -0.35, -0.2, 0.9),
        ((0.5, 0.1, 0.35), (0.05, 0.45, 0.1), 0.15, -0.1, 0.2),
    ]
    for size, at, tilt, roll, yaw in slabs:
        board(p, size, at, "Concrete", bevel=0.012, tilt=tilt, roll=roll, yaw=yaw)
    for k in range(14):
        a = k * 2.4 + w.j(0.3)
        r = 0.15 + 0.5 * ((k * 7) % 10) / 10.0
        s = 0.07 + 0.1 * ((k * 3) % 5) / 5.0
        x, z = math.cos(a) * r * 0.9, math.sin(a) * r * 0.75
        y = max(s * 0.5, 0.42 - r * 0.6) + w.j(0.02)
        board(p, (s * 1.3, s, s * 1.1), (x, y, z), "Concrete", bevel=0.01,
              tilt=w.j(0.6), roll=w.j(0.6), yaw=a)
    # Rebar out of the broken edges.
    for (a, b) in (((-0.3, 0.36, -0.2), (-0.45, 0.5, -0.28)),
                   ((0.25, 0.3, 0.25), (0.38, 0.42, 0.34)),
                   ((0.0, 0.48, 0.05), (0.04, 0.66, -0.04))):
        rod(p, a, b, 0.007, "Rusty", segs=6)
    return [p]


prop("filter_unit", filter_unit)
prop("switch_cabinet", switch_cabinet)
prop("mine_tub", mine_tub)
prop("bulkhead_lamp", bulkhead_lamp)
prop("rubble", rubble)
