"""Shared bits for the world props: furniture, crates, signposts, fences,
door leaves and window sashes. Each prop is its own .glb under
assets/models/props/<name>.glb, one mesh node `static`, built in prop
space -- metres, +Y up, -Z the front -- with its origin where the Rust
code places the blockout it replaces (`src/world/buildings.rs`,
`src/render/props.rs`): on the floor under the prop's centre for
furniture, so the prop's collider boxes and loot heights stay true.

Everything is worn: boards a few millimetres off true, edges bevelled so
they catch the light at arm's length, paint chipped back to the wood.
The wobble comes from a seeded RNG, so a rebuild gives the same prop.
"""

import math
import os
import random
import sys

from mathutils import Matrix, Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import bake_ao as common_bake_ao  # noqa: E402
from common import Part, RX, RY, RZ, T, S, export, fillet, make, report, reset  # noqa: E402,F401

UP = RX(-math.pi / 2)       # lathe/cyl axis (Z) -> +Y
ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> +X


def prop(name, build):
    """Build one prop in a fresh scene and write its .glb. `build` returns
    the Parts to make."""
    if not wanted(name):
        return
    reset()
    parts = build()
    for p in parts:
        make(p)
    bake_ao(ground=name not in AIRBORNE)
    print(f"{name}:")
    report()
    export(f"assets/models/props/{name}.glb", colors=True)


# Props that don't stand on the floor: no ground under them in the bake.
AIRBORNE = {"door_leaf", "door_metal", "window_sash", "window_casing", "stash_rack_2",
            "med_cabinet", "aerial", "barrier_arm", "camp_board", "lantern"}

# Ambient occlusion: how far a corner looks for what shades it, how dark a
# fully shut-in corner gets (the shading's own lighting does the rest).
AO_DISTANCE = 0.6
AO_STRENGTH = 0.75


def bake_ao(ground=True, distance=None, rays=40):
    """Bake this prop's ambient occlusion (see `common.bake_ao`)."""
    common_bake_ao(ground=ground, distance=distance or AO_DISTANCE, strength=AO_STRENGTH,
                   rays=rays)


def wanted(name):
    """Scripts can be run for just some props: `-- name1 name2`."""
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or name in argv


class Wear:
    """Seeded wobble: boards a little off true."""

    def __init__(self, seed):
        self.rng = random.Random(seed)

    def j(self, amount):
        """A jitter in [-amount, amount]."""
        return self.rng.uniform(-amount, amount)

    def chance(self, p):
        return self.rng.random() < p


def board(p, size, centre, mat, wear=None, bevel=0.004, tilt=0.0, yaw=0.0, roll=0.0, xf=None):
    """A bevelled board, optionally turned a little about its centre (small
    angles in radians: `tilt` about X, `yaw` about Y, `roll` about Z)."""
    if wear is not None:
        tilt += wear.j(0.006)
        yaw += wear.j(0.006)
        roll += wear.j(0.004)
    m = T(*centre) @ RY(yaw) @ RX(tilt) @ RZ(roll) @ T(*(-Vector(centre)))
    if xf is not None:
        m = xf @ m
    p.box(size, centre, mat, bevel=bevel, segs=1, xf=m)


def rod(p, a, b, r, mat, segs=8, chamfer=0.0):
    """A round bar from a to b (game space)."""
    a, b = Vector(a), Vector(b)
    d = b - a
    rot = Vector((0, 0, 1)).rotation_difference(d.normalized()).to_matrix().to_4x4()
    p.cyl(r, 0.0, d.length, mat, segs=segs, xf=T(*a) @ rot, chamfer=chamfer)


def bolt(p, at, axis, r=0.008, h=0.006, mat="Iron", segs=6):
    """A round bolt head standing `h` out of a face, `axis` its outward
    direction (a unit vector along X, Y or Z, signed)."""
    a = Vector(at)
    rod(p, a - Vector(axis) * 0.002, a + Vector(axis) * h, r, mat, segs=segs)


def angle_iron(p, length, centre, along, mat, w=0.035, t=0.004, flip=(1, 1)):
    """An L-section bar along `along` ('x', 'y' or 'z'), its outside corner
    at `centre`; the two legs run in the +/- directions given by `flip`
    across the other two axes."""
    cx, cy, cz = centre
    fa, fb = flip
    if along == "x":
        p.box((length, t, w), (cx, cy + fa * t / 2, cz + fb * w / 2), mat)
        p.box((length, w, t), (cx, cy + fa * w / 2, cz + fb * t / 2), mat)
    elif along == "y":
        p.box((w, length, t), (cx + fa * w / 2, cy, cz + fb * t / 2), mat)
        p.box((t, length, w), (cx + fa * t / 2, cy, cz + fb * w / 2), mat)
    else:
        p.box((w, t, length), (cx + fa * w / 2, cy + fb * t / 2, cz), mat)
        p.box((t, w, length), (cx + fa * t / 2, cy + fb * w / 2, cz), mat)
