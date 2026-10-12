"""The drone: a pale lens-shaped body on four ducted rotors, a camera dome
under the nose and a gun pod slung beneath. Writes
assets/models/robots/drone.glb.

Robot space as in src/robots/drone.rs: metres, +Y up, -Z forward. One
object per part kind, each with its origin at that part's spawn offset, so
the game can hang it straight on the part entity:

- `core`    (0, 0, 0)            shell, arms; collider cuboid 0.42 x 0.13 x 0.42
- `sensor`  (0, -0.06, -0.22)    camera dome; collider sphere r 0.07
- `gun`     (0, -0.14, -0.06)    gun pod; collider cuboid 0.08 x 0.08 x 0.32
- `rotor`   (0.32, 0.05, 0.32)   duct and motor of rotor 0 (the game turns
                                 copies to the other corners); collider
                                 cylinder r 0.15 h 0.04
- `blades`  (0.32, 0.07, 0.32)   the propeller, spun by the game

Materials: Shell (pale), Dark (joints, motors), Eye (emissive), Rotor.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, T, S, export, fillet, join, main_guard, make, report,
                    section_rect)

main_guard()

UP = RX(-math.pi / 2)  # lathe axis (Z) -> up (Y)
ARM = 0.32
ROTOR = (ARM, 0.05, ARM)
DIAG = math.atan2(ARM, ARM)

# --- Core ------------------------------------------------------------------------------
core = Part("core")
# Lens-shaped body revolved about the vertical, with panel seams as notches.
prof = [(0.0, 0.066), (0.06, 0.064), (0.062, 0.0615), (0.064, 0.0635), (0.12, 0.055),
        (0.124, 0.051), (0.127, 0.0535), (0.175, 0.038), (0.205, 0.018), (0.213, 0.004),
        (0.211, -0.006), (0.204, -0.008), (0.2, -0.012), (0.17, -0.032), (0.11, -0.05),
        (0.0, -0.056)]
core.lathe(prof, "Shell", segs=48, xf=S(0.96, 1.0, 1.1) @ UP)
# Dark equator seam band.
core.lathe([(0.2095, -0.0075), (0.2125, -0.0075), (0.2125, 0.0005), (0.2095, 0.0005)], "Dark",
           segs=48, xf=S(0.96, 1.0, 1.1) @ UP, closed=True)
# Top spine: a raised dark sensor strip with vents either side.
spine = fillet([(-0.016, -0.17), (0.016, -0.17), (0.022, 0.12), (-0.022, 0.12)], 0.012, 3)
core.prism(spine, 0.052, 0.07, "Dark", axis="y", bevel=0.004, segs=2)
for sx in (-1, 1):
    for i in range(4):
        z = -0.06 + i * 0.03
        core.box((0.04, 0.008, 0.012), (sx * 0.06, 0.058 - abs(z) * 0.05, z), "Dark", bevel=0.003,
                 segs=1)
# Accent: a thin emissive line along the front of the spine.
core.box((0.006, 0.004, 0.09), (0.0, 0.0705, -0.11), "Eye", bevel=0.0015, segs=1)
# Arms out to the ducts: tapered aerofoil beams on the diagonals.
arm_sec = fillet([(-0.026, -0.016), (0.026, -0.016), (0.022, 0.014), (-0.022, 0.014)],
                 [0.01, 0.01, 0.012, 0.012], 3)
for i, (sx, sz) in enumerate(((1, 1), (-1, 1), (-1, -1), (1, -1))):
    yaw = math.atan2(sx, sz)  # angle of the arm about Y, from +Z
    core.prism(arm_sec, 0.15, 0.31, "Shell", axis="z", bevel=0.006, segs=2,
               xf=RY(yaw) @ T(0, 0.02, 0) @ T(0, 0, 0))
    # Dark joint collar where the arm meets the body.
    core.cyl(0.03, 0.15, 0.165, "Dark", segs=20, xf=RY(yaw) @ T(0, 0.02, 0) @ S(1.0, 0.65, 1.0))

# --- Camera dome --------------------------------------------------------------------------
SENSOR = (0.0, -0.06, -0.22)
sensor = Part("sensor", origin=SENSOR)
sensor.sphere(0.056, SENSOR, "Dark", segs=28, rings=14)
# Cowl joining the dome to the body.
sensor.lathe([(0.0, 0.0), (0.05, 0.0), (0.062, 0.02), (0.062, 0.034), (0.0, 0.036)], "Shell",
             segs=28, xf=T(SENSOR[0], SENSOR[1] + 0.012, SENSOR[2]) @ UP)
# The lens: a glowing glass disc in a bezel, looking forward and a little down.
aim = T(*SENSOR) @ RX(math.radians(12))
sensor.cyl(0.032, -0.06, -0.048, "Dark", segs=28, xf=aim, chamfer=0.003)
sensor.lathe([(0.0, -0.0635), (0.012, -0.0628), (0.022, -0.0608), (0.026, -0.059),
              (0.0, -0.059)], "Eye", segs=28, xf=aim)

# --- Gun pod -------------------------------------------------------------------------------
GUN = (0.0, -0.14, -0.06)
gun = Part("gun", origin=GUN)
pod = [(0.0, 0.11), (0.024, 0.108), (0.036, 0.09), (0.04, 0.03), (0.039, -0.08), (0.03, -0.13),
       (0.018, -0.15), (0.0, -0.152)]
gun.lathe(pod, "Shell", segs=24, xf=T(*GUN) @ S(1.0, 0.95, 1.0))
gun.tube(0.016, 0.006, -0.25, -0.13, "Dark", segs=18, xf=T(*GUN), chamfer=0.002)
gun.tube(0.019, 0.006, -0.255, -0.235, "Dark", segs=18, xf=T(*GUN), chamfer=0.003)
# Cooling vents along the pod sides.
for sx in (-1, 1):
    for i in range(5):
        gun.box((0.006, 0.026, 0.008), (GUN[0] + sx * 0.037, GUN[1], GUN[2] - 0.04 + i * 0.022),
                "Dark", bevel=0.002, segs=1)
# Pylon up to the body, and an amber muzzle light.
gun.prism(fillet([(-0.05, -0.105), (0.05, -0.105), (0.03, -0.05), (-0.03, -0.05)], 0.008, 2),
          -0.014, 0.014, "Dark", axis="x", bevel=0.004, segs=1, xf=T(0, 0, GUN[2]))
gun.box((0.012, 0.006, 0.006), (0.0, GUN[1] + 0.024, GUN[2] - 0.12), "Eye", bevel=0.002, segs=1)

# --- Rotor duct (rotor 0) ------------------------------------------------------------------
rotor = Part("rotor", origin=ROTOR)
at = T(*ROTOR)
# Duct ring: an aerofoil section revolved, lip rounded on top.
duct = [(0.148, -0.024), (0.162, -0.026), (0.170, -0.012), (0.171, 0.012), (0.166, 0.028),
        (0.156, 0.034), (0.148, 0.028)]
rotor.lathe(duct, "Shell", segs=48, xf=at @ UP, closed=True)
# Motor pod and three struts to the ring.
rotor.lathe([(0.0, -0.03), (0.022, -0.028), (0.034, -0.012), (0.034, 0.016), (0.028, 0.024),
             (0.0, 0.026)], "Dark", segs=24, xf=at @ UP)
for k in range(3):
    a = math.radians(30 + 120 * k)
    rotor.box((0.118, 0.008, 0.012), (0.091, -0.016, 0.0), "Dark", bevel=0.003, segs=1,
              xf=at @ RY(a))
# A glow strip on the outside of the duct, facing away from the body.
rotor.lathe([(0.1705, -0.004), (0.1735, -0.004), (0.1735, 0.004), (0.1705, 0.004)], "Eye",
            segs=10, xf=at @ RY(DIAG - math.radians(15)) @ UP, closed=True, start=0.0, sweep=30.0)

# --- Propeller --------------------------------------------------------------------------
BLADES = (ARM, 0.07, ARM)
blades = Part("blades", origin=BLADES)
bat = T(*BLADES)
blades.cyl(0.016, -0.006, 0.008, "Rotor", segs=16, xf=bat @ UP, chamfer=0.003)
for k in range(3):
    blade = fillet([(0.012, -0.008), (0.14, -0.013), (0.143, 0.0), (0.135, 0.01), (0.012, 0.011)],
                   [0.0, 0.004, 0.004, 0.004, 0.0], 2)
    # Profile (x, z) in the rotor plane, a thin twisted plate.
    blades.prism(blade, -0.0015, 0.0015, "Rotor", axis="y", bevel=0.0008, segs=1,
                 xf=bat @ RY(math.radians(120 * k)))

for p in (core, sensor, gun, rotor, blades):
    make(p)
report()
export("assets/models/robots/drone.glb")
