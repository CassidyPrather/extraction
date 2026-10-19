"""Bench materials: scrap metal, salvaged electronics, polymer stock, a
powder tin and brass cases. Writes assets/models/items/<id>.glb for each
(item space: see _kit.py). None has `dims`, so each sits on the floor of
a cube of side cbrt(bulk / 1000).

Each is a handful of the stuff, bundled the way a scavenger would keep
it: offcuts wired together, boards stacked with their wires, bar stock
tied with string, a labelled tin, a tray of empties.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, FRONT, UP, centred, cube, item, panel, ring, text, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, arc, fillet


def hex_nut(p, r, h, xf, mat="Steel"):
    """A hex nut lying flat (then `xf`)."""
    p.lathe([(r * 0.45, 0.0), (r, 0.0), (r, h), (r * 0.45, h)], mat, segs=6, xf=xf @ UP,
            closed=True)


# --- Scrap metal ----------------------------------------------------------------------------

def scrap_metal():
    s = cube(0.4)
    yb = -s / 2
    p = Part("body")
    # A bundle of offcuts wired together: a length of angle iron, a bent
    # strip of painted plate, a stub of rebar, nuts and a bolt, rust on it.
    leg, t = 0.014, 0.0028
    ang = [(0.0, 0.0), (leg, 0.0), (leg, t), (t, t), (t, leg), (0.0, leg)]
    p.prism([(z - 0.012, yb + y) for z, y in ang], -0.034, 0.033, "Iron", axis="x")
    p.prism([(z + 0.004, yb + y) for z, y in ang], -0.03, 0.031, "Iron", axis="x",
            xf=T(0, 0, 0) @ RY(0.08))
    # The plate: painted olive, bent up at one end, paint gone at the edges.
    xf = T(-0.004, yb + 0.0175, 0.0) @ RY(-0.35) @ RZ(0.06)
    p.box((0.044, 0.003, 0.03), (0.0, 0.0, 0.0), "Paint", xf=xf)
    p.box((0.02, 0.003, 0.03), (0.0, 0.0, 0.0), "Paint",
          xf=xf @ T(0.022, 0.0, 0.0) @ RZ(0.6) @ T(0.0095, 0.0, 0.0))
    p.box((0.044, 0.0032, 0.004), (0.0, 0.0, -0.0135), "Steel", xf=xf)
    for x, z, w in ((-0.012, 0.006, 0.012), (0.008, -0.004, 0.008)):
        p.box((w, 0.0034, w * 0.7), (x, 0.0, z), "Bakelite", xf=xf)
    # Rebar, ribbed and bent.
    pts = [(-0.033, yb + 0.027, 0.012), (-0.01, yb + 0.025, 0.006), (0.012, yb + 0.026, -0.004),
           (0.026, yb + 0.03, -0.018)]
    tube_path(p, pts, 0.0032, "Iron", sides=6, closed=False)
    for k in range(5):
        t = (k + 0.5) / 5
        x = -0.033 + 0.045 * t
        ring(p, 0.0033, 0.0007, "Iron", xf=T(x, yb + 0.0258, 0.012 - 0.016 * t) @ ALONG_X @
             RX(0.3), segs=8, sides=3)
    # Nuts and a bolt.
    hex_nut(p, 0.0055, 0.004, T(0.022, yb + 0.0028, 0.022))
    hex_nut(p, 0.0045, 0.0035, T(-0.024, yb, -0.024) @ RY(0.4))
    bolt = T(0.008, yb + 0.0175, 0.014) @ RY(1.2)
    p.cyl(0.0028, 0.0, 0.022, "Steel", segs=8, xf=bolt)
    hex_nut(p, 0.005, 0.004, bolt @ RX(math.pi / 2) @ T(0, -0.002, 0))
    # Binding wire twice round the bundle, twisted off on top.
    for x in (-0.018, 0.016):
        tube_path(p, [(x, yb + 0.0006, -0.016), (x, yb + 0.0006, 0.018), (x, yb + 0.022, 0.02),
                      (x, yb + 0.033, 0.0), (x, yb + 0.024, -0.02)], 0.0007, "Steel", sides=4,
                  closed=True)
        tube_path(p, [(x, yb + 0.033, 0.0), (x + 0.004, yb + 0.037, 0.002),
                      (x + 0.007, yb + 0.036, 0.004)], 0.0007, "Steel", sides=4, closed=False)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


# --- Salvaged electronics ---------------------------------------------------------------------

def electronics():
    s = cube(0.3)
    yb = -s / 2
    p = Part("body")
    # Two green circuit boards pulled out of something, chips and cans on
    # them, one stacked on the other on brass standoffs, wires trailing.
    def board(xf, w, d, seed):
        rnd = random.Random(seed)
        p.box((w, 0.0016, d), (0.0, 0.0008, 0.0), "Olive", xf=xf)
        for k in range(5):
            p.box((w - 0.004, 0.0002, 0.0007), (0.0, 0.0017, -d / 2 + 0.004 + k * d / 5.5),
                  "Copper", xf=xf)
        for cx, cz, cw, cd in ((-0.012, -0.008, 0.014, 0.008), (0.008, 0.006, 0.01, 0.01)):
            p.box((cw, 0.0022, cd), (cx, 0.0027, cz), "Black", bevel=0.0003, segs=1, xf=xf)
            for k in range(int(cw / 0.0022)):
                for sz in (-1, 1):
                    p.box((0.0007, 0.0012, 0.0016), (cx - cw / 2 + 0.0015 + k * 0.0022, 0.0021,
                                                     cz + sz * (cd / 2 + 0.0005)), "Steel", xf=xf)
        for k in range(3):
            x, z = rnd.uniform(-w / 2 + 0.005, w / 2 - 0.005), rnd.uniform(-d / 2 + 0.005, 0.0)
            p.cyl(0.0034, 0.0016, 0.0105, "Blued", segs=10, xf=xf @ T(x, 0, z) @ UP)
            p.cyl(0.0032, 0.0105, 0.011, "Steel", segs=10, xf=xf @ T(x, 0, z) @ UP)
        for k in range(4):
            xf2 = xf @ T(0.012 - k * 0.0035, 0.0028, d / 2 - 0.006)
            p.cyl(0.0011, -0.003, 0.003, "Cream", segs=6, xf=xf2)
            p.cyl(0.00115, -0.0006, 0.0006, "Red", segs=6, xf=xf2)
        # Edge connector fingers.
        for k in range(8):
            p.box((0.0018, 0.0003, 0.005), (-w / 2 + 0.006 + k * 0.0032, 0.0017, d / 2 - 0.0022),
                  "Brass", xf=xf)
    board(T(-0.002, yb + 0.003, 0.002), 0.056, 0.05, 1)
    for x, z in ((-0.026, -0.021), (0.022, -0.021), (-0.026, 0.023), (0.022, 0.023)):
        p.cyl(0.0016, yb, yb + 0.003, "Brass", segs=6, xf=T(x, 0, z) @ UP)
        p.cyl(0.0016, yb + 0.0046, yb + 0.0175, "Brass", segs=6, xf=T(x, 0, z) @ UP)
    board(T(0.003, yb + 0.0175, -0.003) @ RY(0.18) @ RZ(0.03), 0.05, 0.044, 2)
    # A heat sink with its fins, and a little transformer.
    xf = T(0.008, yb + 0.0193, 0.005) @ RY(0.18)
    p.box((0.016, 0.002, 0.014), (0.0, 0.004, 0.0), "Steel", xf=xf)
    for k in range(5):
        p.box((0.0012, 0.009, 0.014), (-0.007 + k * 0.0035, 0.0095, 0.0), "Steel", xf=xf)
    p.box((0.012, 0.01, 0.009), (0.0, 0.0, 0.0), "Iron", xf=T(-0.015, yb + 0.025, -0.008))
    p.cyl(0.0052, -0.0035, 0.0035, "Copper", segs=12, xf=T(-0.015, yb + 0.025, -0.008))
    for mat, pts in (("Red", [(-0.02, yb + 0.02, 0.018), (-0.03, yb + 0.012, 0.024),
                              (-0.028, yb + 0.003, 0.03), (-0.012, yb + 0.0018, 0.031)]),
                     ("Black", [(-0.018, yb + 0.02, 0.016), (-0.03, yb + 0.017, 0.012),
                                (-0.031, yb + 0.006, 0.0), (-0.024, yb + 0.002, -0.02)]),
                     ("Yellow", [(0.02, yb + 0.022, 0.012), (0.03, yb + 0.014, 0.02),
                                 (0.029, yb + 0.004, 0.03), (0.016, yb + 0.0018, 0.031)])):
        tube_path(p, pts, 0.0012, mat, sides=5, closed=False)
    return [p]


# --- Polymer stock ----------------------------------------------------------------------------

def polymer():
    s = cube(0.4)
    yb = -s / 2
    p = Part("body")
    # Offcuts of plastic bar stock, round and square, in four colours,
    # tied in a bundle with string and a paper tag.
    L = 0.068
    bars = [("round", 0.0065, -0.0125, 0.0065, "Black"), ("round", 0.0065, 0.0005, 0.0065, "Cream"),
            ("square", 0.012, 0.0135, 0.006, "Polymer"), ("square", 0.012, -0.0095, 0.0185, "Red"),
            ("round", 0.006, 0.004, 0.018, "Polymer"), ("round", 0.0055, -0.017, 0.0175, "Cream")]
    for kind, size, z, y, mat in bars:
        xf = T(0.0, yb + y, z)
        if kind == "round":
            p.cyl(size, -L / 2, L / 2, mat, segs=12, xf=xf @ ALONG_X, chamfer=0.0008)
        else:
            p.box((L, size, size), (0.0, 0.0, 0.0), mat, bevel=0.0008, segs=1, xf=xf)
    for x in (-0.022, 0.022):
        ring(p, 0.0205, 0.0009, "Rope", xf=T(x, yb + 0.0128, -0.0005) @ ALONG_X @
             S(1.0, 0.62, 1.0), segs=16, sides=4)
    tube_path(p, [(0.022, yb + 0.026, 0.0), (0.025, yb + 0.028, -0.008),
                  (0.026, yb + 0.026, -0.014)], 0.0008, "Rope", sides=4, closed=False)
    xf = T(0.025, yb + 0.0262, -0.021) @ RY(0.4) @ RZ(-0.08)
    p.box((0.011, 0.0005, 0.016), (0.0, 0.0, 0.0), "Paper", xf=xf)
    for k in range(3):
        p.box((0.007, 0.0002, 0.0008), (0.0, 0.0004, -0.004 + k * 0.003), "Black", xf=xf)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


# --- Powder tin -------------------------------------------------------------------------------

def powder():
    s = cube(0.2)
    yb = -s / 2
    p = Part("body")
    # A flat rectangular tin of smokeless powder, screw cap on the
    # shoulder, the name and a red flame on a yellow label. A rolled bead
    # at the foot and the shoulder; between them the body, its flat faces
    # set back so the labels are flush plates there, not stuck on.
    bead = fillet([(-0.0225, -0.0125), (0.0225, -0.0125), (0.0225, 0.0125), (-0.0225, 0.0125)],
                  0.0055, 3)
    p.prism(bead, yb, yb + 0.0042, "Blued", axis="y", bevel=0.0012, segs=2)
    p.prism(bead, yb + 0.0438, yb + 0.049, "Blued", axis="y", bevel=0.0012, segs=2)
    fx, r, zl = 0.017, 0.005, 0.0085
    plan = ([(-fx, -zl), (fx, -zl)] + arc(fx, -0.012 + r, r, -90, 0, 3) +
            arc(fx, 0.012 - r, r, 0, 90, 3) + [(fx, zl), (-fx, zl)] +
            arc(-fx, 0.012 - r, r, 90, 180, 3) + arc(-fx, -0.012 + r, r, 180, 270, 3))
    p.prism(plan, yb + 0.0042, yb + 0.0438, "Blued", axis="y")
    for sz in (-1, 1):
        panel(p, "z", -fx, fx, yb + 0.0042, yb + 0.0438, sz * zl, sz * 0.012, "Blued",
              [(-fx, fx, yb + 0.0085, yb + 0.0385, "Yellow"),
               (-fx, fx, yb + 0.0095, yb + 0.0115, "Red")])
        zf = sz * 0.012
        rot = FRONT if sz < 0 else T()
        text(p, "ПОРОХ", 0.0065, "Black", T(0.0, yb + 0.031, zf) @ rot, depth=0.0004)
        flame = [(-0.0045, 0.0), (0.0045, 0.0), (0.003, 0.0055), (0.001, 0.004), (0.0, 0.009),
                 (-0.0012, 0.0045), (-0.003, 0.006)]
        p.prism([(x * -sz, yb + 0.0135 + y) for x, y in flame], zf, zf + sz * 0.0007, "Red",
                axis="z")
    # Screw cap on a neck at one end of the top, with a red seal band.
    p.cyl(0.0068, yb + 0.049, yb + 0.0515, "Steel", segs=14, xf=T(-0.011, 0, 0) @ UP)
    p.lathe([(0.0, yb + 0.0575), (0.0068, yb + 0.0575), (0.0076, yb + 0.0567), (0.0076, yb + 0.051),
             (0.0, yb + 0.051)], "Steel", segs=16, xf=T(-0.011, 0, 0) @ UP)
    for k in range(16):
        p.box((0.0007, 0.0052, 0.0007), (0.0, yb + 0.054, 0.0), "Steel",
              xf=T(-0.011, 0, 0) @ RY(2 * math.pi * k / 16) @ T(0.0077, 0, 0))
    p.tube(0.0071, 0.0066, yb + 0.0496, yb + 0.0512, "Red", segs=14, xf=T(-0.011, 0, 0) @ UP)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


# --- Brass cases ------------------------------------------------------------------------------

def brass():
    s = cube(0.15)
    yb = -s / 2
    p = Part("body")
    # A tin lid turned up as a tray, heaped with fired brass of mixed
    # calibres: open mouths, dented, primers struck.
    p.lathe([(0.0, yb), (0.0235, yb), (0.025, yb + 0.0015), (0.0254, yb + 0.008),
             (0.0244, yb + 0.008), (0.024, yb + 0.0018), (0.0, yb + 0.0018)], "Steel", segs=28,
            xf=UP)
    ring(p, 0.0249, 0.0008, "Steel", xf=T(0, yb + 0.008, 0) @ UP, segs=28, sides=4)
    rnd = random.Random(41)
    kinds = [(0.0048, 0.018, 0.0), (0.005, 0.039, 0.0029), (0.0062, 0.053, 0.0042)]
    lay = [(0, -0.01, 0.006, 0.4, 0), (0, 0.008, -0.009, 2.0, 0), (1, 0.0, 0.0, 0.2, 1),
           (0, -0.006, -0.012, 1.1, 1), (0, 0.012, 0.01, -0.6, 1), (1, -0.004, 0.006, 2.6, 2),
           (0, 0.003, -0.002, 0.9, 2), (2, 0.001, -0.002, -1.2, 3), (0, -0.013, -0.004, 1.7, 3)]
    for kind, x, z, a, layer in lay:
        r, L, neck = kinds[kind]
        L = min(L, 0.044)
        y = yb + 0.0018 + r + layer * 0.0062
        xf = T(x, y, z) @ RY(a) @ RX(rnd.uniform(-0.12, 0.12)) @ T(0, 0, -L / 2)
        prof = [(r - 0.0006, L), (r, L), (r, 0.0014), (r - 0.0006, 0.0011), (r - 0.0006, 0.0005),
                (r, 0.0)]
        if neck:
            prof = [(neck - 0.0004, L), (neck, L), (neck, L - 0.006), (r - 0.0003, L - 0.01),
                    (r, 0.0014), (r - 0.0006, 0.0011), (r - 0.0006, 0.0005), (r, 0.0)]
        p.lathe(prof + [(0.0, 0.0)], "Brass", segs=10, xf=xf)
        p.cyl(r * 0.36, -0.0002, 0.0001, "Copper", segs=8, xf=xf)
    # Two stood on end at the back.
    for x, z in ((0.014, 0.012), (0.019, 0.004)):
        xf = T(x, yb + 0.0018, z) @ UP
        p.tube(0.0048, 0.0042, 0.0, 0.018, "Brass", segs=10, xf=xf)
        p.cyl(0.0048, 0.0, 0.0012, "Brass", segs=10, xf=xf)
    return [p]


for item_id, fn in (("scrap_metal", scrap_metal), ("electronics", electronics),
                    ("polymer", polymer), ("powder", powder), ("brass", brass)):
    if wanted(item_id):
        item(item_id, fn)
