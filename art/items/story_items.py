"""The things the jobs and the first hour hand you or send you for, and a
few tools and kit that had no model: each was the item placeholder's plain
slab until now (`models::tests::no_item_falls_back_to_the_placeholder`).
Writes assets/models/items/<id>.glb, in item space (_kit.py), centred on
its `dims` box.

- `camera`             a Zorki rangefinder: black leatherette body, chrome
                       top plate, rangefinder windows, the lens forward
- `relay_box`          Lev's relay: a steel box, a green lamp, a toggle, an
                       aerial lead coiled off its side
- `holm_watch`         a plain steel watch face up on a worn strap
- `hajek_tin`          a tobacco tin, M.H. scratched on its lid
- `scour_key`          a long box spanner with a T-bar, a cut chain on it
- `pump_log`           a board-backed ledger swollen with water
- `deputy_lamp`        a brass safety lamp: glass, gauze, bonnet, hook and
                       a tally on it
- `charging_manifest`  a clipboard of pencilled columns
- `intake_ledger`      a canvas-bound ledger stencilled GATE-M INTAKE
- `hana_letter`        a sealed envelope, a line of handwriting on it
- `spanner`            a ring spanner
- `work_boots`         a pair of black work boots, laces knotted together
- `can_suppressor`     a food tin on a hose clamp and a muzzle coupling, as
                       `pbs_suppressor` (its back face on the muzzle)
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import (ALONG_X, LIFT, ON_TOP, UP, centred, item, ring, text, transform,  # noqa: E402
                  tube_path, wanted)
from common import RX, RY, RZ, T, Part  # noqa: E402


def camera():
    p = Part("body")
    # Body 0.135 wide, its leatherette front and back, a chrome top plate.
    W, H, D = 0.135, 0.062, 0.032
    y0 = -0.045
    p.box((W, H, D), (0.0, y0 + H / 2, 0.019), "Feather", bevel=0.006, segs=2)
    p.box((W - 0.004, 0.017, D + 0.002), (0.0, y0 + H + 0.0085, 0.019), "Steel", bevel=0.004,
          segs=2)
    # The rangefinder's two windows on the top plate's front, the
    # viewfinder's eyepiece at the back.
    for x in (-0.045, 0.012):
        p.box((0.014, 0.009, 0.002), (x, y0 + H + 0.008, 0.019 - D / 2 - 0.0008), "Glass")
    p.box((0.012, 0.008, 0.002), (-0.045, y0 + H + 0.008, 0.019 + D / 2 + 0.0008), "Glass")
    # Knobs on top: the wind, the shutter speeds, the button, the rewind.
    for x, r, h in ((0.05, 0.009, 0.006), (0.03, 0.008, 0.004), (-0.05, 0.008, 0.005)):
        p.cyl(r, y0 + H + 0.017, y0 + H + 0.017 + h, "Steel", segs=16, xf=T(x, 0, 0) @ UP,
              chamfer=0.001)
    p.cyl(0.003, y0 + H + 0.017, y0 + H + 0.021, "Black", segs=10, xf=T(0.015, 0, 0.012) @ UP)
    # The lens: chrome barrel, black focusing ring, the glass.
    ly = y0 + H / 2
    p.cyl(0.021, 0.019 - D / 2 - 0.004, 0.019 - D / 2 + 0.001, "Steel", segs=24,
          xf=T(0.0, ly, 0.0))
    p.cyl(0.018, -0.0318, 0.019 - D / 2 - 0.003, "Steel", segs=24, xf=T(0.0, ly, 0.0))
    p.cyl(0.0195, -0.022, -0.012, "Black", segs=24, xf=T(0.0, ly, 0.0))
    p.cyl(0.013, -0.035, -0.0318, "Lens", segs=20, xf=T(0.0, ly, 0.0))
    # Strap lugs.
    for sx in (-1, 1):
        p.box((0.003, 0.008, 0.006), (sx * (W / 2 + 0.0012), y0 + H - 0.008, 0.019), "Steel")
    return centred([p])


def relay_box():
    p = Part("body")
    W, H, D = 0.15, 0.08, 0.11
    y0 = -0.05
    p.box((W, H, D), (0.0, y0 + H / 2, 0.0), "Olive", bevel=0.004, segs=1)
    # The lid's seam and its four screws.
    p.box((W + 0.002, 0.004, D + 0.002), (0.0, y0 + H - 0.012, 0.0), "Iron", segs=1)
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.cyl(0.0035, y0 + H, y0 + H + 0.0015, "Steel", segs=10,
                  xf=T(sx * (W / 2 - 0.012), 0, sz * (D / 2 - 0.012)) @ UP)
    # One green lamp in a chrome bezel, a toggle switch beside it.
    p.cyl(0.0085, y0 + H, y0 + H + 0.004, "Steel", segs=16, xf=T(0.03, 0, -0.02) @ UP)
    p.lathe([(0.0, y0 + H + 0.004), (0.0065, y0 + H + 0.004), (0.006, y0 + H + 0.009),
             (0.0, y0 + H + 0.011)], "Leaf", segs=16, xf=T(0.03, 0, -0.02) @ UP)
    p.cyl(0.006, y0 + H, y0 + H + 0.005, "Iron", segs=12, xf=T(-0.01, 0, -0.02) @ UP)
    p.cyl(0.0018, 0.0, 0.016, "Steel", segs=8,
          xf=T(-0.01, y0 + H + 0.005, -0.02) @ RX(-math.pi / 2 + 0.4))
    # Stencilled number on the lid.
    text(p, "R-7", 0.012, "Cream", T(-0.035, y0 + H + LIFT, 0.025) @ ON_TOP)
    # The aerial lead out of a gland on the side, coiled on the lid.
    p.cyl(0.006, W / 2, W / 2 + 0.006, "Black", segs=12, xf=T(0, y0 + H - 0.025, 0.02) @ ALONG_X)
    pts = [(W / 2 + 0.006, y0 + H - 0.025, 0.02), (W / 2 + 0.004, y0 + H + 0.004, 0.035)]
    for k in range(13):
        a = k * 0.55
        pts.append((0.035 + 0.025 * math.cos(a), y0 + H + 0.004, 0.025 + 0.022 * math.sin(a)))
    tube_path(p, pts, 0.0025, "Black", sides=6, closed=False)
    return centred([p])


def holm_watch():
    p = Part("body")
    # Face up on its strap, laid out flat.
    R = 0.0165
    y0 = -0.006
    # Short stubs of strap: a watch, not a belt (the catalogue's 4 cm).
    for sz in (-1, 1):
        p.box((0.018, 0.003, 0.008), (0.0, y0 + 0.0015, sz * (R + 0.002)), "Leather",
              bevel=0.001, segs=1)
    p.box((0.008, 0.0035, 0.003), (0.0, y0 + 0.0018, R + 0.0045), "Steel", segs=1)
    p.lathe([(0.0, y0), (R, y0), (R + 0.0008, y0 + 0.002), (R + 0.0008, y0 + 0.008),
             (R - 0.0008, y0 + 0.009), (0.0, y0 + 0.009)], "Steel", segs=28, xf=UP)
    p.cyl(R - 0.0016, y0 + 0.009, y0 + 0.0094, "Cream", segs=28, xf=UP)
    for k in range(12):
        L = 0.003 if k % 3 == 0 else 0.0016
        p.box((0.0008, 0.0003, L), (0.0, y0 + 0.0096, -(R - 0.0035) + L / 2), "Black",
              xf=RY(2 * math.pi * k / 12))
    yh = y0 + 0.0098
    p.box((0.0012, 0.0004, 0.0075), (0.0, yh, -0.0035), "Black", xf=RY(math.radians(-60)))
    p.box((0.0009, 0.0004, 0.011), (0.0, yh + 0.0003, -0.005), "Black", xf=RY(math.radians(150)))
    p.cyl(0.0018, y0 + 0.004, y0 + 0.007, "Steel", segs=10, xf=T(R + 0.0016, 0, 0) @ ALONG_X)
    return centred([p])


def hajek_tin():
    p = Part("body")
    W, H, D = 0.088, 0.024, 0.058
    y0 = -0.0125
    p.box((W - 0.002, H - 0.006, D - 0.002), (0.0, y0 + (H - 0.006) / 2, 0.0), "Tarnish",
          bevel=0.006, segs=3)
    p.box((W, 0.007, D), (0.0, y0 + H - 0.0035, 0.0), "Tarnish", bevel=0.005, segs=3)
    # M.H., scratched through the paint to the tin.
    text(p, "M.H.", 0.016, "Zinc", T(0.0, y0 + H + LIFT, 0.004) @ ON_TOP)
    for k in range(5):
        p.box((0.02, 0.0004, 0.0006), (-0.02 + 0.006 * k, y0 + H + 0.0006, -0.016), "Zinc",
              xf=T(-0.02 + 0.006 * k, 0, -0.016) @ RY(0.4 + 0.3 * k) @ T(0.02 - 0.006 * k, 0, 0.016))
    return centred([p])


def scour_key():
    p = Part("body")
    L = 0.6
    # The shaft, the square socket at its foot, the T-bar through its head.
    p.cyl(0.011, -L / 2 + 0.05, L / 2 - 0.02, "Iron", segs=14)
    p.box((0.03, 0.03, 0.06), (0.0, 0.0, -L / 2 + 0.03), "Iron", bevel=0.003, segs=1)
    p.box((0.016, 0.016, 0.004), (0.0, 0.0, -L / 2 - 0.0005), "Dark", segs=1)
    p.cyl(0.009, -0.024, 0.024, "Iron", segs=12, xf=T(0, 0, L / 2 - 0.012) @ ALONG_X)
    p.cyl(0.0105, -0.0245, -0.018, "Rusty", segs=12, xf=T(0, 0, L / 2 - 0.012) @ ALONG_X)
    p.cyl(0.0105, 0.018, 0.0245, "Rusty", segs=12, xf=T(0, 0, L / 2 - 0.012) @ ALONG_X)
    # A cut chain hanging off the handle: four links, the last one open.
    for k in range(4):
        z = L / 2 - 0.035 - k * 0.022
        rot = RZ(math.pi / 2) if k % 2 else RZ(0.0)
        ring(p, 0.008, 0.0022, "Steel", xf=T(0.0, -0.012, z) @ rot @ RY(math.pi / 2), sides=6)
    # On its box's floor: it's slimmer than the box is tall.
    parts = centred([p])
    lo = min(v.co.y for v in p.bm.verts)
    return transform(parts, T(0, -0.025 - lo, 0))


def ledger(p, W, H, D, cover, swell=0.0):
    """A bound ledger lying flat: two covers on a block of pages, a spine on
    its -X side; `swell` puffs the pages' fore-edge (a soaked book)."""
    y0 = -H / 2
    t = 0.004
    p.box((W, t, D), (0.0, y0 + t / 2, 0.0), cover, bevel=0.0015, segs=1)
    p.box((W, t, D), (0.0, y0 + H - t / 2, 0.0), cover, bevel=0.0015, segs=1)
    ph = H - 2 * t
    n = 5
    for k in range(n):
        f = 1.0 - abs(k - (n - 1) / 2) / ((n - 1) / 2)
        w = W - 0.008 + swell * f
        p.box((w, ph / n + 0.0002, D - 0.008), (swell * f / 2 - 0.002, y0 + t + ph * (k + 0.5) / n,
              0.0), "Paper", segs=1)
    # (1.5 mm proud of both covers: flush, its faces fought theirs.)
    p.box((0.008, H + 0.003, D + 0.003), (-W / 2 + 0.002, 0.0, 0.0), cover, bevel=0.003, segs=1)


def pump_log():
    p = Part("body")
    ledger(p, 0.14, 0.028, 0.2, "Boards", swell=0.008)
    # Water's tide marks across the board.
    for k, z in enumerate((-0.05, 0.02, 0.07)):
        p.box((0.11, 0.0004, 0.004 + 0.002 * k), (0.004, 0.014 + LIFT, z), "Char", segs=1)
    return centred([p])


def intake_ledger():
    p = Part("body")
    ledger(p, 0.24, 0.05, 0.32, "Canvas")
    text(p, "GATE-M", 0.024, "Black", T(0.01, 0.025 + LIFT, -0.05) @ ON_TOP)
    text(p, "INTAKE", 0.024, "Black", T(0.01, 0.025 + LIFT, 0.0) @ ON_TOP)
    return centred([p])


def deputy_lamp():
    p = Part("body")
    y0 = -0.125
    # The brass oil vessel, the glass over it, the gauze chimney above,
    # three pillars round them, the bonnet and the hook.
    p.lathe([(0.0, y0), (0.042, y0), (0.045, y0 + 0.006), (0.045, y0 + 0.05), (0.04, y0 + 0.055),
             (0.0, y0 + 0.055)], "Brass", segs=28, xf=UP)
    p.lathe([(0.03, y0 + 0.055), (0.031, y0 + 0.055), (0.031, y0 + 0.11), (0.03, y0 + 0.11)],
            "Glass", segs=24, xf=UP, closed=True)
    p.cyl(0.006, y0 + 0.06, y0 + 0.075, "Steel", segs=10, xf=UP)
    p.lathe([(0.0, y0 + 0.11), (0.036, y0 + 0.11), (0.036, y0 + 0.118), (0.0, y0 + 0.118)],
            "Brass", segs=24, xf=UP)
    p.lathe([(0.026, y0 + 0.118), (0.027, y0 + 0.118), (0.022, y0 + 0.19), (0.021, y0 + 0.19)],
            "Iron", segs=20, xf=UP, closed=True)
    for k in range(3):
        a = 2 * math.pi * k / 3
        p.cyl(0.0035, y0 + 0.055, y0 + 0.19, "Brass", segs=8,
              xf=T(0.039 * math.cos(a), 0, 0.039 * math.sin(a)) @ UP)
    p.lathe([(0.0, y0 + 0.19), (0.042, y0 + 0.19), (0.03, y0 + 0.21), (0.012, y0 + 0.22),
             (0.0, y0 + 0.222)], "Brass", segs=24, xf=UP)
    ring(p, 0.012, 0.0025, "Brass", xf=T(0, y0 + 0.234, 0) @ RY(0.0), sides=6)
    # The tally on its own small ring off the hook: a brass disc, no name.
    ring(p, 0.004, 0.001, "Brass", xf=T(0.012, y0 + 0.222, 0) @ RY(math.pi / 2), sides=5)
    p.cyl(0.009, -0.0008, 0.0008, "Brass", segs=16,
          xf=T(0.014, y0 + 0.21, 0.0) @ RY(math.pi / 2) @ RX(math.pi / 2) @ RX(math.pi / 2))
    return centred([p])


def charging_manifest():
    p = Part("body")
    y0 = -0.005
    p.box((0.21, 0.004, 0.3), (0.0, y0 + 0.002, 0.0), "Wood", bevel=0.001, segs=1)
    p.box((0.19, 0.0012, 0.27), (0.0, y0 + 0.0046, 0.012), "Paper", segs=1)
    # Columns of pencilled figures: rows of short dashes.
    for c in range(4):
        for r in range(14):
            w = 0.022 + 0.008 * ((r * 7 + c * 3) % 3)
            p.box((w, 0.0004, 0.002), (-0.065 + c * 0.045, y0 + 0.0054,
                                       -0.09 + r * 0.016), "Dark", segs=1)
    # The clip: a steel plate, its spring roll and lever.
    p.box((0.1, 0.004, 0.03), (0.0, y0 + 0.006, -0.135), "Steel", bevel=0.001, segs=1)
    p.cyl(0.004, -0.04, 0.04, "Steel", segs=10, xf=T(0, y0 + 0.008, -0.148) @ ALONG_X)
    return centred([p])


def hana_letter():
    p = Part("body")
    y0 = -0.002
    p.box((0.11, 0.002, 0.16), (0.0, y0 + 0.001, 0.0), "Paper", segs=1)
    # The flap's edge, a red wax seal on its point.
    for sx in (-1, 1):
        p.box((0.0006, 0.0006, 0.07), (sx * 0.028, y0 + 0.0024, 0.045), "Pale",
              xf=T(sx * 0.028, 0, 0.045) @ RY(sx * 0.66) @ T(-sx * 0.028, 0, -0.045))
    p.cyl(0.008, y0 + 0.002, y0 + 0.0036, "Red", segs=14, xf=T(0.0, 0, 0.018) @ UP)
    # The careful hand: two lines of ink on the front.
    for k, w in enumerate((0.07, 0.05)):
        p.box((w, 0.0004, 0.0016), (-0.005, y0 + 0.0022, -0.035 - 0.012 * k), "Blued", segs=1)
    return centred([p])


def spanner():
    p = Part("body")
    # A flat bar between two rings, the rings cranked a little.
    p.box((0.014, 0.006, 0.22), (0.0, 0.0, 0.0), "Steel", bevel=0.002, segs=1)
    for sz, r in ((-1, 0.018), (1, 0.015)):
        p.tube(r, r - 0.0075, -0.004, 0.004, "Steel", segs=24,
               xf=T(0, 0, sz * (0.11 + r - 0.004)) @ RX(math.pi / 2 + sz * 0.15))
    text(p, "22", 0.006, "Dark", T(0.0, 0.003 + LIFT, -0.05) @ ON_TOP)
    return centred([p])


def boot(p, x, mirror):
    """One black work boot standing, its toe toward -Z."""
    y0 = -0.08
    p.box((0.1, 0.022, 0.3), (x, y0 + 0.011, 0.005), "Rubber", bevel=0.008, segs=2)
    # The foot: a rounded toe and the vamp.
    # Black leather: the glossy near-black, not the glove's tan.
    p.box((0.092, 0.06, 0.2), (x, y0 + 0.052, -0.035), "Feather", bevel=0.025, segs=3)
    p.box((0.095, 0.13, 0.11), (x, y0 + 0.087, 0.09), "Feather", bevel=0.02, segs=3)
    # The tongue and the laces up the front of the shaft.
    for k in range(5):
        y = y0 + 0.07 + k * 0.016
        p.box((0.06, 0.004, 0.004), (x, y, 0.0355), "Cream", segs=1)
    p.box((0.05, 0.012, 0.03), (x, y0 + 0.155, 0.08), "Feather", bevel=0.004, segs=1)


def work_boots():
    p = Part("body")
    boot(p, -0.065, False)
    boot(p, 0.065, True)
    # The laces knotted together between the shafts.
    tube_path(p, [(-0.03, 0.06, 0.06), (0.0, 0.07, 0.07), (0.03, 0.06, 0.06)], 0.003, "Cream",
              sides=5, closed=False)
    return centred([p])


def can_suppressor():
    p = Part("body")
    L = 0.065
    # A food tin pushed over the muzzle and clamped there: its back 45 mm
    # an open sleeve the muzzle and its front sight go into, a hose clamp
    # round it; then the tin packed with washers, its rolled rims and ribs,
    # along -Z.
    zs = L - 0.045
    p.tube(0.0375, 0.0345, zs, L, "Zinc", segs=28)
    p.tube(0.0388, 0.0375, L - 0.016, L - 0.006, "Steel", segs=28)
    p.box((0.008, 0.007, 0.01), (0.0, 0.0405, L - 0.011), "Steel", segs=1)
    p.cyl(0.0355, -L + 0.004, zs, "Zinc", segs=28)
    p.tube(0.0345, 0.012, zs - 0.001, zs + 0.001, "Steel", segs=24)
    for z in (-L + 0.004, zs - 0.004):
        p.tube(0.0375, 0.0345, z - 0.002, z + 0.002, "Zinc", segs=28)
    for k in range(2):
        z = -L + 0.018 + k * 0.016
        p.tube(0.0368, 0.0352, z - 0.001, z + 0.001, "Zinc", segs=28)
    # The front lid, punched through with a ragged hole.
    p.tube(0.034, 0.008, -L + 0.002, -L + 0.004, "Zinc", segs=28)
    p.tube(0.0085, 0.0075, -L, -L + 0.004, "Char", segs=12)
    # A slot snipped along its underside, end to end, for a fixed
    # bayonet's spike to run through (the Mosin's lies 21 mm under the bore).
    slot = Part("cut")
    slot.box((0.013, 0.03, 2 * L + 0.01), (0.0, -0.0285, 0.0), "Char", segs=1)
    p.cut(slot)
    return [p]


for item_id, fn in (("camera", camera), ("relay_box", relay_box), ("holm_watch", holm_watch),
                    ("hajek_tin", hajek_tin), ("scour_key", scour_key),
                    ("pump_log", pump_log), ("deputy_lamp", deputy_lamp),
                    ("intake_ledger", intake_ledger),
                    ("charging_manifest", charging_manifest), ("hana_letter", hana_letter),
                    ("spanner", spanner), ("work_boots", work_boots),
                    ("can_suppressor", can_suppressor)):
    if wanted(item_id):
        item(item_id, fn)
