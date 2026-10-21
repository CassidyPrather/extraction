"""The plough: the Custodian's snow plough, a tracked rotary snow-blower that
keeps the mountain pass road open. Municipal roadworks plant gone feral:
Shell and Dark bodywork, hazard chevrons on the auger housing, a big amber
beacon on the roof. Nobody drives it; the cab is a sensor house with dark
glass. Writes assets/models/robots/plough.glb.

World scale, metres, +Y up, -Z forward; the ground is y 0 under its
middle. About 3.1 m wide at the auger, 2.2 m across the body, 4.9 m long,
2.6 m to the cab roof (3.0 to the top of the beacon). Objects and their
origins (each the spawn offset of its part; every mesh is relative to its
own origin):

- `hull`     (0, 1.1, 0.4)      the body and engine deck on the tracks: the
                                sensor house ("cab") at its front top with
                                dark windows, a radiator grille and a
                                chevroned bumper at the back, and a dark
                                recess in the deck under the hatch
- `track_l`  (-0.85, 0.45, 0.4) the left track unit: 0.5 W x 0.9 H x 3.6 L, a
                                rubber belt round a sprocket, an idler, four
                                road wheels and two return rollers
- `track_r`  (0.85, 0.45, 0.4)  the right one, mirrored
- `auger`    (0, 0.75, -2.0)    the rotary drum across the front, its axis
                                along X through the origin: 3.0 wide, 0.9
                                across the flights, two opposed helices
                                meeting at paddles in the middle (the game
                                spins it about X)
- `housing`  (0, 0.85, -1.95)   the fixed shroud round the drum, open at the
                                front and bottom: side plates and a top rail
                                with chevrons, a steel cutting edge, skid
                                shoes, the push frame into the hull and the
                                trunk up to the chute's swivel collar
- `chute`    (0.6, 2.0, -1.2)   the discharge chute: a swivel ring on the
                                collar, a pipe rising and curving out to +X
                                (downhill), a deflector hood at its mouth
- `beacon`   (0, 2.75, 0)       the big amber beacon on its post on the roof
- `eye`      (0, 2.2, -0.75)    the sensor bar across the cab front
- `hatch`    (0, 1.75, 1.9)     the engine hatch on the back of the deck: a
                                panel with two handles and a yellow pull ring
- `cell`     (0, 1.5, 1.85)     the power cell in the recess under the hatch
- `bank`     (6, 0, 0)          a 4 m section of snow windrow along Z, 2 m
                                wide, ~1.2 m high, its base on y 0, centred on
                                its origin in X and Z; its lumps repeat every
                                4 m so the game can tile it (apart in the file)

Every object has `<name>_lod1` and `<name>_lod2` stand-ins (fewer sides,
no small detail), but the bank has only `bank_lod1`.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from mathutils import Vector

from common import (RX, RY, T, Part, export, fillet, join, make, report, reset, section_circle,
                    section_rect)

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
SEGS = (24, 12, 6)

HULL = (0.0, 1.1, 0.4)
TRACK_X = 0.85
TRACK = (0.45, 0.4)  # (y, z) of the track units' origins
AUGER = (0.0, 0.75, -2.0)
HOUSING = (0.0, 0.85, -1.95)
CHUTE = (0.6, 2.0, -1.2)
BEACON = (0.0, 2.75, 0.0)
EYE = (0.0, 2.2, -0.75)
HATCH = (0.0, 1.75, 1.9)
CELL = (0.0, 1.5, 1.85)
BANK = (6.0, 0.0, 0.0)

# The auger's axis (y, z) and the shroud's radii round it.
AY, AZ = AUGER[1], AUGER[2]
SHROUD_IN, SHROUD_OUT = 0.5, 0.54
# The chute's swivel axis (x, z) and the top of the housing's collar.
CX, CZ = CHUTE[0], CHUTE[2]
COLLAR_TOP = 1.85


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


def plate(p, poly, frame, n0, n1, mat):
    """A flat 2D polygon (u, v) extruded from n0 to n1 along a face's
    normal. `frame` is (origin, U, V, N) in game space."""
    o, U, V, N = frame
    rings = [[o + U * u + V * v + N * n for u, v in poly] for n in (n0, n1)]
    p.loft(rings, mat)


def chevrons(p, frame, half, v0, v1, width=0.07, pitch=0.16, proud=0.006, mat="Warn"):
    """Hazard chevrons on a flat face: diagonal stripes mirrored about u 0
    into V shapes pointing down (-v), across u in [-half, half], v in
    [v0, v1]. `frame` as `plate`."""
    h = v1 - v0
    gap = 0.004
    k = int(math.floor((-h - width) / pitch)) - 1
    while k * pitch < half:
        a = k * pitch
        para = [(a, v0), (a + width, v0), (a + width + h, v1), (a + h, v1)]
        k += 1
        poly = clip_rect(para, gap, half, v0, v1)
        if len(poly) < 3:
            continue
        for s in (1, -1):
            pts = [(s * u, v) for u, v in poly]
            if s < 0:
                pts.reverse()
            plate(p, pts, frame, -0.001, proud, mat)


def rod(p, a, b, r, mat, segs):
    """A round bar from game-space point a to b."""
    a, b = Vector(a), Vector(b)
    d = b - a
    rot = d.to_track_quat("Z", "Y").to_matrix().to_4x4()
    p.cyl(r, 0.0, d.length, mat, segs=segs, xf=T(*a) @ rot)


def pipe(p, path, radius, mat, n, mouth=None):
    """A round pipe along a path of game-space points lying in an XY plane
    (z constant). `mouth` = (r_in, depth) turns its last end into an open
    lip going `depth` back inside."""
    pts = [Vector(q) for q in path]
    ez = Vector((0, 0, 1))
    rings = []

    def ring(c, t, r):
        e2 = t.cross(ez).normalized()
        return [c + ez * (r * math.cos(2 * math.pi * k / n)) + e2 * (r * math.sin(2 * math.pi * k / n))
                for k in range(n)]

    for i, c in enumerate(pts):
        t = (pts[min(i + 1, len(pts) - 1)] - pts[max(i - 1, 0)]).normalized()
        rings.append(ring(c, t, radius))
    if mouth:
        r_in, depth = mouth
        t = (pts[-1] - pts[-2]).normalized()
        rings.append(ring(pts[-1], t, r_in))
        rings.append(ring(pts[-1] - t * depth, t, r_in))
    p.loft(rings, mat)


# ---------------------------------------------------------------------------
# Tracks


def track(sx, lod):
    n = SEGS[lod]
    full = lod == 0
    x0 = sx * TRACK_X
    ty, tz = TRACK
    t = Part(("track_l" if sx < 0 else "track_r") + suffix(lod), origin=(x0, ty, tz))
    # The belt: a stadium 3.6 long and 0.86 tall, 8 cm thick, the grousers
    # standing 2 cm proud of it to make 0.9.
    r = 0.39
    z_front, z_back = tz - 1.37, tz + 1.37
    arc_n = 12 if full else (8 if lod == 1 else 4)
    path = []
    for i in range(arc_n + 1):  # the back end, bottom to top
        a = -math.pi / 2 + math.pi * i / arc_n
        path.append((z_back + r * math.cos(a), ty + r * math.sin(a)))
    for i in range(arc_n + 1):  # the front end, top to bottom
        a = math.pi / 2 + math.pi * i / arc_n
        path.append((z_front + r * math.cos(a), ty + r * math.sin(a)))
    t.sweep(path, section_rect(0.5, 0.08), "Rubber", closed=True, xf=T(x0, 0, 0))
    if full:
        # Grousers across the top and bottom runs.
        z = z_front
        while z <= z_back + 1e-6:
            for y in (0.015, ty * 2 - 0.015):
                t.box((0.48, 0.03, 0.06), (x0, y, z), "Rubber", segs=1)
            z += 0.137
    # Idler at the front, sprocket at the back (raised a touch), four road
    # wheels on the bottom run, two return rollers under the top run.
    wheels = ([(z_front, ty, 0.32, 0.36), (z_back, ty, 0.33, 0.36)]
              + [(z, 0.31, 0.2, 0.38) for z in (-0.45, 0.15, 0.75, 1.35)])
    for z, y, rr, w in wheels:
        t.cyl(rr, -w / 2, w / 2, "Dark", segs=n, xf=T(x0, y, z) @ SIDE,
              chamfer=0.015 if lod < 2 else 0.0)
        # Its axle into the hull.
        a, b = sorted((x0, sx * 0.5))
        t.cyl(0.04, a, b, "Steel", segs=max(6, n // 2), xf=T(0, y, z) @ SIDE)
        if lod < 2:
            a, b = sorted((x0 + sx * w / 2 - sx * 0.01, x0 + sx * (w / 2 + 0.04)))
            t.cyl(rr * 0.45, a, b, "Steel", segs=max(8, n // 2), xf=T(0, y, z) @ SIDE,
                  chamfer=0.008)
    for z in (0.0, 0.9):
        t.cyl(0.07, -0.2, 0.2, "Dark", segs=max(8, n // 2), xf=T(x0, 0.72, z) @ SIDE)
    if full:
        # The sprocket's teeth.
        for k in range(14):
            a = 2 * math.pi * k / 14
            t.box((0.12, 0.05, 0.05), (x0, 0.0, 0.335), "Steel", segs=1,
                  xf=T(0, ty, z_back) @ RX(a))
    # The frame the wheels hang on, between them and the hull.
    xa, xb = sorted((x0 - sx * 0.2, x0 - sx * 0.24))
    t.box((xb - xa, 0.38, 2.4), ((xa + xb) / 2, 0.55, tz), "Dark",
          bevel=0.015 if lod < 2 else 0.0, segs=1)
    return t


# ---------------------------------------------------------------------------
# The body


def hull(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.02 if lod < 2 else 0.0
    sfx = suffix(lod)
    deck = Part("hull_deck" + sfx, origin=HULL)
    # The engine deck over the tracks, its bonnet sloping down to the front.
    side = fillet([(-1.45, 0.92), (2.35, 0.92), (2.35, 1.62), (2.22, 1.75), (-0.95, 1.75),
                   (-1.45, 1.4)], 0.05, 3 if full else 1)
    deck.prism(side, -1.1, 1.1, "Shell", axis="x", bevel=bev, segs=2 if full else 1)
    # The belly between the tracks.
    deck.box((1.16, 0.6, 3.8), (0.0, 0.65, 0.4), "Dark", bevel=0.02 if lod < 2 else 0.0, segs=1)
    # The recess under the engine hatch, its walls and floor dark.
    cut = Part("hull_cut")
    cut.box((0.84, 0.65, 0.68), (0.0, 1.575, 1.86), "Dark")
    deck.cut(cut, paint=True)

    rest = Part("hull_rest" + sfx, origin=HULL)
    # The sensor house: a box with a dark roof plate and dark glass.
    rest.box((1.4, 0.87, 1.25), (0.0, 2.165, -0.075), "Shell", bevel=0.03 if lod < 2 else 0.0,
             segs=2 if full else 1)
    rest.box((1.5, 0.05, 1.38), (0.0, 2.605, -0.08), "Dark", bevel=0.012 if lod < 2 else 0.0,
             segs=1)
    for sx in (-1, 1):
        rest.box((0.012, 0.42, 0.9), (sx * 0.703, 2.24, -0.13), "Dark", segs=1)
    for y in (1.97, 2.42):
        rest.box((1.12, 0.2, 0.012), (0.0, y, -0.703), "Dark", segs=1)
    # The radiator grille at the back.
    rest.box((1.6, 0.6, 0.04), (0.0, 1.3, 2.36), "Dark", bevel=0.008 if lod < 2 else 0.0, segs=1)
    if lod < 2:
        for k in range(7):
            rest.box((1.48, 0.035, 0.03), (0.0, 1.06 + k * 0.08, 2.385), "Steel", segs=1)
    # The rear bumper with chevrons, over the back of the tracks.
    rest.box((2.0, 0.26, 0.16), (0.0, 0.72, 2.37), "Dark", bevel=0.015 if lod < 2 else 0.0,
             segs=1)
    rear = (Vector((0.0, 0.72, 2.45)), Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1)))
    if lod < 2:
        chevrons(rest, rear, 0.95, -0.1, 0.1, width=0.06, pitch=0.15)
    else:
        rest.box((1.9, 0.18, 0.012), (0.0, 0.72, 2.452), "Warn", segs=1)
    if lod < 2:
        # The exhaust stack, by the hatch.
        rest.cyl(0.07, 1.7, 2.35, "Dark", segs=max(8, n // 2), xf=T(-0.8, 0, 1.2) @ UP)
        rest.tube(0.085, 0.06, 2.3, 2.42, "Rusty", segs=max(8, n // 2), xf=T(-0.8, 0, 1.2) @ UP)
        # Hinge knuckles for the hatch along its front edge.
        for x in (-0.3, 0.3):
            rest.cyl(0.025, x - 0.06, x + 0.06, "Steel", segs=8, xf=T(0, 1.765, 1.47) @ SIDE)
    if full:
        # Amber markers at the back corners, work lamps on the roof.
        for sx in (-1, 1):
            rest.box((0.12, 0.08, 0.03), (sx * 0.95, 1.5, 2.36), "Beacon", bevel=0.008, segs=1)
            rest.lathe([(0.0, -0.06), (0.07, -0.06), (0.07, 0.04), (0.0, 0.04)], "Dark", segs=12,
                       xf=T(sx * 0.6, 2.7, -0.68))
            rest.cyl(0.058, -0.068, -0.058, "Lamp", segs=12, xf=T(sx * 0.6, 2.7, -0.68))
            rest.box((0.03, 0.08, 0.03), (sx * 0.6, 2.645, -0.66), "Dark", segs=1)
        # An engine louvre on each flank, rust weeping from it and the seams.
        for sx in (-1, 1):
            xo = sx * 1.1
            a_, b_ = sorted((xo - sx * 0.002, xo + sx * 0.012))
            rest.prism(fillet([(0.45, 1.12), (1.35, 1.12), (1.35, 1.5), (0.45, 1.5)], 0.02, 1),
                       a_, b_, "Dark", axis="x")
            for k in range(6):
                y = 1.17 + 0.057 * k
                rest.box((0.04, 0.012, 0.84), (0.0, 0.0, 0.0), "Steel", segs=1,
                         xf=T(xo + sx * 0.012, y, 0.9))
            for z, top, h, w in ((0.55, 1.12, 0.17, 0.05), (0.95, 1.12, 0.12, 0.035),
                                 (1.25, 1.12, 0.15, 0.045), (-0.62, 1.68, 0.5, 0.05),
                                 (-0.25, 1.4, 0.3, 0.03), (1.95, 1.62, 0.42, 0.06),
                                 (2.2, 1.2, 0.2, 0.04)):
                a_, b_ = sorted((xo - sx * 0.001, xo + sx * 0.004))
                rest.prism([(z - w / 2, top), (z + w / 2, top), (z + w * 0.1, top - h),
                            (z - w * 0.15, top - h * 0.8)], a_, b_, "Rusty", axis="x")
            for z in (0.5, 1.3):
                for y in (1.15, 1.47):
                    rest.cyl(0.012, 0.0, 0.012, "Steel", segs=6,
                             xf=T(xo + sx * 0.01, y, z) @ RY(sx * math.pi / 2))
        # An antenna stub on the roof.
        rest.cyl(0.012, 2.62, 3.05, "Dark", segs=6, xf=T(-0.55, 0, 0.4) @ UP)
    return join("hull" + sfx, deck, rest)


def auger(lod):
    n = SEGS[lod]
    full = lod == 0
    p = Part("auger" + suffix(lod), origin=AUGER)
    ax = T(0, AY, AZ) @ SIDE
    # Drum and shaft.
    p.cyl(0.15, -1.48, 1.48, "Dark", segs=n, xf=ax, chamfer=0.02 if lod < 2 else 0.0)
    p.cyl(0.06, -1.56, 1.56, "Steel", segs=max(8, n // 2), xf=ax)
    # Two opposed helices from the middle to the ends.
    pitch, x0, x1 = 0.6, 0.14, 1.44
    per_turn = (28, 14, 8)[lod]
    th = 0.025
    ri, ro = 0.14, 0.44
    turns = (x1 - x0) / pitch
    steps = int(math.ceil(turns * per_turn))
    for sx in (-1, 1):
        for start in ((0.0, math.pi) if lod < 2 else (0.0,)):
            rings = []
            for i in range(steps + 1):
                f = i / steps
                a = start + 2 * math.pi * turns * f
                x = sx * (x0 + (x1 - x0) * f)
                c, s = math.cos(a), math.sin(a)
                rings.append([Vector((x + dx, AY + rr * s, AZ + rr * c))
                              for dx, rr in ((-th / 2, ri), (th / 2, ri), (th / 2, ro),
                                             (-th / 2, ro))])
            p.loft(rings, "Steel")
    # Paddles in the middle that throw the snow up into the trunk.
    for k in range(2 if lod < 2 else 1):
        p.box((0.26, 0.29, 0.03), (0.0, 0.285, 0.0), "Steel", segs=1,
              xf=T(0, AY, AZ) @ RX(math.pi * k + 0.4))
    if lod < 2:
        # End rings where the flights meet the bearings.
        for sx in (-1, 1):
            a, b = sorted((sx * 1.44, sx * 1.48))
            p.cyl(0.2, a, b, "Dark", segs=n, xf=T(0, AY, AZ) @ SIDE)
    if full:
        # Serrated teeth on the flights' outer edges, every half turn.
        for sx in (-1, 1):
            for i in range(1, int(turns * 2)):
                f = i / (turns * 2)
                for start in (0.0, math.pi):
                    a = start + 2 * math.pi * turns * f
                    x = sx * (x0 + (x1 - x0) * f)
                    p.box((0.04, 0.06, 0.05), (0.0, 0.0, 0.0), "Steel", segs=1,
                          xf=T(x, AY, AZ) @ RX(-a) @ T(0, 0, 0.45))
    return p


def housing(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.012 if lod < 2 else 0.0
    p = Part("housing" + suffix(lod), origin=HOUSING)
    arc_n = (36, 18, 9)[lod]

    def at(theta, r):
        """(z, y) on a circle round the auger axis, theta from the front
        (-Z) up over the top (degrees)."""
        t = math.radians(theta)
        return (AZ - r * math.cos(t), AY + r * math.sin(t))

    # The shroud: a thick arc from in front over the top and back to the
    # bottom, open at the front and bottom.
    t0, t1 = 22.0, 250.0
    band = ([at(t0 + (t1 - t0) * i / arc_n, SHROUD_OUT) for i in range(arc_n + 1)]
            + [at(t1 - (t1 - t0) * i / arc_n, SHROUD_IN) for i in range(arc_n + 1)])
    p.prism(band, -1.52, 1.52, "Shell", axis="x")
    # Side plates: round at the back, square at the front bottom.
    sp = [at(250.0 * i / arc_n, 0.56) for i in range(arc_n + 1)] + [(AZ - 0.56, 0.2)]
    for sx in (-1, 1):
        a, b = sorted((sx * 1.5, sx * 1.545))
        p.prism(sp, a, b, "Dark", axis="x")
        # The bearing hub.
        a, b = sorted((sx * 1.54, sx * 1.6))
        p.cyl(0.1, a, b, "Steel", segs=max(8, n // 2), xf=T(0, AY, AZ) @ SIDE,
              chamfer=0.01 if lod < 2 else 0.0)
        # Skid shoes.
        p.box((0.06, 0.22, 0.55), (sx * 1.575, 0.13, AZ - 0.15), "Steel",
              bevel=0.01 if lod < 2 else 0.0, segs=1)
        frame = (Vector((sx * 1.545, AY, AZ)), Vector((0, 0, -sx)), Vector((0, 1, 0)),
                 Vector((sx, 0, 0)))
        if lod < 2:
            chevrons(p, frame, 0.4, -0.34, 0.24, width=0.08, pitch=0.19)
        else:
            plate(p, [(-0.4, -0.05), (0.4, -0.05), (0.4, 0.15), (-0.4, 0.15)], frame, -0.001,
                  0.006, "Warn")
    # The top rail along the shroud's front lip, chevroned.
    th = 38.0
    tr = math.radians(th)
    nrm = Vector((0, math.sin(tr), -math.cos(tr)))
    tan = Vector((0, math.cos(tr), math.sin(tr)))
    zc, yc = at(th, 0.57)
    p.box((3.08, 0.06, 0.24), (0, 0, 0), "Dark", bevel=bev, segs=1,
          xf=T(0, yc, zc) @ RX(tr - math.pi / 2))
    rail = (Vector((0, yc, zc)) + nrm * 0.03, Vector((1, 0, 0)), tan, nrm)
    if lod < 2:
        chevrons(p, rail, 1.5, -0.1, 0.1, width=0.07, pitch=0.17)
    else:
        plate(p, [(-1.5, -0.08), (1.5, -0.08), (1.5, 0.08), (-1.5, 0.08)], rail, -0.001, 0.006,
              "Warn")
    # The cutting edge along the bottom at the back of the opening.
    p.prism([(-1.85, 0.32), (-1.79, 0.30), (-1.90, 0.03), (-1.97, 0.05)], -1.5, 1.5, "Steel",
            axis="x")
    if lod < 2:
        # A dark mounting strip and its bolts above the blade.
        p.prism([(-1.82, 0.36), (-1.74, 0.33), (-1.79, 0.2), (-1.87, 0.22)], -1.49, 1.49,
                "Dark", axis="x")
    if full:
        for k in range(11):
            x = -1.35 + 0.27 * k
            p.cyl(0.018, 0.0, 0.025, "Steel", segs=6,
                  xf=T(x, 0.27, -1.835) @ RX(math.atan2(0.13, 0.05)) @ RY(math.pi))
    # The push frame into the hull, and its rams.
    p.box((1.8, 0.6, 0.24), (0.0, 0.85, -1.5), "Dark", bevel=bev, segs=1)
    if lod < 2:
        for sx in (-1, 1):
            p.box((0.14, 0.14, 0.5), (sx * 0.75, 0.6, -1.45), "Steel", bevel=0.01, segs=1)
    if full:
        for sx in (-1, 1):
            # Rams from the frame up to the bonnet.
            rod(p, (sx * 0.45, 1.0, -1.58), (sx * 0.45, 1.25, -1.33), 0.05, "Dark", 10)
            rod(p, (sx * 0.45, 1.2, -1.38), (sx * 0.45, 1.46, -1.12), 0.03, "Steel", 8)
    # The trunk up from the shroud's back to the chute's collar.
    hx, hz = 0.17, 0.15
    y_top = COLLAR_TOP - 0.05
    trunk = []
    nst = 6 if lod < 2 else 3
    for i in range(nst + 1):
        f = i / nst
        y = 1.15 + (y_top - 1.15) * f
        zc_ = -1.86 + (CZ + 1.86) * f
        ring = []
        for dx, dz in ((-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)):
            yy = y
            if i == 0:
                # Seated in the shroud: just under its outer skin.
                d = (zc_ + dz) - AZ
                yy = AY + math.sqrt(max(0.0, ((SHROUD_IN + SHROUD_OUT) / 2) ** 2 - d * d))
            ring.append(Vector((CX + dx, yy, zc_ + dz)))
        trunk.append(ring)
    p.loft(trunk, "Shell")
    p.cyl(0.21, y_top - 0.03, COLLAR_TOP, "Dark", segs=n, xf=T(CX, 0, CZ) @ UP,
          chamfer=0.01 if lod < 2 else 0.0)
    if lod < 2:
        # A hazard stripe on the trunk's downhill side.
        zs = -1.86 + (CZ + 1.86) * (1.62 - 1.15) / (y_top - 1.15)
        p.box((0.006, 0.07, 0.24), (CX + hx + 0.0025, 1.62, zs), "Warn", segs=1)
    return p


def chute(lod):
    n = SEGS[lod]
    full = lod == 0
    p = Part("chute" + suffix(lod), origin=CHUTE)
    # Swivel ring on the collar.
    p.cyl(0.23, COLLAR_TOP - 0.01, COLLAR_TOP + 0.08, "Dark", segs=n, xf=T(CX, 0, CZ) @ UP,
          chamfer=0.012 if lod < 2 else 0.0)
    # The pipe: up, then bending out to +X.
    r = 0.17
    bend_c, bend_r = (CX + 0.35, 2.22), 0.35
    path = [(CX, COLLAR_TOP + 0.02, CZ), (CX, 2.1, CZ), (CX, 2.22, CZ)]
    nb = (8, 5, 3)[lod]
    for i in range(1, nb + 1):
        a = math.pi - (math.pi / 2) * i / nb
        path.append((bend_c[0] + bend_r * math.cos(a), bend_c[1] + bend_r * math.sin(a), CZ))
    end_x = CX + 0.58
    path.append((end_x, 2.57, CZ))
    pipe(p, path, r, "Shell", max(8, n), mouth=(0.14, 0.12) if lod < 2 else None)
    # The deflector hood over the mouth, curling down.
    hc = (end_x, 2.37)
    h_in, h_out = 0.37, 0.4
    hn = (10, 6, 3)[lod]
    a0, a1 = 100.0, -15.0

    def hp(a, rr):
        t = math.radians(a)
        return (hc[0] + rr * math.cos(t), hc[1] + rr * math.sin(t))

    hood = ([hp(a0 + (a1 - a0) * i / hn, h_out) for i in range(hn + 1)]
            + [hp(a1 - (a1 - a0) * i / hn, h_in) for i in range(hn + 1)])
    p.prism(hood, CZ - 0.21, CZ + 0.21, "Dark", axis="z")
    # Cheeks either side of it.
    cheek = [hp(a0 + (a1 - a0) * i / hn, h_in + 0.005) for i in range(hn + 1)] + [hc]
    for s in (-1, 1):
        a, b = sorted((CZ + s * 0.19, CZ + s * 0.216))
        p.prism(cheek, a, b, "Dark", axis="z")
    if lod < 2:
        # A hazard stripe along the hood's lip.
        lip = [hp(a1 + 1.0, h_out - 0.002), hp(a1 + 16.0, h_out - 0.002),
               hp(a1 + 16.0, h_out + 0.006), hp(a1 + 1.0, h_out + 0.006)]
        p.prism(lip, CZ - 0.2, CZ + 0.2, "Warn", axis="z")
    if full:
        # The deflector's tilt ram, from the pipe's elbow up to the hood.
        rod(p, (CX + 0.2, 2.5, CZ - 0.19), (end_x + 0.12, 2.74, CZ - 0.215), 0.016, "Steel", 8)
        # The swivel motor on the ring.
        p.box((0.16, 0.14, 0.12), (CX - 0.25, COLLAR_TOP + 0.05, CZ), "Dark", bevel=0.01, segs=1)
        p.cyl(0.04, -0.05, 0.05, "Steel", segs=10, xf=T(CX - 0.25, COLLAR_TOP + 0.13, CZ) @ UP)
    return p


def beacon(lod):
    n = SEGS[lod]
    full = lod == 0
    p = Part("beacon" + suffix(lod), origin=BEACON)
    xf = T(BEACON[0], 0, BEACON[2]) @ UP
    p.cyl(0.05, 2.6, 2.72, "Dark", segs=max(8, n // 2), xf=xf)
    p.cyl(0.18, 2.7, 2.76, "Dark", segs=max(8, n), xf=xf, chamfer=0.01 if lod < 2 else 0.0)
    p.lathe([(0.0, 2.755), (0.15, 2.755), (0.15, 2.88), (0.12, 2.96), (0.0, 2.985)], "Lamp",
            segs=max(8, n), xf=xf)
    p.cyl(0.04, 2.975, 3.0, "Dark", segs=max(6, n // 2), xf=xf)
    if lod < 2:
        # A wire cage over the lens.
        for k in range(6 if full else 4):
            a = 2 * math.pi * k / (6 if full else 4)
            x, z = 0.165 * math.cos(a), 0.165 * math.sin(a)
            p.box((0.016, 0.2, 0.016), (BEACON[0] + x, 2.85, BEACON[2] + z), "Steel", segs=1)
        p.tube(0.175, 0.155, 2.94, 2.955, "Steel", segs=max(8, n), xf=xf)
    return p


def eye(lod):
    n = SEGS[lod]
    full = lod == 0
    ex, ey, ez = EYE
    p = Part("eye" + suffix(lod), origin=EYE)
    p.box((1.2, 0.14, 0.11), (ex, ey, -0.745), "Dark", bevel=0.015 if lod < 2 else 0.0, segs=1)
    p.box((1.04, 0.05, 0.01), (ex, ey, -0.804), "Eye", segs=1)
    if lod < 2:
        # A shell brow over it and three round lenses on the strip.
        p.box((1.26, 0.03, 0.09), (ex, ey + 0.085, -0.77), "Shell", bevel=0.006, segs=1)
        for x in (-0.42, 0.0, 0.42):
            p.cyl(0.045, 0.79, 0.817, "Dark", segs=max(8, n), xf=T(ex + x, ey, 0) @ RY(math.pi))
            p.cyl(0.032, 0.812, 0.824, "Eye", segs=max(8, n), xf=T(ex + x, ey, 0) @ RY(math.pi))
    if full:
        # Mounting lugs into the cab.
        for x in (-0.55, 0.55):
            p.box((0.05, 0.08, 0.06), (ex + x, ey - 0.06, -0.69), "Steel", segs=1)
    return p


def hatch(lod):
    full = lod == 0
    hx, hy, hz = HATCH
    p = Part("hatch" + suffix(lod), origin=HATCH)
    p.box((0.92, 0.04, 0.78), (hx, hy + 0.02, 1.86), "Shell", bevel=0.01 if lod < 2 else 0.0,
          segs=1)
    if lod < 2:
        # Two handles and the yellow pull ring on a steel eye.
        for x in (-0.33, 0.33):
            p.sweep([(1.66, hy + 0.035), (1.66, hy + 0.09), (1.88, hy + 0.09),
                     (1.88, hy + 0.035)], section_circle(0.014, 6 if full else 4), "Steel",
                    xf=T(x, 0, 0))
        p.box((0.06, 0.03, 0.04), (hx, hy + 0.05, 2.1), "Steel", segs=1)
        ring = [(0.07 + 0.014 * math.cos(2 * math.pi * k / 8), 0.014 * math.sin(2 * math.pi * k / 8))
                for k in range(8)]
        p.lathe(ring, "Yellow", segs=16 if full else 10, closed=True,
                xf=T(hx, hy + 0.065, 2.16) @ UP)
    else:
        p.box((0.16, 0.03, 0.16), (hx, hy + 0.05, 2.14), "Yellow", segs=1)
    if full:
        # Chevrons along its front edge, and its four latches.
        frame = (Vector((hx, hy + 0.04, 1.52)), Vector((1, 0, 0)), Vector((0, 0, -1)),
                 Vector((0, 1, 0)))
        chevrons(p, frame, 0.42, -0.035, 0.035, width=0.035, pitch=0.09)
        for x in (-0.4, 0.4):
            for z in (1.56, 2.16):
                p.box((0.06, 0.02, 0.05), (x, hy + 0.045, z), "Steel", segs=1)
    return p


def cell(lod):
    n = SEGS[lod]
    full = lod == 0
    cx, cy, cz = CELL
    p = Part("cell" + suffix(lod), origin=CELL)
    p.box((0.68, 0.46, 0.54), (cx, 1.25 + 0.23, cz), "Dark", bevel=0.025 if lod < 2 else 0.0,
          segs=2 if full else 1)
    # A glowing charge strip round it.
    p.box((0.7, 0.05, 0.56), (cx, 1.52, cz), "Glow", segs=1)
    if lod < 2:
        # Terminals and a carry bar on top.
        for x in (-0.2, 0.2):
            p.cyl(0.045, 1.7, 1.73, "Steel", segs=max(8, n // 2), xf=T(x, 0, cz - 0.15) @ UP)
            p.cyl(0.03, 1.73, 1.745, "Copper", segs=max(8, n // 2), xf=T(x, 0, cz - 0.15) @ UP)
        p.sweep([(cz + 0.05, 1.7), (cz + 0.05, 1.735), (cz + 0.15, 1.735), (cz + 0.15, 1.7)],
                section_rect(0.36, 0.02), "Steel")
    if full:
        p.box((0.3, 0.12, 0.006), (cx, 1.4, cz - 0.272), "Warn", segs=1)
    return p


def bank(lod):
    """A windrow of ploughed snow, its lumps periodic along Z (4 m)."""
    bx, by, bz = BANK
    p = Part("bank" + suffix(lod), origin=BANK)
    nz, nx = (40, 16)[lod], (26, 12)[lod]
    L = 4.0
    rings = []
    for i in range(nz + 1):
        z = -L / 2 + L * i / nz
        w = 2 * math.pi * (z + L / 2) / L
        half = 1.0 + 0.06 * math.sin(w + 0.7) + 0.03 * math.sin(3 * w + 2.1)
        shift = 0.12 * math.sin(2 * w + 0.4)
        ring = []
        for k in range(nx + 1):
            u = -1.0 + 2.0 * k / nx  # -1 .. 1 across
            prof = math.cos(math.pi / 2 * u) ** 0.75
            lump = (0.1 * math.sin(w + 1.3 * u + 0.5)
                    + 0.07 * math.sin(3 * w - 2.0 * u + 1.9)
                    + 0.04 * math.sin(5 * w + 4.0 * u + 0.3)
                    + 0.03 * math.sin(7 * w - 6.0 * u + 2.6))
            h = 1.05 * prof * (1.0 + lump) if abs(u) < 1.0 else 0.0
            x = bx + u * half + shift * prof
            ring.append(Vector((x, by + max(0.0, h), bz + z)))
        rings.append(ring)
    p.loft(rings, "Shell")
    return p


def build(lod):
    out = [hull(lod), track(-1, lod), track(1, lod), auger(lod), housing(lod), chute(lod),
           beacon(lod), eye(lod), hatch(lod), cell(lod)]
    if lod < 2:
        out.append(bank(lod))
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        if isinstance(p, Part):
            make(p)
report()
export("assets/models/robots/plough.glb")
