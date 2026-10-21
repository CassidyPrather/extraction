"""A small fish of the creeks (a trout, to look at from the bank) and a
moth for the lamps at night: writes assets/models/animals/fish.glb.

Fish space as in src/wildlife/fish.rs: metres, +Y up, -Z forward (the
snout), the origin at the middle of the body. A trout of 0.26 m; the game
scales it. Each object at its own place along X:

- `fish`        body, fins and tail, dark back, paler belly
- `moth_up`, `moth_down`   a moth's wings raised and lowered (the game
                           swaps them), 4 cm across

Materials: Hide (the fish's back), Pale (its belly, a moth).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RZ, export, main_guard, make, report

main_guard()

SPACING = 0.5


def ring(z, rx, ry, cy, ox, n=6):
    return [(ox + rx * math.cos(2 * math.pi * k / n), cy + ry * math.sin(2 * math.pi * k / n), z)
            for k in range(n)]


def fish(k):
    ox = k * SPACING
    p = Part("fish", origin=(ox, 0, 0))
    body = [(0.12, 0.004, 0.006, 0.0), (0.08, 0.012, 0.018, 0.0), (0.0, 0.02, 0.03, 0.0),
            (-0.08, 0.018, 0.026, 0.0), (-0.12, 0.01, 0.014, -0.002), (-0.13, 0.003, 0.004, -0.003)]
    p.loft([ring(z, rx, ry, cy, ox) for z, rx, ry, cy in body], "Hide")
    # Belly, a little paler, under the back.
    p.loft([ring(z, rx, ry, cy, ox, 5) for z, rx, ry, cy in
            [(0.06, 0.008, 0.008, -0.014), (0.0, 0.014, 0.01, -0.02), (-0.07, 0.01, 0.008, -0.016)]],
           "Pale")
    # The tail fin, flat and upright, forked; a dorsal fin.
    tail = [(0.11, 0.0), (0.155, 0.03), (0.145, 0.004), (0.145, -0.004), (0.155, -0.028)]
    p.prism([(z, y) for z, y in tail], ox - 0.002, ox + 0.002, "Hide", axis="x")
    dorsal = [(-0.02, 0.022), (0.03, 0.022), (0.0, 0.045)]
    p.prism(dorsal, ox - 0.0015, ox + 0.0015, "Hide", axis="x")
    return p


def moth(name, k, lift):
    ox = k * SPACING
    p = Part(name, origin=(ox, 0, 0))
    p.loft([ring(z, r, r, 0.0, ox, 4) for z, r in [(0.008, 0.002), (0.0, 0.004), (-0.01, 0.003)]],
           "Pale")
    for side in (-1, 1):
        wing = [(0.0, -0.006), (0.019, -0.004), (0.021, 0.008), (0.008, 0.012), (0.0, 0.008)]
        plan = [(ox + side * (0.002 + x), z) for x, z in wing]
        p.prism(plan, -0.0005, 0.0005, "Pale", axis="y", xf=RZ(side * lift, about=(ox, 0, 0)))
    return p


for part in [fish(0), moth("moth_up", 1, math.radians(55)), moth("moth_down", 2, math.radians(-25))]:
    make(part)

report()
export("assets/models/animals/fish.glb")
