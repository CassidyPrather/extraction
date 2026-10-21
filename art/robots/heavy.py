"""The heavy: the Custodian's demolition unit (src/robots/heavy.rs). A pale
hull the size of a van on four long legs, a sensor brow up front, one
manipulator arm off its right shoulder. Writes
assets/models/robots/heavy.glb, and the salvage that comes off it to
assets/models/items/heavy_{plate,cell,knee,claw}.glb.

Robot space as in src/robots/heavy.rs, at size 1 (the game scales the
whole robot by SIZE = 2): metres, +Y up, -Z forward. Objects and their
origins (each the spawn offset of its part):

- `hull`       (0, 0, 0)        the body: shell panels over a dark frame, hip
                                housings at the corners, a window down each flank
                                (where the vents sit, under the flank plates) and
                                one in its back (the cell's bay, under the hatch)
- `eye`        (0, 0.2, -0.56)  the sensor brow, three lenses
- `vent`       (0.365, 0, 0.05) the right flank's vent: a louvred grille, lit
                                (the game turns a copy round for the left)
- `bolt`       (0, 0, 0)        one plate bolt, a hex head facing +Z
- `thigh`      origin at the hip, along +Y, 0.8 to the knee
- `shin`       origin at the knee, along +Y, 0.95 to the foot (its pad)
- `knee`       origin at the knee: the actuator, a drum across X, lit
- `arm_upper`  origin at the shoulder, along +Y, 0.8 to the elbow
- `arm_fore`   origin at the elbow, along +Y, 0.95 to the claw's palm, the
               three fingers past it

Segments bend about X. Every object has `<name>_lod1` and `<name>_lod2`
stand-ins (fewer sides, no small parts) for 25 m and 70 m.

The items are world-sized (twice the robot space), centred:
- `heavy_plate` 1.08 x 0.66 x 0.06, its outer face +Z
- `heavy_cell`  0.44 x 0.29 x 0.21, its handle on +Z (out of the bay)
- `heavy_knee`  0.31 x 0.48 x 0.26, `heavy_claw` 0.32 x 0.56 x 0.32 (centred on their bounds)
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import Part, RX, RY, RZ, S, T, export, fillet, make, report, reset

UP = RX(-math.pi / 2)  # cyl axis Z -> Y
SIDE = RY(math.pi / 2)  # cyl axis Z -> X
UPPER, LOWER = 0.8, 0.95
SIZE = 2.0

# heavy.rs, at size 1.
HX, HY0, HY1, HZ = 0.40, -0.20, 0.28, 0.52
EYE = (0.0, 0.2, -0.56)
SHOULDER = (0.3, 0.12, -0.5)
VENT = (0.365, 0.0, 0.05)
HIPS = [(sx * 0.36, -0.2, sz * 0.45) for sx, sz in ((1, 1), (-1, 1), (-1, -1), (1, -1))]
# The flank windows (vents) and the back window (the cell's bay).
WIN_Y = 0.15
WIN_Z = (-0.20, 0.30)
BAY_X, BAY_Y = 0.24, (-0.15, 0.12)

SEGS = (24, 12, 6)


def suffix(lod):
    return "" if lod == 0 else f"_lod{lod}"


def rounded(w, h, r, cx=0.0, cy=0.0, n=3):
    return fillet([(cx - w / 2, cy - h / 2), (cx + w / 2, cy - h / 2), (cx + w / 2, cy + h / 2),
                   (cx - w / 2, cy + h / 2)], r, n)


def build(lod):
    n = SEGS[lod]
    full = lod == 0
    bev = 0.012 if lod < 2 else 0.0
    out = []

    # --- Hull ----------------------------------------------------------------
    hull = Part("hull" + suffix(lod))
    # The frame inside: dark, seen through the windows.
    # (Inside every shell face by a few mm: never in a plane with one.)
    hull.box((0.68, 0.29, 0.6), (0.0, 0.0, 0.11), "Dark", bevel=0.01 if full else 0.0, segs=1)
    # Top shell: the long back, rounded over its shoulders.
    top = fillet([(-HX, WIN_Y), (HX, WIN_Y), (HX, 0.22), (HX - 0.07, HY1), (-HX + 0.07, HY1),
                  (-HX, 0.22)], [0.0, 0.0, 0.05, 0.05, 0.05, 0.05], 3 if full else 1)
    hull.prism(top, -HZ + 0.02, HZ, "Shell", axis="z", bevel=bev, segs=2 if full else 1)
    # Belly.
    belly = fillet([(-HX + 0.03, HY0), (HX - 0.03, HY0), (HX, HY0 + 0.03), (HX, -WIN_Y),
                    (-HX, -WIN_Y), (-HX, HY0 + 0.03)], [0.02, 0.02, 0.02, 0.0, 0.0, 0.02],
                   2 if full else 1)
    hull.prism(belly, -HZ + 0.03, HZ - 0.01, "Shell", axis="z", bevel=bev, segs=1)
    # The face: a wedge of plate below the brow, the full width.
    face = fillet([(-0.52, -WIN_Y), (WIN_Z[0], -WIN_Y), (WIN_Z[0], WIN_Y), (-0.5, WIN_Y),
                   (-0.54, 0.06), (-0.55, -0.1)], [0.0, 0.0, 0.0, 0.02, 0.03, 0.02],
                  2 if full else 1)
    hull.prism(face, -HX, HX, "Shell", axis="x", bevel=bev, segs=1)
    # Behind the flank windows, before the bay.
    for sx in (-1, 1):
        hull.box((0.16, 2 * WIN_Y - 0.006, 0.12), (sx * (HX - 0.08), 0.0, 0.36), "Shell",
                 bevel=bev, segs=1)
        # The bay's pillars, either side of the cell.
        hull.box((HX - BAY_X, 2 * WIN_Y - 0.006, 0.1), (sx * (HX + BAY_X) / 2, 0.0, 0.474),
                 "Shell", bevel=bev, segs=1)
    # Over the bay.
    hull.box((2 * BAY_X + 0.02, WIN_Y - BAY_Y[1] - 0.012, 0.094),
             (0.0, (WIN_Y + BAY_Y[1]) / 2 - 0.003, 0.474),
             "Shell", bevel=0.004 if full else 0.0, segs=1)
    # The bay's dark back wall.
    hull.box((2 * BAY_X, BAY_Y[1] - BAY_Y[0], 0.02), (0.0, (BAY_Y[0] + BAY_Y[1]) / 2, 0.42),
             "Dark", segs=1)
    # Hip housings: a dark ball in a shell collar at each corner.
    for hx, hy, hz in HIPS:
        hull.sphere(0.075, (hx, hy, hz), "Dark", segs=n, rings=max(4, n // 2))
        if lod < 2:
            hull.tube(0.11, 0.078, -0.06, 0.03, "Shell", segs=n, xf=T(hx, hy, hz) @ UP,
                      chamfer=0.008 if full else 0.0)
    # The shoulder: a slewing drum and the ball the arm turns on.
    sx_, sy_, sz_ = SHOULDER
    hull.cyl(0.11, -0.1, 0.02, "Dark", segs=n, xf=T(sx_, sy_ + 0.08, sz_ + 0.05) @ UP,
             chamfer=0.01 if full else 0.0)
    hull.sphere(0.085, SHOULDER, "Dark", segs=n, rings=max(4, n // 2))
    if lod < 2:
        # Cyan strips along the top edges (5 mm proud): it reads by them in
        # fog, and they go out when it dies.
        for sx in (-1, 1):
            hull.box((0.012, 0.016, 0.86), (sx * (HX + 0.003), 0.19, 0.0), "Glow", bevel=0.003,
                     segs=1)
        # A dark spine down its back, and two amber beacons on it.
        hull.box((0.14, 0.02, 0.8), (0.0, HY1 + 0.006, 0.02), "Dark", bevel=0.005, segs=1)
        for z in (-0.3, 0.3):
            hull.cyl(0.03, HY1 + 0.012, HY1 + 0.05, "Beacon", segs=max(6, n // 2),
                     xf=T(0.0, 0.0, z) @ UP,
                     chamfer=0.006)
    if full:
        # Panel seams across the back (dark grooves, proud by 5 mm so they
        # read as inset strips without fighting the shell).
        for z in (-0.17, 0.17):
            for sx in (-1, 1):
                hull.box((0.24, 0.012, 0.012), (sx * 0.2, HY1 + 0.003, z), "Dark", segs=1)
        # Grab rungs on the hull's back corners: the Custodian services it.
        for sx in (-1, 1):
            hull.cyl(0.012, -0.08, 0.08, "Steel", segs=8,
                     xf=T(sx * (HX - 0.08), HY1 + 0.03, 0.44) @ SIDE)
            for k in (-1, 1):
                hull.cyl(0.01, HY1 - 0.005, HY1 + 0.03, "Steel", segs=6,
                         xf=T(sx * (HX - 0.08) + k * 0.075, 0, 0.44) @ UP)
        # Hazard chevrons over the bay.
        hull.box((0.36, 0.03, 0.006), (0.0, 0.205, HZ + 0.0035), "Warn", segs=1)
    # --- What makes it a heavy and not a big walker ------------------------
    # A demolition ram across its front, low: the face it leads with.
    hull.box((0.9, 0.12, 0.1), (0.0, -0.13, -0.6), "Dark", bevel=0.02 if lod < 2 else 0.0,
             segs=2 if full else 1)
    hull.box((0.94, 0.05, 0.06), (0.0, -0.205, -0.63), "Steel", bevel=0.01 if lod < 2 else 0.0,
             segs=1)
    for sx in (-1, 1):
        # Rams bracing it back into the belly.
        hull.cyl(0.03, -0.62, -0.42, "Steel", segs=max(6, n // 2), xf=T(sx * 0.3, -0.16, 0))
    # Two exhaust stacks up its back on the left, sooted at the tips: the
    # arm's on the right, these lean it the other way.
    for x, z, h in ((-0.27, 0.26, 0.36), (-0.17, 0.33, 0.28)):
        hull.cyl(0.045, HY1 - 0.02, HY1 + h, "Dark", segs=max(6, n // 2), xf=T(x, 0, z) @ UP,
                 chamfer=0.006 if lod < 2 else 0.0)
        if lod < 2:
            hull.tube(0.052, 0.03, HY1 + h - 0.05, HY1 + h + 0.005, "Char", segs=max(6, n // 2),
                      xf=T(x, 0, z) @ UP)
    # A sensor mast on the front left, its own little red eye on top.
    hull.cyl(0.018, HY1 - 0.02, HY1 + 0.42, "Dark", segs=max(6, n // 3), xf=T(-0.3, 0, -0.38) @ UP)
    hull.box((0.1, 0.06, 0.08), (-0.3, HY1 + 0.45, -0.38), "Shell", bevel=0.01 if lod < 2 else 0.0,
             segs=1)
    hull.box((0.05, 0.015, 0.012), (-0.3, HY1 + 0.45, -0.425), "Eye", segs=1)
    if lod < 2:
        # A handrail round its back, on posts: the Custodian's crews stood
        # up there once. (Each rail a metre and a half up, at your scale.)
        rail_y = HY1 + 0.07
        for sx in (-1, 1):
            x = sx * (HX - 0.05)
            hull.cyl(0.009, -0.3, 0.42, "Steel", segs=6, xf=T(x, rail_y, 0))
            for z in (-0.28, 0.06, 0.4):
                hull.cyl(0.008, HY1 - 0.005, rail_y, "Steel", segs=6, xf=T(x, 0, z) @ UP)
        # A ladder up its right flank, behind the plate.
        lx, z0, z1 = HX + 0.03, 0.35, 0.45
        for z in (z0, z1):
            hull.cyl(0.008, HY0 - 0.03, HY1 + 0.07, "Steel", segs=6, xf=T(lx, 0, z) @ UP)
        k = 0
        y = HY0 + 0.02
        while y < HY1 + 0.05:
            hull.cyl(0.006, z0, z1, "Steel", segs=6, xf=T(lx, y, 0))
            y += 0.13
            k += 1
        for z in (z0, z1):
            hull.box((0.03, 0.012, 0.012), (HX + 0.016, 0.0, z), "Steel", segs=1)
    if full:
        # Its number stencilled on each flank behind the plate, and hazard
        # bands on the ram.
        for sx in (-1, 1):
            for i, (dy, dz, h) in enumerate(((0.0, 0.0, 0.08), (0.0, 0.035, 0.08), (0.03, 0.07, 0.02),
                                              (0.0, 0.1, 0.08))):
                hull.box((0.006, h, 0.012), (sx * (HX + 0.003), 0.06 + dy, 0.32 + dz), "Dark",
                         segs=1)
        for x in (-0.3, -0.1, 0.1, 0.3):
            hull.box((0.08, 0.1, 0.006), (x, -0.13, -0.6535), "Warn", segs=1)
    out.append(hull)

    # --- Eye -----------------------------------------------------------------
    eye = Part("eye" + suffix(lod), origin=EYE)
    ex, ey, ez = EYE
    eye.box((0.26, 0.11, 0.09), (ex, ey, ez + 0.005), "Dark", bevel=0.015 if lod < 2 else 0.0,
            segs=2 if full else 1)
    for k, x in enumerate((-0.075, 0.0, 0.075)):
        r = 0.026 if k == 1 else 0.018
        eye.cyl(r, -0.004, 0.012, "Eye", segs=max(6, n), xf=T(ex + x, ey, ez - 0.045) @ RY(math.pi))
    if full:
        # A hood over the lenses.
        eye.box((0.28, 0.02, 0.05), (ex, ey + 0.065, ez - 0.02), "Shell", bevel=0.006, segs=1)
    out.append(eye)

    # --- Vent ------------------------------------------------------------------
    vent = Part("vent" + suffix(lod), origin=VENT)
    vx, vy, vz = VENT
    vent.box((0.06, 0.24, 0.44), (vx - 0.005, vy, vz), "Dark", bevel=0.006 if lod < 2 else 0.0,
             segs=1)
    slats = 7 if full else (4 if lod == 1 else 0)
    for i in range(slats):
        y = vy - 0.09 + 0.18 * i / max(1, slats - 1)
        vent.box((0.012, 0.012, 0.4), (vx + 0.031, y, vz), "Glow", segs=1)
    out.append(vent)

    # --- Bolt ------------------------------------------------------------------
    if lod < 2:
        # Big, and ringed amber: the weak point a player learns to look for.
        bolt = Part("bolt" + suffix(lod))
        bolt.cyl(0.04, -0.016, 0.016, "Steel", segs=6, chamfer=0.006 if full else 0.0)
        bolt.cyl(0.018, 0.016, 0.03, "Steel", segs=8)
        bolt.tube(0.054, 0.044, -0.012, 0.004, "Warn", segs=max(8, n))
        out.append(bolt)

    # --- Legs ------------------------------------------------------------------
    thigh = Part("thigh" + suffix(lod))
    # A boxy thigh: shell over a dark ram.
    thigh.box((0.15, UPPER - 0.16, 0.17), (0.0, UPPER / 2, 0.0), "Shell", bevel=0.03 if lod < 2 else 0.0,
              segs=2 if full else 1)
    thigh.sphere(0.08, (0, 0, 0), "Dark", segs=n, rings=max(4, n // 2))
    if lod < 2:
        thigh.cyl(0.03, 0.08, UPPER - 0.08, "Dark", segs=max(6, n // 2),
                  xf=T(0, 0, 0.1) @ UP)
        thigh.cyl(0.02, 0.06, 0.1, "Dark", segs=6, xf=T(0, 0.06, 0.06))
        thigh.cyl(0.02, 0.06, 0.1, "Dark", segs=6, xf=T(0, UPPER - 0.06, 0.06))
    out.append(thigh)

    shin = Part("shin" + suffix(lod))
    # Tapering to the ankle, a broad pad underfoot.
    sec = [(0.0, 0.06), (0.075, 0.08), (0.085, 0.3), (0.06, LOWER - 0.16), (0.045, LOWER - 0.1),
           (0.0, LOWER - 0.09)]
    shin.lathe(sec, "Shell", segs=n, xf=S(1.0, 1.0, 0.8) @ UP)
    shin.cyl(0.03, LOWER - 0.12, LOWER - 0.03, "Dark", segs=max(6, n // 2), xf=UP)
    shin.cyl(0.13, LOWER - 0.04, LOWER, "Dark", segs=n, xf=UP, chamfer=0.01 if lod < 2 else 0.0)
    if lod < 2:
        # Rubber tread lip.
        shin.tube(0.135, 0.09, LOWER - 0.035, LOWER - 0.01, "Rubber", segs=n, xf=UP)
    out.append(shin)

    knee = Part("knee" + suffix(lod))
    knee.cyl(0.1, -0.1, 0.1, "Dark", segs=n, xf=SIDE, chamfer=0.012 if lod < 2 else 0.0)
    if lod < 2:
        for sx in (-1, 1):
            knee.tube(0.085, 0.06, sx * 0.1, sx * 0.106, "Glow", segs=n, xf=SIDE)
            knee.cyl(0.05, sx * 0.104, sx * 0.118, "Steel", segs=max(6, n // 2), xf=SIDE)
    out.append(knee)

    # --- Arm -----------------------------------------------------------------
    # The demolition arm: heavier than a leg, rams along it.
    up_ = Part("arm_upper" + suffix(lod))
    up_.box((0.22, UPPER - 0.12, 0.22), (0.0, UPPER / 2, 0.0), "Shell", bevel=0.035 if lod < 2 else 0.0,
            segs=2 if full else 1)
    up_.sphere(0.11, (0, UPPER, 0), "Dark", segs=n, rings=max(4, n // 2))
    if lod < 2:
        for sx in (-1, 1):
            up_.cyl(0.035, 0.12, UPPER - 0.1, "Dark", segs=max(6, n // 2),
                    xf=T(sx * 0.06, 0, -0.135) @ UP)
        up_.box((0.23, 0.04, 0.006), (0.0, UPPER / 2, 0.1135), "Warn", segs=1)
    out.append(up_)

    fore = Part("arm_fore" + suffix(lod))
    fore.box((0.17, LOWER - 0.255, 0.18), (0.0, (LOWER - 0.255) / 2 + 0.075, 0.0), "Shell",
             bevel=0.03 if lod < 2 else 0.0, segs=2 if full else 1)
    if lod < 2:
        fore.cyl(0.03, 0.1, LOWER - 0.22, "Dark", segs=max(6, n // 2), xf=T(0, 0, -0.11) @ UP)
    # The palm: a drum the fingers hinge on.
    fore.cyl(0.13, LOWER - 0.22, LOWER - 0.06, "Dark", segs=n, xf=UP, chamfer=0.015 if lod < 2 else 0.0)
    # Three long fingers, curled: a hand that tears doors off.
    for k in range(3):
        a = k * 2 * math.pi / 3
        fx, fz = math.cos(a) * 0.09, math.sin(a) * 0.09
        rot = RY(-a) @ RZ(0.3)
        fore.box((0.05, 0.3, 0.07), (0.0, 0.15, 0.0), "Steel", bevel=0.012 if lod < 2 else 0.0,
                 segs=1, xf=T(fx, LOWER - 0.08, fz) @ rot)
        if lod < 2:
            fore.box((0.045, 0.1, 0.065), (0.0, 0.33, -0.02), "Dark", bevel=0.01, segs=1,
                     xf=T(fx, LOWER - 0.08, fz) @ rot @ RX(0.5, about=(0, 0.3, 0)))
    out.append(fore)
    return out


def items():
    """The salvage, world-sized and centred."""
    out = []
    plate = Part("heavy_plate")
    w, h, d = 1.08, 0.66, 0.05
    plate.prism(rounded(w, h, 0.05, n=4), -d / 2, d / 2 - 0.012, "Dark", axis="z", bevel=0.008, segs=1)
    plate.prism(rounded(w - 0.04, h - 0.04, 0.04, n=4), d / 2 - 0.02, d / 2, "Shell", axis="z",
                bevel=0.006, segs=2)
    # Stiffeners and the holes the bolts went through.
    for y in (-0.12, 0.12):
        plate.box((w - 0.2, 0.05, 0.012), (0.0, y, d / 2 + 0.004), "Shell", bevel=0.004, segs=1)
    for sx in (-1, 1):
        for sy in (-1, 1):
            plate.tube(0.04, 0.026, d / 2 - 0.005, d / 2 + 0.006, "Dark", segs=16,
                       xf=T(sx * (w / 2 - 0.08), sy * (h / 2 - 0.07), 0))
    plate.box((0.3, 0.05, 0.006), (-0.3, 0.24, d / 2 + 0.003), "Warn", segs=1)
    out.append(plate)

    cell = Part("heavy_cell")
    cw, ch, cd = 0.44, 0.28, 0.18
    cell.box((cw, ch, cd - 0.02), (0.0, 0.0, -0.01), "Dark", bevel=0.015, segs=2)
    for sx in (-1, 1):
        cell.box((0.05, ch + 0.01, cd - 0.03), (sx * (cw / 2 - 0.03), 0.0, -0.01), "Shell",
                 bevel=0.01, segs=1)
    # The window on its core, lit.
    cell.box((0.22, 0.05, 0.006), (0.0, 0.05, cd / 2 - 0.017), "Glow", segs=1)
    # The handle you pull it out by.
    cell.cyl(0.016, -0.12, 0.12, "Steel", segs=10, xf=T(0, -0.04, cd / 2 - 0.004) @ SIDE)
    for sx in (-1, 1):
        cell.cyl(0.014, cd / 2 - 0.03, cd / 2 - 0.002, "Steel", segs=8, xf=T(sx * 0.11, -0.04, 0))
    # Terminals on its front.
    for sx in (-1, 1):
        cell.cyl(0.02, -cd / 2 - 0.015, -cd / 2 + 0.01, "Copper", segs=10, xf=T(sx * 0.1, 0.0, 0))
    out.append(cell)

    knee = Part("heavy_knee")
    knee.cyl(0.13, -0.15, 0.15, "Dark", segs=24, xf=T(0, 0.1, 0) @ SIDE, chamfer=0.015)
    knee.tube(0.115, 0.08, 0.15, 0.158, "Glow", segs=24, xf=T(0, 0.1, 0) @ SIDE)
    knee.cyl(0.04, -0.25, 0.0, "Steel", segs=12, xf=T(0, 0.0, 0) @ UP)
    knee.box((0.16, 0.12, 0.16), (0.0, -0.18, 0.0), "Shell", bevel=0.02, segs=2)
    out.append(knee)

    claw = Part("heavy_claw")
    claw.cyl(0.16, 0.12, 0.35, "Dark", segs=24, xf=UP, chamfer=0.02)
    claw.box((0.2, 0.23, 0.22), (0.0, 0.255, 0.0), "Shell", bevel=0.03, segs=2)
    for k in range(3):
        a = k * 2 * math.pi / 3
        fx, fz = math.cos(a) * 0.11, math.sin(a) * 0.11
        claw.box((0.06, 0.34, 0.08), (0.0, -0.17, 0.0), "Steel", bevel=0.012, segs=1,
                 xf=T(fx, 0.14, fz) @ RY(-a) @ RZ(-0.25))
    out.append(claw)
    return out


reset()
for lod in (0, 1, 2):
    for p in build(lod):
        make(p)
report()
export("assets/models/robots/heavy.glb")

for p in items():
    reset()
    obj = make(p)
    # Centred on its bounds: the item's box (items.ron dims) sits round
    # its origin.
    me = obj.data
    lo = [min(v.co[k] for v in me.vertices) for k in range(3)]
    hi = [max(v.co[k] for v in me.vertices) for k in range(3)]
    mid = [(a + b) / 2 for a, b in zip(lo, hi)]
    for v in me.vertices:
        v.co = [v.co[k] - mid[k] for k in range(3)]
    size = [hi[k] - lo[k] for k in range(3)]
    # (Blender is Z up: game x, y, z = its x, z, y.)
    print(f"{p.name} dims {size[0]:.3f} {size[2]:.3f} {size[1]:.3f}")
    export(f"assets/models/items/{p.name}.glb")
