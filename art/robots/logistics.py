"""The dirigible's cargo pod and the grappler's harpoon launcher. Writes
assets/models/robots/cargo_pod.glb and assets/models/robots/harpoon.glb.

Cargo pod, in pod space (src/robots/dirigible.rs): an open-topped tub,
collider 1.3 x 0.9 x 1.8 centred 0.1 m below the origin; it sits on the
ground with its origin 0.45 m up and hangs from the cable at (0, 0.45, 0).

- `pod`  the tub, skids, lifting eyes and a strongback over the top with
         the lifting eye the cable hooks into; a glowing band round it

Harpoon launcher, in robot space (a walker body, src/robots/walker.rs):
the orb is a sphere of radius 0.4 at the origin, its turret ring on top.

- `launcher`  a derrick on the turret ring: a boom leaning forward to a
              head 3 m off the ground with the launcher on it (a barbed
              harpoon in its muzzle), the line drum behind, a hazard band
              round the orb

Materials: Shell, Dark, Glow, Warn, Yellow, Steel.
"""

import math
import os
import sys

import bmesh

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, S, export, fillet, main_guard, make, report, reset)

UP = RX(-math.pi / 2)
ALONG_X = RY(math.pi / 2)


def rrect(hx, hz, r, n=4):
    """A rounded rectangle in plan (x, z), counter-clockwise."""
    return fillet([(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)], r, n)


def cup(p, outer_top, outer_bot, inner, y_bot, y_top, y_floor, mat, ring=False):
    """An open-topped tub: outer wall from y_bot to y_top, a rim, the inner
    wall down to a floor at y_floor, and a closed bottom. Outlines are
    matching (x, z) loops. A `ring` has no floor: its inner wall comes back
    to the outer at the bottom."""
    bm = p.bm
    n = len(outer_top)
    ob = [bm.verts.new((x, y_bot, z)) for x, z in outer_bot]
    ot = [bm.verts.new((x, y_top, z)) for x, z in outer_top]
    it = [bm.verts.new((x, y_top, z)) for x, z in inner]
    ib = [bm.verts.new((x, y_floor, z)) for x, z in inner]
    faces = []
    for i in range(n):
        j = (i + 1) % n
        faces.append(bm.faces.new((ob[i], ob[j], ot[j], ot[i])))
        faces.append(bm.faces.new((ot[i], ot[j], it[j], it[i])))
        faces.append(bm.faces.new((it[i], it[j], ib[j], ib[i])))
    if ring:
        for i in range(n):
            j = (i + 1) % n
            faces.append(bm.faces.new((ib[i], ib[j], ob[j], ob[i])))
    else:
        faces.append(bm.faces.new(ib))
        faces.append(bm.faces.new(list(reversed(ob))))
    p._finish_piece(ob + ot + it + ib, faces, mat)


# =============================================================================
# Cargo pod
# =============================================================================
main_guard()
pod = Part("pod")
HX, HZ = 0.65, 0.9
Y_BOT, Y_TOP = -0.43, 0.34
cup(pod, rrect(HX, HZ, 0.16), rrect(HX - 0.04, HZ - 0.04, 0.13), rrect(HX - 0.05, HZ - 0.05, 0.12),
    Y_BOT, Y_TOP, -0.36, "Shell")
# A dark rim cap and corner bumpers.
rim_o = rrect(HX + 0.012, HZ + 0.012, 0.17)
rim_i = rrect(HX - 0.06, HZ - 0.06, 0.11)
cup(pod, rim_o, rim_o, rim_i, Y_TOP - 0.05, Y_TOP + 0.025, Y_TOP - 0.05, "Dark", ring=True)
for sx in (-1, 1):
    for sz in (-1, 1):
        pod.box((0.1, 0.62, 0.1), (sx * (HX - 0.035), -0.08, sz * (HZ - 0.035)), "Dark",
                bevel=0.03, segs=2, xf=T(0, 0, 0))
# Vertical ribs on the long sides and ends, a glowing band between them.
for sx in (-1, 1):
    for z in (-0.45, 0.0, 0.45):
        pod.box((0.03, 0.66, 0.06), (sx * (HX + 0.005), -0.07, z), "Dark", bevel=0.01, segs=1)
    pod.box((0.012, 0.035, 1.5), (sx * (HX + 0.008), 0.17, 0.0), "Glow", bevel=0.006, segs=1)
for sz in (-1, 1):
    for x in (-0.28, 0.28):
        pod.box((0.06, 0.66, 0.03), (x, -0.07, sz * (HZ + 0.005)), "Dark", bevel=0.01, segs=1)
    pod.box((1.0, 0.035, 0.012), (0.0, 0.17, sz * (HZ + 0.008)), "Glow", bevel=0.006, segs=1)
# Skids underneath.
for sx in (-1, 1):
    pod.box((0.1, 0.06, 1.7), (sx * 0.42, Y_BOT - 0.015, 0.0), "Dark", bevel=0.02, segs=2)
# Inside: a grating floor so loot doesn't sit on bare shell.
for k in range(7):
    pod.box((1.1, 0.012, 0.03), (0.0, -0.354, -0.66 + k * 0.22), "Dark")
# Strongback over the top, along the pod, with the lifting eye mid-way.
beam = fillet([(-0.94, 0.3), (0.94, 0.3), (0.94, 0.36), (0.2, 0.41), (-0.2, 0.41), (-0.94, 0.36)],
              [0.0, 0.0, 0.02, 0.04, 0.04, 0.02], 3)
pod.prism(beam, -0.045, 0.045, "Dark", axis="x", bevel=0.01, segs=1)
pod.box((0.11, 0.02, 0.3), (0.0, 0.418, 0.0), "Glow", bevel=0.006, segs=1)
pod.lathe([(0.03, -0.012), (0.045, -0.016), (0.055, 0.0), (0.045, 0.016), (0.03, 0.012),
           (0.026, 0.0)], "Steel", segs=16, xf=T(0, 0.45, 0) @ ALONG_X, closed=True)
# Lifting eyes on the corners.
for sx in (-1, 1):
    for sz in (-1, 1):
        pod.lathe([(0.025, -0.01), (0.035, -0.012), (0.042, 0.0), (0.035, 0.012),
                   (0.025, 0.01)], "Steel", segs=12,
                  xf=T(sx * (HX - 0.05), Y_TOP + 0.05, sz * (HZ - 0.05)) @ RY(math.pi / 4 * sx * sz)
                  @ ALONG_X, closed=True)
make(pod)
report()
export("assets/models/robots/cargo_pod.glb")

# =============================================================================
# Harpoon launcher
# =============================================================================
reset()
ln = Part("launcher")
R = 0.4
# A derrick, not a turret: a walker with a launcher on top had a walker's
# outline from any side at 30 m, and with a spar on it, a hairline more
# (docs/craft/ROBOT_READING.md, the silhouette gauge `robots::silhouette`).
# A boom as thick as a leg rises from a slewing yoke on the turret ring,
# leaning forward to a head 3 m up with the launcher on it; a line drum as
# wide as the orb hangs behind it, its counterweight. Side-on: a diagonal
# and a disc; facing you or away: a tall upright over the orb.
# Slewing yoke on the turret ring.
ln.lathe([(0.0, R - 0.005), (0.11, R - 0.005), (0.105, R + 0.05), (0.0, R + 0.055)], "Dark",
         segs=16, xf=UP)
for sx in (-1, 1):
    ln.box((0.03, 0.16, 0.2), (sx * 0.1, R + 0.11, 0.08), "Dark", bevel=0.01, segs=1)
FOOT = (0.0, R + 0.15, 0.1)
ln.cyl(0.045, -0.13, 0.13, "Dark", segs=14, xf=T(*FOOT) @ ALONG_X, chamfer=0.006)
# The boom: a shell box section, a teal strip down each side (the robots'
# own glow), from its foot pin to the head.
ANGLE = math.radians(62.0)
LENGTH = 1.42
DIR = (0.0, math.sin(ANGLE), -math.cos(ANGLE))
HEAD = tuple(FOOT[i] + DIR[i] * LENGTH for i in range(3))
boom = T(*FOOT) @ RX(-(math.pi / 2 - ANGLE))
ln.box((0.15, LENGTH + 0.06, 0.17), (0.0, LENGTH / 2, 0.0), "Shell", bevel=0.02, segs=2, xf=boom)
for sx in (-1, 1):
    ln.box((0.006, LENGTH - 0.3, 0.03), (sx * 0.077, LENGTH / 2, 0.0), "Glow", bevel=0.002,
           segs=1, xf=boom)
# The luffing ram under it, from the yoke's front to the boom's middle.
ram0 = (0.0, R + 0.07, -0.12)
ram1 = tuple(FOOT[i] + DIR[i] * LENGTH * 0.5 + (0.0, -0.06, -0.04)[i] for i in range(3))
rv = [ram1[i] - ram0[i] for i in range(3)]
rl = math.sqrt(sum(c * c for c in rv))
ram = T(*ram0) @ RX(-math.atan2(-rv[2], rv[1]))
ln.cyl(0.05, 0.0, rl * 0.55, "Dark", segs=12, xf=ram @ UP)
ln.cyl(0.03, rl * 0.5, rl, "Steel", segs=10, xf=ram @ UP)
# The launcher on the head: a pale housing like the walker's turret, the
# tube out of its front with the barbed harpoon in it. (No warning stripes
# up here: they glow, and alone 3 m up they bloomed like a lamp.)
hx = T(*HEAD)
side = fillet([(0.16, -0.08), (0.2, 0.0), (0.14, 0.08), (-0.14, 0.08), (-0.22, 0.04),
               (-0.22, -0.05), (-0.16, -0.08)], [0.02, 0.03, 0.03, 0.03, 0.02, 0.02, 0.02], 3)
ln.prism([(x, y + HEAD[1]) for x, y in side], -0.09, 0.09, "Shell", axis="x", bevel=0.018,
         segs=2, xf=T(0, 0, HEAD[2]))
ln.cyl(0.05, -0.11, 0.11, "Dark", segs=14, xf=hx @ ALONG_X, chamfer=0.006)
tube = T(0, HEAD[1] + 0.01, HEAD[2])
ln.tube(0.05, 0.032, -0.5, -0.14, "Dark", segs=14, xf=tube, chamfer=0.004)
for i in range(3):
    ln.tube(0.056, 0.032, -0.26 - i * 0.08, -0.24 - i * 0.08, "Dark", segs=14, xf=tube,
            chamfer=0.004)
ln.cyl(0.012, -0.56, -0.3, "Steel", segs=10, xf=tube)
ln.lathe([(0.0, -0.66), (0.012, -0.64), (0.026, -0.585), (0.024, -0.56), (0.0, -0.56)], "Steel",
         segs=12, xf=tube)
for k in range(3):
    a = 2 * math.pi * k / 3 + 0.5
    ln.box((0.008, 0.008, 0.07), (0.0, 0.0, 0.0), "Steel",
           xf=tube @ RZ(a) @ T(0.03, 0, -0.565) @ RY(-0.35))
# The line drum behind, on its own trunnions off the yoke: yellow line
# between dark flanges, as wide as the orb is deep.
DRUM = (0.0, R + 0.1, 0.46)
reel = T(*DRUM) @ ALONG_X
ln.cyl(0.2, -0.15, 0.15, "Yellow", segs=16, xf=reel)
for x in (-0.17, 0.17):
    ln.cyl(0.25, x - 0.02, x + 0.02, "Dark", segs=18, xf=reel, chamfer=0.004)
ln.cyl(0.04, -0.21, 0.21, "Dark", segs=12, xf=reel)
for sx in (-1, 1):
    ln.box((0.03, 0.06, 0.4), (sx * 0.2, DRUM[1] - 0.02, 0.27), "Dark", bevel=0.01, segs=1)
# The line from the drum up the boom to the launcher: steel, not the
# drum's yellow (a robot's yellow and warning paint glow its own teal,
# `weak_points::paint`: a lit line up there bloomed into a lamp).
lv = [HEAD[i] - DRUM[i] for i in range(3)]
ll = math.sqrt(sum(c * c for c in lv))
line = T(0, DRUM[1] + 0.2, DRUM[2]) @ RX(-math.atan2(-lv[2], lv[1]))
ln.cyl(0.012, 0.0, ll - 0.05, "Steel", segs=6, xf=line @ UP)
# Hazard band round the orb.
ln.lathe([(0.385, 0.08), (0.398, 0.085), (0.398, 0.11), (0.385, 0.115)], "Warn",
         segs=40, xf=UP, closed=True)
print("muzzle", HEAD[0], HEAD[1] + 0.01, HEAD[2] - 0.66)
make(ln)
report()
export("assets/models/robots/harpoon.glb")
