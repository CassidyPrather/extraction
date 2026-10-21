"""A rook (the valleys' crows, ravens and jackdaws are this bird, scaled and
tinted by the game): low-poly, in the poses the game swaps between instead
of skinning it. Writes assets/models/animals/rook.glb.

Bird space, as in src/wildlife/visuals.rs: metres, +Y up, -Z forward
(beak), the origin at its feet. A rook is 0.45 m beak to tail, 0.9 m
across the wings. One object a pose, each laid out at its own place along
X in the file so the checker never takes two poses for z-fighting (the
game uses each mesh about its own origin):

- `sit`, `sit_lod1`          wings folded, standing, legs down
- `up`, `mid`, `down`        the wingbeat, from the top of the stroke down
- `glide`                    wings out flat, a little swept
- `fly_lod1`                 every flying pose from far off: a few quads

Materials: Feather (near black, a sheen), Beak (the rook's bare grey face
and bill; the game tints a jackdaw's or raven's darker).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, RZ, T, export, main_guard, make, report

main_guard()

SPACING = 1.2  # metres between poses along X in the file

# The body: elliptical rings along Z, tail end to neck, (z, half width,
# half height, centre height).
BODY = [
    (0.10, 0.018, 0.016, 0.135),
    (0.07, 0.038, 0.034, 0.13),
    (0.03, 0.052, 0.05, 0.128),
    (-0.01, 0.056, 0.056, 0.13),
    (-0.05, 0.05, 0.05, 0.138),
    (-0.08, 0.036, 0.038, 0.154),
    (-0.1, 0.03, 0.031, 0.166),
]
HEAD = (0.0, 0.176, -0.118)
HEAD_R = 0.033
SHOULDER = (0.035, 0.155, -0.03)


def ring(z, rx, ry, cy, ox, n=8):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


def body(p, ox, n=8, pitch=0.0):
    """Body, head and bill; `pitch` tips it nose-up (radians) about the feet."""
    xf = T(ox, 0, 0) @ RX(pitch) @ T(-ox, 0, 0)
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in BODY], "Feather", xf=xf)
    hx, hy, hz = HEAD
    p.sphere(HEAD_R, (ox + hx, hy, hz), "Feather", segs=n, rings=max(4, n // 2 + 1),
             scale=(0.92, 0.9, 1.3), xf=xf)
    # The bill: a cone out of the face, the bare grey skin round its root.
    bill = [(0.0, 0.0), (0.017, -0.004), (0.013, -0.03), (0.0, -0.062)]
    p.lathe(bill, "Beak", segs=max(4, n // 2 + 2),
            xf=xf @ T(ox + hx, hy - 0.006, hz - HEAD_R * 0.9) @ RX(math.radians(8)))


def tail(p, ox, spread, droop, pitch=0.0):
    """The tail: a wedge back from the rump, fanned by `spread` (0..1)."""
    half = 0.022 + 0.035 * spread
    out = [(-0.016, 0.0), (0.016, 0.0), (half, 0.135), (half * 0.35, 0.15),
           (-half * 0.35, 0.15), (-half, 0.135)]
    plan = [(ox + x, 0.075 + z) for x, z in out]
    xf = T(ox, 0, 0) @ RX(pitch) @ T(-ox, 0, 0) @ RX(droop, about=(ox, 0.135, 0.075))
    p.prism(plan, 0.128, 0.138, "Feather", axis="y", xf=xf)


# One wing's outline from above (x out from the body, z back), the right
# wing; fingered primaries at the tip, the secondaries' ragged trailing
# edge.
WING = [
    (0.0, -0.05), (0.14, -0.068), (0.26, -0.05), (0.42, -0.03), (0.33, -0.012),
    (0.425, 0.008), (0.325, 0.022), (0.41, 0.042), (0.31, 0.052), (0.385, 0.075),
    (0.27, 0.085), (0.2, 0.1), (0.15, 0.09), (0.09, 0.105), (0.0, 0.09),
]


def wing(p, ox, side, lift, sweep=0.0, span=1.0, lod=False):
    """A wing raised `lift` radians above level at the shoulder, swept back
    by `sweep`, its span scaled by `span` (a downstroke's folding tip)."""
    pts = [(0.0, -0.05), (0.42, -0.03), (0.385, 0.075), (0.0, 0.09)] if lod else WING
    sx, sy, sz = SHOULDER
    plan = [(ox + side * (sx - 0.012 + x * span), sz + z) for x, z in pts]
    a = side * lift
    xf = RZ(a, about=(ox + side * sx, sy, sz)) @ RY(
        -side * sweep, about=(ox + side * sx, sy, sz))
    p.prism(plan, sy - 0.006, sy + 0.006, "Feather", axis="y", xf=xf)


def folded_wings(p, ox, pitch):
    """Wings closed along the flanks, tips crossed over the tail."""
    xf = T(ox, 0, 0) @ RX(pitch) @ T(-ox, 0, 0)
    side_view = [(-0.055, 0.165), (0.02, 0.174), (0.1, 0.156), (0.17, 0.142), (0.13, 0.13),
                 (0.04, 0.1), (-0.035, 0.105)]
    for side in (-1, 1):
        x0 = ox + side * 0.05
        p.prism(side_view, x0 - 0.007, x0 + 0.007, "Feather", axis="x",
                xf=xf @ RZ(side * 0.12, about=(x0, 0.14, 0.0)))


def legs(p, ox, pitch):
    for side in (-1, 1):
        x = ox + side * 0.022
        # From the feet (the origin) up into the belly.
        p.cyl(0.0055, 0.0, 0.098, "Feather", segs=5, xf=T(x, 0.0, 0.0) @ RX(-math.pi / 2))
        # Toes forward along the perch.
        p.box((0.008, 0.006, 0.042), (x, 0.003, -0.012), "Feather")


def sit(name, k, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0.0, 0.0))
    pitch = math.radians(14)
    if lod:
        p.loft([ring(z, rx, ry, cy, ox, 4) for z, rx, ry, cy in BODY[::2]], "Feather",
               xf=T(ox, 0, 0) @ RX(pitch) @ T(-ox, 0, 0))
        p.box((0.05, 0.05, 0.07), (ox, 0.17, -0.105), "Feather",
              xf=T(ox, 0, 0) @ RX(pitch) @ T(-ox, 0, 0))
        tail(p, ox, 0.0, 0.35, pitch)
        p.box((0.03, 0.12, 0.02), (ox, 0.06, 0.0), "Feather")
        return make(p)
    body(p, ox, pitch=pitch)
    folded_wings(p, ox, pitch)
    tail(p, ox, 0.0, 0.35, pitch)
    legs(p, ox, pitch)
    return make(p)


def fly(name, k, lift, sweep=0.0, span=1.0, spread=0.6, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0.0, 0.0))
    if lod:
        p.loft([ring(z, rx, ry, cy, ox, 4) for z, rx, ry, cy in BODY[::2]], "Feather")
        p.box((0.05, 0.05, 0.07), (ox, 0.17, -0.105), "Feather")
    else:
        body(p, ox)
    tail(p, ox, spread, 0.05)
    for side in (-1, 1):
        wing(p, ox, side, lift, sweep, span, lod)
    return make(p)


sit("sit", 0)
sit("sit_lod1", 1, lod=True)
fly("up", 2, math.radians(62), sweep=math.radians(8), span=0.92)
fly("mid", 3, math.radians(8))
fly("down", 4, math.radians(-48), sweep=math.radians(-4), span=0.88)
fly("glide", 5, math.radians(6), sweep=math.radians(10), spread=1.0)
fly("fly_lod1", 6, math.radians(6), sweep=math.radians(10), spread=1.0, lod=True)

report()
export("assets/models/animals/rook.glb")
