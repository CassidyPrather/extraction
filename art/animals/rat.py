"""A rat, for the houses: writes assets/models/animals/rat.glb, sitting and
in two frames of a scurry that the game swaps between (src/wildlife/rats.rs).

Rat space: metres, +Y up, -Z forward (the nose), the origin on the floor
under its middle. 0.2 m nose to rump, a tail as long again. Each pose at
its own place along X:

- `sit`             hunched, nose up a little, tail curled round
- `run_a`, `run_b`  stretched out, legs reaching and gathered

Materials: Hide (the fur), Pale (ears, feet, tail).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, export, main_guard, make, report

main_guard()

SPACING = 0.6


def ring(z, rx, ry, cy, ox, n=6):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


def rat(name, k, stretch, legs):
    """`stretch` lengthens and lowers the body (running); `legs` the reach
    of the feet fore and aft (+ out, - gathered)."""
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    s = 1.0 + 0.25 * stretch
    h = 1.0 - 0.3 * stretch
    body = [(0.1, 0.012, 0.012, 0.035), (0.08, 0.035, 0.035, 0.045), (0.02, 0.042, 0.045, 0.05),
            (-0.04, 0.035, 0.04, 0.05), (-0.08, 0.022, 0.024, 0.045), (-0.11, 0.008, 0.008, 0.04)]
    p.loft([ring(z * s, rx, ry * h, cy * h, ox) for z, rx, ry, cy in body], "Hide")
    # Ears and a nose.
    for sx in (-1, 1):
        p.sphere(0.009, (ox + sx * 0.016, 0.07 * h + 0.008, -0.07 * s), "Pale", segs=5, rings=3,
                 scale=(1.0, 1.0, 0.4))
    p.sphere(0.004, (ox, 0.04 * h, -0.112 * s), "Pale", segs=4, rings=3)
    # The tail: out behind, curled round sitting, straight running.
    pts = []
    for i in range(6):
        t = i / 5
        curl = (1.0 - stretch) * 1.4 * t
        z = 0.1 * s + 0.2 * t * math.cos(curl)
        x = 0.2 * t * math.sin(curl) * 0.7
        y = 0.03 * h * (1 - t) + 0.006
        r = 0.006 * (1 - t) + 0.0015
        pts.append(ring(z, r, r, y, ox + x, 4))
    p.loft(pts, "Pale")
    # Feet: four small pads under it, reaching or gathered.
    for sx in (-1, 1):
        for fz, sign in ((-0.05, -1), (0.06, 1)):
            z = fz * s + sign * 0.02 * legs
            p.box((0.012, 0.008, 0.02), (ox + sx * 0.025, 0.004, z), "Pale")
            p.box((0.01, 0.04 * h, 0.012), (ox + sx * 0.025, 0.02 * h + 0.004, z), "Hide",
                  xf=RX(sign * 0.3 * legs, about=(ox, 0.03 * h, z)))
    return p


for part in [rat("sit", 0, 0.0, 0.0), rat("run_a", 1, 1.0, 1.0), rat("run_b", 2, 0.85, -1.0)]:
    make(part)

report()
export("assets/models/animals/rat.glb")
