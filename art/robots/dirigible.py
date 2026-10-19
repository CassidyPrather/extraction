"""The dirigible: a long, pale, panelled robotic airship with a gondola
slung under it, two ducted engines and a cargo winch. Writes
assets/models/robots/dirigible.glb.

Robot space as in src/robots/dirigible.rs: metres, +Y up, -Z forward. The
envelope is 16 m long with a 2.6 m radius. One object per part, origin at
the part's spawn offset, so damage maps to what you see:

- `cell_0`  (0, 0, -4.8)    nose third of the envelope; collider sphere r 2.6
- `cell_1`  (0, 0, 0)       middle third
- `cell_2`  (0, 0, 4.8)     tail third, with the fins
- `core`    (0, -3.1, 0)    gondola, engine pylons and nacelles, winch;
                            collider cuboid 1.4 x 1.1 x 4
- `sensor`  (0, -3.7, -2.0) ball turret under the gondola's nose; sphere r 0.3
- `prop`    (1.3, -2.9, 1.45) the right engine's propeller, spun by the game
                            (it draws a copy on the left engine too)

Materials: Shell (pale), Dark, Eye (emissive accents), Rotor.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, S, export, fillet, main_guard, make, report,
                    section_rect)

main_guard()

LENGTH, RADIUS = 16.0, 2.6
Z0, Z1 = -8.0, 8.0
ZMAX = -1.5          # fattest point
CELLS = [(-8.0, -2.4), (-2.4, 2.4), (2.4, 8.0)]
GAP = 1.0            # degrees between panels


def r_at(z):
    """Envelope radius along the hull."""
    if z <= ZMAX:
        t = (z - ZMAX) / (ZMAX - Z0)
        return RADIUS * math.sqrt(max(0.0, 1 - t * t)) ** 0.9
    t = (z - ZMAX) / (Z1 - ZMAX)
    return RADIUS * max(0.0, 1 - t ** 2.4) ** 0.8


def zs(a, b, n):
    return [a + (b - a) * i / n for i in range(n + 1)]


# Lathe angle 0 is +X, 90 is +Y (lathe about Z, the hull axis).

def cell(i):
    z0, z1 = CELLS[i]
    p = Part(f"cell_{i}", origin=(0.0, 0.0, (i - 1) * 4.8))
    # Dark inner hull under the panels.
    lo = max(z0, Z0 + 0.25)
    hi = min(z1, Z1 - 0.35)
    prof = [(r_at(z) - 0.035, z) for z in zs(lo, hi, 10)]
    p.lathe(prof, "Dark", segs=24)
    # Shell panels: 16 around, three rings per cell, gaps showing the hull.
    rings = 3
    for k in range(rings):
        a = z0 + (z1 - z0) * k / rings + (0.03 if k or i else 0.0)
        b = z0 + (z1 - z0) * (k + 1) / rings - 0.03
        a = max(a, Z0 + 0.6)
        b = min(b, Z1 - 0.5)
        if b <= a:
            continue
        prof = [(r_at(z), z) for z in zs(a, b, 4)]
        for s in range(12):
            # Leave the top strip for the spine.
            start = 30.0 * s + GAP / 2 + 15.0
            p.lathe(prof, "Shell", segs=4, start=start, sweep=30.0 - GAP)
    # Dorsal spine: a dark strip following the top, accent lights on it.
    sa, sb = max(z0, -6.6), min(z1, 6.3)
    path = [(z, r_at(z) + 0.03) for z in zs(sa, sb, 10)]
    p.sweep(path, [(x, n) for x, n in fillet([(-0.22, -0.06), (0.22, -0.06), (0.16, 0.05),
                                               (-0.16, 0.05)], 0.03, 2)], "Dark")
    for z in zs(sa + 0.5, sb - 0.5, 2):
        p.box((0.05, 0.03, 0.5), (0.0, r_at(z) + 0.085, z), "Eye", bevel=0.01, segs=1,
              xf=RX(-math.atan2(r_at(z + 0.1) - r_at(z - 0.1), 0.2), about=(0, r_at(z) + 0.085, z)))
    # Running lights: cyan lamps in rows along the belly and both flanks,
    # seen from below at 80 m as dotted lines the length of the ship. Each
    # cell's go out when the cell is shot down.
    la, lb = max(z0 + 0.4, -6.9), min(z1 - 0.4, 6.4)
    n = max(1, int((lb - la) / 0.9))
    for j in range(n + 1):
        z = la + (lb - la) * j / n
        slope = -math.atan2(r_at(z + 0.1) - r_at(z - 0.1), 0.2)
        for ang in (-90.0, -25.0, -155.0):
            a = math.radians(ang)
            rr = r_at(z) + 0.02
            at = (rr * math.cos(a), rr * math.sin(a), z)
            p.box((0.16, 0.06, 0.42), (0.0, 0.0, 0.0), "Glow", bevel=0.02, segs=1,
                  xf=T(*at) @ RZ(a + math.pi / 2) @ RX(slope))
    # A seam band at each end of the cell (where it meets the next).
    for z in (z0, z1):
        if Z0 < z < Z1:
            p.lathe([(r_at(z) - 0.04, z - 0.06), (r_at(z) + 0.035, z - 0.05),
                     (r_at(z) + 0.035, z + 0.05), (r_at(z) - 0.04, z + 0.06)], "Dark", segs=32,
                    closed=True)
    # Side running lights on the widest band.
    if i == 1:
        for sx in (-1, 1):
            p.box((0.04, 0.12, 1.4), (sx * (r_at(-0.9) + 0.03), 0.0, -0.9), "Eye", bevel=0.015,
                  segs=1)
    if i == 0:
        # Nose cap: dark with a glowing ring.
        cap = [(0.0, Z0 - 0.02)] + [(r_at(z) + 0.01, z) for z in zs(Z0 + 0.02, Z0 + 0.62, 6)]
        p.lathe(cap, "Dark", segs=32)
        z = Z0 + 0.66
        p.lathe([(r_at(z) - 0.03, z - 0.04), (r_at(z) + 0.03, z - 0.03),
                 (r_at(z) + 0.03, z + 0.03), (r_at(z) - 0.03, z + 0.04)], "Eye", segs=40,
                closed=True)
        p.sphere(0.12, (0.0, 0.0, Z0 - 0.05), "Eye", segs=12, rings=6)
    if i == 2:
        tail_cell(p)
    return p


def tail_cell(p):
    # Tail cone and a beacon.
    cone = [(r_at(z) + 0.01, z) for z in zs(Z1 - 0.55, Z1 - 0.05, 4)] + [(0.0, Z1 + 0.15)]
    p.lathe(cone, "Dark", segs=24)
    p.sphere(0.07, (0.0, 0.0, Z1 + 0.15), "Eye", segs=10, rings=5)
    # Four fins, a rudder/elevator panel on each trailing edge.
    for k in range(4):
        rot = RZ(math.radians(90 * k))
        fin = [(4.7, r_at(4.7) - 0.15), (7.35, r_at(7.35) - 0.15), (7.55, 2.55), (7.25, 2.7),
               (6.6, 2.72)]
        fin = fillet(fin, [0.0, 0.0, 0.08, 0.15, 0.12], 4)

        def width(z, y):
            # Thick at the root, thin at the tip.
            t = max(0.0, min(1.0, (y - 1.0) / 1.7))
            h = 0.09 - 0.06 * t
            return (-h, h)

        p.prism(fin, 0, 0, "Shell", axis="x", width=width, xf=rot)
        rud = fillet([(7.4, r_at(7.4) - 0.1), (7.9, r_at(7.9) - 0.1), (8.05, 2.45),
                      (7.6, 2.5)], 0.05, 2)
        p.prism(rud, -0.035, 0.035, "Dark", axis="x", xf=rot)
        # Tip light. (5 mm wider a side than the fin tip: flush faces z-fight.)
        p.box((0.07, 0.05, 0.5), (0.0, 2.72, 6.95), "Eye", bevel=0.015, segs=1,
              xf=rot)


# --- Gondola (core) -----------------------------------------------------------------
CORE = (0.0, -3.1, 0.0)
core = Part("core", origin=CORE)
at = T(*CORE)
# A long rounded pod, flattened into a 1.4 x 1.1 section.
gprof = [(0.0, -2.0), (0.22, -1.97), (0.38, -1.85), (0.48, -1.65), (0.54, -1.35), (0.55, -1.0),
         (0.55, 1.1), (0.52, 1.5), (0.44, 1.8), (0.3, 1.97), (0.0, 2.02)]
gsc = S(0.7 / 0.55, 1.0, 1.0)
core.lathe(gprof, "Shell", segs=28, xf=at @ gsc)
# Panel seams round the pod, and a dark visor band round the nose.
for z in (-0.9, 0.2, 1.25):
    core.lathe([(0.545, z - 0.025), (0.558, z - 0.02), (0.558, z + 0.02), (0.545, z + 0.025)],
               "Dark", segs=28, xf=at @ gsc, closed=True)
core.lathe([(0.47, -1.62), (0.535, -1.4), (0.545, -1.18), (0.53, -1.18), (0.52, -1.4),
            (0.455, -1.62)], "Dark", segs=20, xf=at @ gsc @ RZ(math.radians(200)), closed=True,
           sweep=140)
for k in range(5):
    a = math.radians(225 + 22.5 * k)
    core.box((0.05, 0.05, 0.04), (0.0, 0.0, 0.0), "Eye", bevel=0.01, segs=1,
             xf=at @ gsc @ T(0.548 * math.cos(a), 0.548 * math.sin(a), -1.3))
# Dorsal fairing up into the envelope.
fair = fillet([(-1.6, 0.4), (1.6, 0.4), (1.2, 0.75), (-1.3, 0.75)], 0.1, 3)
core.prism([(z, y - 3.1) for z, y in fair], -0.32, 0.32, "Shell", axis="x", bevel=0.05, segs=2)
# Engine pylons and nacelles with ducted propellers.
ENG = (1.3, -2.9, 0.8)
for sx in (-1, 1):
    ex, ey, ez = sx * ENG[0], ENG[1], ENG[2]
    pyl = fillet([(0.25, -0.05), (1.15, -0.05), (1.05, 0.06), (0.35, 0.06)], 0.03, 2)
    core.prism([(ez + a - 0.7, ey + b) for a, b in pyl], 0.0, 0.0, "Dark", axis="x",
               width=lambda a, b, sx=sx: (min(sx * 0.6, sx * 1.1), max(sx * 0.6, sx * 1.1)))
    nac = [(0.0, -0.55), (0.12, -0.52), (0.2, -0.42), (0.24, -0.2), (0.24, 0.35), (0.2, 0.55),
           (0.12, 0.62), (0.0, 0.64)]
    core.lathe(nac, "Shell", segs=20, xf=T(ex, ey, ez))
    core.lathe([(0.243, -0.1), (0.25, -0.08), (0.25, 0.08), (0.243, 0.1)], "Eye", segs=20,
               xf=T(ex, ey, ez), closed=True, start=-30 if sx > 0 else 150, sweep=60)
    # Duct ring round the propeller (the disc at z 1.45 in the blockout).
    duct = [(0.53, 1.25), (0.6, 1.22), (0.66, 1.3), (0.67, 1.48), (0.63, 1.62), (0.57, 1.63),
            (0.54, 1.55)]
    core.lathe([(r, z) for r, z in duct], "Shell", segs=28, xf=T(ex, ey, 0.0), closed=True)
    for k in range(3):
        a = math.radians(90 + 120 * k)
        core.box((0.36, 0.035, 0.08), (0.36, 0.0, 0.0), "Dark", segs=1,
                 xf=T(ex, ey, 1.3) @ RZ(a))
# Winch under the middle: a drum in a housing, the cable fairlead at the bottom.
core.box((0.6, 0.12, 0.7), (0.0, -3.62, 0.0), "Dark", bevel=0.04, segs=2)
core.cyl(0.13, -0.24, 0.24, "Dark", segs=16, xf=T(0, -3.58, 0.0) @ RY(math.pi / 2))
core.lathe([(0.0, 0.0), (0.07, 0.0), (0.09, 0.03), (0.07, 0.06), (0.0, 0.06)], "Dark", segs=16,
           xf=T(0, -3.66, 0) @ RX(math.pi / 2))
core.lathe([(0.05, -0.01), (0.06, 0.0), (0.05, 0.01), (0.04, 0.0)], "Eye", segs=16,
           xf=T(0, -3.69, 0) @ RX(math.pi / 2), closed=True)
# The sensor turret's mount under the nose.
core.box((0.2, 0.3, 0.42), (0.0, -3.42, -1.82), "Dark", bevel=0.05, segs=2)
# Docking guides the pod's top sits between.
for sx in (-1, 1):
    core.box((0.06, 0.12, 1.4), (sx * 0.45, -3.62, 0.0), "Dark", bevel=0.02, segs=1)

# --- Sensor turret -----------------------------------------------------------------
SENSOR = (0.0, -3.7, -2.0)
sensor = Part("sensor", origin=SENSOR)
sensor.sphere(0.24, SENSOR, "Dark", segs=24, rings=12)
sensor.lathe([(0.0, 0.0), (0.22, 0.0), (0.28, 0.08), (0.28, 0.16), (0.0, 0.18)], "Shell",
             segs=24, xf=T(SENSOR[0], SENSOR[1] + 0.05, SENSOR[2]) @ RX(-math.pi / 2))
aim = T(*SENSOR) @ RX(math.radians(20))
sensor.cyl(0.11, -0.26, -0.2, "Dark", segs=20, xf=aim, chamfer=0.01)
sensor.lathe([(0.0, -0.275), (0.05, -0.272), (0.08, -0.265), (0.09, -0.258), (0.0, -0.258)],
             "Eye", segs=20, xf=aim)

# --- Propeller (right engine) ----------------------------------------------------------
PROP = (1.3, -2.9, 1.45)
prop = Part("prop", origin=PROP)
pat = T(*PROP)
prop.lathe([(0.0, -0.14), (0.07, -0.12), (0.1, -0.05), (0.1, 0.05), (0.0, 0.07)], "Dark",
           segs=16, xf=pat)
for k in range(5):
    blade = fillet([(0.08, -0.05), (0.5, -0.07), (0.52, 0.0), (0.49, 0.05), (0.08, 0.06)],
                   [0.0, 0.02, 0.02, 0.02, 0.0], 2)
    # A blade in the disc plane (x, y), pitched about its own length.
    prop.prism([(a, b) for a, b in blade], -0.008, 0.008, "Rotor", axis="z",
               xf=pat @ RZ(2 * math.pi * k / 5) @ RX(0.35))

cells = [cell(i) for i in range(3)]
for p in cells + [core, sensor, prop]:
    make(p)
report()
export("assets/models/robots/dirigible.glb")
