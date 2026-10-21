"""A mallard, for the valleys' open water: low-poly, in the poses the game
swaps between instead of skinning it. Writes assets/models/animals/duck.glb.

Duck space, as in src/wildlife/duck_look.rs: metres, +Y up, -Z forward
(the bill), the origin at the waterline: a swimming duck floats with its
belly a few centimetres under it. A mallard is 0.55 m bill to tail, 0.9 m
across the wings. One object a pose, each at its own place along X in the
file so the checker never takes two poses for z-fighting (the game uses
each mesh about its own origin):

- `swim`, `swim_lod1`        floating, wings folded, head up
- `dabble`                   up-ending: head under, tail to the sky
- `up`, `mid`, `down`        the wingbeat, quick and shallow
- `glide`                    wings out, a little bowed, coming in
- `fly_lod1`                 every flying pose from far off: a few quads

The game paints a drake and a duck (the hen) from the same mesh, by
material: Drake (the drake's green head and neck; a hen's brown), Hide
(the chestnut breast), Pale (grey flanks and the white collar and tail),
Feather (the wings), Hoof (the black rump), Beak (the yellow bill).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, RZ, T, export, main_guard, make, report

main_guard()

SPACING = 1.2  # metres between poses along X in the file

# The body: elliptical rings along Z, tail end to breast, (z, half width,
# half height, centre height above the waterline).
BODY = [
    (0.25, 0.014, 0.012, 0.085),
    (0.2, 0.048, 0.032, 0.062),
    (0.12, 0.095, 0.058, 0.042),
    (0.03, 0.118, 0.074, 0.036),
    (-0.06, 0.11, 0.072, 0.042),
    (-0.115, 0.08, 0.06, 0.052),
    (-0.15, 0.04, 0.034, 0.062),
]

# The neck's line (y, z) and the head at its end, swimming and flying.
NECK_UP = [(0.07, -0.125), (0.125, -0.15), (0.172, -0.162)]
NECK_OUT = [(0.065, -0.14), (0.08, -0.2), (0.092, -0.245)]


def ring(z, rx, ry, cy, ox, n=8):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


def cross_ring(ox, y, z, dy, dz, r, n=8):
    """A ring of radius `r` round (ox, y, z), square to the direction
    (0, dy, dz): the neck's section wherever it points."""
    ln = math.hypot(dy, dz)
    vy, vz = dz / ln, -dy / ln
    return [(ox + r * math.cos(2 * math.pi * k / n),
             y + r * math.sin(2 * math.pi * k / n) * vy,
             z + r * math.sin(2 * math.pi * k / n) * vz) for k in range(n)]


def trunk(p, ox, xf, n=8):
    """Body, chestnut breast, black rump and the white tail."""
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in BODY], "Pale", xf=xf)
    p.sphere(0.072, (ox, 0.058, -0.118), "Hide", segs=n, rings=max(4, n // 2 + 1),
             scale=(1.15, 1.0, 1.0), xf=xf)
    p.sphere(0.04, (ox, 0.074, 0.175), "Hoof", segs=n, rings=max(4, n // 2 + 1),
             scale=(1.05, 0.8, 1.25), xf=xf)
    tail = [(-0.02, 0.19), (0.02, 0.19), (0.036, 0.27), (0.0, 0.295), (-0.036, 0.27)]
    p.prism([(ox + x, z) for x, z in tail], 0.08, 0.09, "Pale", axis="y",
            xf=xf @ RX(-0.3, about=(ox, 0.085, 0.19)))


def head(p, ox, xf, neck, n=8):
    """Neck along `neck` (y, z points), the white collar round its foot,
    the head at its end and the flat bill."""
    rings = []
    for i, (y, z) in enumerate(neck):
        a, b = neck[max(i - 1, 0)], neck[min(i + 1, len(neck) - 1)]
        r = 0.034 - 0.008 * i / (len(neck) - 1)
        rings.append(cross_ring(ox, y, z, b[0] - a[0], b[1] - a[1], r, n))
    p.loft(rings, "Drake", xf=xf)
    # The collar: a thin band across the neck, a little proud of it.
    (y0, z0), (y1, z1) = neck[0], neck[1]
    cy, cz = y0 + (y1 - y0) * 0.8, z0 + (z1 - z0) * 0.8
    a = math.atan2(-(y1 - y0), z1 - z0)
    p.cyl(0.0345, -0.005, 0.005, "Pale", segs=n, xf=xf @ T(ox, cy, cz) @ RX(a))
    hy, hz = neck[-1]
    hz -= 0.012
    p.sphere(0.037, (ox, hy + 0.01, hz), "Drake", segs=n, rings=max(4, n // 2 + 1),
             scale=(0.85, 0.9, 1.2), xf=xf)
    # The bill: broad and flat, out of the face, tipped down a little.
    b0 = hz - 0.03
    bill = [(-0.017, b0), (0.017, b0), (0.021, b0 - 0.045), (0.012, b0 - 0.06),
            (-0.012, b0 - 0.06), (-0.021, b0 - 0.045)]
    p.prism([(ox + x, z) for x, z in bill], hy - 0.006, hy + 0.008, "Beak", axis="y",
            xf=xf @ RX(-0.18, about=(ox, hy, b0)))


def folded_wings(p, ox, xf):
    """Wings closed along the back, their tips over the rump."""
    side_view = [(-0.08, 0.088), (0.0, 0.108), (0.12, 0.1), (0.215, 0.088), (0.16, 0.07),
                 (0.02, 0.066), (-0.06, 0.07)]
    for side in (-1, 1):
        x0 = ox + side * 0.056
        p.prism(side_view, x0 - 0.008, x0 + 0.008, "Feather", axis="x",
                xf=xf @ RZ(side * 0.62, about=(x0, 0.09, 0.0)))


# One wing from above (x out from the body, z back), the right one: a
# duck's pointed, narrow wing.
WING = [
    (0.0, -0.06), (0.12, -0.075), (0.26, -0.062), (0.38, -0.03), (0.445, 0.012),
    (0.36, 0.026), (0.24, 0.05), (0.12, 0.068), (0.0, 0.062),
]
SHOULDER = (0.07, 0.1, -0.06)


def wing(p, ox, side, lift, sweep=0.0, span=1.0, lod=False):
    pts = [(0.0, -0.06), (0.26, -0.062), (0.445, 0.012), (0.0, 0.062)] if lod else WING
    sx, sy, sz = SHOULDER
    plan = [(ox + side * (sx - 0.012 + x * span), sz + z) for x, z in pts]
    at = (ox + side * sx, sy, sz)
    xf = RZ(side * lift, about=at) @ RY(-side * sweep, about=at)
    p.prism(plan, sy - 0.006, sy + 0.006, "Feather", axis="y", xf=xf)


def lod_body(p, ox, xf, neck):
    p.loft([ring(z, rx, ry, cy, ox, 4) for z, rx, ry, cy in BODY[::2]], "Pale", xf=xf)
    (y0, z0), (y1, z1) = neck[0], neck[-1]
    # Neck and head as one box from the breast to the crown, the bill ahead.
    zc, yc = (z0 + z1) / 2, (y0 + y1) / 2
    p.box((0.06, abs(y1 - y0) + 0.07, abs(z1 - z0) + 0.07), (ox, yc, zc), "Drake", xf=xf)
    p.box((0.036, 0.014, 0.07), (ox, y1, z1 - 0.06), "Beak", xf=xf)


def swim(name, k, lod=False, dabble=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0.0, 0.0))
    xf = T(0, 0, 0)
    if dabble:
        # Up-ended about a point under the back: head and neck under, the
        # tail and rump up to the sky.
        xf = RX(-1.0, about=(ox, 0.03, 0.02))
    if lod:
        lod_body(p, ox, xf, NECK_UP)
        return make(p)
    trunk(p, ox, xf)
    head(p, ox, xf, NECK_UP)
    folded_wings(p, ox, xf)
    return make(p)


def fly(name, k, lift, sweep=0.0, span=1.0, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0.0, 0.0))
    xf = T(0, 0, 0)
    if lod:
        lod_body(p, ox, xf, NECK_OUT)
    else:
        trunk(p, ox, xf)
        head(p, ox, xf, NECK_OUT)
    for side in (-1, 1):
        wing(p, ox, side, lift, sweep, span, lod)
    return make(p)


swim("swim", 0)
swim("swim_lod1", 1, lod=True)
swim("dabble", 2, dabble=True)
fly("up", 3, math.radians(55), sweep=math.radians(6), span=0.94)
fly("mid", 4, math.radians(6))
fly("down", 5, math.radians(-42), sweep=math.radians(-4), span=0.9)
fly("glide", 6, math.radians(-4), sweep=math.radians(8))
fly("fly_lod1", 7, math.radians(2), sweep=math.radians(8), lod=True)

report()
export("assets/models/animals/duck.glb")
