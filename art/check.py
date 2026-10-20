"""Check exported models for the two faults the first headset playtest
saw on the truck: z-fighting (faces of different pieces lying in the same
plane, facing the same way, overlapping) and parts that float, touching
nothing else in the model.

    blender -b --factory-startup --python art/check.py -- assets/models/vehicles/truck.glb ...

A directory checks every .glb under it. Prints each fault with where it
is, in game space (metres, +Y up), and exits 1 if any model has one.
"""

import os
import sys
from collections import defaultdict

import bpy
from mathutils import Vector

# Faces closer than this along their normal, overlapping by more than
# MIN_AREA, z-fight at a few metres in a headset.
PLANE_GAP = 0.003
MIN_AREA = 2e-4
# A piece whose bounds come no nearer than this to any other piece floats.
TOUCH = 0.005
# CHECK_DETAIL=1 prints every overlapping pair of faces (game-space bounds).
DETAIL = os.environ.get("CHECK_DETAIL") == "1"


def game(v):
    """Blender's Z-up import back to the game's Y-up."""
    return (round(v.x, 3), round(v.z, 3), round(-v.y, 3))


def bounds(vs):
    g = [game(v) for v in vs]
    return (tuple(min(p[k] for p in g) for k in range(3)), tuple(max(p[k] for p in g) for k in range(3)))


def load(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(path))
    return [o for o in bpy.context.scene.objects if o.type == "MESH"]


def faces_of(objs):
    out = []
    for ob in objs:
        m, me = ob.matrix_world, ob.data
        for p in me.polygons:
            if p.area < 1e-7:
                continue
            vs = [m @ me.vertices[i].co for i in p.vertices]
            n = (m.to_3x3() @ p.normal).normalized()
            keys = frozenset((round(v.x, 4), round(v.y, 4), round(v.z, 4)) for v in vs)
            mat = me.materials[p.material_index].name if me.materials else "-"
            out.append((f"{ob.name}/{mat}", n, n.dot(vs[0]), vs, keys))
    return out


def area2d(poly):
    return 0.5 * sum(poly[i][0] * poly[(i + 1) % len(poly)][1]
                     - poly[(i + 1) % len(poly)][0] * poly[i][1] for i in range(len(poly)))


def clip(subject, clipper):
    """Sutherland-Hodgman: subject clipped to a convex, anticlockwise clipper."""
    def inside(p, a, b):
        return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= -1e-12

    def cross(p, q, a, b):
        den = (p[0] - q[0]) * (a[1] - b[1]) - (p[1] - q[1]) * (a[0] - b[0])
        if abs(den) < 1e-15:
            return q
        t = ((p[0] - a[0]) * (a[1] - b[1]) - (p[1] - a[1]) * (a[0] - b[0])) / den
        return (p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1]))

    out = subject
    for i in range(len(clipper)):
        a, b = clipper[i], clipper[(i + 1) % len(clipper)]
        src, out = out, []
        if not src:
            break
        s = src[-1]
        for e in src:
            if inside(e, a, b):
                if not inside(s, a, b):
                    out.append(cross(s, e, a, b))
                out.append(e)
            elif inside(s, a, b):
                out.append(cross(s, e, a, b))
            s = e
    return out


def zfighting(objs):
    faces = faces_of(objs)
    buckets = defaultdict(list)
    for i, f in enumerate(faces):
        n = f[1]
        buckets[(round(n.x, 2), round(n.y, 2), round(n.z, 2), round(f[2] / PLANE_GAP))].append(i)
    hits = defaultdict(lambda: [0.0, None])
    for idx in buckets.values():
        for x in range(len(idx)):
            for y in range(x + 1, len(idx)):
                a, b = faces[idx[x]], faces[idx[y]]
                # Neighbours share a vertex; a face can't fight itself.
                if abs(a[2] - b[2]) > PLANE_GAP or a[4] & b[4]:
                    continue
                n = a[1]
                t = Vector((1, 0, 0)) if abs(n.x) < 0.9 else Vector((0, 1, 0))
                u = n.cross(t).normalized()
                v = n.cross(u)
                pa = [(p.dot(u), p.dot(v)) for p in a[3]]
                pb = [(p.dot(u), p.dot(v)) for p in b[3]]
                pa = pa if area2d(pa) > 0 else pa[::-1]
                pb = pb if area2d(pb) > 0 else pb[::-1]
                o = clip(pa, pb)
                if len(o) < 3:
                    continue
                ar = abs(area2d(o))
                if ar > MIN_AREA:
                    h = hits[tuple(sorted((a[0], b[0])))]
                    h[0] += ar
                    h[1] = h[1] or game(sum(a[3], Vector()) / len(a[3]))
                    if DETAIL:
                        print(f"      {a[0]} {bounds(a[3])} / {b[0]} {bounds(b[3])}")
    return [f"z-fighting {k[0]} / {k[1]}: {ar * 1e4:.0f} cm2, e.g. at {at}"
            for k, (ar, at) in sorted(hits.items(), key=lambda kv: -kv[1][0])]


def islands(objs):
    """Each connected piece of every mesh, as world-space bounds."""
    out = []
    for ob in objs:
        m, me = ob.matrix_world, ob.data
        parent = list(range(len(me.vertices)))

        def find(i):
            while parent[i] != i:
                parent[i] = parent[parent[i]]
                i = parent[i]
            return i

        for e in me.edges:
            a, b = find(e.vertices[0]), find(e.vertices[1])
            if a != b:
                parent[a] = b
        groups = defaultdict(list)
        for i, vtx in enumerate(me.vertices):
            groups[find(i)].append(m @ vtx.co)
        for vs in groups.values():
            lo = Vector((min(p.x for p in vs), min(p.y for p in vs), min(p.z for p in vs)))
            hi = Vector((max(p.x for p in vs), max(p.y for p in vs), max(p.z for p in vs)))
            out.append((ob.name, lo, hi))
    return out


def floating(objs):
    pieces = islands(objs)
    if len(pieces) < 2:
        return []
    out = []
    for i, (name, lo, hi) in enumerate(pieces):
        touches = any(
            all(lo[k] - TOUCH <= h2[k] and l2[k] <= hi[k] + TOUCH for k in range(3))
            for j, (_, l2, h2) in enumerate(pieces) if j != i
        )
        if not touches:
            size = hi - lo
            out.append(f"floating piece in {name} at {game((lo + hi) / 2)}, "
                       f"{size.x:.3f} x {size.z:.3f} x {size.y:.3f} m")
    return out


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    paths = []
    for a in argv:
        if os.path.isdir(a):
            for root, _, files in os.walk(a):
                paths += [os.path.join(root, f) for f in sorted(files) if f.endswith(".glb")]
        else:
            paths.append(a)
    bad = 0
    for path in paths:
        objs = load(path)
        faults = zfighting(objs) + floating(objs)
        # World props carry their baked ambient occlusion (art/props/_kit.py
        # `bake_ao`) as a colour attribute; one without it would read flat.
        if os.sep + "props" + os.sep in os.path.abspath(path):
            faults += [f"no baked AO (colour attribute) on {o.name}" for o in objs
                       if not o.data.color_attributes]
        print(f"{'ok  ' if not faults else 'FAIL'} {path}")
        for f in faults:
            print("    " + f)
        bad += bool(faults)
    print(f"{len(paths) - bad} of {len(paths)} models clean")
    sys.exit(1 if bad else 0)


main()
