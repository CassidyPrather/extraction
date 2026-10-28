"""A home-made crossbow from the zone: the kind a poacher builds in a
garage. Writes assets/models/guns/crossbow.glb.

Gun space: origin at the palm on the grip, -Z forward along the shot, +Y
up, +X right, metres. The bolt's axis runs along the top of the rail at
y = 0.06.

The prod is a cut-down car leaf spring, three leaves stacked on its belly
side, clamped between cheek blocks on the stock and a nose block by two
U-bolts; hose clamps hold the leaves together where the shortest ends,
the main leaf's tips are bent forward and drilled for the cord string.
The stock is a plank sawn to a rifle butt and a long tiller; the rail is
two steel angles screwed along its top with a gap between them (the
flight groove, cut down into the wood for the cock vane). The latch is a
gate latch: two levers on a pin whose hooks drop in front of the drawn
string, lifted by a pushrod from the bent strap trigger. A door bolt on
the right is the safety. A rebar stirrup at the front takes the foot.

Objects: `static` (all of it that never moves), `bolt_string` (the
string's served middle and its leather cradle, sliding back with the
draw; origin where it crosses the rail at rest), `string_rest` (the two
string halves at rest), `string_drawn` (the V drawn to the latch; its
origin on the line through the string's tip anchors, so the game can
scale it along Z for a partial draw), `trigger` (origin on its pin), `safety_bolt` (the door bolt, drawn back
+Z to fire) and
`static_rail_bolt` (a bolt lying in the groove, nocked on the latched
string, shown while loaded).
"""

import math
import os
import sys

from mathutils import Matrix, Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_circle, section_rect)
from _kit import ring  # noqa: E402
from crossbow_bolts import HALF, NOCK_DEPTH, add_bolt, tube3d  # noqa: E402

main_guard()

AXIS = 0.06            # the bolt's axis
TOP = 0.054            # the tiller's top, under the rail
RAIL = 0.058           # the rail's top
GAP = 0.0035           # half the gap between the angles (the groove)
TW = 0.0195            # half the plank's thickness
REST_Z, LATCH_Z = -0.33, -0.07
STRING_Y = 0.0605      # the string's middle where it rides over the rail
MUZZLE = -0.46


def frame(origin, xa, ya, za):
    """A matrix taking local x, y, z to the given axes at `origin`."""
    m = Matrix.Identity(4)
    for i, a in enumerate((xa, ya, za)):
        for r in range(3):
            m[r][i] = a[r]
    for r in range(3):
        m[r][3] = origin[r]
    return m


def screw(p, cutter, r, pos, xf, turn=0.6, mat="Steel", segs=10):
    """A domed slotted screw head of radius r at `pos`, its axis +Y turned
    by `xf`, its rim sunk 0.6 mm; `cutter` gets the slot."""
    h = r * 0.55
    prof = [(0.0, -0.0006), (r, -0.0006)]
    prof += [(r * math.cos(math.radians(d)), h * math.sin(math.radians(d))) for d in (0, 35, 65)]
    prof.append((0.0, h))
    p.lathe(prof, mat, segs=segs, xf=T(*pos) @ xf @ RX(-math.pi / 2))
    cutter.box((r * 2.4, h * 1.4, r * 0.32), (0.0, h, 0.0), mat, xf=T(*pos) @ xf @ RY(turn))


PX, NX = RZ(-math.pi / 2), RZ(math.pi / 2)    # a screw facing +X, -X
DOWN, BACK = RX(math.pi), RX(math.pi / 2)      # facing -Y, +Z


def catmull3(pts, samples=4):
    """An open Catmull-Rom spline through 3D points."""
    pts = [Vector(p) for p in pts]
    n = len(pts)
    out = []
    for i in range(n - 1):
        p0, p1, p2 = pts[max(i - 1, 0)], pts[i], pts[i + 1]
        p3 = pts[min(i + 2, n - 1)]
        for s in range(samples):
            t = s / samples
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t
                              + (-p0 + 3 * p1 - 3 * p2 + p3) * t ** 3))
    out.append(pts[-1])
    return out


# --- The prod: a car leaf spring ------------------------------------------------------------
LT = 0.007             # leaf thickness
PY = 0.028             # the prod's middle, heightwise
LEAVES = [  # (half length, u0, u1 along the belly normal, half width)
    (0.306, -LT / 2, LT / 2, 0.022),
    (0.200, LT / 2, 1.5 * LT, 0.021),
    (0.120, 1.5 * LT, 2.5 * LT, 0.020),
]
CLAMP_X, CLAMP_W = 0.165, 0.011
HOLE_X = 0.295


def zc(x):
    """The main leaf's mid-line (z at x): flat where it's clamped, curving
    back to the tips, which are bent forward again."""
    a = abs(x)
    if a <= 0.05:
        return -0.407
    if a <= 0.27:
        return -0.407 + 0.066 * ((a - 0.05) / 0.22) ** 2
    t = (a - 0.27) / 0.036
    h = 0.036
    return ((2 * t ** 3 - 3 * t ** 2 + 1) * -0.341 + (t ** 3 - 2 * t ** 2 + t) * h * 0.6
            + (-2 * t ** 3 + 3 * t ** 2) * -0.3392 + (t ** 3 - t ** 2) * h * -0.25)


def leaf_frame(x):
    """Point on the main leaf's mid-line, its tangent (toward +x) and its
    belly normal (toward the shooter)."""
    e = 1e-4
    dz = (zc(x + e) - zc(x - e)) / (2 * e)
    t = Vector((1.0, 0.0, dz)).normalized()
    n = Vector((-dz, 0.0, 1.0)).normalized()
    return Vector((x, PY, zc(x))), t, n


def section(x, u0, u1, hw, r=0.0012):
    p, t, n = leaf_frame(x)
    y = Vector((0, 1, 0))
    prof = fillet([(u0, -hw), (u1, -hw), (u1, hw), (u0, hw)], r, 2)
    return [p + n * u + y * v for u, v in prof]


def stations(a, b):
    xs, x = [a], a
    while x < b - 1e-6:
        x = min(b, x + (0.004 if abs(x) > 0.25 or abs(x) > CLAMP_X - 0.02 and abs(x) < CLAMP_X
                        + 0.02 else 0.009))
        xs.append(x)
    return xs


prod = Part("prod")
for i, (L, u0, u1, hw) in enumerate(LEAVES):
    # Split round the hose clamps (the clamp replaces the leaf under it).
    cuts = [-L]
    if L > CLAMP_X:
        cuts += [-CLAMP_X - CLAMP_W / 2, -CLAMP_X + CLAMP_W / 2, CLAMP_X - CLAMP_W / 2,
                 CLAMP_X + CLAMP_W / 2]
    cuts.append(L)
    for a, b in zip(cuts[::2], cuts[1::2]):
        rings = []
        for x in stations(a, b):
            # The shorter leaves taper thinner toward their ends.
            uu = u1
            if i > 0:
                k = max(0.0, 1.0 - (L - abs(x)) / 0.03)
                uu = u1 - 0.0025 * k * k
            rings.append(section(x, u0, uu, hw))
        prod.loft(rings, "Iron")
# The string holes drilled through the main leaf's tips.
holes = Part("cut")
for sx in (-1, 1):
    p, t, n = leaf_frame(sx * HOLE_X)
    holes.cyl(0.0032, -0.01, 0.01, "Iron", segs=12,
              xf=T(*p) @ RY(math.atan2(n.x, n.z)))
prod.cut(holes)

kit = Part("kit")       # small steel bits, no cuts
screws = Part("screws")
slots = Part("cut")

# Hose clamps holding the two long leaves together.
for sx in (-1, 1):
    xs = [sx * CLAMP_X - CLAMP_W / 2, sx * CLAMP_X, sx * CLAMP_X + CLAMP_W / 2]
    kit.loft([section(x, -LT / 2 - 0.0012, 1.5 * LT + 0.0012, 0.022 + 0.0012, 0.002) for x in xs],
             "Steel")
    p, t, n = leaf_frame(sx * CLAMP_X)
    y = Vector((0, 1, 0))
    c = p + n * (1.5 * LT + 0.0012 + 0.003) + y * 0.004
    m = frame(c, t, y, n)
    kit.box((0.013, 0.012, 0.006), (0, 0, 0), "Steel", bevel=0.001, xf=m)  # the worm housing
    kit.cyl(0.0032, 0.006, 0.0105, "Steel", segs=6, xf=m @ RX(-math.pi / 2), chamfer=0.0006)

# --- String anchors -------------------------------------------------------------------------
SR = 0.0015
ANCHORS = []
for sx in (-1, 1):
    p, t, n = leaf_frame(sx * HOLE_X)
    ANCHORS.append(p + n * (LT / 2 + SR))
    # The cord's loop through the hole and round the tip's end.
    tout = t * sx
    ring(kit, 0.0068, SR, "Rope", xf=T(*(p + tout * 0.0068)) @ RX(-math.pi / 2), segs=14,
         sides=6)
TIP_Y, TIP_Z = ANCHORS[1].y, ANCHORS[1].z

# --- Stock ------------------------------------------------------------------------------------
stock_prof = fillet([
    (-0.3892, TOP), (-0.012, TOP),             # tiller top, under the rail
    (0.035, 0.028), (0.12, 0.036),             # dip to the wrist, comb
    (0.358, 0.042), (0.358, -0.112),           # heel, butt end, toe
    (0.22, -0.090), (0.095, -0.075),           # belly
    (0.070, -0.080), (0.048, -0.071),          # under the grip
    (0.022, -0.042), (-0.004, -0.013),         # grip front
    (-0.03, -0.008), (-0.13, -0.008),          # under the trigger
    (-0.17, -0.004), (-0.3892, -0.004),        # tiller's underside
], [0.005, 0.008, 0.03, 0.04, 0.006, 0.008, 0.06, 0.012, 0.008, 0.01, 0.02, 0.012, 0.01, 0.02,
    0.02, 0.005], 4)
stock = Part("stock")
stock.prism(stock_prof, -TW, TW, "Wood", axis="x", bevel=0.004, segs=2)
wood_cut = Part("cut")
# The flight groove sawn down into the wood under the rail's gap.
wood_cut.box((0.009, 0.02, 0.45), (0.0, 0.054, -0.235), "Wood")
stock.cut(wood_cut)

# Cheek blocks either side of the tiller's end, behind the prod.
cheek = fillet([(-0.390, -0.010), (-0.350, -0.010), (-0.350, TOP), (-0.390, TOP)],
               [0.005, 0.005, 0.0, 0.0], 3)
nose = Part("nose")
for sx in (-1, 1):
    a, b = sorted((sx * TW, sx * 0.046))
    nose.prism(cheek, a, b, "Wood", axis="x", bevel=0.003, segs=2)
# The nose block in front of the prod, under the rail.
nose.prism(fillet([(-0.4100, TOP), (-0.458, TOP), (-0.458, 0.024), (-0.449, 0.012),
                   (-0.4100, 0.010)], [0.0, 0.004, 0.006, 0.01, 0.004], 3),
           -TW, TW, "Wood", axis="x", bevel=0.003, segs=2)
nose_cut = Part("cut")
nose_cut.box((0.009, 0.02, 0.06), (0.0, 0.054, -0.435), "Wood")
nose.cut(nose_cut)

# The front grip: a block screwed under the tiller, wrapped in blue tape.
grip = Part("grip")
grip.prism([(-0.320, -0.004), (-0.240, -0.004), (-0.240, -0.024), (-0.320, -0.024)],
           -0.0155, 0.0155, "Wood", axis="x", bevel=0.002, segs=2)
grip.prism([(-0.3215, -0.024), (-0.2385, -0.024), (-0.2385, -0.050), (-0.3215, -0.050)],
           -0.017, 0.017, "Blue", axis="x", bevel=0.001, segs=1)
grip.prism(fillet([(-0.320, -0.050), (-0.240, -0.050), (-0.243, -0.075), (-0.255, -0.086),
                   (-0.305, -0.086), (-0.317, -0.075)], [0.0, 0.0, 0.006, 0.01, 0.01, 0.006], 3),
           -0.0155, 0.0155, "Wood", axis="x", bevel=0.003, segs=2)
# The tape's loose end, folded flat on the right.
grip.prism([(-0.288, -0.026), (-0.276, -0.026), (-0.272, -0.034), (-0.282, -0.040)],
           0.017, 0.0175, "Blue", axis="x")
for z in (-0.300, -0.260):
    screw(screws, slots, 0.0035, (0.0, -0.086, z), DOWN, turn=z * 20)

# Rubber butt pad, cut from a tyre.
pad = Part("pad")
pad.prism(fillet([(0.358, 0.042), (0.368, 0.042), (0.368, -0.112), (0.358, -0.112)], 0.003, 3),
          -0.0215, 0.0215, "Rubber", axis="x", bevel=0.0025, segs=2)
for y in (0.022, -0.092):
    screw(screws, slots, 0.0038, (0.0, y, 0.368), BACK, turn=y * 30)

# Wire round a crack in the butt: four turns and a twisted tail.
WIRE = 0.0011


def stock_y(z, top):
    ys = []
    for (za, ya), (zb, yb) in zip(stock_prof, stock_prof[1:] + stock_prof[:1]):
        if (za - z) * (zb - z) <= 0 and za != zb:
            ys.append(ya + (z - za) / (zb - za) * (yb - ya))
    return max(ys) if top else min(ys)


wpts = []
for k in range(4 * 24 + 1):
    s = k / 24
    z = 0.150 + 0.0042 * s
    yt, yb = stock_y(z, True) + WIRE, stock_y(z, False) - WIRE
    a = 2 * math.pi * s
    # A rounded rectangle round the plank's section, parametrised by angle.
    cx, cy = math.cos(a), math.sin(a)
    px = math.copysign(min(1.0, abs(cx) * 1.6), cx) * (TW + WIRE)
    py = (yt + yb) / 2 + math.copysign(min(1.0, abs(cy) * 1.6), cy) * (yt - yb) / 2
    wpts.append((px, py, z))
tube3d(kit, wpts, WIRE, "Steel", sides=8, phase=math.pi / 16)
end = Vector(wpts[-1])
for s in (0, 1):
    tube3d(kit, [(end.x + 0.0012 * math.cos(s * math.pi + i * 1.2) + 0.0015 * i,
                  end.y + 0.0012 * math.sin(s * math.pi + i * 1.2) - 0.0008 * i, end.z)
                 for i in range(7)], WIRE * 0.9, "Steel", sides=6, phase=0.2)

# --- The rail: two steel angles with a gap between them -----------------------------------
rail = Part("rail")
ang = [(GAP, RAIL), (0.0235, RAIL), (0.0235, 0.034), (TW, 0.034), (TW, TOP), (GAP, TOP)]
ang = fillet(ang, [0.0008, 0.002, 0.0008, 0.0008, 0.001, 0.0008], 2)
for sx in (-1, 1):
    rail.prism([(sx * a, b) for a, b in ang], MUZZLE, -0.02, "Steel", axis="z", bevel=0.0006,
               segs=1)
rcut = Part("cut")
# The angles' sides cut away over the prod and the cheek blocks.
rcut.box((0.06, 0.0211, 0.0604), (0.0, 0.0435, -0.380), "Steel")
rail.cut(rcut)
# Screws along both angles' sides.
for z in (-0.44, -0.30, -0.20, -0.15, -0.04):
    screw(screws, slots, 0.0032, (0.0235, 0.043, z), PX, turn=z * 13)
    screw(screws, slots, 0.0032, (-0.0235, 0.043, z), NX, turn=z * 17)

# The U-bolts round the prod, through the cheek blocks, nutted behind.
for sx in (-1, 1):
    x = sx * 0.033
    kit.sweep(fillet([(-0.3415, 0.0545), (-0.4150, 0.0545), (-0.4150, 0.0015), (-0.3415, 0.0015)],
                     [0.0, 0.008, 0.008, 0.0], 4, closed=False),
              section_circle(0.0042, 8), "Steel", xf=T(x, 0, 0))
    for y in (0.0545, 0.0015):
        kit.cyl(0.0075, -0.350, -0.3445, "Iron", segs=6, chamfer=0.0008,
                xf=T(x, y, 0) @ RZ(math.pi / 6))
    # A carriage bolt through the cheeks and the tiller.
    kit.sphere(0.0065, (sx * 0.046, 0.026, -0.370), "Iron", segs=14, rings=7,
               scale=(0.35, 1, 1))

# The rebar stirrup for the foot, its ends flattened to eyes bolted to the
# nose block's sides.
stir = []
for sx in (1, -1):
    half = [(sx * 0.0255, 0.022, -0.4495), (sx * 0.028, 0.019, -0.462), (sx * 0.043, 0.006, -0.478),
            (sx * 0.058, -0.018, -0.492), (sx * 0.062, -0.048, -0.500), (sx * 0.050, -0.066, -0.505),
            (sx * 0.022, -0.073, -0.507)]
    stir.append(half)
spath = stir[0] + [(0.0, -0.0735, -0.5075)] + list(reversed(stir[1]))
spts = catmull3(spath, 4)
tube3d(kit, spts, 0.0052, "Iron", sides=10, phase=0.1)
# Its ribs.
acc = 0.0
for a, b in zip(spts, spts[1:]):
    d = (b - a)
    seg = d.length
    while acc < seg:
        q = a + d * (acc / seg)
        rot = Vector((0, 0, 1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
        ring(kit, 0.0052, 0.0007, "Iron", xf=T(*q) @ rot @ RX(0.3), segs=10, sides=4)
        acc += 0.009
    acc -= seg
for sx in (-1, 1):
    ring(kit, 0.0072, 0.0028, "Iron", xf=T(sx * 0.0225, 0.022, -0.442) @ RY(math.pi / 2),
         segs=14, sides=6)
    kit.sphere(0.0055, (sx * 0.0262, 0.022, -0.442), "Iron", segs=12, rings=6, scale=(0.4, 1, 1))

# The front sight: a wire hoop over the rail's end with a bead.
kit.sweep(fillet([(-0.019, RAIL - 0.001), (-0.019, 0.089), (0.019, 0.089), (0.019, RAIL - 0.001)],
                 [0, 0.008, 0.008, 0], 4, closed=False), section_circle(0.0015, 6), "Steel",
          xf=T(0, 0, -0.45) @ RY(math.pi / 2))
for sx in (-1, 1):
    kit.sphere(0.003, (sx * 0.019, RAIL, -0.45), "Steel", segs=10, rings=5, scale=(1, 0.6, 1))
kit.sphere(0.0025, (0.0, 0.0925, -0.45), "Worn", segs=10, rings=6)

# --- The latch: a gate latch on a plate at the back of the rail ------------------------------
latch = Part("latch")
latch.box((0.043, 0.004, 0.040), (0.0, RAIL + 0.002, -0.045), "Iron", bevel=0.0008, segs=1)
for x in (-0.0165, 0.0165):
    for z in (-0.060, -0.030):
        screw(screws, slots, 0.0024, (x, RAIL + 0.004, z), T(), turn=x * 40 + z * 9)
lever = fillet([
    (-0.0755, 0.0587), (-0.0722, 0.0587), (-0.0722, 0.0618),   # the hook in front of the string
    (-0.0690, 0.0635), (-0.0660, 0.0640),                      # the notch's roof
    (-0.050, 0.0640), (-0.034, 0.0640), (-0.032, 0.0660),      # under the pivot to the tail
    (-0.032, 0.0730), (-0.050, 0.0770), (-0.068, 0.0740), (-0.0755, 0.0670),
], [0.0005, 0.0005, 0.0, 0.0012, 0.0, 0.0, 0.0, 0.001, 0.001, 0.004, 0.003, 0.0012], 2)
for sx in (-1, 1):
    a, b = sorted((sx * 0.0045, sx * 0.0085))
    latch.prism(lever, a, b, "Worn", axis="x", bevel=0.0004, segs=1)
    a, b = sorted((sx * 0.0105, sx * 0.0135))
    latch.prism([(-0.056, RAIL + 0.004), (-0.044, RAIL + 0.004), (-0.044, 0.073), (-0.050, 0.0765),
                 (-0.056, 0.073)], a, b, "Iron", axis="x", bevel=0.0005, segs=1)
latch.cyl(0.0018, -0.0148, 0.0148, "Steel", segs=10, chamfer=0.0004,
          xf=T(0, 0.0705, -0.050) @ RY(math.pi / 2))
latch.box((0.017, 0.006, 0.004), (0.0, 0.070, -0.0345), "Worn", bevel=0.0006, segs=1)
latch.cyl(0.0016, 0.050, 0.067, "Steel", segs=8, xf=T(0, 0, -0.0345) @ RX(-math.pi / 2))  # pushrod
# The rear sight: a notched plate behind the latch.
latch.prism([(-0.012, RAIL + 0.004), (0.012, RAIL + 0.004), (0.012, 0.0875), (0.003, 0.0875),
             (0.0, 0.084), (-0.003, 0.0875), (-0.012, 0.0875)], -0.029, -0.025, "Iron", axis="z",
            bevel=0.0005, segs=1)
# The door bolt on the right: the safety.
latch.prism(fillet([(-0.130, 0.035), (-0.085, 0.035), (-0.085, 0.051), (-0.130, 0.051)], 0.002, 2),
            0.0235, 0.0275, "Iron", axis="x", bevel=0.0007, segs=1)
for z0 in (-0.128, -0.095):
    latch.tube(0.0045, 0.0033, z0, z0 + 0.008, "Iron", segs=12, xf=T(0.032, 0.043, 0))
# The bolt itself (`safety_bolt`): shot forward under the latch's levers
# (safe), drawn back to fire.
safety_bolt = Part("safety_bolt", origin=(0.032, 0.043, -0.108))
safety_bolt.cyl(0.0032, -0.138, -0.080, "Worn", segs=10, chamfer=0.0006, xf=T(0.032, 0.043, 0))
safety_bolt.cyl(0.0022, 0.0, 0.016, "Worn", segs=8, xf=T(0.032, 0.043, -0.108) @ RY(math.pi / 2))
safety_bolt.sphere(0.004, (0.049, 0.043, -0.108), "Worn", segs=12, rings=6)

# --- Trigger and guard -------------------------------------------------------------------------
kit.box((0.016, 0.004, 0.028), (0.0, -0.010, -0.048), "Worn", bevel=0.0008, segs=1)
for sx in (-1, 1):
    kit.sphere(0.0035, (sx * TW, 0.002, -0.048), "Steel", segs=10, rings=5, scale=(0.4, 1, 1))
guard = catmull([(-0.112, -0.00975), (-0.096, -0.00975), (-0.088, -0.020), (-0.082, -0.040),
                 (-0.065, -0.050), (-0.035, -0.051), (-0.012, -0.044), (0.002, -0.030),
                 (0.010, -0.022)], 4, closed=False)
kit.sweep(guard, section_rect(0.012, 0.0035, 0.001), "Iron")
screw(screws, slots, 0.0028, (0.0, -0.0115, -0.106), DOWN, turn=0.7)

trig = Part("trigger", origin=(0.0, 0.002, -0.048))
trig.sweep(catmull([(-0.048, 0.004), (-0.0485, -0.012), (-0.0465, -0.026), (-0.041, -0.035),
                    (-0.035, -0.038)], 4, closed=False), section_rect(0.006, 0.0032, 0.001),
           "Iron")

# --- Sling eyes ----------------------------------------------------------------------------------
for z, y in ((-0.37, -0.004), (0.25, stock_y(0.25, False))):
    ring(kit, 0.0065, 0.0018, "Steel", xf=T(0, y - 0.0085, z) @ RY(math.pi / 2), segs=14,
         sides=6)
    kit.cyl(0.0019, y - 0.003, y + 0.006, "Steel", segs=8, xf=T(0, 0, z) @ RX(-math.pi / 2))

# --- The string ----------------------------------------------------------------------------------
CR = 0.03   # the cradle's half width
cradle = Part("bolt_string", origin=(0.0, STRING_Y, REST_Z))
cradle.cyl(0.0019, -CR, CR, "Black", segs=10, xf=T(0, STRING_Y, REST_Z) @ RY(math.pi / 2))
for sx in (-1, 1):
    cradle.box((0.019, 0.007, 0.010), (sx * 0.0205, STRING_Y + 0.0005, REST_Z), "Leather",
               bevel=0.0015, segs=2)

rest = Part("string_rest")
drawn = Part("string_drawn", origin=(0.0, TIP_Y, TIP_Z))
for sx, anc in zip((-1, 1), ANCHORS):
    tube3d(rest, [(sx * CR, STRING_Y, REST_Z), anc], SR, "Rope", sides=6)
    tube3d(drawn, [(sx * CR, STRING_Y, LATCH_Z), anc], SR, "Rope", sides=6)

# --- A bolt in the groove, nocked on the latched string ---------------------------------------------
rb_body, rb_vanes, rb_cut = Part("static_rail_bolt"), Part("rail_bolt_vanes"), Part("cut")
# The nock's floor against the string's front.
NOCK_BACK = LATCH_Z - 0.0019 + NOCK_DEPTH
add_bolt(rb_body, rb_vanes, rb_cut, T(0.0, AXIS, NOCK_BACK - HALF))
rb_body.cut(rb_cut)

screws.cut(slots)
join("static", stock, nose, grip, pad, rail, latch, prod, kit, screws)
for p in (cradle, rest, drawn, trig, safety_bolt):
    make(p)
join("static_rail_bolt", rb_body, rb_vanes)

empty("pt_muzzle", (0.0, AXIS, MUZZLE))
empty("pt_sight", (0.0, 0.085, -0.027))
empty("pt_port", (0.0, 0.07, -0.2))
empty("pt_load_port", (0.0, 0.07, -0.2))
empty("pt_support", (0.0, 0.0, -0.28))
empty("pt_handle", (0.0, 0.065, REST_Z))
empty("pt_latch", (0.0, 0.065, LATCH_Z))
empty("pt_string_tips", (0.0, TIP_Y, TIP_Z))
empty("pt_sling_front", (0.0, -0.0125, -0.37))
empty("pt_sling_rear", (0.0, stock_y(0.25, False) - 0.0085, 0.25))
print(f"  string tips (anchors) at x = +-{ANCHORS[1].x:.4f}, y {TIP_Y:.4f}, z {TIP_Z:.4f}; "
      f"rail bolt nock back at z {NOCK_BACK:.4f}; rear sling at y "
      f"{stock_y(0.25, False) - 0.0085:.4f}")
report()
export("assets/models/guns/crossbow.glb")
