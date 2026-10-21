"""The flatbed lorry wreck: a small Soviet lorry (GAZ-52 family), its cab
faded grey-green, a plank bed with low drop sides. Writes
assets/models/props/wreck_lorry.glb, 6.4 x 2.3 x 2.2.

Prop space as in wrecks.py: metres, +Y up from the ground, -Z the front,
the origin on the ground under the middle of the wheelbase.

Objects (each one node; its origin is its pivot):

- `body`                      the cab, wings, chassis, engine, the bed
                              and its fixed sides (static)
- `door_fl`, `door_fr`        origin on the front hinge line at the door's
                              mid height; they swing about Y
- `bonnet`                    origin on its rear hinge; lifts about X
                              (alligator)
- `tailboard`                 origin on its bottom hinge at the bed's
                              floor; drops about X
- `wheel_fl`, `wheel_fr`      origin at the hub
- `wheel_rl`, `wheel_rr`      the twin rears, both tyres in one node;
                              origin at the hub between them
- `pt_seat_driver`            the driver's end of the bench (left-hand drive)
- `pt_bed`                    the middle of the bed floor, on its surface

Materials: GreyGreen, Rusty, Steel, Black, Dark, Iron, Rubber, Glass, Lens,
LampRed, Padding, Leather, Boards, Planks, Cream.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import RX, RY, RZ, T, Part, fillet, reset  # noqa: E402
from wrecks import door, rust, side_prism, wheel, write  # noqa: E402

ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> X

AX = 1.65                   # axles at z -+AX
HUB_Y, R = 0.43, 0.45       # the hubs sit low on flat tyres
W = 1.0                     # the cab's half width
BELT, ROOF = 1.55, 2.15
ZA, ZB, ZC = -0.98, 0.0, 0.32   # door hinge, door's back, cab's back
FLOOR = 1.18                # the bed's floor
BW, BZ0, BZ1 = 1.15, 0.5, 3.55  # the bed's half width and ends
SIDE = 0.5                  # drop side height


def twin(name, hub, sx, r=R, w=0.22, rim=0.27, gap=0.03):
    """Twin rear wheels on flat tyres, both round one hub: the outer rim
    dished out on side sx, the inner one in."""
    q = Part(name, origin=hub)
    x, y, z = hub
    for k in (-1, 1):
        c = x + k * sx * (w + gap) / 2
        q.lathe([(rim - 0.01, -w / 2), (r - 0.02, -w / 2), (r, -w / 2 + 0.03),
                 (r, w / 2 - 0.03), (r - 0.02, w / 2), (rim - 0.01, w / 2)], "Rubber", segs=24,
                xf=T(c, y, z) @ ALONG_X, closed=True)
        q.lathe([(0.0, -w / 2 + 0.02), (rim, -w / 2 + 0.02), (rim, w / 2 - 0.02),
                 (0.0, w / 2 - 0.02)], "Rusty", segs=20, xf=T(c, y, z) @ ALONG_X, closed=True)
    # The hub and its nuts showing through the outer rim.
    xo = x + sx * (w + gap) / 2 + sx * (w / 2 - 0.02)
    q.cyl(0.12, *sorted((xo, xo + sx * 0.04)), "Iron", segs=16, xf=T(0.0, y, z) @ ALONG_X,
          chamfer=0.01)
    q.cyl(0.06, *sorted((xo + sx * 0.03, xo + sx * 0.08)), "Steel", segs=12,
          xf=T(0.0, y, z) @ ALONG_X, chamfer=0.008)
    return q


def planks(p, a0, a1, y0, n, z0, z1, mat, h=0.155, gap=0.012, axis="x"):
    """A wall of n planks stacked from y0, each z0..z1 long (in z) and
    a0..a1 thick (in x) on a side; with axis "z", a0..a1 is their length
    across in x and z0..z1 their thickness."""
    for k in range(n):
        ya = y0 + k * (h + gap)
        if axis == "x":
            p.prism(fillet([(z0, ya), (z1, ya), (z1, ya + h), (z0, ya + h)], 0.008, 1),
                    a0, a1, mat, axis="x")
        else:
            p.prism(fillet([(a0, ya), (a1, ya), (a1, ya + h), (a0, ya + h)], 0.008, 1),
                    z0, z1, mat, axis="z")


def lorry():
    reset()
    paint = "GreyGreen"
    b = Part("body")

    # --- The cab shell: the cowl and dash block in front of the doors,
    # the back and rear quarters a U round the bench, sills under the doors.
    cowl = fillet([(-W, -1.25), (W, -1.25), (W, ZA - 0.005), (-W, ZA - 0.005)],
                  [0.2, 0.2, 0.0, 0.0], 3)
    b.prism(cowl, 0.8, BELT, paint, axis="y", bevel=0.025, segs=2)
    back = fillet([(-W, ZB + 0.005), (-W, ZC), (W, ZC), (W, ZB + 0.005), (W - 0.07, ZB + 0.005),
                   (W - 0.07, ZC - 0.07), (-W + 0.07, ZC - 0.07), (-W + 0.07, ZB + 0.005)],
                  [0.0, 0.2, 0.2, 0.0, 0.0, 0.13, 0.13, 0.0], 3)
    b.prism(back, 0.8, ROOF - 0.12, paint, axis="y", bevel=0.02, segs=1)
    for sx in (-1, 1):
        b.box((0.065, 0.17, ZB - ZA + 0.01), (sx * (W - 0.0375), 0.89, (ZA + ZB) / 2), paint,
              bevel=0.01, segs=1)
    cut = Part("cut")
    # The rear window, and rust on the quarters, the cowl and the sills.
    cut.box((0.9, 0.3, 0.2), (0.0, 1.74, ZC - 0.03), "Dark", bevel=0.04, segs=2)
    for z, y, w, h in ((0.2, 0.95, 0.22, 0.2), (-1.15, 0.9, 0.16, 0.14), (0.15, 1.5, 0.12, 0.1),
                       (-0.5, 0.86, 0.4, 0.06)):
        for sx in (-1, 1):
            rust(cut, sx * W, y, z * (1 if sx > 0 else 0.96), w, h)
    b.cut(cut, paint=True)
    shell, b = b, Part("body_detail")

    # The roof, rounded all round, over the quarters and the pillars.
    roof = fillet([(-W - 0.01, -0.99), (W + 0.01, -0.99), (W + 0.01, ZC + 0.005),
                   (-W - 0.01, ZC + 0.005)], [0.12, 0.12, 0.21, 0.21], 3)
    b.prism(roof, ROOF - 0.17, ROOF, paint, axis="y", bevel=0.07, segs=3)
    b.box((1.1, 0.05, 0.7), (0.25, ROOF - 0.005, -0.3), "Rusty", bevel=0.02, segs=1)

    def strut(a, c, x, t=0.08, w=0.07, mat=paint):
        (za, ya), (zb, yb) = a, c
        L = math.hypot(zb - za, yb - ya)
        ang = math.atan2(yb - ya, zb - za)
        b.box((w, t, L + t), (0.0, 0.0, 0.0), mat, bevel=0.015, segs=1,
              xf=T(x, (ya + yb) / 2, (za + zb) / 2) @ RX(-ang))
    # The windscreen pillars and its middle bar, a few shards left in it.
    for sx in (-1, 1):
        strut((-1.12, BELT - 0.02), (-0.92, ROOF - 0.1), sx * (W - 0.075))
    strut((-1.12, BELT - 0.02), (-0.92, ROOF - 0.1), 0.0, t=0.05, w=0.05)
    glass = [(-1.11, BELT + 0.01), (-1.04, BELT + 0.22), (-1.06, BELT + 0.12),
             (-1.09, BELT + 0.07)]
    for x0, x1 in ((-0.88, -0.45), (0.3, 0.86)):
        b.prism(glass, x0, x1, "Glass", axis="x")
    b.prism([(-0.95, ROOF - 0.13), (-0.98, ROOF - 0.22), (-0.965, ROOF - 0.3)], 0.5, 0.75,
            "Glass", axis="x")

    # The cab's floor, the bench, the dash and wheel (left-hand drive), the
    # gear lever, the back of the engine through the floor.
    b.box((2 * W - 0.14, 0.07, ZC - 0.07 - ZA), (0.0, 0.845, (ZA + ZC - 0.07) / 2), "Dark",
          segs=1)
    b.box((1.7, 0.26, 0.42), (0.0, 1.01, -0.22), "Dark", bevel=0.02, segs=1)
    b.box((1.7, 0.14, 0.5), (0.0, 1.21, -0.24), "Leather", bevel=0.04, segs=1)
    b.box((1.7, 0.6, 0.13), (0.0, 1.6, 0.12), "Leather", bevel=0.04, segs=1,
          xf=RX(-0.15, about=(0.0, 1.3, 0.12)))
    b.box((0.6, 0.04, 0.4), (0.4, 1.285, -0.24), "Padding", bevel=0.015, segs=1,
          xf=RZ(0.08, about=(0.4, 1.285, -0.24)))
    b.box((2 * W - 0.16, 0.2, 0.22), (0.0, 1.43, ZA + 0.1), "Black", bevel=0.03, segs=1)
    b.tube(0.23, 0.21, -0.013, 0.013, "Black", segs=24, xf=T(-0.45, 1.6, -0.62) @ RX(-1.0))
    b.cyl(0.025, 0.0, 0.5, "Black", segs=8, xf=T(-0.45, 1.6, -0.62) @ RX(-1.0 + math.pi))
    b.cyl(0.015, 0.0, 0.5, "Steel", segs=6, xf=T(0.0, 0.86, -0.62) @ RX(-math.pi / 2 + 0.3))
    b.sphere(0.03, (0.0, 1.34, -0.47), "Black", segs=8, rings=5)

    # --- The front wings: a domed section swept over the wheel from the
    # running board to the grille, the headlamps in their noses.
    path = [(-0.99, 0.74), (-1.07, 0.84), (-1.19, 0.96), (-1.38, 1.07), (-1.6, 1.12),
            (-1.85, 1.13), (-2.1, 1.1), (-2.33, 1.02), (-2.52, 0.9), (-2.64, 0.74)]
    top = [(-0.2, -0.07), (-0.16, -0.01), (-0.08, 0.02), (0.02, 0.03), (0.1, 0.01),
           (0.16, -0.04), (0.2, -0.13)]
    for sx in (-1, 1):
        sec = [(sx * x, n) for x, n in top] + [(sx * (x - 0.025), n - 0.025)
                                               for x, n in reversed(top)]
        b.sweep(path, sec, paint, xf=T(sx * 0.8, 0.0, 0.0))
        # The headlamp pod and its lens, one lamp smashed; the side lamp.
        b.cyl(0.11, -2.56, -2.3, paint, segs=18, xf=T(sx * 0.8, 1.0, 0.0), chamfer=0.02)
        b.cyl(0.09, -2.575, -2.55, "Lens" if sx > 0 else "Dark", segs=18,
              xf=T(sx * 0.8, 1.0, 0.0), chamfer=0.008)
        b.cyl(0.035, -0.03, 0.03, "Cream", segs=10, xf=T(sx * 0.8, 1.17, -2.15) @ RX(-0.3))
        # The inner apron under the bonnet's edge, down to the frame.
        b.box((0.03, 0.4, 1.38), (sx * 0.585, 0.88, -1.95), "Dark", segs=1)
        # The running board, ribbed, on two brackets.
        b.box((0.36, 0.04, 1.32), (sx * 0.84, 0.71, -0.33), "Iron", bevel=0.008, segs=1)
        for k in range(4):
            b.box((0.3, 0.012, 0.02), (sx * 0.84, 0.735, -0.85 + k * 0.32), "Iron", segs=1)
        for z in (-0.8, 0.15):
            b.box((0.3, 0.04, 0.05), (sx * 0.58, 0.72, z), "Iron", segs=1)

    # The grille: a tall panel with its slots recessed, vertical bars in them.
    gr = Part("grille")
    gr.prism(fillet([(-0.55, 0.62), (0.55, 0.62), (0.55, 1.44), (-0.55, 1.44)],
                    [0.02, 0.02, 0.14, 0.14], 3), -2.72, -2.66, paint, axis="z", bevel=0.012,
             segs=1)
    gcut = Part("cut")
    gcut.box((0.78, 0.6, 0.06), (0.0, 1.06, -2.72), "Dark", bevel=0.02, segs=1)
    gr.cut(gcut, paint=True)
    for k in range(13):
        b.box((0.022, 0.6, 0.03), (-0.36 + k * 0.06, 1.06, -2.705), "Steel", segs=1)
    b.box((0.24, 0.07, 0.012), (0.0, 0.66, -2.725), "Cream", segs=1)

    # --- The chassis: two rails with crossmembers, leaf springs on beam
    # axles, the front bumper, the rear crossmember and its lamps.
    for sx in (-1, 1):
        b.box((0.08, 0.2, 6.15), (sx * 0.44, 0.76, 0.35), "Iron", segs=1)
        for z in (-AX, AX):
            b.box((0.07, 0.05, 1.2), (sx * 0.44, 0.635, z), "Iron", bevel=0.01, segs=1)
            b.box((0.06, 0.08, 0.06), (sx * 0.44, 0.57, z), "Iron", segs=1)
    for z in (-2.5, -0.9, 0.6, 2.1):
        b.box((0.8, 0.12, 0.08), (0.0, 0.78, z), "Iron", segs=1)
    b.box((1.36, 0.1, 0.1), (0.0, HUB_Y + 0.05, -AX), "Iron", segs=1)
    b.cyl(0.07, -0.66, 0.66, "Iron", segs=12, xf=T(0.0, HUB_Y, AX) @ ALONG_X)
    b.sphere(0.2, (0.0, HUB_Y, AX), "Iron", segs=14, rings=8, scale=(1.0, 0.85, 0.8))
    b.cyl(0.04, AX - 0.15, -0.9, "Iron", segs=8, xf=T(0.0, HUB_Y + 0.06, 0.0))
    b.box((2.1, 0.15, 0.09), (0.0, 0.0, 0.0), "Iron", bevel=0.015, segs=1,
          xf=T(0.0, 0.6, -2.77) @ RZ(0.05))
    for sx in (-1, 1):
        b.box((0.06, 0.1, 0.2), (sx * 0.44, 0.66, -2.67), "Iron", segs=1)
        b.box((0.16, 0.09, 0.05), (sx * 0.8, 0.79, 3.445), "LampRed", bevel=0.012, segs=1)
    b.box((2.0, 0.08, 0.08), (0.0, 0.79, 3.38), "Iron", segs=1)
    b.cyl(0.03, 3.4, 3.62, "Iron", segs=8, xf=T(0.0, 0.66, 0.0))

    # Under the bed: the fuel tank on its straps, the battery box, the
    # exhaust, mud flaps behind the twins.
    b.cyl(0.17, 0.62, 1.12, "Iron", segs=16, xf=T(-0.72, 0.68, 0.0), chamfer=0.03)
    for z in (0.72, 1.02):
        b.box((0.3, 0.03, 0.03), (-0.6, 0.86, z), "Steel", segs=1)
    b.box((0.36, 0.3, 0.36), (0.78, 0.7, 0.85), "Iron", bevel=0.02, segs=1)
    b.cyl(0.035, -1.4, 1.15, "Rusty", segs=8, xf=T(0.3, 0.5, 0.0))
    b.cyl(0.035, 0.0, 0.3, "Rusty", segs=8, xf=T(0.3, 0.5, 1.15) @ RY(0.9))
    for sx in (-1, 1):
        b.box((0.52, 0.5, 0.015), (0.0, 0.0, 0.0), "Rubber", segs=1,
              xf=T(sx * 0.84, 0.72, 2.3) @ RX(0.12 if sx > 0 else 0.0))
        b.box((0.56, 0.04, 0.04), (sx * 0.84, 0.97, 2.3), "Iron", segs=1)

    # The engine in the bay: the block, head and air filter, the radiator
    # behind the grille, the fan.
    b.box((0.5, 0.45, 0.95), (0.0, 0.92, -1.95), "Iron", bevel=0.03, segs=1)
    b.box((0.36, 0.1, 0.8), (0.0, 1.19, -1.95), "Rusty", bevel=0.01, segs=1)
    b.cyl(0.13, 1.24, 1.32, "Black", segs=16, xf=T(-0.1, 0.0, -1.55) @ RX(-math.pi / 2))
    b.box((0.94, 0.66, 0.08), (0.0, 1.05, -2.6), "Dark", segs=1)
    b.box((0.4, 0.06, 0.02), (0.0, 0.98, -2.5), "Black", segs=1)
    b.cyl(0.06, -2.53, -2.43, "Iron", segs=10, xf=T(0.0, 0.98, 0.0))

    # Mirrors: one left on its arm off the driver's pillar, the other's
    # arm bent and empty.
    b.cyl(0.012, 0.0, 0.32, "Black", segs=6, xf=T(-W + 0.01, BELT + 0.05, -1.0) @ RY(-1.3))
    b.box((0.03, 0.22, 0.14), (-W - 0.31, BELT + 0.14, -1.07), "Black", bevel=0.01, segs=1)
    b.cyl(0.012, 0.0, 0.3, "Black", segs=6, xf=T(W - 0.01, BELT + 0.05, -1.0) @ RY(1.5)
          @ RX(0.5))

    # --- The bed: sills on the rails, crossbeams, floor planks lengthwise
    # (a couple gone), a high front board, the drop sides on iron stakes.
    for sx in (-1, 1):
        b.box((0.12, 0.12, BZ1 - BZ0 - 0.05), (sx * 0.44, 0.92, (BZ0 + BZ1) / 2 - 0.025),
              "Boards", segs=1)
    for k in range(7):
        z = BZ0 + 0.06 + k * (BZ1 - BZ0 - 0.17) / 6
        b.box((2 * BW - 0.02, 0.14, 0.1), (0.0, 1.05, z), "Boards", segs=1)
        for sx in (-1, 1):
            b.box((0.05, 0.1, 0.12), (sx * (BW - 0.03), 1.06, z), "Iron", segs=1)
    n = 9
    pw = (2 * BW - 0.006 * (n - 1)) / n
    for k in range(n):
        if k in (2, 6):
            continue
        x = -BW + pw / 2 + k * (pw + 0.006)
        b.box((pw, 0.06, BZ1 - 0.05 - BZ0), (x, FLOOR - 0.03, (BZ0 + BZ1 - 0.05) / 2),
              "Planks", bevel=0.006, segs=1)
    # (One short broken stub of each missing plank, sagging.)
    for k, (z0, z1) in ((2, (BZ0, 1.4)), (6, (2.6, BZ1 - 0.05))):
        x = -BW + pw / 2 + k * (pw + 0.006)
        b.box((pw - 0.01, 0.05, z1 - z0), (0.0, 0.0, 0.0), "Planks", bevel=0.006, segs=1,
              xf=T(x, FLOOR - 0.045, (z0 + z1) / 2) @ RX(0.03 if k == 2 else -0.03))
    planks(b, -BW, BW, FLOOR + 0.004, 4, BZ0, BZ0 + 0.06, "Boards", axis="z")
    b.box((2 * BW + 0.02, 0.04, 0.07), (0.0, FLOOR + 0.69, BZ0 + 0.03), "Iron", segs=1)
    for sx in (-1, 1):
        b.box((0.04, 0.7, 0.08), (sx * (BW - 0.025), FLOOR + 0.35, BZ0 + 0.03), "Iron", segs=1)
        planks(b, BW - 0.05, BW, FLOOR + 0.004, 3, BZ0 + 0.065, BZ1 - 0.055, "Boards")
        for z in (BZ0 + 0.85, BZ0 + 1.85, BZ1 - 0.35):
            b.box((0.015, SIDE + 0.03, 0.07), (sx * (BW + 0.0075), FLOOR + SIDE / 2 - 0.01, z), "Iron",
                  segs=1)
        b.box((0.07, 0.03, BZ1 - BZ0 - 0.12), (sx * (BW - 0.015), FLOOR + SIDE + 0.01,
                                                (BZ0 + BZ1) / 2), "Iron", segs=1)
    # (A plank of the right side split and hanging off its stake.)
    b.box((0.04, 0.15, 1.1), (0.0, 0.0, 0.0), "Boards", segs=1,
          xf=T(BW + 0.045, FLOOR + 0.3, BZ1 - 0.62) @ RX(0.35))

    # The front wings' and the cowl's rust: cut into the wings would spoil
    # their sweep, so a few scraps lie painted on top of them.
    for sx, z in ((-1, -1.45), (1, -2.2)):
        b.box((0.16, 0.012, 0.2), (0.0, 0.0, 0.0), "Rusty", segs=1,
              xf=T(sx * 0.82, 1.14, z) @ RX(0.25 if z < -2 else 0.1))

    # --- The bonnet: an inverted U over the bay between the wings, its
    # louvres down the sides, rust through the top.
    bonnet = Part("bonnet", origin=(0.0, 1.47, -1.255))
    bonnet.prism(fillet([(-0.55, 1.0), (-0.55, 1.47), (0.55, 1.47), (0.55, 1.0), (0.525, 1.0),
                         (0.525, 1.445), (-0.525, 1.445), (-0.525, 1.0)],
                        [0.0, 0.15, 0.15, 0.0, 0.0, 0.125, 0.125, 0.0], 4),
                 -2.655, -1.255, paint, axis="z")
    for sx in (-1, 1):
        for k in range(5):
            bonnet.box((0.012, 0.02, 0.3), (sx * 0.556, 1.1 + k * 0.05, -2.0), "Dark", segs=1)
    bcut = Part("cut")
    rust(bcut, 0.2, 1.47, -2.1, 0.35, 0.3, n=5, axis="y")
    rust(bcut, -0.25, 1.47, -1.5, 0.25, 0.2, n=4, axis="y")
    rust(bcut, 0.55, 1.1, -2.45, 0.2, 0.12, n=4)
    bonnet.cut(bcut, paint=True)

    # --- The tailboard: planks between two iron stiles, hinged at the floor.
    tail = Part("tailboard", origin=(0.0, FLOOR, BZ1 - 0.025))
    planks(tail, -BW, BW, FLOOR + 0.004, 3, BZ1 - 0.05, BZ1, "Boards", axis="z")
    for x in (-BW + 0.03, -0.4, 0.4, BW - 0.03):
        tail.box((0.07, SIDE + 0.01, 0.02), (x, FLOOR + SIDE / 2, BZ1 + 0.01), "Iron", segs=1)
    tail.box((2 * BW, 0.03, 0.07), (0.0, FLOOR + SIDE + 0.01, BZ1 - 0.025), "Iron", segs=1)
    tail.box((0.4, 0.15, 0.012), (0.3, FLOOR + 0.25, BZ1 + 0.026), "Cream", segs=1)
    tcut = Part("cut")
    for k in (0, 1, 2):
        tcut.box((0.5 - 0.1 * k, 0.04, 0.03), (-0.6 + 0.5 * k, FLOOR + 0.08 + k * 0.16, BZ1),
                 "Planks", bevel=0.01, segs=1)
    tail.cut(tcut, paint=True)

    parts = [("body", shell, gr, b), bonnet, tail]
    for sx, s in ((-1, "l"), (1, "r")):
        parts.append(door(f"door_f{s}", sx, ZA, ZB - 0.03, 0.98, BELT,
                          [(ZA + 0.03, BELT), (ZA + 0.1, ROOF - 0.22), (ZB - 0.05, ROOF - 0.22),
                           (ZB - 0.05, BELT)], paint, xs=(W - 0.08, W)))
        parts.append(wheel(f"wheel_f{s}", (sx * 0.8, HUB_Y, -AX), sx, r=R, w=0.24, rim=0.28))
        parts.append(twin(f"wheel_r{s}", (sx * 0.83, HUB_Y, AX), sx))
    write("wreck_lorry", parts, {"pt_seat_driver": (-0.45, 1.28, -0.24),
                                 "pt_bed": (0.0, FLOOR, (BZ0 + 0.06 + BZ1 - 0.05) / 2)})


if __name__ == "__main__":
    lorry()
