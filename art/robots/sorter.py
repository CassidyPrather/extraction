"""The Sorter: the Depot's ceiling gantry. Writes assets/models/robots/sorter.glb.

Hall space (src/robots/sorter.rs): metres, +Y up, the arm reaching down.
Objects and their origins:

- `carriage` (0, 0, 0)   the trolley under the bridge, 1.4 x 0.6 x 1.2; wheels
                         on top, a cyan strip round it, the shoulder mount below
- `head`     (0, -0.42, -0.5)  the scanning head under the front, an eye slit
- `plate`    (0.74, 0, 0)  an armour plate on the carriage's +X side (the game
                         turns a copy to -X)
- `arm_upper` origin at the shoulder, modelled along +Y, 2.3 m to the elbow
- `arm_lower` origin at the elbow, modelled along +Y, 2.1 m to the wrist
- `grab`     origin at the wrist: the claw's housing, hanging -Y
- `jaw`      (0.12, -0.18, 0)  one jaw (+X); the game mirrors it and swings both
- `bridge`   (0, 0, 0)   the beam the carriage runs on, along Z, 15 m long,
                         its trucks at z = +-7.2 on the rails
- `rail`     (0, 0, 0)   one ceiling rail along X, 24 m
- `tote`     (0, 0, 0)   a sorting tote, 0.6 x 0.4 x 0.45, centred

Arm segments bend about X (the game points X along the arm plane's normal).
Materials: Shell, Dark, Glow, Eye, Steel, Warn, Orange.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, make, report, reset

UP = RX(-math.pi / 2)  # lathe/cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
UPPER, LOWER = 2.3, 2.1

reset()

# --- Carriage --------------------------------------------------------------
car = Part("carriage")
car.box((1.4, 0.6, 1.2), (0.0, 0.0, 0.0), "Shell", bevel=0.06, segs=3)
car.box((1.42, 0.05, 1.22), (0.0, -0.12, 0.0), "Glow", bevel=0.012, segs=1)
car.box((1.2, 0.08, 1.0), (0.0, 0.33, 0.0), "Dark", bevel=0.02, segs=1)
for sx in (-1, 1):
    for sz in (-1, 1):
        car.cyl(0.09, -0.05, 0.05, "Steel", segs=16, xf=T(sx * 0.45, 0.42, sz * 0.4) @ SIDE,
                chamfer=0.01)
# Shoulder mount: a slewing ring under the middle.
car.cyl(0.26, -0.36, -0.26, "Dark", segs=24, xf=UP, chamfer=0.015)
car.cyl(0.16, -0.46, -0.34, "Steel", segs=20, xf=UP, chamfer=0.01)
# Hazard chevrons on the ends.
for sz in (-1, 1):
    car.box((0.9, 0.12, 0.012), (0.0, 0.15, sz * 0.605), "Warn", bevel=0.004, segs=1)

# --- Head -----------------------------------------------------------------
HEAD = (0.0, -0.42, -0.5)
head = Part("head", origin=HEAD)
hx, hy, hz = HEAD
head.box((0.34, 0.2, 0.3), (hx, hy, hz), "Shell", bevel=0.04, segs=2)
head.box((0.26, 0.05, 0.03), (hx, hy - 0.01, hz - 0.15), "Dark", bevel=0.01, segs=1)
head.box((0.22, 0.02, 0.012), (hx, hy - 0.01, hz - 0.163), "Eye", bevel=0.004, segs=1)
head.cyl(0.05, hy + 0.08, hy + 0.2, "Dark", segs=12, xf=T(hx, 0, hz) @ UP)

# --- Armour plate ----------------------------------------------------------
PLATE = (0.74, 0.0, 0.0)
plate = Part("plate", origin=PLATE)
plate.box((0.06, 0.5, 1.0), PLATE, "Shell", bevel=0.02, segs=2)
plate.box((0.04, 0.54, 1.04), (0.725, 0.0, 0.0), "Dark", bevel=0.01, segs=1)
for sy in (-1, 1):
    for sz in (-1, 1):
        plate.sphere(0.018, (0.772, sy * 0.19, sz * 0.42), "Steel", segs=10, rings=5)

# --- Arm -------------------------------------------------------------------
upper = Part("arm_upper")
upper.sphere(0.17, (0, 0, 0), "Dark", segs=20, rings=10)
upper.box((0.22, UPPER - 0.2, 0.26), (0, UPPER / 2, 0), "Shell", bevel=0.04, segs=2)
upper.cyl(0.035, 0.25, UPPER - 0.3, "Steel", segs=12, xf=T(0, 0, 0.17) @ UP)
upper.cyl(0.05, 0.2, 1.1, "Dark", segs=12, xf=T(0, 0, 0.17) @ UP)
upper.cyl(0.16, -0.16, 0.16, "Dark", segs=24, xf=T(0, UPPER, 0) @ SIDE, chamfer=0.015)
for sx in (-1, 1):
    upper.box((0.012, 0.6, 0.04), (sx * 0.112, UPPER * 0.55, 0.0), "Glow", bevel=0.004, segs=1)

lower = Part("arm_lower")
lower.box((0.17, LOWER - 0.15, 0.2), (0, LOWER / 2, 0), "Shell", bevel=0.035, segs=2)
lower.cyl(0.03, 0.2, LOWER - 0.25, "Steel", segs=12, xf=T(0, 0, -0.13) @ UP)
lower.cyl(0.1, LOWER - 0.12, LOWER + 0.02, "Dark", segs=18, xf=UP, chamfer=0.01)
lower.box((0.18, 0.1, 0.012), (0.0, LOWER * 0.4, 0.106), "Warn", bevel=0.004, segs=1)

grab = Part("grab")
grab.cyl(0.13, -0.16, 0.0, "Dark", segs=20, xf=UP, chamfer=0.012)
grab.box((0.34, 0.08, 0.2), (0, -0.19, 0), "Steel", bevel=0.015, segs=1)
grab.box((0.2, 0.012, 0.012), (0, -0.04, -0.131), "Glow", bevel=0.003, segs=1)

JAW = (0.12, -0.18, 0.0)
jaw = Part("jaw", origin=JAW)
jaw.box((0.04, 0.34, 0.16), (0.14, -0.36, 0.0), "Dark", bevel=0.012, segs=1)
jaw.box((0.09, 0.04, 0.16), (0.11, -0.2, 0.0), "Dark", bevel=0.01, segs=1)
jaw.box((0.05, 0.04, 0.17), (0.105, -0.52, 0.0), "Orange", bevel=0.01, segs=1)

# --- Bridge and rails ------------------------------------------------------
bridge = Part("bridge")
bridge.box((0.36, 0.3, 15.0), (0, 0.6, 0), "Dark", bevel=0.02, segs=1)
bridge.box((0.38, 0.03, 14.6), (0, 0.44, 0), "Yellow", bevel=0.006, segs=1)
for sz in (-1, 1):
    bridge.box((0.9, 0.32, 0.5), (0, 0.6, sz * 7.2), "Shell", bevel=0.04, segs=2)
    bridge.box((0.2, 0.06, 0.5), (0, 0.75, sz * 7.2), "Glow", bevel=0.01, segs=1)

rail = Part("rail")
rail.box((24.0, 0.18, 0.16), (0, 0, 0), "Steel", bevel=0.015, segs=1)
for i in range(13):
    rail.box((0.08, 0.3, 0.3), (-12.0 + i * 2.0, 0.2, 0), "Dark", bevel=0.01, segs=1)

# --- Tote ------------------------------------------------------------------
tote = Part("tote")
W, H, D, t = 0.6, 0.4, 0.45, 0.025
# (The base sits inside the walls' outline and the walls start 3 mm up
# it: flush faces z-fight.)
tote.box((W - 0.012, t, D - 0.012), (0, -H / 2 + t / 2, 0), "Orange", bevel=0.006, segs=1)
for sx in (-1, 1):
    tote.box((t, H - 0.006, D), (sx * (W / 2 - t / 2), 0.003, 0), "Orange", bevel=0.006, segs=1)
for sz in (-1, 1):
    tote.box((W - 2 * t - 0.002, H - 0.012, t), (0, 0.006, sz * (D / 2 - t / 2 - 0.003)), "Orange",
             bevel=0.006, segs=1)
tote.box((0.2, 0.06, 0.006), (0, 0.06, D / 2 + 0.004), "Shell", bevel=0.002, segs=1)

for p in (car, head, plate, upper, lower, grab, jaw, bridge, rail, tote):
    make(p)
report()
export("assets/models/robots/sorter.glb")
