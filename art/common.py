"""Shared modelling kit for the procedural gun and robot models.

Run inside Blender (`blender -b --python art/guns/ak74.py`). Everything is
built in **game space** -- metres, +X right, +Y up, -Z forward (down the
bore for guns) -- inside a bmesh, then converted to Blender's Z-up space
when the object is made, so the glTF exporter's Y-up conversion lands it
back in game space. Coordinates in the model scripts therefore read the
same as `src/weapons/layout.rs` and the robot spawn code.

A `Part` is one exported mesh object (one glTF node). Its name tells the
game how it moves (`static*`, `bolt*`, `turnbolt*`, `barrels*`, `trigger*`,
`selector*` for guns; part names for robots), and its origin is where it
moves from. Faces carry material names (`Steel`, `Blued`, `Wood`, ...),
which the game swaps for its own PBR materials.

UVs are box-projected in metres (1 UV unit = 1 m), with U running along
the gun (game Z) on the sides and top, so tiling scans line up and wood
grain follows the stock.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Game (x, y, z) -> Blender (x, -z, y). A proper rotation (det +1).
TO_BLENDER = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))

# Preview colours only; the game replaces these by name (src/models.rs,
# `MATERIAL_NAMES`: keep the two lists the same; a test checks).
MATERIALS = {
    "Steel": ((0.42, 0.42, 0.43), 1.0, 0.35, None),
    "Blued": ((0.05, 0.055, 0.065), 0.9, 0.38, None),
    "Wood": ((0.30, 0.13, 0.05), 0.0, 0.5, None),
    "Polymer": ((0.24, 0.08, 0.09), 0.0, 0.55, None),
    "Bakelite": ((0.17, 0.06, 0.035), 0.0, 0.42, None),
    "Black": ((0.025, 0.025, 0.025), 0.5, 0.5, None),
    "Brass": ((0.75, 0.55, 0.25), 1.0, 0.3, None),
    "Shell": ((0.84, 0.85, 0.86), 0.0, 0.32, None),
    "Dark": ((0.06, 0.065, 0.07), 0.7, 0.4, None),
    "Eye": ((0.1, 0.02, 0.02), 0.0, 0.3, (1.0, 0.14, 0.07)),
    "Rotor": ((0.06, 0.06, 0.065), 0.3, 0.45, None),
    # The truck, gadgets and the rest (src/models.rs maps them).
    "Paint": ((0.2, 0.24, 0.13), 0.3, 0.6, None),
    "Canvas": ((0.3, 0.31, 0.21), 0.0, 0.95, None),
    "Planks": ((0.38, 0.33, 0.27), 0.0, 0.85, None),
    "Boards": ((0.35, 0.27, 0.2), 0.0, 0.7, None),
    "Iron": ((0.09, 0.09, 0.09), 0.8, 0.55, None),
    "Rubber": ((0.07, 0.07, 0.07), 0.0, 0.9, None),
    "Glass": ((0.1, 0.12, 0.13), 0.0, 0.05, None),
    "Lamp": ((0.9, 0.85, 0.7), 0.0, 0.2, (1.0, 0.85, 0.6)),
    "LampRed": ((0.5, 0.03, 0.02), 0.0, 0.2, (1.0, 0.05, 0.02)),
    "Lens": ((0.08, 0.1, 0.14), 0.5, 0.05, None),
    "Copper": ((0.72, 0.38, 0.22), 1.0, 0.35, None),
    "Glow": ((0.1, 0.4, 0.5), 0.0, 0.3, (0.2, 0.8, 1.0)),
    "Rope": ((0.5, 0.42, 0.28), 0.0, 0.9, None),
    "Cream": ((0.82, 0.78, 0.66), 0.0, 0.6, None),
    "Olive": ((0.25, 0.27, 0.17), 0.0, 0.8, None),
    "Clear": ((0.8, 0.82, 0.8), 0.0, 0.1, None),
    "Warn": ((0.9, 0.6, 0.1), 0.0, 0.4, (1.0, 0.55, 0.07)),
    "Beacon": ((1.0, 0.75, 0.4), 0.0, 0.3, (1.0, 0.6, 0.2)),
    "Yellow": ((0.9, 0.74, 0.16), 0.0, 0.55, None),
    "Orange": ((0.86, 0.38, 0.08), 0.0, 0.6, None),
    "Blue": ((0.1, 0.24, 0.55), 0.0, 0.5, None),
    "Leaf": ((0.2, 0.4, 0.1), 0.0, 0.7, None),
    "Enamel": ((0.88, 0.88, 0.85), 0.0, 0.3, None),
    "Worn": ((0.5, 0.5, 0.5), 1.0, 0.45, None),
    "Red": ((0.5, 0.06, 0.04), 0.0, 0.6, None),
    "Pebble": ((0.42, 0.42, 0.44), 0.0, 0.45, None),
    "Char": ((0.1, 0.092, 0.085), 0.0, 0.9, None),
    "Ember": ((0.9, 0.35, 0.08), 0.0, 0.5, (1.0, 0.35, 0.05)),
    "Paper": ((0.85, 0.83, 0.76), 0.0, 0.9, None),
    "Strange": ((0.5, 0.35, 0.8), 0.6, 0.15, (0.6, 0.35, 1.0)),
    # World props (art/props; src/models.rs maps them).
    "Timber": ((0.55, 0.45, 0.33), 0.0, 0.8, None),
    "Rusty": ((0.4, 0.25, 0.15), 0.6, 0.7, None),
    "Painted": ((0.3, 0.38, 0.35), 0.3, 0.6, None),
    "Cast": ((0.07, 0.07, 0.07), 0.6, 0.7, None),
    "Wool": ((0.27, 0.26, 0.23), 0.0, 0.95, None),
    "Ticking": ((0.62, 0.6, 0.55), 0.0, 0.9, None),
    "Plaster": ((0.62, 0.6, 0.55), 0.0, 0.9, None),
    "Concrete": ((0.45, 0.45, 0.43), 0.0, 0.85, None),
    "Zinc": ((0.5, 0.52, 0.53), 0.9, 0.45, None),
    "Straw": ((0.58, 0.5, 0.3), 0.0, 0.95, None),
    # The gloves (art/hands; src/models.rs maps them).
    "Glove": ((0.30, 0.29, 0.23), 0.0, 0.9, None),
    "Leather": ((0.33, 0.27, 0.2), 0.0, 0.7, None),
    "Padding": ((0.17, 0.17, 0.16), 0.0, 0.75, None),
}


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name):
    m = bpy.data.materials.get(name)
    if m:
        return m
    col, metal, rough, emit = MATERIALS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = (*col, 1.0)
        bsdf.inputs["Metallic"].default_value = metal
        bsdf.inputs["Roughness"].default_value = rough
        if emit:
            bsdf.inputs["Emission Color"].default_value = (*emit, 1.0)
            bsdf.inputs["Emission Strength"].default_value = 6.0
    return m


# ---------------------------------------------------------------------------
# 2D helpers (profiles are lists of (a, b) tuples)


def fillet(points, radius, segments=4, closed=True):
    """Round a polygon's corners. `radius` is a number or a per-point list
    (0 keeps a corner sharp)."""
    n = len(points)
    radii = radius if isinstance(radius, (list, tuple)) else [radius] * n
    out = []
    for i in range(n):
        p = Vector(points[i])
        r = radii[i]
        if r <= 0 or (not closed and (i == 0 or i == n - 1)):
            out.append(tuple(p))
            continue
        a = Vector(points[i - 1])
        b = Vector(points[(i + 1) % n])
        da = (a - p)
        db = (b - p)
        la, lb = da.length, db.length
        if la < 1e-9 or lb < 1e-9:
            out.append(tuple(p))
            continue
        da /= la
        db /= lb
        cosang = max(-1.0, min(1.0, da.dot(db)))
        ang = math.acos(cosang)
        if ang < 1e-3 or ang > math.pi - 1e-3:
            out.append(tuple(p))
            continue
        t = r / math.tan(ang / 2)
        t = min(t, la * 0.49, lb * 0.49)
        r_eff = t * math.tan(ang / 2)
        p0 = p + da * t
        p1 = p + db * t
        bis = (da + db).normalized()
        centre = p + bis * (r_eff / math.sin(ang / 2))
        v0 = p0 - centre
        v1 = p1 - centre
        a0 = math.atan2(v0.y, v0.x)
        a1 = math.atan2(v1.y, v1.x)
        d = a1 - a0
        while d > math.pi:
            d -= 2 * math.pi
        while d < -math.pi:
            d += 2 * math.pi
        for k in range(segments + 1):
            aa = a0 + d * k / segments
            out.append((centre.x + r_eff * math.cos(aa), centre.y + r_eff * math.sin(aa)))
    return out


def arc(cx, cy, r, a0, a1, n):
    """Points on an arc, angles in degrees, inclusive."""
    return [
        (cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
         cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / n)))
        for i in range(n + 1)
    ]


def catmull(points, samples=4, closed=True):
    """A Catmull-Rom spline through the points."""
    pts = [Vector(p) for p in points]
    n = len(pts)
    out = []
    rng = range(n) if closed else range(n - 1)
    for i in rng:
        p0 = pts[(i - 1) % n] if closed or i > 0 else pts[0]
        p1 = pts[i]
        p2 = pts[(i + 1) % n]
        p3 = pts[(i + 2) % n] if closed or i + 2 < n else pts[-1]
        for s in range(samples):
            t = s / samples
            t2, t3 = t * t, t * t * t
            q = 0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2
                       + (-p0 + 3 * p1 - 3 * p2 + p3) * t3)
            out.append((q.x, q.y))
    if not closed:
        out.append(tuple(pts[-1]))
    return out


def area2(points):
    s = 0.0
    for i in range(len(points)):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % len(points)]
        s += x0 * y1 - x1 * y0
    return s * 0.5


# ---------------------------------------------------------------------------
# Transforms in game space


def T(x=0.0, y=0.0, z=0.0):
    return Matrix.Translation((x, y, z))


def RX(a, about=None):
    m = Matrix.Rotation(a, 4, "X")
    if about is not None:
        return T(*about) @ m @ T(*(-Vector(about)))
    return m


def RY(a, about=None):
    m = Matrix.Rotation(a, 4, "Y")
    if about is not None:
        return T(*about) @ m @ T(*(-Vector(about)))
    return m


def RZ(a, about=None):
    m = Matrix.Rotation(a, 4, "Z")
    if about is not None:
        return T(*about) @ m @ T(*(-Vector(about)))
    return m


def S(x, y, z):
    return Matrix.Diagonal((x, y, z, 1.0))


# ---------------------------------------------------------------------------
# Parts


class Part:
    """One mesh object being built, in game space."""

    def __init__(self, name, origin=(0.0, 0.0, 0.0)):
        self.name = name
        self.origin = Vector(origin)
        self.bm = bmesh.new()
        self.mats = []
        self.cutters = []

    def _mi(self, mat):
        if mat not in self.mats:
            self.mats.append(mat)
        return self.mats.index(mat)

    def _finish_piece(self, verts, faces, mat, xf=None, bevel=0.0, segs=2, bevel_edges=None):
        bm = self.bm
        if xf is not None:
            for v in verts:
                v.co = xf @ v.co
        mi = self._mi(mat)
        for f in faces:
            f.material_index = mi
        bmesh.ops.recalc_face_normals(bm, faces=faces)
        # recalc can pick the wrong side when a cap self-intersects (a big
        # rounding on a fine outline); a closed piece must have positive
        # volume, so flip it if not.
        edges = {e for f in faces for e in f.edges}
        if faces and all(e.is_manifold for e in edges):
            vol = 0.0
            for f in faces:
                vs = [v.co for v in f.verts]
                for i in range(1, len(vs) - 1):
                    vol += vs[0].dot(vs[i].cross(vs[i + 1]))
            if vol < 0:
                bmesh.ops.reverse_faces(bm, faces=faces)
        if bevel > 0:
            edges = bevel_edges if bevel_edges is not None else list(
                {e for f in faces for e in f.edges})
            edges = [e for e in edges if e.is_valid]
            res = bmesh.ops.bevel(bm, geom=edges, offset=bevel, segments=segs, profile=0.5,
                                  affect="EDGES", clamp_overlap=True)
            for f in res["faces"]:
                f.material_index = mi
        return verts, faces

    # -- primitives ---------------------------------------------------------

    def prism(self, profile, a0, a1, mat, axis="x", bevel=0.0, segs=2, xf=None,
              width=None, taper=None):
        """Extrude a closed 2D profile between a0 and a1 along `axis`.

        axis 'x': profile is (z, y), the side view of a gun.
        axis 'z': profile is (x, y), a cross-section along the bore.
        axis 'y': profile is (x, z), a plan view.
        `width(a, b) -> (lo, hi)` overrides a0/a1 per profile point, for
        stocks that thicken and thin. Only the outline edges of the two caps
        are bevelled; round the profile itself with `fillet`.
        """
        bm = self.bm
        if area2(profile) < 0:
            profile = list(reversed(profile))

        def co(p, w):
            a, b = p
            if axis == "x":
                return Vector((w, b, a))
            if axis == "z":
                return Vector((a, b, w))
            return Vector((a, w, b))

        if bevel > 0:
            return self._slab(profile, a0, a1, mat, co, bevel, max(1, segs), xf, width)
        lo_v, hi_v = [], []
        for p in profile:
            lo, hi = (a0, a1) if width is None else width(*p)
            lo_v.append(bm.verts.new(co(p, lo)))
            hi_v.append(bm.verts.new(co(p, hi)))
        faces = [bm.faces.new(lo_v), bm.faces.new(list(reversed(hi_v)))]
        n = len(profile)
        for i in range(n):
            j = (i + 1) % n
            faces.append(bm.faces.new((lo_v[i], lo_v[j], hi_v[j], hi_v[i])))
        cap_edges = [e for f in faces[:2] for e in f.edges]
        return self._finish_piece(lo_v + hi_v, faces, mat, xf, bevel, segs, cap_edges)

    def _slab(self, profile, a0, a1, mat, co, r, segs, xf, width):
        """An extrusion whose outline edges on both caps are rounded with
        radius r, built directly (bevel ops clamp to nothing on finely
        filleted outlines)."""
        bm = self.bm
        pts = [Vector(p) for p in profile]
        n = len(pts)
        rings = []
        for i in range(n):
            p = pts[i]
            e0 = (p - pts[i - 1]).normalized()
            e1 = (pts[(i + 1) % n] - p).normalized()
            # Inward normals of a CCW outline are the left normals.
            n0 = Vector((-e0.y, e0.x))
            n1 = Vector((-e1.y, e1.x))
            nm = n0 + n1
            nm = nm.normalized() if nm.length > 1e-9 else n0
            miter = 1.0 / max(0.4, nm.dot(n0))
            lo, hi = (a0, a1) if width is None else width(p.x, p.y)
            rr = min(r, (hi - lo) * 0.45)
            ring = []
            for k in range(segs + 1):
                phi = math.pi / 2 * k / segs
                w = hi - rr + rr * math.cos(phi)
                inset = r - r * math.sin(phi)
                q = p + nm * inset * miter
                ring.append(bm.verts.new(co((q.x, q.y), w)))
            for k in range(1, segs + 1):
                phi = math.pi / 2 + math.pi / 2 * k / segs
                w = lo + rr + rr * math.cos(phi)
                inset = r - r * math.sin(phi)
                q = p + nm * inset * miter
                ring.append(bm.verts.new(co((q.x, q.y), w)))
            rings.append(ring)
        faces = []
        m = len(rings[0])
        for i in range(n):
            ra, rb = rings[i], rings[(i + 1) % n]
            for k in range(m - 1):
                faces.append(bm.faces.new((ra[k], rb[k], rb[k + 1], ra[k + 1])))
        faces.append(bm.faces.new([r_[0] for r_ in rings]))
        faces.append(bm.faces.new([r_[-1] for r_ in reversed(rings)]))
        allv = [v for r_ in rings for v in r_]
        return self._finish_piece(allv, faces, mat, xf)

    def box(self, size, centre, mat, bevel=0.0, segs=2, xf=None, round_=0.0):
        """An axis-aligned box (then `xf`), bevelled on every edge, or with
        its long edges along X rounded by `round_` first."""
        sx, sy, sz = size
        cx, cy, cz = centre
        prof = [(cz - sz / 2, cy - sy / 2), (cz + sz / 2, cy - sy / 2),
                (cz + sz / 2, cy + sy / 2), (cz - sz / 2, cy + sy / 2)]
        if round_ > 0:
            prof = fillet(prof, round_, 3)
            return self.prism(prof, cx - sx / 2, cx + sx / 2, mat, "x", bevel, segs, xf)
        bm = self.bm
        vs = []
        for dx in (-1, 1):
            for dy in (-1, 1):
                for dz in (-1, 1):
                    vs.append(bm.verts.new((cx + dx * sx / 2, cy + dy * sy / 2, cz + dz * sz / 2)))
        idx = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        faces = [bm.faces.new([vs[i] for i in q]) for q in idx]
        return self._finish_piece(vs, faces, mat, xf, bevel, segs)

    def lathe(self, profile, mat, segs=20, xf=None, closed=False, smooth=True, start=0.0,
              sweep=360.0):
        """Revolve (r, z) points around the Z axis (the bore), then `xf`.

        Points with r == 0 collapse to the axis. `closed` joins the last
        point back to the first (a tube wall). A partial `sweep` (degrees)
        leaves the shape open.
        """
        bm = self.bm
        full = abs(sweep - 360.0) < 1e-6
        nring = segs if full else segs + 1
        rings = []
        allv = []
        for r, z in profile:
            if r <= 1e-7:
                v = bm.verts.new((0.0, 0.0, z))
                rings.append([v] * nring)
                allv.append(v)
            else:
                ring = []
                for k in range(nring):
                    a = math.radians(start + sweep * k / segs)
                    v = bm.verts.new((r * math.cos(a), r * math.sin(a), z))
                    ring.append(v)
                    allv.append(v)
                rings.append(ring)
        faces = []
        npro = len(profile)
        last = npro if closed else npro - 1
        for i in range(last):
            ra, rb = rings[i], rings[(i + 1) % npro]
            kmax = segs
            for k in range(kmax):
                k1 = (k + 1) % nring if full else k + 1
                quad = [ra[k], ra[k1], rb[k1], rb[k]]
                uniq = []
                for v in quad:
                    if v not in uniq:
                        uniq.append(v)
                if len(uniq) >= 3:
                    try:
                        faces.append(bm.faces.new(uniq))
                    except ValueError:
                        pass
        if not closed:
            # Cap open ends that are not on the axis.
            for ring in (rings[0], rings[-1]):
                if len(set(ring)) > 2 and full:
                    try:
                        faces.append(bm.faces.new(ring))
                    except ValueError:
                        pass
        return self._finish_piece(list(set(allv)), faces, mat, xf)

    def cyl(self, r, z0, z1, mat, segs=16, xf=None, chamfer=0.0):
        """A solid cylinder along Z from z0 to z1, optionally chamfered."""
        if z1 < z0:
            z0, z1 = z1, z0
        c = min(chamfer, r * 0.5, (z1 - z0) * 0.3)
        if c > 0:
            prof = [(0, z0), (r - c, z0), (r, z0 + c), (r, z1 - c), (r - c, z1), (0, z1)]
        else:
            prof = [(0, z0), (r, z0), (r, z1), (0, z1)]
        return self.lathe(prof, mat, segs, xf)

    def tube(self, r_out, r_in, z0, z1, mat, segs=20, xf=None, chamfer=0.0):
        """A hollow tube along Z (a barrel with its bore)."""
        if z1 < z0:
            z0, z1 = z1, z0
        c = chamfer
        prof = [(r_in, z0), (r_out - c, z0), (r_out, z0 + c), (r_out, z1 - c), (r_out - c, z1),
                (r_in, z1)]
        return self.lathe(prof, mat, segs, xf, closed=True)

    def sweep(self, path, section, mat, closed=False, xf=None, plane="zy", up=None):
        """Sweep a 2D section along a 2D path in the gun's side plane.

        path: (z, y) points at x = 0. section: (x, n) points, where n is the
        offset along the path's in-plane normal. Good for trigger guards,
        loops and bent levers.
        """
        bm = self.bm
        pts = [Vector((0.0, b, a)) for a, b in path]
        n = len(pts)
        rings = []
        for i in range(n):
            if closed:
                t = pts[(i + 1) % n] - pts[i - 1]
            else:
                t = pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]
            t.normalize()
            nrm = Vector((1, 0, 0)).cross(t).normalized()
            ring = [bm.verts.new(pts[i] + Vector((sx, 0, 0)) + nrm * sn) for sx, sn in section]
            rings.append(ring)
        faces = []
        m = len(section)
        last = n if closed else n - 1
        for i in range(last):
            ra, rb = rings[i], rings[(i + 1) % n]
            for k in range(m):
                k1 = (k + 1) % m
                faces.append(bm.faces.new((ra[k], ra[k1], rb[k1], rb[k])))
        if not closed:
            faces.append(bm.faces.new(rings[0]))
            faces.append(bm.faces.new(list(reversed(rings[-1]))))
        allv = [v for r in rings for v in r]
        return self._finish_piece(allv, faces, mat, xf)

    def sphere(self, r, centre, mat, segs=24, rings=12, xf=None, scale=(1, 1, 1)):
        bm = self.bm
        res = bmesh.ops.create_uvsphere(bm, u_segments=segs, v_segments=rings, radius=r)
        verts = res["verts"]
        m = T(*centre) @ S(*scale)
        if xf is not None:
            m = xf @ m
        faces = list({f for v in verts for f in v.link_faces})
        return self._finish_piece(verts, faces, mat, m)

    def loft(self, rings, mat, xf=None, caps=True):
        """Skin a list of closed rings (each a list of game-space points,
        all the same length) into a tube, capping both ends."""
        bm = self.bm
        vr = [[bm.verts.new(p) for p in r] for r in rings]
        m = len(rings[0])
        faces = []
        for a, b in zip(vr, vr[1:]):
            for k in range(m):
                k1 = (k + 1) % m
                faces.append(bm.faces.new((a[k], a[k1], b[k1], b[k])))
        if caps:
            faces.append(bm.faces.new(list(reversed(vr[0]))))
            faces.append(bm.faces.new(vr[-1]))
        return self._finish_piece([v for r in vr for v in r], faces, mat, xf)

    def stock(self, outline, z0, z1, mat, width, stations=40, segs=24, power=2.6, xf=None,
              spacing=None):
        """A rounded piece of furniture (stock, forend, grip) from its side
        outline: cut across every few millimetres along Z, each cut a
        superellipse filling the outline's height there. `width(z, y_mid,
        half_height) -> (w_bottom, w_top)` gives its half widths at the
        belly and the top; `power` (a number, or a function of z) 2 is an
        ellipse, higher is squarer.
        `spacing(t)` maps 0..1 to 0..1 to bunch the cuts (default even).
        The outline must be single-valued across Z between z0 and z1."""
        if area2(outline) < 0:
            outline = list(reversed(outline))

        def span(z):
            ys = []
            n = len(outline)
            for i in range(n):
                (za, ya), (zb, yb) = outline[i], outline[(i + 1) % n]
                if (za - z) * (zb - z) <= 0 and za != zb:
                    ys.append(ya + (z - za) / (zb - za) * (yb - ya))
            return (min(ys), max(ys)) if len(ys) >= 2 else None

        rings = []
        for i in range(stations + 1):
            t = i / stations
            t = spacing(t) if spacing else t
            z = z0 + (z1 - z0) * t
            sp = span(z)
            if sp is None:
                continue
            lo, hi = sp
            yc, h = (lo + hi) / 2, (hi - lo) / 2
            wb, wt = width(z, yc, h)
            e = 2.0 / (power(z) if callable(power) else power)
            ring = []
            for k in range(segs):
                th = 2 * math.pi * k / segs
                c, sn = math.cos(th), math.sin(th)
                w = wb + (wt - wb) * (sn + 1) / 2
                ring.append(Vector((math.copysign(abs(c) ** e, c) * w,
                                    yc + math.copysign(abs(sn) ** e, sn) * h, z)))
            rings.append(ring)
        return self.loft(rings, mat, xf)

    def checkering(self, cutter, outline, width, power, panel, pitch=0.0028, angle=30.0,
                   depth=0.0006, sides=(-1, 1)):
        """Checkering on a `stock`, both sides: `cutter` (cut from the stock)
        gets a recess 4 mm deep over `panel` = (z0, z1, y0, y1), and this
        Part (joined to the stock after the cut, or the cut eats it) an
        insert filling it whose face is a lattice of little pyramids, their
        tips on the stock's surface and the grooves between `depth` below
        it, the grooves at +-`angle` degrees to the bore, `pitch` apart.
        (Cutting each groove with a boolean costs ten times the triangles.)
        Give it the stock's outline, width and power."""
        surf = stock_surface(outline, width, power)
        z0, z1, y0, y1 = panel
        a = math.radians(angle)
        e1 = Vector((math.cos(a), math.sin(a)))
        e2 = Vector((math.cos(a), -math.sin(a)))
        L = pitch / math.sin(2 * a)   # rhombus side for grooves `pitch` apart
        centre = Vector(((z0 + z1) / 2, (y0 + y1) / 2))
        n = int(math.hypot(z1 - z0, y1 - y0) / L) + 2

        def at(i, j):
            return centre + e1 * (i * L) + e2 * (j * L)

        def inside(q):
            return z0 <= q.x <= z1 and y0 <= q.y <= y1 and surf(q.x, q.y) is not None

        cells = {(i, j) for i in range(-n, n) for j in range(-n, n)
                 if all(inside(at(i + di, j + dj)) for di, dj in ((0, 0), (1, 0), (1, 1), (0, 1)))}
        if not cells:
            return
        floor = 0.004
        for sx in sides:
            def co(q, dx):
                return Vector((sx * (surf(q.x, q.y) + dx), q.y, q.x))

            for part, top_dx, relief in ((self, 0.0, True), (cutter, 0.002, False)):
                bm = part.bm
                top, bot = {}, {}

                def corner(i, j, store, dx):
                    if (i, j) not in store:
                        store[(i, j)] = bm.verts.new(co(at(i, j), dx))
                    return store[(i, j)]

                faces, verts = [], []
                for (i, j) in cells:
                    ring = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
                    tc = [corner(ci, cj, top, top_dx - (depth if relief else 0.0))
                          for ci, cj in ring]
                    bc = [corner(ci, cj, bot, -floor) for ci, cj in ring]
                    if relief:
                        apex = bm.verts.new(co(at(i + 0.5, j + 0.5), 0.0))
                        verts.append(apex)
                        for k in range(4):
                            faces.append(bm.faces.new((tc[k], tc[(k + 1) % 4], apex)))
                    else:
                        faces.append(bm.faces.new(tc))
                    faces.append(bm.faces.new(list(reversed(bc))))
                    for k, (di, dj) in enumerate(((0, -1), (1, 0), (0, 1), (-1, 0))):
                        if (i + di, j + dj) not in cells:
                            faces.append(bm.faces.new((tc[(k + 1) % 4], tc[k], bc[k],
                                                       bc[(k + 1) % 4])))
                verts += list(top.values()) + list(bot.values())
                part._finish_piece(verts, faces, "Wood")

    def cut(self, part):
        """Subtract another Part (a cutter) when the object is made."""
        self.cutters.append(part)


def section_rect(w, h, r=0.0, segs=2):
    """A rectangle section (x, n) centred on the path, rounded by r."""
    pts = [(-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2)]
    return fillet(pts, r, segs) if r > 0 else pts


def section_circle(r, n=8):
    return [(r * math.cos(2 * math.pi * i / n), r * math.sin(2 * math.pi * i / n)) for i in range(n)]


def stock_surface(outline, width, power):
    """x(z, y) of a `Part.stock`'s +X side at (z, y), or None outside it."""
    if area2(outline) < 0:
        outline = list(reversed(outline))

    def surf(z, y):
        ys = []
        n = len(outline)
        for i in range(n):
            (za, ya), (zb, yb) = outline[i], outline[(i + 1) % n]
            if (za - z) * (zb - z) <= 0 and za != zb:
                ys.append(ya + (z - za) / (zb - za) * (yb - ya))
        if len(ys) < 2:
            return None
        lo, hi = min(ys), max(ys)
        yc, h = (lo + hi) / 2, (hi - lo) / 2
        t = (y - yc) / h
        if abs(t) >= 0.98:
            return None
        p = power(z) if callable(power) else power
        sn = math.copysign(abs(t) ** (p / 2), t)
        c = math.sqrt(max(0.0, 1 - sn * sn))
        wb, wt = width(z, yc, h)
        w = wb + (wt - wb) * (sn + 1) / 2
        return c ** (2.0 / p) * w

    return surf


# ---------------------------------------------------------------------------
# Making objects


def _bm_to_object(part, name=None):
    bm = part.bm
    # Game -> Blender, around the part's origin.
    m = TO_BLENDER @ T(*(-part.origin))
    bmesh.ops.transform(bm, matrix=m, verts=bm.verts)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-6)
    me = bpy.data.meshes.new(name or part.name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name or part.name, me)
    obj.location = TO_BLENDER @ part.origin
    bpy.context.scene.collection.objects.link(obj)
    for mat in part.mats:
        me.materials.append(material(mat))
    return obj


def _apply_boolean(obj, cutter_obj):
    mod = obj.modifiers.new("cut", "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.solver = "EXACT"
    mod.object = cutter_obj
    # Cutters are often several overlapping pieces.
    mod.use_self = True
    mod.use_hole_tolerant = True
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)
    bpy.data.objects.remove(cutter_obj, do_unlink=True)


def box_uv(me, origin_b):
    """Per-face box projection in metres, in game axes, U along the gun."""
    bm = bmesh.new()
    bm.from_mesh(me)
    uv = bm.loops.layers.uv.verify()
    inv = TO_BLENDER.inverted()
    for f in bm.faces:
        n = inv.to_3x3() @ f.normal
        ax, ay, az = abs(n.x), abs(n.y), abs(n.z)
        for loop in f.loops:
            p = inv @ (loop.vert.co + origin_b)  # game space, world position
            if ax >= ay and ax >= az:
                u, v = p.z, -p.y
            elif ay >= az:
                u, v = p.z, p.x
            else:
                u, v = p.x, -p.y
            loop[uv].uv = (u, v)
    bm.to_mesh(me)
    bm.free()


def wear(me, mats=("Blued",), width=0.0028, turn=25.0, amount=0.45, seed=0.0):
    """Edge wear: the narrow faces of bevels and roundings on `mats`, where
    a face turns more than `turn` degrees from a neighbour, go to bare
    `Worn` steel in noisy patches (`amount` of the length), the way bluing
    rubs off a gun's edges in the hand and the holster."""
    from mathutils import noise
    idx = [i for i, m in enumerate(me.materials) if m and m.name in mats]
    if not idx:
        return
    me.materials.append(material("Worn"))
    worn = len(me.materials) - 1
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    cos_t = math.cos(math.radians(turn))
    hits = []
    for f in bm.faces:
        if f.material_index not in idx:
            continue
        # Narrow: its area is small for its longest edge.
        longest = max(e.calc_length() for e in f.edges)
        if longest < 1e-6 or f.calc_area() / longest > width:
            continue
        # A strip along an edge turns across its long sides; a sliver of a
        # flat face (booleans leave many) does not.
        if not any(g is not f and f.normal.dot(g.normal) < cos_t
                   for e in f.edges if e.calc_length() > 0.5 * longest
                   for g in e.link_faces):
            continue
        c = f.calc_center_median()
        if noise.noise(c * 60.0 + Vector((seed, seed, seed))) > 0.5 - amount:
            continue
        hits.append(f)
    for f in hits:
        f.material_index = worn
    bm.to_mesh(me)
    bm.free()


def make(part, sharp_angle=40.0, worn=None):
    """Turn a Part into a Blender object: booleans, UVs, smoothing, and
    edge wear if `worn` is a dict of `wear` arguments."""
    cutters = part.cutters
    obj = _bm_to_object(part)
    # Wear before the cuts: engraving and slots stay the plain finish.
    if worn is not None:
        wear(obj.data, **worn)
    for c in cutters:
        c.origin = part.origin.copy()
        cobj = _bm_to_object(c, part.name + "_cutter")
        cobj.location = obj.location
        _apply_boolean(obj, cobj)
    me = obj.data
    box_uv(me, obj.location)
    me.shade_smooth()
    me.set_sharp_from_angle(angle=math.radians(sharp_angle))
    # Big flat faces own their vertex normals, so bevels read crisp and
    # long boolean triangles do not smear the shading.
    wn = obj.modifiers.new("wn", "WEIGHTED_NORMAL")
    wn.mode = "FACE_AREA"
    wn.keep_sharp = True
    wn.weight = 100
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=wn.name)
    return obj


def join(name, *parts, worn=None):
    """Make several Parts (e.g. one with boolean cuts and one without, which
    the exact solver would mangle) and join them into one object. They
    should share an origin; the first one's is kept."""
    objs = [make(p, worn=worn) if isinstance(p, Part) else p for p in parts]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    if len(objs) > 1:
        bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    obj.name = name
    obj.data.name = name
    return obj


def empty(name, pos, rot=None):
    """A named point (glTF node without a mesh), in game space."""
    obj = bpy.data.objects.new(name, None)
    obj.empty_display_size = 0.01
    game = T(*pos) @ (rot if rot is not None else Matrix.Identity(4))
    # Conjugate: the exporter undoes TO_BLENDER, so the node lands as `game`.
    obj.matrix_world = TO_BLENDER @ game @ TO_BLENDER.inverted()
    bpy.context.scene.collection.objects.link(obj)
    return obj


def parent(child, par):
    """Parent keeping the child's world transform."""
    mw = child.matrix_world.copy()
    child.parent = par
    child.matrix_world = mw


def report():
    total = 0
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        dg = bpy.context.evaluated_depsgraph_get()
        me = obj.evaluated_get(dg).to_mesh()
        me.calc_loop_triangles()
        tris = len(me.loop_triangles)
        total += tris
        # Bounds in game space.
        inv = TO_BLENDER.inverted()
        pts = [inv @ (obj.matrix_world @ v.co) for v in me.vertices]
        lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
        hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
        print(f"  {obj.name:24s} {tris:6d} tris  x[{lo.x:+.3f},{hi.x:+.3f}] "
              f"y[{lo.y:+.3f},{hi.y:+.3f}] z[{lo.z:+.3f},{hi.z:+.3f}]")
        obj.evaluated_get(dg).to_mesh_clear()
    print(f"  total {total} tris")
    return total


def export(rel_path):
    path = os.path.join(REPO, rel_path)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_yup=True,
        export_apply=True,
        export_texcoords=True,
        export_normals=True,
        export_tangents=True,
        export_materials="EXPORT",
        export_cameras=False,
        export_lights=False,
        export_animations=False,
        export_extras=False,
        use_selection=False,
    )
    print(f"wrote {rel_path} ({os.path.getsize(path) / 1024:.0f} KiB)")


def main_guard():
    """Scripts call this to reset to an empty scene."""
    reset()
