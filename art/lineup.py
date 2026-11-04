"""Render a family line-up of item models as a headset sees them: side by
side on a shelf, under indoor light, from a set distance, at the
headset's sharpness. The review procedure is docs/craft/ITEM_READING.md.

    blender -b --python art/lineup.py -- out.png DIST [grey] -- a.glb b.glb ...

- DIST: metres from the eye to the shelf's front edge (1 and 3 for a
  review: a shelf in reach, a shelf across a room).
- grey: render in greyscale (value and silhouette alone).
- down: look down at the items from DIST away, 55 degrees below level
  (a crate at your feet, a table you lean over), not across a room.
  Flags combine: `1 grey down`.
- turn: turn each item half round first (a prop's -Z face, its front,
  to the eye).
- an item `proc:bolt` or `proc:detector`: built as `src/anomaly/visuals.rs`
  builds it (those have no model file).

The camera is 1.6 m up, looking at the shelf (0.9 m up) from DIST away,
with a 100 degree horizontal view on a 1430 px wide image: 14.3 px a
degree, what the Beyond 2 renders in the middle of the view at the
game's auto render scale (docs/LEGIBILITY.md). Nothing is sharpened or
enlarged: crop the result, never scale it up, to judge it.

The light: a room lit through a window to its left by overcast daylight
(a large soft cool panel), and the room's own dim fill. No lamp, no rim
light: an item must read under the light most rooms in the Zone have.
Items stand as their models rest (on their own floor), in the order
given, 6 cm apart.
"""

import math
import os
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:]
out, dist = argv[0], float(argv[1])
flags = argv[2:argv.index("--")] if "--" in argv else argv[2:3]
grey = "grey" in flags
# Looking down into a crate or onto a table at your feet: the eye DIST
# from the items, 55 degrees below level.
down = "down" in flags
turn = "turn" in flags
paths = argv[argv.index("--") + 1:] if "--" in argv else argv[3 if grey else 2:]

SHELF_Y = 0.9  # game y of the shelf's top
GAP = 0.06
EYE = 1.6

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def import_glb(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(path))
    new = [o for o in bpy.data.objects if o not in before]
    root = bpy.data.objects.new("item_" + os.path.basename(path), None)
    scene.collection.objects.link(root)
    for o in new:
        if o.parent is None:
            o.parent = root
    bpy.context.view_layer.update()
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in new:
        if o.type != "MESH":
            continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    return root, lo, hi


def srgb(c):
    return tuple(((v + 0.055) / 1.055) ** 2.4 if v > 0.04045 else v / 12.92 for v in c)


def proc(name):
    """The items `src/anomaly/visuals.rs` builds in code rather than loads,
    built the same way: each part a primitive placed, turned and scaled in
    game space (x right, y up, -z forward). Keep in step with that file."""
    from mathutils import Matrix

    def q(axis, angle):
        return Matrix.Rotation(angle, 4, axis)

    along = q("X", math.pi / 2)  # a cylinder's axis (Y) laid along Z
    parts = []  # (kind, size, position, turn, colour)
    if name == "bolt":
        steel, tape = (0.36, 0.3, 0.25), (0.95, 0.36, 0.04)
        parts += [("cyl", (0.005, 0.06), (0, 0, 0.004), along, steel),
                  ("hex", (0.0085, 0.007), (0, 0, -0.0315), along, steel),
                  ("hex", (0.0085, 0.008), (0, 0, 0.018), along, steel),
                  ("box", (0.013, 0.013, 0.007), (0, 0, 0), Matrix(), tape)]
        for side in (-1, 1):
            parts.append(("box", (0.006, 0.0015, 0.04), (side * 0.006, 0.006, 0.019),
                          q("Y", side * 0.35) @ q("X", -0.25), tape))
    elif name == "detector":
        olive, face, black = (0.36, 0.4, 0.24), (0.85, 0.82, 0.7), (0.04, 0.04, 0.04)
        parts += [("box", (0.07, 0.04, 0.11), (0, 0, 0.025), Matrix(), olive),
                  ("box", (0.066, 0.002, 0.104), (0, 0.0205, 0.025), Matrix(), (0.93, 0.8, 0.25)),
                  ("cyl", (0.016, 0.003), (0.012, 0.021, 0.05), Matrix(), face),
                  ("box", (0.03, 0.004, 0.03), (-0.015, 0.021, 0.005), Matrix(), black),
                  ("box", (0.012, 0.012, 0.09), (-0.041, 0, 0.025), Matrix(), black),
                  ("cyl", (0.004, 0.05), (0.018, 0, -0.055), along, black),
                  ("sph", (0.006,), (0.018, 0, -0.078), Matrix(), black),
                  ("sph", (0.005,), (-0.02, 0.021, -0.01), Matrix(), (0.15, 0.03, 0.02))]
    else:
        raise SystemExit(f"no procedural look for {name}")
    # Game space to Blender's: (x, y, z) -> (x, -z, y).
    C = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
    root = bpy.data.objects.new("item_" + name, None)
    scene.collection.objects.link(root)
    for kind, size, at, turn, col in parts:
        if kind == "box":
            bpy.ops.mesh.primitive_cube_add(size=1)
            scale = Matrix.Diagonal((*size, 1))
        elif kind in ("cyl", "hex"):
            # Bevy's cylinders stand along their Y; Blender's along Z: made
            # unit along Blender Z, which is game Y once turned back below.
            bpy.ops.mesh.primitive_cylinder_add(vertices=6 if kind == "hex" else 16, radius=1,
                                                depth=1)
            r, h = size
            scale = Matrix.Diagonal((r, h, r, 1))
        else:
            bpy.ops.mesh.primitive_uv_sphere_add(radius=1, segments=12, ring_count=8)
            scale = Matrix.Diagonal((size[0],) * 3 + (1,))
        o = bpy.context.object
        game = Matrix.Translation(at) @ turn @ scale
        o.matrix_world = C @ game @ C.inverted()
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (*srgb(col), 1)
        o.data.materials.append(m)
        o.parent = root
        o.matrix_parent_inverse = root.matrix_world.inverted()
    bpy.context.view_layer.update()
    lo = Vector((1e9, 1e9, 1e9))
    hi = Vector((-1e9, -1e9, -1e9))
    for o in root.children:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    return root, lo, hi


# Blender space: x right, y away from the eye (game -z), z up.
items = [proc(p[5:]) if p.startswith("proc:") else import_glb(p) for p in paths]
if turn:
    # Half round about the vertical: its bounds mirror in x and y.
    for root, lo, hi in items:
        root.rotation_euler = (0.0, 0.0, math.pi)
    items = [(r, Vector((-hi.x, -hi.y, lo.z)), Vector((-lo.x, -lo.y, hi.z))) for r, lo, hi in items]
width = sum(hi.x - lo.x for _, lo, hi in items) + GAP * (len(items) - 1)
x = -width / 2
depth = max(hi.y - lo.y for _, lo, hi in items)
for root, lo, hi in items:
    # Its left edge at x, its front on the shelf's front line, its foot on
    # the shelf.
    root.location = Vector((x - lo.x, 0.05 - lo.y, SHELF_Y - lo.z))
    x += hi.x - lo.x + GAP

# The shelf: a worn grey-brown board, and the wall behind it.
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, depth / 2 + 0.05, SHELF_Y - 0.015))
shelf = bpy.context.object
shelf.scale = (max(width + 0.4, 1.0), depth + 0.3, 0.03)
mat = bpy.data.materials.new("shelf")
mat.use_nodes = True
mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.16, 0.13, 0.10, 1)
mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.85
shelf.data.materials.append(mat)
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, depth + 0.35, SHELF_Y + 0.6))
wall = bpy.context.object
wall.rotation_euler = (math.pi / 2, 0, 0)
wall.scale = (30, 8, 1)
wmat = bpy.data.materials.new("wall")
wmat.use_nodes = True
wmat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.22, 0.21, 0.19, 1)
wmat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
wall.data.materials.append(wmat)

# The window: a big soft cool panel up and to the left; the room's fill.
bpy.ops.object.light_add(type="AREA", location=(-2.2, -0.6, SHELF_Y + 1.0))
win = bpy.context.object
win.data.shape = "RECTANGLE"
win.data.size, win.data.size_y = 1.2, 1.5
win.data.energy = 180
win.data.color = (0.86, 0.92, 1.0)
win.rotation_euler = (0, -math.radians(65), 0)
world = bpy.data.worlds.new("room")
world.use_nodes = True
world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.20, 0.21, 0.23, 1)
world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.35
scene.world = world

# The eye.
cam_data = bpy.data.cameras.new("eye")
cam_data.sensor_fit = "HORIZONTAL"
cam_data.angle = math.radians(100)
cam = bpy.data.objects.new("eye", cam_data)
scene.collection.objects.link(cam)
target = Vector((0, 0.05 + depth / 2, SHELF_Y + 0.05))
if down:
    a = math.radians(55)
    cam.location = target + Vector((0, -dist * math.cos(a), dist * math.sin(a)))
else:
    cam.location = Vector((0, -dist, EYE))
cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
scene.camera = cam

scene.render.engine = "CYCLES"
scene.cycles.samples = 64
scene.cycles.use_denoising = True
scene.render.resolution_x = 1430
scene.render.resolution_y = 600
scene.render.resolution_percentage = 100
scene.view_settings.view_transform = "AgX" if "AgX" in [
    v.name for v in bpy.types.ColorManagedViewSettings.bl_rna.properties["view_transform"].enum_items
] else "Filmic"
if grey:
    scene.view_settings.look = "None"
    scene.render.image_settings.color_mode = "BW"
scene.render.filepath = os.path.abspath(out)
bpy.ops.render.render(write_still=True)
