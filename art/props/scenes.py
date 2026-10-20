"""Props for staged scenes (the Story workstream's "scene:<n>" prefabs).
Writes assets/models/props/<name>.glb, one node `static` each:

- `remains_seated` someone long dead, slumped on a chair: a dark quilted
                   work coat (a telogreika) with its collar up, a knitted
                   cap pulled down, boots, gloved hands; the head bowed so
                   no face shows. Origin at the chair's floor centre, on
                   its 0.45 m seat, facing -Z like `chair`.
- `remains_lying`  the same, face down on the ground and half curled,
                   about 1.7 m along Z (head toward -Z); origin on the
                   ground at its middle.
- `bedroll`        a quilted sleeping bag rolled out, 0.7 x 1.9 along Z,
                   its head end rolled back; origin on the ground at its
                   middle.
- `campfire`       a cold fire: a ring of stones 0.8 across, charred logs
                   and ash; origin on the ground at its middle.

Nothing here has a face or a wound: the playtester's rule is unease, not
gore.

Materials: Wool, Canvas, Rubber, Olive, Pebble, Char, Timber.
"""

import math
import os
import random
import sys

from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, prop, rod  # noqa: E402
from common import RY, Part, T, fillet  # noqa: E402


def limb(p, a, b, r0, r1, mat, segs=10):
    """A tapered, round-ended limb from a (radius r0) to b (radius r1)."""
    a, b = Vector(a), Vector(b)
    v = b - a
    L = v.length
    prof = [(0.0, -r0)]
    for k in range(1, 4):
        t = math.pi / 2 * k / 4
        prof.append((r0 * math.sin(t), -r0 * math.cos(t)))
    prof += [(r0, 0.0), (r1, L)]
    for k in range(1, 4):
        t = math.pi / 2 * k / 4
        prof.append((r1 * math.cos(t), L + r1 * math.sin(t)))
    prof.append((0.0, L + r1))
    rot = Vector((0, 0, 1)).rotation_difference(v.normalized()).to_matrix().to_4x4()
    p.lathe(prof, mat, segs=segs, xf=T(*a) @ rot)


def blob(p, centre, r, scale, mat, segs=14, rings=8):
    p.sphere(r, centre, mat, segs=segs, rings=rings, scale=scale)


def quilting(p, a, b, r, mat, n):
    """Stitched ribs round a limb or torso from a to b: thin rings 4 mm
    proud of radius r."""
    a, b = Vector(a), Vector(b)
    v = b - a
    rot = Vector((0, 0, 1)).rotation_difference(v.normalized()).to_matrix().to_4x4()
    for k in range(1, n):
        z = v.length * k / n
        p.lathe([(r - 0.01, z - 0.008), (r + 0.004, z - 0.008), (r + 0.004, z + 0.008),
                 (r - 0.01, z + 0.008)], mat, segs=12, closed=True, xf=T(*a) @ rot)


def figure(p, j):
    """A body from joint positions `j` (game space): coat, trousers,
    boots, gloves, cap."""
    coat, trousers = "Wool", "Canvas"
    # Torso: the coat from hips to shoulders, quilted, a collar up round
    # the neck.
    limb(p, j["hips"], j["chest"], 0.17, 0.2, coat, segs=12)
    quilting(p, j["hips"], j["chest"], 0.175, coat, 5)
    limb(p, j["chest"], j["neck"], 0.19, 0.11, coat, segs=12)
    # The head bowed into the collar, under a knitted cap.
    blob(p, j["head"], 0.11, (1.0, 1.05, 1.1), "Wool")
    blob(p, Vector(j["head"]) + Vector((0, 0.03, 0)), 0.115, (1.0, 0.75, 1.05), "Olive")
    # Arms in quilted sleeves, gloved hands.
    for s in ("l", "r"):
        limb(p, j["shoulder_" + s], j["elbow_" + s], 0.07, 0.06, coat)
        limb(p, j["elbow_" + s], j["wrist_" + s], 0.06, 0.055, coat)
        blob(p, j["hand_" + s], 0.05, (0.8, 0.5, 1.2), "Rubber", segs=10, rings=6)
    # Legs in trousers, tall boots.
    for s in ("l", "r"):
        limb(p, j["hip_" + s], j["knee_" + s], 0.085, 0.07, trousers)
        limb(p, j["knee_" + s], j["ankle_" + s], 0.065, 0.06, "Rubber")
        limb(p, j["ankle_" + s], j["toe_" + s], 0.055, 0.045, "Rubber", segs=8)


def remains_seated():
    p = Part("static")
    # Slumped on the seat (0.45), leaning a little to the left, head down.
    j = {
        "hips": (0.0, 0.61, 0.08), "chest": (0.04, 0.88, 0.02), "neck": (0.06, 1.02, -0.06),
        "head": (0.07, 1.05, -0.16),
        "shoulder_l": (-0.17, 0.95, 0.03), "elbow_l": (-0.24, 0.7, 0.04),
        "wrist_l": (-0.26, 0.47, 0.0), "hand_l": (-0.26, 0.42, -0.02),
        "shoulder_r": (0.23, 0.96, 0.0), "elbow_r": (0.2, 0.72, -0.12),
        "wrist_r": (0.08, 0.6, -0.24), "hand_r": (0.04, 0.59, -0.28),
        "hip_l": (-0.1, 0.53, 0.04), "knee_l": (-0.13, 0.55, -0.38),
        "ankle_l": (-0.16, 0.1, -0.46), "toe_l": (-0.17, 0.05, -0.6),
        "hip_r": (0.1, 0.53, 0.04), "knee_r": (0.15, 0.55, -0.36),
        "ankle_r": (0.22, 0.1, -0.4), "toe_r": (0.26, 0.05, -0.53),
    }
    figure(p, j)
    return [p]


def remains_lying():
    p = Part("static")
    # Face down along Z, head toward -Z, the knees drawn up a little to one
    # side, one arm under the chest, the other flung out.
    j = {
        "hips": (0.0, 0.16, 0.2), "chest": (0.03, 0.18, -0.25), "neck": (0.05, 0.15, -0.45),
        "head": (0.1, 0.12, -0.58),
        "shoulder_l": (-0.18, 0.17, -0.35), "elbow_l": (-0.45, 0.08, -0.5),
        "wrist_l": (-0.5, 0.06, -0.78), "hand_l": (-0.52, 0.05, -0.84),
        "shoulder_r": (0.22, 0.17, -0.35), "elbow_r": (0.18, 0.09, -0.12),
        "wrist_r": (0.06, 0.07, -0.1), "hand_r": (0.02, 0.06, -0.08),
        "hip_l": (-0.1, 0.14, 0.26), "knee_l": (-0.22, 0.09, 0.62),
        "ankle_l": (-0.15, 0.11, 0.96), "toe_l": (-0.15, 0.04, 1.05),
        "hip_r": (0.1, 0.14, 0.26), "knee_r": (0.25, 0.09, 0.55),
        "ankle_r": (0.3, 0.11, 0.9), "toe_r": (0.32, 0.04, 0.99),
    }
    figure(p, j)
    return [p]


def bedroll():
    p = Part("static")
    L, W = 1.6, 0.7
    prof = fillet([(-W / 2, 0.0), (W / 2, 0.0), (W / 2, 0.08), (-W / 2, 0.08)], 0.035, 3)
    p.prism(prof, -L / 2 + 0.15, L / 2, "Canvas", axis="z", bevel=0.025, segs=2)
    # Quilting seams across it, the zip down one side.
    for k in range(1, 6):
        z = -L / 2 + 0.15 + (L - 0.15) * k / 6
        p.box((W - 0.06, 0.006, 0.012), (0.0, 0.081, z), "Olive")
    p.box((0.012, 0.012, L - 0.25), (W / 2 - 0.02, 0.082, 0.07), "Rubber")
    # The head end rolled back on itself.
    p.cyl(0.11, -W / 2 + 0.01, W / 2 - 0.01, "Canvas", segs=14,
          xf=T(0.0, 0.11, -L / 2 + 0.1) @ RY(math.pi / 2), chamfer=0.03)
    return [p]


def campfire():
    p = Part("static")
    rng = random.Random(97)
    # Ash and charred earth, a ring of stones, logs burnt through.
    p.cyl(0.32, -0.01, 0.015, "Char", segs=16, xf=UP)
    n = 10
    for k in range(n):
        a = 2 * math.pi * k / n + rng.uniform(-0.1, 0.1)
        r = 0.36 + rng.uniform(-0.02, 0.03)
        s = rng.uniform(0.07, 0.1)
        p.sphere(s, (r * math.cos(a), s * 0.45, r * math.sin(a)), "Pebble", segs=10, rings=6,
                 scale=(1.2, 0.8, 1.0))
    for k in range(4):
        a = math.pi / 4 * k + rng.uniform(-0.2, 0.2)
        d = Vector((math.cos(a), 0.0, math.sin(a)))
        a0 = d * -0.3 + Vector((0, 0.04, 0))
        a1 = d * 0.12 + Vector((0, 0.1, 0))
        rod(p, a0, a1, rng.uniform(0.03, 0.04), "Char", segs=7)
    rod(p, (0.05, 0.03, 0.18), (0.28, 0.035, 0.5), 0.035, "Timber", segs=7)
    return [p]


prop("remains_seated", remains_seated)
prop("remains_lying", remains_lying)
prop("bedroll", bedroll)
prop("campfire", campfire)
