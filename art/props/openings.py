"""Door leaves and windows for the buildings' openings
(`Builder::wall` in src/world/buildings.rs). Writes
assets/models/props/<name>.glb:

- `door_leaf`     a ledged and braced plank door, 0.91 x 2.03 (the leaf
                  of every `door()`), in door space: hinge line at x 0, the
                  leaf along +X, its bottom at y 0.01, centred on z 0 in
                  0.05; strap hinges and a handle on the -Z face, ledges, a
                  brace and a knob on the +Z face. The game stretches it to
                  a leaf of another size.
- `window_sash`   a 1.0 x 1.1 window (a `window()` opening) in window
                  space: centred on the opening, the wall's centre plane at
                  z 0, 0.07 deep: a frame, two casements and a fortochka
                  (the little vent pane) over them, glazed. Wide windows
                  are rows of these, stretched.
- `window_casing` carved board surrounds (nalichniki) for the outside of a
                  timber house's window, in the same space but standing on
                  the wall face at z 0 (-Z out): side boards, a sill board
                  with a fretted apron, a pointed head board.

Materials: Planks, Timber, Iron, Glass, Painted, Cream.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import Wear, board, bolt, prop, rod  # noqa: E402
from common import RX, Part, T, fillet  # noqa: E402

UP_Z = RX(-math.pi / 2)  # a cylinder's axis (Z) -> +Y

DW, DH = 0.91, 2.03


def door_leaf():
    w = Wear(71)
    p = Part("static")
    y0, y1 = 0.01, 0.01 + DH
    # Six boards, V-jointed, the bottoms a little ragged.
    n, gap = 6, 0.003
    bw = (DW - gap * (n - 1)) / n
    for k in range(n):
        x = bw / 2 + k * (bw + gap)
        trim = abs(w.j(0.006))
        board(p, (bw, DH - trim, 0.03), (x, (y0 + trim + y1) / 2, -0.01), "Planks", w,
              bevel=0.004, tilt=0.0)
    # Ledges and the brace on the back face (+Z), the brace rising from
    # the hinge side's bottom ledge.
    ys = (y0 + 0.3, y1 - 0.3)
    for y in ys:
        board(p, (DW - 0.1, 0.12, 0.025), (DW / 2, y, 0.0175), "Planks", bevel=0.004)
    a = (0.12, ys[0] + 0.06)
    b = (DW - 0.12, ys[1] - 0.06)
    L = math.hypot(b[0] - a[0], b[1] - a[1])
    ang = math.atan2(b[1] - a[1], b[0] - a[0])
    board(p, (L - 0.06, 0.1, 0.025), ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, 0.0135), "Planks",
          roll=ang, bevel=0.004)
    # Strap hinges on the front (-Z), pinned on pintles at the hinge line.
    for y in ys:
        p.box((0.52, 0.045, 0.004), (0.26, y, -0.027), "Iron", bevel=0.001, segs=1)
        rod(p, (0.0, y - 0.05, -0.034), (0.0, y + 0.05, -0.034), 0.011, "Iron", segs=8)
        for x in (0.12, 0.26, 0.4):
            bolt(p, (x, y, -0.029), (0, 0, -1), r=0.008, h=0.004)
    # A bent bar handle and a latch plate on the front, a knob behind.
    hx, hy = DW - 0.1, 1.0
    p.box((0.05, 0.2, 0.004), (hx, hy, -0.027), "Iron", bevel=0.001, segs=1)
    for y in (hy - 0.07, hy + 0.07):
        rod(p, (hx, y, -0.029), (hx, y, -0.065), 0.007, "Iron", segs=6)
    rod(p, (hx, hy - 0.077, -0.065), (hx, hy + 0.077, -0.065), 0.009, "Iron", segs=8)
    rod(p, (hx, hy, 0.0), (hx, hy, 0.06), 0.008, "Iron", segs=6)
    p.sphere(0.022, (hx, hy, 0.07), "Iron", segs=10, rings=6)
    # A keyhole escutcheon under the handle.
    p.cyl(0.018, -0.029, -0.025, "Iron", segs=10, xf=T(hx, hy - 0.16, 0.0))
    return [p]


SW, SH = 1.0, 1.1


def window_sash():
    p = Part("static")
    # The frame round the opening.
    f, d = 0.06, 0.07
    for sx in (-1, 1):
        p.box((f, SH, d), (sx * (SW / 2 - f / 2), 0.0, 0.0), "Timber", bevel=0.004, segs=1)
    for sy in (-1, 1):
        p.box((SW - 2 * f + 0.004, f, d), (0.0, sy * (SH / 2 - f / 2), 0.0), "Timber",
              bevel=0.004, segs=1)
    # Transom and mullion: two casements below, a vent pane and a fixed
    # pane above.
    ty = SH / 2 - 0.3
    p.box((SW - 2 * f + 0.004, 0.05, d - 0.01), (0.0, ty, 0.0), "Timber", bevel=0.003, segs=1)
    p.box((0.05, ty - (-SH / 2 + f) + 0.004, d - 0.02), (0.0, (ty + (-SH / 2 + f)) / 2, 0.0),
          "Timber", bevel=0.003, segs=1)
    p.box((0.04, SH / 2 - f - ty - 0.025 + 0.004, d - 0.02),
          (0.1, (SH / 2 - f + ty + 0.025) / 2, 0.0), "Timber", bevel=0.003, segs=1)
    # Casement sashes inside the frame, set back, each a bar across.
    def sash(x0, x1, y0, y1, bar=True):
        s = 0.04
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        for xx in (x0 + s / 2, x1 - s / 2):
            p.box((s, y1 - y0, 0.04), (xx, cy, -0.01), "Painted")
        for yy in (y0 + s / 2, y1 - s / 2):
            p.box((x1 - x0 - 2 * s + 0.004, s, 0.04), (cx, yy, -0.01), "Painted")
        if bar:
            p.box((x1 - x0 - 2 * s + 0.004, 0.025, 0.03), (cx, cy + 0.06, -0.01), "Painted")
        # The pane, in its middle.
        p.box((x1 - x0 - 2 * s + 0.006, y1 - y0 - 2 * s + 0.006, 0.004), (cx, cy, -0.01),
              "Glass")
    xin, yin = SW / 2 - f, SH / 2 - f
    sash(-xin, -0.025, -yin, ty - 0.025)
    sash(0.025, xin, -yin, ty - 0.025)
    sash(-xin, 0.08, ty + 0.025, yin, bar=False)
    sash(0.12, xin, ty + 0.025, yin, bar=False)
    # Latch handles on the casements' meeting stiles.
    for sx in (-1, 1):
        p.box((0.012, 0.06, 0.008), (sx * 0.04, -0.05, -0.034), "Iron")
    return [p]


def window_casing():
    w = Wear(73)
    p = Part("static")
    # Out from the wall face (z 0) toward -Z.
    t, cw = 0.025, 0.1
    z = -t / 2
    for sx in (-1, 1):
        board(p, (cw, SH + 0.1, t), (sx * (SW / 2 + cw / 2 - 0.01), 0.0, z), "Cream", w,
              bevel=0.004)
    # The sill board and a fretted apron under it.
    board(p, (SW + 2 * cw + 0.08, 0.04, 0.09), (0.0, -SH / 2 - 0.07, -0.045), "Cream",
          bevel=0.005)
    apron = [(-0.62, -SH / 2 - 0.09)]
    n = 7
    for k in range(n + 1):
        x = -0.62 + 1.24 * k / n
        apron.append((x, -SH / 2 - 0.09 - (0.12 if k % 2 == 0 else 0.07)))
    apron.append((0.62, -SH / 2 - 0.09))
    p.prism(apron, -t * 0.8 - 0.004, -0.004, "Cream", axis="z")
    # The head: a cornice board and a pointed pediment over it.
    board(p, (SW + 2 * cw + 0.1, 0.05, 0.07), (0.0, SH / 2 + 0.075, -0.035), "Cream",
          bevel=0.005)
    ped = fillet([(-0.66, SH / 2 + 0.1), (0.66, SH / 2 + 0.1), (0.0, SH / 2 + 0.34)],
                 [0.0, 0.0, 0.02], 2)
    p.prism(ped, -t - 0.004, -0.004, "Cream", axis="z")
    # A carved sun in the pediment, and rosettes at the head's corners.
    p.cyl(0.05, -t - 0.012, -t - 0.004, "Painted", segs=12, xf=T(0.0, SH / 2 + 0.19, 0.0))
    for sx in (-1, 1):
        p.cyl(0.03, -t - 0.008, -t, "Painted", segs=10,
              xf=T(sx * (SW / 2 + cw / 2 - 0.01), SH / 2 + 0.02, 0.0))
    return [p]


prop("door_leaf", door_leaf)
prop("window_sash", window_sash)
prop("window_casing", window_casing)

def door_metal():
    """A steel door for masonry and metal buildings, in door space like
    `door_leaf`: a sheet leaf in an angle frame, pressed stiffening ribs on
    the front (-Z), barrel hinges at the hinge line, a lever handle and
    lock, a kick plate worn bright."""
    p = Part("static")
    y0, y1 = 0.01, 0.01 + DH
    p.box((DW, DH, 0.04), (DW / 2, (y0 + y1) / 2, 0.0), "Painted", bevel=0.004, segs=1)
    # Ribs pressed out of the front face: a frame and a cross.
    rz = -0.022
    ys = (y0 + 0.27, (y0 + y1) / 2, y1 - 0.1)
    for y in ys:
        p.box((DW - 0.11, 0.05, 0.006), (DW / 2, y, rz), "Painted", bevel=0.002, segs=1)
    # Uprights between them, stopping a millimetre short (no shared faces).
    for ya, yb in zip(ys, ys[1:]):
        for x in (0.08, DW - 0.08):
            h = yb - ya - 0.052
            p.box((0.05, h, 0.006), (x, (ya + yb) / 2, rz), "Painted", bevel=0.002, segs=1)
    # Barrel hinges on the hinge line.
    for y in (y0 + 0.25, y1 - 0.25):
        p.cyl(0.016, y - 0.07, y + 0.07, "Iron", segs=10, xf=T(0.0, 0.0, -0.01) @ UP_Z)
    # Lever handle and the lock cylinder, both faces.
    hx, hy = DW - 0.09, 1.0
    for s_ in (-1, 1):
        z = s_ * 0.02
        p.box((0.04, 0.16, 0.006), (hx, hy - 0.03, z + s_ * 0.003), "Zinc", bevel=0.001, segs=1)
        rod(p, (hx, hy, z), (hx, hy, z + s_ * 0.05), 0.008, "Zinc", segs=8)
        rod(p, (hx, hy, z + s_ * 0.05), (hx - 0.12, hy, z + s_ * 0.05), 0.009, "Zinc", segs=8)
        p.cyl(0.012, 0.0, 0.006, "Brass", segs=10, xf=T(hx, hy - 0.09, z + s_ * 0.006))
    # A kick plate along the bottom of the front.
    p.box((DW - 0.06, 0.18, 0.004), (DW / 2, y0 + 0.11, -0.022), "Zinc")
    return [p]


prop("door_metal", door_metal)
