"""Curios: the three depot keys, and small things kept for no use. Writes
assets/models/items/<id>.glb for each (item space: see _kit.py).

The keys are heavy grey data cartridges in a 0.09 x 0.03 x 0.14 box,
connector to -Z: each stencilled with a big letter, its KEY- code and
valley name, and a band of its own colour, so the three tell apart from
across a table. Marrow carries the Hollow survey post's tag, Kettle is
scorched along one edge, Cinder is gritty with ash. The watch and chess
knight have no `dims`, so each sits in a cube of side cbrt(bulk / 1000);
the photograph has its own flat box.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, ON_TOP, UP, blob, cube, item, ragged, text, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, fillet

KEY_Y0, KEY_Y1 = -0.015, 0.008   # the cartridge body; marks stand on its top


def cartridge(p, letter, name, band):
    """A depot key: a rounded grey slab, a dark connector with copper
    contacts at -Z, a ribbed grip at +Z, a lanyard eye in one corner, and
    the stencils on top."""
    # The shell in three lengths, the coloured band round it the middle
    # one, so the band is the shell there rather than painted over it.
    sec = fillet([(-0.044, KEY_Y0), (0.044, KEY_Y0), (0.044, KEY_Y1), (-0.044, KEY_Y1)], 0.003, 2)
    for z0, z1, mat in ((-0.064, 0.01, "Pebble"), (0.01, 0.018, band), (0.018, 0.069, "Pebble")):
        p.prism(sec, z0, z1, mat, axis="z", bevel=0.0015 if mat == "Pebble" else 0.0, segs=2)
    # Connector: a dark lip and a row of contacts in its slot.
    p.box((0.07, 0.016, 0.008), (0.0, -0.0035, -0.0655), "Dark", bevel=0.0015, segs=1)
    p.box((0.058, 0.006, 0.002), (0.0, -0.0035, -0.0694), "Black")
    for k in range(10):
        p.box((0.0028, 0.0035, 0.0012), (-0.0234 + k * 0.0052, -0.0035, -0.0698), "Copper")
    # Grip: dark rubber ridges round the +Z end.
    for k in range(5):
        p.box((0.0896, 0.0018, 0.0026), (0.0, KEY_Y1 - 0.0002, 0.04 + k * 0.0055), "Dark",
              bevel=0.0006, segs=1)
    # Lanyard eye in the +X/+Z corner.
    p.cyl(0.0042, KEY_Y0 + 0.002, KEY_Y1 + 0.0004, "Steel", segs=12, xf=T(0.034, 0, 0.058) @ UP)
    p.cyl(0.0026, KEY_Y0 + 0.001, KEY_Y1 + 0.0007, "Black", segs=10, xf=T(0.034, 0, 0.058) @ UP)
    # Stencils: the letter big in a box, the code and the name, a band.
    top = KEY_Y1
    for x0, x1, z0, z1 in ((-0.036, 0.0, -0.05, -0.046), (-0.036, 0.0, -0.016, -0.012),
                           (-0.036, -0.032, -0.05, -0.012), (-0.004, 0.0, -0.05, -0.012)):
        p.box((x1 - x0, 0.0006, z1 - z0), ((x0 + x1) / 2, top + 0.0003, (z0 + z1) / 2), "Cream")
    text(p, letter, 0.026, "Cream", T(-0.018, top, -0.031) @ ON_TOP, depth=0.0006)
    text(p, "KEY-" + letter, 0.0075, "Cream", T(0.021, top, -0.042) @ ON_TOP, depth=0.0006)
    text(p, name, 0.0068, "Cream", T(0.0, top, 0.0) @ ON_TOP, depth=0.0006)
    # Rivets on the corners of the stencil panel.
    for x, z in ((-0.039, -0.055), (0.039, -0.055), (-0.039, 0.022), (0.039, 0.022)):
        p.cyl(0.0014, top - 0.0002, top + 0.0007, "Steel", segs=8, xf=T(x, 0, z) @ UP)


def key_marrow():
    p = Part("body")
    # Key M, with a card tag on a wire through its eye: the survey post's.
    cartridge(p, "M", "MARROW", "Yellow")
    tube_path(p, [(0.034, KEY_Y1 + 0.001, 0.058), (0.03, KEY_Y1 + 0.0045, 0.052),
                  (0.022, KEY_Y1 + 0.0048, 0.047), (0.016, KEY_Y1 + 0.004, 0.043)], 0.0007,
              "Steel", sides=4, closed=False)
    xf = T(0.012, KEY_Y1 + 0.0036, 0.033) @ RY(0.5) @ RX(0.03)
    tag = fillet([(-0.009, -0.014), (0.009, -0.014), (0.009, 0.01), (0.0, 0.016), (-0.009, 0.01)],
                 0.002, 1)
    p.prism(tag, -0.0004, 0.0004, "Paper", axis="y", xf=xf)
    p.cyl(0.0018, -0.0005, 0.0005, "Yellow", segs=8, xf=xf @ T(0, 0, 0.0115) @ UP)
    text(p, "HOLLOW", 0.0035, "Black", xf @ T(0, 0.0004, 0.002) @ ON_TOP, depth=0.0003)
    for z in (-0.004, -0.008):
        p.box((0.013, 0.0003, 0.0009), (0.0, 0.0005, z), "Black", xf=xf)
    return [p]


def key_kettle():
    p = Part("body")
    # Key K, the +X edge scorched black, the paint blistered there.
    cartridge(p, "K", "KETTLE", "Red")
    rnd = random.Random(31)
    # Soot over the edge: a ragged burn across the top and down the side.
    ragged(p, 0.122, 0.0446, 0.016, KEY_Y1, KEY_Y1 + 0.0005, "Char", xf=T(0, 0, 0.002) @
           RY(math.pi / 2), n=15, seed=2)
    side = [(0.06, KEY_Y1 - 0.0005), (-0.058, KEY_Y1 - 0.0005)]
    for k in range(16):
        z = -0.058 + 0.118 * k / 15
        side.append((z, KEY_Y0 + 0.003 + rnd.uniform(0.0, 0.012)))
    p.prism(side, 0.0441, 0.0449, "Char", axis="x")
    for k in range(8):
        p.sphere(rnd.uniform(0.0012, 0.002), (0.041 - rnd.uniform(0, 0.012), KEY_Y1 + 0.0002,
                                              rnd.uniform(-0.055, 0.06)), "Bakelite", segs=6,
                 rings=3, scale=(1, 0.5, 1))
    return [p]


def key_cinder():
    p = Part("body")
    # Key C, filmed with pale ash and gritty with cinders.
    cartridge(p, "C", "CINDER", "Cream")
    rnd = random.Random(32)
    for k in range(10):
        x, z = rnd.uniform(-0.036, 0.036), rnd.uniform(-0.056, 0.06)
        p.cyl(rnd.uniform(0.003, 0.0065), KEY_Y1 + 0.0002, KEY_Y1 + 0.0008, "Pebble", segs=8,
              xf=T(x, 0, z) @ UP @ S(1.0, 0.6, 1.0))
    for k in range(34):
        x, z = rnd.uniform(-0.04, 0.04), rnd.uniform(-0.06, 0.065)
        blob(p, rnd.uniform(0.0007, 0.0016), (x, KEY_Y1 + 0.0009, z),
             "Char" if k % 3 else "Pebble", segs=5, rings=3)
    return [p]


# --- Stopped wristwatch -------------------------------------------------------------------------

def wristwatch():
    s = cube(0.02)
    yb = -s / 2
    p = Part("body")
    # A steel watch standing on its buckled leather strap, face up, the
    # hands stopped at twenty to five, the crystal cracked.
    R = 0.0103
    cy = yb + R + 0.0007
    p.lathe([(R - 0.0013, -0.0042), (R + 0.0007, -0.0042), (R + 0.0007, 0.0042),
             (R - 0.0013, 0.0042)], "Bakelite", segs=24, xf=T(0, cy, 0) @ ALONG_X, closed=True)
    p.box((0.0098, 0.0012, 0.004), (0.0, yb + 0.0007, 0.0), "Steel")
    p.box((0.0012, 0.0012, 0.0036), (0.0, yb + 0.0008, 0.0), "Steel")
    ct = cy + R + 0.0002
    p.lathe([(0.0, ct), (0.0088, ct), (0.0094, ct + 0.001), (0.0094, ct + 0.0032),
             (0.0084, ct + 0.0042), (0.0, ct + 0.0042)], "Steel", segs=24, xf=UP)
    for sz in (-1, 1):
        for sx in (-1, 1):
            p.box((0.0016, 0.0022, 0.004), (sx * 0.0036, ct + 0.0016, sz * 0.0094), "Steel",
                  bevel=0.0004, segs=1)
    p.cyl(0.0078, ct + 0.0042, ct + 0.0044, "Cream", segs=24, xf=UP)
    for k in range(12):
        L = 0.0018 if k % 3 == 0 else 0.0011
        p.box((0.0005, 0.0003, L), (0.0, ct + 0.0045, -0.0068 + L / 2), "Black",
              xf=RY(2 * math.pi * k / 12))
    yh = ct + 0.0046
    p.box((0.0008, 0.0003, 0.0045), (0.0, yh, -0.0022), "Black", xf=RY(math.radians(-140)))
    p.box((0.0006, 0.0003, 0.0065), (0.0, yh + 0.0002, -0.0032), "Black", xf=RY(math.radians(-240)))
    p.box((0.0003, 0.0002, 0.0068), (0.0, yh + 0.0004, -0.0028), "Red", xf=RY(math.radians(-75)))
    p.lathe([(0.0, ct + 0.0058), (0.0045, ct + 0.0056), (0.0078, ct + 0.0048),
             (0.0083, ct + 0.0042), (0.0, ct + 0.0042)], "Clear", segs=20, xf=UP)
    p.box((0.0002, 0.0002, 0.012), (0.0, ct + 0.0058, 0.0), "Paper", xf=RY(0.6) @ RX(0.0))
    p.cyl(0.0014, 0.0094, 0.0118, "Steel", segs=8, xf=T(0, ct + 0.0021, 0) @ ALONG_X,
          chamfer=0.0003)
    return [p]


# --- Faded photograph -------------------------------------------------------------------------

def old_photo():
    # A 9 x 12.5 cm print with a white border and one dog-eared corner: a
    # woman and a man standing in a field by a pine, sepia gone pale, and
    # a year written in blue ink under the picture. It lies flat on the
    # floor of its box.
    #
    # Built as a mosaic of full-thickness tiles (border, sky, field,
    # figures, tree) cut into each other, not as prints stacked on a card:
    # stacked layers a fraction of a millimetre apart z-fight.
    t = 0.0007
    y0 = -0.005
    X, Z = 0.044, 0.0615
    FOLD = 0.0935  # the dog-ear's fold line, x + z = FOLD

    def clip(poly):
        """Cut a polygon at the fold line (the corner is folded over)."""
        out = []
        n = len(poly)
        for i in range(n):
            a, b = poly[i], poly[(i + 1) % n]
            ia, ib = a[0] + a[1] <= FOLD + 1e-9, b[0] + b[1] <= FOLD + 1e-9
            if ia:
                out.append(a)
            if ia != ib:
                k = (FOLD - a[0] - a[1]) / ((b[0] + b[1]) - (a[0] + a[1]))
                out.append((a[0] + k * (b[0] - a[0]), a[1] + k * (b[1] - a[1])))
        return out

    def rect(x0, x1, z0, z1):
        return [(x0, z0), (x1, z0), (x1, z1), (x0, z1)]

    def disc(cx, cz, r, n=14, wob=0.0, seed=0):
        rnd = random.Random(seed)
        return [(cx + r * (1 + rnd.uniform(-wob, wob)) * math.cos(2 * math.pi * i / n),
                 cz + r * (1 + rnd.uniform(-wob, wob)) * math.sin(2 * math.pi * i / n))
                for i in range(n)]

    PX, PZ0, PZ1 = 0.039, -0.048, 0.056   # the picture; a deeper margin below
    HORIZON = 0.012
    sky = clip(rect(-PX, PX, HORIZON, PZ1))
    field = rect(-PX, PX, PZ0, HORIZON)
    # The picture's top is toward +Z (upright seen from the front, -Z).
    def head(cx, hz, r, hair_from):
        """A face (light) under a cap of hair (dark), split by a chord."""
        pts = disc(cx, hz, r, n=16)
        cut = hz + hair_from
        face, hair = [], []
        for i in range(len(pts)):
            a, b = pts[i], pts[(i + 1) % len(pts)]
            for out, keep in ((face, lambda q: q[1] <= cut), (hair, lambda q: q[1] >= cut)):
                if keep(a):
                    out.append(a)
                if keep(a) != keep(b):
                    k = (cut - a[1]) / (b[1] - a[1])
                    out.append((a[0] + k * (b[0] - a[0]), cut))
        return [(face, "Rope"), (hair, "Bakelite")]

    # Figures: (outline, material). Every outline is a hole cut in what lies
    # behind it, so no two tiles overlap.
    figs = []
    # The man, left of centre as you look at it: dark jacket, lighter
    # trousers, short hair.
    mx, mh, mr = 0.011, 0.026, 0.0056
    figs += head(mx, mh, mr, 0.0012)
    mt = mh - mr
    figs.append((rect(mx - 0.0017, mx + 0.0017, mt - 0.0014, mt), "Rope"))
    figs.append(([(mx - 0.0108, mt - 0.0014), (mx + 0.0108, mt - 0.0014),
                  (mx + 0.0124, -0.0105), (mx + 0.0098, -0.0105), (mx + 0.0094, -0.009),
                  (mx - 0.0094, -0.009), (mx - 0.0098, -0.0105), (mx - 0.0124, -0.0105)],
                 "Bakelite"))
    for sx in (-1, 1):
        x0 = mx + (0.0011 if sx > 0 else -0.0074)
        figs.append((rect(x0, x0 + 0.0063, -0.037, -0.009), "Wood"))
        figs.append((rect(x0, x0 + 0.0063, -0.0395, -0.037), "Char"))
    # The woman: a long-sleeved dress, hair pinned up, a light collar.
    wx, wh, wr = -0.012, 0.0215, 0.0051
    figs += head(wx, wh, wr, 0.0004)
    figs.append((disc(wx - 0.0004, wh + wr + 0.0019, 0.0021, n=10), "Bakelite"))  # the bun
    wt = wh - wr
    figs.append((rect(wx - 0.0016, wx + 0.0016, wt - 0.0013, wt), "Rope"))
    figs.append(([(wx - 0.0035, wt - 0.0013), (wx + 0.0035, wt - 0.0013), (wx, wt - 0.0042)],
                  "Paper"))
    figs.append(([(wx + 0.0035, wt - 0.0013), (wx + 0.0079, wt - 0.0013), (wx + 0.0088, 0.004),
                  (wx + 0.0068, -0.006), (wx + 0.0118, -0.026), (wx - 0.0118, -0.026),
                  (wx - 0.0068, -0.006), (wx - 0.0088, 0.004), (wx - 0.0079, wt - 0.0013),
                  (wx - 0.0035, wt - 0.0013), (wx, wt - 0.0042)], "Wood"))
    for sx in (-1, 1):
        x0 = wx + (0.0012 if sx > 0 else -0.0038)
        figs.append((rect(x0, x0 + 0.0026, -0.036, -0.026), "Bakelite"))
    # A pine at the right edge: dark trunk, ragged crown.
    figs.append((rect(-0.0322, -0.0298, -0.022, 0.026), "Bakelite"))
    crown = []
    for i in range(7):
        zt = 0.026 + i * 0.0042
        hw = 0.0072 - i * 0.0009
        crown.append(([(-0.031 - hw, zt), (-0.031 + hw, zt), (-0.031 + hw * 0.35, zt + 0.0042),
                       (-0.031 - hw * 0.35, zt + 0.0042)], "Bakelite"))
    figs += crown
    # The far treeline along the horizon, a ragged dark band behind them.
    rnd = random.Random(9)
    tl = [(-PX, HORIZON), (PX, HORIZON)]
    n = 26
    for i in range(n + 1):
        x = PX - 2 * PX * i / n
        tl.append((x, HORIZON + 0.0012 + rnd.uniform(0.0, 0.0018)))
    tl = clip(tl)
    # Its cutter reaches below the horizon, so the cut shares no face with
    # the sky's lower edge.
    tl_cut = [(x, z - 0.002 if z <= HORIZON else z) for x, z in tl]

    # The cutters: each figure's whole silhouette, its pieces overlapping
    # (never just touching, which the boolean solver mangles).
    cuts = [disc(mx, mh, mr, n=16), rect(mx - 0.0017, mx + 0.0017, mt - 0.0024, mt + 0.001),
            figs[3][0]]
    for sx in (-1, 1):
        x0 = mx + (0.0011 if sx > 0 else -0.0074)
        cuts.append(rect(x0, x0 + 0.0063, -0.0395, -0.008))
    cuts += [disc(wx, wh, wr, n=16), disc(wx - 0.0004, wh + wr + 0.0019, 0.0021, n=10),
             rect(wx - 0.0016, wx + 0.0016, wt - 0.0023, wt + 0.001),
             [(wx - 0.0079, wt - 0.0013), (wx + 0.0079, wt - 0.0013), (wx + 0.0088, 0.004),
              (wx + 0.0068, -0.006), (wx + 0.0118, -0.026), (wx - 0.0118, -0.026),
              (wx - 0.0068, -0.006), (wx - 0.0088, 0.004)]]
    for sx in (-1, 1):
        x0 = wx + (0.0012 if sx > 0 else -0.0038)
        cuts.append(rect(x0, x0 + 0.0026, -0.036, -0.025))
    cuts.append(rect(-0.0322, -0.0298, -0.022, 0.0265))
    right, left = [], []
    for i in range(7):
        zt = 0.026 + i * 0.0042
        hw = 0.0072 - i * 0.0009
        right += [(-0.031 + hw, zt), (-0.031 + hw * 0.35, zt + 0.0042)]
        left += [(-0.031 - hw, zt), (-0.031 - hw * 0.35, zt + 0.0042)]
    cuts.append(right + list(reversed(left)))

    def cut_by(part, polys, mat):
        holes = Part("cut")
        for poly in polys:
            holes.prism(poly, y0 - 0.001, y0 + t + 0.001, mat, axis="y")
        part.cut(holes)
        return part

    skyp = Part("sky")
    skyp.prism(sky, y0, y0 + t, "Cream", axis="y")
    cut_by(skyp, [tl_cut] + cuts, "Cream")
    fieldp = Part("field")
    fieldp.prism(field, y0, y0 + t, "Rope", axis="y")
    cut_by(fieldp, cuts, "Rope")
    trees = Part("treeline")
    trees.prism(tl, y0, y0 + t, "Wood", axis="y")
    cut_by(trees, cuts, "Wood")
    figures = Part("figures")
    for poly, mat in figs:
        figures.prism(poly, y0, y0 + t, mat, axis="y")

    card = Part("card")
    card.prism(clip(rect(-X, X, -Z, Z)), y0, y0 + t, "Paper", axis="y")
    hole = Part("cut")
    hole.prism(rect(-PX, PX, PZ0, PZ1 + 0.01), y0 - 0.001, y0 + t + 0.001, "Paper", axis="y")
    card.cut(hole)
    # The folded corner lies on top, its back (plain paper) showing.
    card.prism([(X, FOLD - X), (FOLD - Z, Z), (FOLD - Z, FOLD - X)], y0 + t, y0 + 2 * t,
               "Paper", axis="y")
    text(card, "1979", 0.006, "Blue", T(-0.018, y0 + t, -0.055) @ ON_TOP, depth=0.0003)
    return [card, skyp, fieldp, trees, figures]


# --- Chess knight -----------------------------------------------------------------------------

def chess_piece():
    s = cube(0.02)
    yb = -s / 2
    p = Part("body")
    # A turned wooden knight on a felt-bottomed base, the horse's head cut
    # in profile, an eye and mane picked out.
    p.cyl(0.0105, yb, yb + 0.0008, "Olive", segs=20, xf=UP)
    p.lathe([(0.0, yb + 0.0008), (0.0108, yb + 0.0008), (0.011, yb + 0.002), (0.0098, yb + 0.003),
             (0.0085, yb + 0.0042), (0.0086, yb + 0.0052), (0.0074, yb + 0.006),
             (0.0066, yb + 0.0078), (0.0, yb + 0.0078)], "Wood", segs=20, xf=UP)
    y0 = yb + 0.0074
    head = [(0.0062, 0.0), (0.0066, 0.004), (0.006, 0.0088), (0.0047, 0.013), (0.0022, 0.0168),
            (0.0006, 0.0186), (-0.0011, 0.0162), (-0.0043, 0.0142), (-0.0082, 0.0104),
            (-0.0093, 0.0086), (-0.0084, 0.0068), (-0.0042, 0.0072), (-0.0028, 0.0056),
            (-0.0039, 0.0022), (-0.0054, 0.0)]
    p.prism([(z, y0 + y) for z, y in head], -0.0038, 0.0038, "Wood", axis="x", bevel=0.0014,
            segs=2)
    for sx in (-1, 1):
        p.sphere(0.0008, (sx * 0.0037, y0 + 0.0128, -0.0036), "Black", segs=6, rings=3,
                 scale=(0.4, 1, 1))
        p.box((0.0004, 0.0004, 0.0018), (sx * 0.0036, y0 + 0.0083, -0.0074), "Black",
              xf=RX(0.0))
    for k in range(5):
        p.box((0.0026, 0.0012, 0.0014), (0.0, 0.0, 0.0), "Bakelite",
              xf=T(0.0, y0 + 0.006 + k * 0.0024, 0.0061 - k * 0.0006) @ RX(-0.4 - 0.12 * k))
    return [p]


for item_id, fn in (("key_marrow", key_marrow), ("key_kettle", key_kettle),
                    ("key_cinder", key_cinder), ("wristwatch", wristwatch),
                    ("old_photo", old_photo), ("chess_piece", chess_piece)):
    if wanted(item_id):
        item(item_id, fn)
