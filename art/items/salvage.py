"""Robot salvage and artifacts. Writes assets/models/items/<id>.glb for
each (item space: see _kit.py). The gun pod has `dims` (barrel to -Z);
the rest have none, so each sits in a cube of side cbrt(bulk / 1000).

Salvage is in the robots' look (pale shell, dark joints): the rotor
assembly, servo, sensor eye, robot cell and a walker's gun pod. Artifacts
are small wrong things: the featherstone (a pebble with glowing veins), the hearth (a dark
lump split over a glowing core), the glass eye (a clear orb with an iris
inside) and the aegis knot (a trefoil tangle of copper).
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, blob, centred, cube, item, ring, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, fillet


# --- Salvage ----------------------------------------------------------------------------------

def rotor_assembly():
    s = cube(0.6)
    p = Part("body")
    # A drone motor torn off its arm: the pod, the hub, stubs of blades,
    # the strut it hung from snapped short.
    p.lathe([(0.0, -0.022), (0.022, -0.021), (0.032, -0.01), (0.032, 0.012), (0.026, 0.018),
             (0.0, 0.02)], "Dark", segs=24, xf=UP)
    p.lathe([(0.0, 0.019), (0.012, 0.02), (0.014, 0.026), (0.0, 0.029)], "Black", segs=16, xf=UP)
    for k in range(3):
        blade = fillet([(0.01, -0.008), (0.04, -0.012), (0.04, 0.01), (0.01, 0.01)],
                       [0.0, 0.0, 0.0, 0.0], 1)
        p.prism(blade, 0.022, 0.0245, "Black", axis="y", xf=RY(2 * math.pi * k / 3))
    for k in range(3):
        a = math.radians(30 + 120 * k)
        p.box((0.03 if k else 0.016, 0.006, 0.01), (0.032 + (0.015 if k else 0.008), -0.012, 0.0),
              "Dark", bevel=0.002, segs=1, xf=RY(a))
    # A scrap of the pale duct still on one strut.
    p.lathe([(0.036, -0.016), (0.041, -0.017), (0.042, 0.0), (0.04, 0.012), (0.036, 0.008)],
            "Shell", segs=6, xf=UP, closed=True, start=200, sweep=40)
    # Torn wires.
    tube_path(p, [(0.0, -0.022, 0.0), (0.004, -0.03, 0.006), (0.012, -0.035, 0.004),
                  (0.016, -0.04, -0.006)], 0.0018, "Red", sides=5, closed=False)
    tube_path(p, [(0.002, -0.022, 0.002), (-0.006, -0.03, 0.008), (-0.01, -0.037, 0.016)],
              0.0018, "Black", sides=5, closed=False)
    return [p]


def servo():
    s = cube(0.4)
    p = Part("body")
    # A pale housing, a dark gearbox cap with the output spline, mounting
    # ears, a cable tail.
    body = fillet([(-0.03, -0.022), (0.03, -0.022), (0.03, 0.022), (-0.03, 0.022)], 0.006, 3)
    p.prism(body, -0.03, 0.012, "Shell", axis="y", bevel=0.003, segs=2)
    p.prism(fillet([(-0.026, -0.018), (0.018, -0.018), (0.018, 0.018), (-0.026, 0.018)], 0.005, 2),
            0.012, 0.02, "Dark", axis="y", bevel=0.002, segs=1)
    p.cyl(0.011, 0.02, 0.026, "Dark", segs=20, xf=T(0.006, 0, 0) @ UP, chamfer=0.0015)
    p.cyl(0.006, 0.026, 0.034, "Steel", segs=12, xf=T(0.006, 0, 0) @ UP)
    for k in range(6):
        p.box((0.0012, 0.008, 0.0015), (0.0, 0.03, 0.0), "Steel",
              xf=T(0.006, 0, 0) @ RY(math.pi * k / 3) @ T(0.0062, 0, 0))
    for sx in (-1, 1):
        p.box((0.012, 0.004, 0.03), (sx * 0.035, 0.0, 0.0), "Shell", bevel=0.001, segs=1)
        p.cyl(0.003, -0.001, 0.0045, "Dark", segs=8, xf=T(sx * 0.037, 0, 0) @ UP)
    p.box((0.062, 0.003, 0.002), (0.0, -0.01, 0.0225), "Dark")
    tube_path(p, [(-0.03, -0.02, 0.0), (-0.034, -0.026, 0.006), (-0.03, -0.034, 0.014),
                  (-0.02, -0.036, 0.02)], 0.0022, "Black", sides=6, closed=False)
    p.box((0.008, 0.006, 0.01), (-0.018, -0.036, 0.023), "Dark", bevel=0.001, segs=1)
    return [p]


def sensor_eye():
    s = cube(0.25)
    p = Part("body")
    # The drone's camera dome, pulled: dark ball, pale cowl, the lens.
    p.sphere(0.024, (0.0, -0.004, 0.0), "Dark", segs=24, rings=12)
    p.lathe([(0.0, 0.0), (0.022, 0.0), (0.028, 0.01), (0.028, 0.018), (0.0, 0.02)], "Shell",
            segs=24, xf=T(0, 0.004, 0) @ UP)
    aim = T(0, -0.004, 0) @ RX(math.radians(10))
    p.cyl(0.014, -0.027, -0.021, "Dark", segs=20, xf=aim, chamfer=0.0015)
    p.lathe([(0.0, -0.0285), (0.006, -0.028), (0.01, -0.0272), (0.0115, -0.0265),
             (0.0, -0.0265)], "Eye", segs=20, xf=aim)
    # Torn mount and wires out of the top.
    p.box((0.014, 0.006, 0.01), (0.0, 0.026, 0.004), "Dark", bevel=0.0015, segs=1)
    tube_path(p, [(0.003, 0.026, 0.006), (0.008, 0.03, 0.012), (0.012, 0.028, 0.02)], 0.0016,
              "Red", sides=5, closed=False)
    tube_path(p, [(-0.003, 0.026, 0.006), (-0.006, 0.031, 0.01), (-0.014, 0.03, 0.012)], 0.0016,
              "Black", sides=5, closed=False)
    return [p]


def robot_cell():
    s = cube(0.5)
    p = Part("body")
    # A heavy pale cell pack: ribbed shell, dark end caps, terminals, a
    # charge window.
    prof = fillet([(-0.032, -0.022), (0.032, -0.022), (0.032, 0.022), (-0.032, 0.022)], 0.008, 3)
    p.prism(prof, -0.032, 0.032, "Shell", axis="z", bevel=0.003, segs=2)
    for z in (-0.035, 0.035):
        p.prism(fillet([(-0.033, -0.023), (0.033, -0.023), (0.033, 0.023), (-0.033, 0.023)],
                       0.009, 3), z - 0.004, z + 0.004, "Dark", axis="z", bevel=0.002, segs=1)
    for k in range(4):
        p.box((0.0655, 0.003, 0.004), (0.0, -0.006 + 0.0, -0.018 + k * 0.012), "Dark")
    p.box((0.003, 0.008, 0.04), (0.0325, 0.006, 0.0), "Glow", bevel=0.001, segs=1)
    for x, mat in ((-0.014, "Red"), (0.014, "Black")):
        p.cyl(0.004, 0.039, 0.046, mat, segs=10, xf=T(x, 0.006, 0), chamfer=0.001)
    p.box((0.024, 0.006, 0.006), (0.0, 0.023, 0.0), "Dark", bevel=0.002, segs=1)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


def robot_gunpod():
    p = Part("body")
    # A walker's gun turret torn off its mount, barrel to -Z: the pale
    # shell with dark vents and a dead sensor bar, the barrel in its ribbed
    # shroud and muzzle brake, the feed chute with a stub of belt, the
    # ripped mount and wires under it. Someone has tied a rope sling on and
    # wired out the trigger solenoid's lever.
    yb = -0.125
    side = [(0.5, -0.09), (0.535, -0.01), (0.47, 0.08), (0.0, 0.083), (-0.13, 0.055),
            (-0.15, -0.01), (-0.12, -0.09)]
    p.prism(fillet(side, [0.03, 0.04, 0.05, 0.05, 0.04, 0.03, 0.03], 4), -0.085, 0.085, "Shell",
            axis="x", bevel=0.022, segs=2)
    for sx in (-1, 1):
        for i in range(4):
            p.box((0.01, 0.06, 0.016), (sx * 0.084, 0.0, 0.1 + i * 0.045), "Dark", bevel=0.003,
                  segs=1)
        p.box((0.01, 0.004, 0.42), (sx * 0.085, -0.055, 0.2), "Dark")
    p.box((0.1, 0.022, 0.03), (0.0, 0.0, 0.0), "Dark", bevel=0.006, segs=1,
          xf=T(0.0, 0.065, -0.095) @ RX(0.2))
    p.box((0.07, 0.007, 0.004), (0.0, 0.0, -0.0175), "Lens", xf=T(0.0, 0.065, -0.095) @ RX(0.2))
    # Barrel, shroud, brake.
    by = -0.005
    bx = T(0.0, by, 0.0)
    p.lathe([(0.0, -0.165), (0.04, -0.165), (0.046, -0.15), (0.046, -0.12), (0.0, -0.12)], "Dark",
            segs=24, xf=bx)
    p.tube(0.022, 0.008, -0.5, -0.16, "Dark", segs=16, xf=bx, chamfer=0.002)
    for i in range(5):
        z0 = -0.25 - i * 0.03
        p.tube(0.03, 0.02, z0 - 0.012, z0, "Dark", segs=16, xf=bx, chamfer=0.003)
    p.tube(0.032, 0.009, -0.55, -0.498, "Dark", segs=16, xf=bx, chamfer=0.004)
    for sx in (-1, 1):
        for z in (-0.538, -0.518):
            p.box((0.006, 0.014, 0.012), (sx * 0.03, by, z), "Black")
    # Feed chute on the right flank and the belt stub hanging from it.
    p.box((0.024, 0.07, 0.09), (0.096, -0.02, 0.27), "Dark", bevel=0.004, segs=1)
    for k in range(3):
        y = -0.062 - k * 0.0135
        p.box((0.012, 0.009, 0.034), (0.096, y, 0.27), "Bakelite")
        p.lathe([(0.0, 0.0), (0.0038, 0.0), (0.0038, 0.008), (0.0, 0.014)], "Steel", segs=6,
                xf=T(0.096, y, 0.253) @ RY(math.pi))
        for z in (0.258, 0.282):
            p.box((0.016, 0.011, 0.005), (0.096, y, z), "Dark")
    # The torn mount under it: a stub of bracket, bolts, a snapped ram,
    # wires hanging.
    p.box((0.09, 0.045, 0.12), (0.0, yb + 0.0225, 0.22), "Dark", bevel=0.004, segs=1)
    for sx in (-1, 1):
        p.box((0.008, 0.03, 0.07), (0.0, 0.0, 0.0), "Dark",
              xf=T(sx * 0.05, yb + 0.022, 0.2) @ RZ(sx * 0.35) @ RX(0.1))
        for z in (0.18, 0.26):
            p.cyl(0.006, 0.045, 0.051, "Steel", segs=6,
                  xf=T(0, yb + 0.028, z) @ RY(sx * math.pi / 2))
    p.cyl(0.011, 0.1, 0.165, "Steel", segs=12, xf=T(0.0, yb + 0.02, 0.0) @ RX(-0.12))
    p.lathe([(0.0, 0.098), (0.012, 0.098), (0.0135, 0.1), (0.008, 0.096), (0.0, 0.092)], "Steel",
            segs=10, xf=T(0.0, yb + 0.02, 0.0) @ RX(-0.12))
    for mat, pts in (("Red", [(0.02, yb + 0.03, 0.28), (0.035, yb + 0.012, 0.3),
                              (0.03, yb + 0.003, 0.33), (0.045, yb + 0.004, 0.36)]),
                     ("Black", [(-0.015, yb + 0.03, 0.282), (-0.03, yb + 0.01, 0.31),
                                (-0.022, yb + 0.003, 0.345)]),
                     ("Yellow", [(0.0, yb + 0.03, 0.28), (0.006, yb + 0.006, 0.31),
                                 (0.018, yb + 0.004, 0.33)])):
        tube_path(p, pts, 0.003, mat, sides=6, closed=False)
    # Rope sling: tied round the shroud, over the top, to a lug at the back.
    ring(p, 0.034, 0.004, "Rope", xf=T(0.0, by, -0.33), segs=16, sides=6)
    p.box((0.02, 0.03, 0.016), (0.0, 0.066, 0.49), "Steel", bevel=0.003, segs=1)
    tube_path(p, [(0.0, by + 0.036, -0.33), (0.0, 0.05, -0.22), (0.0, 0.08, -0.08),
                  (0.0, 0.093, 0.1), (0.0, 0.093, 0.33), (0.0, 0.085, 0.47)], 0.0055, "Rope",
              sides=6, closed=False)
    # The trigger: a solenoid lever hanging out of the belly, wired.
    p.box((0.008, 0.032, 0.012), (0.0, 0.0, 0.0), "Steel", bevel=0.002, segs=1,
          xf=T(0.03, -0.1, 0.43) @ RX(0.25))
    p.cyl(0.009, -0.012, 0.012, "Copper", segs=12, xf=T(0.03, -0.085, 0.45) @ ALONG_X)
    tube_path(p, [(0.03, -0.085, 0.462), (0.04, -0.1, 0.48), (0.05, -0.095, 0.5)], 0.0025, "Red",
              sides=5, closed=False)
    return [p]


# --- Artifacts --------------------------------------------------------------------------------

def art_featherstone():
    s = cube(0.05)
    p = Part("body")
    # A smooth grey pebble, flattened, with faint glowing veins on it.
    blob(p, 0.017, (0.0, 0.0, 0.0), "Pebble", scale=(1.0, 0.62, 0.82),
         bumps=[((1, 0.2, 0.3), 0.08), ((-0.5, -0.3, -1), 0.1), ((0.2, 1, -0.4), -0.05)])
    for k, (a, b) in enumerate(((0.2, 1.9), (2.4, 4.0), (4.5, 5.9))):
        pts = []
        for i in range(7):
            t = a + (b - a) * i / 6
            y = 0.0035 * math.sin(t * 3 + k)
            pts.append((0.0171 * math.cos(t) * 1.0, 0.0105 * 0.98 + y * 0.0 - 0.0005 + 0.0,
                        0.0171 * 0.82 * math.sin(t)))
        tube_path(p, [(x * 0.98, y * 0.3 + 0.002 * math.sin(i), z * 0.98)
                      for i, (x, y, z) in enumerate(pts)], 0.0007, "Strange", sides=4,
                  closed=False)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


def art_hearth():
    s = cube(0.3)
    p = Part("body")
    # A glowing core showing through the splits of a dark, rough lump.
    p.sphere(0.022, (0.0, 0.0, 0.0), "Ember", segs=16, rings=8)
    chunks = [((0.012, 0.006, 0.004), (1, 0.4, 0.2)), ((-0.011, 0.008, -0.004), (-1, 0.5, -0.2)),
              ((0.002, -0.012, 0.008), (0.1, -1, 0.5)), ((-0.004, 0.0, 0.014), (-0.2, 0.1, 1)),
              ((0.006, 0.004, -0.014), (0.3, 0.2, -1)), ((0.0, 0.016, 0.0), (0, 1, 0))]
    for c, d in chunks:
        blob(p, 0.0152, c, "Char", bumps=[(d, 0.25), ((d[1], d[2], d[0]), -0.15)], segs=12,
             rings=7, scale=(1.0, 0.9, 1.0))
    return [p]


def art_glasseye():
    s = cube(0.04)
    p = Part("body")
    # A clear orb with an iris and pupil floating inside it.
    p.sphere(0.0165, (0.0, 0.0, 0.0), "Clear", segs=24, rings=12)
    p.lathe([(0.0, -0.006), (0.004, -0.0062), (0.0085, -0.0058), (0.0105, -0.005),
             (0.0105, -0.004), (0.0, -0.004)], "Strange", segs=24)
    p.cyl(0.0038, -0.0066, -0.0056, "Black", segs=16)
    p.sphere(0.0125, (0.0, 0.0, 0.0012), "Paper", segs=16, rings=8, scale=(1, 1, 0.55))
    return [p]


def art_aegis():
    s = cube(0.2)
    p = Part("body")
    # A trefoil knot of copper wire, a second finer strand round it.
    pts, pts2 = [], []
    n = 72
    for i in range(n):
        t = 2 * math.pi * i / n
        x = math.sin(t) + 2 * math.sin(2 * t)
        y = math.cos(t) - 2 * math.cos(2 * t)
        z = -math.sin(3 * t)
        k = 0.0072
        pts.append((x * k, z * k * 1.4, y * k))
        q = t * 7
        pts2.append((x * k + 0.0028 * math.cos(q), z * k * 1.4 + 0.0028 * math.sin(q),
                     y * k + 0.0028 * math.sin(q)))
    tube_path(p, pts, 0.0034, "Copper", sides=8)
    tube_path(p, pts2, 0.0009, "Strange", sides=4)
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


for item_id, fn in (("rotor_assembly", rotor_assembly), ("servo", servo),
                    ("sensor_eye", sensor_eye), ("robot_cell", robot_cell),
                    ("robot_gunpod", robot_gunpod),
                    ("art_featherstone", art_featherstone), ("art_hearth", art_hearth),
                    ("art_glasseye", art_glasseye), ("art_aegis", art_aegis)):
    if wanted(item_id):
        item(item_id, fn)
