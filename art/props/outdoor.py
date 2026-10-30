"""Props outside: signposts, fence posts and the camp. Writes
assets/models/props/<name>.glb:

- `signpost`      a sign board on a post (`MarkerKind::Sign`, src/world/notes.rs),
                  in sign space: origin on the ground under the post, the
                  text facing +Z on the plane z 0.05 over x +-0.5, y 1.1 to
                  1.8, so everything in front of the boards stays outside
                  that; a plank board in a batten frame under a little
                  pitched roof
- `fence_post`    a concrete quarantine fence post, 2.7 tall, sunk 0.3,
                  kept to a few dozen triangles (there are hundreds)
- `camp_table`    the camp's table (`camp::TABLE`, top at 0.76 over 1.3 x 0.7);
                  `camp_stool` and `camp_box` stand by it, each on its own ground
                  on two trestles, an ammo box under it, a stool beside it
- `camp_board`    the camp's notice board, 1.2 x 0.9, in board space (cards
                  on +Z at z 0.016), with the strut behind
- `camp_post`     the yard's skip-card post: the card's plate at y 1.35
- `lantern`       a hurricane lantern (origin under its base), its globe lit

Materials: Timber, Planks, Boards, Concrete, Iron, Paint, Rope, Lamp, Zinc,
Cream.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, Wear, board, bolt, prop, rod  # noqa: E402
from common import Part, T, fillet  # noqa: E402
from crates import frame_battens, lid_boards, rope_handle, side_boards  # noqa: E402


def square_post(p, side_lo, side_hi, y0, y1, mat, cap=0.03):
    """A square post tapering from side_lo at y0 to side_hi near y1, its
    top chamfered to a point `cap` high."""
    r0, r1 = side_lo / math.sqrt(2), side_hi / math.sqrt(2)
    p.lathe([(0.0, y0), (r0, y0), (r1, y1 - cap), (r1 * 0.55, y1), (0.0, y1)], mat, segs=4,
            start=45.0, xf=UP)


def signpost():
    w = Wear(81)
    p = Part("static")
    pz = -0.035
    square_post(p, 0.105, 0.095, -0.3, 1.9, "Timber", cap=0.04)
    for v in p.bm.verts:
        v.co.z += pz
    # Three boards, a frame of battens round them.
    bz = 0.026
    for k, dy in enumerate((-0.255, 0.0, 0.255)):
        board(p, (1.1, 0.25, 0.022), (w.j(0.004), 1.45 + dy, bz), "Planks", w, bevel=0.004)
    fz = 0.047
    for y in (1.07, 1.83):
        board(p, (1.14, 0.04, 0.02), (0.0, y, fz), "Timber", bevel=0.004)
    for sx in (-1, 1):
        board(p, (0.04, 0.718, 0.02), (sx * 0.53, 1.45, fz), "Timber", bevel=0.004)
    for y in (1.07, 1.83):
        bolt(p, (0.0, y, fz + 0.01), (0, 0, 1), r=0.013, h=0.006)
    # The roof: two boards over a ridge on the post.
    a = math.radians(28)
    ridge_y, ridge_z = 1.9, -0.005
    for s in (-1, 1):
        cz = ridge_z + s * math.cos(a) * 0.065
        cy = ridge_y - math.sin(a) * 0.065
        board(p, (1.26, 0.02, 0.14), (0.0, cy, cz), "Planks", bevel=0.003, tilt=s * a)
    return [p]


def fence_post():
    p = Part("static")
    square_post(p, 0.13, 0.095, -0.3, 2.72, "Concrete", cap=0.06)
    return [p]


def camp_table_parts():
    """The table alone (its stool and box are their own props)."""
    w = Wear(83)
    table = Part("static")
    top = 0.76
    n = 5
    bw = (0.7 - 0.004 * (n - 1)) / n
    for k in range(n):
        z = -0.35 + bw / 2 + k * (bw + 0.004)
        board(table, (1.3 + w.j(0.01), 0.03, bw), (w.j(0.004), top - 0.015, z), "Planks",
              yaw=w.j(0.004), bevel=0.004)
    for x in (-0.48, 0.48):
        board(table, (0.06, 0.06, 0.62), (x, top - 0.06, 0.0), "Timber", bevel=0.004)
        for s in (-1, 1):
            z0, z1, y0, y1 = s * 0.22, s * 0.31, top - 0.06, 0.0
            L = math.hypot(z1 - z0, y1 - y0)
            ang = math.atan2(z1 - z0, y0 - y1)
            board(table, (0.05, L, 0.045), (x - 0.055, (y0 + y1) / 2, (z0 + z1) / 2), "Timber",
                  tilt=-ang, bevel=0.004)
        board(table, (0.035, 0.05, 0.44), (x - 0.055, 0.5, 0.0), "Timber", bevel=0.003)
    board(table, (0.93, 0.05, 0.04), (0.0, 0.55, 0.0), "Timber", bevel=0.003)
    return [table]


def camp_stool():
    """The stool by the table: a round seat on three splayed legs, its feet
    at y 0 round its own origin (`camp::models` stands it on the ground)."""
    p = Part("static")
    p.cyl(0.17, 0.43, 0.47, "Boards", segs=16, xf=UP, chamfer=0.008)
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.3
        top_p = (0.09 * math.cos(a), 0.44, 0.09 * math.sin(a))
        bot_p = (0.15 * math.cos(a), 0.0, 0.15 * math.sin(a))
        rod(p, bot_p, top_p, 0.018, "Timber", segs=6)
    return [p]


def camp_box():
    """The ammo box under the table, its bottom at y 0 round its own origin."""
    w = Wear(84)
    box = Part("static")
    W, H, D = 0.46, 0.4, 0.36
    side_boards(box, W, H - 0.022, D, 0.0, "Paint", w, 3, 0.004)
    lid_boards(box, W, D, H, "Paint", w, n=3)
    frame_battens(box, W, D, 0.0, H - 0.026, "Paint", bw=0.05)
    for sx in (-1, 1):
        rope_handle(box, sx * W / 2, 0.24, sx, span=0.1)
    return [box]


def camp_board():
    w = Wear(87)
    p = Part("static")
    # Planks across, the cards' plane 4 mm in front of them.
    for k, y in enumerate((-0.3375, -0.1125, 0.1125, 0.3375)):
        board(p, (1.2, 0.222, 0.02), (w.j(0.003), y, 0.002), "Planks", bevel=0.003)
    for y in (-0.47, 0.47):
        board(p, (1.32, 0.04, 0.03), (0.0, y, 0.005), "Timber", bevel=0.004)
    for sx in (-1, 1):
        board(p, (0.04, 0.898, 0.03), (sx * 0.62, 0.0, 0.005), "Timber", bevel=0.004)
    # The strut behind.
    board(p, (0.04, 0.81, 0.04), (0.0, -0.05, -0.2), "Timber", tilt=0.45, bevel=0.004)
    board(p, (0.1, 0.05, 0.03), (0.0, 0.3, -0.023), "Timber", bevel=0.003)
    return [p]


def camp_post():
    w = Wear(89)
    p = Part("static")
    square_post(p, 0.085, 0.075, -0.25, 1.56, "Timber", cap=0.03)
    board(p, (0.42, 0.28, 0.022), (0.0, 1.35, 0.029), "Planks", w, bevel=0.004)
    for x in (-0.18, 0.18):
        bolt(p, (x, 1.47, 0.04), (0, 0, 1), r=0.007, h=0.003)
    return [p]


def lantern():
    p = Part("static")
    # Fuel tank, the burner collar, the globe, its wire guard, the cap and
    # the bail.
    p.lathe([(0.0, 0.0), (0.055, 0.0), (0.062, 0.012), (0.062, 0.035), (0.05, 0.05),
             (0.03, 0.056), (0.0, 0.056)], "Paint", segs=16, xf=UP)
    p.lathe([(0.0, 0.054), (0.034, 0.054), (0.034, 0.07), (0.0, 0.07)], "Zinc", segs=12, xf=UP)
    p.lathe([(0.0, 0.066), (0.03, 0.068), (0.045, 0.1), (0.047, 0.13), (0.04, 0.16),
             (0.028, 0.176), (0.0, 0.176)], "Lamp", segs=16, xf=UP)
    for k in range(4):
        a = math.pi / 2 * k + math.pi / 4
        c, s = math.cos(a), math.sin(a)
        rod(p, (0.05 * c, 0.05, 0.05 * s), (0.052 * c, 0.172, 0.052 * s), 0.0025, "Iron",
            segs=5)
    p.lathe([(0.0, 0.17), (0.045, 0.17), (0.05, 0.185), (0.03, 0.205), (0.012, 0.215),
             (0.0, 0.215)], "Paint", segs=16, xf=UP)
    pts = [(0.055 * math.cos(math.pi * i / 8), 0.19 + 0.08 * math.sin(math.pi * i / 8))
           for i in range(9)]
    for a_, b_ in zip(pts, pts[1:]):
        rod(p, (a_[0], a_[1], 0.0), (b_[0], b_[1], 0.0), 0.0025, "Iron", segs=5)
    return [p]


def fence_panel():
    """A garden fence's plank panel between two posts: 2.0 x 1.1 x 0.08,
    origin at its centre, its boards' face to -Z (the street), the rails
    behind; the boards weathered, one short, gaps between. A breakable
    thing (`buildings::Breakable`): Destruction spawns and breaks it."""
    p = Part("static")
    w = Wear(97)
    n = 13
    for k in range(n):
        x = -1.0 + (k + 0.5) * 2.0 / n
        h = 1.1 - (0.18 if k == 7 else 0.0) - abs(w.j(0.03))
        board(p, (2.0 / n - 0.035, h, 0.02), (x, -0.55 + h / 2, -0.03), "Planks", wear=w,
              bevel=0.002)
    for y in (-0.3, 0.35):
        p.box((2.0, 0.08, 0.035), (0.0, y, -0.0025), "Timber", bevel=0.003, segs=1)
    return [p]


prop("signpost", signpost)
prop("fence_post", fence_post)
prop("fence_panel", fence_panel)
prop("camp_table", camp_table_parts)
prop("camp_stool", camp_stool)
prop("camp_box", camp_box)
prop("camp_board", camp_board)
prop("camp_post", camp_post)
prop("lantern", lantern)

# --- The quarantine gate (`quarantine_gate` in src/world/buildings.rs) ------------------------

def road_block():
    """A precast foundation block, 1.4 x 1.2 x 1.4, origin under its centre:
    chamfered, a slot along each side, two lifting loops on top, a
    stencilled number."""
    p = Part("static")
    prof = fillet([(-0.7, 0.0), (0.7, 0.0), (0.7, 1.2), (-0.7, 1.2)], [0.0, 0.0, 0.04, 0.04], 2)
    p.prism(prof, -0.7, 0.7, "Concrete", axis="z", bevel=0.03, segs=1)
    for sx in (-1, 1):
        # Loops of rebar, bent up out of the top.
        x = sx * 0.4
        pts = [(x - 0.07, 1.19), (x - 0.06, 1.27), (x, 1.3), (x + 0.06, 1.27), (x + 0.07, 1.19)]
        for a_, b_ in zip(pts, pts[1:]):
            rod(p, (a_[0], a_[1], 0.0), (b_[0], b_[1], 0.0), 0.012, "Rusty", segs=6)
    p.box((0.32, 0.12, 0.004), (0.25, 0.75, -0.702), "Cream")
    p.box((0.18, 0.08, 0.004), (0.3, 0.58, -0.702), "Cream")
    return [p]


def gate_post():
    """A steel post 0.25 square, 3.5 tall, on a base plate, capped."""
    p = Part("static")
    p.box((0.25, 3.48, 0.25), (0.0, 1.76, 0.0), "Painted", bevel=0.006, segs=1)
    p.box((0.42, 0.02, 0.42), (0.0, 0.01, 0.0), "Iron", bevel=0.003, segs=1)
    for sx in (-1, 1):
        for sz in (-1, 1):
            bolt(p, (sx * 0.17, 0.02, sz * 0.17), (0, 1, 0), r=0.014, h=0.012)
    p.box((0.3, 0.03, 0.3), (0.0, 3.515, 0.0), "Iron", bevel=0.004, segs=1)
    # Red and white bands at eye height.
    for k in range(4):
        p.box((0.258, 0.25, 0.258), (0.0, 1.0 + k * 0.25 + 0.125, 0.0),
              "Red" if k % 2 == 0 else "Cream", bevel=0.004, segs=1)
    return [p]


def barrier_arm():
    """The barrier arm in arm space: pivot at the origin, the arm up +Y
    (the game leans it), banded red and white, a counterweight below the
    pivot, the pin along Z."""
    p = Part("static")
    n = 12
    seg = 6.0 / n
    for k in range(n):
        y = -0.1 + seg * (k + 0.5)
        p.box((0.12, seg, 0.12), (0.0, y, 0.0), "Red" if k % 2 == 0 else "Cream",
              bevel=0.006 if k in (0, n - 1) else 0.0, segs=1)
    p.box((0.3, 0.45, 0.2), (0.0, -0.3, 0.0), "Concrete", bevel=0.02, segs=1)
    p.cyl(0.05, -0.33, 0.09, "Iron", segs=10)
    for z in (-0.075, 0.075):
        p.box((0.16, 0.24, 0.012), (0.0, 0.0, z), "Iron", bevel=0.003, segs=1)
    # A rubber end cap.
    p.box((0.13, 0.04, 0.13), (0.0, 5.92, 0.0), "Rubber", bevel=0.01, segs=1)
    return [p]


prop("road_block", road_block)
prop("gate_post", gate_post)
prop("barrier_arm", barrier_arm)
