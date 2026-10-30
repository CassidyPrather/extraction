"""The hero landmarks' insides: what the gatekeeper, the ropeway's engineman
and the cast house crew left. For `mill_race`, `ropeway_station` and
`blast_furnace` in src/world/buildings.rs; each prop's origin on the floor
under its centre, -Z its front (where you'd stand to work it):

- `winch`          a hand winch on cast side frames, rope on its drum, two
                   cranks, the pawl on its ratchet; 1.2 x 1.1 x 0.8
- `rope_coil`      a coil of rope on the floor, 0.7 across
- `drive_motor`    the ropeway's electric motor on its bedplate, ribbed,
                   its terminal box, shaft along X; 1.7 x 1.0 x 0.9
- `gearbox`        a reduction gearbox, input shaft along X, the output
                   up through the floor to the bull wheel; 1.0 x 1.0 x 0.9
- `brake_drum`     the brake: a drum on its bearings, the band round it, a
                   long lever with a weight; 1.8 x 1.5 x 1.0
- `control_stand`  the engineman's stand: two levers in quadrants, a dial,
                   an indicator showing where the buckets are; 0.9 x 1.4 x 0.5
- `mud_gun`        the cast house's clay gun on its swing post, aimed at
                   the taphole; 2.8 x 1.4 x 0.8
- `bar_rack`       a rack of tapping bars, rakes and a shovel on the wall;
                   2.0 x 2.2 x 0.4
- `pig_bed`        a sand bed by the runner with a cast's pigs in it;
                   3.0 x 0.2 x 2.0
- `gate_valve`     a stove's hot-blast valve: the body on its pipe stub,
                   the stem and handwheel above; 1.3 x 2.4 x 1.1
- `tool_chest`     a steel chest, lid ajar; 0.9 x 0.6 x 0.5

Materials: Cast, Iron, Steel, Rusty, Painted, Timber, Planks, Rope, Brass,
Cream, Black, Char, Concrete, Enamel.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import UP, ALONG_X, Wear, board, bolt, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, T  # noqa: E402


def wheel(p, at, r, axis_xf, mat="Iron", spokes=5, rim=0.03):
    """A handwheel: rim, spokes, hub, in the plane normal to `axis_xf`'s Z."""
    xf = T(*at) @ axis_xf
    p.lathe([(r - rim, -rim / 2), (r, -rim / 2), (r, rim / 2), (r - rim, rim / 2)], mat,
            segs=20, closed=True, xf=xf)
    p.cyl(rim * 1.4, -rim, rim, mat, segs=8, xf=xf)
    for k in range(spokes):
        a = 2 * math.pi * k / spokes
        c, s = math.cos(a), math.sin(a)
        from mathutils import Vector
        q0 = xf @ Vector((c * rim * 1.3, s * rim * 1.3, 0.0))
        q1 = xf @ Vector((c * (r - rim * 0.9), s * (r - rim * 0.9), 0.0))
        rod(p, tuple(q0), tuple(q1), rim * 0.35, mat, segs=5)


def winch():
    p = Part("static")
    w = Wear(31)
    # Side frames: A-shaped castings on a timber sole.
    for x in (-0.5, 0.5):
        p.box((0.12, 0.08, 0.8), (x, 0.04, 0.0), "Timber", bevel=0.006, segs=1)
        p.prism([(-0.36, 0.08), (0.36, 0.08), (0.08, 0.95), (-0.08, 0.95)], x - 0.03, x + 0.03,
                "Cast")
    rod(p, (-0.56, 0.08 + 0.04, -0.33), (0.56, 0.08 + 0.04, -0.33), 0.02, "Iron", segs=6)
    rod(p, (-0.56, 0.08 + 0.04, 0.33), (0.56, 0.08 + 0.04, 0.33), 0.02, "Iron", segs=6)
    # The drum on its shaft, rope wound on, the end hanging off.
    p.cyl(0.03, -0.62, 0.62, "Steel", segs=8, xf=T(0, 0.75, 0) @ ALONG_X)
    p.cyl(0.13, -0.42, 0.42, "Timber", segs=14, xf=T(0, 0.75, 0) @ ALONG_X)
    for x in (-0.44, 0.44):
        p.cyl(0.22, x - 0.02, x + 0.02, "Cast", segs=16, xf=T(0, 0.75, 0) @ ALONG_X)
    for k in range(10):
        x = -0.38 + k * 0.084
        p.lathe([(0.13, -0.035), (0.165, -0.03), (0.165, 0.03), (0.13, 0.035)], "Rope",
                segs=14, closed=True, xf=T(x, 0.75, 0) @ ALONG_X)
    rod(p, (0.1, 0.75 + 0.16, 0.0), (0.12, 1.1, -0.08), 0.016, "Rope", segs=5)
    # The cranks, a ratchet wheel and its pawl.
    for sx in (-1, 1):
        x = sx * 0.62
        p.box((0.025, 0.36, 0.04), (x, 0.75 - 0.15, 0.0), "Iron",
              xf=T(x, 0.75, 0) @ RX(0.7 * sx) @ T(-x, -0.75, 0))
        a = 0.7 * sx
        # On the arm's end (RX(a) takes the arm's -Y to -z sin(a): the
        # handles were on the shaft's other side, in the air).
        hy, hz = 0.75 - 0.3 * math.cos(a), -0.3 * math.sin(a)
        rod(p, (x, hy, hz), (x + sx * 0.18, hy, hz), 0.018, "Timber", segs=6)
    p.cyl(0.17, 0.46, 0.49, "Iron", segs=16, xf=T(0, 0.75, 0) @ ALONG_X)
    rod(p, (0.475, 0.98, -0.3), (0.475, 0.9, -0.12), 0.015, "Iron", segs=5)
    bolt(p, (0.5, 0.98, -0.3), (1, 0, 0))
    _ = w
    return [p]


def rope_coil():
    p = Part("static")
    w = Wear(32)
    for k in range(6):
        r = 0.18 + k * 0.03
        y = 0.022 + (k % 3) * 0.04
        p.lathe([(r - 0.02, y - 0.02), (r + 0.02, y - 0.02), (r + 0.02, y + 0.02),
                 (r - 0.02, y + 0.02)], "Rope", segs=18, closed=True,
                xf=RX(w.j(0.04)) @ RZ(w.j(0.04)) @ UP)
    rod(p, (0.33, 0.02, 0.0), (0.6, 0.02, 0.25), 0.018, "Rope", segs=5)
    return [p]


def drive_motor():
    p = Part("static")
    # Bedplate, the frame on its feet, cooling ribs, end shields.
    p.box((1.7, 0.1, 0.8), (0.0, 0.05, 0.0), "Cast", bevel=0.01, segs=1)
    for x in (-0.5, 0.5):
        p.box((0.2, 0.18, 0.7), (x, 0.19, 0.0), "Cast", bevel=0.01, segs=1)
    ax = T(0, 0.58, 0) @ ALONG_X
    p.cyl(0.36, -0.55, 0.55, "Painted", segs=24, xf=ax)
    for k in range(9):
        x = -0.48 + k * 0.12
        p.lathe([(0.355, -0.015), (0.4, -0.015), (0.4, 0.015), (0.355, 0.015)], "Painted",
                segs=24, closed=True, xf=T(x, 0.58, 0) @ ALONG_X)
    for x in (-0.6, 0.6):
        p.lathe([(0.0, -0.05), (0.32, -0.05), (0.32, 0.02), (0.12, 0.05), (0.0, 0.05)],
                "Cast", segs=20, xf=T(x, 0.58, 0) @ RY(math.pi / 2 * (1 if x > 0 else -1)))
    p.cyl(0.05, 0.6, 0.85, "Steel", segs=10, xf=ax)
    p.cyl(0.14, 0.72, 0.84, "Cast", segs=14, xf=ax)
    # The terminal box on top, its conduit away.
    p.box((0.3, 0.18, 0.26), (0.1, 0.98, 0.0), "Cast", bevel=0.01, segs=1)
    rod(p, (0.1, 1.07, 0.1), (0.1, 1.07, 0.45), 0.025, "Iron", segs=6)
    p.box((0.16, 0.05, 0.03), (0.1, 0.62, -0.395), "Brass")
    return [p]


def gearbox():
    p = Part("static")
    p.box((1.0, 0.1, 0.85), (0.0, 0.05, 0.0), "Cast", bevel=0.01, segs=1)
    p.box((0.9, 0.6, 0.7), (0.0, 0.4, 0.0), "Painted", bevel=0.03, segs=2)
    p.box((0.94, 0.05, 0.74), (0.0, 0.72, 0.0), "Cast", bevel=0.01, segs=1)
    for x in (-0.3, 0.0, 0.3):
        for z in (-0.33, 0.33):
            bolt(p, (x, 0.745, z), (0, 1, 0), r=0.015, h=0.015)
    p.cyl(0.05, 0.45, 0.75, "Steel", segs=10, xf=T(0, 0.4, 0) @ ALONG_X)
    p.cyl(0.12, 0.6, 0.66, "Cast", segs=14, xf=T(0, 0.4, 0) @ ALONG_X)
    # The output shaft up, its coupling.
    p.cyl(0.08, 0.745, 1.0, "Steel", segs=12, xf=UP)
    p.cyl(0.16, 0.85, 0.95, "Cast", segs=14, xf=UP)
    p.box((0.12, 0.2, 0.04), (-0.3, 0.45, -0.37), "Glass")
    return [p]


def brake_drum():
    p = Part("static")
    # Two pedestals, the drum on its shaft between them.
    for x in (-0.65, 0.65):
        p.box((0.25, 0.75, 0.5), (x, 0.375, 0.0), "Cast", bevel=0.015, segs=1)
        p.cyl(0.11, x - 0.13, x + 0.13, "Cast", segs=12, xf=T(0, 0.78, 0) @ ALONG_X)
    p.cyl(0.05, -0.85, 0.85, "Steel", segs=10, xf=T(0, 0.78, 0) @ ALONG_X)
    p.cyl(0.58, -0.35, 0.35, "Cast", segs=28, xf=T(0, 0.78, 0) @ ALONG_X)
    for x in (-0.37, 0.37):
        p.cyl(0.6, x - 0.02, x + 0.02, "Iron", segs=28, xf=T(0, 0.78, 0) @ ALONG_X)
    # The band: a strap round most of the drum, lined.
    p.lathe([(0.6, -0.3), (0.63, -0.3), (0.63, 0.3), (0.6, 0.3)], "Rusty", segs=24,
            closed=True, sweep=300.0, start=-60.0, xf=T(0, 0.78, 0) @ RY(math.pi / 2))
    # The lever: pivoted at the floor in front, a counterweight at its end.
    rod(p, (0.0, 0.12, -0.7), (0.0, 1.45, -0.55), 0.035, "Iron", segs=8)
    p.box((0.18, 0.18, 0.28), (0.0, 0.09, -0.7), "Cast", bevel=0.01, segs=1)
    rod(p, (0.0, 0.75, -0.62), (0.0, 0.2, -0.6), 0.02, "Iron", segs=6)
    p.box((0.22, 0.26, 0.22), (0.0, 1.32, -0.53), "Cast", bevel=0.015, segs=1)
    p.box((0.05, 0.05, 0.14), (0.0, 1.47, -0.56), "Timber")
    return [p]


def control_stand():
    p = Part("static")
    p.box((0.8, 0.9, 0.4), (0.0, 0.45, 0.05), "Painted", bevel=0.015, segs=1)
    p.box((0.86, 0.05, 0.46), (0.0, 0.925, 0.05), "Cast", bevel=0.008, segs=1)
    # Two levers in their quadrants, one pulled.
    for x, a in ((-0.2, 0.35), (0.2, -0.3)):
        p.lathe([(0.2, -0.015), (0.24, -0.015), (0.24, 0.015), (0.2, 0.015)], "Cast", segs=12,
                closed=True, sweep=120.0, start=30.0, xf=T(x, 0.95, 0.05) @ RY(math.pi / 2))
        top = (x, 0.95 + 0.55 * math.cos(a), 0.05 - 0.55 * math.sin(a))
        rod(p, (x, 0.95, 0.05), top, 0.018, "Steel", segs=6)
        p.sphere(0.035, top, "Black", segs=8, rings=5)
    # The dial and the bucket indicator on a board behind.
    p.box((0.8, 0.45, 0.04), (0.0, 1.2, 0.27), "Timber", bevel=0.006, segs=1)
    p.cyl(0.11, 0.0, 0.04, "Brass", segs=16, xf=T(-0.2, 1.22, 0.25) @ RY(math.pi))
    p.cyl(0.095, 0.04, 0.045, "Cream", segs=16, xf=T(-0.2, 1.22, 0.25) @ RY(math.pi))
    p.box((0.01, 0.07, 0.004), (-0.2, 1.25, 0.2), "Black",
          xf=T(-0.2, 1.22, 0.2) @ RZ(0.6) @ T(0.2, -1.22, -0.2))
    p.box((0.3, 0.06, 0.01), (0.17, 1.3, 0.247), "Cream")
    p.box((0.02, 0.05, 0.012), (0.1, 1.3, 0.24), "Red")
    for k in range(4):
        p.box((0.04, 0.04, 0.01), (0.06 + k * 0.08, 1.1, 0.247), "Brass")
    return [p]


def mud_gun():
    p = Part("static")
    # The swing post on its base, the arm, the gun's barrel aimed at -Z
    # and down a little, the clay hopper on top, a ram cylinder behind.
    p.box((0.7, 0.2, 0.7), (0.0, 0.1, 0.9), "Cast", bevel=0.015, segs=1)
    p.cyl(0.14, 0.2, 1.3, "Rusty", segs=14, xf=T(0, 0, 0.9) @ UP)
    p.box((0.3, 0.25, 1.0), (0.0, 1.25, 0.45), "Rusty", bevel=0.01, segs=1)
    tilt = RX(-0.12)
    xf = T(0.0, 1.0, 0.0) @ tilt
    p.cyl(0.16, -1.3, 0.6, "Cast", segs=16, xf=xf)
    p.lathe([(0.0, -1.5), (0.06, -1.5), (0.16, -1.3), (0.0, -1.3)], "Iron", segs=16, xf=xf)
    p.cyl(0.12, 0.6, 1.4, "Steel", segs=12, xf=xf)
    p.box((0.3, 0.3, 0.35), (0.0, 0.3, -0.4), "Rusty", xf=xf)
    for z in (-0.9, -0.2, 0.4):
        p.lathe([(0.155, -0.03), (0.19, -0.03), (0.19, 0.03), (0.155, 0.03)], "Iron", segs=16,
                closed=True, xf=xf @ T(0, 0, z))
    # Clay caked on the nozzle.
    p.sphere(0.12, (0.0, 1.17, -1.45), "Char", segs=10, rings=6, scale=(1.2, 0.8, 1.0))
    return [p]


def bar_rack():
    p = Part("static")
    w = Wear(33)
    # Two uprights and rails on the wall (+Z face at z 0.2), bars leaning
    # in them.
    for x in (-0.9, 0.9):
        p.box((0.08, 2.0, 0.08), (x, 1.0, 0.16), "Rusty")
    for y in (0.4, 1.6):
        p.box((1.9, 0.06, 0.06), (0.0, y, 0.14), "Rusty")
    for k in range(9):
        x = -0.75 + k * 0.18 + w.j(0.02)
        lean = 0.12 + w.j(0.04)
        top = (x + w.j(0.03), 2.15 + w.j(0.05), 0.08)
        rod(p, (x, 0.0, 0.08 - lean), top, 0.017 if k % 3 else 0.024, "Iron", segs=6)
        if k % 4 == 1:
            # A rake's head at the top.
            p.box((0.2, 0.03, 0.12), (top[0], top[1] + 0.02, top[2] - 0.04), "Iron")
    # A shovel, a sledge leaning at the end.
    rod(p, (0.95, 0.0, -0.1), (1.0, 1.3, 0.15), 0.02, "Timber", segs=6)
    p.box((0.24, 0.3, 0.02), (0.95, 0.15, -0.12), "Rusty",
          xf=T(0.95, 0.15, -0.12) @ RX(0.2) @ T(-0.95, -0.15, 0.12))
    return [p]


def pig_bed():
    p = Part("static")
    w = Wear(34)
    # The sand bed, its pigs in rows off a sow channel.
    p.box((3.0, 0.12, 2.0), (0.0, 0.06, 0.0), "Pebble")
    p.box((3.0, 0.03, 0.18), (0.0, 0.135, -0.88), "Cast")
    for r in range(4):
        z = -0.6 + r * 0.42
        for k in range(10):
            x = -1.35 + k * 0.3
            h = 0.035 + w.j(0.008)
            p.box((0.22, h, 0.36), (x, 0.12 + h / 2, z), "Cast", bevel=0.01, segs=1)
    return [p]


def gate_valve():
    p = Part("static")
    # The pipe stub along X through the valve body, flanges, a bonnet up,
    # the stem and its handwheel.
    ax = T(0, 0.75, 0) @ ALONG_X
    p.cyl(0.32, -0.65, 0.65, "Rusty", segs=20, xf=ax)
    for x in (-0.62, -0.3, 0.3, 0.62):
        p.cyl(0.42, x - 0.03, x + 0.03, "Cast", segs=20, xf=ax)
        for k in range(8):
            a = 2 * math.pi * (k + 0.5) / 8
            bolt(p, (x + (0.035 if x > 0 else -0.035), 0.75 + 0.37 * math.sin(a),
                     0.37 * math.cos(a)), (1 if x > 0 else -1, 0, 0), r=0.018, h=0.02)
    p.box((0.5, 0.6, 0.75), (0.0, 0.75, 0.0), "Cast", bevel=0.04, segs=2)
    p.box((0.34, 0.7, 0.4), (0.0, 1.35, 0.0), "Cast", bevel=0.03, segs=2)
    p.cyl(0.035, 1.7, 2.35, "Steel", segs=10, xf=UP)
    for x in (-0.12, 0.12):
        rod(p, (x, 1.7, 0.0), (x, 2.05, 0.0), 0.025, "Cast", segs=6)
    p.box((0.34, 0.05, 0.12), (0.0, 2.05, 0.0), "Cast")
    wheel(p, (0.0, 2.25, 0.0), 0.32, UP @ RX(0.0), "Iron", spokes=5)
    # A chalked tag on the body.
    p.box((0.14, 0.1, 0.004), (0.0, 0.8, -0.377), "Cream")
    # Feet.
    for x in (-0.45, 0.45):
        p.box((0.2, 0.43, 0.5), (x, 0.215, 0.0), "Concrete", bevel=0.02, segs=1)
    return [p]


def tool_chest():
    p = Part("static")
    p.box((0.9, 0.46, 0.5), (0.0, 0.27, 0.0), "Painted", bevel=0.01, segs=1)
    p.box((0.92, 0.04, 0.52), (0.0, 0.02, 0.0), "Iron")
    # The lid, hinged at the back, ajar.
    hinge = (0.0, 0.5, 0.25)
    lid = T(*hinge) @ RX(-0.35) @ T(*(-v for v in hinge))
    p.box((0.92, 0.05, 0.52), (0.0, 0.525, 0.0), "Painted", bevel=0.01, segs=1, xf=lid)
    for x in (-0.35, 0.35):
        p.box((0.12, 0.03, 0.03), (x, 0.38, -0.265), "Iron")
    p.box((0.1, 0.08, 0.02), (0.0, 0.46, -0.26), "Brass")
    return [p]


prop("winch", winch)
prop("rope_coil", rope_coil)
prop("drive_motor", drive_motor)
prop("gearbox", gearbox)
prop("brake_drum", brake_drum)
prop("control_stand", control_stand)
prop("mud_gun", mud_gun)
prop("bar_rack", bar_rack)
prop("pig_bed", pig_bed)
prop("gate_valve", gate_valve)
prop("tool_chest", tool_chest)
