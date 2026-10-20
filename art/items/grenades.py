"""Hand grenades: the RGD-5 (a smooth egg) and the F1 (the segmented
"pineapple"), both on the UZRGM-style fuse, and the pin ring and spoon that
come off them. Writes assets/models/items/{rgd5,f1,grenade_pin,
grenade_spoon}.glb (item space: see _kit.py; fuse up).

Nodes on a grenade, for the throw (src/weapons, Weapon feel's):

- `body`   the grenade and its fuse, the item's model
- `spoon`  the lever down the +Z side, origin on its hinge at the top of the
           fuse; it swings out (+Z) turning about local +X
- `pin`    the split pin through the fuse and its ring on the +X side,
           origin at the ring's centre; hidden once pulled
- empties `pt_pin` (the ring's centre, where a hand takes it) and
  `pt_spoon` (the middle of the spoon on the body, under the palm)

`grenade_pin` and `grenade_spoon` are those two on their own, centred, for
the loose ring left in the hand and the spoon that flies off.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import common  # noqa: E402
from _kit import UP, ring, text, wanted  # noqa: E402
from common import (RX, RY, RZ, T, Part, empty, export, make, report, reset,  # noqa: E402
                    section_rect)
from mathutils import Vector  # noqa: E402

FUSE_Y = 0.0  # set per grenade: where the fuse's collar meets the body


def fuse(p, y0):
    """The fuse screwed into the body's neck at `y0`: a collar, the striker
    housing, a domed cap; the lever's hinge ears on the +Z side."""
    p.lathe([(0.0, y0 - 0.002), (0.0112, y0 - 0.002), (0.0112, y0 + 0.006), (0.0098, y0 + 0.007),
             (0.0098, y0 + 0.024), (0.0085, y0 + 0.026), (0.004, y0 + 0.0275),
             (0.0, y0 + 0.0275)], "Zinc", segs=20, xf=UP)
    # Knurled grip on the collar.
    for k in range(16):
        a = 2 * math.pi * k / 16
        p.box((0.0012, 0.007, 0.0016), (0.0, 0.0, 0.0), "Zinc",
              xf=RY(a) @ T(0.0114, y0 + 0.002, 0.0))
    # Hinge ears either side of the lever's hook, and the pin's bosses.
    for sx in (-1, 1):
        p.box((0.0016, 0.007, 0.006), (sx * 0.0068, y0 + 0.0215, 0.0102), "Zinc", bevel=0.0005,
              segs=1)
        p.cyl(0.0024, 0.0, 0.003, "Zinc", segs=10,
              xf=T(sx * 0.0098, y0 + 0.017, 0.004) @ RY(sx * math.pi / 2))


def spoon(y0, foot):
    """The lever: a hook over the hinge at the fuse's top, down the fuse
    and out over the body's shoulder, ending at y `foot`; built in place,
    origin on the hinge."""
    hinge = (0.0, y0 + 0.0235, 0.0108)
    s = Part("spoon", origin=hinge)
    body_r = PROFILE_R
    path = [(0.0085, y0 + 0.0255), (0.0118, y0 + 0.0255), (0.0122, y0 + 0.02), (0.0122, y0 + 0.008)]
    y = y0 + 0.002
    while y > foot:
        path.append((body_r(y) + 0.0022, y))
        y -= 0.006
    path.append((body_r(foot) + 0.0022, foot))
    s.sweep(path, section_rect(0.012, 0.0012, 0.0004), "Olive")
    return s


def pin_ring(y0, name="pin"):
    """The split pin through the bosses (along X) and its ring on the +X
    side, origin at the ring's centre."""
    centre = (0.0235, y0 + 0.012, 0.004)
    p = Part(name, origin=centre)
    p.cyl(0.0008, -0.0125, 0.0125, "Steel", segs=8, xf=T(0.0, y0 + 0.017, 0.004) @ RY(math.pi / 2))
    # The ring hangs from the pin's end, in the XY plane.
    ring(p, 0.0105, 0.0011, "Steel", xf=T(*centre), segs=24, sides=6)
    p.box((0.003, 0.0016, 0.0016), (0.0128, y0 + 0.017, 0.004), "Steel", bevel=0.0004, segs=1)
    return p, centre


PROFILE_R = None


def shifted(parts, points, d):
    """Move Parts (their geometry and origins) and points by `d`."""
    dv = Vector(d)
    for p in parts:
        for q in [p] + p.cutters:
            for v in q.bm.verts:
                v.co += dv
        p.origin = p.origin + dv
    return {k: tuple(Vector(v) + dv) for k, v in points.items()}


def centre_of(parts):
    vs = [v.co for p in parts for v in p.bm.verts]
    lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
    hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
    return -(lo + hi) / 2, hi - lo


def write(item_id, parts, points):
    d, size = centre_of(parts)
    points = shifted(parts, points, d)
    for p in parts:
        make(p)
    for k, v in points.items():
        empty(k, v)
    print(f"{item_id}: {size.x:.3f} x {size.y:.3f} x {size.z:.3f} m")
    report()
    common.FOLDS.clear()
    export(f"assets/models/items/{item_id}.glb")


def rgd5():
    """The RGD-5: a smooth two-piece egg, 57 mm across, a rolled seam round
    its middle; 0.31 kg."""
    global PROFILE_R
    reset()
    prof = [(0.0, 0.0), (0.008, 0.0004), (0.017, 0.0028), (0.0235, 0.0075), (0.0275, 0.0145),
            (0.0285, 0.026), (0.0285, 0.05), (0.0272, 0.063), (0.0232, 0.0735), (0.0168, 0.0805),
            (0.0118, 0.0832), (0.0, 0.0832)]

    def r(y):
        for (r0, y0), (r1, y1) in zip(prof, prof[1:]):
            if y0 <= y <= y1 and y1 > y0:
                return r0 + (r1 - r0) * (y - y0) / (y1 - y0)
        return 0.0
    PROFILE_R = r
    body = Part("body")
    body.lathe(prof, "Olive", segs=32, xf=UP)
    body.lathe([(0.0285, 0.0372), (0.0297, 0.0378), (0.0297, 0.0392), (0.0285, 0.0398)], "Olive",
               segs=32, xf=UP, closed=True)
    # The stencilled marking on the -Z face.
    text(body, "PГД-5", 0.006, "Black", T(0.0, 0.026, -0.0284) @ RY(math.pi), depth=0.0004)
    fuse(body, 0.0832)
    sp = spoon(0.0832, 0.03)
    pin, rc = pin_ring(0.0832)
    write("rgd5", [body, sp, pin], {"pt_pin": rc, "pt_spoon": (0.0, 0.05, 0.034)})


def f1():
    """The F1: a cast-iron egg in 8 x 6 raised segments, 55 mm across, the
    fuse in a threaded neck on top; 0.6 kg."""
    global PROFILE_R
    reset()
    yc, ra, rb = 0.047, 0.0275, 0.043   # ellipsoid: centre, radial and vertical semi-axes

    def r(y):
        t = (y - yc) / rb
        return ra * math.sqrt(max(0.0, 1.0 - t * t))
    PROFILE_R = lambda y: r(y) if y < 0.088 else 0.0118  # noqa: E731
    body = Part("body")
    core = []
    n = 16
    for k in range(n + 1):
        y = 0.004 + (0.086 - 0.004) * k / n
        core.append((r(y) - 0.0028, y))
    body.lathe([(0.0, 0.0), (0.0085, 0.0), (0.0095, 0.004)] + core[1:] +
               [(0.0122, 0.0915), (0.0, 0.0915)], "Olive", segs=32, xf=UP)
    # The segments: 6 rows of 8 raised blocks, their faces on the egg.
    rows, cols = 6, 8
    y_lo, y_hi = 0.011, 0.083
    pitch = (y_hi - y_lo) / rows
    for i in range(rows):
        y = y_lo + pitch * (i + 0.5)
        rr = r(y)
        ny = (y - yc) / (rb * rb)
        nr = rr / (ra * ra)
        tilt = math.atan2(ny, nr)
        h = pitch / math.cos(tilt) * 0.78
        w = 2 * math.pi * rr / cols * 0.74
        # A block in section: chamfered on its top and bottom edges.
        block = [(-0.003, -h / 2), (0.0011, -h / 2), (0.002, -h / 2 + 0.0011),
                 (0.002, h / 2 - 0.0011), (0.0011, h / 2), (-0.003, h / 2)]
        for j in range(cols):
            a = 2 * math.pi * (j + 0.5) / cols
            body.prism(block, -w / 2, w / 2, "Olive", axis="z", segs=1,
                       xf=RY(a) @ T(rr - 0.0018, y, 0.0) @ RZ(tilt))
    fuse(body, 0.0915)
    sp = spoon(0.0915, 0.045)
    pin, rc = pin_ring(0.0915)
    write("f1", [body, sp, pin], {"pt_pin": rc, "pt_spoon": (0.0, 0.07, 0.033)})


def loose_pin():
    reset()
    p, _ = pin_ring(0.0, name="grenade_pin")
    write("grenade_pin", [p], {})


def loose_spoon():
    global PROFILE_R
    reset()
    PROFILE_R = lambda y: 0.0118 if y > -0.004 else 0.022 + (-0.004 - y) * 0.3  # noqa: E731
    s = spoon(0.0, -0.05)
    s.name = "grenade_spoon"
    write("grenade_spoon", [s], {})


if __name__ == "__main__":
    for item_id, build in (("rgd5", rgd5), ("f1", f1), ("grenade_pin", loose_pin),
                           ("grenade_spoon", loose_spoon)):
        if wanted(item_id):
            build()
