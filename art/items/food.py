"""Food from the shelves and the stash: water, tins, packets and a few
vegetables. Writes assets/models/items/<id>.glb for each (item space: see
_kit.py). Most have no `dims`, so each sits on the floor of a cube of side
cbrt(bulk / 1000); the bottle and the tins have their own.

Soviet stores packing: tins with paper labels, card packets, a cloth
sack, each with its name stencilled (МОЛОКО, ЧАЙ, СОЛЬ...) and a picture
where the real one had one, so they read at a glance by colour as much as
by the word.
"""

import math
import os
import random
import sys

import bmesh

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import (ALONG_X, FRONT, ON_TOP, UP, blob, cube, item, panel, ragged, ring, text,
                  text_round, tube_path, wanted)
from common import Part, RX, RY, RZ, T, S, fillet
from mathutils import Vector


def tin(p, r, y0, y1, mat="Steel", segs=24, recess=None):
    """A food tin standing on y0: rolled rims, a slightly waisted body and
    a recessed lid with a bead round it. `recess` (ya, yb) sinks the wall
    there to make room for a label that then is the wall."""
    wall = [(r - 0.0008, y0 + 0.005), (r - 0.0008, y1 - 0.005)]
    if recess:
        ya, yb_ = recess
        wall = [wall[0], (r - 0.0008, ya), (r - 0.0045, ya), (r - 0.0045, yb_),
                (r - 0.0008, yb_), wall[1]]
    p.lathe([(0.0, y0 + 0.0015), (r - 0.003, y0 + 0.0015), (r - 0.0012, y0), (r, y0 + 0.0012),
             (r, y0 + 0.004)] + wall +
            [(r, y1 - 0.004), (r, y1 - 0.0012), (r - 0.0012, y1), (r - 0.003, y1 - 0.0015),
             (0.0, y1 - 0.0015)], mat, segs=segs, xf=UP)
    p.tube(r * 0.72, r * 0.72 - 0.0012, y1 - 0.0018, y1 - 0.0011, mat, segs=segs, xf=UP)


def band(p, r_in, r_out, y0, y1, mat, start=0.0, sweep=360.0, segs=24):
    """A paper band (a label, or a scrap of one) round a tin; `start` 90
    is the front."""
    p.lathe([(r_in, y0), (r_out, y0), (r_out, y1), (r_in, y1)], mat, segs=segs, xf=UP,
            closed=True, start=start, sweep=sweep)


def labelled_tin(p, r, y0, y1, label, trim):
    """A tin with a wrap-round label and a trim band top and bottom;
    returns the label's outer radius."""
    tin(p, r, y0, y1, recess=(y0 + 0.0055, y1 - 0.0055))
    rl = r - 0.0002
    band(p, r - 0.0045, rl, y0 + 0.0055, y1 - 0.0055, label)
    for y in (y0 + 0.0055, y1 - 0.0105):
        band(p, rl - 0.0001, rl + 0.0002, y, y + 0.005, trim)
    return rl


def sack(p, r, centre, y_floor, neck, mat, seed=1, segs=20, rings=12):
    """A soft filled bag: a ball of radii `r` slumped flat on y_floor,
    lumpy, gathered in to a neck at height `neck`."""
    bm = p.bm
    rnd = random.Random(seed)
    bumps = [(Vector((rnd.uniform(-1, 1), rnd.uniform(-0.4, 0.6), rnd.uniform(-1, 1))).normalized(),
              rnd.uniform(-0.08, 0.1)) for _ in range(6)]
    res = bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=1.0)
    verts = res["verts"]
    cx, cy, cz = centre
    for v in verts:
        n = Vector((v.co.x, v.co.z, -v.co.y)).normalized()
        d = sum(a * max(0.0, n.dot(b)) ** 2 for b, a in bumps)
        x, y, z = n.x * r[0] * (1 + d), n.y * r[1] * (1 + d), n.z * r[2] * (1 + d)
        y += cy
        t = min(1.0, max(0.0, (y - (cy + 0.1 * r[1])) / (neck - cy - 0.1 * r[1])))
        f = 1.0 - 0.86 * t ** 1.4
        if y < y_floor + 0.004:
            # Slumped: the bottom spreads a little and sits flat.
            f *= 1.0 + 0.12 * min(1.0, (y_floor + 0.004 - y) / 0.01)
            y = max(y, y_floor)
        v.co = Vector((cx + x * f, y, cz + z * f))
    faces = list({f for v in verts for f in v.link_faces})
    p._finish_piece(verts, faces, mat)


# --- Water bottle --------------------------------------------------------------------------

def water_bottle():
    yb = -0.12
    p = Part("body")
    # A clear plastic bottle two-thirds full, a paper label, a red cap.
    p.lathe([(0.0, yb), (0.027, yb), (0.032, yb + 0.004), (0.0335, yb + 0.012),
             (0.0335, 0.035), (0.031, 0.056), (0.022, 0.08), (0.0135, 0.096), (0.0125, 0.1),
             (0.0125, 0.112), (0.0, 0.112)], "Clear", segs=24, xf=UP)
    p.lathe([(0.0, yb + 0.002), (0.031, yb + 0.002), (0.032, yb + 0.01), (0.032, 0.03),
             (0.0, 0.03)], "Glass", segs=20, xf=UP)
    for y in (-0.09, -0.08, 0.012):
        ring(p, 0.0335, 0.0009, "Clear", xf=T(0, y, 0) @ UP, segs=24, sides=4)
    band(p, 0.0335, 0.0338, -0.05, 0.0, "Paper")
    for y in (-0.047, -0.006):
        band(p, 0.0337, 0.034, y, y + 0.003, "Red")
    text_round(p, "ВОДА", 0.013, "Blued", 0.0338, -0.026)
    text_round(p, "ВОДА", 0.013, "Blued", 0.0338, -0.026, at=math.pi)
    p.tube(0.0158, 0.0124, 0.098, 0.0995, "Clear", segs=16, xf=UP)
    p.lathe([(0.0, 0.1), (0.0145, 0.1), (0.0148, 0.101), (0.0148, 0.1185), (0.0138, 0.12),
             (0.0, 0.12)], "Red", segs=20, xf=UP)
    for k in range(16):
        p.box((0.0012, 0.015, 0.0012), (0.0, 0.11, 0.0), "Red",
              xf=RY(2 * math.pi * k / 16) @ T(0.0148, 0, 0))
    return [p]


# --- Condensed milk -------------------------------------------------------------------------

def condensed_milk():
    s = cube(0.35)
    yb = -s / 2
    p = Part("body")
    # The squat tin of sweetened condensed milk: white label, blue bands,
    # the word round it front and back.
    y1 = yb + 0.052
    rl = labelled_tin(p, 0.0345, yb, y1, "Paper", "Blued")
    for k in range(2):
        text_round(p, "МОЛОКО", 0.009, "Blued", rl + 0.0001, yb + 0.026, at=k * math.pi)
    band(p, rl - 0.0001, rl + 0.0002, yb + 0.0145, yb + 0.0165, "Blued")
    band(p, rl - 0.0001, rl + 0.0002, yb + 0.036, yb + 0.038, "Blued")
    text(p, "1983", 0.006, "Steel", T(0.0, y1 - 0.0015, 0.008) @ ON_TOP, depth=0.0005)
    return [p]


# --- Egg powder -----------------------------------------------------------------------------

def egg_powder():
    s = cube(0.3)
    yb = -s / 2
    p = Part("body")
    # A yellow card packet of dried egg, two white eggs on the front.
    w, h, d = 0.046, 0.062, 0.03
    p.box((w, h, d), (0.0, yb + h / 2, 0.0), "Yellow", bevel=0.001, segs=1)
    p.box((w - 0.002, 0.0006, 0.0008), (0.0, yb + h, 0.0), "Black")
    p.box((w + 0.0004, 0.006, d + 0.0004), (0.0, yb + 0.05, 0.0), "Red")
    for sz in (-1, 1):
        zf = sz * d / 2
        rot = FRONT if sz < 0 else T()
        for x, y, sc in ((-0.006, yb + 0.03, 1.0), (0.008, yb + 0.027, 0.85)):
            egg = [(0.0085 * sc * math.sin(t), 0.011 * sc * math.cos(t) *
                    (1.0 + 0.12 * math.cos(t)))
                   for t in (2 * math.pi * i / 18 for i in range(18))]
            p.prism([(x * -sz + a, y + b) for a, b in egg], zf, zf + 0.0004 * sz, "Paper",
                    axis="z")
        text(p, "ЯИЧНЫЙ", 0.0045, "Black", T(0.0, yb + 0.05, zf + 0.0003 * sz) @ rot)
        text(p, "ПОРОШОК", 0.0045, "Black", T(0.0, yb + 0.009, zf) @ rot)
    return [p]


# --- Carrots ----------------------------------------------------------------------------------

def carrot():
    s = cube(0.3)
    yb = -s / 2
    p = Part("body")
    # Three carrots tied in a bunch by their tops, the leaves trimmed short.
    prof = [(0.0, -0.0145), (0.0068, -0.0148), (0.0098, -0.0115), (0.0105, -0.005),
            (0.0098, 0.003), (0.0085, 0.011), (0.0063, 0.02), (0.0036, 0.028), (0.0012, 0.0325),
            (0.0, 0.033)]
    lay = [((-0.003, yb + 0.0105, -0.0108), 0.0), ((-0.004, yb + 0.0105, 0.0108), 0.05),
           ((-0.001, yb + 0.0285, 0.0), -0.08)]
    rnd = random.Random(3)
    for (x, y, z), a in lay:
        xf = T(x, y, z) @ RY(a) @ ALONG_X
        p.lathe(prof, "Yellow", segs=10, xf=xf)
        # Growth rings, and a few leaf stalks off the crown.
        for zr in (-0.004, 0.006, 0.016):
            r = 0.0104 - 0.00016 * zr * 100
            p.tube(r + 0.0003, r - 0.0004, zr, zr + 0.0008, "Yellow", segs=10, xf=xf)
        for k in range(3):
            ang = 2 * math.pi * k / 3 + rnd.uniform(-0.3, 0.3)
            dy, dz = math.cos(ang) * 0.005, math.sin(ang) * 0.005
            pts = [(x - 0.0145, y, z), (x - 0.02, y + dy * 0.6, z + dz * 0.6),
                   (x - 0.027, y + dy, z + dz)]
            tube_path(p, pts, 0.001, "Olive", sides=4, closed=False)
            blob(p, 0.003, (x - 0.027, y + dy, z + dz), "Olive", scale=(1.0, 0.5, 1.4), segs=6,
                 rings=3)
    ring(p, 0.0185, 0.001, "Rope", xf=T(-0.0135, yb + 0.0185, 0.0) @ ALONG_X @ S(1.0, 1.0, 1.0),
         segs=16, sides=4)
    return [p]


# --- Onion ------------------------------------------------------------------------------------

def onion():
    s = cube(0.15)
    yb = -s / 2
    p = Part("body")
    # A brown-skinned onion: the bulb, darker veins down its skin, the dry
    # neck, a tuft of roots under it.
    prof = [(0.0, -0.0236), (0.008, -0.0238), (0.017, -0.0196), (0.0237, -0.0112),
            (0.0256, -0.003), (0.0242, 0.0045), (0.0195, 0.0105), (0.012, 0.0152),
            (0.006, 0.0182), (0.0034, 0.0215), (0.0026, 0.0238), (0.0, 0.024)]
    xf = T(0.0, -0.0004, 0.0) @ S(1.0, 0.98, 0.94)
    p.lathe(prof, "Rope", segs=20, xf=xf @ UP)
    for k in range(9):
        a = 2 * math.pi * k / 9 + 0.2
        pts = []
        for r, y in prof[1:-2]:
            r2 = r + 0.0003
            pts.append((r2 * math.cos(a), y, r2 * math.sin(a)))
        tube_path(p, [tuple(xf @ Vector(q)) for q in pts], 0.00035,
                  "Bakelite", sides=3, closed=False)
    p.cyl(0.0042, -0.0248, -0.0236, "Wood", segs=10, xf=UP)
    # The dry neck, twisted over.
    tube_path(p, [(0.0, 0.021, 0.0), (0.0008, 0.0235, 0.0), (0.003, 0.0252, 0.0004),
                  (0.0058, 0.0258, 0.001)], 0.0017, "Wood", sides=5, closed=False)
    rnd = random.Random(5)
    for k in range(10):
        a = 2 * math.pi * k / 10
        r0 = 0.0025
        tube_path(p, [(r0 * math.cos(a), -0.0245, r0 * math.sin(a)),
                      (0.006 * math.cos(a + 0.3), -0.0262, 0.006 * math.sin(a + 0.3)),
                      (rnd.uniform(0.008, 0.01) * math.cos(a + 0.5), -0.026,
                       rnd.uniform(0.008, 0.01) * math.sin(a + 0.5))], 0.0004, "Cream", sides=3,
                  closed=False)
    return [p]


# --- Tins -----------------------------------------------------------------------------------

def canned_tomato():
    p = Part("body")
    # A tall tin with a red label, a tomato on a white panel, the name round
    # the top.
    y0, y1 = -0.055, 0.055
    rl = labelled_tin(p, 0.036, y0, y1, "Red", "Cream")
    band(p, rl - 0.0001, rl + 0.0002, -0.032, 0.022, "Paper", start=55, sweep=70)
    p.sphere(0.0125, (0.0, -0.006, -rl - 0.0002), "Red", segs=16, rings=8,
             scale=(1.0, 0.88, 0.1))
    for k in range(5):
        a = 2 * math.pi * k / 5 + math.pi / 2
        p.box((0.0055, 0.0016, 0.0008), (0.0, 0.0, 0.0), "Olive",
              xf=T(0.0, 0.0045, -rl - 0.0016) @ RZ(a) @ T(0.0027, 0, 0))
    p.box((0.0012, 0.004, 0.0008), (0.0, 0.0075, -rl - 0.0016), "Olive")
    for k in range(2):
        text_round(p, "ТОМАТЫ", 0.008, "Cream", rl + 0.0001, 0.033, at=k * math.pi)
    text_round(p, "ТОМАТЫ", 0.007, "Red", rl + 0.0001, -0.026)
    return [p]


def canned_beans():
    p = Part("body")
    # The same tin in a green label: a white panel with a heap of brown
    # beans on it, the name round the top.
    y0, y1 = -0.055, 0.055
    rl = labelled_tin(p, 0.036, y0, y1, "Olive", "Paper")
    band(p, rl - 0.0001, rl + 0.0002, -0.032, 0.022, "Paper", start=55, sweep=70)
    for x, y, a in ((-0.007, -0.01, 0.4), (0.002, -0.012, -0.3), (0.009, -0.006, 0.9),
                    (-0.003, -0.002, -0.8), (0.006, 0.004, 0.2), (-0.009, 0.004, 1.3),
                    (0.0, 0.01, -0.1)):
        zz = -math.sqrt(max(rl * rl - x * x, 0.0)) - 0.0003
        p.sphere(0.004, (0.0, 0.0, 0.0), "Bakelite", segs=8, rings=4,
                 xf=T(x, y, zz) @ RY(math.atan2(x, -zz) * -1.0) @ RZ(a) @ S(1.0, 0.62, 0.3))
    for k in range(2):
        text_round(p, "ФАСОЛЬ", 0.008, "Cream", rl + 0.0001, 0.033, at=k * math.pi)
    text_round(p, "ФАСОЛЬ", 0.007, "Olive", rl + 0.0001, -0.026)
    return [p]


def canned_stew():
    p = Part("body")
    # Tushonka: a wide tin whose label has peeled, leaving bare steel, a
    # few scraps of paper and streaks of glue; the batch stamped in the lid.
    y0, y1 = -0.035, 0.035
    r = 0.0445
    tin(p, r, y0, y1, segs=28)
    for y in (-0.016, -0.004, 0.008, 0.02):
        ring(p, r - 0.0008, 0.0006, "Steel", xf=T(0, y, 0) @ UP, segs=28, sides=4)
    for a0, sw, ya, yb_ in ((200, 55, -0.027, 0.009), (318, 26, -0.025, -0.004),
                            (80, 14, 0.004, 0.027)):
        band(p, r - 0.0008, r - 0.0003, ya, yb_, "Paper", start=a0, sweep=sw, segs=28)
    for a0, ya in ((130, -0.02), (150, 0.01), (20, -0.012)):
        band(p, r - 0.0008, r - 0.0004, ya, ya + 0.0025, "Cream", start=a0, sweep=40, segs=28)
    text(p, "87", 0.007, "Steel", T(-0.012, y1 - 0.0015, 0.0) @ ON_TOP, depth=0.0005)
    text(p, "ГОВ", 0.007, "Steel", T(0.008, y1 - 0.0015, 0.0) @ ON_TOP, depth=0.0005)
    return [p]


# --- Flour sack ----------------------------------------------------------------------------

def flour_sack():
    s = cube(1.2)
    yb = -s / 2
    p = Part("body")
    # A white cotton sack, slumped, its neck tied off with string; the word
    # and two red stripes stencilled on the front.
    neck = yb + 0.084
    sack(p, (0.047, 0.05, 0.037), (0.0, yb + 0.036, 0.0), yb, neck, "Paper", seed=4, segs=22,
         rings=14)
    ring(p, 0.0072, 0.0019, "Rope", xf=T(0, neck, 0) @ UP, segs=14, sides=5)
    ring(p, 0.007, 0.0016, "Rope", xf=T(0, neck + 0.0035, 0) @ UP @ RX(0.15), segs=14, sides=5)
    # The gathered cloth above the tie, crumpled into points.
    tuft = [((0.0125 if k % 2 else 0.0055) * math.cos(2 * math.pi * k / 14),
             (0.0125 if k % 2 else 0.0055) * math.sin(2 * math.pi * k / 14)) for k in range(14)]
    p.prism(tuft, neck + 0.002, neck + 0.0195, "Paper", axis="y",
            width=lambda a, b: (neck + 0.002,
                                neck + 0.0125 + 0.007 * abs(math.sin(a * 900 + b * 500))))
    tube_path(p, [(0.006, neck, -0.004), (0.013, neck - 0.004, -0.011),
                  (0.016, neck - 0.012, -0.018)], 0.0011, "Rope", sides=4, closed=False)
    # Stencil on the front, which bulges: the letters sit on its crown.
    zf = -0.0372
    text(p, "МУКА", 0.0125, "Black", T(0.0, yb + 0.036, zf) @ FRONT, depth=0.0008)
    for y in (yb + 0.022, yb + 0.05):
        p.box((0.034, 0.0028, 0.0008), (0.0, y, zf + 0.0003), "Red")
    text(p, "1 COPT", 0.0055, "Black", T(0.0, yb + 0.0145, zf + 0.0012) @ FRONT, depth=0.0008)
    return [p]


# --- Rice -----------------------------------------------------------------------------------

def rice_bag():
    s = cube(0.5)
    yb = -s / 2
    p = Part("body")
    # A kraft paper bag rolled down at the top, open on a heap of white
    # grains; a label with the name in red.
    plan = fillet([(-0.03, -0.022), (0.03, -0.022), (0.03, 0.022), (-0.03, 0.022)], 0.004, 2)
    p.prism(plan, yb, yb + 0.051, "Rope", axis="y", bevel=0.002, segs=1)
    cuff = fillet([(-0.0318, -0.0238), (0.0318, -0.0238), (0.0318, 0.0238), (-0.0318, 0.0238)],
                  0.005, 2)
    p.prism(cuff, yb + 0.051, yb + 0.0615, "Rope", axis="y", bevel=0.003, segs=2)
    p.lathe([(0.0, 0.0), (0.03, 0.0), (0.024, 0.005), (0.014, 0.009), (0.0, 0.0105)], "Paper",
            segs=16, xf=T(0, yb + 0.06, 0) @ S(1.0, 1.0, 0.74) @ UP)
    rnd = random.Random(7)
    for k in range(30):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0.0, 1.0)) * 0.024
        x, z = rr * math.cos(a), rr * math.sin(a) * 0.74
        t = rr / 0.03
        y = yb + 0.06 + 0.0105 * (1.0 - t * t) - 0.0003
        p.sphere(0.0017, (0.0, 0.0, 0.0), "Paper", segs=6, rings=3,
                 xf=T(x, y, z) @ RY(rnd.uniform(0, math.pi)) @ S(2.0, 0.7, 0.85))
    zf = -0.022 - 0.0003
    p.box((0.044, 0.026, 0.0005), (0.0, yb + 0.03, zf + 0.0002), "Paper")
    text(p, "РИС", 0.011, "Red", T(0.0, yb + 0.033, zf) @ FRONT, depth=0.0005)
    p.box((0.044, 0.0025, 0.0006), (0.0, yb + 0.021, zf), "Red")
    return [p]


# --- Sugar cubes ----------------------------------------------------------------------------

def sugar_cubes():
    s = cube(0.2)
    yb = -s / 2
    p = Part("body")
    # A card box of lump sugar, its lid standing open: rows of white cubes,
    # one lying loose on top.
    w, d, h = 0.052, 0.04, 0.034
    wall = 0.0012
    p.box((w, wall, d), (0.0, yb + wall / 2, 0.0), "Cream")
    for sx in (-1, 1):
        p.box((wall, h, d), (sx * (w - wall) / 2, yb + h / 2, 0.0), "Cream")
    for sz in (-1, 1):
        p.box((w - 2 * wall, h, wall), (0.0, yb + h / 2, sz * (d - wall) / 2), "Cream")
    p.box((w - 0.002, 0.001, d - 0.002), (0.0, yb + 0.021, 0.0), "Black")
    for i in range(4):
        for j in range(3):
            p.box((0.0112, 0.0112, 0.0112), (-0.018 + i * 0.012, yb + 0.0272, -0.012 + j * 0.012),
                  "Paper", bevel=0.0009, segs=1)
    p.box((0.0112, 0.0112, 0.0112), (0.0, 0.0, 0.0), "Paper", bevel=0.0009, segs=1,
          xf=T(0.005, yb + 0.0392, -0.004) @ RY(0.5) @ RX(0.08))
    # The lid, hinged at the back and standing up.
    p.box((w, 0.024, wall), (0.0, 0.012, 0.0), "Cream",
          xf=T(0.0, yb + h, d / 2 - wall / 2) @ RX(0.18))
    p.box((w + 0.0004, 0.004, 0.0004), (0.0, 0.016, -0.0008), "Red",
          xf=T(0.0, yb + h, d / 2 - wall / 2) @ RX(0.18))
    zf = -d / 2 - 0.0002
    text(p, "САХАР", 0.009, "Red", T(0.0, yb + 0.018, zf) @ FRONT, depth=0.0005)
    for y in (yb + 0.006, yb + 0.03):
        p.box((w + 0.0004, 0.0025, d + 0.0004), (0.0, y, 0.0), "Red")
    return [p]


# --- Tea tin ----------------------------------------------------------------------------------

def tea_tin():
    s = cube(0.2)
    yb = -s / 2
    p = Part("body")
    # A square red tea caddy with yellow trim, a panel with the name on
    # front and back, and a round brass lid.
    # Yellow-banded foot and shoulder; between them a red core faced with
    # printed plates, so the panels are the tin's skin, not stuck on it.
    for y in (yb + 0.00275, yb + 0.04325):
        p.box((0.0426, 0.0055, 0.0426), (0.0, y, 0.0), "Yellow", bevel=0.0018, segs=2)
    y0, y1 = yb + 0.0055, yb + 0.0405
    p.box((0.034, y1 - y0, 0.034), (0.0, (y0 + y1) / 2, 0.0), "Red")
    for sz in (-1, 1):
        panel(p, "z", -0.021, 0.021, y0, y1, sz * 0.017, sz * 0.021, "Red",
              [(-0.015, 0.015, yb + 0.01, yb + 0.036, "Yellow"),
               (-0.013, 0.013, yb + 0.012, yb + 0.034, "Red"),
               (-0.012, 0.012, yb + 0.013, yb + 0.033, "Yellow")])
        zf = sz * 0.021
        rot = FRONT if sz < 0 else T()
        text(p, "ЧАЙ", 0.009, "Red", T(0.0, yb + 0.026, zf) @ rot, depth=0.0005)
        p.box((0.004, 0.004, 0.0008), (0.0, 0.0, 0.0), "Red",
              xf=T(0.0, yb + 0.0165, zf) @ RZ(math.pi / 4))
    for sx in (-1, 1):
        panel(p, "x", -0.017, 0.017, y0, y1, sx * 0.017, sx * 0.021, "Red",
              [(-0.006, 0.006, yb + 0.008, yb + 0.038, "Yellow")])
    p.cyl(0.011, yb + 0.045, yb + 0.05, "Brass", segs=20, xf=UP)
    p.lathe([(0.0, yb + 0.056), (0.0115, yb + 0.056), (0.0128, yb + 0.0548), (0.0128, yb + 0.0492),
             (0.0, yb + 0.0492)], "Brass", segs=20, xf=UP)
    return [p]


# --- Spice jar ------------------------------------------------------------------------------

def spice_jar():
    s = cube(0.05)
    yb = -s / 2
    p = Part("body")
    # A little glass jar of red ground pepper, a ribbed tin lid and a band
    # of label.
    p.lathe([(0.0, yb), (0.0135, yb), (0.0152, yb + 0.0015), (0.0155, yb + 0.004),
             (0.0155, yb + 0.021), (0.0135, yb + 0.024), (0.0115, yb + 0.0248),
             (0.0115, yb + 0.027), (0.0, yb + 0.027)], "Clear", segs=20, xf=UP)
    p.lathe([(0.0, yb + 0.0008), (0.0144, yb + 0.0008), (0.0147, yb + 0.003),
             (0.0147, yb + 0.016), (0.011, yb + 0.0178), (0.0, yb + 0.0186)], "Red", segs=16,
            xf=UP)
    p.lathe([(0.0, yb + 0.0362), (0.0118, yb + 0.0362), (0.0126, yb + 0.0352), (0.0126, yb + 0.025),
             (0.0, yb + 0.025)], "Steel", segs=20, xf=UP)
    for k in range(20):
        p.box((0.0008, 0.0085, 0.0008), (0.0, yb + 0.0302, 0.0), "Steel",
              xf=RY(2 * math.pi * k / 20) @ T(0.0127, 0, 0))
    band(p, 0.0155, 0.0158, yb + 0.006, yb + 0.0175, "Paper", segs=20)
    text_round(p, "ПЕРЕЦ", 0.0045, "Black", 0.0158, yb + 0.0118, depth=0.0004)
    return [p]


# --- Salt -------------------------------------------------------------------------------------

def salt_packet():
    s = cube(0.04)
    yb = -s / 2
    p = Part("body")
    # A small white card box of salt with a tin pour spout in the top, its
    # blue and red bands printed on the front and back faces.
    w, h, d = 0.03, 0.031, 0.016
    p.box((w, h, d - 0.008), (0.0, yb + h / 2, 0.0), "Paper")
    for sz in (-1, 1):
        panel(p, "z", -w / 2, w / 2, yb, yb + h, sz * (d / 2 - 0.004), sz * d / 2, "Paper",
              [(-w / 2, w / 2, yb + 0.00475, yb + 0.00725, "Blued"),
               (-w / 2, w / 2, yb + 0.0089, yb + 0.0101, "Red"),
               (-w / 2, w / 2, yb + 0.02575, yb + 0.02825, "Blued")])
        rot = FRONT if sz < 0 else T()
        text(p, "СОЛЬ", 0.008, "Blued", T(0.0, yb + 0.017, sz * d / 2) @ rot, depth=0.0004)
    p.box((0.008, 0.0016, 0.005), (0.008, yb + h + 0.0006, 0.0), "Steel", bevel=0.0004, segs=1)
    return [p]


# --- Army crackers ------------------------------------------------------------------------------

def cracker(p, xf):
    """One square hard-tack biscuit, pricked with a grid of holes."""
    p.box((0.046, 0.0058, 0.046), (0.0, 0.0, 0.0), "Cream", bevel=0.0012, segs=1, xf=xf)
    for i in range(3):
        for j in range(3):
            p.cyl(0.0012, 0.0024, 0.0031, "Bakelite", segs=5,
                  xf=xf @ T(-0.013 + i * 0.013, 0.0, -0.013 + j * 0.013) @ UP)


def crackers():
    s = cube(0.3)
    yb = -s / 2
    p = Part("body")
    # A packet of army hard-tack in olive wax paper, torn open at the
    # front; a biscuit slid out on top.
    zt = -0.008
    plan = fillet([(-0.0255, zt), (0.0255, zt), (0.0255, 0.0325), (-0.0255, 0.0325)],
                  [0.0025, 0.0025, 0.004, 0.004], 2)
    p.prism(plan, yb, yb + 0.031, "Olive", axis="y", bevel=0.002, segs=1)
    for y0, y1, sd in ((yb + 0.0302, yb + 0.031, 1), (yb, yb + 0.0008, 2)):
        ragged(p, 0.05, zt + 0.001, 0.008, y0, y1, "Olive", seed=sd)
    for sx in (-1, 1):
        ragged(p, 0.028, zt + 0.001, 0.007, -0.0004, 0.0004, "Olive", seed=3 + sx,
               xf=T(sx * 0.0251, yb + 0.0155, 0.0) @ RZ(math.pi / 2))
    # The stack inside, showing only where it comes out of the paper.
    for k in range(4):
        p.box((0.046, 0.0058, 0.016), (0.0, yb + 0.0045 + k * 0.0062, -0.012), "Cream",
              bevel=0.0012, segs=1)
    cracker(p, T(-0.001, yb + 0.0348, -0.006) @ RY(0.22) @ RX(0.03))
    for sx in (-1, 1):
        xf = T(sx * 0.0258, yb + 0.0155, 0.017) @ RY(sx * math.pi / 2)
        p.box((0.026, 0.014, 0.0004), (0.0, 0.0, 0.0), "Paper", xf=xf)
        text(p, "ГАЛЕТЫ", 0.0055, "Black", xf, depth=0.0004)
    return [p]


for item_id, fn in (("water_bottle", water_bottle), ("condensed_milk", condensed_milk),
                    ("egg_powder", egg_powder), ("carrot", carrot), ("onion", onion),
                    ("canned_tomato", canned_tomato), ("canned_beans", canned_beans),
                    ("canned_stew", canned_stew), ("flour_sack", flour_sack),
                    ("rice_bag", rice_bag), ("sugar_cubes", sugar_cubes), ("tea_tin", tea_tin),
                    ("spice_jar", spice_jar), ("salt_packet", salt_packet),
                    ("crackers", crackers)):
    if wanted(item_id):
        item(item_id, fn)
