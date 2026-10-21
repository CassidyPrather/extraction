"""The sentry (src/robots/sentry.rs): an armoured turret head on a mast,
a searchlight on its brow, a twin-barrelled gun slung under it, and a
junction box at the end of its cable. Writes assets/models/robots/sentry.glb.

World scale, metres, +Y up, -Z forward. Objects and their origins (each
the spawn offset of its part or piece):

- `head`   (0, 0, 0)          the turret head (the robot root, at the top of
                              the mast): plated, a yoke under it on the mast's
                              slewing ring; collider 0.5 x 0.36 x 0.6
- `eye`    (0, 0.05, -0.32)   its visor and lens; collider 0.2 x 0.12 x 0.06
- `lamp`   (0.17, 0.13, -0.27) the searchlight on its brow (the game hangs
                              a spotlight here at night)
- `gun`    (0, -0.22, -0.35)  the gun pod; collider 0.12 x 0.12 x 0.6
- `mast`   (0, 0, 0)          from its footing on the ground up to the ring
                              under the head, 4.5 m; pegs up one side, a
                              hazard band at shoulder height, a cabinet
- `box`    (3, 0.4, 0)        the junction box, centred on its origin;
                              0.5 x 0.8 x 0.35, its conduit going down into
                              the ground (apart in the file, as in the game)
- `cable`  (0, 0, 3)          a metre of cable along Z, centred on its origin
                              (the game stretches it to each segment)

Every object but the cable has `<name>_lod1` and `<name>_lod2` stand-ins.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, T, export, fillet, make, report, reset

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
MAST = 4.6
SEGS = (24, 12, 6)
# (The box and the cable stand apart from the rest in the file: in the
# game they're metres away. Their meshes are relative to these.)
BOX = (3.0, 0.4, 0.0)
CABLE = (0.0, 0.0, 3.0)


def suffix(lod):
    return "" if lod == 0 else f"_lod{lod}"


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.012 if lod < 2 else 0.0
    out = []

    # --- Head ------------------------------------------------------------------
    head = Part("head" + suffix(lod))
    # A wedge-nosed body: plated sides, sloped brow.
    side = fillet([(0.3, -0.17), (-0.26, -0.17), (-0.3, -0.06), (-0.22, 0.17), (0.26, 0.17),
                   (0.3, 0.1)], [0.03, 0.03, 0.03, 0.03, 0.03, 0.03], 3 if full else 1)
    head.prism(side, -0.24, 0.24, "Shell", axis="x", bevel=bev, segs=2 if full else 1)
    # Dark cheek plates either side, proud.
    for sx in (-1, 1):
        cheek = fillet([(0.2, -0.13), (-0.2, -0.13), (-0.18, 0.12), (0.2, 0.12)], 0.02,
                       2 if full else 1)
        head.prism(cheek, sx * 0.24, sx * 0.255, "Dark", axis="x", segs=1)
    # The yoke under it, on the ring at the top of the mast.
    head.cyl(0.13, -0.32, -0.17, "Dark", segs=n, xf=UP, chamfer=0.01 if lod < 2 else 0.0)
    head.cyl(0.16, -0.36, -0.32, "Steel", segs=n, xf=UP, chamfer=0.006 if lod < 2 else 0.0)
    if lod < 2:
        # A cyan strip round its middle: lit while it's powered.
        head.box((0.012, 0.016, 0.5), (0.258, -0.02, 0.0), "Glow", segs=1)
        head.box((0.012, 0.016, 0.5), (-0.258, -0.02, 0.0), "Glow", segs=1)
        # An antenna stub at the back.
        head.cyl(0.008, 0.17, 0.42, "Dark", segs=6, xf=T(-0.12, 0, 0.2) @ UP)
    if full:
        # Hazard chevrons on its back.
        head.box((0.2, 0.06, 0.006), (0.0, 0.0, 0.303), "Warn", segs=1)
    out.append(head)

    # --- Eye -------------------------------------------------------------------
    EYE = (0.0, 0.05, -0.32)
    eye = Part("eye" + suffix(lod), origin=EYE)
    ex, ey, ez = EYE
    eye.box((0.2, 0.1, 0.05), (ex, ey, ez + 0.01), "Dark", bevel=0.01 if lod < 2 else 0.0, segs=1)
    eye.cyl(0.03, -0.006, 0.008, "Eye", segs=max(8, n), xf=T(ex, ey, ez - 0.015) @ RY(math.pi))
    out.append(eye)

    # --- Searchlight -------------------------------------------------------------
    LAMP = (0.17, 0.13, -0.27)
    lamp = Part("lamp" + suffix(lod), origin=LAMP)
    lx, ly, lz = LAMP
    lamp.tube(0.07, 0.05, -0.06, 0.08, "Dark", segs=n, xf=T(lx, ly, lz), chamfer=0.006 if lod < 2 else 0.0)
    lamp.cyl(0.052, -0.055, -0.045, "Lamp", segs=n, xf=T(lx, ly, lz))
    lamp.box((0.03, 0.06, 0.06), (lx - 0.07, ly - 0.03, lz + 0.03), "Dark", segs=1)
    out.append(lamp)

    # --- Gun -------------------------------------------------------------------
    GUN = (0.0, -0.22, -0.35)
    gun = Part("gun" + suffix(lod), origin=GUN)
    gx, gy, gz = GUN
    gun.box((0.14, 0.12, 0.34), (gx, gy, gz + 0.1), "Dark", bevel=0.015 if lod < 2 else 0.0,
            segs=1)
    for sx in (-0.035, 0.035):
        gun.cyl(0.018, gz - 0.38, gz - 0.06, "Dark", segs=max(6, n // 2), xf=T(gx + sx, gy, 0))
        if lod < 2:
            gun.tube(0.024, 0.012, gz - 0.4, gz - 0.36, "Steel", segs=max(6, n // 2),
                     xf=T(gx + sx, gy, 0))
    out.append(gun)

    # --- Mast ------------------------------------------------------------------
    mast = Part("mast" + suffix(lod))
    top = MAST - 0.38
    # Footing: a concrete pad with a steel base plate.
    mast.box((1.0, 0.16, 1.0), (0.0, -0.04, 0.0), "Concrete", bevel=0.02 if lod < 2 else 0.0,
             segs=1)
    mast.box((0.5, 0.03, 0.5), (0.0, 0.055, 0.0), "Steel", bevel=0.006 if lod < 2 else 0.0, segs=1)
    # The pole, tapering.
    mast.lathe([(0.0, 0.07), (0.15, 0.07), (0.13, 0.5), (0.1, top), (0.0, top)], "Shell", segs=n,
               xf=UP)
    # The slewing ring at the top.
    mast.cyl(0.17, top - 0.06, top + 0.02, "Dark", segs=n, xf=UP, chamfer=0.01 if lod < 2 else 0.0)
    if lod < 2:
        # Pegs up one side, every 35 cm (a hand's span of a ladder).
        y = 0.6
        while y < top - 0.3:
            r = 0.13 - (y - 0.5) / (top - 0.5) * 0.03
            mast.cyl(0.012, r - 0.01, r + 0.14, "Steel", segs=6, xf=T(0, y, 0) @ SIDE)
            y += 0.35
        # A hazard band at shoulder height, and its cabinet below it.
        mast.lathe([(0.137, 1.35), (0.137, 1.5), (0.125, 1.5), (0.125, 1.35)], "Warn", segs=n,
                   xf=UP, closed=True)
        mast.box((0.26, 0.4, 0.14), (0.0, 1.0, 0.19), "Shell", bevel=0.015, segs=1)
        # The cable's conduit down the mast into the footing.
        mast.cyl(0.02, 0.06, 0.95, "Dark", segs=8, xf=T(0, 0, 0.24) @ UP)
    out.append(mast)

    # --- Junction box -------------------------------------------------------------
    bx, by, bz = BOX
    box = Part("box" + suffix(lod), origin=BOX)
    box.box((0.5, 0.8, 0.35), (bx, by, bz), "Shell", bevel=0.02 if lod < 2 else 0.0,
            segs=2 if full else 1)
    box.box((0.44, 0.06, 0.37), (bx, by + 0.43, bz), "Dark", bevel=0.008 if lod < 2 else 0.0,
            segs=1)
    box.cyl(0.04, -0.6, -0.38, "Dark", segs=max(6, n // 2), xf=T(bx, by, bz) @ UP)
    if lod < 2:
        box.box((0.3, 0.05, 0.006), (bx, by + 0.25, bz - 0.179), "Warn", segs=1)
        box.box((0.04, 0.04, 0.008), (bx + 0.15, by + 0.32, bz - 0.178), "Glow", segs=1)
        # Hinges and a latch.
        for y in (-0.25, 0.25):
            box.cyl(0.012, y - 0.05, y + 0.05, "Steel", segs=8, xf=T(bx - 0.25, by, bz - 0.17) @ UP)
        box.box((0.03, 0.08, 0.02), (bx + 0.22, by, bz - 0.18), "Steel", segs=1)
    out.append(box)

    if lod == 0:
        cable = Part("cable", origin=CABLE)
        cable.cyl(0.018, -0.5, 0.5, "Rubber", segs=8, xf=T(*CABLE))
        out.append(cable)
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        make(p)
report()
export("assets/models/robots/sentry.glb")
