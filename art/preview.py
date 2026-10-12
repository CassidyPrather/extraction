"""Render a quick Cycles preview of an exported model, for checking the
modelling without starting the game.

    blender -b --python art/preview.py -- assets/models/guns/ak74.glb out.png [views] [size] [focus]

`focus` is "x,y,z,distance" in game space for a close-up.
`views` is a comma list of side,top,front,back,threeq,under,left (default
side,threeq,top,front). The views are tiled into one image. Everything
renders in neutral studio light on a grey backdrop; materials are the
preview colours baked into the glb.
"""

import math
import os
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
src, out = argv[0], argv[1]
views = (argv[2] if len(argv) > 2 else "side,threeq,top,front").split(",")
size = int(argv[3]) if len(argv) > 3 else 700
# Optional close-up: game-space point to look at and a distance in metres.
focus = [float(v) for v in argv[4].split(",")] if len(argv) > 4 else None

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.abspath(src))
scene = bpy.context.scene

objs = [o for o in scene.objects if o.type == "MESH"]
lo = Vector((1e9, 1e9, 1e9))
hi = Vector((-1e9, -1e9, -1e9))
for o in objs:
    for c in o.bound_box:
        w = o.matrix_world @ Vector(c)
        lo = Vector(map(min, lo, w))
        hi = Vector(map(max, hi, w))
centre = (lo + hi) / 2
extent = (hi - lo).length
if focus:
    centre = Vector((focus[0], -focus[2], focus[1]))

scene.render.engine = "CYCLES"
scene.cycles.samples = 24
scene.cycles.use_denoising = True
scene.render.resolution_x = size
scene.render.resolution_y = int(size * 0.62)
scene.render.film_transparent = False
world = bpy.data.worlds.new("w")
scene.world = world
world.use_nodes = True
world.node_tree.nodes["Background"].inputs[0].default_value = (0.35, 0.36, 0.38, 1)
world.node_tree.nodes["Background"].inputs[1].default_value = 0.6

for rot, energy in (((50, 10, 30), 4.0), ((60, -20, 200), 1.5)):
    ld = bpy.data.lights.new("sun", "SUN")
    ld.energy = energy
    lo_ = bpy.data.objects.new("sun", ld)
    lo_.rotation_euler = [math.radians(a) for a in rot]
    scene.collection.objects.link(lo_)

cam_data = bpy.data.cameras.new("cam")
cam_data.lens = 70
cam = bpy.data.objects.new("cam", cam_data)
scene.collection.objects.link(cam)
scene.camera = cam

# Blender space: +Y is down the bore (game -Z), +X right, +Z up.
DIRS = {
    "side": Vector((1, 0, 0.05)),
    "left": Vector((-1, 0, 0.05)),
    "top": Vector((0.001, 0, 1)),
    "front": Vector((0.05, 1, 0.1)),
    "back": Vector((0.05, -1, 0.15)),
    "threeq": Vector((0.8, 0.6, 0.45)),
    "rear3q": Vector((0.7, -0.7, 0.45)),
    "under": Vector((0.3, 0.1, -1)),
}

tiles = []
for v in views:
    d = DIRS[v].normalized()
    dist = focus[3] if focus else extent * 2.3
    cam.location = centre + d * dist
    look = centre - cam.location
    cam.rotation_euler = look.to_track_quat("-Z", "Y").to_euler()
    path = f"{out}.{v}.png"
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    tiles.append(path)

# Tile the views into one image (2 columns).
imgs = [bpy.data.images.load(p) for p in tiles]
w, h = imgs[0].size
cols = 2 if len(imgs) > 1 else 1
rows = (len(imgs) + cols - 1) // cols
canvas = bpy.data.images.new("tiles", w * cols, h * rows)
import array

px = array.array("f", [0.0]) * (w * cols * h * rows * 4)
for i, im in enumerate(imgs):
    src_px = array.array("f", [0.0]) * (w * h * 4)
    im.pixels.foreach_get(src_px)
    cx, cy = i % cols, rows - 1 - i // cols
    for y in range(h):
        row = (cy * h + y) * w * cols + cx * w
        px[row * 4:(row + w) * 4] = src_px[y * w * 4:(y + 1) * w * 4]
canvas.pixels.foreach_set(px)
canvas.filepath_raw = out
canvas.file_format = "PNG"
canvas.save()
for p in tiles:
    os.remove(p)
print(f"preview: {out}")
