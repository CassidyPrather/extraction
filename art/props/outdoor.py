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
- `camp_table`    the camp's table (`camp::TABLE`, top at 0.76 over 1.3 x 0.7)
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
from common import Part, T  # noqa: E402
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
    """The table and its ammo box, the box moved to its place."""
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
    # The stool by it: a round seat on three splayed legs.
    sx_, sz_ = 0.75, 0.65
    table.cyl(0.17, 0.43, 0.47, "Boards", segs=16, xf=T(sx_, 0, sz_) @ UP, chamfer=0.008)
    for k in range(3):
        a = 2 * math.pi * k / 3 + 0.3
        top_p = (sx_ + 0.09 * math.cos(a), 0.44, sz_ + 0.09 * math.sin(a))
        bot_p = (sx_ + 0.15 * math.cos(a), 0.0, sz_ + 0.15 * math.sin(a))
        rod(table, bot_p, top_p, 0.018, "Timber", segs=6)
    # The ammo box.
    box = Part("box")
    W, H, D = 0.46, 0.4, 0.36
    side_boards(box, W, H - 0.022, D, 0.0, "Paint", w, 3, 0.004)
    lid_boards(box, W, D, H, "Paint", w, n=3)
    frame_battens(box, W, D, 0.0, H - 0.026, "Paint", bw=0.05)
    for sx in (-1, 1):
        rope_handle(box, sx * W / 2, 0.24, sx, span=0.1)
    import bmesh
    bmesh.ops.translate(box.bm, verts=box.bm.verts, vec=(-0.35, 0.0, -0.03))
    return [table, box]


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


prop("signpost", signpost)
prop("fence_post", fence_post)
prop("camp_table", camp_table_parts)
prop("camp_board", camp_board)
prop("camp_post", camp_post)
prop("lantern", lantern)
