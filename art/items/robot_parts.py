"""Salvage that says which robot it came from: one part per class, each
built from that robot's own geometry (art/robots/), torn off. Writes
assets/models/items/<id>.glb (item space: see _kit.py), centred on its box.

- `drone_rotor_motor`    a drone's motor pod (drone.py `rotor`) with its
                         struts snapped short and a curl of its duct, the
                         glow band dark
- `walker_leg_actuator`  a walker's knee (walker.py `leg_upper`'s hinge
                         drum, caps and glow ring) between a stub of thigh
                         and a stub of shin, the screw jack behind it
- `crawler_capacitor`    a crawler's spark gun (walker.py `emitter`: the
                         sleeve, prongs and ring) on the capacitor bank
                         that fed it
- `dirigible_gas_valve`  the airship's lift-gas valve: a flanged seat with
                         a scrap of envelope round it, the poppet's
                         actuator and its eye
- `sorter_servo_pack`    the Sorter's elbow (sorter.py): the hinge drum
                         with stubs of both arm beams, the ram on the
                         upper arm's back, its glow strips and warning band
- `pylon_relay_coil`     a relay pylon's coil: copper wound on a dark core
                         between white end caps, glowing insulators

Robot bodies drops these from a robot's parts still alive when it dies
(robots/wreck.rs). Materials: Shell, Dark, Glow, Eye, Warn, Steel, Copper,
Red, Black, Canvas.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, centred, item, tube_path, wanted  # noqa: E402
from common import RX, RY, RZ, T, Part  # noqa: E402


def drone_rotor_motor():
    p = Part("body")
    # The pod, as on the drone.
    p.lathe([(0.0, -0.03), (0.022, -0.028), (0.034, -0.012), (0.034, 0.016), (0.028, 0.024),
             (0.0, 0.026)], "Dark", segs=24, xf=UP)
    p.lathe([(0.0, 0.025), (0.012, 0.026), (0.014, 0.032), (0.0, 0.035)], "Black", segs=16,
            xf=UP)
    # Two blade roots on the hub, snapped.
    for k in range(2):
        p.box((0.05, 0.004, 0.018), (0.035, 0.03, 0.0), "Black", bevel=0.001, segs=1,
              xf=RY(math.pi * k + 0.4) @ RX(0.15))
    # The three struts, snapped at different lengths; one still holds a
    # curl of the duct with its dark glow band.
    for k, L in enumerate((0.118, 0.05, 0.07)):
        a = math.radians(30 + 120 * k)
        p.box((L, 0.008, 0.012), (0.032 + L / 2, -0.016, 0.0), "Dark", bevel=0.003, segs=1,
              xf=RY(a))
    duct = [(0.168, -0.03), (0.178, -0.03), (0.178, 0.03), (0.168, 0.03)]
    p.lathe(duct, "Shell", segs=8, xf=RY(math.radians(30)) @ UP, closed=True, start=-25,
            sweep=50)
    p.lathe([(0.1765, -0.012), (0.1795, -0.012), (0.1795, 0.012), (0.1765, 0.012)], "Dark",
            segs=8, xf=RY(math.radians(30)) @ UP, closed=True, start=-25, sweep=50)
    tube_path(p, [(0.0, -0.03, 0.0), (0.006, -0.04, 0.008), (0.016, -0.046, 0.004)], 0.0018,
              "Red", sides=5, closed=False)
    tube_path(p, [(0.003, -0.03, 0.003), (-0.008, -0.042, 0.01)], 0.0018, "Black", sides=5,
              closed=False)
    return centred([p])


def walker_leg_actuator():
    p = Part("body")
    # The knee: the hinge drum across X, white caps, the glow rings.
    p.cyl(0.052, -0.055, 0.055, "Dark", segs=24, xf=ALONG_X, chamfer=0.006)
    for sx in (-1, 1):
        p.cyl(0.04, 0.0, 0.012, "Shell", segs=24, xf=T(sx * 0.055, 0, 0) @ RY(sx * math.pi / 2),
              chamfer=0.003)
        p.tube(0.036, 0.024, 0.0105, 0.0155, "Glow", segs=24,
               xf=T(sx * 0.055, 0, 0) @ RY(sx * math.pi / 2))
    # A stub of thigh up from it and a stub of shin down, torn off ragged.
    p.lathe([(0.0, 0.0), (0.05, 0.0), (0.055, 0.05), (0.052, 0.16), (0.044, 0.17),
             (0.0, 0.17)], "Shell", segs=16, xf=T(0, 0.03, 0) @ UP)
    p.lathe([(0.0, 0.0), (0.05, 0.0), (0.048, 0.12), (0.04, 0.13), (0.0, 0.13)], "Shell",
            segs=16, xf=RX(math.pi / 2 + 0.35) @ T(0, 0, 0.03))
    # The screw jack behind the knee, from thigh to shin.
    p.cyl(0.011, 0.0, 0.2, "Steel", segs=10, xf=T(0, 0.12, 0.05) @ RX(math.pi / 2 + 0.2))
    p.cyl(0.018, 0.0, 0.07, "Dark", segs=12, xf=T(0, 0.12, 0.05) @ RX(math.pi / 2 + 0.2),
          chamfer=0.004)
    tube_path(p, [(0.02, 0.17, 0.0), (0.03, 0.2, 0.02), (0.02, 0.22, 0.05)], 0.003, "Red",
              sides=5, closed=False)
    return centred([p])


def crawler_capacitor():
    p = Part("body")
    # The spark gun's sleeve, ring and prongs, pointing -Z.
    p.tube(0.045, 0.02, -0.07, 0.0, "Dark", segs=20, chamfer=0.004)
    p.tube(0.034, 0.012, -0.076, -0.068, "Glow", segs=20)
    for sx in (-1, 1):
        p.box((0.012, 0.012, 0.06), (sx * 0.026, 0.0, -0.09), "Dark", bevel=0.003, segs=1)
    # The capacitor bank behind it: three cans strapped round a spine.
    for k in range(3):
        a = 2 * math.pi * k / 3 + math.pi / 2
        x, y = 0.032 * math.cos(a), 0.032 * math.sin(a)
        p.cyl(0.026, 0.0, 0.11, "Shell", segs=16, xf=T(x, y, 0.0), chamfer=0.004)
        p.cyl(0.02, 0.11, 0.116, "Black", segs=12, xf=T(x, y, 0.0))
    for z in (0.03, 0.08):
        p.tube(0.062, 0.058, z - 0.007, z + 0.007, "Dark", segs=20)
    tube_path(p, [(0.03, 0.0, 0.116), (0.045, 0.01, 0.14), (0.03, 0.03, 0.16)], 0.0025, "Red",
              sides=5, closed=False)
    tube_path(p, [(-0.03, 0.0, 0.116), (-0.04, -0.015, 0.145)], 0.0025, "Black", sides=5,
              closed=False)
    return centred([p])


def dirigible_gas_valve():
    p = Part("body")
    # The seat: a flanged ring the envelope was clamped to, bolts round it.
    p.lathe([(0.06, 0.0), (0.12, 0.0), (0.12, 0.014), (0.075, 0.014), (0.07, 0.04),
             (0.06, 0.04)], "Shell", segs=28, xf=UP, closed=True)
    for k in range(10):
        a = 2 * math.pi * k / 10
        p.cyl(0.006, 0.014, 0.022, "Steel", segs=6,
              xf=T(0.1 * math.cos(a), 0, 0.1 * math.sin(a)) @ UP)
    # A torn scrap of envelope still under the clamp ring.
    p.lathe([(0.12, 0.004), (0.17, 0.0), (0.21, -0.012), (0.205, -0.016), (0.165, -0.006),
             (0.12, 0.0)], "Canvas", segs=10, xf=UP, closed=True, start=40, sweep=130)
    # The poppet's actuator on its bridge, and its eye.
    for sx in (-1, 1):
        p.box((0.012, 0.07, 0.02), (sx * 0.055, 0.075, 0.0), "Dark", bevel=0.003, segs=1)
    p.box((0.13, 0.016, 0.03), (0.0, 0.11, 0.0), "Dark", bevel=0.004, segs=1)
    p.cyl(0.03, 0.118, 0.17, "Shell", segs=16, xf=UP, chamfer=0.006)
    p.lathe([(0.0, 0.168), (0.016, 0.168), (0.012, 0.178), (0.0, 0.18)], "Eye", segs=12, xf=UP)
    p.cyl(0.05, 0.02, 0.03, "Dark", segs=20, xf=UP)    # the poppet in its seat
    p.cyl(0.006, 0.03, 0.11, "Steel", segs=8, xf=UP)
    return centred([p])


def pylon_relay_coil():
    p = Part("body")
    H = 0.2
    # The dark core, copper wound round it in bands, white end caps.
    p.cyl(0.03, 0.0, H, "Dark", segs=16, xf=UP)
    for k in range(4):
        y0 = 0.025 + k * 0.04
        p.cyl(0.046, y0, y0 + 0.032, "Copper", segs=20, xf=UP, chamfer=0.003)
    for y0, y1 in ((-0.012, 0.012), (H - 0.012, H + 0.012)):
        p.cyl(0.058, y0, y1, "Shell", segs=20, xf=UP, chamfer=0.004)
    # Glowing insulators between the bands.
    for k in range(3):
        y = 0.025 + 0.036 + k * 0.04
        p.tube(0.05, 0.044, y - 0.003, y + 0.003, "Glow", segs=20, xf=UP)
    # Its terminals, one wire torn away.
    for sx in (-1, 1):
        p.box((0.012, 0.02, 0.012), (sx * 0.035, H + 0.022, 0.0), "Steel", bevel=0.002, segs=1)
    tube_path(p, [(0.035, H + 0.032, 0.0), (0.05, H + 0.05, 0.01), (0.07, H + 0.045, 0.03)],
              0.0025, "Red", sides=5, closed=False)
    return centred([p])


def sorter_servo_pack():
    p = Part("body")
    # The Sorter's elbow drum (sorter.py): the hinge along X.
    p.cyl(0.16, -0.16, 0.16, "Dark", segs=28, xf=ALONG_X, chamfer=0.012)
    for sx in (-1, 1):
        p.cyl(0.11, 0.0, 0.015, "Shell", segs=24, xf=T(sx * 0.16, 0, 0) @ RY(sx * math.pi / 2),
              chamfer=0.004)
    # A stub of the upper arm up from it, with its glow strips and the ram
    # on its back; a stub of the forearm down and forward, its warning band.
    p.box((0.22, 0.3, 0.26), (0.0, 0.27, 0.0), "Shell", bevel=0.012, segs=1)
    for sx in (-1, 1):
        p.box((0.004, 0.2, 0.012), (sx * 0.111, 0.3, 0.0), "Glow", segs=1)
    p.cyl(0.05, 0.0, 0.16, "Dark", segs=14, xf=T(0, 0.24, 0.17) @ RX(-math.pi / 2), chamfer=0.006)
    p.cyl(0.035, 0.0, 0.2, "Steel", segs=12, xf=T(0, 0.12, 0.17) @ RX(-math.pi / 2 + 0.25))
    fore = RX(math.pi / 2 + 0.7)
    p.box((0.17, 0.2, 0.24), (0.0, 0.0, 0.0), "Shell", bevel=0.01, segs=1,
          xf=fore @ T(0.0, 0.0, 0.24))
    p.box((0.18, 0.205, 0.08), (0.0, 0.0, 0.0), "Warn", segs=1, xf=fore @ T(0.0, 0.0, 0.28))
    # Torn ends: cables out of both stubs.
    tube_path(p, [(0.04, 0.42, 0.03), (0.07, 0.48, 0.06), (0.05, 0.52, 0.1)], 0.008, "Red",
              sides=6, closed=False)
    tube_path(p, [(-0.05, 0.42, -0.02), (-0.08, 0.47, -0.06)], 0.008, "Black", sides=6,
              closed=False)
    return centred([p])


if __name__ == "__main__":
    for item_id, build in (("drone_rotor_motor", drone_rotor_motor),
                           ("walker_leg_actuator", walker_leg_actuator),
                           ("crawler_capacitor", crawler_capacitor),
                           ("dirigible_gas_valve", dirigible_gas_valve),
                           ("pylon_relay_coil", pylon_relay_coil),
                           ("sorter_servo_pack", sorter_servo_pack)):
        if wanted(item_id):
            item(item_id, build)
