"""Draw a model's cross-sections: the outline of every object cut across the
bore at the stations asked for, side by side at one scale, with a 10 mm
grid. For checking a gun's sections against the real one's (rounds,
tapers, widths), which no rendered view shows.

    blender -b --factory-startup --python art/sections.py -- MODEL.glb OUT.png Z[,Z...] [names]

Z are game-space stations along the bore (metres, -Z toward the muzzle).
`names` (comma list) labels the panels. Objects are drawn in their rest
pose; each object's outline in its own grey, so the slide stands apart
from the frame.
"""

import os
import sys

import bmesh
import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:]
src, out = argv[0], argv[1]
# A station is a z along the bore, or "yN" for a cut across a grip at
# height N (drawn from above: x across, z along).
stations = argv[2].split(",")
names = argv[3].split(",") if len(argv) > 3 else stations

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.abspath(src))
objs = [o for o in bpy.context.scene.objects if o.type == "MESH"]


def section(station):
    """Segments (pairs of 2D points, game space) of every object cut at the
    station: (x, y) across the bore, or (x, -z) across a grip."""
    flat = station.startswith("y")
    v = float(station[1:] if flat else station)
    out = []
    for i, o in enumerate(objs):
        bm = bmesh.new()
        bm.from_mesh(o.data)
        bm.transform(o.matrix_world)
        # Blender: game z is -y.
        geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
        co, no = ((0, 0, v), (0, 0, 1)) if flat else ((0, -v, 0), (0, 1, 0))
        res = bmesh.ops.bisect_plane(bm, geom=geom, plane_co=Vector(co), plane_no=Vector(no),
                                     dist=1e-7)
        cut = [e for e in res["geom_cut"] if isinstance(e, bmesh.types.BMEdge)]
        for e in cut:
            a, b = e.verts[0].co, e.verts[1].co
            # Across a grip: x across, forward (-z game, +y Blender) up.
            out.append((i, (a.x, a.y), (b.x, b.y)) if flat else (i, (a.x, a.z), (b.x, b.z)))
        bm.free()
    return out


cuts = [section(st) for st in stations]
pts = [p for c in cuts for _, a, b in c for p in (a, b)]
if not pts:
    sys.exit("nothing at those stations")
x0 = min(p[0] for p in pts) - 0.004
x1 = max(p[0] for p in pts) + 0.004
y0 = min(p[1] for p in pts) - 0.004
y1 = max(p[1] for p in pts) + 0.004
PX = 8000.0  # pixels a metre: 8 px a millimetre
W, H = int((x1 - x0) * PX), int((y1 - y0) * PX)

# Drawn with Pillow (not shipped with Blender's Python): write the segments
# out, draw outside Blender.
import json  # noqa: E402

data = {"w": W, "h": H, "x0": x0, "y1": y1, "px": PX, "names": names,
        "cuts": [[(i, a, b) for i, a, b in c] for c in cuts]}
with open(out + ".json", "w") as f:
    json.dump(data, f)
print(f"wrote {out}.json: {len(stations)} sections, {W}x{H} px each")
