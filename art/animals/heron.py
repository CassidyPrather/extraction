"""A grey heron, for Sallow Reservoir's shallows: writes
assets/models/animals/heron.glb, in the rook's poses and node names
(art/animals/rook.py) so src/wildlife/visuals.rs swaps them the same way.

Bird space: metres, +Y up, -Z forward (the bill), the origin at its feet.
A heron stands 0.9 m, its wings 1.7 m across. Standing, its neck is a long
S and its bill a dagger; flying, the neck is drawn back into the
shoulders and the legs trail behind. Each pose at its own place along X:

- `sit`, `sit_lod1`          standing in the water, neck up
- `up`, `mid`, `down`        the slow, deep wingbeat
- `glide`                    wings out, a little bowed
- `fly_lod1`                 every flying pose from far off

Materials: Feather (grey back and wings), Pale (white neck and face),
Beak (the yellow bill and legs).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, RZ, T, export, main_guard, make, report

main_guard()

SPACING = 2.2


def ring(z, rx, ry, cy, ox, n=8):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


# Body, tail end to breast: (z, half width, half height, centre height above
# the body's middle).
BODY = [(0.24, 0.02, 0.02, 0.02), (0.16, 0.07, 0.06, 0.0), (0.0, 0.09, 0.085, 0.0),
        (-0.12, 0.08, 0.08, 0.01), (-0.2, 0.05, 0.05, 0.03)]


def body(p, ox, y, pitch, n=8):
    xf = T(ox, y, 0) @ RX(pitch) @ T(-ox, 0, 0)
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in BODY], "Feather", xf=xf)
    return xf


def neck_and_head(p, ox, xf, points, n=6):
    """A neck through `points` (y, z in body space), then the head and bill
    at its end, looking forward."""
    rings = []
    for i, (y, z) in enumerate(points):
        r = 0.035 - 0.012 * i / max(1, len(points) - 1)
        rings.append(ring(z, r, r, y, ox, n))
    p.loft(rings, "Pale", xf=xf)
    hy, hz = points[-1]
    p.sphere(0.03, (ox, hy, hz - 0.01), "Pale", segs=n, rings=4, scale=(0.9, 0.9, 1.3), xf=xf)
    # The dagger bill.
    p.lathe([(0.0, 0.0), (0.012, -0.005), (0.008, -0.08), (0.0, -0.13)], "Beak", segs=5,
            xf=xf @ T(ox, hy - 0.005, hz - 0.035))
    # The black crest, a streak back off the crown.
    p.box((0.01, 0.006, 0.09), (ox, hy + 0.022, hz + 0.05), "Feather", xf=xf @ RX(-0.25, about=(ox, hy, hz)))


def legs(p, ox, standing):
    for side in (-1, 1):
        x = ox + side * 0.035
        if standing:
            p.cyl(0.008, 0.0, 0.5, "Beak", segs=5, xf=T(x, 0.0, 0.0) @ RX(-math.pi / 2))
            p.box((0.012, 0.006, 0.09), (x, 0.003, -0.02), "Beak")
        else:
            # Trailing out behind, past the tail.
            p.cyl(0.007, 0.15, 0.62, "Beak", segs=5, xf=T(x, 0.28, 0.0))


WING = [(0.0, -0.11), (0.3, -0.13), (0.62, -0.09), (0.8, -0.04), (0.84, 0.03), (0.7, 0.09),
        (0.45, 0.13), (0.2, 0.15), (0.0, 0.13)]


def wing(p, ox, side, lift, bow, y, lod=False):
    pts = [WING[0], WING[3], WING[5], WING[-1]] if lod else WING
    plan = [(ox + side * (0.07 + x), z) for x, z in pts]
    xf = RZ(side * lift, about=(ox + side * 0.07, y, 0)) @ RY(-side * bow, about=(ox, y, 0))
    p.prism(plan, y - 0.007, y + 0.007, "Feather", axis="y", xf=xf)


def stand(name, k, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    xf = body(p, ox, 0.6, math.radians(25), 5 if lod else 8)
    # Folded wings over the back.
    for side in (-1, 1):
        fold = [(0.02, -0.15), (0.075, -0.15), (0.075, 0.25), (0.0, 0.28)]
        p.prism([(ox + side * x, z) for x, z in fold], 0.06, 0.08, "Feather", axis="y", xf=xf)
    neck_and_head(p, ox, xf, [(0.04, -0.18), (0.14, -0.22), (0.22, -0.18), (0.3, -0.22)],
                  4 if lod else 6)
    legs(p, ox, True)
    return make(p)


def fly(name, k, lift, bow=0.0, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    xf = body(p, ox, 0.3, 0.0, 5 if lod else 8)
    # The neck drawn back: the head tucked into the shoulders.
    neck_and_head(p, ox, xf, [(0.02, -0.18), (0.06, -0.22), (0.05, -0.28)], 4 if lod else 6)
    legs(p, ox, False)
    tail = [(-0.02, 0.2), (0.02, 0.2), (0.05, 0.32), (-0.05, 0.32)]
    p.prism([(ox + x, z) for x, z in tail], 0.295, 0.305, "Feather", axis="y")
    for side in (-1, 1):
        wing(p, ox, side, lift, bow, 0.31, lod)
    return make(p)


stand("sit", 0)
stand("sit_lod1", 1, lod=True)
fly("up", 2, math.radians(45))
fly("mid", 3, math.radians(5), math.radians(4))
fly("down", 4, math.radians(-40))
fly("glide", 5, math.radians(-8), math.radians(6))
fly("fly_lod1", 6, math.radians(-8), math.radians(6), lod=True)

report()
export("assets/models/animals/heron.glb")
