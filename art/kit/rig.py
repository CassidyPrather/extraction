"""The kit you wear: chest rig and belt, holster, magazine pouches, hip
pockets, the chest mount, and the backpack. Writes
assets/models/kit/rig.glb and assets/models/kit/backpack.glb.

Everything is built in the frames src/interact/body.rs gives it (game
space: metres, +X right, +Y up, -Z forward):

- `harness`: torso space (origin mid-chest, under the neck): the chest
  panel the pouches ride on, shoulder straps, the belt round the hips, the
  straps joining them.
- `holster`, `pouch`, `pocket`, `mount`: each in its slot's space (origin
  the slot, where the item seated in it sits), drawn on every slot of its
  kind. `holster` is for a pistol hung barrel down, grip back (the slot's
  seat); `pouch` for a magazine standing in it, thin side across X.
- `<kind>_flap_closed` / `<kind>_flap_open`: the flap over an empty slot,
  and the same flap thrown open while something's in it.

rig_open.glb has the same slot pieces with their flaps open (the game
takes only the open flaps from it).

backpack.glb (lid closed) and backpack_open.glb (lid open), in the pack's space (the stowing box, half 0.17 x 0.24 x
0.1, centred; the harness straps on its -Z face): `pack` (an open-topped
bag, hollow so what's in it shows when it's open), `pack_lid_closed`,
`pack_lid_open`.

Slot positions, from body.rs (`slot_layout`), torso space:
holster (0.22, -0.42, 0.02); pouches (-0.14 + 0.09 k, -0.16, -0.13);
pockets (+-0.17, -0.45, -0.08); mount (-0.13, 0.06, -0.12).
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, S, T, TO_BLENDER, export, fillet, join, main_guard, make,
                    report)
from mathutils import Vector

HOLSTER = Vector((0.22, -0.42, 0.02))
POUCH_Y, POUCH_Z = -0.16, -0.13
POCKET = Vector((0.17, -0.45, -0.08))
MOUNT = Vector((-0.13, 0.06, -0.12))

# The belt: an ellipse round the hips.
BELT_Y, BELT_A, BELT_B, BELT_Z0 = -0.43, 0.20, 0.13, 0.0
BELT_H, BELT_T = 0.045, 0.005
# The chest panel's front face.
PANEL_Z = -0.104


def strap(p, pts, width, thick, mat, up=None, normals=None):
    """A flat strap along a 3D polyline: `width` across, `thick` through.
    Its flat faces turn toward `normals[i]` (or away from the path's bend
    plane, `up` giving the width's direction)."""
    rings = []
    n = len(pts)
    pts = [Vector(q) for q in pts]
    for i in range(n):
        t = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
        if normals is not None:
            nrm = Vector(normals[i]).normalized()
            w = t.cross(nrm).normalized()
            nrm = w.cross(t).normalized()
        else:
            w = Vector(up).normalized()
            w = (w - t * w.dot(t)).normalized()
            nrm = w.cross(t).normalized()
        c = pts[i]
        hw, ht = width / 2, thick / 2
        rings.append([c - w * hw - nrm * ht, c + w * hw - nrm * ht,
                      c + w * hw + nrm * ht, c - w * hw + nrm * ht])
    return p.loft(rings, mat)


def belt_point(a, grow=0.0, y=BELT_Y):
    return Vector(((BELT_A + grow) * math.sin(a), y, BELT_Z0 - (BELT_B + grow) * math.cos(a)))


# --- Harness ---------------------------------------------------------------

def harness():
    p = Part("harness")
    # The chest panel: a padded plate across the lower chest, flat where the
    # pouches ride, its sides swept back round the ribs.
    xs = [-0.23, -0.2, -0.17, 0.17, 0.2, 0.23]
    def panel_z(x):
        ax = abs(x)
        return PANEL_Z + (0.0 if ax <= 0.17 else (ax - 0.17) ** 2 * 18.0)
    rings = []
    for x in [-0.23 + 0.46 * k / 14 for k in range(15)]:
        z = panel_z(x)
        prof = fillet([(-0.265, z), (-0.06, z), (-0.06, z + 0.012), (-0.265, z + 0.012)],
                      0.004, 2)
        rings.append([Vector((x, yy, zz)) for yy, zz in prof])
    p.loft(rings, "Canvas")
    # Webbing bands across it (what the pouches are sewn to).
    for y in (-0.235, -0.205, -0.175):
        pts = [Vector((x, y, panel_z(x) - 0.0025)) for x in [-0.16 + 0.32 * k / 10 for k in range(11)]]
        strap(p, pts, 0.016, 0.004, "Olive", normals=[(0, 0, -1)] * len(pts))
    # Shoulder straps: up the front of the chest, over the shoulders,
    # down the back behind the pack.
    for sx in (-1, 1):
        x = 0.135 * sx
        path = [(x, -0.07, PANEL_Z + 0.004), (x * 1.02, 0.03, PANEL_Z - 0.0005),
                (x * 1.06, 0.08, PANEL_Z + 0.01), (x * 1.1, 0.115, -0.06),
                (x * 1.12, 0.13, -0.02), (x * 1.12, 0.125, 0.03), (x * 1.1, 0.09, 0.085),
                (x * 1.05, 0.02, 0.115), (x * 1.0, -0.12, 0.125)]
        nrm = [(0, 0.1, -1), (0, 0, -1), (0, 0.3, -1), (0, 0.8, -0.7), (0, 1, -0.2),
               (0, 1, 0.3), (0, 0.6, 0.8), (0, 0.1, 1), (0, 0, 1)]
        strap(p, path, 0.044, 0.005, "Olive", normals=nrm)
        # A buckle where it meets the panel.
        p.box((0.05, 0.016, 0.006), (x, -0.06, PANEL_Z - 0.004), "Iron", bevel=0.0015, segs=1)
    # The belt, with its buckle in front.
    n = 40
    ring_out = [belt_point(2 * math.pi * k / n) for k in range(n)]
    rings = []
    for k in range(n + 1):
        a = 2 * math.pi * k / n
        c = belt_point(a)
        out = Vector((math.sin(a) / BELT_A, 0, -math.cos(a) / BELT_B)).normalized()
        rings.append([c + out * BELT_T / 2 + Vector((0, -BELT_H / 2, 0)),
                      c + out * BELT_T / 2 + Vector((0, BELT_H / 2, 0)),
                      c - out * BELT_T / 2 + Vector((0, BELT_H / 2, 0)),
                      c - out * BELT_T / 2 + Vector((0, -BELT_H / 2, 0))])
    p.loft(rings[:-1] + [rings[0]], "Olive", caps=False)
    front = belt_point(0.0)
    p.box((0.06, 0.05, 0.012), (0.0, BELT_Y, front.z - 0.006), "Iron", bevel=0.003, segs=2)
    p.box((0.044, 0.034, 0.006), (0.0, BELT_Y, front.z - 0.0125), "Steel", bevel=0.002, segs=1)
    # Straps from the panel's lower corners down to the belt.
    for sx in (-1, 1):
        x = 0.16 * sx
        top = Vector((x, -0.255, panel_z(x) + 0.004))
        a = math.asin(min(1.0, abs(x) / BELT_A)) * sx
        bot = belt_point(a, grow=0.004, y=BELT_Y + 0.015)
        mid = (top + bot) / 2 + Vector((0, 0, -0.01))
        out = Vector((0, 0, -1))
        strap(p, [top, mid, bot], 0.03, 0.004, "Olive", normals=[out, out, out])
    # The mount's tab on the left strap (the mount itself is per slot).
    return p


# --- Holster ---------------------------------------------------------------

def holster():
    """Slot space: a PM hung barrel down (slide along slot -Z, its rear at
    y 0.02), grip back (+Z) out of the mouth."""
    p = Part("holster")
    # Side profile (z, y): round the slide and the trigger guard.
    prof = fillet([(-0.1, 0.012), (-0.1, -0.125), (-0.075, -0.155), (-0.035, -0.155),
                   (0.006, -0.075), (0.012, 0.012)], 0.012, 3)
    p.prism(prof, -0.019, 0.019, "Leather", axis="x", bevel=0.007, segs=2)
    # A welt round its front edge, sewn.
    welt = fillet([(-0.103, 0.006), (-0.103, -0.127), (-0.077, -0.158), (-0.035, -0.158),
                   (-0.015, -0.12)], 0.01, 2)
    pts = [Vector((0.0, y, z)) for z, y in welt]
    strap(p, pts, 0.008, 0.006, "Leather", up=(1, 0, 0))
    # The loop it hangs from the belt by, on its inner side.
    p.box((0.008, 0.07, 0.034), (-0.022, -0.005, -0.03), "Leather", bevel=0.002, segs=1)
    return p


def holster_flap(open_):
    p = Part("holster_flap_open" if open_ else "holster_flap_closed")
    if not open_:
        # Over the mouth, and down the front to a stud.
        p.box((0.042, 0.007, 0.118), (0.0, 0.016, -0.043), "Leather", bevel=0.0025, segs=2)
        p.box((0.04, 0.05, 0.007), (0.0, -0.009, -0.106), "Leather", bevel=0.0025, segs=2)
        p.cyl(0.005, 0.0, 0.004, "Iron", segs=10, xf=T(0.0, -0.025, -0.109) @ RX(math.pi))
    else:
        # Thrown forward off the mouth, hanging down the front.
        p.box((0.04, 0.11, 0.006), (0.0, -0.035, -0.113), "Leather", bevel=0.0025, segs=2,
              xf=RX(0.12, about=(0.0, 0.016, -0.1)))
    return p


# --- Pouches ---------------------------------------------------------------

# A magazine stands in it centred on the slot: an AK's is 0.026 x 0.2 x 0.09.
PH = (-0.108, 0.025)  # bottom, mouth (slot y)


def pouch():
    p = Part("pouch")
    prof = fillet([(-0.053, PH[0]), (0.05, PH[0]), (0.05, PH[1]), (-0.053, PH[1])], 0.008, 2)
    p.prism(prof, -0.021, 0.021, "Canvas", axis="x", bevel=0.005, segs=2)
    # A band of webbing round it, and a seam down its front.
    p.box((0.052, 0.016, 0.112), (0.0, -0.07, -0.0015), "Olive", bevel=0.002, segs=1)
    p.box((0.003, 0.11, 0.004), (0.0, -0.04, -0.0545), "Canvas", bevel=0.001, segs=1)
    return p


def pouch_flap(open_):
    p = Part("pouch_flap_open" if open_ else "pouch_flap_closed")
    if not open_:
        p.box((0.046, 0.006, 0.106), (0.0, PH[1] + 0.0025, -0.0015), "Canvas", bevel=0.002, segs=1)
        p.box((0.044, 0.04, 0.006), (0.0, PH[1] - 0.016, -0.057), "Canvas", bevel=0.002, segs=1)
        p.cyl(0.004, 0.0, 0.003, "Iron", segs=10, xf=T(0.0, PH[1] - 0.026, -0.0605) @ RX(math.pi))
    else:
        # Open, hanging down the front.
        p.box((0.044, 0.07, 0.005), (0.0, PH[1] - 0.033, -0.059), "Canvas", bevel=0.002, segs=1,
              xf=RX(0.1, about=(0.0, PH[1], -0.055)))
    return p


def pocket():
    p = Part("pocket")
    prof = fillet([(-0.017, -0.062), (0.016, -0.062), (0.016, 0.012), (-0.017, 0.012)], 0.007, 2)
    p.prism(prof, -0.032, 0.032, "Canvas", axis="x", bevel=0.005, segs=2)
    # Its loop over the belt.
    p.box((0.03, 0.06, 0.006), (0.0, 0.01, 0.018), "Olive", bevel=0.0015, segs=1)
    return p


def pocket_flap(open_):
    p = Part("pocket_flap_open" if open_ else "pocket_flap_closed")
    if not open_:
        p.box((0.068, 0.006, 0.038), (0.0, 0.0145, -0.0005), "Canvas", bevel=0.002, segs=1)
        p.box((0.06, 0.03, 0.006), (0.0, 0.0, -0.0205), "Canvas", bevel=0.002, segs=1)
    else:
        p.box((0.064, 0.05, 0.005), (0.0, -0.012, -0.022), "Canvas", bevel=0.002, segs=1,
              xf=RX(0.12, about=(0.0, 0.012, -0.018)))
    return p


def mount():
    """An open loop (a light or radio slides down into it), on the left
    shoulder strap."""
    p = Part("mount")
    p.tube(0.024, 0.019, -0.05, 0.002, "Olive", segs=16,
           xf=T(0.0, 0.0, -0.008) @ RX(-math.pi / 2))
    p.box((0.03, 0.06, 0.006), (0.0, -0.02, 0.016), "Olive", bevel=0.0015, segs=1)
    return p


# --- Backpack --------------------------------------------------------------

def pack():
    p = Part("pack")
    hx, hy, hz = 0.175, 0.245, 0.105
    outline = fillet([(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)], 0.04, 4)
    p.prism(outline, -hy, hy, "Canvas", axis="y", bevel=0.012, segs=2)
    inner = Part("pack_inside")
    w = 0.007
    io = fillet([(-hx + w, -hz + w), (hx - w, -hz + w), (hx - w, hz - w), (-hx + w, hz - w)],
                0.034, 4)
    inner.prism(io, -hy + 0.012, hy + 0.05, "Canvas", axis="y")
    p.cut(inner)
    return p


def pack_extras():
    p = Part("pack_extras")
    hx, hy, hz = 0.175, 0.245, 0.105
    # Side pockets and a big outer pocket on the face away from the back.
    for sx in (-1, 1):
        prof = fillet([(-0.05, -0.2), (0.05, -0.2), (0.05, -0.02), (-0.05, -0.02)], 0.02, 3)
        p.prism([(z, y) for z, y in prof], sx * hx - 0.01 * sx, sx * (hx + 0.035), "Canvas",
                axis="x", bevel=0.008, segs=2)
        p.box((0.006, 0.03, 0.09), (sx * (hx + 0.037), -0.04, 0.0), "Leather", bevel=0.0015, segs=1)
    prof = fillet([(-0.13, -0.22), (0.13, -0.22), (0.13, -0.04), (-0.13, -0.04)], 0.03, 3)
    p.prism(prof, hz - 0.01, hz + 0.04, "Canvas", axis="z", bevel=0.01, segs=2)
    for x in (-0.07, 0.07):
        p.box((0.022, 0.12, 0.006), (x, -0.07, hz + 0.042), "Leather", bevel=0.0015, segs=1)
        p.box((0.028, 0.02, 0.006), (x, -0.12, hz + 0.045), "Iron", bevel=0.0015, segs=1)
    # Shoulder straps on its back (-Z) face, padded at the top.
    for sx in (-1, 1):
        x = 0.085 * sx
        path = [(x, hy - 0.01, -hz + 0.004), (x, hy - 0.06, -hz - 0.03), (x * 1.05, 0.05, -hz - 0.045),
                (x * 1.15, -0.12, -hz - 0.03), (x * 1.2, -hy + 0.03, -hz - 0.003)]
        strap(p, path, 0.055, 0.012, "Padding", normals=[(0, 0, -1)] * 5)
    # Grab loop on top.
    p.box((0.05, 0.012, 0.008), (0.0, hy + 0.004, -hz + 0.02), "Leather", bevel=0.002, segs=1)
    return p


def lid(open_):
    p = Part("pack_lid_open" if open_ else "pack_lid_closed")
    hx, hy, hz = 0.175, 0.245, 0.105
    outline = fillet([(-hx - 0.006, -hz - 0.004), (hx + 0.006, -hz - 0.004),
                      (hx + 0.006, hz + 0.02), (-hx - 0.006, hz + 0.02)], 0.04, 4)
    hinge = (0.0, hy + 0.012, -hz)
    if not open_:
        p.prism(outline, hy - 0.004, hy + 0.024, "Canvas", axis="y", bevel=0.009, segs=2)
        for x in (-0.07, 0.07):
            p.box((0.022, 0.1, 0.006), (x, hy - 0.04, hz + 0.024), "Leather", bevel=0.0015, segs=1)
    else:
        # Thrown back, hanging down outside the straps.
        xf = T(0.0, 0.0, -0.058) @ RX(math.pi / 2, about=hinge)
        p.prism(outline, hy - 0.004, hy + 0.024, "Canvas", axis="y", bevel=0.009, segs=2, xf=xf)
    return p


# --- Build -----------------------------------------------------------------

def place(obj, pos):
    """Put an object built about the origin at `pos` (game space)."""
    obj.matrix_world = TO_BLENDER @ T(*pos) @ TO_BLENDER.inverted()


POUCH0 = Vector((-0.14, POUCH_Y, POUCH_Z))


def slots(open_):
    """The slot pieces at their places on the rig (the game hangs each
    mesh on its own slot), with their flaps open or closed."""
    for part, at in ((holster(), HOLSTER), (holster_flap(open_), HOLSTER),
                     (pouch(), POUCH0), (pouch_flap(open_), POUCH0),
                     (pocket(), POCKET), (pocket_flap(open_), POCKET)):
        place(make(part), at)


# rig.glb: the rig as worn, empty (flaps closed), and the mount.
main_guard()
make(harness())
slots(False)
place(make(mount()), MOUNT)
report()
export("assets/models/kit/rig.glb")

# rig_open.glb: the same slots with their flaps open (something's in them).
main_guard()
slots(True)
report()
export("assets/models/kit/rig_open.glb")

main_guard()
join("pack", pack(), pack_extras())
make(lid(False))
report()
export("assets/models/kit/backpack.glb")

main_guard()
join("pack", pack(), pack_extras())
make(lid(True))
report()
export("assets/models/kit/backpack_open.glb")
