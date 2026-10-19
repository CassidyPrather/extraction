"""Shared bits for the item scripts: each item is its own .glb under
assets/models/items/<item id>.glb, built in item space -- metres, origin at
the centre of the item's `dims` box from assets/data/items.ron (or a cube
of side cbrt(bulk / 1000) when it has none), +Y up, -Z forward (where a
light shines or a scope looks)."""

import math
import os
import sys

import bmesh
from mathutils import Vector

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, RZ, T, export, fillet, make, report, reset  # noqa: E402

UP = RX(-math.pi / 2)       # lathe/cyl axis (Z) -> +Y
ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> +X
ON_TOP = RY(math.pi) @ RX(-math.pi / 2)  # `text` lying face up, read from the front (-Z)
FRONT = RY(math.pi)                      # `text` facing the front (-Z)


def cube(bulk):
    """Side of the default box for an item without dims."""
    return (bulk / 1000.0) ** (1.0 / 3.0)


def item(item_id, build):
    """Build one item in a fresh scene and write its .glb. `build` returns
    the Parts to make."""
    reset()
    parts = build()
    for p in parts:
        make(p)
    print(f"{item_id}:")
    report()
    export(f"assets/models/items/{item_id}.glb")


def transform(parts, m):
    """Apply a game-space matrix to whole Parts (and their cutters)."""
    for p in parts:
        for q in [p] + p.cutters:
            bmesh.ops.transform(q.bm, matrix=m, verts=q.bm.verts)
    return parts


def centred(parts):
    """Move Parts so their joint bounds are centred on the origin, as the
    item's `dims` box is (so the model rests on the box's floor)."""
    vs = [v.co for p in parts for v in p.bm.verts]
    lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
    hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
    return transform(parts, T(*(-(lo + hi) / 2)))


def wanted(item_id):
    """Scripts can be run for just some items: `-- id1 id2`."""
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or item_id in argv


def cell_bay(p, r_tube, mat, y=0.0, bay=(0.0095, 0.0305), wall=0.003):
    """A power cell bay: two cheeks either side of an open vertical bore
    round the cell's seat at z 0.02, the body's tube left out between
    `bay` (a z range)."""
    z0, z1 = bay
    h = r_tube * 2 + 0.002
    for sx in (-1, 1):
        prof = fillet([(z0 - 0.003, y - h / 2), (z1 + 0.003, y - h / 2), (z1 + 0.003, y + h / 2),
                       (z0 - 0.003, y + h / 2)], 0.003, 2)
        a, b = sorted((sx * 0.0103, sx * (0.0103 + wall)))
        p.prism(prof, a, b, mat, axis="x", bevel=0.001, segs=1)
    # Spring contacts top and bottom, seen through the bore.
    p.box((0.016, 0.002, 0.004), (0.0, y + h / 2 - 0.001, z0 - 0.001), "Steel")
    p.box((0.016, 0.002, 0.004), (0.0, y - h / 2 + 0.001, z1 + 0.001), "Steel")


def blob(p, r, centre, mat, scale=(1, 1, 1), bumps=(), segs=20, rings=10):
    """A sphere pushed about by a few smooth bumps: (direction, amount)."""
    bm = p.bm
    res = bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=r)
    verts = res["verts"]
    for v in verts:
        n = v.co.normalized()
        d = 0.0
        for (dx, dy, dz), amt in bumps:
            dd = Vector((dx, dy, dz)).normalized()
            d += amt * max(0.0, n.dot(dd)) ** 2
        v.co = v.co * (1.0 + d)
        v.co = Vector((v.co.x * scale[0], v.co.y * scale[1], v.co.z * scale[2])) + Vector(centre)
    faces = list({f for v in verts for f in v.link_faces})
    p._finish_piece(verts, faces, mat)


def tube_path(p, pts, r, mat, sides=8, closed=True):
    """A round tube along a 3D polyline."""
    bm = p.bm
    n = len(pts)
    rings = []
    for i in range(n):
        a = Vector(pts[i - 1] if closed or i > 0 else pts[i])
        b = Vector(pts[(i + 1) % n] if closed or i < n - 1 else pts[i])
        t = (b - a).normalized()
        up = Vector((0, 1, 0)) if abs(t.y) < 0.9 else Vector((1, 0, 0))
        u = t.cross(up).normalized()
        w = t.cross(u).normalized()
        c = Vector(pts[i])
        rings.append([bm.verts.new(c + (u * math.cos(2 * math.pi * k / sides) +
                                        w * math.sin(2 * math.pi * k / sides)) * r)
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


def ragged(p, w, z0, depth, y0, y1, mat, xf=None, n=7, seed=1):
    """A torn paper edge: a strip `w` wide across X from z0, its free edge
    jagged up to `depth` toward -Z, between y0 and y1 (then `xf`)."""
    pts = [(w / 2, z0), (-w / 2, z0)]
    for k in range(n + 1):
        x = -w / 2 + w * k / n
        d = depth * (0.35 + 0.65 * ((k * 7 + seed * 3) % 5) / 4.0)
        pts.append((x, z0 - d))
    p.prism(pts, y0, y1, mat, axis="y", xf=xf)


def ring(p, R, r, mat, xf=None, segs=20, sides=6):
    """A torus round the Z axis (then `xf`): ring radius R, wire radius r."""
    prof = [(R + r * math.cos(2 * math.pi * k / sides), r * math.sin(2 * math.pi * k / sides))
            for k in range(sides)]
    p.lathe(prof, mat, segs=segs, xf=xf, closed=True)


# Stencil strokes on a 2 x 4 cell, (x, y) with y up.
_O = [((0, 0), (2, 0)), ((2, 0), (2, 4)), ((2, 4), (0, 4)), ((0, 4), (0, 0))]
GLYPHS = {
    "0": _O, "O": _O,
    "1": [((1, 0), (1, 4)), ((1, 4), (0.3, 3.3))],
    "2": [((0, 4), (2, 4)), ((2, 4), (2, 2)), ((2, 2), (0, 2)), ((0, 2), (0, 0)), ((0, 0), (2, 0))],
    "3": [((0, 4), (2, 4)), ((2, 4), (2, 0)), ((2, 0), (0, 0)), ((0.6, 2), (2, 2))],
    "4": [((1.5, 0), (1.5, 4)), ((1.5, 4), (0, 1.3)), ((0, 1.3), (2, 1.3))],
    "5": [((2, 4), (0, 4)), ((0, 4), (0, 2)), ((0, 2), (2, 2)), ((2, 2), (2, 0)), ((2, 0), (0, 0))],
    "6": [((2, 4), (0, 4)), ((0, 4), (0, 0)), ((0, 0), (2, 0)), ((2, 0), (2, 2)), ((2, 2), (0, 2))],
    "7": [((0, 4), (2, 4)), ((2, 4), (0.8, 0))],
    "8": _O + [((0, 2), (2, 2))],
    "9": [((2, 2), (0, 2)), ((0, 2), (0, 4)), ((0, 4), (2, 4)), ((2, 4), (2, 0)), ((2, 0), (0, 0))],
    "x": [((0.2, 0), (1.8, 2.4)), ((0.2, 2.4), (1.8, 0))],
    ".": [((1, 0), (1, 0.3))],
    "-": [((0.3, 2), (1.7, 2))],
    "A": [((0, 0), (1, 4)), ((1, 4), (2, 0)), ((0.5, 1.6), (1.5, 1.6))],
    "C": [((2, 4), (0, 4)), ((0, 4), (0, 0)), ((0, 0), (2, 0))],
    "D": [((0, 0), (0, 4)), ((0, 4), (1.3, 4)), ((1.3, 4), (2, 3.2)), ((2, 3.2), (2, 0.8)),
          ((2, 0.8), (1.3, 0)), ((1.3, 0), (0, 0))],
    "E": [((0, 0), (0, 4)), ((0, 4), (2, 4)), ((0, 2), (1.5, 2)), ((0, 0), (2, 0))],
    "G": [((2, 4), (0, 4)), ((0, 4), (0, 0)), ((0, 0), (2, 0)), ((2, 0), (2, 2)), ((2, 2), (1, 2))],
    "I": [((1, 0), (1, 4))],
    "K": [((0, 0), (0, 4)), ((0, 2), (2, 4)), ((0.7, 2.6), (2, 0))],
    "L": [((0, 4), (0, 0)), ((0, 0), (2, 0))],
    "M": [((0, 0), (0, 4)), ((0, 4), (1, 2)), ((1, 2), (2, 4)), ((2, 4), (2, 0))],
    "N": [((0, 0), (0, 4)), ((0, 4), (2, 0)), ((2, 0), (2, 4))],
    "P": [((0, 0), (0, 4)), ((0, 4), (2, 4)), ((2, 4), (2, 2)), ((2, 2), (0, 2))],
    "R": [((0, 0), (0, 4)), ((0, 4), (2, 4)), ((2, 4), (2, 2)), ((2, 2), (0, 2)),
          ((0.8, 2), (2, 0))],
    "T": [((0, 4), (2, 4)), ((1, 4), (1, 0))],
    "W": [((0, 4), (0.5, 0)), ((0.5, 0), (1, 2)), ((1, 2), (1.5, 0)), ((1.5, 0), (2, 4))],
    "Y": [((0, 4), (1, 2)), ((2, 4), (1, 2)), ((1, 2), (1, 0))],
    "X": [((0, 0), (2, 4)), ((0, 4), (2, 0))],
    "H": [((0, 0), (0, 4)), ((2, 0), (2, 4)), ((0, 2), (2, 2))],
    # Cyrillic, for Soviet labels (the Latin look-alikes above serve for
    # А, В (8), Е, К, М, Н, О, Р (P), С (C), Т, Х (X)).
    "Б": [((2, 4), (0, 4)), ((0, 4), (0, 0)), ((0, 0), (2, 0)), ((2, 0), (2, 2)),
          ((2, 2), (0, 2))],
    "В": _O + [((0, 2), (2, 2))],
    "Г": [((0, 0), (0, 4)), ((0, 4), (2, 4))],
    "Д": [((0, 0), (2, 0)), ((0.4, 0), (0.7, 4)), ((0.7, 4), (1.7, 4)), ((1.7, 4), (1.7, 0)),
          ((0, 0), (0, -0.7)), ((2, 0), (2, -0.7))],
    "И": [((0, 4), (0, 0)), ((0, 0), (2, 4)), ((2, 4), (2, 0))],
    "Й": [((0, 4), (0, 0)), ((0, 0), (2, 4)), ((2, 4), (2, 0)), ((0.6, 4.7), (1.4, 4.7))],
    "Л": [((0, 0), (0.7, 4)), ((0.7, 4), (2, 4)), ((2, 4), (2, 0))],
    "П": [((0, 0), (0, 4)), ((0, 4), (2, 4)), ((2, 4), (2, 0))],
    "У": [((0, 4), (1, 1.6)), ((2, 4), (0.4, 0))],
    "Ф": [((1, 0), (1, 4)), ((0, 1), (2, 1)), ((2, 1), (2, 3)), ((2, 3), (0, 3)),
          ((0, 3), (0, 1))],
    "Ц": [((0, 4), (0, 0)), ((0, 0), (2, 0)), ((1.7, 4), (1.7, 0)), ((2, 0), (2, -0.7))],
    "Ч": [((0, 4), (0, 2)), ((0, 2), (2, 2)), ((2, 4), (2, 0))],
    "Ш": [((0, 4), (0, 0)), ((0, 0), (2, 0)), ((1, 0), (1, 4)), ((2, 0), (2, 4))],
    "Ы": [((0, 4), (0, 0)), ((0, 0), (1.1, 0)), ((1.1, 0), (1.1, 2)), ((1.1, 2), (0, 2)),
          ((2, 4), (2, 0))],
    "Ь": [((0, 4), (0, 0)), ((0, 0), (2, 0)), ((2, 0), (2, 2)), ((2, 2), (0, 2))],
    "Я": [((2, 0), (2, 4)), ((2, 4), (0, 4)), ((0, 4), (0, 2)), ((0, 2), (2, 2)),
          ((1.2, 2), (0, 0))],
}
LOOKALIKE = {"А": "A", "Е": "E", "К": "K", "М": "M", "Н": "H", "О": "O", "Р": "P", "С": "C",
             "Т": "T", "Х": "X"}


def text(p, s, h, mat, xf, depth=0.0006, align="centre"):
    """Stencil letters `h` high, drawn in the XY plane facing +Z (raised
    `depth` off z 0), then placed by `xf`. Unknown characters are spaces."""
    k = h / 4.0
    stroke = max(h * 0.15, 0.0005)
    adv = 2.9 * k
    width = adv * len(s) - 0.9 * k
    x0 = {"centre": -width / 2, "left": 0.0, "right": -width}[align]
    for i, ch in enumerate(s):
        for (ax, ay), (bx, by) in GLYPHS.get(LOOKALIKE.get(ch, ch), []):
            ax_, ay_ = x0 + i * adv + ax * k, ay * k - h / 2
            bx_, by_ = x0 + i * adv + bx * k, by * k - h / 2
            L = math.hypot(bx_ - ax_, by_ - ay_)
            a = math.atan2(by_ - ay_, bx_ - ax_)
            p.box((L + stroke, stroke, depth), (0.0, 0.0, depth / 2), mat,
                  xf=xf @ T((ax_ + bx_) / 2, (ay_ + by_) / 2, 0.0) @ RZ(a))


def text_round(p, s, h, mat, r, y, xf=None, depth=0.0006, at=0.0):
    """Stencil letters round a can: on a cylinder of radius r about Y, at
    height y, centred on the front (-Z) turned by `at` radians, read from
    outside (then `xf`)."""
    base = xf if xf is not None else T()
    k = h / 4.0
    adv = 2.9 * k
    width = adv * len(s) - 0.9 * k
    for i, ch in enumerate(s):
        u = -width / 2 + i * adv + k
        text(p, ch, h, mat, base @ RY(at + u / r) @ T(0.0, y, -r) @ FRONT, depth=depth)


def panel(p, axis, a0, a1, b0, b1, t0, t1, base, patches=(), xf=None):
    """A flat slab whose printing is part of its surface rather than stuck
    on it, so no face lies over another: one closed piece whose top face
    is a grid of cells, each the colour of the last patch (a0, a1, b0, b1,
    mat) over it, else `base`. axis 'y': (a, b) is (x, z) and the slab
    runs t0..t1 in y (t1 the printed face); 'x': (z, y) and x; 'z': (x, y)
    and z."""
    cuts_a = sorted({a0, a1} | {min(max(v, a0), a1) for q in patches for v in q[0:2]})
    cuts_b = sorted({b0, b1} | {min(max(v, b0), b1) for q in patches for v in q[2:4]})
    cuts_a = [v for i, v in enumerate(cuts_a) if i == 0 or v - cuts_a[i - 1] > 1e-6]
    cuts_b = [v for i, v in enumerate(cuts_b) if i == 0 or v - cuts_b[i - 1] > 1e-6]

    def co(a, b, t):
        if axis == "y":
            return Vector((a, t, b))
        if axis == "x":
            return Vector((t, b, a))
        return Vector((a, b, t))

    bm = p.bm
    na, nb = len(cuts_a), len(cuts_b)
    top = [[bm.verts.new(co(cuts_a[i], cuts_b[j], t1)) for j in range(nb)] for i in range(na)]
    ring_ij = ([(i, 0) for i in range(na)] + [(na - 1, j) for j in range(1, nb)] +
               [(i, nb - 1) for i in range(na - 2, -1, -1)] +
               [(0, j) for j in range(nb - 2, 0, -1)])
    rim = [top[i][j] for i, j in ring_ij]
    under = [bm.verts.new(v.co.copy()) for v in rim]
    for v in under:
        if axis == "y":
            v.co.y = t0
        elif axis == "x":
            v.co.x = t0
        else:
            v.co.z = t0
    faces = []
    for i in range(na - 1):
        for j in range(nb - 1):
            ca, cb = (cuts_a[i] + cuts_a[i + 1]) / 2, (cuts_b[j] + cuts_b[j + 1]) / 2
            mat = base
            for q in patches:
                if q[0] <= ca <= q[1] and q[2] <= cb <= q[3]:
                    mat = q[4]
            f = bm.faces.new((top[i][j], top[i + 1][j], top[i + 1][j + 1], top[i][j + 1]))
            f.material_index = p._mi(mat)
            faces.append(f)
    side = []
    n = len(rim)
    for k in range(n):
        side.append(bm.faces.new((rim[k], rim[(k + 1) % n], under[(k + 1) % n], under[k])))
    side.append(bm.faces.new(list(reversed(under))))
    for f in side:
        f.material_index = p._mi(base)
    verts = [v for row in top for v in row] + under
    if xf is not None:
        for v in verts:
            v.co = xf @ v.co
    allf = faces + side
    bmesh.ops.recalc_face_normals(bm, faces=allf)
    vol = 0.0
    for f in allf:
        vs = [v.co for v in f.verts]
        for i in range(1, len(vs) - 1):
            vol += vs[0].dot(vs[i].cross(vs[i + 1]))
    if vol < 0:
        bmesh.ops.reverse_faces(bm, faces=allf)
