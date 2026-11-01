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


# The items' stencil lettering (`art/items/_kit.py`), for a prop's painted
# marks: a blank pale plate where a stencil should be reads as a
# placeholder (docs/craft/PROP_READING.md).
_items_kit = None


def _glyphs():
    global _items_kit
    if _items_kit is None:
        import importlib.util
        here = os.path.dirname(os.path.abspath(__file__))
        spec = importlib.util.spec_from_file_location(
            "items_kit_lettering", os.path.join(here, "..", "items", "_kit.py"))
        _items_kit = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(_items_kit)
    return _items_kit.GLYPHS, _items_kit.LOOKALIKE


def stencil_strokes(s, h, align="centre"):
    """The strokes of `s` in stencil letters `h` high, in the XY plane:
    (centre x, centre y, length, width, angle) each. The items' glyphs
    (`art/items/_kit.py`) cut as a stencil is: strokes a sixth of the
    height, each stopped short where it meets another (the bridges that
    hold a stencil's centres in), free ends squared. `art/check_glyphs.py`
    holds every glyph apart from every other through this."""
    glyphs, lookalike = _glyphs()
    k = h / 4.0
    stroke = h * 0.16
    bridge = stroke * 0.8
    adv = 2.9 * k + stroke * 0.5
    width = adv * len(s) - 0.9 * k
    x0 = {"centre": -width / 2, "left": 0.0, "right": -width}[align]
    out = []
    for i, ch in enumerate(s):
        segs = glyphs.get(lookalike.get(ch, ch), [])
        ends = [pt for seg in segs for pt in seg]
        for (ax, ay), (bx, by) in segs:
            ax_, ay_ = x0 + i * adv + ax * k, ay * k - h / 2
            bx_, by_ = x0 + i * adv + bx * k, by * k - h / 2
            L = math.hypot(bx_ - ax_, by_ - ay_)
            if L < 1e-6:
                continue
            a = math.atan2(by_ - ay_, bx_ - ax_)
            # Stop short at an end another stroke shares (a joint), and
            # run on past a free end to square it off.
            cut = [bridge if ends.count(e) > 1 else -stroke * 0.5
                   for e in ((ax, ay), (bx, by))]
            L2 = L - cut[0] - cut[1]
            if L2 < stroke * 0.5:
                continue
            mid = (L * 0.5 + (cut[0] - cut[1]) * 0.5) / L
            out.append((ax_ + (bx_ - ax_) * mid, ay_ + (by_ - ay_) * mid, L2, stroke, a))
    return out


def lettering(p, s, h, mat, xf, depth=0.002, align="centre"):
    """Stencil letters `h` high painted on a face: drawn in XY facing +Z,
    then placed by `xf` (`stencil_strokes`: so a В isn't an 8 and the
    letters hold at a crate's reading distance)."""
    for cx, cy, L, w, a in stencil_strokes(s, h, align):
        p.box((L, w, depth), (0.0, 0.0, depth / 2), mat, xf=xf @ T(cx, cy, 0.0) @ RZ(a))


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
# Bake the sky term into alpha too (see `common.bake_ao`): for structures
# with floors under their own roofs, outdoors.
AO_SKY = False


def bake_ao(ground=True, distance=None, rays=40):
    """Bake this prop's ambient occlusion (see `common.bake_ao`)."""
    common_bake_ao(ground=ground, distance=distance or AO_DISTANCE, strength=AO_STRENGTH,
                   rays=rays, sky=AO_SKY)


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
