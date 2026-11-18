"""Mock-ups for the #103 ammo box proposal (not game assets): the 5.45
packet closed (ball, AP, tracer), opened full, opened part-empty, a
generic mixed box, and a few loose rounds. Writes OUT/<name>.glb.

    blender -b -P boxes.py -- OUT
"""

import math
import os
import sys

REPO = "/home/user/extraction"
sys.path.insert(0, os.path.join(REPO, "art", "items"))
sys.path.insert(0, os.path.join(REPO, "art"))
out = sys.argv[sys.argv.index("--") + 1]
# Keep ammo.py's own item loop from building anything on import.
sys.argv = sys.argv[: sys.argv.index("--") + 1] + ["__none__"]

import ammo  # noqa: E402
from _kit import ON_TOP, ragged, text  # noqa: E402
from common import Part, RX, RY, T, export, fillet, make, reset  # noqa: E402

C545 = ammo.C545
SX, SY, SZ = 0.05, 0.035, 0.1


def packet(name, closed, band=None, shown=12, tips=None, wrap="Olive", label="5.45",
           hatch=None):
    """The 5.45 packet as proposed: the type's band right round it, the
    calibre large on top and on the end, opened at the -Z end on the
    noses (`shown` of its 12 visible noses left, front rows first gone)."""
    p = Part("body")
    zt = -0.03
    body = fillet([(-0.0245, -0.017), (0.0245, -0.017), (0.0245, 0.017), (-0.0245, 0.017)],
                  0.003, 2)
    z0 = -SZ / 2 + 0.0005 if closed else zt + 0.0035
    p.prism(body, z0, SZ / 2 - 0.0005, wrap, axis="z", bevel=0.002, segs=1)
    if not closed:
        zp = zt + 0.0035
        ragged(p, 0.047, zp, 0.0115, 0.0162, 0.017, wrap, seed=1)
        ragged(p, 0.047, zp, 0.0105, -0.017, -0.0162, wrap, seed=4)
        for sx_ in (-1, 1):
            ragged(p, 0.032, zp, 0.0105, -0.0004, 0.0004, wrap, seed=3 + sx_,
                   xf=T(sx_ * 0.0241, 0.0, 0.0) @ ammo.RZ(math.pi / 2))
        p.box((0.046, 0.031, 0.0035), (0.0, 0.0, zt + 0.00175), "Black")
        k = 0
        # Taken from the top row first, left to right: what's left stands
        # in the lower rows, the gap growing from the top.
        for j in (2, 1, 0):
            for i in range(4):
                idx = (2 - j) * 4 + i
                if idx >= 12 - shown:
                    x = -0.0158 + i * 0.0105
                    y = -0.0105 + j * 0.0105
                    tip = tips[k % len(tips)] if tips else None
                    ammo.nose(p, C545, -0.028, T(x, y, -SZ / 2 + C545[2]), segs=10, tip=tip)
                    k += 1
    # The type's band right round the packet, near the open end: seen from
    # any side, the same colour as the tips.
    zb = 0.006
    if band or hatch:
        mats = hatch or [band]
        n = 8 if hatch else 1
        w = 0.0512 / n
        for s in range(n):
            m = mats[s % len(mats)]
            x = -0.0256 + w * (s + 0.5)
            p.box((w, 0.0006, 0.009), (x, 0.0175, zb), m, segs=1)
            p.box((w, 0.0006, 0.009), (x, -0.0175, zb), m, segs=1)
        for sx_ in (-1, 1):
            p.box((0.0006, 0.0356, 0.009), (sx_ * 0.0253, 0.0, zb), mats[0], segs=1)
    # The calibre large on top and on the end, read from a step away.
    text(p, label, 0.011, "Black", T(0.0, 0.0177, 0.028) @ ON_TOP, depth=0.0004)
    if closed:
        text(p, label, 0.009, "Black", T(0.0, 0.0, -SZ / 2) @ RY(math.pi), depth=0.0004)
    return [p]


def handful():
    """Five loose rounds lying fanned, as poured into a palm and set down."""
    p = Part("body")
    for k, tip in enumerate([None, "ap", None, "tracer", None]):
        a = -0.35 + k * 0.17
        ammo.cartridge(p, C545, T(-0.03 + k * 0.014, 0.005, 0.0) @ RY(a) @ T(0, 0, C545[2] / 2),
                       segs=12, tip=tip)
    return [p]


MOCKS = {
    "a_closed_ball": lambda: packet("a", True),
    "b_closed_ap": lambda: packet("b", True, band="Black"),
    "c_closed_tracer": lambda: packet("c", True, band="Leaf"),
    "d_open_full_ap": lambda: packet("d", False, band="Black", tips=["ap"]),
    "e_open_part_ap": lambda: packet("e", False, band="Black", shown=5, tips=["ap"]),
    "f_mixed": lambda: packet("f", False, wrap="Rope", label="5.45 MIX", shown=9,
                              tips=["ap", None, "tracer", "sub", None, "ap", None, "tracer", None],
                              hatch=["Black", "Leaf", "Blue"]),
    "g_handful": handful,
}

for name, build in MOCKS.items():
    reset()
    parts = build()
    for prt in parts:
        make(prt)
    export(os.path.join(out, f"{name}.glb"))
    print("wrote", name)
