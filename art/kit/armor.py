"""Wearable protection: the steel chest plate, the steel helmet and the
plate carrier (vest). Writes

- assets/models/items/steel_plate.glb  the plate as an item (centred on its
  box): 6.5 mm armour steel, 0.25 x 0.30, curved round the chest, the top
  corners cut away for the rifle's butt; front (convex) face to -Z
- assets/models/items/helmet.glb       the helmet as an item (centred)
- assets/models/kit/helmet.glb         the helmet worn, in head space (the
  HMD's pose): origin at the eye centre, -Z forward; the rim 2.5 cm above
  the eyes, its front edge at z -0.058. Empty `pt_helmet` at the origin
- assets/models/items/vest.glb         the vest as an item (centred)
- assets/models/kit/vest.glb           the vest worn, in torso space (as
  rig.glb's harness, src/interact/body.rs): a front and a back panel with
  a plate pocket each, padded shoulder straps, a cummerbund round the ribs.
  It takes the place of the harness's chest panel (its front face at
  PANEL_Z, -0.104, so the pouches stay where they are; Hands hides the
  panel and its webbing while it's worn), the back panel's face at
  +0.125. Nodes `plate_front` and `plate_back`: the plates in their
  pockets. Empties `pt_plate_front` and `pt_plate_back` where a plate sits
  (a plate item's own frame there: front face to -Z, the back one turned
  pi about Y so its face points out the back), `pt_vest_stow`

Each item model carries a `<name>_lod1` node: the same thing in a fraction
of the triangles, which the game draws instead past a few metres.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, S, T, Part, empty, export, make, report, reset  # noqa: E402
from mathutils import Vector  # noqa: E402

PLATE_W, PLATE_H, PLATE_T = 0.25, 0.30, 0.0065
PLATE_R = 0.32          # the curve round the chest


def wanted(name):
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or name in argv


def centre_parts(parts, points=None):
    """Move Parts (geometry and origins) so their joint bounds are centred
    on the origin, as an item's box is; the same shift for `points`."""
    vs = [v.co for p in parts for v in p.bm.verts]
    lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
    hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
    d = -(lo + hi) / 2
    for p in parts:
        for v in p.bm.verts:
            v.co += d
        p.origin = p.origin + d
    return {k: tuple(Vector(v) + d) for k, v in (points or {}).items()}, hi - lo


def write(path, parts, points=None):
    for p in parts:
        make(p)
    for k, v in (points or {}).items():
        empty(k, v)
    print(f"{path}:")
    report()
    export(path)


# --- Plate --------------------------------------------------------------------------------

def plate_half_width(y):
    """Half the plate's width at height y (0 at the bottom): full, then
    the shooter's cuts narrowing the top 7 cm."""
    cut = PLATE_H - 0.07
    if y <= cut:
        return PLATE_W / 2
    k = (y - cut) / 0.07
    return PLATE_W / 2 - 0.045 * k


def plate(p, xf, nx=9, ny=9, mat="Painted"):
    """The curved plate, bottom edge at y 0, centred on x, its front face
    bowed toward -Z; then placed by `xf`."""
    ys = [PLATE_H * j / (ny - 1) for j in range(ny)]
    rings = []
    for y in ys:
        hw = plate_half_width(y)
        xs = [-hw + 2 * hw * i / (nx - 1) for i in range(nx)]
        # The front face: the chord's middle is furthest forward.
        front = [Vector((x, y, (x * x) / (2 * PLATE_R) - PLATE_T / 2)) for x in xs]
        back = [Vector((v.x, v.y, v.z + PLATE_T)) for v in reversed(front)]
        rings.append([xf @ v for v in front + back])
    p.loft(rings, mat)


def steel_plate():
    reset()
    a = Part("steel_plate")
    plate(a, T(0, 0, 0))
    lod = Part("steel_plate_lod1")
    plate(lod, T(0, 0, 0), nx=3, ny=3)
    centre_parts([a, lod])
    write("assets/models/items/steel_plate.glb", [a, lod])


# --- Helmet -------------------------------------------------------------------------------

# The shell's profile (radius from the crown's axis, height above the rim),
# a rolled rim, and the flare round the back and sides.
SHELL = [(0.143, -0.006), (0.139, 0.004), (0.134, 0.03), (0.128, 0.066), (0.115, 0.1),
         (0.092, 0.13), (0.06, 0.153), (0.0, 0.165)]


def helmet(p, xf, segs=28, liner=True, mat="Olive"):
    """The steel helmet, its rim at y 0, front to -Z (then `xf`)."""
    inner = [(max(0.0, r - 0.0022), y - 0.0018) for r, y in reversed(SHELL)]
    prof = SHELL + inner + [(0.137, -0.004)]
    p.lathe(prof, mat, segs=segs, closed=True, xf=xf @ S(0.9, 1.0, 1.0) @ RX(-math.pi / 2))
    if not liner:
        return
    # The liner: a leather band inside the rim.
    p.lathe([(0.122, 0.018), (0.124, 0.02), (0.124, 0.045), (0.122, 0.047)], "Leather",
            segs=segs, closed=True, xf=xf @ S(0.9, 1.0, 1.0) @ RX(-math.pi / 2))
    # Rivets at the sides, and the chin strap hanging from them.
    for sx in (-1, 1):
        p.cyl(0.004, -0.002, 0.002, "Steel", segs=8,
              xf=xf @ T(sx * 0.1235, 0.035, 0.0) @ RY(math.pi / 2))
        p.box((0.004, 0.11, 0.016), (sx * 0.118, -0.03, 0.0), "Leather", segs=1, xf=xf)
    p.box((0.2, 0.004, 0.016), (0.0, -0.086, 0.0), "Leather", segs=1, xf=xf)


def helmets():
    reset()
    a = Part("helmet")
    helmet(a, T(0, 0, 0))
    lod = Part("helmet_lod1")
    helmet(lod, T(0, 0, 0), segs=12, liner=False)
    centre_parts([a, lod])
    write("assets/models/items/helmet.glb", [a, lod])
    reset()
    w = Part("helmet")
    # Head space is the HMD's: origin at the eye centre. The rim sits
    # 2.5 cm above the eyes, the skull's centre 8.5 cm behind them, so the
    # brim's front edge is at z -0.058 (clear of the view).
    helmet(w, T(0.0, 0.025, 0.085))
    write("assets/models/kit/helmet.glb", [w], {"pt_helmet": (0.0, 0.0, 0.0)})


# --- Vest ---------------------------------------------------------------------------------

# The panels' faces on the body: the front's outer face where the rig's
# chest panel's is (PANEL_Z, so the pouches stay put), the back's 0.125
# behind the origin (where the pack's straps meet it).
FRONT_Z, BACK_Z, VEST_T = -0.076, 0.097, 0.028
PANEL_W, PANEL_Y0, PANEL_Y1 = 0.29, -0.31, 0.05


def panel(p, z_body, out, mat="Canvas", nx=9, ny=6):
    """A padded panel lying on the chest (out = -1) or the back (+1): a
    soft-cornered slab curving round the ribs, thickness VEST_T."""
    rings = []
    for j in range(ny):
        y = PANEL_Y0 + (PANEL_Y1 - PANEL_Y0) * j / (ny - 1)
        # The top narrows to the shoulder straps' width.
        hw = PANEL_W / 2 - (0.03 if j == ny - 1 else 0.0)
        xs = [-hw + 2 * hw * i / (nx - 1) for i in range(nx)]
        inner = [Vector((x, y, z_body - out * (x * x) / (2 * 0.3))) for x in xs]
        outer = [Vector((v.x, v.y, v.z + out * VEST_T)) for v in reversed(inner)]
        ring = inner + outer if out < 0 else outer + inner
        rings.append(ring)
    p.loft(rings, mat)


def strap_path(p, pts, w, t, mat):
    """A flat strap along a polyline in the YZ plane at its points' x."""
    rings = []
    for i, c in enumerate(pts):
        a = pts[max(i - 1, 0)]
        b = pts[min(i + 1, len(pts) - 1)]
        tng = (b - a).normalized()
        across = Vector((1, 0, 0))
        nrm = across.cross(tng).normalized()
        rings.append([c - across * w / 2 - nrm * t / 2, c + across * w / 2 - nrm * t / 2,
                      c + across * w / 2 + nrm * t / 2, c - across * w / 2 + nrm * t / 2])
    p.loft(rings, mat)


def vest(p, detail=True):
    nx, ny = (9, 6) if detail else (3, 2)
    panel(p, FRONT_Z, -1, nx=nx, ny=ny)
    panel(p, BACK_Z, +1, nx=nx, ny=ny)
    zf, zb = FRONT_Z - VEST_T, BACK_Z + VEST_T
    # Shoulder straps over the top, front panel to back.
    for sx in (-1, 1):
        x = sx * 0.1
        pts = [Vector((x, PANEL_Y1 - 0.01, FRONT_Z - VEST_T / 2)),
               Vector((x, 0.1, -0.075)), Vector((x * 1.05, 0.135, -0.035)),
               Vector((x * 1.08, 0.142, 0.01)), Vector((x * 1.05, 0.13, 0.06)),
               Vector((x, 0.09, 0.105)), Vector((x, PANEL_Y1 - 0.01, BACK_Z + VEST_T / 2))]
        strap_path(p, pts, 0.06, 0.012, "Canvas")
    # The cummerbund: a band round the ribs from the front panel's sides to
    # the back's (an ellipse, the front and back stretches inside the panels).
    n = 10 if detail else 4
    for sx in (-1, 1):
        rings = []
        for k in range(n + 1):
            a = -math.pi / 2 + math.pi * k / n          # front (-pi/2) to back
            x = sx * (0.155 + 0.035 * math.cos(a))
            z = (FRONT_Z + BACK_Z) / 2 + math.sin(a) * ((BACK_Z - FRONT_Z) / 2 + 0.005)
            c = Vector((x, -0.2, z))
            out = Vector((sx * math.cos(a), 0, math.sin(a))).normalized()
            rings.append([c - out * 0.004 + Vector((0, -0.07, 0)),
                          c - out * 0.004 + Vector((0, 0.07, 0)),
                          c + out * 0.004 + Vector((0, 0.07, 0)),
                          c + out * 0.004 + Vector((0, -0.07, 0))])
        if sx > 0:
            rings = [list(reversed(r)) for r in rings]
        p.loft(rings, "Canvas")
    if not detail:
        return
    # Webbing rows on the front (the rig's pouches sew on here when worn
    # without it), a flap over each plate pocket's mouth, a drag handle on
    # the back.
    for k in range(4):
        y = PANEL_Y0 + 0.05 + 0.045 * k
        p.box((0.25, 0.022, 0.004), (0.0, y, zf - 0.002), "Olive", segs=1)
    for z, out in ((zf, -1), (zb, 1)):
        p.box((0.22, 0.05, 0.006), (0.0, PANEL_Y1 - 0.035, z + out * 0.003), "Olive",
              bevel=0.002, segs=1)
        p.box((0.06, 0.02, 0.006), (0.0, PANEL_Y1 - 0.045, z + out * 0.0075), "Black", segs=1)
    strap_path(p, [Vector((0.0, 0.04, zb + 0.002)), Vector((0.0, 0.065, zb + 0.02)),
                   Vector((0.0, 0.085, zb + 0.004))], 0.05, 0.006, "Olive")
    # Side release buckles on the cummerbund.
    for sx in (-1, 1):
        p.box((0.008, 0.045, 0.03), (sx * 0.192, -0.2, (FRONT_Z + BACK_Z) / 2 - 0.06), "Black",
              bevel=0.002, segs=1)


def vests():
    pf = (0.0, (PANEL_Y0 + PANEL_Y1) / 2 - 0.02, FRONT_Z - VEST_T / 2)
    pb = (0.0, (PANEL_Y0 + PANEL_Y1) / 2 - 0.02, BACK_Z + VEST_T / 2)
    reset()
    w = Part("vest")
    vest(w)
    # The plates in their pockets (shown while one's in): the front one as
    # the item lies, the back one turned to face out the back.
    front = Part("plate_front", origin=pf)
    plate(front, T(pf[0], pf[1] - PLATE_H / 2, pf[2]))
    back = Part("plate_back", origin=pb)
    plate(back, T(pb[0], pb[1], pb[2]) @ RY(math.pi) @ T(0.0, -PLATE_H / 2, 0.0))
    for p_ in (w, front, back):
        make(p_)
    empty("pt_plate_front", pf)
    empty("pt_plate_back", pb, RY(math.pi))
    empty("pt_vest_stow", (0.0, 0.0, 0.0))
    print("assets/models/kit/vest.glb:")
    report()
    export("assets/models/kit/vest.glb")
    reset()
    a = Part("vest")
    vest(a)
    lod = Part("vest_lod1")
    vest(lod, detail=False)
    centre_parts([a, lod])
    write("assets/models/items/vest.glb", [a, lod])


if __name__ == "__main__":
    for name, build in (("steel_plate", steel_plate), ("helmet", helmets), ("vest", vests)):
        if wanted(name):
            build()
