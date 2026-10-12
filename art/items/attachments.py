"""Rail and muzzle attachments: the 3.5x scope, the side-mount rail, the
weapon light, the laser module and the artifact adapter. Writes
assets/models/items/<id>.glb for each (item space: see _kit.py).

Sockets they carry (src/gadgets/mod.rs), which the models are built round:

- side_rail: things on its rail sit with their centre at y 0.022, so the
  rail's top is y 0.006.
- weapon_light, laser_module, flashlight: a power cell (0.02 x 0.06 x 0.02,
  standing up) seats with its centre at (0, 0, 0.02), so each has an open
  cell bay through it there.
- artifact_adapter: the gun's muzzle is at its back face (z 0.045); an
  artifact sits in the cradle with its centre at (0, 0.03, 0).
- pu_scope: the eye goes at (0, 0, 0.2), so the optical axis is y = 0.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, UP, cell_bay, item, wanted
from common import Part, RX, RY, RZ, T, S, fillet


def rail_clamp(p, z0, z1, y_top, mat="Black"):
    """A clamp under a rail-mounted thing: jaws hooking the rail's sides, a
    thumb screw on the right. Its underside sits on the rail top
    (y_top - 0.016 below the thing's centre)."""
    yb = -0.016
    p.box((0.022, y_top - yb, z1 - z0), (0.0, (y_top + yb) / 2, (z0 + z1) / 2), mat, bevel=0.0015,
          segs=1)
    for sx in (-1, 1):
        p.box((0.004, 0.009, z1 - z0), (sx * 0.0125, yb - 0.003, (z0 + z1) / 2), mat,
              bevel=0.001, segs=1)
    p.cyl(0.004, 0.0, 0.006, "Steel", segs=10, xf=T(0.0145, yb - 0.002, (z0 + z1) / 2) @ ALONG_X,
          chamfer=0.001)


# --- 3.5x scope -------------------------------------------------------------------------

def pu_scope():
    p = Part("body")
    prof = [(0.0, -0.1355), (0.0165, -0.1355), (0.0175, -0.134), (0.0175, -0.118),
            (0.0165, -0.116), (0.0165, -0.1), (0.013, -0.084), (0.013, 0.07), (0.0155, 0.082),
            (0.0165, 0.1), (0.0165, 0.112), (0.0, 0.112)]
    p.lathe(prof, "Blued", segs=28)
    # Rubber eyecup.
    p.lathe([(0.0145, 0.111), (0.0175, 0.112), (0.0185, 0.132), (0.017, 0.135), (0.0155, 0.135),
             (0.0145, 0.12)], "Rubber", segs=28, closed=True)
    # Lenses, set back in their bells.
    p.cyl(0.0148, -0.134, -0.13, "Lens", segs=24)
    p.cyl(0.0125, 0.112, 0.116, "Lens", segs=24)
    # Elevation drum on top, windage drum on the left, with index marks.
    p.cyl(0.0105, 0.014, 0.0225, "Blued", segs=20, xf=T(0, 0, -0.012) @ UP, chamfer=0.0015)
    p.box((0.002, 0.004, 0.001), (0.0, 0.019, -0.0226), "Cream")
    p.cyl(0.0105, -0.0225, -0.014, "Blued", segs=20, xf=T(0, 0, -0.012) @ ALONG_X, chamfer=0.0015)
    p.box((0.03, 0.03, 0.024), (0.0, 0.0, -0.012), "Blued", bevel=0.004, segs=2)
    # Side mount: two rings, a base on the dovetail, a lever on the left.
    for z in (-0.06, 0.05):
        p.tube(0.0158, 0.0128, z - 0.008, z + 0.008, "Black", segs=24, chamfer=0.001)
        p.box((0.012, 0.012, 0.016), (0.0, -0.016, z), "Black", bevel=0.0015, segs=1)
    p.box((0.016, 0.006, 0.14), (0.0, -0.0195, -0.005), "Black", bevel=0.0015, segs=1)
    for sx in (-1, 1):
        p.box((0.003, 0.006, 0.12), (sx * 0.009, -0.0215, -0.005), "Black", bevel=0.001, segs=1)
    p.box((0.004, 0.006, 0.05), (-0.013, -0.019, -0.03), "Black", bevel=0.0015, segs=1,
          xf=RY(-0.12, about=(-0.013, -0.019, -0.005)))
    p.cyl(0.0045, -0.016, -0.009, "Black", segs=10, xf=T(0, -0.019, -0.005) @ ALONG_X)
    return [p]


# --- Side-mount rail -----------------------------------------------------------------------

def side_rail():
    p = Part("body")
    # Dovetail clamp base.
    base = fillet([(-0.01, -0.006), (0.01, -0.006), (0.01, -0.001), (-0.01, -0.001)], 0.002, 2)
    p.prism(base, -0.05, 0.05, "Black", axis="z", bevel=0.001, segs=1)
    # Rail: a dovetailed top with cross slots (raised lugs between).
    top = [(-0.0065, -0.0012), (0.0065, -0.0012), (0.0085, 0.0015), (0.006, 0.004),
           (-0.006, 0.004), (-0.0085, 0.0015)]
    p.prism(top, -0.048, 0.048, "Black", axis="z")
    for k in range(9):
        z = -0.04 + k * 0.01
        p.box((0.016, 0.002, 0.0055), (0.0, 0.005, z), "Black")
    # Clamp lever and screw on the left.
    p.box((0.003, 0.004, 0.035), (-0.0115, -0.0035, 0.01), "Steel", bevel=0.0008, segs=1,
          xf=RY(0.1, about=(-0.0115, -0.0035, -0.005)))
    p.cyl(0.003, -0.013, -0.01, "Steel", segs=8, xf=T(0, -0.0035, -0.005) @ ALONG_X)
    return [p]


# --- Weapon light --------------------------------------------------------------------------

def weapon_light():
    p = Part("body")
    # Head bell and lens at the front (the beam is at z -0.06).
    p.lathe([(0.0, -0.045), (0.0145, -0.045), (0.015, -0.043), (0.015, -0.033), (0.0125, -0.028),
             (0.0125, 0.0095), (0.0, 0.0095)], "Black", segs=24)
    p.cyl(0.0125, -0.0458, -0.044, "Lens", segs=20)
    # Knurled grip rings.
    for k in range(4):
        p.tube(0.0132, 0.012, -0.022 + k * 0.007, -0.019 + k * 0.007, "Black", segs=20)
    # Tail behind the cell bay, a pressure switch on the end.
    p.lathe([(0.0, 0.0305), (0.0125, 0.0305), (0.0125, 0.042), (0.011, 0.045), (0.0, 0.045)],
            "Black", segs=24)
    p.cyl(0.006, 0.044, 0.0465, "Rubber", segs=12)
    cell_bay(p, 0.0125, "Black")
    rail_clamp(p, -0.034, 0.0, -0.011)
    return [p]


# --- Laser module ---------------------------------------------------------------------------

def laser_module():
    p = Part("body")
    body = fillet([(-0.0105, -0.0105), (0.0105, -0.0105), (0.0105, 0.0105), (-0.0105, 0.0105)],
                  0.003, 2)
    p.prism(body, -0.03, 0.0095, "Black", axis="z", bevel=0.0015, segs=1)
    p.prism(body, 0.0305, 0.035, "Black", axis="z", bevel=0.0015, segs=1)
    # Emitter: a recessed aperture with a red lens (the beam starts at z -0.035).
    p.tube(0.006, 0.0035, -0.034, -0.029, "Black", segs=16)
    p.cyl(0.0036, -0.0325, -0.031, "LampRed", segs=12)
    # Windage/elevation screws.
    p.cyl(0.002, 0.0105, 0.012, "Steel", segs=8, xf=T(0, 0, -0.015) @ UP)
    p.cyl(0.002, 0.0105, 0.012, "Steel", segs=8, xf=T(0, 0, -0.015) @ ALONG_X)
    cell_bay(p, 0.0105, "Black", wall=0.0025)
    rail_clamp(p, -0.026, 0.004, -0.0095)
    return [p]


# --- Artifact adapter -----------------------------------------------------------------------

def artifact_adapter():
    p = Part("body")
    # Muzzle sleeve, its back face on the muzzle.
    p.tube(0.0125, 0.0075, -0.045, 0.045, "Blued", segs=20, chamfer=0.0015)
    p.tube(0.0145, 0.0075, 0.03, 0.045, "Blued", segs=20, chamfer=0.0015)
    p.cyl(0.0025, 0.0125, 0.017, "Steel", segs=8, xf=T(0, 0, 0.037) @ ALONG_X)
    # Copper coil wound along the sleeve: tilted turns read as a helix.
    for k in range(13):
        z = -0.038 + k * 0.0052
        p.lathe([(0.0128, -0.0019), (0.0147, -0.002), (0.0152, 0.0), (0.0147, 0.002),
                 (0.0128, 0.0019)], "Copper", segs=16, xf=T(0, 0, z) @ RX(0.08), closed=True)
    # Cradle: two posts up to a ring the artifact sits in, copper prongs
    # curling up round it.
    for z in (-0.02, 0.02):
        p.box((0.006, 0.012, 0.006), (0.0, 0.017, z), "Blued", bevel=0.001, segs=1)
    p.lathe([(0.012, -0.002), (0.018, -0.003), (0.0195, 0.0), (0.018, 0.003), (0.012, 0.002)],
            "Copper", segs=20, xf=T(0, 0.022, 0) @ UP, closed=True)
    for k in range(3):
        a = 2 * math.pi * k / 3 + math.pi / 2
        path = [(0.0, 0.0), (0.004, 0.008), (0.006, 0.016), (0.004, 0.024)]
        p.sweep([(z, y) for z, y in path], [(x, n) for x, n in
                                            [(-0.0015, -0.0012), (0.0015, -0.0012),
                                             (0.0015, 0.0012), (-0.0015, 0.0012)]],
                "Copper", xf=T(0, 0.022, 0) @ RY(a) @ T(0, 0, -0.0175) @ RY(math.pi))
    # A little glow where the coil is tapped.
    p.box((0.004, 0.003, 0.012), (0.0, -0.0145, 0.0), "Glow", bevel=0.001, segs=1)
    return [p]


for item_id, fn in (("pu_scope", pu_scope), ("side_rail", side_rail),
                    ("weapon_light", weapon_light), ("laser_module", laser_module),
                    ("artifact_adapter", artifact_adapter)):
    if wanted(item_id):
        item(item_id, fn)
