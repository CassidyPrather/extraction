"""Throwable lights for the dark (`world::glow`): a chemlight and a hand
flare. Writes assets/models/items/{chemlight,hand_flare}.glb (item space:
see _kit.py).

Each lies along Z in its item box:
- `chemlight` 0.028 x 0.028 x 0.15: a clear polymer tube, the green fluid
  inside, a black end cap with a hanging loop. The game lights it (a green
  glow along the tube) when it's snapped.
- `hand_flare` 0.03 x 0.03 x 0.23: a red paper tube, a black plastic cap
  over the striking end (-Z), a wooden plug at the other. The game lights
  its end red when it's struck.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import item, ring, wanted  # noqa: E402
from common import T, Part  # noqa: E402


def chemlight():
    L = 0.15
    z0, z1 = -L / 2, L / 2
    p = Part("body")
    # The tube, clear, with the fluid's column inside it.
    p.cyl(0.0105, z0 + 0.012, z1 - 0.004, "Clear", segs=16, chamfer=0.002)
    p.cyl(0.0085, z0 + 0.016, z1 - 0.008, "Leaf", segs=12)
    # The cap at -Z, and its hanging loop.
    p.cyl(0.014, z0 + 0.004, z0 + 0.016, "Black", segs=16, chamfer=0.0015)
    ring(p, 0.006, 0.0014, "Black", xf=T(0, 0, z0 + 0.004), segs=12, sides=4)
    return [p]


def hand_flare():
    L = 0.23
    z0, z1 = -L / 2, L / 2
    p = Part("body")
    # The paper tube.
    p.cyl(0.0135, z0 + 0.045, z1 - 0.006, "Red", segs=18, chamfer=0.001)
    # The black cap over the striking end, its striker face rough.
    p.cyl(0.015, z0, z0 + 0.05, "Black", segs=18, chamfer=0.002)
    p.cyl(0.0125, z0 - 0.0002, z0 + 0.002, "Char", segs=14)
    # The wooden plug at the far end.
    p.cyl(0.011, z1 - 0.006, z1, "Wood", segs=14, chamfer=0.0015)
    return [p]


if __name__ == "__main__":
    if wanted("chemlight"):
        item("chemlight", chemlight)
    if wanted("hand_flare"):
        item("hand_flare", hand_flare)
