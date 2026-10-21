"""What's left in a ruined house: rubble, fallen beams, burnt furniture.
For the damaged and burnt houses (`house_ruin` in src/world/buildings.rs),
each prop in prefab space with its origin on the floor under its centre:

- `rubble_planks`  a heap of broken boards and shingles under a hole in
                   the roof, 1.6 x 0.45 x 1.4
- `rubble_brick`   bricks and mortar spilled from a breach, 1.8 x 0.5 x 1.4
- `beam_fallen`    a roof beam, 4.4 m along X, snapped at one end
- `beam_charred`   the same, burnt: checked into blocks, split, one end gone
                   to a point
- `bed_burnt`      a bedstead with the bedding burnt off: iron rails, the
                   springs sagging, 2.0 x 0.9 (as `bed`)
- `table_burnt`    a table burnt through on one side, its top down on the
                   floor at that end, 1.2 x 0.7
- `ash_heap`       ash and charcoal in a low drift, 1.2 across
- `rubble_logs`    a log cabin's fallen corner: wall logs down in a heap
                   either way, a sheet of its roof iron on them, 1.9 x 0.75
                   x 1.9 (`cabin_ruin`)
- `rubble_concrete` a shop's roof slab broken on its floor: slabs and lumps
                   of concrete, the rebar bent out of them, 1.8 x 0.45 x 1.5
                   (`shop_ruin`)

Materials: Timber, Planks, Shingles, Brick, Plaster, Concrete, Corrugated,
Char, Iron, Rusty.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import UP, ALONG_X, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, T  # noqa: E402


def heap(p, w, seed, n, size, mat, lo, hi, y_top, xf_extra=None):
    """`n` pieces of `size` (x, y, z, each varied) strewn over a w x d oval,
    lying flatter and lower towards its edge, the heap `y_top` high in the
    middle."""
    rng = Wear(seed).rng
    hw, hd = w
    for k in range(n):
        r = math.sqrt(rng.random())
        a = rng.random() * 2 * math.pi
        x, z = r * hw * math.cos(a), r * hd * math.sin(a)
        y = (1 - r * r) * y_top
        sx = size[0] * rng.uniform(lo, hi)
        sy = size[1] * rng.uniform(0.8, 1.2)
        sz = size[2] * rng.uniform(lo, hi)
        tilt = (1 - r) * 0.7
        xf = T(x, y + sy / 2, z) @ RY(rng.random() * math.pi) @ RX(rng.uniform(-tilt, tilt)) \
            @ RZ(rng.uniform(-tilt, tilt))
        p.box((sx, sy, sz), (0.0, 0.0, 0.0), mat if isinstance(mat, str) else rng.choice(mat),
              bevel=0.004 if sx > 0.1 else 0.0, segs=1, xf=xf)


def mound(p, w, h, mat, segs=12):
    """A low lumpy dome w (x, z half) across and h high: the heap's body
    under its pieces."""
    rings = [(1.0, 0.0), (0.85, 0.35), (0.6, 0.7), (0.3, 0.92), (0.0, 1.0)]
    prof = [(r, y * h) for r, y in rings]
    p.lathe(prof, mat, segs=segs, xf=RY(0.3) @ _scale(w[0], 1.0, w[1]) @ UP)


def _scale(x, y, z):
    from common import S
    return S(x, y, z)


def rubble_planks():
    p = Part("static")
    mound(p, (0.75, 0.6), 0.22, "Char")
    heap(p, (0.7, 0.6), 3, 14, (1.1, 0.03, 0.15), "Planks", 0.4, 1.0, 0.25)
    heap(p, (0.75, 0.6), 4, 16, (0.4, 0.015, 0.25), "Shingles", 0.7, 1.1, 0.25)
    heap(p, (0.6, 0.5), 5, 4, (1.3, 0.08, 0.1), "Timber", 0.6, 1.0, 0.2)
    heap(p, (0.5, 0.4), 6, 8, (0.25, 0.12, 0.2), "Plaster", 0.5, 1.0, 0.2)
    return [p]


def rubble_brick():
    p = Part("static")
    mound(p, (0.85, 0.65), 0.32, "Plaster")
    heap(p, (0.85, 0.65), 7, 40, (0.23, 0.075, 0.11), "Brick", 0.85, 1.05, 0.34)
    heap(p, (0.6, 0.5), 8, 5, (0.45, 0.15, 0.3), "Brick", 0.8, 1.2, 0.3)
    heap(p, (0.7, 0.55), 9, 4, (0.9, 0.03, 0.14), "Planks", 0.5, 1.0, 0.3)
    return [p]


def splinters(p, x, sx, w, h, mat, rng, n=5):
    """A snapped end at x (facing sx): spikes of the beam's grain."""
    for k in range(n):
        y = (rng.random() - 0.5) * h * 0.8
        z = (rng.random() - 0.5) * w * 0.8
        ln = rng.uniform(0.08, 0.3)
        rod(p, (x - sx * 0.02, y, z), (x + sx * ln, y + rng.uniform(-0.02, 0.02),
                                        z + rng.uniform(-0.02, 0.02)), rng.uniform(0.015, 0.03),
            mat, segs=4)


def beam_fallen():
    p = Part("static")
    w = Wear(12)
    L, bw, bh = 4.4, 0.2, 0.24
    p.box((L - 0.1, bh, bw), (-0.05, 0.0, 0.0), "Timber", bevel=0.01, segs=1)
    # A notch for the ridge at one end, the other snapped off.
    p.box((0.12, 0.06, bw + 0.004), (-L / 2 + 0.25, bh / 2 - 0.025, 0.0), "Char")
    splinters(p, L / 2 - 0.1, 1, bw, bh, "Timber", w.rng)
    # The nails it was held by.
    for x in (-1.2, 0.3, 1.5):
        rod(p, (x, bh / 2 - 0.01, 0.03), (x + 0.02, bh / 2 + 0.06, 0.04), 0.004, "Iron", segs=4)
    return [p]


def beam_charred():
    p = Part("static")
    w = Wear(13)
    L, bw, bh = 4.4, 0.2, 0.24
    # Checked into blocks, each a little off true, the gaps between them
    # deep and black; one end burnt to a point.
    n = 14
    seg = (L - 0.6) / n
    p.box((L - 0.5, bh - 0.05, bw - 0.05), (0.05, 0.0, 0.0), "Char")
    for k in range(n):
        x = -L / 2 + 0.3 + (k + 0.5) * seg
        shrink = 1.0 - 0.25 * max(0.0, (k - n + 4) / 4)
        p.box((seg - 0.025, (bh - 0.01) * shrink, (bw - 0.01) * shrink),
              (x, w.j(0.006), w.j(0.006)), "Char", bevel=0.008, segs=1,
              xf=T(x, 0, 0) @ RX(w.j(0.04)) @ T(-x, 0, 0))
    p.lathe([(0.0, 0.0), (bw * 0.45, 0.0), (0.03, 0.5)], "Char", segs=6,
            xf=T(L / 2 - 0.3, 0.0, 0.0) @ ALONG_X)
    p.box((0.25, bh - 0.02, bw - 0.02), (-L / 2 + 0.17, 0.0, 0.0), "Timber", bevel=0.01, segs=1)
    return [p]


def bed_burnt():
    p = Part("static")
    w = Wear(14)
    L, W = 2.0, 0.9
    # Head and foot: iron tubes in a frame, the head higher.
    for z, top in ((-L / 2 + 0.03, 1.0), (L / 2 - 0.03, 0.7)):
        for x in (-W / 2 + 0.03, W / 2 - 0.03):
            rod(p, (x, 0.0, z), (x, top, z), 0.022, "Rusty", segs=8)
        for y in (top - 0.05, 0.42):
            rod(p, (-W / 2 + 0.03, y, z), (W / 2 - 0.03, y, z), 0.015, "Rusty", segs=6)
        for k in range(5):
            x = -W / 2 + 0.03 + (k + 1) * (W - 0.06) / 6
            rod(p, (x, 0.42, z), (x, top - 0.05, z), 0.008, "Rusty", segs=5)
    # The side rails, the springs' wires sagging between them.
    for x in (-W / 2 + 0.03, W / 2 - 0.03):
        rod(p, (x, 0.4, -L / 2 + 0.03), (x, 0.4, L / 2 - 0.03), 0.018, "Rusty", segs=6)
    nw = 9
    for k in range(nw):
        z = -L / 2 + 0.15 + k * (L - 0.3) / (nw - 1)
        sag = 0.12 + w.j(0.04)
        pts = [(-W / 2 + 0.05, 0.4, z), (0.0, 0.4 - sag, z + w.j(0.03)), (W / 2 - 0.05, 0.4, z)]
        rod(p, pts[0], pts[1], 0.004, "Iron", segs=4)
        rod(p, pts[1], pts[2], 0.004, "Iron", segs=4)
    # Ash and a few charred slats on the floor under it.
    heap(p, (0.35, 0.8), 15, 10, (0.6, 0.02, 0.07), "Char", 0.4, 1.0, 0.02)
    mound(p, (0.4, 0.85), 0.04, "Char", segs=10)
    return [p]


def table_burnt():
    p = Part("static")
    w = Wear(16)
    # The top, burnt through at +X, its end down on the floor; the two legs
    # left at -X still standing.
    tilt = math.atan2(0.74, 1.05)
    top = T(-0.55, 0.76, 0.0) @ RZ(-tilt) @ T(0.55, 0.0, 0.0)
    p.box((1.0, 0.04, 0.7), (0.0, 0.0, 0.0), "Char", bevel=0.006, segs=1, xf=top)
    for k in range(5):
        z = -0.3 + k * 0.15
        board(p, (0.12 + w.j(0.04), 0.02, 0.1), (0.62 + w.j(0.05), 0.01, z), "Char", wear=w)
    for z in (-0.3, 0.3):
        p.box((0.05, 0.74, 0.05), (-0.55, 0.37, z), "Char")
    # The burnt legs lying where they fell.
    for z in (-0.25, 0.2):
        p.box((0.05, 0.05, 0.5), (0.35 + w.j(0.1), 0.025, z), "Char",
              xf=T(0.35, 0.025, z) @ RY(0.6 + w.j(0.4)) @ T(-0.35, -0.025, -z))
    heap(p, (0.3, 0.3), 17, 6, (0.18, 0.02, 0.05), "Char", 0.5, 1.0, 0.02)
    return [p]


def ash_heap():
    p = Part("static")
    mound(p, (0.6, 0.5), 0.12, "Char")
    heap(p, (0.55, 0.45), 18, 12, (0.35, 0.04, 0.05), "Char", 0.4, 1.0, 0.1)
    heap(p, (0.4, 0.35), 19, 6, (0.18, 0.02, 0.15), "Shingles", 0.6, 1.0, 0.08)
    return [p]


def rubble_logs():
    p = Part("static")
    w = Wear(21)
    mound(p, (0.85, 0.8), 0.18, "Plaster")
    # Layers of logs, the lower ones longer, each across the one under it
    # (the two walls fell either way), a little askew.
    for k, n in enumerate((4, 3, 3, 2)):
        r = 0.12 + w.j(0.015)
        y = r + k * 0.19
        along_x = k % 2 == 0
        for i in range(n):
            off = (i - (n - 1) / 2) * 0.32 + w.j(0.05)
            ln = (1.9 - k * 0.3) * w.rng.uniform(0.75, 1.0)
            skew = w.j(0.4)
            # One end down off the heap, the other up on it.
            drop = w.j(0.1) if k else 0.0
            mid = w.j(0.15)
            if along_x:
                a = (mid - ln / 2, y - drop, off - ln / 2 * math.sin(skew))
                b = (mid + ln / 2, y + drop, off + ln / 2 * math.sin(skew))
            else:
                a = (off - ln / 2 * math.sin(skew), y - drop, mid - ln / 2)
                b = (off + ln / 2 * math.sin(skew), y + drop, mid + ln / 2)
            rod(p, a, b, r, "Timber", segs=10)
            # The end grain, weathered dark, a few millimetres proud.
            d = [b[j] - a[j] for j in range(3)]
            n_ = math.sqrt(sum(c * c for c in d))
            d = [c / n_ * 0.006 for c in d]
            rod(p, tuple(a[j] - d[j] for j in range(3)), a, r * 0.85, "Char", segs=10)
            rod(p, b, tuple(b[j] + d[j] for j in range(3)), r * 0.85, "Char", segs=10)
    # A sheet of the roof's iron slid down onto them, and a few boards.
    p.box((1.1, 0.012, 0.7), (0.0, 0.0, 0.0), "Corrugated",
          xf=T(0.2, 0.72, 0.15) @ RY(0.5) @ RX(0.22) @ RZ(-0.1))
    heap(p, (0.7, 0.65), 22, 6, (1.0, 0.03, 0.14), "Planks", 0.5, 1.0, 0.55)
    return [p]


def rubble_concrete():
    p = Part("static")
    w = Wear(23)
    mound(p, (0.8, 0.62), 0.24, "Concrete")
    heap(p, (0.8, 0.62), 24, 14, (0.42, 0.13, 0.34), "Concrete", 0.6, 1.2, 0.3)
    heap(p, (0.6, 0.5), 25, 12, (0.16, 0.08, 0.12), ["Concrete", "Plaster"], 0.6, 1.1, 0.3)
    # The rebar torn out of the slab, bent up out of the heap.
    for k in range(7):
        a = k / 7 * 2 * math.pi + w.j(0.3)
        r = w.rng.uniform(0.2, 0.6)
        x, z = r * 0.8 * math.cos(a), r * 0.62 * math.sin(a)
        y = (1 - r * r) * 0.3 + 0.05
        lean = w.rng.uniform(0.6, 1.4)
        ln = w.rng.uniform(0.12, 0.3)
        knee = (x + math.cos(a) * 0.12, y + 0.06, z + math.sin(a) * 0.12)
        rod(p, (x, y, z), knee, 0.007, "Rusty", segs=5)
        rod(p, knee, (knee[0] + math.cos(a) * ln * lean, knee[1] + ln, knee[2] + math.sin(a) * ln
                      * lean), 0.007, "Rusty", segs=5)
    return [p]


prop("rubble_planks", rubble_planks)
prop("rubble_brick", rubble_brick)
prop("beam_fallen", beam_fallen)
prop("beam_charred", beam_charred)
prop("bed_burnt", bed_burnt)
prop("table_burnt", table_burnt)
prop("ash_heap", ash_heap)
prop("rubble_logs", rubble_logs)
prop("rubble_concrete", rubble_concrete)
