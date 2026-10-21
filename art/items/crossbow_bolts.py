"""Home-made crossbow bolts: an 8 mm steel rod filed to a point, three
vanes cut from a plastic bottle (the odd one, the cock vane, from a
different bottle) bound on with thread, and a nock notch sawn across the
back. Writes assets/models/items/crossbow_bolt.glb (one bolt) and
bolt_bundle.glb (five, tied with twists of copper wire). Item space: see
_kit.py; the bolts lie along Z, point to -Z.

`add_bolt` is imported by art/guns/crossbow.py for the bolt lying in the
rail, so the two always match. In bolt space the rod's axis is the Z axis,
its back (nock) at z = +0.19 and its point at z = -0.19, the cock vane
pointing -Y (down into the crossbow's flight groove); the nock slot runs
across X, the way the string lies.
"""

import math
import os
import sys

from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import item, transform, wanted  # noqa: E402
from common import RZ, T, Part  # noqa: E402

ROD_R = 0.004
HALF = 0.19          # the bolt is 0.38 m long
VANE_TIP = 0.014     # how far a vane reaches from the axis
NOCK_DEPTH = 0.006   # the notch sawn into the back
# A vane (z along the bolt, r out from the axis): cut from a bottle, low at
# the front, rising to a swept-back tip near the back.
VANE = [(0.100, 0.0036), (0.165, 0.0036), (0.168, 0.0125), (0.161, VANE_TIP), (0.128, 0.0105),
        (0.100, 0.0052)]


def tube3d(p, pts, r, mat, sides=6, closed=False, phase=0.0):
    """A round tube along a 3D polyline; `phase` turns its facets so none
    lies flat against a face it runs along."""
    bm = p.bm
    n = len(pts)
    pts = [Vector(q) for q in pts]
    rings = []
    for i in range(n):
        a = pts[i - 1] if closed or i > 0 else pts[i]
        b = pts[(i + 1) % n] if closed or i < n - 1 else pts[i]
        t = (b - a).normalized()
        up = Vector((0, 1, 0)) if abs(t.y) < 0.9 else Vector((1, 0, 0))
        u = t.cross(up).normalized()
        w = t.cross(u).normalized()
        rings.append([bm.verts.new(pts[i] + (u * math.cos(2 * math.pi * k / sides + phase) +
                                             w * math.sin(2 * math.pi * k / sides + phase)) * r)
                      for k in range(sides)])
    faces = []
    last = n if closed else n - 1
    for i in range(last):
        ra, rb = rings[i], rings[(i + 1) % n]
        for k in range(sides):
            k1 = (k + 1) % sides
            faces.append(bm.faces.new((ra[k], ra[k1], rb[k1], rb[k])))
    if not closed:
        faces.append(bm.faces.new(rings[0]))
        faces.append(bm.faces.new(list(reversed(rings[-1]))))
    p._finish_piece([v for r_ in rings for v in r_], faces, mat)


def add_bolt(body, vanes, cut, xf, roll=0.0, segs=12):
    """One bolt in bolt space, placed by `xf` and turned `roll` about its
    axis: rod, filed point and thread whippings into `body`, the vanes into
    `vanes`, the nock notch into the cutter `cut` (cut from `body`)."""
    x = xf @ RZ(roll)
    # The rod in pieces, the thread whippings that bind the vanes' ends
    # between them (so no face lies just over another).
    for z0, z1, r, mat in ((-0.165, 0.095, ROD_R, "Steel"), (0.095, 0.102, 0.0047, "Black"),
                           (0.102, 0.165, ROD_R, "Steel"), (0.165, 0.171, 0.0047, "Black")):
        body.cyl(r, z0, z1, mat, segs=segs, xf=x)
    body.cyl(ROD_R, 0.171, HALF, "Steel", segs=segs, xf=x, chamfer=0.0007)
    # Filed point: four flat facets ground on, their shoulders showing.
    body.lathe([(0.0, -0.160), (0.0042, -0.160), (0.0, -HALF)], "Steel", segs=4,
               xf=x @ RZ(math.pi / 4))
    # Three vanes, the odd (cock) vane down.
    for k in range(3):
        a = math.pi + k * 2 * math.pi / 3
        vanes.prism(VANE, -0.0003, 0.0003, "Polymer" if k == 0 else "Clear", axis="x",
                    xf=x @ RZ(a))
    # The nock: a notch sawn across X into the back of the rod.
    cut.box((0.02, 0.003, 2 * NOCK_DEPTH), (0.0, 0.0, HALF), "Steel", xf=x)


def on_floor(parts, half_height):
    """Centre the parts on the box and drop them onto its floor."""
    vs = [v.co for p in parts for v in p.bm.verts]
    lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
    hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
    c = (lo + hi) / 2
    return transform(parts, T(-c.x, -half_height - lo.y, -c.z))


def crossbow_bolt():
    body, vanes, cut = Part("body"), Part("vanes"), Part("cut")
    add_bolt(body, vanes, cut, T())
    body.cut(cut)
    # dims (0.03, 0.03, 0.38): resting on the box floor.
    return on_floor([body, vanes], 0.015)


# --- The bundle -------------------------------------------------------------------------------
# Five bolts, three below and two in the hollows above, all pointing the
# same way, each turned so its vanes splay out between its neighbours.
GAP = 2 * ROD_R + 0.0004
CENTRES = [(-GAP, 0.0), (0.0, 0.0), (GAP, 0.0), (-GAP / 2, GAP * 0.866), (GAP / 2, GAP * 0.866)]


def _seg_dist(p, a, b):
    ab = b - a
    t = max(0.0, min(1.0, (p - a).dot(ab) / max(ab.dot(ab), 1e-12)))
    return (a + ab * t - p).length


def _seg_seg(a, b, c, d):
    return min(_seg_dist(a, c, d), _seg_dist(b, c, d), _seg_dist(c, a, b), _seg_dist(d, a, b))


def _vanes2d(c, roll):
    out = []
    for k in range(3):
        a = roll + math.pi + k * 2 * math.pi / 3
        d = Vector((-math.sin(a), math.cos(a)))
        out.append((c + d * ROD_R, c + d * VANE_TIP))
    return out


def bundle_rolls():
    """Greedy: turn each bolt in 5 degree steps so its vanes keep clear of
    the other rods and of the vanes already placed, inside the box."""
    cs = [Vector(c) for c in CENTRES]
    placed, rolls = [], []
    for i, c in enumerate(cs):
        best = None
        for step in range(72):
            roll = math.radians(5 * step)
            vs = _vanes2d(c, roll)
            score = 1.0
            for a, b in vs:
                for j, o in enumerate(cs):
                    if j != i:
                        score = min(score, _seg_dist(o, a, b) - ROD_R)
                for e, f in placed:
                    score = min(score, _seg_seg(a, b, e, f))
                # Keep inside 0.045 x 0.04.
                if abs(b.x) > 0.0225 - 0.0004:
                    score -= 1.0
            if best is None or score > best[0]:
                best = (score, roll, vs)
        rolls.append(best[1])
        placed += best[2]
    return rolls


def hull_loop(z, R, n=28):
    """The tight loop of a wire round the rods: the envelope of circles of
    radius R about each rod's axis, at z."""
    cs = [Vector(c) for c in CENTRES]
    pts = []
    for k in range(n):
        a = 2 * math.pi * k / n
        d = Vector((math.cos(a), math.sin(a)))
        c = max(cs, key=lambda q: q.dot(d))
        q = c + d * R
        pts.append((q.x, q.y, z))
    return pts


def bolt_bundle():
    body, vanes, cut, wire = Part("body"), Part("vanes"), Part("cut"), Part("wire")
    for (x, y), roll in zip(CENTRES, bundle_rolls()):
        add_bolt(body, vanes, cut, T(x, y, 0.0), roll=roll, segs=8)
    body.cut(cut)
    wr = 0.0008
    top = max(c[1] for c in CENTRES) + ROD_R + wr
    for z in (-0.09, 0.065):
        tube3d(wire, hull_loop(z, ROD_R + wr * 0.8), wr, "Copper", sides=5, closed=True,
               phase=0.3)
        # The ends twisted together and the twist folded down along the
        # bolts on top.
        for s in (0, 1):
            pts = []
            for i in range(9):
                u = i / 8
                a = s * math.pi + u * 4 * math.pi
                pts.append((0.0011 * math.cos(a), top + wr + 0.0011 * math.sin(a) * 0.6,
                            z + 0.002 + 0.016 * u))
            tube3d(wire, pts, wr * 0.85, "Copper", sides=4)
    # dims (0.045, 0.04, 0.38): resting on the box floor.
    return on_floor([body, vanes, wire], 0.02)


if __name__ == "__main__":
    for item_id, fn in (("crossbow_bolt", crossbow_bolt), ("bolt_bundle", bolt_bundle)):
        if wanted(item_id):
            item(item_id, fn)
