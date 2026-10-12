"""The truck: an old Soviet six-wheel army truck (ZiL-131 / Ural-375
family) with a canvas-covered bed. Writes assets/models/vehicles/truck.glb.

Truck space as in src/raid/truck.rs: metres, +Y up from the ground, -Z
forward. The gameplay colliders (`TRUCK_PARTS`) stay boxes; this models
round them:

- cab        x +-1.15, y 0.9..2.6, z -3.9..-1.9
- bonnet     x +-1.0,  y 0.9..1.8, z -4.95..-3.85
- bed floor  top at FLOOR = 1.1, x +-1.15, z -1.8..1.8; bed sides 0.55 high
- canvas     sides at x +-1.15 up to the roof at FLOOR + 2.05; the front
             wall at z -1.8 (the memorial board hangs on it at z -1.76)
- steps      treads at y 0.37 (z 2.1..2.4) and 0.74 (z 1.8..2.1)

Objects (all static but the wheel):

- `chassis`  frame, axles, bumpers, tanks, spare wheel, mud flaps, lamps
- `cab`      cab and bonnet, glass, a little interior
- `bed`      plank floor, drop sides, front board, tie-down rails
- `canopy`   canvas, hoops and roof slats, rolled rear curtain, work lamp
- `wheel`    one road wheel built at the front right wheel (1.0, 0.55,
             -3.9), hub facing +X; the game draws a copy at every wheel and
             turns the left ones round

Materials: Paint (olive painted steel), Canvas, Boards, Iron (dark steel),
Rubber, Glass, Lamp, LampRed, Rope.
"""

import math
import os
import random
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, S, export, fillet, arc, main_guard, make, report,
                    section_rect, section_circle)

main_guard()
random.seed(131)

FLOOR = 1.1
ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> X
UP = RX(-math.pi / 2)       # lathe/cyl axis (Z) -> Y
WHEEL_R = 0.545
WHEEL = (1.0, 0.55, -3.9)
HOOPS = [-1.775, -0.89, 0.0, 0.89, 1.775]


def make_cut(part, cutter):
    """Like common.make, but the faces the cutter leaves take the cutter's
    own materials (the cab's inside is dark, not paint)."""
    from common import _bm_to_object, box_uv
    obj = _bm_to_object(part)
    cutter.origin = part.origin.copy()
    cobj = _bm_to_object(cutter, part.name + "_cutter")
    cobj.location = obj.location
    mod = obj.modifiers.new("cut", "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.solver = "EXACT"
    mod.object = cobj
    mod.use_self = True
    mod.use_hole_tolerant = True
    mod.material_mode = "TRANSFER"
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)
    bpy.data.objects.remove(cobj, do_unlink=True)
    # Round the shell's edges after cutting (the exact solver shatters a
    # pre-rounded slab's corners).
    bev = obj.modifiers.new("bevel", "BEVEL")
    bev.width = 0.035
    bev.segments = 2
    bev.limit_method = "ANGLE"
    bev.angle_limit = math.radians(50)
    bev.use_clamp_overlap = True
    bev.harden_normals = False
    bpy.ops.object.modifier_apply(modifier=bev.name)
    me = obj.data
    box_uv(me, obj.location)
    me.shade_smooth()
    me.set_sharp_from_angle(angle=math.radians(40))
    wn = obj.modifiers.new("wn", "WEIGHTED_NORMAL")
    wn.mode = "FACE_AREA"
    wn.keep_sharp = True
    wn.weight = 100
    bpy.ops.object.modifier_apply(modifier=wn.name)
    return obj


def mirror_x(fn):
    for sx in (-1, 1):
        fn(sx)


# =============================================================================
# Wheel
# =============================================================================


def build_wheel(p, xf):
    """A road wheel, axle along X, hub facing +X, centred at the origin of
    `xf`."""
    ax = xf @ ALONG_X
    # Tyre carcass: bulging sidewalls, a flat crown under the lugs.
    tyre = [(0.31, -0.168), (0.37, -0.19), (0.44, -0.196), (0.49, -0.185), (0.512, -0.165),
            (0.518, -0.13), (0.518, 0.13), (0.512, 0.165), (0.49, 0.185), (0.44, 0.196),
            (0.37, 0.19), (0.31, 0.168)]
    p.lathe(tyre, "Rubber", segs=32, xf=ax, closed=True)
    # Cross-country lugs: chevrons, two half-lugs per pitch, staggered.
    n = 20
    for k in range(n):
        for side in (-1, 1):
            a = 2 * math.pi * (k + (0.5 if side > 0 else 0.0)) / n
            lug = RX(a) @ T(side * 0.085, 0.0, 0.0) @ RY(side * math.radians(22))
            p.box((0.15, 0.034, 0.075), (0.0, WHEEL_R - 0.016, 0.0), "Rubber", xf=xf @ lug)
    # Steel disc wheel: rim flanges, dished centre, hub and nuts.
    rim = [(0.315, -0.165), (0.325, -0.17), (0.33, -0.16), (0.3, -0.14), (0.29, -0.06),
           (0.295, 0.06), (0.3, 0.14), (0.33, 0.16), (0.325, 0.172), (0.31, 0.168),
           (0.28, 0.14), (0.27, 0.06)]
    p.lathe(rim, "Paint", segs=24, xf=ax, closed=True)
    disc = [(0.28, 0.06), (0.24, 0.1), (0.17, 0.115), (0.14, 0.14), (0.0, 0.14), (0.0, 0.12),
            (0.14, 0.12), (0.17, 0.095), (0.24, 0.08), (0.28, 0.04)]
    p.lathe(disc, "Paint", segs=24, xf=ax, closed=True)
    # Lightening holes in the disc read as dark ovals.
    for k in range(6):
        a = 2 * math.pi * k / 6 + 0.3
        p.cyl(0.033, 0.1, 0.119, "Iron", segs=10, xf=ax @ RZ(a) @ T(0.205, 0, 0)
              @ S(1.0, 1.35, 1.0))
    # Hub: the axle cap, a ring of nuts and the tyre-pressure valve housing.
    p.lathe([(0.0, 0.215), (0.06, 0.212), (0.075, 0.2), (0.085, 0.14), (0.11, 0.135),
             (0.11, 0.13)], "Iron", segs=20, xf=ax)
    for k in range(8):
        a = 2 * math.pi * k / 8
        p.cyl(0.016, 0.138, 0.162, "Iron", segs=6, xf=ax @ RZ(a) @ T(0.125, 0, 0))
    p.cyl(0.03, 0.215, 0.25, "Iron", segs=12, xf=ax, chamfer=0.006)
    p.cyl(0.008, 0.0, 0.16, "Iron", segs=6, xf=ax @ T(0, 0, 0.235) @ RY(math.pi / 2))


wheel = Part("wheel", origin=WHEEL)
build_wheel(wheel, T(*WHEEL))

# =============================================================================
# Chassis: frame, axles, bumpers, tanks, spare, flaps, lamps
# =============================================================================

ch = Part("chassis")

# Frame rails: C-channels, open side inward.
for sx in (-1, 1):
    x = sx * 0.46
    ch.box((0.012, 0.24, 7.1), (x + sx * 0.034, 0.74, -1.45), "Paint")
    for dy in (-0.114, 0.114):
        ch.box((0.08, 0.012, 7.1), (x, 0.74 + dy, -1.45), "Paint")
# Cross-members.
for z in (-4.85, -3.2, -1.95, -0.85, 0.8, 1.92):
    ch.box((0.86, 0.16, 0.08), (0.0, 0.74, z), "Paint")

# Front bumper: a deep channel with tow hooks and a winch fairlead.
bump = fillet([(-0.12, 0.6), (0.06, 0.6), (0.06, 0.88), (-0.12, 0.88)], 0.02, 2)
ch.prism([(z - 5.0 + 0.0, y) for z, y in bump], -1.2, 1.2, "Paint", axis="x", bevel=0.012,
         segs=1)
for sx in (-1, 1):
    # (forward, up) offsets of a towing hook's side outline.
    hook = [(0.0, 0.0), (0.05, 0.0), (0.08, 0.04), (0.06, 0.1), (0.02, 0.11), (0.0, 0.08),
            (0.03, 0.07), (0.04, 0.04), (0.0, 0.03)]
    ch.prism([(-5.12 - f, 0.66 + u) for f, u in hook], sx * 0.62 - 0.015, sx * 0.62 + 0.015,
             "Iron", axis="x")
ch.box((0.3, 0.12, 0.06), (0.0, 0.74, -5.14), "Iron", bevel=0.01, segs=1)
ch.cyl(0.12, -0.45, 0.45, "Iron", segs=16, xf=T(0, 0.72, -4.9) @ ALONG_X, chamfer=0.01)
# Front number plate.
ch.box((0.5, 0.11, 0.01), (0.55, 0.74, -5.127), "Cream", bevel=0.003, segs=1)

# Axles: banjo housings, differentials, leaf springs.
for z in (-3.9, 0.2, 1.4):
    ch.cyl(0.055, -0.82, 0.82, "Iron", segs=12, xf=T(0, 0.55, z) @ ALONG_X)
    ch.sphere(0.16, (0.0, 0.53, z), "Iron", segs=16, rings=8, scale=(1.0, 0.95, 1.1))
    for sx in (-1, 1):
        # Leaf pack, curved, under the rail.
        path = [(z - 0.55 + 1.1 * i / 8, 0.6 + 0.04 * math.sin(math.pi * i / 8)) for i in range(9)]
        for leaf in range(4):
            sec = section_rect(0.07, 0.012)
            sec = [(sx * 0.46 + a, b - 0.014 * leaf) for a, b in sec]
            ch.sweep([(a, b) for a, b in path[1 + leaf // 2: 8 - leaf // 2]], sec, "Iron")
        ch.box((0.09, 0.07, 0.12), (sx * 0.46, 0.585, z), "Iron")
# Prop shafts and the transfer box.
ch.box((0.4, 0.3, 0.45), (0.0, 0.6, -2.4), "Iron", bevel=0.03, segs=1)
for z0, z1, y0, y1 in ((-3.75, -2.6, 0.56, 0.6), (-2.2, 0.05, 0.6, 0.55), (0.35, 1.25, 0.55, 0.55)):
    dz, dy = z1 - z0, y1 - y0
    ln = math.hypot(dz, dy)
    ch.cyl(0.035, 0.0, ln, "Iron", segs=10, xf=T(0, y0, z0) @ RX(-math.atan2(dy, dz)))

# Engine sump and gearbox under the bonnet (seen between the wheels).
ch.box((0.55, 0.2, 1.2), (0.0, 0.8, -4.3), "Iron", bevel=0.04, segs=1)

# Fuel tanks under the doors, strapped to the frame, steps above them.
for sx in (-1, 1):
    tank = fillet([(-0.17, -0.2), (0.17, -0.2), (0.17, 0.2), (-0.17, 0.2)], 0.06, 3)
    ch.prism([(sx * 0.82 + a, 0.62 + b) for a, b in tank], -3.15, -2.15, "Paint", axis="z",
             bevel=0.02, segs=2)
    for z in (-2.95, -2.35):
        ch.prism([(sx * 0.82 + a * 1.06, 0.62 + b * 1.06) for a, b in tank], z - 0.02, z + 0.02,
                 "Iron", axis="z")
    ch.cyl(0.04, 0.0, 0.03, "Iron", segs=10, xf=T(sx * 0.82, 0.82, -2.65) @ UP)
    # Step: a perforated tread on brackets.
    ch.box((0.32, 0.03, 0.55), (sx * 1.0, 0.92, -2.62), "Iron", bevel=0.006, segs=1)
    for z in (-2.85, -2.4):
        ch.box((0.2, 0.18, 0.03), (sx * 0.95, 0.83, z), "Paint")
    # Mud flaps behind the front wheels.
    ch.box((0.38, 0.5, 0.012), (sx * 1.0, 0.6, -3.24), "Rubber")

# Spare wheel on the right, in a carrier under the bed front.
build_wheel(ch, T(0.93, 0.54, -1.18) @ S(0.95, 0.95, 0.95))
for z in (-1.62, -0.74):
    ch.box((0.08, 0.5, 0.06), (0.68, 0.8, z), "Paint")
ch.box((0.06, 0.06, 1.0), (0.68, 0.55, -1.18), "Paint")
# Toolbox and battery box on the left.
ch.box((0.4, 0.34, 0.75), (-0.88, 0.73, -1.25), "Paint", bevel=0.015, segs=1)
ch.box((0.41, 0.03, 0.76), (-0.88, 0.9, -1.25), "Paint", bevel=0.008, segs=1)
for z in (-1.45, -1.05):
    ch.box((0.02, 0.06, 0.05), (-1.085, 0.84, z), "Iron")
ch.box((0.36, 0.28, 0.4), (-0.8, 0.72, -0.62), "Iron", bevel=0.01, segs=1)
# Muffler and tailpipe, right side inboard.
ch.cyl(0.1, -1.95, -1.25, "Iron", segs=14, xf=T(0.42, 0.42, 0), chamfer=0.02)
ch.cyl(0.035, -1.25, -0.6, "Iron", segs=8, xf=T(0.42, 0.42, 0))

# Rear: cross-member with tail lamps, a tow pintle, mud flaps.
ch.box((2.26, 0.14, 0.1), (0.0, 0.92, 1.87), "Paint")
for sx in (-1, 1):
    ch.box((0.2, 0.12, 0.06), (sx * 0.92, 0.84, 1.92), "Iron", bevel=0.01, segs=1)
    ch.box((0.08, 0.06, 0.01), (sx * 0.96, 0.84, 1.952), "LampRed", bevel=0.004, segs=1)
    ch.box((0.06, 0.06, 0.01), (sx * 0.87, 0.84, 1.952), "Lamp", bevel=0.004, segs=1)
    ch.box((0.4, 0.62, 0.012), (sx * 1.0, 0.57, 2.02), "Rubber")
    ch.box((0.42, 0.04, 0.04), (sx * 1.0, 0.9, 2.02), "Paint")
# Rear steps: a hanging ladder whose treads are the step colliders' tops.
stringer = [(1.80, 1.0), (1.80, 0.745), (2.10, 0.745), (2.10, 0.375), (2.40, 0.375), (2.40, 0.30),
            (2.16, 0.30), (1.86, 0.62), (1.86, 0.96)]
for sx in (-1, 1):
    ch.prism(stringer, sx * 0.47 - 0.012, sx * 0.47 + 0.012, "Paint", axis="x")
for y, z0, z1 in ((0.74, 1.8, 2.1), (0.37, 2.1, 2.4)):
    ch.box((0.9, 0.025, z1 - z0), (0.0, y - 0.0125, (z0 + z1) / 2), "Iron", bevel=0.004, segs=1)
    ch.box((0.9, 0.04, 0.02), (0.0, y - 0.03, z1 - 0.01), "Paint")
    for k in range(5):
        ch.box((0.1, 0.004, 0.012), (-0.32 + k * 0.16, y + 0.001, (z0 + z1) / 2), "Paint")

# =============================================================================
# Cab and bonnet
# =============================================================================

cab = Part("cab")
cut = Part("cab_cut")

# Cab shell: the side profile extruded across, edges rounded.
side = [(-1.9, 1.2), (-1.9, 2.6), (-3.4, 2.6), (-3.6, 1.92), (-3.86, 1.84), (-3.86, 1.2)]
side = fillet(side, [0.03, 0.14, 0.08, 0.04, 0.04, 0.03], 4)
cab.prism(side, -1.13, 1.13, "Paint", axis="x")
# Hollow it, and open the windows (the cutter's faces become the inside).
inner = [(-1.94, 1.26), (-1.94, 2.55), (-3.37, 2.55), (-3.56, 1.92), (-3.81, 1.88), (-3.81, 1.26)]
cut.prism(fillet(inner, 0.05, 2), -1.09, 1.09, "Iron", axis="x")
# Side windows (door glass and the quarter light ahead of it).
for sx in (-1, 1):
    win = fillet([(-2.6, 1.95), (-2.6, 2.46), (-3.32, 2.46), (-3.5, 1.95)], 0.04, 2)
    cut.prism(win, sx * 1.0, sx * 1.25, "Iron", axis="x")
# Windscreen: two panes split by a pillar, leaning back with the screen.
tilt = math.atan2(0.2, 0.68)
for sx in (-1, 1):
    cut.box((0.9, 0.56, 0.3), (0.0, 0.0, 0.0), "Iron", bevel=0.03, segs=1,
            xf=T(sx * 0.52, 2.24, -3.5) @ RX(tilt))
# Rear window.
cut.box((0.7, 0.3, 0.2), (0.0, 2.3, -1.9), "Iron", bevel=0.03, segs=1)
cab_obj = make_cut(cab, cut)

cab2 = Part("cab_parts")
# Glass in the openings.
for sx in (-1, 1):
    win = fillet([(-2.6, 1.95), (-2.6, 2.46), (-3.32, 2.46), (-3.5, 1.95)], 0.04, 2)
    cab2.prism(win, sx * 1.105, sx * 1.115, "Glass", axis="x")
    cab2.box((0.9, 0.56, 0.008), (0.0, 0.0, 0.0), "Glass",
             xf=T(sx * 0.52, 2.24, -3.5) @ RX(tilt))
    # Window seals.
cab2.box((0.7, 0.3, 0.01), (0.0, 2.3, -1.915), "Glass")
# Door seams, hinges, handles; mirrors on arms.
for sx in (-1, 1):
    x = sx * 1.132
    cab2.box((0.006, 1.25, 0.008), (x, 1.845, -2.55), "Iron")
    cab2.box((0.006, 0.72, 0.008), (x, 1.58, -3.55), "Iron")
    cab2.box((0.006, 0.008, 1.0), (x, 1.225, -3.05), "Iron")
    for y in (1.4, 1.8):
        cab2.cyl(0.018, -0.05, 0.05, "Iron", segs=8, xf=T(x, y, -3.56) @ UP)
    cab2.box((0.03, 0.025, 0.14), (x + sx * 0.012, 1.85, -2.72), "Iron", bevel=0.008, segs=1)
    # Mirror arm and head.
    cab2.sweep([(-3.45, 2.2), (-3.62, 2.35), (-3.7, 2.45)],
               [(sx * 1.14 + a, b) for a, b in section_circle(0.012, 6)], "Iron")
    cab2.box((0.34, 0.012, 0.012), (sx * 1.3, 2.45, -3.7), "Iron")
    cab2.box((0.03, 0.26, 0.17), (sx * 1.46, 2.45, -3.68), "Iron", bevel=0.01, segs=1)
    cab2.box((0.004, 0.23, 0.14), (sx * 1.46, 2.45, -3.763), "Lens")
# Roof marker lamps and a whip antenna on the back corner.
for sx in (-1, 0, 1):
    cab2.box((0.07, 0.04, 0.05), (sx * 0.35, 2.62, -3.36), "Lamp", bevel=0.01, segs=1)
cab2.cyl(0.03, 0.0, 0.08, "Iron", segs=10, xf=T(-1.0, 2.6, -2.0) @ UP)
cab2.cyl(0.006, 0.0, 1.6, "Iron", segs=6, xf=T(-1.0, 2.68, -2.0) @ RX(-math.pi / 2 + 0.08))
# Interior: a bench seat, dashboard, steering wheel (left-hand drive).
cab2.box((2.0, 0.12, 0.55), (0.0, 1.62, -2.3), "Olive", bevel=0.04, segs=2)
cab2.box((2.0, 0.6, 0.1), (0.0, 1.95, -2.02), "Olive", bevel=0.04, segs=2,
         xf=RX(0.15, about=(0, 1.65, -2.02)))
cab2.box((2.1, 0.32, 0.25), (0.0, 1.72, -3.58), "Paint", bevel=0.02, segs=1)
for x in (-0.75, -0.45):
    cab2.cyl(0.045, 0.0, 0.01, "Lens", segs=12, xf=T(x, 1.78, -3.452))
wheel_at = T(-0.5, 1.95, -3.2) @ RX(-0.9)
cab2.lathe([(0.2, -0.012), (0.212, 0.0), (0.2, 0.012), (0.188, 0.0)], "Black", segs=24,
           xf=wheel_at, closed=True)
cab2.cyl(0.03, -0.35, 0.0, "Black", segs=8, xf=wheel_at)
for k in range(3):
    cab2.box((0.19, 0.02, 0.012), (0.1, 0.0, 0.0), "Black",
             xf=wheel_at @ RZ(math.pi / 2 + 2 * math.pi * k / 3))
# Wipers.
for sx in (-1, 1):
    cab2.box((0.012, 0.45, 0.012), (0.0, 0.0, 0.0), "Black",
             xf=T(sx * 0.55, 2.2, -3.535) @ RX(tilt) @ RZ(sx * 0.4))

# Bonnet: a narrow crowned hood between fenders, louvred sides.
hood = fillet([(-0.62, 0.95), (0.62, 0.95), (0.62, 1.72), (0.5, 1.8), (-0.5, 1.8),
               (-0.62, 1.72)], [0.0, 0.0, 0.08, 0.1, 0.1, 0.08], 4)
cab2.prism(hood, -4.96, -3.84, "Paint", axis="z", bevel=0.03, segs=2)
for sx in (-1, 1):
    for k in range(5):
        cab2.box((0.012, 0.22, 0.05), (sx * 0.625, 1.45, -4.6 + k * 0.13), "Iron", bevel=0.004,
                 segs=1)
    # Fenders: a pressed arch over the wheel, the front holding the headlamp.
    path = [(-4.97, 1.05), (-4.97, 1.34), (-4.93, 1.4), (-4.85, 1.42), (-4.3, 1.42)]
    path += [(-3.9 + 0.87 * math.cos(math.radians(a)), 0.55 + 0.87 * math.sin(math.radians(a)))
             for a in (90, 75, 60, 47, 35, 26)]
    # A rolled outer lip (n runs down here).
    sec = [(sx * a, b) for a, b in [(0.6, -0.02), (1.12, -0.02), (1.2, -0.005), (1.245, 0.03),
                                    (1.26, 0.08), (1.24, 0.085), (1.225, 0.04), (1.19, 0.015),
                                    (1.12, 0.01), (0.6, 0.01)]]
    cab2.sweep(path, sec, "Paint")
    # Fender side skirt down to the bumper.
    cab2.box((0.025, 0.42, 0.45), (sx * 1.245, 1.17, -4.72), "Paint")
    # Headlamp in a pod on the fender front, sidelight above.
    cab2.cyl(0.12, -0.12, 0.0, "Paint", segs=20, xf=T(sx * 0.93, 1.3, -4.93), chamfer=0.02)
    cab2.cyl(0.1, -0.13, -0.11, "Iron", segs=20, xf=T(sx * 0.93, 1.3, -4.93))
    cab2.lathe([(0.0, -0.145), (0.06, -0.142), (0.095, -0.13), (0.0, -0.13)], "Lamp", segs=20,
               xf=T(sx * 0.93, 1.3, -4.93))
    cab2.box((0.09, 0.05, 0.05), (sx * 0.93, 1.47, -4.98), "Lamp", bevel=0.012, segs=1)
# Grille: frame and vertical bars, radiator behind.
cab2.box((1.12, 0.74, 0.04), (0.0, 1.33, -4.93), "Iron")
cab2.box((1.2, 0.06, 0.06), (0.0, 1.73, -4.97), "Paint", bevel=0.01, segs=1)
cab2.box((1.2, 0.06, 0.06), (0.0, 0.93, -4.97), "Paint", bevel=0.01, segs=1)
for k in range(13):
    x = -0.54 + k * 0.09
    cab2.box((0.03, 0.76, 0.05), (x, 1.33, -4.975), "Paint", bevel=0.006, segs=1)
# Cowl vent ahead of the windscreen.
cab2.box((1.5, 0.02, 0.12), (0.0, 1.86, -3.72), "Iron", bevel=0.005, segs=1)

# =============================================================================
# Bed: plank floor, drop sides, front board, tie-down rails
# =============================================================================

bed = Part("bed")
# Floor planks along the bed, with gaps.
n = 12
w = 2.3 / n
for i in range(n):
    x = -1.15 + w * (i + 0.5)
    jitter = random.uniform(-0.003, 0.0)
    bed.box((w - 0.008, 0.04, 3.6), (x, FLOOR - 0.02 + jitter, 0.0), "Boards", bevel=0.004,
            segs=1)
# Steel angle round the floor, sub-frame and bearers under it.
for sx in (-1, 1):
    bed.box((0.04, 0.12, 3.64), (sx * 1.16, FLOOR - 0.06, 0.0), "Paint")
    bed.box((0.12, 0.14, 3.6), (sx * 0.46, 0.93, 0.0), "Paint")
for z in (-1.79, 1.8):
    bed.box((2.36, 0.12, 0.04), (0.0, FLOOR - 0.06, z), "Paint")
# (Full width only where no tyre comes up under the floor.)
for z in (-1.75, -0.55):
    bed.box((2.28, 0.07, 0.07), (0.0, 1.025, z), "Paint")
for z in (-1.15, 0.2, 0.8, 1.4):
    bed.box((1.3, 0.07, 0.07), (0.0, 1.025, z), "Paint")

# Drop sides: three boards between steel stakes, a steel top cap.
STAKES = [-1.75, -0.89, 0.0, 0.89, 1.75]
for sx in (-1, 1):
    for k in range(3):
        y = FLOOR + 0.09 + k * 0.18
        bed.box((0.04, 0.17, 3.56), (sx * 1.14, y, 0.0), "Boards", bevel=0.005, segs=1)
    bed.box((0.06, 0.03, 3.62), (sx * 1.14, FLOOR + 0.55, 0.0), "Paint", bevel=0.005, segs=1)
    for z in STAKES:
        bed.box((0.025, 0.56, 0.06), (sx * 1.172, FLOOR + 0.27, z), "Paint", bevel=0.004, segs=1)
    # Hinges at the bottom, latches at the ends, rope hooks along the top.
    for z in (-1.3, -0.45, 0.45, 1.3):
        bed.cyl(0.018, -0.07, 0.07, "Iron", segs=8, xf=T(sx * 1.175, FLOOR + 0.0, z))
    for z in (-1.32, -0.44, 0.44, 1.32):
        bed.box((0.03, 0.04, 0.02), (sx * 1.178, FLOOR + 0.42, z), "Iron")
    # Inside tie-down rail with rings.
    bed.box((0.03, 0.04, 3.4), (sx * 1.105, FLOOR + 0.48, 0.0), "Iron", bevel=0.006, segs=1)
    for z in (-1.3, -0.45, 0.45, 1.3):
        bed.lathe([(0.03, -0.006), (0.036, 0.0), (0.03, 0.006), (0.024, 0.0)], "Iron", segs=12,
                  xf=T(sx * 1.085, FLOOR + 0.43, z) @ ALONG_X, closed=True)
# Front board (behind the memorial board, inner face at z -1.78).
for k in range(3):
    y = FLOOR + 0.09 + k * 0.18
    bed.box((2.24, 0.17, 0.04), (0.0, y, -1.8), "Boards", bevel=0.005, segs=1)
bed.box((2.3, 0.03, 0.06), (0.0, FLOOR + 0.55, -1.8), "Paint", bevel=0.005, segs=1)
# Floor tie-down rings, recessed.
for x in (-0.8, 0.8):
    for z in (-1.4, 1.4):
        bed.lathe([(0.035, -0.005), (0.04, 0.0), (0.035, 0.005), (0.03, 0.0)], "Iron", segs=12,
                  xf=T(x, FLOOR + 0.004, z) @ UP, closed=True)
# A fold-up troop bench along the left side, latched up.
bed.box((0.03, 0.3, 1.7), (-1.095, FLOOR + 0.82, 0.75), "Boards", bevel=0.006, segs=1)
for z in (0.05, 0.75, 1.45):
    bed.box((0.03, 0.3, 0.03), (-1.075, FLOOR + 0.82, z), "Paint")

# =============================================================================
# Canopy: canvas over hoops, roof slats, rolled rear curtain, lamp, bell pull
# =============================================================================

cv = Part("canopy")


def canopy_profile(inset, n_shoulder=5):
    """The canopy's outline, left bottom to right bottom (x, y)."""
    xo = 1.198 - inset
    top = 3.172 - inset
    r = 0.24
    pts = [(-xo, FLOOR + 0.36)]
    pts.append((-xo, top - r))
    for k in range(1, n_shoulder):
        a = math.pi - (math.pi / 2) * k / n_shoulder
        pts.append((-xo + r + r * math.cos(a), top - r + r * math.sin(a)))
    for k in range(5):
        x = -xo + r + (2 * (xo - r)) * k / 4
        pts.append((x, top - 0.012 * (1 - (x / xo) ** 2) * 0 - 0.0))
    for k in range(n_shoulder - 1, 0, -1):
        a = (math.pi / 2) * k / n_shoulder
        pts.append((xo - r + r * math.cos(a), top - r + r * math.sin(a)))
    pts.append((xo, top - r))
    pts.append((xo, FLOOR + 0.36))
    # Dedupe the roof ends that meet the shoulders.
    out = []
    for p in pts:
        if not out or (abs(out[-1][0] - p[0]) + abs(out[-1][1] - p[1])) > 1e-4:
            out.append(p)
    return out


def normals(pts):
    """Inward unit normals of an open polyline (left to right over the top)."""
    out = []
    for i, p in enumerate(pts):
        a = pts[max(i - 1, 0)]
        b = pts[min(i + 1, len(pts) - 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(tx, ty)
        # Walking left->right over the top, inward (down/in) is the right normal.
        out.append((ty / ln, -tx / ln))
    return out


def sag(z, depth):
    """Canvas drooping between hoops."""
    for a, b in zip(HOOPS, HOOPS[1:]):
        if a <= z <= b:
            t = (z - a) / (b - a)
            return depth * math.sin(math.pi * t) ** 1.5
    return 0.0


def canvas_skin(p, z0, z1, nz=36):
    bm = p.bm
    prof = canopy_profile(0.0)
    nrm = normals(prof)
    thick = 0.012
    zs = [z0 + (z1 - z0) * i / nz for i in range(nz + 1)]
    # Snap a ring onto each hoop so the sag is exactly zero there.
    zs = sorted(set([round(z, 4) for z in zs] + [h for h in HOOPS if z0 < h < z1]))
    outer, inner = [], []
    for z in zs:
        ro, ri = [], []
        for (x, y), (nx, ny) in zip(prof, nrm):
            on_roof = ny < -0.7
            d = sag(z, 0.022 if on_roof else 0.014)
            # Low wrinkles, fixed by a hash of the point.
            d += 0.003 * math.sin(z * 9.0 + x * 5.0) * math.sin(y * 7.0)
            ox, oy = x + nx * d, y + ny * d
            ro.append(bm.verts.new((ox, oy, z)))
            ri.append(bm.verts.new((ox + nx * thick, oy + ny * thick, z)))
        outer.append(ro)
        inner.append(ri)
    faces = []
    m = len(prof)
    for i in range(len(zs) - 1):
        for k in range(m - 1):
            faces.append(bm.faces.new((outer[i][k], outer[i][k + 1], outer[i + 1][k + 1],
                                       outer[i + 1][k])))
            faces.append(bm.faces.new((inner[i][k], inner[i + 1][k], inner[i + 1][k + 1],
                                       inner[i][k + 1])))
    # Close the hems (bottom edges and ends).
    for i in range(len(zs) - 1):
        for k in (0, m - 1):
            faces.append(bm.faces.new((outer[i][k], outer[i + 1][k], inner[i + 1][k],
                                       inner[i][k])))
    for i in (0, len(zs) - 1):
        for k in range(m - 1):
            faces.append(bm.faces.new((outer[i][k], inner[i][k], inner[i][k + 1],
                                       outer[i][k + 1])))
    verts = [v for r in outer + inner for v in r]
    p._finish_piece(verts, faces, "Canvas")


canvas_skin(cv, -1.83, 1.83)
# Front wall of canvas, a flat panel inside the outline.
front = canopy_profile(0.004)
front = [(x, y) for x, y in front] + [(front[-1][0], FLOOR + 0.36)]
cv.prism(front, -1.832, -1.815, "Canvas", axis="z")
# A rope lacing hem along the bottom edge of each side.
for sx in (-1, 1):
    cv.box((0.02, 0.05, 3.66), (sx * 1.203, FLOOR + 0.38, 0.0), "Canvas", bevel=0.008, segs=1)
    for z in [-1.7 + 0.425 * k for k in range(9)]:
        cv.box((0.012, 0.09, 0.012), (sx * 1.21, FLOOR + 0.43, z), "Rope")

# Hoops: steel bows under the canvas, legs down to the bed sides.
for z in HOOPS:
    prof = canopy_profile(0.026)
    path = [(x, y) for x, y in prof]
    path[0] = (path[0][0], FLOOR + 0.5)
    path[-1] = (path[-1][0], FLOOR + 0.5)
    cv.sweep(path, [(a, b) for a, b in section_rect(0.035, 0.024, 0.006, 1)], "Paint",
             xf=T(0, 0, z) @ RY(math.pi / 2))
    # Stake pockets the hoop legs drop into.
    for sx in (-1, 1):
        cv.box((0.05, 0.12, 0.06), (sx * 1.15, FLOOR + 0.58, z), "Paint")
# Roof slats (timber) and side slats lashed to the hoops.
for x in (-0.6, 0.0, 0.6):
    cv.box((0.07, 0.025, 3.6), (x, 3.118, 0.0), "Boards", bevel=0.005, segs=1)
for sx in (-1, 1):
    cv.box((0.025, 0.07, 3.6), (sx * 1.125, 2.45, 0.0), "Boards", bevel=0.005, segs=1)

# Rear curtain rolled up under the roof edge, tied with straps.
cv.cyl(0.07, -1.15, 1.15, "Canvas", segs=14, xf=T(0, 3.05, 1.89) @ ALONG_X, chamfer=0.03)
for x in (-0.7, 0.0, 0.7):
    cv.lathe([(0.074, -0.02), (0.078, 0.0), (0.074, 0.02)], "Rope", segs=12,
             xf=T(x, 3.05, 1.89) @ ALONG_X, closed=True)
    cv.box((0.03, 0.3, 0.006), (x, 2.93, 1.962), "Rope")

# Work lamp: a caged bulb hanging from the centre slat (the light is at y 2.9).
cv.cyl(0.006, 2.98, 3.095, "Iron", segs=6, xf=UP @ T(0, 0, 0))
cv.lathe([(0.0, 2.93), (0.045, 2.95), (0.06, 2.98), (0.0, 2.985)], "Iron", segs=16,
         xf=UP @ T(0, 0, 0))
cv.sphere(0.035, (0.0, 2.905, 0.0), "Lamp", segs=12, rings=6)
for k in range(4):
    cv.box((0.004, 0.08, 0.004), (0.0, 0.0, 0.0), "Iron",
           xf=RY(math.pi / 4 + k * math.pi / 2) @ T(0.042, 2.915, 0.0))
cv.lathe([(0.042, 2.875), (0.046, 2.87), (0.042, 2.865)], "Iron", segs=12, xf=UP, closed=True)

# Bell pull: the cord runs over a pulley on the front hoop, through the
# canvas to the cab (the pull itself is at (0.7, FLOOR + 1.6, -1.7)).
cv.box((0.05, 0.16, 0.04), (0.7, 3.06, -1.74), "Iron", bevel=0.006, segs=1)
cv.cyl(0.03, -0.012, 0.012, "Iron", segs=12, xf=T(0.7, 2.97, -1.72) @ ALONG_X)
cv.box((0.006, 0.006, 0.14), (0.7, 2.997, -1.79), "Rope")
# A brass hand bell on the front wall above the board (the driver hears it).
cv.lathe([(0.0, 0.0), (0.03, 0.0), (0.045, 0.06), (0.06, 0.08), (0.0, 0.08)], "Brass",
         segs=16, xf=T(0.45, 2.9, -1.74) @ RX(math.pi / 2))
cv.box((0.015, 0.015, 0.08), (0.45, 2.91, -1.78), "Iron")

for p_ in (wheel, ch, cab2, bed, cv):
    make(p_)
# The cab shell and its fittings are one object.
bpy.ops.object.select_all(action="DESELECT")
cab_parts = bpy.data.objects["cab_parts"]
cab_obj.select_set(True)
cab_parts.select_set(True)
bpy.context.view_layer.objects.active = cab_obj
bpy.ops.object.join()
report()
export("assets/models/vehicles/truck.glb")
