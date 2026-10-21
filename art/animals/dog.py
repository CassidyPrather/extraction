"""A stray dog (Cinder Works' pack), in the same parts as the deer
(art/animals/deer.py) for the same procedural gait: writes
assets/models/animals/dog.glb. A rangy mongrel, 0.5 m at the shoulder,
dark and dusty, a paler chest.

Dog space as in src/wildlife/herd_look.rs (`build(Kind::Dog)`): metres,
+Y up, -Z forward, the origin at the middle of the body.

- `body`        torso and tail; origin the body's middle
- `head`        neck and head, origin the root of the neck at
                NECK = (0, 0.05, -0.28)
- `upper_f`, `lower_f`   front leg, from the shoulder and the knee down -Y
                         (UPPER_F, LOWER_F)
- `upper_h`, `lower_h`   hind leg (UPPER_H, LOWER_H)
- `far`         the whole dog standing, past 90 m

Materials: Hide, Pale (chest, muzzle), Hoof (pads, nose, eyes).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RZ, T, export, main_guard, make, report

main_guard()

SPACING = 1.4
NECK = (0.0, 0.05, -0.28)
UPPER_F, LOWER_F = 0.2, 0.22
UPPER_H, LOWER_H = 0.22, 0.24


def ring(z, rx, ry, cy, ox, n=8):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


# Torso, rump to chest: deep in the chest, tucked up at the loins.
TORSO = [
    (0.32, 0.05, 0.05, 0.03),
    (0.28, 0.09, 0.09, 0.02),
    (0.16, 0.085, 0.08, 0.02),
    (0.0, 0.09, 0.1, 0.0),
    (-0.16, 0.1, 0.13, -0.01),
    (-0.27, 0.085, 0.11, 0.02),
    (-0.32, 0.05, 0.07, 0.04),
]


def body(k, lod=False):
    ox = k * SPACING
    p = Part("body", origin=(ox, 0, 0))
    n = 5 if lod else 9
    p.loft([ring(z, rx, ry, cy, ox, n) for z, rx, ry, cy in (TORSO[::2] if lod else TORSO)], "Hide")
    # The tail: down and out, a gentle curve, thin at the tip.
    pts = []
    for i, (t, r) in enumerate([(0.0, 0.025), (0.12, 0.02), (0.24, 0.012), (0.32, 0.005)]):
        a = 0.5 + i * 0.25
        pts.append(ring(0.3 + t * math.cos(a) * 0.8, r, r, 0.04 - t * math.sin(a) * 0.9, ox, 4 if lod else 5))
    p.loft(pts, "Hide")
    if not lod:
        # A paler chest under the brisket.
        p.loft([ring(z, rx, ry, cy, ox, 6) for z, rx, ry, cy in
                [(-0.08, 0.05, 0.03, -0.11), (-0.2, 0.065, 0.04, -0.12), (-0.3, 0.04, 0.03, -0.06)]],
               "Pale")
    return p


def head(k, lod=False):
    ox = k * SPACING
    nx, ny, nz = NECK
    p = Part("head", origin=(ox + nx, ny, nz))
    n = 5 if lod else 8
    lean = math.radians(50)
    neck = []
    for t, rx, ry in [(0.0, 0.065, 0.075), (0.1, 0.05, 0.055), (0.18, 0.045, 0.05)]:
        y = ny + t * math.cos(lean)
        z = nz - t * math.sin(lean)
        neck.append([(ox + rx * math.cos(2 * math.pi * j / n), y, z + ry * math.sin(2 * math.pi * j / n))
                     for j in range(n)])
    p.loft(neck, "Hide")
    hy = ny + 0.18 * math.cos(lean)
    hz = nz - 0.18 * math.sin(lean)
    # Skull to muzzle: a broad head, a long narrow snout.
    skull = [(0.03, 0.045, 0.045, 0.01), (-0.03, 0.055, 0.05, 0.0), (-0.07, 0.035, 0.03, -0.02),
             (-0.15, 0.02, 0.02, -0.03)]
    p.loft([ring(hz + z, rx, ry, hy + cy, ox, n) for z, rx, ry, cy in skull], "Hide")
    if not lod:
        p.sphere(0.014, (ox, hy - 0.025, hz - 0.15), "Hoof", segs=6, rings=4)
        for sx in (-1, 1):
            p.sphere(0.009, (ox + sx * 0.035, hy + 0.015, hz - 0.045), "Hoof", segs=6, rings=4)
            # Pricked ears, a bit ragged.
            ear = [(-0.025, 0.0), (0.025, 0.0), (0.004, 0.075)]
            p.prism(ear, -0.004, 0.004, "Hide", axis="z",
                    xf=T(ox + sx * 0.03, hy + 0.04, hz + 0.0) @ RZ(-sx * 0.35) @ RX(-0.15))
    return p


def leg(name, k, length, r0, r1, lod=False):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    n = 4 if lod else 6
    rings = [(0.0, r0), (length * 0.5, (r0 + r1) * 0.55), (length, r1)]
    p.loft([[(ox + r * math.cos(2 * math.pi * j / n), -t, r * 0.9 * math.sin(2 * math.pi * j / n))
             for j in range(n)] for t, r in rings], "Hide")
    return p


def paw(name, k, length, r0, r1):
    p = leg(name, k, length, r0, r1)
    ox = k * SPACING
    # The paw: a pad forward of the foot.
    p.sphere(r1 * 1.6, (ox, -length + r1 * 0.6, -0.012), "Hoof", segs=6, rings=4,
             scale=(1.0, 0.6, 1.4))
    return p


def far(k):
    ox = k * SPACING
    p = Part("far", origin=(ox, 0, 0))
    p.loft([ring(z, rx, ry, cy, ox, 4) for z, rx, ry, cy in TORSO[::2]], "Hide")
    nx, ny, nz = NECK
    p.box((0.07, 0.18, 0.08), (ox, ny + 0.07, nz - 0.07), "Hide",
          xf=RX(math.radians(50), about=(ox, ny, nz)))
    p.box((0.08, 0.07, 0.2), (ox, ny + 0.13, nz - 0.2), "Hide")
    for sx, z, top in ((-1, -0.22, UPPER_F + LOWER_F), (1, -0.22, UPPER_F + LOWER_F),
                       (-1, 0.22, UPPER_H + LOWER_H), (1, 0.22, UPPER_H + LOWER_H)):
        p.box((0.035, top + 0.03, 0.04), (ox + sx * 0.07, -0.04 - top / 2 + 0.02, z), "Hide")
    p.box((0.025, 0.025, 0.25), (ox, 0.0, 0.42), "Hide", xf=RX(0.6, about=(ox, 0.03, 0.3)))
    return p


for part in [
    body(0),
    head(1),
    leg("upper_f", 2, UPPER_F, 0.04, 0.025),
    paw("lower_f", 3, LOWER_F, 0.022, 0.016),
    leg("upper_h", 4, UPPER_H, 0.06, 0.03),
    paw("lower_h", 5, LOWER_H, 0.024, 0.016),
    far(6),
]:
    make(part)

report()
export("assets/models/animals/dog.glb")
