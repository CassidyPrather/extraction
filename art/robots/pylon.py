"""The relay pylon walkers put up at work sites. Writes
assets/models/robots/pylon.glb.

In robot space (src/robots/pylon.rs): the root sits on the ground.

- `core`    the cabinet, 1.0 x 1.2 x 1.0 centred at (0, 0.6, 0): a white
            shell on a dark plinth, a glowing seam under its lid, vents,
            hazard stripes on the corners, the mast's collar on top
- `sensor`  the head that rides up the mast, centred at (0, 1.5, 0) when
            packed: a drum with three eyes looking out all round, a
            glowing band, a whip antenna

The mast itself is drawn by the game (it grows with the pylon).
Materials: Shell, Dark, Glow, Eye, Warn, Steel.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, fillet, make, report, reset

UP = RX(-math.pi / 2)

reset()

# --- Cabinet --------------------------------------------------------------

core = Part("core", origin=(0.0, 0.6, 0.0))
# Plinth and feet.
core.box((1.06, 0.1, 1.06), (0.0, 0.05, 0.0), "Dark", bevel=0.02, segs=1)
for sx in (-1, 1):
    for sz in (-1, 1):
        core.box((0.16, 0.06, 0.16), (sx * 0.42, 0.03, sz * 0.42), "Steel", bevel=0.01, segs=1)
# The shell, in two halves round the glowing seam.
core.box((1.0, 0.8, 1.0), (0.0, 0.5, 0.0), "Shell", bevel=0.05, segs=3)
core.box((1.02, 0.03, 1.02), (0.0, 0.915, 0.0), "Glow", bevel=0.01, segs=1)
core.box((1.0, 0.27, 1.0), (0.0, 1.065, 0.0), "Shell", bevel=0.05, segs=3)
# Vents down each face.
for a in range(4):
    xf = RY(a * math.pi / 2)
    for k in range(5):
        core.box((0.5, 0.025, 0.02), (0.0, 0.25 + k * 0.08, 0.505), "Dark", xf=xf)
# Hazard stripes down the corners.
for a in range(4):
    xf = RY(a * math.pi / 2 + math.pi / 4)
    for k in range(3):
        core.box((0.09, 0.06, 0.02), (0.0, 0.25 + k * 0.14, 0.675), "Warn", xf=xf)
# The mast collar on the lid.
core.cyl(0.2, 1.2, 1.26, "Dark", segs=20, xf=UP, chamfer=0.01)
core.tube(0.16, 0.13, 1.26, 1.3, "Steel", segs=20, xf=UP)

# --- Sensor head ----------------------------------------------------------

sensor = Part("sensor", origin=(0.0, 1.5, 0.0))
y = 1.5
sensor.cyl(0.32, y - 0.14, y + 0.12, "Shell", segs=28, xf=UP, chamfer=0.02)
sensor.tube(0.325, 0.3, y - 0.03, y + 0.01, "Glow", segs=28, xf=UP)
sensor.cyl(0.2, y + 0.12, y + 0.2, "Dark", segs=20, xf=UP, chamfer=0.01)
for k in range(3):
    a = k * 2 * math.pi / 3
    xf = RY(a)
    # An eye in a dark socket on each third of the drum.
    sensor.box((0.16, 0.11, 0.06), (0.0, y + 0.05, 0.3), "Dark", bevel=0.01, segs=1, xf=xf)
    sensor.sphere(0.04, (0.0, y + 0.05, 0.335), "Eye", segs=12, rings=6, xf=xf)
# A whip antenna, a little off centre.
sensor.cyl(0.008, y + 0.2, y + 0.75, "Steel", segs=6, xf=T(0.08, 0.0, 0.0) @ UP)
sensor.sphere(0.018, (0.08, y + 0.76, 0.0), "Glow", segs=8, rings=4)

for p in (core, sensor):
    make(p)
report()
export("assets/models/robots/pylon.glb")
