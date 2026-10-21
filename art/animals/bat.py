"""A bat, for dusk and night over the water and round the buildings
(src/wildlife/bats.rs): low-poly, in the poses the game swaps between
instead of skinning it. Writes assets/models/animals/bat.glb.

Bat space, as in src/wildlife/visuals.rs: metres, +Y up, -Z forward (the
nose), the origin in the middle of its body (it's never anywhere but in
the air). A noctule's size: 0.09 m nose to tail, 0.36 m across the wings.
One object a pose, each laid out at its own place along X in the file:

- `up`, `down`   the wingbeat, top of the stroke and bottom (the game
                 swaps them about ten times a second)
- `far`          every pose from far off: a body and two flat wings

Materials: Hide (the fur), Feather (the near-black wing membrane, ears and
face; the game paints both by name).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, RZ, T, export, main_guard, make, report

main_guard()

SPACING = 0.6  # metres between poses along X in the file

# The body: elliptical rings along Z, tail end to the neck, (z, half width,
# half height, centre height).
BODY = [
    (0.045, 0.006, 0.006, 0.0),
    (0.03, 0.014, 0.012, 0.0),
    (0.008, 0.02, 0.017, 0.0),
    (-0.014, 0.019, 0.017, 0.001),
    (-0.028, 0.013, 0.012, 0.002),
]
HEAD = (0.0, 0.003, -0.036)
HEAD_R = 0.013
# Where a wing hinges: at the shoulder, on the body's flank.
SHOULDER = (0.012, 0.004, -0.012)

# One wing's outline from above (x out from the body, z back), the right
# wing: the leading edge out along the arm to the wrist and the long third
# finger's tip, then the membrane scalloped between the fingers' tips back
# to the ankle.
WING = [
    (0.0, -0.012), (0.05, -0.022), (0.085, -0.02), (0.13, -0.008), (0.175, 0.006),
    (0.138, 0.018), (0.142, 0.042), (0.108, 0.04), (0.1, 0.064), (0.07, 0.054),
    (0.045, 0.062), (0.022, 0.05), (0.0, 0.046),
]
WING_FAR = [(0.0, -0.012), (0.085, -0.02), (0.175, 0.006), (0.1, 0.062), (0.0, 0.046)]


def ring(z, rx, ry, cy, ox, n=6):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


def body(p, ox, n=6):
    """Body, head, snout and ears."""
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in BODY], "Hide")
    hx, hy, hz = HEAD
    p.sphere(HEAD_R, (ox + hx, hy, hz), "Hide", segs=n, rings=4, scale=(1.0, 0.9, 1.1))
    # A short dark snout.
    p.box((0.01, 0.008, 0.012), (ox, hy - 0.002, hz - HEAD_R * 0.95), "Feather")
    # Ears: two thin triangles up and back off the crown.
    ear = [(-0.005, 0.0), (0.005, 0.0), (0.001, 0.016)]
    for side in (-1, 1):
        x0 = ox + side * 0.007
        p.prism([(hz + 0.004 + a, hy + 0.006 + b) for a, b in ear], x0 - 0.0015, x0 + 0.0015,
                "Feather", axis="x", xf=RZ(side * 0.35, about=(x0, hy + 0.006, hz)))


def tail_membrane(p, ox):
    """The membrane between the legs and round the tail, out behind."""
    plan = [(ox - 0.012, 0.03), (ox + 0.012, 0.03), (ox + 0.02, 0.05), (ox, 0.068),
            (ox - 0.02, 0.05)]
    p.prism(plan, -0.003, 0.0, "Feather", axis="y")


def wing(p, ox, side, lift, sweep=0.0, span=1.0, far=False):
    """A wing raised `lift` radians above level at the shoulder, swept back
    by `sweep`, its span scaled by `span` (the downstroke folds the tip)."""
    pts = WING_FAR if far else WING
    sx, sy, sz = SHOULDER
    plan = [(ox + side * (sx - 0.01 + x * span), sz + z) for x, z in pts]
    if side < 0:
        plan.reverse()
    about = (ox + side * sx, sy, sz)
    xf = RZ(side * lift, about=about) @ RY(-side * sweep, about=about)
    p.prism(plan, sy - 0.0025, sy + 0.0025, "Feather", axis="y", xf=xf)


def pose(name, k, lift, sweep=0.0, span=1.0, far=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0.0, 0.0))
    if far:
        p.loft([ring(z, rx, ry, cy, ox, 4) for z, rx, ry, cy in BODY[::2]], "Hide")
        p.box((0.02, 0.018, 0.02), (ox, 0.002, -0.034), "Hide")
    else:
        body(p, ox)
        tail_membrane(p, ox)
    for side in (-1, 1):
        wing(p, ox, side, lift, sweep, span, far)
    return make(p)


pose("up", 0, math.radians(58), sweep=math.radians(6), span=0.9)
pose("down", 1, math.radians(-42), sweep=math.radians(-6), span=0.8)
pose("far", 2, math.radians(10), far=True)

report()
export("assets/models/animals/bat.glb")
