"""A roe deer, in parts the game moves (procedural gait, two-bone legs):
writes assets/models/animals/deer.glb. Its October coat: grey-brown, a
white rump, a black nose.

Deer space, as in src/wildlife/deer_look.rs: metres, +Y up, -Z forward,
the origin at the middle of the body. A roe deer stands 0.72 m at the
shoulder and is 1.1 m long. Each part is its own object, origin where it
turns, laid out at its own place along X in the file (the game uses each
mesh about its own origin; laid out apart the checker never takes two
parts for z-fighting):

- `body`        torso, rump patch and tail; origin the body's middle
- `head`        neck and head, origin the root of the neck on the body at
                NECK = (0, 0.06, -0.33); grazing pitches it down about there
- `upper_f`     front upper leg: from its origin (the shoulder) down -Y for
                UPPER_F; the game lays it from shoulder to knee
- `lower_f`     front lower leg and hoof, from the knee down -Y for LOWER_F
- `upper_h`, `lower_h`   the hind leg likewise (UPPER_H, LOWER_H)
- `far`         the whole deer standing, a handful of quads, past 60 m

Materials: Hide, Pale (the rump and the belly), Hoof (hooves, nose, eyes).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, T, export, main_guard, make, report

main_guard()

SPACING = 1.6
NECK = (0.0, 0.06, -0.33)
UPPER_F, LOWER_F = 0.25, 0.3
UPPER_H, LOWER_H = 0.3, 0.33


def ring(z, rx, ry, cy, ox, n=8, dy=0.0):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + dy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


# Torso rings, rump to chest: (z, half width, half height, centre height).
TORSO = [
    (0.42, 0.04, 0.05, 0.03),
    (0.38, 0.1, 0.12, 0.02),
    (0.28, 0.12, 0.155, 0.0),
    (0.1, 0.12, 0.16, -0.01),
    (-0.1, 0.115, 0.165, 0.0),
    (-0.26, 0.11, 0.165, 0.02),
    (-0.36, 0.085, 0.13, 0.05),
    (-0.4, 0.05, 0.08, 0.07),
]


def body(k, lod=False):
    ox = k * SPACING
    p = Part("body", origin=(ox, 0, 0))
    n = 5 if lod else 10
    rings = TORSO[::2] + [TORSO[-1]] if lod else TORSO
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in rings], "Hide")
    if not lod:
        # The white rump patch, a shield proud of the haunches.
        p.loft([ring(z, rx, ry, cy, ox, 8) for z, rx, ry, cy in
                [(0.445, 0.02, 0.025, 0.035), (0.425, 0.055, 0.07, 0.03),
                 (0.405, 0.07, 0.085, 0.025)]], "Pale")
        # Pale belly, a little below the torso's underside.
        p.loft([ring(z, rx, ry, cy, ox, 6) for z, rx, ry, cy in
                [(0.2, 0.06, 0.03, -0.14), (0.0, 0.075, 0.035, -0.15), (-0.2, 0.06, 0.03, -0.13)]],
               "Pale")
    return p


def head(k, lod=False):
    """Neck up and forward from NECK, the head at its top, looking -Z."""
    ox = k * SPACING
    nx, ny, nz = NECK
    p = Part("head", origin=(ox + nx, ny, nz))
    n = 5 if lod else 8
    # The neck, leaning forward 35 degrees from upright.
    lean = math.radians(35)
    neck = []
    for i, (t, rx, ry) in enumerate([(0.0, 0.07, 0.085), (0.12, 0.05, 0.06), (0.26, 0.04, 0.05)]):
        y = ny + t * math.cos(lean)
        z = nz - t * math.sin(lean)
        neck.append([(ox + rx * math.cos(2 * math.pi * j / n), y, z + ry * math.sin(2 * math.pi * j / n))
                     for j in range(n)])
    p.loft(neck, "Hide")
    # The head: a wedge-shaped loft along -Z from the poll to the muzzle.
    hy = ny + 0.26 * math.cos(lean)
    hz = nz - 0.26 * math.sin(lean)
    skull = [(0.02, 0.045, 0.05, 0.0), (-0.04, 0.05, 0.055, -0.005), (-0.1, 0.035, 0.04, -0.02),
             (-0.15, 0.022, 0.028, -0.032)]
    p.loft([ring(hz + z, rx, ry, hy + cy, ox, n) for z, rx, ry, cy in skull], "Hide")
    if not lod:
        # Black nose; eyes; big ears up and out.
        p.sphere(0.02, (ox, hy - 0.035, hz - 0.155), "Hoof", segs=6, rings=4)
        for sx in (-1, 1):
            p.sphere(0.012, (ox + sx * 0.045, hy + 0.01, hz - 0.04), "Hoof", segs=6, rings=4)
            ear = [(0.0, 0.0), (0.03, 0.02), (0.035, 0.08), (0.0, 0.11), (-0.025, 0.07)]
            # A flat leaf in the ear's own plane, tipped out to the side.
            pts = [(x, y) for x, y in ear]
            from common import RZ
            p.prism(pts, -0.005, 0.005, "Hide", axis="z",
                    xf=T(ox + sx * 0.03, hy + 0.04, hz + 0.01) @ RZ(-sx * 0.6) @ RX(0.2))
    return p


def leg(name, k, length, r0, r1, hoof=False, lod=False):
    """A leg segment from its origin down -Y, tapering r0 -> r1."""
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    n = 4 if lod else 6
    rings = [(0.0, r0), (length * 0.5, (r0 + r1) * 0.55), (length, r1)]
    pts = [[(ox + r * math.cos(2 * math.pi * j / n), -t, r * 0.85 * math.sin(2 * math.pi * j / n))
            for j in range(n)] for t, r in rings]
    p.loft(pts, "Hide")
    if hoof:
        p.loft([[(ox + r * math.cos(2 * math.pi * j / n), -length + y, z0 + r * math.sin(2 * math.pi * j / n))
                 for j in range(n)] for y, r, z0 in [(0.035, r1 * 1.05, 0.0), (0.0, r1 * 1.25, -0.008)]],
               "Hoof")
    return p


def far(k):
    ox = k * SPACING
    p = Part("far", origin=(ox, 0, 0))
    n = 4
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in TORSO[1::2]], "Hide")
    # Neck and head as two boxes.
    nx, ny, nz = NECK
    p.box((0.08, 0.26, 0.09), (ox, ny + 0.1, nz - 0.06), "Hide",
          xf=RX(math.radians(35), about=(ox, ny, nz)))
    p.box((0.07, 0.07, 0.17), (ox, ny + 0.24, nz - 0.22), "Hide")
    for sx, z, top in ((-1, -0.25, UPPER_F + LOWER_F), (1, -0.25, UPPER_F + LOWER_F),
                       (-1, 0.27, UPPER_H + LOWER_H), (1, 0.27, UPPER_H + LOWER_H)):
        p.box((0.04, top + 0.02, 0.045), (ox + sx * 0.075, -0.03 - top / 2 + 0.01, z), "Hide")
    return p


parts = [
    body(0),
    head(1),
    leg("upper_f", 2, UPPER_F, 0.05, 0.028),
    leg("lower_f", 3, LOWER_F, 0.024, 0.015, hoof=True),
    leg("upper_h", 4, UPPER_H, 0.075, 0.035),
    leg("lower_h", 5, LOWER_H, 0.026, 0.015, hoof=True),
    far(6),
]
for part in parts:
    make(part)

report()
export("assets/models/animals/deer.glb")
