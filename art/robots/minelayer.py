"""The minelayer: a low tracked crawler that sows pressure mines behind it.
An armoured hull with a sloped glacis on two rubber track units, a hazard-
striped mine hopper on its back (its weak point), a sensor head up front
and the chute at the rear. Writes assets/models/robots/minelayer.glb, and
the mine it lays as an item to assets/models/items/pressure_mine.glb.

World scale, metres, +Y up, -Z forward; the ground is y 0 under its
centre. About 1.5 m long, 1.1 m wide, 0.9 m tall (1.06 to the hopper's
lid). Objects and their origins (each the spawn offset of its part or
piece):

- `hull`     (0, 0.45, 0)       the armoured hull, 1.4 L x 0.8 W x 0.4 H
                                centred on its origin, the glacis sloping up
                                from the nose, fenders over the tracks, the
                                sensor head's neck
- `tread_l`  (-0.5, 0.25, 0)    the left track unit: 0.22 W x 0.5 H x 1.5 L, a
                                rubber belt round a sprocket, an idler and three
                                road wheels, axles into the hull
- `tread_r`  (0.5, 0.25, 0)     the right one, mirrored
- `hopper`   (0, 0.85, 0.3)     the mine hopper on its back: 0.6 x 0.42 x 0.56,
                                hazard-striped, a hatch on top
- `eye`      (0, 0.72, -0.62)   the sensor head at the front, its lens -Z
- `chute`    (0, 0.3, 0.78)     the mine chute at the rear, angled down
- `lamp`     (0, 0.95, 0)       a small amber beacon on a post, in front of
                                the hopper
- `mine`     (3, 0, 0)          a pressure mine as it lies laid, sitting on the
                                ground at its origin: 0.3 across, 0.07 tall, a
                                pressure plate and prong on top, a red LED at
                                (3.1, 0.07, 0) (apart in the file, as in the game)

Every object has `<name>_lod1` and `<name>_lod2` stand-ins (fewer sides,
no small detail), but the mine has only `mine_lod1`.

The item is the mine alone, centred on its bounds (item space, as
art/items/_kit.py): `pressure_mine` 0.3 x ~0.1 x 0.3.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, fillet, make, report, reset, section_rect

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
SEGS = (24, 12, 6)

HULL = (0.0, 0.45, 0.0)
HOPPER = (0.0, 0.85, 0.3)
EYE = (0.0, 0.72, -0.62)
CHUTE = (0.0, 0.3, 0.78)
LAMP = (0.0, 0.95, 0.0)
MINE = (3.0, 0.0, 0.0)


def suffix(lod):
    return "" if lod == 0 else f"_lod{lod}"


def clip_rect(poly, a0, a1, b0, b1):
    """Clip a convex 2D polygon to the rectangle [a0, a1] x [b0, b1]."""
    def clip(pts, inside, cut):
        out = []
        for i in range(len(pts)):
            p, q = pts[i - 1], pts[i]
            if inside(q):
                if not inside(p):
                    out.append(cut(p, q))
                out.append(q)
            elif inside(p):
                out.append(cut(p, q))
        return out

    def at_a(v):
        return lambda p, q: (v, p[1] + (q[1] - p[1]) * (v - p[0]) / (q[0] - p[0]))

    def at_b(v):
        return lambda p, q: (p[0] + (q[0] - p[0]) * (v - p[1]) / (q[1] - p[1]), v)

    for inside, cut in ((lambda p: p[0] >= a0, at_a(a0)), (lambda p: p[0] <= a1, at_a(a1)),
                        (lambda p: p[1] >= b0, at_b(b0)), (lambda p: p[1] <= b1, at_b(b1))):
        if not poly:
            break
        poly = clip(poly, inside, cut)
    return poly


def hazard(p, mid, half, b0, b1, face, mat="Warn", width=0.055, pitch=0.12, proud=0.005):
    """Diagonal stripes on a flat face, `half` either side of its middle
    `mid` (z on an x face, x on a z face), from height b0 to b1. `face` is
    (axis, at, sign): the face's normal axis ('x' or 'z'), its position
    and which way it faces."""
    axis, at, sign = face
    k = -4
    while k * pitch < 2 * half:
        a = -half + k * pitch
        para = [(a, b0), (a + width, b0), (a + width + (b1 - b0), b1), (a + (b1 - b0), b1)]
        poly = clip_rect(para, -half, half, b0, b1)
        k += 1
        if len(poly) < 3:
            continue
        lo, hi = sorted((at - sign * 0.001, at + sign * proud))
        if axis == "x":
            # profile (z, y), extruded along x
            p.prism([(mid - sign * u, v) for u, v in poly], lo, hi, mat, axis="x")
        else:
            # profile (x, y), extruded along z
            p.prism([(mid + sign * u, v) for u, v in poly], lo, hi, mat, axis="z")


def mine(p, cx, lod):
    """A pressure mine lying on the ground, centred on (cx, 0, 0)."""
    n = 24 if lod == 0 else 12
    ch = 0.004 if lod == 0 else 0.0
    p.lathe([(0.0, 0.0), (0.142, 0.0), (0.15, 0.008), (0.15, 0.036), (0.128, 0.056),
             (0.0, 0.056)], "Dark", segs=n, xf=T(cx, 0, 0) @ UP)
    # A hazard ring round its rim.
    p.lathe([(0.149, 0.014), (0.155, 0.014), (0.155, 0.03), (0.149, 0.03)], "Warn", segs=n,
            xf=T(cx, 0, 0) @ UP, closed=True)
    # The pressure plate, ribbed, standing proud of the top.
    p.cyl(0.075, 0.05, 0.07, "Steel", segs=n, xf=T(cx, 0, 0) @ UP, chamfer=ch)
    if lod == 0:
        for k in range(3):
            p.box((0.12, 0.006, 0.01), (0.0, 0.072, 0.0), "Steel", segs=1,
                  xf=T(cx, 0, 0) @ RY(math.pi / 3 * k))
        # The prong (the fuse's tilt rod) and the arming LED.
        p.cyl(0.005, 0.07, 0.1, "Steel", segs=6, xf=T(cx, 0, 0) @ UP)
        p.cyl(0.009, 0.05, 0.068, "Dark", segs=8, xf=T(cx + 0.1, 0, 0) @ UP)
        p.lathe([(0.0, 0.066), (0.007, 0.066), (0.006, 0.07), (0.0, 0.072)], "Eye", segs=8,
                xf=T(cx + 0.1, 0, 0) @ UP)


def tread(sx, lod):
    """A track unit on side sx (-1 left, +1 right)."""
    n = SEGS[lod]
    full = lod == 0
    x0 = sx * 0.5
    t = Part(("tread_l" if sx < 0 else "tread_r") + suffix(lod), origin=(x0, 0.25, 0.0))
    # The belt: a stadium 1.5 long and 0.5 tall, 5 cm thick, its grousers
    # standing 1 cm proud of it to the ground.
    r, zc, yc = 0.225, 0.5, 0.26
    arc_n = 10 if full else (6 if lod == 1 else 4)
    path = []
    for i in range(arc_n + 1):  # the back end, bottom to top
        a = -math.pi / 2 + math.pi * i / arc_n
        path.append((zc + r * math.cos(a), yc + r * math.sin(a)))
    for i in range(arc_n + 1):  # the front end, top to bottom
        a = math.pi / 2 + math.pi * i / arc_n
        path.append((-zc + r * math.cos(a), yc + r * math.sin(a)))
    t.sweep(path, section_rect(0.22, 0.05), "Rubber", closed=True, xf=T(x0, 0, 0))
    if full:
        # Grousers across the belt's tread, top and bottom runs.
        for k in range(9):
            z = -0.4 + 0.1 * k
            for y in (0.008, 0.512):
                t.box((0.21, 0.016, 0.025), (x0, y, z), "Rubber", segs=1)
    # Sprocket at the front, idler at the back, three road wheels.
    wheels = [(-0.5, 0.26, 0.17), (0.5, 0.26, 0.17)] + [(z, 0.13, 0.09) for z in (-0.25, 0.0, 0.25)]
    for z, y, rr in wheels:
        t.cyl(rr, -0.07, 0.07, "Dark", segs=n, xf=T(x0, y, z) @ SIDE,
              chamfer=0.01 if lod < 2 else 0.0)
        # Its axle into the hull.
        a, b = sorted((x0 + sx * 0.085, 0.36 * sx))
        t.cyl(0.025, a, b, "Steel", segs=max(6, n // 2), xf=T(0, y, z) @ SIDE)
        if lod < 2:
            t.cyl(rr * 0.45, 0.0, 0.085, "Steel", segs=max(8, n // 2),
                  xf=T(x0, y, z) @ SIDE @ RY(0 if sx > 0 else math.pi), chamfer=0.006)
    if full:
        # The sprocket's teeth.
        for k in range(10):
            a = 2 * math.pi * k / 10
            t.box((0.04, 0.035, 0.03), (x0, 0.0, 0.18), "Steel", segs=1,
                  xf=T(0, 0.26, -0.5) @ RX(a))
    # The frame the road wheels hang on, inboard of them.
    xa, xb = sorted((x0 - sx * 0.072, x0 - sx * 0.1))
    t.box((xb - xa, 0.08, 0.84), ((xa + xb) / 2, 0.2, 0.0), "Dark",
          bevel=0.008 if lod < 2 else 0.0, segs=1)
    return t


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.015 if lod < 2 else 0.0
    out = []

    # --- Hull -------------------------------------------------------------------------
    hull = Part("hull" + suffix(lod), origin=HULL)
    side = fillet([(-0.62, 0.25), (0.64, 0.25), (0.7, 0.3), (0.7, 0.58), (0.62, 0.65),
                   (-0.42, 0.65), (-0.7, 0.36), (-0.7, 0.3)], 0.03, 3 if full else 1)
    hull.prism(side, -0.4, 0.4, "Shell", axis="x", bevel=bev, segs=2 if full else 1)
    # Dark belly plate and the fenders over the tracks.
    hull.prism(fillet([(-0.6, 0.24), (0.62, 0.24), (0.66, 0.32), (-0.66, 0.32)], 0.01, 1),
               -0.37, 0.37, "Dark", axis="x")
    for sx in (-1, 1):
        a, b = sorted((sx * 0.36, sx * 0.63))
        fender = fillet([(-0.74, 0.5), (-0.62, 0.53), (0.7, 0.53), (0.74, 0.5), (0.74, 0.555),
                         (-0.74, 0.555)], [0.0, 0.0, 0.0, 0.0, 0.01, 0.01], 1)
        hull.prism(fender, a, b, "Dark", axis="x")
    # The sensor head's neck, up off the glacis.
    hull.cyl(0.03, 0.5, 0.68, "Dark", segs=max(6, n // 2), xf=T(0, 0, -0.5) @ UP)
    if lod < 2:
        # A hatch on the deck.
        hull.box((0.3, 0.012, 0.26), (0.0, 0.654, -0.24), "Dark", bevel=0.004, segs=1)
    if full:
        hull.cyl(0.015, -0.13, 0.13, "Steel", segs=8, xf=T(0, 0.666, -0.33) @ SIDE)
        # Hazard chevrons on the glacis, a lit strip down each flank.
        gl = math.atan2(0.29, 0.28)
        for x in (-0.22, 0.22):
            hull.box((0.12, 0.006, 0.12), (0.0, 0.0, 0.0), "Warn", segs=1,
                     xf=T(x, 0.51, -0.56) @ RX(-gl) @ T(0, 0.002, 0))
        for sx in (-1, 1):
            hull.box((0.01, 0.018, 0.5), (sx * 0.401, 0.6, 0.05), "Glow", segs=1)
        # Tow eyes at the back.
        for sx in (-1, 1):
            hull.tube(0.035, 0.018, -0.008, 0.008, "Steel", segs=10,
                      xf=T(sx * 0.28, 0.33, 0.705) @ SIDE)
    out.append(hull)

    # --- Tracks -------------------------------------------------------------------------
    out.append(tread(-1, lod))
    out.append(tread(1, lod))

    # --- Hopper -------------------------------------------------------------------------
    hx, hy, hz = HOPPER
    hop = Part("hopper" + suffix(lod), origin=HOPPER)
    HZ0, HZ1 = 0.05, 0.61
    hc = (HZ0 + HZ1) / 2
    hop.box((0.6, 0.42, HZ1 - HZ0), (hx, hy, hc), "Dark", bevel=0.02 if lod < 2 else 0.0,
            segs=2 if full else 1)
    # A lid and its hatch.
    hop.box((0.56, 0.03, HZ1 - HZ0 - 0.04), (hx, hy + 0.22, hc), "Shell",
            bevel=0.01 if lod < 2 else 0.0, segs=1)
    if lod < 2:
        hop.box((0.3, 0.02, 0.26), (hx, hy + 0.24, hc), "Steel", bevel=0.005, segs=1)
        hop.cyl(0.012, -0.08, 0.08, "Steel", segs=8, xf=T(hx, hy + 0.262, hc - 0.09) @ SIDE)
    # Hazard stripes round its four sides (simple bands on the stand-ins).
    b0, b1 = hy - 0.14, hy + 0.13
    if lod < 2:
        hazard(hop, hc, 0.26, b0, b1, ("x", 0.3, 1))
        hazard(hop, hc, 0.26, b0, b1, ("x", -0.3, -1))
        hazard(hop, hx, 0.26, b0, b1, ("z", HZ0, -1))
        hazard(hop, hx, 0.26, b0, b1, ("z", HZ1, 1))
    else:
        hop.box((0.61, 0.1, HZ1 - HZ0 + 0.01), (hx, hy, hc), "Warn", segs=1)
    if full:
        # Lit level gauge on its front, under the stripes.
        hop.box((0.2, 0.025, 0.006), (hx, hy - 0.17, HZ0 - 0.002), "Glow", segs=1)
    out.append(hop)

    # --- Eye ------------------------------------------------------------------------------
    ex, ey, ez = EYE
    eye = Part("eye" + suffix(lod), origin=EYE)
    eye.box((0.22, 0.12, 0.18), (ex, ey, -0.55), "Dark", bevel=0.02 if lod < 2 else 0.0,
            segs=2 if full else 1)
    aim = T(ex, ey, 0) @ RY(math.pi)
    eye.cyl(0.045, 0.62, 0.65, "Dark", segs=max(8, n), xf=aim, chamfer=0.006 if lod < 2 else 0.0)
    eye.cyl(0.032, 0.645, 0.656, "Eye", segs=max(8, n), xf=aim)
    if lod < 2:
        # A brow over the lens and two small range-finder ports.
        eye.box((0.24, 0.02, 0.06), (ex, ey + 0.065, -0.62), "Shell", segs=1)
        for sx in (-1, 1):
            eye.cyl(0.012, 0.635, 0.646, "Eye", segs=8, xf=T(ex + sx * 0.075, ey - 0.02, 0)
                    @ RY(math.pi))
    out.append(eye)

    # --- Chute ------------------------------------------------------------------------------
    cx, cy, cz = CHUTE
    chute = Part("chute" + suffix(lod), origin=CHUTE)
    tilt = RX(0.55, about=CHUTE)
    chute.box((0.26, 0.025, 0.44), (cx, cy - 0.06, cz), "Steel", bevel=0.005 if lod < 2 else 0.0,
              segs=1, xf=tilt)
    for sx in (-1, 1):
        chute.box((0.025, 0.12, 0.44), (cx + sx * 0.14, cy, cz), "Dark",
                  bevel=0.005 if lod < 2 else 0.0, segs=1, xf=tilt)
    if lod < 2:
        # A flap hinged over its mouth.
        chute.box((0.3, 0.02, 0.12), (cx, cy + 0.07, cz + 0.12), "Dark", segs=1, xf=tilt)
    if full:
        chute.box((0.006, 0.04, 0.2), (cx + 0.1555, cy, cz + 0.05), "Warn", segs=1, xf=tilt)
        chute.box((0.006, 0.04, 0.2), (cx - 0.1555, cy, cz + 0.05), "Warn", segs=1, xf=tilt)
    out.append(chute)

    # --- Beacon -----------------------------------------------------------------------------
    lx, ly, lz = LAMP
    lamp = Part("lamp" + suffix(lod), origin=LAMP)
    lamp.cyl(0.016, 0.64, 0.93, "Dark", segs=max(6, n // 2), xf=T(lx, 0, lz) @ UP)
    lamp.cyl(0.034, 0.9, 0.93, "Dark", segs=max(8, n), xf=T(lx, 0, lz) @ UP,
             chamfer=0.004 if lod < 2 else 0.0)
    lamp.lathe([(0.0, 0.925), (0.03, 0.925), (0.03, 0.95), (0.022, 0.975), (0.0, 0.985)], "Lamp",
               segs=max(8, n), xf=T(lx, 0, lz) @ UP)
    out.append(lamp)

    # --- Mine -------------------------------------------------------------------------------
    if lod < 2:
        m = Part("mine" + suffix(lod), origin=MINE)
        mine(m, MINE[0], lod)
        out.append(m)
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        make(p)
report()
export("assets/models/robots/minelayer.glb")

# The mine as an item, centred on its bounds (as heavy.py's salvage).
reset()
p = Part("pressure_mine")
mine(p, 0.0, 0)
obj = make(p)
me = obj.data
lo = [min(v.co[k] for v in me.vertices) for k in range(3)]
hi = [max(v.co[k] for v in me.vertices) for k in range(3)]
mid = [(a + b) / 2 for a, b in zip(lo, hi)]
for v in me.vertices:
    v.co = [v.co[k] - mid[k] for k in range(3)]
size = [hi[k] - lo[k] for k in range(3)]
# (Blender is Z up: game x, y, z = its x, z, y.)
print(f"{p.name} dims {size[0]:.3f} {size[2]:.3f} {size[1]:.3f}")
export("assets/models/items/pressure_mine.glb")
