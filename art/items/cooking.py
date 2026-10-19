"""Camp-kitchen dishes: what the field kitchen turns food into, from flour
and dough to soup, pancakes and something burnt. Writes
assets/models/items/<id>.glb for each (item space: see _kit.py). None has
`dims`, so each sits on the floor of a cube of side cbrt(bulk / 1000).

Each is served the way a camp would: in a white enamel bowl, plate or mug
with a dark rim, on a cutting board with the knife, in the skillet, a tea
glass in its metal holder, the kettle, the soldier's mess tin, or a
sieve. The vessels are shared; what is in them is each dish's own, in
colour and shape, so they tell apart at arm's length.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, blob, centred, cube, item, ring, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, fillet
from mathutils import Vector

YB = -cube(0.3) / 2   # every dish has bulk 0.3: the floor of its cube
ENAMEL, RIM = "Enamel", "Blued"


# --- Vessels ----------------------------------------------------------------------------------

def _wall(pts):
    """Radius at height y up an inner wall given as (r, y) points."""
    pts = sorted(pts, key=lambda q: q[1])

    def r_at(y):
        if y <= pts[0][1]:
            return pts[0][0]
        for (r0, y0), (r1, y1) in zip(pts, pts[1:]):
            if y <= y1:
                return r0 + (r1 - r0) * ((y - y0) / (y1 - y0) if y1 > y0 else 0.0)
        return pts[-1][0]
    return r_at


def bowl(p, r=0.031, h=0.026, x=0.0, z=0.0):
    """An enamel bowl on the floor with a dark rim; returns its inner wall
    as radius(y)."""
    k, q = r / 0.031, h / 0.026
    outer = [(0.0, 0.002), (0.015, 0.002), (0.0155, 0.0), (0.0175, 0.0), (0.018, 0.002),
             (0.026, 0.008), (0.0302, 0.017), (0.031, 0.024)]
    inner = [(0.0299, 0.0255), (0.0287, 0.017), (0.0242, 0.0092), (0.015, 0.0036), (0.0, 0.0036)]
    p.lathe([(a * k, YB + b * q) for a, b in outer + inner], ENAMEL, segs=24,
            xf=T(x, 0, z) @ UP)
    ring(p, 0.0305 * k, 0.0009, RIM, xf=T(x, YB + h - 0.0005, z) @ UP, segs=24, sides=4)
    return _wall([(a * k, YB + b * q) for a, b in inner])


def plate(p, r=0.033, x=0.0, z=0.0):
    """A shallow enamel plate with a dark rim; returns the top of its well."""
    k = r / 0.033
    prof = [(0.0, 0.0015), (0.02, 0.0015), (0.0205, 0.0), (0.022, 0.0), (0.0225, 0.0015),
            (0.03, 0.005), (0.033, 0.0075), (0.0322, 0.0082), (0.0293, 0.0062), (0.022, 0.003),
            (0.0, 0.003)]
    p.lathe([(a * k, YB + b) for a, b in prof], ENAMEL, segs=28, xf=T(x, 0, z) @ UP)
    ring(p, 0.0327 * k, 0.0007, RIM, xf=T(x, YB + 0.0079, z) @ UP, segs=28, sides=4)
    return YB + 0.003


def skillet(p, cx=-0.008):
    """A small black iron frying pan, its handle out to +X; returns the
    top of its floor."""
    p.lathe([(0.0, YB), (0.0195, YB), (0.0215, YB + 0.0015), (0.0245, YB + 0.011),
             (0.0236, YB + 0.0118), (0.021, YB + 0.0042), (0.019, YB + 0.0028),
             (0.0, YB + 0.0028)], "Iron", segs=24, xf=T(cx, 0, 0) @ UP)
    p.box((0.008, 0.0028, 0.0065), (0.0, 0.0, 0.0), "Iron", bevel=0.0008, segs=1,
          xf=T(cx + 0.0262, YB + 0.0085, 0.0) @ RZ(0.3))
    p.box((0.011, 0.0042, 0.0062), (0.0, 0.0, 0.0), "Wood", bevel=0.0015, segs=1,
          xf=T(cx + 0.0352, YB + 0.0118, 0.0) @ RZ(0.3))
    return YB + 0.0028


def mug(p, r=0.0205, h=0.045, x=-0.006, z=0.0):
    """An enamel mug, the handle to +X; returns its inner radius."""
    p.lathe([(0.0, YB), (r - 0.002, YB), (r, YB + 0.0015), (r, YB + h), (r - 0.0012, YB + h),
             (r - 0.0012, YB + 0.003), (0.0, YB + 0.003)], ENAMEL, segs=24,
            xf=T(x, 0, z) @ UP)
    ring(p, r - 0.0006, 0.0008, RIM, xf=T(x, YB + h, z) @ UP, segs=24, sides=4)
    xr = x + r
    tube_path(p, [(xr - 0.001, YB + h - 0.008, z), (xr + 0.006, YB + h - 0.0085, z),
                  (xr + 0.0095, YB + h - 0.015, z), (xr + 0.009, YB + 0.018, z),
                  (xr + 0.005, YB + 0.0125, z), (xr - 0.001, YB + 0.012, z)], 0.0022, ENAMEL,
              sides=6, closed=False)
    return r - 0.0012


def board(p):
    """A worn wooden chopping board; returns its top."""
    plan = fillet([(-0.032, -0.0215), (0.032, -0.0215), (0.032, 0.0215), (-0.032, 0.0215)],
                  0.004, 2)
    p.prism(plan, YB, YB + 0.007, "Boards", axis="y", bevel=0.001, segs=1)
    p.cyl(0.0028, YB + 0.0069, YB + 0.0072, "Black", segs=10, xf=T(0.026, 0, -0.015) @ UP)
    return YB + 0.007


def knife(p, x, y, z, a):
    """A kitchen knife lying flat: blade toward +X before turning by `a`."""
    xf = T(x, y, z) @ RY(a)
    p.prism([(0.0, -0.0032), (0.025, -0.0032), (0.031, 0.0012), (0.0, 0.0034)], 0.0, 0.0008,
            "Steel", axis="y", xf=xf)
    p.box((0.016, 0.0036, 0.0058), (-0.008, 0.0018, 0.0), "Wood", bevel=0.001, segs=1, xf=xf)
    for xr in (-0.004, -0.012):
        p.cyl(0.0008, 0.0036, 0.0039, "Brass", segs=6, xf=xf @ T(xr, 0, 0) @ UP)


def spoon(p, xf, mat="Steel"):
    """A spoon along its +Y axis (bowl at the origin, handle up), then `xf`."""
    p.sphere(0.0055, (0.0, 0.0, 0.0), mat, segs=10, rings=5, xf=xf, scale=(0.7, 1.1, 0.22))
    p.box((0.0022, 0.04, 0.0009), (0.0, 0.025, 0.0), mat, xf=xf)


def fill(p, wall, y, mat, x=0.0, z=0.0, dome=0.0, segs=20):
    """What fills a vessel to height y (a dome on it), down a few mm."""
    r0, r1 = wall(y) - 0.0002, wall(y - 0.004) - 0.0002
    p.lathe([(0.0, y + dome), (r0 * 0.5, y + dome * 0.78), (r0 * 0.85, y + dome * 0.3), (r0, y),
             (r1, y - 0.004), (0.0, y - 0.004)], mat, segs=segs, xf=T(x, 0, z) @ UP)


def grains(p, n, mat, cx, cz, rad, top, dome, seed, size=0.0016):
    """Grains of rice scattered over a dome of radius `rad` peaking at `top`."""
    rnd = random.Random(seed)
    for _ in range(n):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0.0, 1.0)) * rad * 0.92
        t = rr / rad
        y = top - dome * t * t - 0.0002
        p.sphere(size, (0.0, 0.0, 0.0), mat, segs=6, rings=3,
                 xf=T(cx + rr * math.cos(a), y, cz + rr * math.sin(a)) @
                 RY(rnd.uniform(0, math.pi)) @ S(2.0, 0.7, 0.85))


def bits(p, n, mat, cx, cz, rad, y, size, seed, shape="box"):
    """Small pieces (diced, flecks) scattered over a disc at height y."""
    rnd = random.Random(seed)
    for _ in range(n):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0.0, 1.0)) * rad
        xf = T(cx + rr * math.cos(a), y, cz + rr * math.sin(a)) @ RY(rnd.uniform(0, 3)) @ \
            RX(rnd.uniform(-0.4, 0.4))
        if shape == "box":
            p.box((size, size * 0.8, size), (0.0, 0.0, 0.0), mat, bevel=size * 0.12, segs=1,
                  xf=xf)
        else:
            p.box((size, size * 0.25, size * 0.45), (0.0, 0.0, 0.0), mat, xf=xf)


def steam(p, x, y, z, h, seed):
    """Two faint wisps rising from a point."""
    rnd = random.Random(seed)
    for k in range(2):
        ph = rnd.uniform(0, 6)
        pts = [(x + 0.003 * math.sin(ph + i * 1.3) + k * 0.003, y + h * i / 6,
                z + 0.002 * math.cos(ph + i) - k * 0.002) for i in range(7)]
        tube_path(p, pts, 0.0011 - 0.0003 * k, "Clear", sides=4, closed=False)


def egg_profile(L=0.0145, R=0.0105, n=10):
    """An egg's outline (r, z), blunt end at -z."""
    pts = []
    for i in range(n + 1):
        t = math.pi * i / n
        z = -L * math.cos(t)
        r = R * math.sin(t) * (1.0 + 0.1 * math.cos(t))
        pts.append((max(r, 0.0), z))
    return pts


# --- Flour, dough, batter, butter -----------------------------------------------------------

def cook_flour():
    p = Part("body")
    # Fresh-ground flour heaped in a wooden-rimmed sieve, its wire mesh
    # showing at the edge.
    p.lathe([(0.0285, YB), (0.0315, YB), (0.0315, YB + 0.017), (0.0285, YB + 0.017)], "Wood",
            segs=28, xf=UP, closed=True)
    p.cyl(0.0286, YB + 0.0045, YB + 0.0052, "Steel", segs=24, xf=UP)
    for k in range(-5, 6):
        c = k * 0.005
        L = 2 * math.sqrt(0.0285 ** 2 - c * c)
        p.box((L, 0.0004, 0.0005), (0.0, YB + 0.0054, c), "Black")
        p.box((0.0005, 0.0004, L), (c, YB + 0.0054, 0.0), "Black")
    ring(p, 0.03, 0.0009, "Steel", xf=T(0, YB + 0.0172, 0) @ UP, segs=28, sides=4)
    p.lathe([(0.0, YB + 0.0225), (0.003, YB + 0.0215), (0.008, YB + 0.0178), (0.015, YB + 0.0128),
             (0.021, YB + 0.0092), (0.0255, YB + 0.0068), (0.0, YB + 0.0068)], "Paper", segs=20,
            xf=T(-0.002, 0, 0.002) @ UP)
    # A wooden scoop pushed into the heap.
    xf = T(0.009, YB + 0.0175, -0.004) @ RZ(-0.55) @ RX(-math.pi / 2)
    p.lathe([(0.0, 0.0), (0.0058, 0.0), (0.0064, 0.0085), (0.0056, 0.0085), (0.005, 0.001),
             (0.0, 0.001)], "Wood", segs=12, xf=xf @ RX(math.pi))
    p.cyl(0.0018, 0.0, 0.018, "Wood", segs=8, xf=xf)
    return [p]


def cook_dough():
    p = Part("body")
    # A ball of dough on a floured board, the rolling pin beside it.
    top = board(p)
    for x, z, r in ((-0.008, 0.002, 0.016), (0.012, -0.012, 0.006), (-0.022, -0.012, 0.005)):
        p.cyl(r, top, top + 0.0003, "Paper", segs=12, xf=T(x, 0, z) @ UP)
    blob(p, 0.0135, (-0.008, top + 0.0088, 0.002), "Cream", scale=(1.15, 0.68, 1.05),
         bumps=[((1, -0.2, 0.3), 0.08), ((-0.4, 0.3, -1), 0.06), ((0, -1, 0), -0.25)], segs=16,
         rings=8)
    p.cyl(0.0048, -0.024, 0.024, "Wood", segs=12, xf=T(0.017, top + 0.0048, 0.002) @ RY(0.15))
    for sz in (-1, 1):
        p.cyl(0.0022, 0.0, 0.0075, "Wood", segs=8,
              xf=T(0.017, top + 0.0048, 0.002) @ RY(0.15) @ T(0, 0, sz * 0.024) @
              RY(0.0 if sz > 0 else math.pi))
    return [p]


def cook_risen_dough():
    p = Part("body")
    # Dough risen in its bowl, domed up over the rim, a cloth tucked half
    # back off it.
    wall = bowl(p)
    blob(p, 0.03, (0.0, YB + 0.023, 0.0), "Cream", scale=(0.97, 0.5, 0.97),
         bumps=[((0.4, 1, 0.2), 0.08), ((-0.5, 1, -0.4), 0.06), ((0, -1, 0), -0.3)], segs=20,
         rings=10)
    # A dusting of flour and the dimple of a poking finger.
    for x, z in ((0.008, -0.01), (-0.012, 0.004), (0.004, 0.011)):
        p.cyl(0.0035, 0.0, 0.0004, "Paper", segs=8,
              xf=T(x, YB + 0.0378 - 9.0 * (x * x + z * z), z) @ UP)
    p.sphere(0.003, (-0.004, YB + 0.0385, -0.006), "Rope", segs=8, rings=4, scale=(1, 0.3, 1))
    return [p]


def cook_batter():
    p = Part("body")
    # Pale batter in a bowl, smooth as cream, a whisk leaning in it.
    wall = bowl(p)
    fill(p, wall, YB + 0.0205, "Cream", dome=0.0004)
    for k, (x, z) in enumerate(((0.012, -0.006), (0.0, 0.01), (-0.01, -0.004))):
        ring(p, 0.0025 + 0.001 * k, 0.0005, "Cream", xf=T(x, YB + 0.0207, z) @ UP, segs=10,
             sides=3)
    # Whisk: four wire loops round a handle, leaning out over the rim.
    xf = T(-0.002, YB + 0.0105, 0.004) @ RZ(0.62) @ RX(0.1)
    for k in range(4):
        ph = math.pi * k / 4
        pts = []
        for i in range(14):
            t = 2 * math.pi * i / 14
            w = 0.0085 * math.sin(t)
            pts.append(tuple(xf @ Vector((w * math.cos(ph), 0.022 * (1 - math.cos(t)) / 2,
                                          w * math.sin(ph)))))
        tube_path(p, pts, 0.00045, "Steel", sides=3)
    p.cyl(0.0032, 0.022, 0.044, "Wood", segs=10, xf=xf @ RX(-math.pi / 2))
    p.cyl(0.0034, 0.02, 0.0235, "Steel", segs=10, xf=xf @ RX(-math.pi / 2))
    return [p]


def cook_butter():
    p = Part("body")
    # A block of butter on its paper on a saucer, one end sliced off, the
    # knife across it.
    top = plate(p, r=0.031)
    p.box((0.038, 0.0004, 0.034), (0.0, 0.0, 0.0), "Paper", xf=T(0.0, top + 0.0003, 0.0) @
          RY(0.35))
    xf = T(-0.002, top + 0.0005, 0.0) @ RY(0.35)
    p.box((0.028, 0.017, 0.019), (0.003, 0.0085, 0.0), "Cream", bevel=0.0018, segs=2, xf=xf)
    p.box((0.004, 0.012, 0.018), (-0.016, 0.006, 0.0), "Cream", bevel=0.0012, segs=1,
          xf=xf @ RZ(0.25))
    p.box((0.032, 0.001, 0.003), (0.0, 0.0, 0.0), "Steel", xf=T(0.004, top + 0.0185, 0.003) @
          RY(-0.5))
    p.box((0.015, 0.003, 0.0045), (0.0, 0.0, 0.0), "Wood", bevel=0.0008, segs=1,
          xf=T(0.004, top + 0.0185, 0.003) @ RY(-0.5) @ T(0.022, 0.0, 0.0))
    return [p]


# --- Drinks ---------------------------------------------------------------------------------

def cook_hot_water():
    p = Part("body")
    # A white enamel kettle off the fire, steam curling from its spout.
    x = 0.006
    p.lathe([(0.0, YB), (0.018, YB), (0.0215, YB + 0.0025), (0.0248, YB + 0.011),
             (0.0245, YB + 0.021), (0.0205, YB + 0.0295), (0.0125, YB + 0.0335),
             (0.012, YB + 0.0355), (0.0, YB + 0.0355)], ENAMEL, segs=24, xf=T(x, 0, 0) @ UP)
    ring(p, 0.0205, 0.0009, RIM, xf=T(x, YB + 0.0015, 0) @ UP, segs=24, sides=4)
    p.lathe([(0.0, YB + 0.0385), (0.004, YB + 0.0385), (0.0045, YB + 0.036), (0.0115, YB + 0.0352),
             (0.0115, YB + 0.0345), (0.0, YB + 0.0345)], RIM, segs=20, xf=T(x, 0, 0) @ UP)
    spout = [(x - 0.019, YB + 0.01, 0.0), (x - 0.026, YB + 0.017, 0.0),
             (x - 0.0305, YB + 0.026, 0.0), (x - 0.0325, YB + 0.031, 0.0)]
    tube_path(p, spout, 0.0032, ENAMEL, sides=8, closed=False)
    ring(p, 0.0032, 0.0006, RIM, xf=T(x - 0.0325, YB + 0.031, 0.0) @ UP @ RX(0.0), segs=8,
         sides=3)
    arc = [(x + 0.017 * math.cos(math.pi * i / 10), YB + 0.031 + 0.02 * math.sin(math.pi * i / 10),
            0.0) for i in range(11)]
    tube_path(p, arc, 0.0011, "Steel", sides=5, closed=False)
    p.cyl(0.0028, -0.007, 0.007, "Bakelite", segs=10, xf=T(x, YB + 0.0512, 0.0) @ ALONG_X)
    steam(p, x - 0.0322, YB + 0.0335, 0.0, 0.03, seed=2)
    return [p]


def tea_glass(p, liquid, lemon=False):
    """A faceted tea glass in a metal holder with a handle, a spoon in it,
    filled with `liquid`."""
    p.lathe([(0.0, YB + 0.003), (0.0155, YB + 0.003), (0.0165, YB + 0.005), (0.0172, YB + 0.06),
             (0.0164, YB + 0.06), (0.0157, YB + 0.0062), (0.0, YB + 0.0062)], "Clear", segs=10,
            xf=UP)
    p.lathe([(0.0, YB + 0.0064), (0.0155, YB + 0.0064), (0.0162, YB + 0.049), (0.0, YB + 0.049)],
            liquid, segs=10, xf=UP)
    p.lathe([(0.0, YB), (0.0168, YB), (0.0192, YB + 0.0018), (0.0188, YB + 0.0038),
             (0.0178, YB + 0.005), (0.0181, YB + 0.034), (0.0188, YB + 0.036),
             (0.0185, YB + 0.0375), (0.0174, YB + 0.0365), (0.0174, YB + 0.0062),
             (0.0, YB + 0.0062)], "Steel", segs=24, xf=UP)
    for y in (YB + 0.013, YB + 0.025):
        ring(p, 0.0182, 0.0007, "Steel", xf=T(0, y, 0) @ UP, segs=24, sides=4)
    for k in range(6):
        p.box((0.004, 0.0065, 0.0006), (0.0, 0.0, 0.0), "Black",
              xf=RY(2 * math.pi * k / 6 + 0.5) @ T(0.0, YB + 0.019, -0.0182))
    tube_path(p, [(0.0178, YB + 0.032, 0.0), (0.0255, YB + 0.0315, 0.0), (0.0292, YB + 0.025, 0.0),
                  (0.0278, YB + 0.0125, 0.0), (0.0178, YB + 0.0095, 0.0)], 0.0019, "Steel", sides=6,
              closed=False)
    spoon(p, T(-0.006, YB + 0.03, 0.004) @ RZ(0.22) @ S(0.78, 0.78, 0.78))
    if lemon:
        p.cyl(0.0085, YB + 0.0488, YB + 0.0503, "Yellow", segs=14, xf=T(0.003, 0, -0.004) @ UP)
        p.cyl(0.0072, YB + 0.0503, YB + 0.0506, "Cream", segs=14, xf=T(0.003, 0, -0.004) @ UP)
        for k in range(5):
            p.box((0.0004, 0.0003, 0.0065), (0.0, 0.0, 0.0), "Yellow",
                  xf=T(0.003, YB + 0.0507, -0.004) @ RY(math.pi * k / 5))


def cook_tea():
    p = Part("body")
    # Strong black tea in a glass in its metal holder, a slice of lemon.
    tea_glass(p, "Bakelite", lemon=True)
    return [p]


def cook_milk_tea():
    p = Part("body")
    # Milky tea in an enamel mug, a skin of froth at the edge.
    r = mug(p)
    p.cyl(r - 0.0001, YB + 0.032, YB + 0.0385, "Rope", segs=20, xf=T(-0.006, 0, 0) @ UP)
    ring(p, r - 0.0012, 0.0008, "Cream", xf=T(-0.006, YB + 0.0386, 0) @ UP, segs=20, sides=3)
    for x, z in ((-0.01, 0.004), (-0.002, -0.006), (-0.005, 0.009)):
        p.sphere(0.0014, (x, YB + 0.0386, z), "Cream", segs=6, rings=3, scale=(1, 0.4, 1))
    return [p]


# --- Chopped ----------------------------------------------------------------------------------

def cook_chopped_carrot():
    p = Part("body")
    # Carrot cut into coins in a heap on the board, the knife beside them.
    top = board(p)
    rnd = random.Random(11)
    for k in range(13):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0, 1)) * 0.012
        x, z = -0.008 + rr * math.cos(a), 0.003 + rr * math.sin(a)
        y = top + 0.0013 + (0.0022 if rr < 0.007 and k % 2 else 0.0)
        r = rnd.uniform(0.0038, 0.0055)
        xf = T(x, y, z) @ RX(rnd.uniform(-0.25, 0.25)) @ RZ(rnd.uniform(-0.25, 0.25)) @ UP
        p.cyl(r, -0.0012, 0.0012, "Orange", segs=10, xf=xf)
        p.cyl(r * 0.45, 0.0012, 0.0014, "Cream", segs=8, xf=xf)
    knife(p, 0.004, top, -0.015, 0.25)
    return [p]


def cook_chopped_onion():
    p = Part("body")
    # Onion sliced into crescents beside the half it came from, its rings
    # showing on the cut face; the knife.
    top = board(p)
    rnd = random.Random(12)
    for k in range(12):
        x, z = -0.012 + rnd.uniform(-0.011, 0.011), 0.002 + rnd.uniform(-0.01, 0.01)
        r = rnd.uniform(0.005, 0.009)
        p.lathe([(r, -0.0012), (r + 0.0016, -0.0012), (r + 0.0016, 0.0012), (r, 0.0012)],
                "Paper", segs=6, closed=True, start=rnd.uniform(0, 360), sweep=110,
                xf=T(x, top + 0.0013 + (0.0012 if k % 3 == 0 else 0.0), z) @ UP)
    # The half onion, its cut face to the front.
    xf = T(0.017, top + 0.0118, -0.003)
    p.lathe([(0.0, 0.0), (0.0118, 0.0), (0.0112, 0.004), (0.0092, 0.0082), (0.0055, 0.011),
             (0.0025, 0.0125), (0.0015, 0.0145), (0.0, 0.0145)], "Rope", segs=16, xf=xf)
    p.cyl(0.0114, -0.0004, 0.0, "Paper", segs=16, xf=xf)
    for r in (0.0035, 0.0063, 0.0091):
        p.tube(r + 0.0004, r - 0.0003, -0.0007, -0.0003, "Cream", segs=16, xf=xf)
    knife(p, 0.002, top, -0.016, 0.1)
    return [p]


def cook_chopped_tomato():
    p = Part("body")
    # Tomato diced in a heap, the other half cut face up with its seeds,
    # the knife.
    top = board(p)
    bits(p, 14, "Red", -0.011, 0.002, 0.011, top + 0.0032, 0.0062, seed=13)
    xf = T(0.016, top + 0.0098, 0.006)
    p.lathe([(0.0, -0.0098), (0.006, -0.0092), (0.0095, -0.006), (0.0112, -0.002),
             (0.0114, 0.0), (0.0, 0.0)], "Red", segs=16, xf=xf @ UP)
    p.cyl(0.0108, 0.0, 0.0003, "Red", segs=16, xf=xf @ UP)
    p.cyl(0.0042, 0.0003, 0.0006, "Cream", segs=10, xf=xf @ UP)
    for k in range(4):
        a = math.pi / 2 * k + 0.4
        p.sphere(0.0022, (0.0068 * math.cos(a), 0.0004, 0.0068 * math.sin(a)), "Yellow", segs=8,
                 rings=4, xf=xf, scale=(1.2, 0.3, 0.8))
    knife(p, 0.0, top, -0.016, -0.12)
    return [p]


# --- In a bowl ------------------------------------------------------------------------------

def cook_soaked_beans():
    p = Part("body")
    # Beans swollen in a bowl of water, the top ones breaking the surface.
    wall = bowl(p)
    fill(p, wall, YB + 0.014, "Bakelite", dome=0.002)
    fill(p, wall, YB + 0.0205, "Clear")
    rnd = random.Random(14)
    for k in range(16):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0, 1)) * 0.021
        y = YB + (0.0198 if k < 7 else 0.0168)
        p.sphere(0.0042, (0.0, 0.0, 0.0), "Bakelite", segs=8, rings=4,
                 xf=T(rr * math.cos(a), y, rr * math.sin(a)) @ RY(rnd.uniform(0, 3)) @
                 S(1.0, 0.55, 0.62))
    return [p]


def cook_salad():
    p = Part("body")
    # A bowl of salad: green leaves, red tomato, orange carrot sticks and
    # rings of onion.
    wall = bowl(p)
    fill(p, wall, YB + 0.0215, "Leaf", dome=0.005)
    rnd = random.Random(15)
    for k in range(7):
        a = 2 * math.pi * k / 7 + rnd.uniform(-0.3, 0.3)
        rr = rnd.uniform(0.006, 0.019)
        leaf = [(0.0, -0.0075), (0.0045, -0.003), (0.004, 0.004), (0.0, 0.0075), (-0.004, 0.004),
                (-0.0045, -0.003)]
        p.prism(leaf, -0.0004, 0.0004, "Leaf", axis="y",
                xf=T(rr * math.cos(a), YB + 0.0255 - rr * 0.08, rr * math.sin(a)) @
                RY(rnd.uniform(0, 3)) @ RX(rnd.uniform(-0.6, 0.6)) @ RZ(rnd.uniform(-0.5, 0.5)))
    bits(p, 5, "Red", 0.0, 0.0, 0.017, YB + 0.0262, 0.0055, seed=16)
    for k in range(5):
        a = rnd.uniform(0, 2 * math.pi)
        rr = rnd.uniform(0.003, 0.018)
        p.box((0.011, 0.0024, 0.0024), (0.0, 0.0, 0.0), "Orange",
              xf=T(rr * math.cos(a), YB + 0.0262, rr * math.sin(a)) @ RY(rnd.uniform(0, 3)) @
              RZ(rnd.uniform(-0.3, 0.3)))
    for k in range(3):
        a = 2 * math.pi * k / 3 + 1.0
        ring(p, 0.0045, 0.0009, "Paper",
             xf=T(0.012 * math.cos(a), YB + 0.0268, 0.012 * math.sin(a)) @ RX(-1.4 + 0.2 * k) @
             RY(0.3), segs=12, sides=4)
    return [p]


def cook_steamed_rice():
    p = Part("body")
    # A bowl heaped with white rice, grains loose on the top, a spoon in it.
    wall = bowl(p)
    fill(p, wall, YB + 0.0235, "Paper", dome=0.012)
    grains(p, 34, "Paper", 0.0, 0.0, 0.026, YB + 0.0355, 0.012, seed=17, size=0.0021)
    spoon(p, T(-0.01, YB + 0.029, 0.004) @ RZ(-1.05) @ RX(0.15))
    return [p]


def cook_soup():
    p = Part("body")
    # A bowl of red beetroot soup with a spoon of sour cream and green
    # herbs on it, a spoon at the rim.
    wall = bowl(p)
    fill(p, wall, YB + 0.0215, "Red")
    blob(p, 0.006, (0.004, YB + 0.0215, -0.003), "Paper", scale=(1.0, 0.45, 0.9),
         bumps=[((1, 1, 0), 0.2), ((-1, 0.5, 0.5), 0.15)], segs=12, rings=6)
    bits(p, 12, "Leaf", -0.002, 0.002, 0.019, YB + 0.0217, 0.0028, seed=18, shape="fleck")
    bits(p, 6, "Yellow", 0.0, 0.0, 0.02, YB + 0.0215, 0.0016, seed=19, shape="fleck")
    spoon(p, T(0.008, YB + 0.0225, 0.008) @ RZ(1.05) @ RX(0.2))
    return [p]


def cook_rice_pudding():
    p = Part("body")
    # Creamy rice pudding in a bowl, grains showing, a blob of red jam.
    wall = bowl(p)
    fill(p, wall, YB + 0.022, "Cream", dome=0.0025)
    grains(p, 18, "Paper", 0.0, 0.0, 0.025, YB + 0.0245, 0.0025, seed=20, size=0.0015)
    blob(p, 0.0065, (-0.003, YB + 0.0245, 0.002), "Red", scale=(1.0, 0.4, 1.0),
         bumps=[((1, 0, 0.3), 0.25), ((-0.6, 0, -1), 0.2)], segs=12, rings=6)
    return [p]


def cook_fried_rice():
    p = Part("body")
    # Fried rice heaped in a bowl: browned grains with scrambled egg,
    # green onion and red pepper through it.
    wall = bowl(p)
    fill(p, wall, YB + 0.0235, "Rope", dome=0.011)
    grains(p, 24, "Cream", 0.0, 0.0, 0.026, YB + 0.0345, 0.011, seed=21, size=0.002)
    rnd = random.Random(22)
    for mat, n, size in (("Yellow", 6, 0.0045), ("Leaf", 7, 0.003), ("Red", 5, 0.0032)):
        for _ in range(n):
            a = rnd.uniform(0, 2 * math.pi)
            rr = math.sqrt(rnd.uniform(0, 1)) * 0.02
            t = rr / 0.026
            p.box((size, size * 0.55, size * 0.8), (0.0, 0.0, 0.0), mat, bevel=size * 0.15,
                  segs=1, xf=T(rr * math.cos(a), YB + 0.0345 - 0.011 * t * t, rr * math.sin(a)) @
                  RY(rnd.uniform(0, 3)) @ RZ(rnd.uniform(-0.4, 0.4)))
    return [p]


# --- On a plate -------------------------------------------------------------------------------

def cook_boiled_egg():
    p = Part("body")
    # A boiled egg whole beside another halved on a plate, the yolks up.
    top = plate(p)
    p.lathe(egg_profile(), "Cream", segs=14,
            xf=T(-0.012, top + 0.0105, 0.004) @ RY(0.5))
    for k, (x, z, a) in enumerate(((0.011, -0.006, 0.3), (0.013, 0.014, 0.1))):
        xf = T(x, top + 0.0072, z) @ RY(a) @ S(1.0, 0.7, 1.0)
        p.lathe(egg_profile(n=8), "Paper", segs=12, xf=xf, start=180, sweep=180)
        out = egg_profile(n=8)
        p.prism([(r, z_) for r, z_ in out[1:-1]] + [(-r, z_) for r, z_ in reversed(out[1:-1])]
                + [(0.0, out[0][1])], -0.0003, 0.0, "Paper", axis="y", xf=xf)
        p.sphere(0.0052, (0.0, 0.0, 0.0015), "Yellow", segs=12, rings=6, xf=xf,
                 scale=(1.0, 0.35, 1.15))
    return [p]


def cook_custard():
    p = Part("body")
    # Baked custard turned out on a saucer, dark caramel on its crown and
    # running down its sides.
    top = plate(p, r=0.029)
    p.lathe([(0.0, top), (0.0175, top), (0.0172, top + 0.0012), (0.0138, top + 0.0225),
             (0.0128, top + 0.0238), (0.0, top + 0.0238)], "Cream", segs=24, xf=UP)
    p.lathe([(0.0, top + 0.0246), (0.0128, top + 0.0244), (0.0137, top + 0.0232),
             (0.0141, top + 0.0205), (0.0, top + 0.0205)], "Wood", segs=24, xf=UP)
    for k in range(6):
        a = 2 * math.pi * k / 6 + 0.3
        L = 0.006 + 0.004 * (k % 3)
        p.box((0.0032, L, 0.0012), (0.0, -L / 2, 0.0), "Wood", bevel=0.0005, segs=1,
              xf=RY(a) @ T(0.0, top + 0.0215, -0.0141) @ RX(0.17))
    p.cyl(0.021, top, top + 0.0007, "Wood", segs=20, xf=UP)
    return [p]


def cook_fried_egg():
    p = Part("body")
    # Two fried eggs in the iron skillet, the whites spread and crisp-edged.
    floor = skillet(p)
    rnd = random.Random(23)
    for cx, cz, r in ((-0.016, -0.004, 0.0105), (-0.002, 0.007, 0.0095)):
        out = [((r + rnd.uniform(-0.002, 0.0015)) * math.cos(2 * math.pi * k / 14),
                (r + rnd.uniform(-0.002, 0.0015)) * math.sin(2 * math.pi * k / 14))
               for k in range(14)]
        p.prism(fillet([(cx + a, cz + b) for a, b in out], 0.001, 2), floor, floor + 0.0016,
                "Paper", axis="y", bevel=0.0007, segs=1)
        p.sphere(0.0048, (cx + 0.001, floor + 0.0016, cz), "Yellow", segs=12, rings=6,
                 scale=(1.0, 0.62, 1.0))
    return [p]


def cook_omelette():
    p = Part("body")
    # A folded omelette on a plate, browned at the fold, green herbs on it.
    top = plate(p)
    half = [(0.02 * math.cos(math.pi * k / 12), 0.022 * math.sin(math.pi * k / 12))
            for k in range(13)]
    p.prism(fillet([(a, b - 0.009) for a, b in half], 0.0045, 3), top, top + 0.0095, "Yellow",
            axis="y", bevel=0.0042, segs=2, xf=RY(0.3))
    p.box((0.036, 0.0015, 0.0026), (0.0, top + 0.0083, -0.008), "Rope", bevel=0.0006, segs=1,
          xf=RY(0.3))
    bits(p, 8, "Leaf", 0.0, 0.0, 0.012, top + 0.0098, 0.0028, seed=24, shape="fleck")
    return [p]


def cook_pancake():
    p = Part("body")
    # A stack of thin pancakes on a plate, sour cream and red jam on top.
    top = plate(p)
    rnd = random.Random(25)
    y = top
    for k in range(5):
        xo, zo = rnd.uniform(-0.0015, 0.0015), rnd.uniform(-0.0015, 0.0015)
        p.cyl(0.0225, y, y + 0.0028, "Rope", segs=20, xf=T(xo, 0, zo) @ UP, chamfer=0.0011)
        ring(p, 0.0215, 0.0006, "Wood", xf=T(xo, y + 0.0014, zo) @ UP, segs=20, sides=3)
        y += 0.0028
    blob(p, 0.0062, (-0.004, y + 0.0004, 0.002), "Paper", scale=(1.0, 0.5, 0.9),
         bumps=[((0, 1, 0), 0.3)], segs=12, rings=6)
    blob(p, 0.005, (0.008, y + 0.0002, -0.006), "Red", scale=(1.1, 0.35, 0.9), segs=10, rings=5)
    return [p]


def cook_cake():
    p = Part("body")
    # A round layer cake on a plate, a slice cut out of the front showing
    # sponge and cream; berries on the top.
    top = plate(p)
    R, H = 0.0255, 0.027
    a0, sweep = 120.0, 300.0
    p.lathe([(0.0, top), (R, top), (R + 0.0004, top + H - 0.002), (R - 0.0015, top + H),
             (0.0, top + H)], "Cream", segs=24, xf=UP, start=a0, sweep=sweep)
    layers = [(0.0, 0.008, "Rope"), (0.008, 0.0105, "Cream"), (0.0105, 0.018, "Rope"),
              (0.018, 0.0205, "Red"), (0.0205, H, "Cream")]
    for ang in (a0, a0 + sweep):
        for y0, y1, mat in layers:
            p.box((R, y1 - y0, 0.0005), (R / 2, top + (y0 + y1) / 2, 0.0), mat,
                  xf=RY(math.radians(ang)))
    for k in range(7):
        a = math.radians(a0 + 20 + k * 40)
        x, z = 0.0185 * math.cos(a), -0.0185 * math.sin(a)
        p.sphere(0.0028, (x, top + H + 0.0018, z), "Red", segs=8, rings=4)
        p.sphere(0.0032, (x * 0.62, top + H + 0.0008, z * 0.62), "Paper", segs=8, rings=4,
                 scale=(1, 0.6, 1))
    return [p]


# --- Bread ---------------------------------------------------------------------------------

def cook_bread():
    p = Part("body")
    # A brick tin loaf: golden sides, a dark domed crust scored across, the
    # end cut off on the pale crumb.
    # The loaf stops 4 mm short of the cut, where a crust rim round the
    # crumb face makes up the end, so the crumb is the surface there.
    zc, zb = -0.0255, -0.0215
    p.box((0.036, 0.028, 0.032 - zb), (0.0, YB + 0.014, (0.032 + zb) / 2), "Rope", bevel=0.003,
          segs=2)
    crown = fillet([(-0.0182, 0.026), (0.0182, 0.026), (0.0176, 0.032), (0.0132, 0.0372),
                    (0.0065, 0.0396), (0.0, 0.04), (-0.0065, 0.0396), (-0.0132, 0.0372),
                    (-0.0176, 0.032)], [0.003, 0.003, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 2)
    p.prism([(x, YB + y) for x, y in crown], zb, 0.032, "Wood", axis="z", bevel=0.0025, segs=2)
    for z in (-0.012, 0.004, 0.02):
        p.box((0.022, 0.0016, 0.0028), (0.0, 0.0, 0.0), "Rope", xf=T(0.0, YB + 0.0393, z) @
              RY(0.45))
    cut = [(-0.0166, 0.0025), (0.0166, 0.0025), (0.0166, 0.0305), (0.0122, 0.0362),
           (0.006, 0.0384), (0.0, 0.0388), (-0.006, 0.0384), (-0.0122, 0.0362),
           (-0.0166, 0.0305)]
    p.prism([(x, YB + y) for x, y in cut], zc, zb, "Cream", axis="z")
    p.box((0.036, 0.0025, zb - zc), (0.0, YB + 0.00125, (zc + zb) / 2), "Rope")
    for sx in (-1, 1):
        p.box((0.0014, 0.028, zb - zc), (sx * 0.0173, YB + 0.0165, (zc + zb) / 2), "Rope")
    inner = [(x, y) for x, y in cut[3:8]][::-1]
    rim = ([(-0.018, 0.0305), (-0.0166, 0.0305)] + inner +
           [(0.0166, 0.0305), (0.018, 0.0305), (0.0176, 0.034), (0.0132, 0.0378),
            (0.0065, 0.0399), (0.0, 0.0403), (-0.0065, 0.0399), (-0.0132, 0.0378),
            (-0.0176, 0.034)])
    p.prism([(x, YB + y) for x, y in rim], zc, zb, "Wood", axis="z")
    rnd = random.Random(28)
    for _ in range(9):
        p.cyl(rnd.uniform(0.0006, 0.0012), 0.0, 0.0003, "Rope", segs=6,
              xf=T(rnd.uniform(-0.013, 0.013), YB + rnd.uniform(0.006, 0.032), zc) @
              RY(math.pi))
    return [p]


# --- Mess tin and skillet -------------------------------------------------------------------

def cook_mush():
    p = Part("body")
    # Grey, lumpy porridge in a soldier's oval mess tin, a spoon standing
    # in it and the wire bail folded back.
    sc = S(1.24, 1.0, 0.84)
    p.lathe([(0.0, YB), (0.022, YB), (0.0245, YB + 0.002), (0.0245, YB + 0.036),
             (0.0233, YB + 0.036), (0.0233, YB + 0.003), (0.0, YB + 0.003)], "Steel", segs=24,
            xf=sc @ UP)
    ring(p, 0.0243, 0.0008, "Steel", xf=T(0, YB + 0.036, 0) @ sc @ UP, segs=24, sides=4)
    p.lathe([(0.0, YB + 0.0285), (0.012, YB + 0.0282), (0.0232, YB + 0.0272), (0.0232, YB + 0.024),
             (0.0, YB + 0.024)], "Pebble", segs=20, xf=sc @ UP)
    rnd = random.Random(26)
    for k in range(9):
        a = rnd.uniform(0, 2 * math.pi)
        rr = math.sqrt(rnd.uniform(0, 1)) * 0.018
        blob(p, rnd.uniform(0.0025, 0.004), (rr * math.cos(a) * 1.2, YB + 0.0282,
                                             rr * math.sin(a) * 0.8),
             "Rope" if k % 3 == 0 else "Pebble", scale=(1.0, 0.55, 1.0), segs=8, rings=4)
    for sx in (-1, 1):
        p.box((0.003, 0.006, 0.005), (sx * 0.0312, YB + 0.031, 0.0), "Steel", bevel=0.0008, segs=1)
    bail = [(0.0312 * math.cos(math.pi * i / 12), YB + 0.031 + 0.03 * math.sin(math.pi * i / 12) *
             math.cos(1.25), 0.03 * math.sin(math.pi * i / 12) * math.sin(1.25)) for i in range(13)]
    tube_path(p, bail, 0.0009, "Steel", sides=4, closed=False)
    spoon(p, T(-0.006, YB + 0.0255, -0.004) @ RZ(0.25) @ RX(-0.25) @ S(0.85, 0.85, 0.85))
    return [p]


def cook_charcoal():
    p = Part("body")
    # Something burnt to a black, cracked lump on a plate, still smoking,
    # a last ember or two in it.
    floor = plate(p)
    blob(p, 0.0165, (-0.002, floor + 0.0072, 0.0), "Char", scale=(1.1, 0.62, 1.0),
         bumps=[((1, 0.4, 0.2), 0.25), ((-0.5, 0.6, -1), 0.2), ((0.3, 1, 0.6), 0.3),
                ((-1, 0.2, 0.6), 0.15), ((0, -1, 0), -0.5)], segs=16, rings=8)
    # Cracks through the crust, glowing faintly.
    for a, L in ((0.3, 0.014), (1.9, 0.011), (3.6, 0.012)):
        p.box((L, 0.0012, 0.0011), (L / 2, 0.0, 0.0), "Ember",
              xf=T(-0.002, floor + 0.0168, 0.0) @ RY(a) @ RZ(-0.35))
    rnd = random.Random(27)
    for k in range(6):
        a = rnd.uniform(0, 2 * math.pi)
        rr = rnd.uniform(0.008, 0.017)
        blob(p, rnd.uniform(0.002, 0.0035), (-0.002 + rr * math.cos(a), floor + 0.0015,
                                             rr * math.sin(a)), "Char", scale=(1, 0.7, 1),
             segs=6, rings=4)
    steam(p, -0.002, floor + 0.016, 0.0, 0.03, seed=3)
    return [p]


for item_id, fn in (("cook_flour", cook_flour), ("cook_dough", cook_dough),
                    ("cook_risen_dough", cook_risen_dough), ("cook_batter", cook_batter),
                    ("cook_butter", cook_butter), ("cook_hot_water", cook_hot_water),
                    ("cook_chopped_carrot", cook_chopped_carrot),
                    ("cook_chopped_onion", cook_chopped_onion),
                    ("cook_chopped_tomato", cook_chopped_tomato),
                    ("cook_soaked_beans", cook_soaked_beans), ("cook_tea", cook_tea),
                    ("cook_milk_tea", cook_milk_tea), ("cook_salad", cook_salad),
                    ("cook_boiled_egg", cook_boiled_egg),
                    ("cook_steamed_rice", cook_steamed_rice), ("cook_soup", cook_soup),
                    ("cook_custard", cook_custard), ("cook_rice_pudding", cook_rice_pudding),
                    ("cook_fried_egg", cook_fried_egg), ("cook_omelette", cook_omelette),
                    ("cook_pancake", cook_pancake), ("cook_fried_rice", cook_fried_rice),
                    ("cook_bread", cook_bread), ("cook_cake", cook_cake), ("cook_mush", cook_mush),
                    ("cook_charcoal", cook_charcoal)):
    if wanted(item_id):
        # Each dish sits on the floor of a box fitted to it (`dims`).
        item(item_id, lambda fn=fn: centred(fn()))
