"""The walker: a panelled sensor orb on four ball-jointed legs, a visor slit
for an eye and a small gun turret on top. Writes
assets/models/robots/walker.glb.

Robot space as in src/robots/walker.rs: metres, +Y up, -Z forward. Objects
and their origins (each the spawn offset of its part):

- `core`    (0, 0, 0)          panelled orb with hip collars; collider sphere r 0.4
- `sensor`  (0, 0.12, -0.36)   visor; collider cuboid 0.3 x 0.08 x 0.06
- `turret`  (0, 0.46, 0)       gun turret; collider cuboid 0.16 x 0.12 x 0.4
- `leg_upper`  origin at the hip, modelled along +Y, 0.8 m to the knee
- `leg_lower`  origin at the knee, modelled along +Y, 0.95 m to the foot

Leg segments have their bend axis along X (the game points X along the
normal of the leg's plane) so knees hinge the right way.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, S, export, fillet, main_guard, make, report)

main_guard()

UP = RX(-math.pi / 2)  # lathe axis (Z) -> up (Y); lathe angle 90 deg faces -Z (forward)
R = 0.4
UPPER, LOWER = 0.8, 0.95
HIP_RADIUS = 0.32


def band(lat0, lat1, n=8):
    return [(R * math.cos(math.radians(lat0 + (lat1 - lat0) * i / n)),
             R * math.sin(math.radians(lat0 + (lat1 - lat0) * i / n))) for i in range(n + 1)]


# --- Core: an orb of shell panels over a dark inner sphere ----------------------------
core = Part("core")
core.sphere(R - 0.007, (0, 0, 0), "Dark", segs=32, rings=16)
GAP = 1.6  # degrees between panels
bands = [(-88, -52, 6, 3), (-52, -14, 8, 4), (14, 50, 8, 4), (50, 89.9, 4, 2)]
for lat0, lat1, sectors, rings_ in bands:
    for k in range(sectors):
        a0 = 360.0 / sectors * k + 22.5 + GAP / 2
        sweep = 360.0 / sectors - GAP
        prof = band(lat0 + GAP / 2, lat1 - GAP / 2, rings_ * 2)
        if lat1 > 89:
            prof = band(lat0 + GAP / 2, 89.9, rings_ * 2)
        core.lathe(prof, "Shell", segs=max(3, int(sweep / 6)), xf=UP, start=a0, sweep=sweep)
# The equator band: a slightly proud ring of shell segments with dark vents.
for k in range(12):
    a0 = 30.0 * k + 0.8
    core.lathe([(R + 0.004, -0.06), (R + 0.012, -0.03), (R + 0.012, 0.03), (R + 0.004, 0.06),
                (R - 0.01, 0.06), (R - 0.01, -0.06)], "Shell", segs=5, xf=UP, start=a0,
               sweep=28.4, closed=True)
    # (5 mm proud of the band: flush faces z-fight.)
    core.lathe([(R + 0.017, -0.018), (R + 0.017, 0.018), (R + 0.006, 0.018), (R + 0.006, -0.018)],
               "Dark", segs=3, xf=UP, start=a0 + 9, sweep=10, closed=True)
# Hip collars and the ball sockets inside them.
for i in range(4):
    a = math.pi / 4 + i * math.pi / 2
    hip = (math.cos(a) * HIP_RADIUS, -0.12, math.sin(a) * HIP_RADIUS)
    d = [hip[0], hip[1], hip[2]]
    ln = math.sqrt(sum(c * c for c in d))
    d = [c / ln for c in d]
    # Rotate +Z onto d.
    yaw = math.atan2(d[0], d[2])
    pitch = -math.asin(d[1])
    orient = RY(yaw) @ RX(pitch)
    core.tube(0.088, 0.062, R - 0.02, R + 0.022, "Dark", segs=24, xf=orient, chamfer=0.004)
    core.tube(0.093, 0.084, R - 0.03, R + 0.006, "Shell", segs=24, xf=orient, chamfer=0.002)
    core.sphere(0.066, hip, "Dark", segs=20, rings=10)
# Turret ring on top.
core.lathe([(0.0, R + 0.002), (0.085, R + 0.002), (0.095, R - 0.01), (0.095, R - 0.03),
            (0.0, R - 0.03)], "Dark", segs=32, xf=UP)

# --- Visor (the sensor part) -----------------------------------------------------------
SENSOR = (0.0, 0.12, -0.36)
sensor = Part("sensor", origin=SENSOR)


def lat_arc(r0, r1, l0, l1, n=4):
    out = [(r0 * math.cos(math.radians(l0 + (l1 - l0) * i / n)),
            r0 * math.sin(math.radians(l0 + (l1 - l0) * i / n))) for i in range(n + 1)]
    out += [(r1 * math.cos(math.radians(l1 - (l1 - l0) * i / n)),
             r1 * math.sin(math.radians(l1 - (l1 - l0) * i / n))) for i in range(n + 1)]
    return out


# Dark visor housing proud of the orb, the glowing slit inside it.
sensor.lathe(lat_arc(R - 0.01, R + 0.022, 8, 28), "Dark", segs=12, xf=UP, start=67, sweep=46,
             closed=True)
sensor.lathe(lat_arc(R + 0.022, R + 0.026, 14.5, 21.5, 2), "Eye", segs=12, xf=UP, start=72,
             sweep=36, closed=True)
# A brighter pupil in the middle of the slit.
sensor.lathe(lat_arc(R + 0.026, R + 0.031, 15.5, 20.5, 2), "Eye", segs=3, xf=UP, start=86,
             sweep=8, closed=True)

# --- Turret -----------------------------------------------------------------------------------
TURRET = (0.0, 0.46, 0.0)
turret = Part("turret", origin=TURRET)
# (Every corner rounder than the 0.02 bevel: a tighter one folds the
# bevelled side's outline over itself, and the folded side z-fights.)
side = fillet([(0.19, 0.405), (0.205, 0.462), (0.16, 0.517), (-0.07, 0.519), (-0.185, 0.48),
               (-0.198, 0.44), (-0.165, 0.405)], [0.022, 0.025, 0.03, 0.03, 0.022, 0.022, 0.022],
              4)
turret.prism(side, -0.078, 0.078, "Shell", axis="x", bevel=0.02, segs=3)
# Dark vents on the flanks and a sensor bar on the nose.
for sx in (-1, 1):
    for i in range(4):
        turret.box((0.01, 0.05, 0.012), (sx * 0.078, 0.462, 0.02 + i * 0.03), "Dark", bevel=0.003,
                   segs=1)
turret.box((0.09, 0.016, 0.012), (0.0, 0.497, -0.165), "Dark", bevel=0.004, segs=1)
turret.box((0.05, 0.006, 0.006), (0.0, 0.498, -0.1705), "Eye", bevel=0.002, segs=1)
# Barrel with a ribbed shroud and muzzle brake.
turret.tube(0.019, 0.007, -0.50, -0.17, "Dark", segs=20, xf=T(0, 0.455, 0), chamfer=0.002)
for i in range(5):
    turret.tube(0.025, 0.007, -0.27 - i * 0.035, -0.255 - i * 0.035, "Dark", segs=20,
                xf=T(0, 0.455, 0), chamfer=0.003)
turret.tube(0.026, 0.008, -0.53, -0.495, "Dark", segs=20, xf=T(0, 0.455, 0), chamfer=0.004)

# --- Legs ----------------------------------------------------------------------------------
upper = Part("leg_upper")
upper.sphere(0.064, (0, 0, 0), "Dark", segs=20, rings=10)
thigh = [(0.0, 0.05), (0.046, 0.06), (0.062, 0.14), (0.068, 0.30), (0.066, 0.31), (0.066, 0.32),
         (0.068, 0.33), (0.064, 0.52), (0.062, 0.53), (0.062, 0.54), (0.06, 0.55), (0.05, 0.69),
         (0.04, 0.73), (0.0, 0.74)]
upper.lathe(thigh, "Shell", segs=24, xf=S(0.78, 1.0, 1.0) @ UP)
# Hydraulic ram along the back of the thigh.
upper.cyl(0.014, 0.10, 0.62, "Dark", segs=10, xf=T(0, 0, 0.062) @ UP)
upper.cyl(0.009, 0.62, 0.77, "Dark", segs=10, xf=T(0, 0, 0.055) @ UP)
# Knee: a hinge drum across the leg's plane (X), and its cap.
upper.cyl(0.052, -0.055, 0.055, "Dark", segs=24, xf=T(0, UPPER, 0) @ RY(math.pi / 2),
          chamfer=0.006)
upper.cyl(0.04, -0.065, 0.065, "Shell", segs=24, xf=T(0, UPPER, 0) @ RY(math.pi / 2),
          chamfer=0.008)

lower = Part("leg_lower")
shin = [(0.0, 0.03), (0.046, 0.05), (0.056, 0.12), (0.05, 0.36), (0.049, 0.37), (0.049, 0.38),
        (0.046, 0.39), (0.036, 0.5), (0.0, 0.52)]
lower.lathe(shin, "Shell", segs=24, xf=S(0.82, 1.0, 1.0) @ UP)
# Accent ring at the bottom of the shin shell.
lower.lathe([(0.039, 0.47), (0.0415, 0.47), (0.0405, 0.48), (0.038, 0.48)], "Eye", segs=24,
            xf=S(0.82, 1.0, 1.0) @ UP, closed=True)
lower.cyl(0.022, 0.45, LOWER - 0.03, "Dark", segs=14, xf=UP)
lower.cyl(0.03, 0.70, 0.74, "Dark", segs=14, xf=UP, chamfer=0.006)
# Ball foot with a rubber tread ring.
lower.sphere(0.045, (0, LOWER - 0.01, 0), "Dark", segs=20, rings=10)
lower.lathe([(0.0, LOWER - 0.06), (0.03, LOWER - 0.06), (0.042, LOWER - 0.035), (0.0, LOWER - 0.035)],
            "Dark", segs=18, xf=UP)

for p in (core, sensor, turret, upper, lower):
    make(p)
report()
export("assets/models/robots/walker.glb")
