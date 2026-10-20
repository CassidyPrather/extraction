"""Lev's hand: the driver's, reaching back through the cab hatch to pass
the player something (src/raid/lev.rs, claude/polish-story). An older
man's bare right hand, curled round a small thing (a tin, a folded
photograph) about 7 cm across, palm up with the thumb over it, and the
cuff of a grey wool sleeve. Writes assets/models/props/lev_hand.glb, one
static object `lev_hand` (no joints: it holds the one pose).

The frame is the gloves' (art/hands/gloves.py): palm down, fingers
along -Z, the back of the hand +Y, the palm's centre at the origin, the
thumb on -X; the arm runs back along +Z. Turn it pi about Z for palm up.
What it holds sits at `HELD` (centre), so the game hangs the item there.

Materials: Skin (the hand), Nail, Wool (the sleeve).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (RX, RY, RZ, S, T, TO_BLENDER, Part, bake_ao, export, join, main_guard,
                    make, report)
from mathutils import Vector

# The glove's bones and roots (gloves.py, src/render/props.rs), a touch
# shorter: a bare hand is smaller than a glove on it.
SHRINK = 0.96
FINGER_BONES = [[b * SHRINK for b in f] for f in [
    [0.040, 0.025, 0.020],
    [0.044, 0.028, 0.022],
    [0.041, 0.026, 0.020],
    [0.033, 0.020, 0.018],
    [0.036, 0.030],
]]
# Radius at each joint and at the tip: thinner than the glove's, the
# knuckles a little proud (an old hand's).
RADII = [
    [0.0090, 0.0084, 0.0076, 0.0065],
    [0.0092, 0.0086, 0.0078, 0.0067],
    [0.0088, 0.0082, 0.0074, 0.0064],
    [0.0080, 0.0074, 0.0067, 0.0058],
    [0.0104, 0.0094, 0.0080],
]
JOINT_BEND = [1.4, 1.65, 1.1]
THUMB_BEND = [0.0, 0.9]
TS = -1.0  # a right hand: the thumb on -X
SIDES = 10

# The pose: each finger's curl (0 open, 1 a fist), wrapped round `HELD`.
CURL = [0.5, 0.56, 0.6, 0.64, 0.45]
# What the hand holds: its centre, in this frame (palm down), and radius.
HELD = (0.0, -0.043, -0.052)
HELD_R = 0.035


def finger_root(f):
    if f == 4:
        return (0.038 * TS, -0.008, -0.005)
    return ((0.03 - f * 0.02) * TS * SHRINK, 0.0, -0.048 * SHRINK)


def finger_aim(f, c):
    """The first joint's turn, curled `c` (src/render/props.rs,
    finger_aim_spread): the thumb swings, the others bend."""
    if f == 4:
        t = lambda x, y, z: Vector((x * TS, y, z))
        opn, grip, fist = t(0.45, -0.25, -0.86), t(-0.15, -0.55, -0.82), t(-0.75, -0.45, -0.5)
        on = 0.62
        d = opn.lerp(grip, c / on) if c < on else grip.lerp(fist, (c - on) / (1 - on))
        q = Vector((0, 0, -1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
        return q @ RZ(-0.6 * TS)
    return RY(f * 0.04 * TS * (1 - c)) @ RX(-JOINT_BEND[0] * c)


def joint_bend(f, j, c):
    return RX(-(THUMB_BEND[j] if f == 4 else JOINT_BEND[j]) * c)


def segment(f, j):
    """One bone of a finger, built along -Z from its joint: a tapered
    capsule, its ends round on the joints so the finger reads as one shape;
    a nail on the last."""
    bones = FINGER_BONES[f]
    L = bones[j]
    last = j == len(bones) - 1
    r0, r1 = RADII[f][j], RADII[f][j + 1]
    c1 = -(L - r1) if last else -L
    p = Part(f"finger_{f}_{j}")
    n = 3
    prof = [(r0 * math.sin(math.pi / 2 * k / n), r0 * math.cos(math.pi / 2 * k / n))
            for k in range(n + 1)]
    # Bony: a waist between the knuckles.
    prof.append(((r0 + r1) * 0.5 - 0.0004, c1 * 0.5))
    prof += [(r1 * math.cos(math.pi / 2 * k / n), c1 - r1 * math.sin(math.pi / 2 * k / n))
             for k in range(n + 1)]
    p.lathe(prof, "Skin", segs=SIDES, start=360.0 / SIDES / 2)
    if last:
        # The nail: a thin shell over the back of the tip, short and ridged
        # with age (a flattened sphere, slightly proud).
        p.sphere(1.0, (0, 0, 0), "Nail", segs=10, rings=5,
                 xf=T(0.0, r1 * 0.72, c1 + r1 * 0.2) @ S(r1 * 0.78, 0.0018, r1 * 1.25))
    return p


# --- Palm and wrist ----------------------------------------------------------

Z_WRIST, Z_FRONT = 0.054, -0.055


def palm_shape(z):
    """(half width, centre y, half height) of the hand's cut at z: the
    glove's palm, slimmer."""
    t = (z - Z_FRONT) / (Z_WRIST - Z_FRONT)
    w = (0.0405 - 0.0125 * t ** 1.4) * SHRINK
    top = 0.0092 + 0.0045 * math.sin(math.pi * min(1.0, t * 1.1)) - 0.001 * t
    bot = -0.0095 - 0.005 * min(1.0, t * 1.6)
    nose = -0.043
    if z < nose:
        k = math.sqrt(max(0.0, 1.0 - ((z - nose) / (Z_FRONT - nose)) ** 2))
        k = 0.35 + 0.65 * k
        w *= 0.8 + 0.2 * k
        top, bot = top * k, bot * k
    return w, (top + bot) / 2, (top - bot) / 2


def ring_at(z, w, yc, h, segs, power=2.0):
    pts = []
    e = 2.0 / power
    for k in range(segs):
        th = 2 * math.pi * k / segs
        c, s = math.cos(th), math.sin(th)
        pts.append(Vector((math.copysign(abs(c) ** e, c) * w, yc + math.copysign(abs(s) ** e, s) * h,
                           z)))
    return pts


# The forearm from the wrist back: (z, half width, half height), rounder
# as it goes.
ARM = [(0.054, 0.0285, 0.0155), (0.075, 0.0272, 0.0165), (0.10, 0.0285, 0.019),
       (0.15, 0.0315, 0.022), (0.20, 0.0335, 0.024), (0.28, 0.035, 0.026)]


def hand():
    p = Part("hand")
    segs = 20
    rings = []
    for i in range(17):
        t = i / 16
        z = Z_WRIST + (Z_FRONT - Z_WRIST) * (1 - (1 - t) ** 1.5)
        w, yc, h = palm_shape(z)
        rings.append(ring_at(z, w, yc, h, segs))
    # Back toward the elbow (inside the sleeve past its cuff), closed.
    w0, yc0, _ = palm_shape(Z_WRIST)
    arm = []
    for z, w, h in ARM[1:]:
        arm.append(ring_at(z, w, yc0 + 0.001, h, segs, power=2.0))
    p.loft(list(reversed(arm)) + rings, "Skin")
    # The thenar pad, the thumb's root. (Knuckle and wrist bumps read
    # as stuck-on discs, not bone: the hand stays smooth.)
    p.sphere(1.0, (0, 0, 0), "Skin", segs=16, rings=8,
             xf=T(0.022 * TS, -0.006, 0.008) @ RY(-0.35 * TS) @ RZ(0.35 * TS)
             @ S(0.0145, 0.0072, 0.026))
    return p


def sleeve():
    """The grey wool sleeve: a loose tube over the forearm from just behind
    the wrist, its cuff turned back once, a little rumpled."""
    p = Part("sleeve")
    segs = 18
    _, yc0, _ = palm_shape(Z_WRIST)
    rings = []
    # The cuff's lip turns in over the arm (no open hole to see into).
    stations = [(0.088, 0.85), (0.086, 1.0), (0.09, 1.07), (0.105, 1.1), (0.112, 1.1),
                (0.118, 1.04), (0.15, 1.05), (0.2, 1.07), (0.26, 1.08), (0.30, 1.08)]
    for z, grow in stations:
        # The forearm's size here (interpolated), and the cloth over it.
        for (za, wa, ha), (zb, wb, hb) in zip(ARM, ARM[1:]):
            if za <= z <= zb or z > ARM[-1][0]:
                t = min(1.0, (z - za) / (zb - za))
                w, h = wa + (wb - wa) * t, ha + (hb - ha) * t
                if za <= z <= zb:
                    break
        w, h = (w + 0.006) * grow, (h + 0.006) * grow
        ring = []
        for k in range(segs):
            th = 2 * math.pi * k / segs
            # Folds: a slow ripple round the sleeve, shifting along it.
            f = 1.0 + 0.035 * math.sin(3 * th + z * 40) + 0.02 * math.sin(7 * th - z * 25)
            ring.append(Vector((math.cos(th) * w * f, yc0 + 0.001 + math.sin(th) * h * f, z)))
        rings.append(ring)
    p.loft(rings, "Wool")
    return p


# --- Assembly ----------------------------------------------------------------

def placed(part, game):
    obj = make(part, sharp_angle=60.0)
    obj.matrix_world = TO_BLENDER @ game @ TO_BLENDER.inverted()
    return obj


def lev_hand(with_tin=False):
    main_guard()
    objs = [make(hand(), sharp_angle=70.0), make(sleeve(), sharp_angle=70.0)]
    tips = []
    for f in range(5):
        chain = T(*finger_root(f)) @ finger_aim(f, CURL[f])
        for j, L in enumerate(FINGER_BONES[f]):
            if j > 0:
                chain = chain @ joint_bend(f, j, CURL[f])
            objs.append(placed(segment(f, j), chain))
            chain = chain @ T(0.0, 0.0, -L)
        tips.append(chain.translation)
    for f, tip in enumerate(tips):
        d = (tip - Vector(HELD)).length - HELD_R
        print(f"finger {f} tip {tuple(round(v, 3) for v in tip)}, {d * 1000:.0f} mm off the item")
    join("lev_hand", *objs)
    # Shade between the curled fingers and where the hand meets the cuff,
    # as the world props are (the game multiplies it in).
    bake_ao(ground=False, distance=0.04, strength=0.6)
    if with_tin:
        # A preview stand-in for what it holds (not exported with the hand).
        t = Part("tin")
        t.cyl(HELD_R, -0.012, 0.012, "Steel", segs=24, xf=T(*HELD) @ RY(math.pi / 2))
        make(t)
    report()
    export("assets/models/props/lev_hand.glb" if not with_tin
           else os.environ.get("LEV_PREVIEW", "/tmp/lev_hand_tin.glb"), colors=True)


lev_hand(with_tin=bool(os.environ.get("LEV_PREVIEW")))
