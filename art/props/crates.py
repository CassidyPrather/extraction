"""Crates and containers. Writes assets/models/props/<name>.glb:

- `crate_slat`, `crate_army`  unit cubes (1 m, origin on the floor under
                  the centre, top at y 1.0) that `Builder::crate_box`
                  scales to its size: a nailed plank crate with battens
                  and a brace, and an olive army crate framed in battens
                  with steel corners and rope handles
- `supply_crate`  the yard and raid supply crate (`items::loot::SUPPLY_CRATE`,
                  0.9 x 0.78 x 0.6, origin at its centre): an army crate
                  with a hinged lid, hasps and stencils
- `stash_stand`   the truck stash's stand and shelf, in stash space
                  (`raid::truck::STASH_RAISE`): a trestle from the bed
                  (y -0.65) to under the crate (-0.35), and the shelf the
                  kit sits on at y 0 on four short posts
- `barrel`        a ribbed 200 l steel drum, 0.58 across, 0.88 tall
- `pallet`        a 1.2 x 0.8 wooden pallet, 0.144 tall

Materials: Boards, Planks, Timber, Paint, Iron, Rope, Cream, Rusty, Painted.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, Wear, board, bolt, prop, rod  # noqa: E402
from common import Part, T, fillet, section_circle  # noqa: E402

T_BOARD = 0.022


def side_boards(p, W, Hb, D, y0, mat, wear, n, gap):
    """Planked walls: full-width boards front and back (Z faces), boards
    fitted between them at the ends (X faces), from y0 to y0 + Hb."""
    t = T_BOARD
    bh = (Hb - gap * (n - 1)) / n
    for k in range(n):
        y = y0 + bh / 2 + k * (bh + gap)
        for sz in (-1, 1):
            board(p, (W, bh, t), (wear.j(0.002), y, sz * (D / 2 - t / 2)), mat, wear,
                  bevel=0.003)
        for sx in (-1, 1):
            board(p, (t, bh, D - 2 * t - 0.002), (sx * (W / 2 - t / 2), y, 0.0), mat, wear,
                  bevel=0.003)


def lid_boards(p, W, D, y_top, mat, wear, n=4, gap=0.004, over=0.0):
    t = T_BOARD
    Dl = D + 2 * over
    bw = (Dl - gap * (n - 1)) / n
    for k in range(n):
        z = -Dl / 2 + bw / 2 + k * (bw + gap)
        board(p, (W + 2 * over, t, bw), (0.0, y_top - t / 2, z), mat, wear, bevel=0.003,
              tilt=0.0)


def frame_battens(p, W, D, ya, yb, mat, bw=0.07, bt=0.02):
    """Battens framing each wall between heights ya and yb, standing bt
    proud: the front and back ones run past the ends' to cover them."""
    t = bt
    h = yb - ya
    inner = h - 2 * bw - 0.002
    for sz in (-1, 1):
        z = sz * (D / 2 + t / 2)
        for y in (ya + bw / 2, yb - bw / 2):
            p.box((W + 2 * t, bw, t), (0.0, y, z), mat, bevel=0.003, segs=1)
        for sx in (-1, 1):
            p.box((bw, inner, t), (sx * (W / 2 + t - bw / 2), (ya + yb) / 2, z), mat,
                  bevel=0.003, segs=1)
    for sx in (-1, 1):
        x = sx * (W / 2 + t / 2)
        for y in (ya + bw / 2, yb - bw / 2):
            p.box((t, bw, D), (x, y, 0.0), mat, bevel=0.003, segs=1)
        for sz in (-1, 1):
            p.box((t, inner, bw), (x, (ya + yb) / 2, sz * (D / 2 - bw / 2)), mat, bevel=0.003,
                  segs=1)


def corner_plates(p, W, D, y, s=0.1, bt=0.02):
    """Steel caps over the battens at the corners (between y and y + s),
    the end's plate wrapping the corner, the front's stopping short of it,
    so no two plates share a face."""
    pt = 0.004
    for sx in (-1, 1):
        for sz in (-1, 1):
            zf = sz * (D / 2 + bt + 0.001 + pt / 2)
            xs = sx * (W / 2 + bt - (s - 0.001) / 2 - 0.001)
            p.box((s - 0.001, s, pt), (xs, y + s / 2, zf), "Iron")
            xe = sx * (W / 2 + bt + 0.001 + pt / 2)
            ze = sz * (D / 2 + bt + 0.001 + pt - s / 2)
            p.box((pt, s, s), (xe, y + s / 2, ze), "Iron")
            bolt(p, (xs - sx * 0.02, y + s / 2, zf + sz * pt / 2), (0, 0, sz), r=0.007, h=0.004)
            bolt(p, (xe + sx * pt / 2, y + s / 2, ze - sz * 0.02), (sx, 0, 0), r=0.007, h=0.004)


def rope_handle(p, x, y, sx, span=0.17):
    """A rope loop through two cleats on an end wall whose battens' outer
    face is at |x|."""
    for z in (-span + 0.025, span - 0.025):
        p.box((0.035, 0.05, 0.05), (x + sx * 0.0175, y, z), "Boards", bevel=0.005, segs=1)
    path = [(-span, 0.0), (-span + 0.05, -0.05), (-0.05, -0.075), (0.05, -0.075),
            (span - 0.05, -0.05), (span, 0.0)]
    p.sweep(path, section_circle(0.011, 6), "Rope", xf=T(x + sx * 0.02, y, 0.0))


def stencil(p, x, y, z, sz, rows=((0.3, 0.05), (0.18, 0.035)), gap=0.025):
    """Painted stencil marks 4 mm proud of a front at z (facing sz)."""
    yy = y
    for w, h in rows:
        p.box((w, h, 0.004), (x, yy, z + sz * 0.002), "Cream")
        yy -= h + gap


# --- Crates -----------------------------------------------------------------------------------

def crate_slat():
    w = Wear(51)
    p = Part("static")
    top = 1.0
    Hb = top - T_BOARD
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.06, Hb, 0.06), (sx * 0.445, Hb / 2, sz * 0.445), "Timber", bevel=0.004)
    side_boards(p, 1.0, Hb, 1.0, 0.0, "Planks", w, 4, 0.012)
    lid_boards(p, 1.0, 1.0, top, "Planks", w)
    # Front and back: top and bottom battens, one upright between.
    for sz in (-1, 1):
        z = sz * 0.51
        for y in (0.04, Hb - 0.04):
            board(p, (1.0, 0.08, 0.02), (0.0, y, z), "Timber", bevel=0.003)
        board(p, (0.08, Hb - 0.162, 0.02), (w.j(0.01), Hb / 2, z), "Timber", bevel=0.003)
        for y in (0.04, Hb - 0.04):
            for x in (-0.44, 0.0, 0.44):
                bolt(p, (x, y, z + sz * 0.01), (0, 0, sz), r=0.006, h=0.003)
    # Ends: a diagonal brace.
    for sx in (-1, 1):
        L = math.hypot(0.84, 0.82)
        board(p, (0.02, L, 0.08), (sx * 0.51, Hb / 2, 0.0), "Timber",
              tilt=sx * math.atan2(0.84, 0.82), bevel=0.003)
    stencil(p, -0.24, 0.6, -0.5, -1)
    return [p]


def crate_army():
    w = Wear(53)
    p = Part("static")
    W = D = 0.96
    top = 1.0
    Hb = top - T_BOARD
    side_boards(p, W, Hb, D, 0.0, "Paint", w, 5, 0.004)
    lid_boards(p, W, D, top, "Paint", w, over=0.0)
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.05, Hb, 0.05), (sx * (W / 2 - 0.047), Hb / 2, sz * (D / 2 - 0.047)),
                  "Paint", bevel=0.004)
    frame_battens(p, W, D, 0.0, Hb - 0.004, "Paint")
    corner_plates(p, W, D, 0.0)
    corner_plates(p, W, D, Hb - 0.104)
    for sx in (-1, 1):
        rope_handle(p, sx * W / 2, 0.62, sx)
    stencil(p, 0.0, 0.62, -(D / 2), -1, rows=((0.42, 0.06), (0.3, 0.04), (0.16, 0.04)))
    return [p]


def supply_crate():
    w = Wear(57)
    p = Part("static")
    W, H, D = 0.88, 0.78, 0.58
    y0 = -H / 2
    lid_h = 0.09
    Hb = H - lid_h - 0.004
    side_boards(p, W, Hb, D, y0, "Paint", w, 4, 0.004)
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.05, Hb, 0.05), (sx * (W / 2 - 0.047), y0 + Hb / 2,
                                        sz * (D / 2 - 0.047)), "Paint", bevel=0.004)
    frame_battens(p, W, D, y0, y0 + Hb - 0.005, "Paint", bw=0.06)
    corner_plates(p, W, D, y0, s=0.09)
    # The lid: boards over a skirt that stands 5 mm proud of the walls.
    o = 0.005
    ys = y0 + Hb + 0.004
    lid_boards(p, W, D, H / 2, "Paint", w, n=3, over=o)
    sk = H / 2 - T_BOARD - ys
    for sz in (-1, 1):
        board(p, (W + 2 * o, sk, T_BOARD), (0.0, ys + sk / 2, sz * (D / 2 + o - T_BOARD / 2)),
              "Paint", bevel=0.003)
    for sx in (-1, 1):
        board(p, (T_BOARD, sk, D + 2 * o - 2 * T_BOARD - 0.002),
              (sx * (W / 2 + o - T_BOARD / 2), ys + sk / 2, 0.0), "Paint", bevel=0.003)
    # Hinges on the back, hasps on the front, over the seam.
    zb = D / 2
    for x in (-0.26, 0.26):
        p.box((0.06, sk - 0.01, 0.004), (x, ys + sk / 2, zb + o + 0.002), "Iron")
        p.box((0.06, 0.055, 0.004), (x, ys - 0.0325, zb + 0.02 + 0.002), "Iron")
        rod(p, (x - 0.03, ys + 0.002, zb + 0.016), (x + 0.03, ys + 0.002, zb + 0.016), 0.008,
            "Iron", segs=8)
        # Hasp: a strap down from the lid, a staple through it.
        p.box((0.04, sk - 0.01, 0.004), (x, ys + sk / 2, -(zb + o + 0.002)), "Iron")
        p.box((0.06, 0.05, 0.004), (x, ys - 0.06, -(zb + 0.02 + 0.002)), "Iron")
        rod(p, (x, ys - 0.045, -(zb + 0.024)), (x, ys - 0.045, -(zb + 0.04)), 0.005, "Iron",
            segs=6)
    for sx in (-1, 1):
        rope_handle(p, sx * W / 2, y0 + Hb * 0.62, sx, span=0.15)
    stencil(p, 0.0, 0.06, -(D / 2), -1, rows=((0.46, 0.06), (0.26, 0.04)))
    stencil(p, -0.12, 0.06 + 0.005, D / 2, 1, rows=((0.2, 0.04),))
    return [p]


# --- The truck stash's stand ------------------------------------------------------------------

def stash_stand():
    w = Wear(61)
    p = Part("static")
    bed, under = -0.65, -0.35
    # Legs, rails round the top, boards across that the crate sits on.
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.07, under - bed - 0.022, 0.07),
                  (sx * 0.39, (bed + under - 0.022) / 2, sz * 0.54), "Timber", bevel=0.005)
            # An angle bracket to the bed each side.
            p.box((0.004, 0.06, 0.07), (sx * 0.427, bed + 0.03, sz * 0.54), "Iron")
            p.box((0.06, 0.004, 0.07), (sx * 0.459, bed + 0.002, sz * 0.54), "Iron")
            bolt(p, (sx * 0.47, bed + 0.004, sz * 0.54), (0, 1, 0), r=0.008)
        board(p, (0.05, 0.07, 1.04), (sx * 0.39, under - 0.022 - 0.035 - 0.004, 0.0), "Timber",
              bevel=0.004)
        board(p, (0.04, 0.05, 1.04), (sx * 0.39, bed + 0.08, 0.0), "Timber", bevel=0.004)
    for sz in (-1, 1):
        board(p, (0.74, 0.07, 0.05), (0.0, under - 0.022 - 0.035 - 0.009, sz * 0.54), "Timber",
              bevel=0.004)
    n = 5
    bw = (1.16 - 0.006 * (n - 1)) / n
    for k in range(n):
        z = -0.58 + bw / 2 + k * (bw + 0.006)
        board(p, (0.86, 0.022, bw), (0.0, under - 0.011, z), "Boards", w, bevel=0.003)
    # The shelf inside the crate: a board on cleats and four short posts.
    board(p, (0.84, 0.02, 1.14), (0.0, 0.0, 0.0), "Boards", bevel=0.003)
    for sx in (-1, 1):
        board(p, (0.03, 0.035, 1.08), (sx * 0.4, -0.0275, 0.0), "Timber", bevel=0.003)
        for sz in (-1, 1):
            board(p, (0.04, -0.045 - under, 0.04), (sx * 0.4, (under - 0.045) / 2, sz * 0.52),
                  "Timber", bevel=0.003)
    return [p]


# --- Barrel and pallet ------------------------------------------------------------------------

def barrel():
    p = Part("static")
    r, h = 0.29, 0.88
    # A drum with two rolling hoops, chimes top and bottom, a dished lid
    # with a bung.
    prof = [(0.0, 0.006), (r - 0.012, 0.006), (r - 0.006, 0.0), (r, 0.012),
            (r, 0.29), (r + 0.012, 0.3), (r + 0.012, 0.31), (r, 0.32),
            (r, 0.56), (r + 0.012, 0.57), (r + 0.012, 0.58), (r, 0.59),
            (r, h - 0.012), (r - 0.006, h), (r - 0.012, h - 0.006),
            (r - 0.02, h - 0.014), (0.0, h - 0.014)]
    p.lathe(prof, "Painted", segs=24, xf=UP)
    p.cyl(0.03, h - 0.016, h - 0.006, "Iron", segs=8, xf=T(0.17, 0, 0.0) @ UP)
    p.cyl(0.018, h - 0.016, h - 0.008, "Iron", segs=8, xf=T(-0.17, 0, 0.05) @ UP)
    return [p]


def pallet():
    w = Wear(67)
    p = Part("static")
    L, Wd = 1.2, 0.8
    # Three bottom boards, nine blocks, three stringer boards, top deck.
    for z in (-0.35, 0.0, 0.35):
        board(p, (L, 0.022, 0.1 if z else 0.145), (0.0, 0.011, z), "Planks", w, bevel=0.003)
    for x in (-0.53, 0.0, 0.53):
        for z in (-0.35, 0.0, 0.35):
            board(p, (0.14 if x else 0.1, 0.078, 0.1), (x, 0.061, z), "Timber", w, bevel=0.004)
    for x in (-0.53, 0.0, 0.53):
        board(p, (0.1 if x else 0.145, 0.022, Wd), (x, 0.111, 0.0), "Planks", w, bevel=0.003)
    n = 7
    for k in range(n):
        z = -Wd / 2 + 0.05 + k * (Wd - 0.1) / (n - 1)
        board(p, (L, 0.022, 0.1 if k in (0, n - 1) else 0.08), (0.0, 0.133, z), "Planks", w,
              bevel=0.003)
    return [p]


def hay_bale():
    """A small square bale, 1.1 x 0.6 x 0.5 (origin under its centre),
    soft-edged and a little lumpy, tied with two twines."""
    p = Part("static")
    w = Wear(71)
    prof = fillet([(-0.25, 0.0), (0.25, 0.0), (0.25, 0.6), (-0.25, 0.6)], 0.06, 3)
    p.prism(prof, -0.55, 0.55, "Straw", axis="x", bevel=0.05, segs=2)
    for x in (-0.28, 0.28):
        x += w.j(0.02)
        tw = fillet([(-0.254, -0.004), (0.254, -0.004), (0.254, 0.604), (-0.254, 0.604)],
                    0.06, 3)
        # A twine band: the outline swept thin across X.
        p.prism(tw, x - 0.006, x + 0.006, "Rope", axis="x")
    return [p]


if __name__ == "__main__":
    prop("crate_slat", crate_slat)
    prop("crate_army", crate_army)
    prop("supply_crate", supply_crate)
    prop("stash_stand", stash_stand)
    prop("barrel", barrel)
    prop("pallet", pallet)
    prop("hay_bale", hay_bale)
