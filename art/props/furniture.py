"""Furniture for the buildings (`Builder::table`, `shelf`, `bed`, `counter`,
`stove`, `workbench` in src/world/buildings.rs). Writes
assets/models/props/<name>.glb for each; prop space as in _kit.py: origin
on the floor under the piece's centre, -Z its front. Each one fills the
collider boxes the Rust recipe already uses and keeps their tops where the
loot spots expect them:

- `table`         kitchen table, top surface at y 0.785 over 1.2 x 0.7
- `shelf`         timber rack, boards' tops at 0.315, 0.765, 1.215, 1.665,
                  uprights at x +-0.5, 0.35 deep, 1.8 high
- `shelf_metal`   the same in painted steel angle and sheet
- `bed`           iron bedstead in 0.9 x 1.95, head at +Z, mattress at 0.56
- `counter_cabinet`, `counter_drawers`
                  0.6 m modules of a counter carcass (y 0 to 0.86, 0.6
                  deep), laid side by side and stretched to the counter's
                  length; `counter_top` is its 1 m top (0.86 to 0.9),
                  stretched along X
- `stove`         whitewashed kitchen range in 0.7 x 0.85 x 0.6, flue at
                  (0.2, y, 0.15) up to 2.4
- `workbench`     steel-framed bench, top 2.0 x 0.8 at 0.94

Materials: Timber, Boards, Painted, Rusty, Iron, Cast, Zinc, Plaster, Concrete,
Enamel, Ticking, Wool, Cream.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, ALONG_X, Wear, angle_iron, board, bolt, prop, rod  # noqa: E402
from common import Part, RX, RY, T, fillet  # noqa: E402


def tapered_leg(p, x, z, h, bottom, top, mat):
    """A square leg tapering from `top` (side) under the table to `bottom`
    at the floor."""
    rb, rt = bottom / math.sqrt(2), top / math.sqrt(2)
    p.lathe([(0.0, 0.0), (rb, 0.0), (rt, h), (0.0, h)], mat, segs=4, start=45.0,
            xf=T(x, 0, z) @ UP, smooth=False)


# --- Table ------------------------------------------------------------------------------------

def table():
    w = Wear(11)
    p = Part("static")
    top_y = 0.785
    # Four boards across the top, a hair apart, each a little off true.
    n, gap = 4, 0.003
    bw = (0.7 - gap * (n - 1)) / n
    for k in range(n):
        z = -0.35 + bw / 2 + k * (bw + gap)
        board(p, (1.2 + w.j(0.004), 0.03, bw), (w.j(0.003), top_y - 0.015, z), "Timber",
              yaw=w.j(0.004), bevel=0.004)
    # Tapered legs, an apron round under the top (its top 5 mm under the
    # boards' undersides, so the legs' tops don't share its plane).
    for sx in (-1, 1):
        for sz in (-1, 1):
            tapered_leg(p, sx * 0.53, sz * 0.28, top_y - 0.03, 0.038, 0.055, "Timber")
    ay = top_y - 0.03 - 0.005 - 0.045
    for sz in (-1, 1):
        board(p, (1.02, 0.09, 0.022), (0.0, ay, sz * 0.2845), "Timber", bevel=0.003)
    for sx in (-1, 1):
        board(p, (0.022, 0.09, 0.52), (sx * 0.5345, ay, 0.0), "Timber", bevel=0.003)
    # A drawer in the front apron: its face proud of the apron, a knob.
    board(p, (0.42, 0.07, 0.018), (0.0, ay, -0.3055), "Timber", bevel=0.003)
    rod(p, (0.0, ay, -0.314), (0.0, ay, -0.336), 0.013, "Timber", segs=8, chamfer=0.004)
    # An H of stretchers low down.
    for sx in (-1, 1):
        board(p, (0.03, 0.04, 0.54), (sx * 0.53, 0.16, 0.0), "Timber", bevel=0.003)
    board(p, (1.04, 0.035, 0.03), (0.0, 0.16, 0.0), "Timber", bevel=0.003)
    return [p]


# --- Shelves ----------------------------------------------------------------------------------

SHELF_YS = [0.3 + k * 0.45 for k in range(4)]


def shelf():
    w = Wear(23)
    p = Part("static")
    # Corner posts on the uprights' lines, a cleat inside each pair under
    # every shelf, two boards a shelf.
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.04, 1.8, 0.04), (sx * 0.5, 0.9, sz * 0.155), "Timber", bevel=0.004)
        for y in SHELF_YS:
            board(p, (0.03, 0.04, 0.29), (sx * 0.465, y - 0.035, 0.0), "Timber", bevel=0.003)
        # Side rails top and bottom tie each pair of posts.
        for y in (0.08, 1.74):
            board(p, (0.02, 0.06, 0.29), (sx * 0.5, y, 0.0), "Timber", bevel=0.003)
    for y in SHELF_YS:
        sag = w.j(0.006)
        for sz in (-1, 1):
            board(p, (0.958, 0.03, 0.165), (w.j(0.004), y, sz * 0.088), "Boards", w,
                  bevel=0.003, roll=sag)
    # Two slats crossed on the back, one behind the other.
    for k, s in enumerate((1, -1)):
        L = math.hypot(1.55, 0.92) + 0.04
        board(p, (0.07, L, 0.012), (0.0, 0.92, 0.181 + k * 0.013), "Timber",
              roll=-math.atan2(0.92, 1.55) * s, bevel=0.002)
    # A cap on top and a kick board along the bottom front.
    board(p, (1.08, 0.022, 0.37), (0.0, 1.811, 0.0), "Timber", bevel=0.004)
    board(p, (0.958, 0.09, 0.016), (0.0, 0.06, -0.162), "Timber", bevel=0.003)
    return [p]


def shelf_metal():
    w = Wear(29)
    p = Part("static")
    # Slotted angle uprights at the corners, folded sheet shelves with a
    # lip front and back, a cross brace each side.
    for sx in (-1, 1):
        for sz in (-1, 1):
            angle_iron(p, 1.794, (sx * 0.52, 0.903, sz * 0.175), "y", "Painted", w=0.035, t=0.003,
                       flip=(-sx, -sz))
            # Bolt heads at each shelf.
            for y in SHELF_YS:
                bolt(p, (sx * 0.5205, y - 0.005, sz * 0.15), (sx, 0, 0), r=0.006, h=0.004)
            # Feet.
            p.box((0.05, 0.006, 0.05), (sx * 0.5, 0.003, sz * 0.155), "Painted")
        for k in range(2):
            a, b = (0.08, 1.7) if k == 0 else (1.7, 0.08)
            # A strap from bottom front to top back and the other way.
            dz, dy = 0.3, abs(b - a)
            L = math.hypot(dz, dy)
            ang = math.atan2(dz, dy) * (1 if k == 0 else -1)
            board(p, (0.003, L, 0.025), (sx * (0.5245 + k * 0.005), 0.89, 0.0), "Painted",
                  tilt=ang, bevel=0.0)
    for y in SHELF_YS:
        sag = w.j(0.004)
        board(p, (1.0, 0.003, 0.334), (0.0, y + 0.0135, 0.0), "Painted", tilt=0.0, roll=sag,
              bevel=0.0)
        for sz in (-1, 1):
            board(p, (1.0, 0.027, 0.003), (0.0, y - 0.0015, sz * 0.165), "Painted", roll=sag,
                  bevel=0.0)
    return [p]


# --- Bed --------------------------------------------------------------------------------------

def bed():
    p = Part("static")
    r = 0.016
    head, foot = 0.955, -0.955
    for z, top in ((head, 0.86), (foot, 0.66)):
        for sx in (-1, 1):
            rod(p, (sx * 0.43, 0.012, z), (sx * 0.43, top, z), r, "Painted", segs=10)
            # A ball on each post.
            p.sphere(0.024, (sx * 0.43, top + 0.012, z), "Zinc", segs=10, rings=6)
            p.cyl(0.02, 0.0, 0.012, "Rubber", segs=10, xf=T(sx * 0.43, 0.0, z) @ UP)
        rod(p, (-0.43, top - 0.04, z), (0.43, top - 0.04, z), 0.012, "Painted", segs=8)
        rod(p, (-0.43, 0.42, z), (0.43, 0.42, z), 0.012, "Painted", segs=8)
        # Bars between.
        nb = 7 if z > 0 else 5
        for k in range(nb):
            x = -0.43 + 0.86 * (k + 1) / (nb + 1)
            rod(p, (x, 0.42, z), (x, top - 0.04, z), 0.007, "Painted", segs=6)
    # Angle-iron side rails with the spring mesh hooked between.
    for sx in (-1, 1):
        angle_iron(p, 1.91, (sx * 0.44, 0.43, 0.0), "z", "Painted", w=0.03, t=0.004,
                   flip=(-sx, -1))
    for k in range(9):
        z = -0.84 + k * 0.21
        p.box((0.87, 0.004, 0.012), (0.0, 0.424, z), "Iron")
    # Mattress, pillow, a folded blanket over the foot.
    prof = fillet([(-0.42, 0.432), (0.42, 0.432), (0.42, 0.56), (-0.42, 0.56)], 0.045, 3)
    p.prism(prof, -0.935, 0.935, "Ticking", axis="z", bevel=0.03, segs=2)
    p.sphere(0.2, (0.0, 0.592, 0.72), "Ticking", segs=14, rings=8, scale=(1.0, 0.24, 0.55))
    prof = fillet([(-0.445, 0.5), (0.445, 0.5), (0.445, 0.578), (-0.445, 0.578)], 0.03, 3)
    p.prism(prof, -0.92, -0.3, "Wool", axis="z", bevel=0.02, segs=2)
    p.cyl(0.032, -0.43, 0.43, "Wool", segs=10, xf=T(0.0, 0.56, -0.3) @ ALONG_X)
    return [p]


# --- Counter ----------------------------------------------------------------------------------

def carcass(p, w):
    """A 0.6 m module's sides, bottom, back and plinth."""
    for sx in (-1, 1):
        board(p, (0.018, 0.86, 0.58), (sx * 0.291, 0.43, 0.01), "Timber", bevel=0.002)
    board(p, (0.564, 0.018, 0.56), (0.0, 0.089, 0.01), "Timber", bevel=0.002)
    board(p, (0.564, 0.6, 0.006), (0.0, 0.5, 0.294), "Timber", bevel=0.0)
    board(p, (0.564, 0.08, 0.018), (0.0, 0.04, -0.22), "Cast", bevel=0.002)
    # Rail under the top.
    board(p, (0.564, 0.05, 0.02), (0.0, 0.83, -0.27), "Timber", bevel=0.002)


def handle(p, x, y, z, vertical=True):
    """A bent bar handle standing off a front at z."""
    d = 0.05
    a = (x, y - d, z) if vertical else (x - d, y, z)
    b = (x, y + d, z) if vertical else (x + d, y, z)
    for q in (a, b):
        rod(p, q, (q[0], q[1], z - 0.022), 0.005, "Zinc", segs=6)
    rod(p, (a[0], a[1], z - 0.022), (b[0], b[1], z - 0.022), 0.006, "Zinc", segs=6)


def counter_cabinet():
    w = Wear(31)
    p = Part("static")
    carcass(p, w)
    # Two doors.
    for sx in (-1, 1):
        x = sx * 0.1435
        board(p, (0.281, 0.7, 0.018), (x, 0.455, -0.289), "Painted", bevel=0.003,
              yaw=w.j(0.01))
        handle(p, sx * 0.03, 0.68, -0.298)
    return [p]


def counter_drawers():
    w = Wear(37)
    p = Part("static")
    carcass(p, w)
    hs = [0.2, 0.25, 0.25]
    y = 0.105
    for h in hs:
        board(p, (0.566, h - 0.006, 0.018), (w.j(0.002), y + h / 2, -0.289), "Painted",
              bevel=0.003)
        handle(p, 0.0, y + h - 0.06, -0.298, vertical=False)
        y += h
    return [p]


def counter_top():
    p = Part("static")
    # One slab 1 m long (stretched to the counter), its front edge bound
    # in an aluminium strip.
    board(p, (1.0, 0.04, 0.62), (0.0, 0.88, -0.01), "Boards", bevel=0.004)
    p.box((1.0, 0.046, 0.004), (0.0, 0.879, -0.322), "Zinc")
    return [p]


# --- Stove ------------------------------------------------------------------------------------

def stove():
    p = Part("static")
    # A brick range, whitewashed, on a plinth, under a cast iron plate.
    p.box((0.7, 0.06, 0.6), (0.0, 0.03, 0.0), "Concrete", bevel=0.006, segs=1)
    p.prism(fillet([(-0.33, -0.28), (0.33, -0.28), (0.33, 0.28), (-0.33, 0.28)], 0.03, 3),
            0.059, 0.823, "Plaster", axis="y", bevel=0.01, segs=2)
    p.box((0.7, 0.03, 0.6), (0.0, 0.835, 0.0), "Cast", bevel=0.004, segs=1)
    # Two hotplates of rings.
    for x, r in ((-0.13, 0.11), (0.14, 0.085)):
        p.cyl(r, 0.84, 0.855, "Cast", segs=20, xf=T(x, 0, -0.06) @ UP, chamfer=0.003)
        p.cyl(r * 0.6, 0.85, 0.86, "Cast", segs=16, xf=T(x, 0, -0.06) @ UP, chamfer=0.002)
        rod(p, (x - r * 0.3, 0.86, -0.06), (x + r * 0.3, 0.86, -0.06), 0.006, "Cast", segs=6)
    # Doors: firebox, ash pit, oven, each in a frame with a latch.
    for (x, y, dw, dh) in ((-0.13, 0.62, 0.22, 0.15), (-0.13, 0.2, 0.22, 0.1),
                           (0.16, 0.4, 0.26, 0.3)):
        p.box((dw + 0.04, dh + 0.04, 0.012), (x, y, -0.283), "Cast", bevel=0.003, segs=1)
        p.box((dw, dh, 0.012), (x, y, -0.295), "Cast", bevel=0.003, segs=1)
        rod(p, (x + dw / 2 - 0.03, y, -0.301), (x + dw / 2 - 0.03, y, -0.33), 0.006, "Steel")
        rod(p, (x + dw / 2 - 0.03, y, -0.33), (x + dw / 2 + 0.03, y, -0.33), 0.007, "Steel")
        for yy in (y - dh / 2 + 0.02, y + dh / 2 - 0.02):
            p.box((0.03, 0.02, 0.008), (x - dw / 2 + 0.005, yy, -0.305), "Iron")
    # A towel rail along the front.
    for x in (-0.3, 0.3):
        rod(p, (x, 0.77, -0.28), (x, 0.77, -0.33), 0.006, "Steel", segs=6)
    rod(p, (-0.32, 0.77, -0.33), (0.32, 0.77, -0.33), 0.009, "Steel", segs=8)
    # The flue: a collar, a damper, a joint.
    x, z = 0.2, 0.15
    p.cyl(0.085, 0.849, 0.89, "Cast", segs=16, xf=T(x, 0, z) @ UP, chamfer=0.004)
    p.cyl(0.07, 0.88, 2.4, "Iron", segs=14, xf=T(x, 0, z) @ UP)
    p.cyl(0.076, 1.58, 1.62, "Iron", segs=14, xf=T(x, 0, z) @ UP)
    rod(p, (x - 0.06, 1.3, z), (x - 0.15, 1.3, z), 0.006, "Steel", segs=6)
    p.box((0.03, 0.02, 0.012), (x - 0.16, 1.3, z), "Cast")
    return [p]


# --- Workbench --------------------------------------------------------------------------------

def workbench():
    w = Wear(41)
    p = Part("static")
    top0, top1 = 0.88, 0.94
    # Thick boards along the top.
    n, gap = 5, 0.003
    bw = (0.8 - gap * (n - 1)) / n
    for k in range(n):
        z = -0.4 + bw / 2 + k * (bw + gap)
        board(p, (2.0 + w.j(0.006), top1 - top0, bw), (w.j(0.004), (top0 + top1) / 2, z),
              "Timber", yaw=w.j(0.003), bevel=0.005)
    board(p, (1.99, 0.1, 0.03), (0.0, top0 - 0.055, -0.385), "Timber", bevel=0.004)
    # Steel angle frame: legs, rails under the top and over the shelf.
    for sx in (-1, 1):
        for sz in (-1, 1):
            angle_iron(p, top0 - 0.011, (sx * 0.95, (top0 + 0.001) / 2, sz * 0.35), "y",
                       "Rusty", w=0.045, t=0.005, flip=(-sx, -sz))
            p.box((0.07, 0.006, 0.07), (sx * 0.925, 0.003, sz * 0.325), "Rusty")
    for sz in (-1, 1):
        for y in (0.13, top0 - 0.12):
            angle_iron(p, 1.88, (0.0, y, sz * 0.344), "x", "Rusty", w=0.04, t=0.005,
                       flip=(-1, -sz))
    for sx in (-1, 1):
        angle_iron(p, 0.68, (sx * 0.944, 0.14, 0.0), "z", "Rusty", w=0.04, t=0.005,
                   flip=(-sx, -1))
    # Shelf planks on the low rails.
    for k in range(4):
        z = -0.3 + k * 0.2
        board(p, (1.86, 0.02, 0.19), (w.j(0.004), 0.141, z), "Boards", w, bevel=0.003)
    # A drawer pedestal under the right of the top.
    board(p, (0.46, 0.66, 0.62), (0.66, 0.48, 0.0), "Painted", bevel=0.004)
    for k, (y, h) in enumerate(((0.75, 0.16), (0.56, 0.2), (0.32, 0.26))):
        board(p, (0.42, h - 0.01, 0.016), (0.66, y, -0.315), "Painted", bevel=0.003)
        handle(p, 0.66, y + h / 2 - 0.045, -0.323, vertical=False)
    # A vice on the front left corner.
    vx = -0.78
    p.box((0.16, 0.06, 0.22), (vx, top1 + 0.03, -0.29), "Painted", bevel=0.006, segs=1)
    p.box((0.18, 0.09, 0.03), (vx, top1 + 0.085, -0.39), "Painted", bevel=0.005, segs=1)
    p.box((0.18, 0.09, 0.03), (vx, top1 + 0.085, -0.46), "Painted", bevel=0.005, segs=1)
    for z in (-0.407, -0.443):
        p.box((0.17, 0.035, 0.004), (vx, top1 + 0.11, z), "Iron")
    rod(p, (vx - 0.04, top1 + 0.07, -0.4), (vx - 0.04, top1 + 0.07, -0.47), 0.008, "Steel")
    rod(p, (vx + 0.04, top1 + 0.07, -0.4), (vx + 0.04, top1 + 0.07, -0.47), 0.008, "Steel")
    rod(p, (vx, top1 + 0.07, -0.3), (vx, top1 + 0.07, -0.56), 0.011, "Steel")
    rod(p, (vx - 0.11, top1 + 0.07, -0.55), (vx + 0.11, top1 + 0.07, -0.55), 0.007, "Steel")
    for sx in (-1, 1):
        p.sphere(0.012, (vx + sx * 0.115, top1 + 0.07, -0.55), "Steel", segs=8, rings=5)
    # A tool board along the back, with a few hooks.
    board(p, (1.96, 0.52, 0.018), (0.0, top1 + 0.25, 0.411), "Boards", bevel=0.003)
    for x in (-0.7, -0.45, -0.2, 0.3, 0.55):
        rod(p, (x, top1 + 0.35, 0.402), (x, top1 + 0.35, 0.36), 0.004, "Iron", segs=6)
    return [p]


prop("table", table)
prop("shelf", shelf)
prop("shelf_metal", shelf_metal)
prop("bed", bed)
prop("counter_cabinet", counter_cabinet)
prop("counter_drawers", counter_drawers)
prop("counter_top", counter_top)
prop("stove", stove)
prop("workbench", workbench)
