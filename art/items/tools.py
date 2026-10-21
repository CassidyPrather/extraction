"""Hand tools for Hands' interactions: a crowbar and bolt cutters. Writes
assets/models/items/<id>.glb in item space (origin at the centre of the
item's dims box, -Z forward), built in place round the grip points Hands'
code uses (so not re-centred).

- `crowbar` (dims 0.03 x 0.05 x 0.76): a hexagon steel bar, painted, its
  claw at -Z (the chisel tip at (0, 0.012, -0.37), bent a little up and
), a short pry tip bent down at +Z; the grip at z +0.30, the steadying
  hand at z 0.0 (one node, `body`)
- `bolt_cutters` (dims 0.16 x 0.04 x 0.62), jaws forward:
  - `head`          the cheek plates and bolts (static)
  - `jaw_left`, `jaw_right`      origin on their pivot (0, 0, -0.2), the
                                 jaws meeting at (0, 0, -0.30) shut; they
                                 open turning about Y
  - `handle_left`, `handle_right` origins on their pivots (-+0.014, 0,
                                 -0.14), rubber grips ending at (-+0.07, 0,
                                 +0.26); they spread turning about Y

Materials: Iron, Red, Steel, Black, Rubber.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import item, wanted  # noqa: E402
from common import RX, RY, T, Part, fillet, section_circle  # noqa: E402
from mathutils import Vector  # noqa: E402


def rod(p, a, b, r, mat, segs=10, chamfer=0.0):
    """A cylinder from a to b."""
    a, b = Vector(a), Vector(b)
    d = b - a
    yaw = math.atan2(d.x, d.z)
    pitch = -math.asin(d.y / d.length)
    p.cyl(r, 0.0, d.length, mat, segs=segs, xf=T(*a) @ RY(yaw) @ RX(pitch), chamfer=chamfer)


def hexring(w, t, y, z):
    """The bar's hexagon (as the sweep's section) squashed to w x t at
    height y: so the claw and pry tip flatten out of the bar seamlessly."""
    return [Vector((w / 2 * math.cos(math.radians(60 * k)), y + t / 2 * math.sin(math.radians(60 * k)),
                    z)) for k in range(6)]


# --- Crowbar -------------------------------------------------------------------------------

def crowbar():
    p = Part("body")
    R = 0.0085
    # The hexagon bar, straight from the claw's neck to the curl.
    p.sweep([(-0.33, 0.0), (0.0, 0.0), (0.33, 0.0)], section_circle(R, 6), "Iron")
    # A red painted grip band at the handle end, a little proud.
    p.sweep([(0.24, 0.0), (0.33, 0.0)], section_circle(R + 0.0008, 6), "Red")
    # The claw: from the hexagon flattening to a chisel, bent up to its tip.
    rings = []
    for z, w, t, y in ((-0.328, 2 * R, 2 * R, 0.0), (-0.345, 0.02, 0.011, 0.002),
                       (-0.36, 0.022, 0.006, 0.007), (-0.37, 0.022, 0.0016, 0.012)):
        rings.append(hexring(w, t, y, z))
    p.loft(rings, "Steel")
    # The far end: a short flattened pry tip, bent down (a full gooseneck
    # wouldn't fit the item's 5 cm).
    rings = []
    for z, w, t, y in ((0.328, 2 * R, 2 * R, 0.0), (0.35, 0.018, 0.01, -0.002),
                       (0.366, 0.019, 0.005, -0.008), (0.377, 0.019, 0.0016, -0.014)):
        rings.append(hexring(w, t, y, z))
    p.loft(rings, "Iron")
    return [p]


# --- Bolt cutters ----------------------------------------------------------------------------

JAW_PIVOT = (0.0, 0.0, -0.2)


def bolt_cutters():
    head = Part("head")
    parts = [head]
    # Cheek plates either side of the jaws and handles' pivots, and bolts.
    plate = fillet([(-0.032, -0.225), (0.032, -0.225), (0.026, -0.12), (-0.026, -0.12)],
                   [0.008, 0.008, 0.01, 0.01], 3)
    for sy in (-1, 1):
        head.prism([(x, z) for x, z in plate], sy * 0.009, sy * 0.014, "Black", axis="y",
                   bevel=0.0015, segs=1)
    for x, z in ((0.0, -0.2), (-0.014, -0.14), (0.014, -0.14)):
        rod(head, (x, -0.0165, z), (x, 0.0165, z), 0.0055, "Steel", segs=10, chamfer=0.001)
    for side, sx in (("left", -1), ("right", 1)):
        # The jaw: a stout blade from its pivot to the cutting edge at the
        # front, meeting its fellow on the centre line.
        jaw = Part(f"jaw_{side}", origin=JAW_PIVOT)
        out = [(0.0, -0.3), (sx * 0.006, -0.3), (sx * 0.024, -0.255), (sx * 0.028, -0.2),
               (sx * 0.02, -0.17), (0.0, -0.17)]
        if sx > 0:
            out = out[::-1]
        jaw.prism(fillet(out, [0.001, 0.002, 0.008, 0.008, 0.006, 0.002], 2), -0.008, 0.008,
                  "Black", axis="y", bevel=0.0012, segs=1)
        jaw.prism([(sx * 0.0005, -0.299), (sx * 0.004, -0.299), (sx * 0.0045, -0.24),
                   (sx * 0.0005, -0.24)][::(-1 if sx > 0 else 1)], -0.0085, 0.0085, "Steel",
                  axis="y")
        parts.append(jaw)
        # The handle: a painted steel tube out to the rubber grip.
        piv = (sx * 0.014, 0.0, -0.14)
        h = Part(f"handle_{side}", origin=piv)
        end = Vector((sx * 0.07, 0.0, 0.26))
        start = Vector(piv)
        grip0 = start + (end - start) * 0.6
        rod(h, start, grip0 + (end - start) * 0.02, 0.0085, "Red", segs=10)
        rod(h, grip0, end, 0.012, "Rubber", segs=12, chamfer=0.003)
        for k in range(5):
            c = grip0 + (end - grip0) * (0.15 + 0.17 * k)
            rod(h, c - (end - start).normalized() * 0.003, c + (end - start).normalized() * 0.003,
                0.0128, "Rubber", segs=12)
        parts.append(h)
    return parts


if __name__ == "__main__":
    for item_id, build in (("crowbar", crowbar), ("bolt_cutters", bolt_cutters)):
        if wanted(item_id):
            item(item_id, build)
