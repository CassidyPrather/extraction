"""A wrecked field jeep by the Zone's roads, for Destruction to strip and World
models to place. Writes assets/models/props/wreck_uaz.glb.

Prop space: metres, +Y up from the ground, -Z the front, the origin on the
ground under the middle of the wheelbase. It sits low on flat tyres.

- `wreck_uaz`  a boxy four-door jeep, faded olive, 4.0 x 1.8 x 2.0, the
               canvas top gone to its bare hoops

Objects (each one node; its origin is its pivot):

- `body`                      the shell, chassis, engine, interior and hoops
                              (static)
- `door_fl`, `door_fr`,       origin on the front hinge line at the door's
  `door_rl`, `door_rr`        mid height; they swing about Y. A front door
                              is the `car_door` item's size
- `bonnet`                    origin on its rear hinge; lifts about X
- `tailgate`                  origin on its bottom hinge; drops about X.
                              The spare wheel rides on it
- `wheel_fl`, `wheel_fr`,     origin at the hub
  `wheel_rl`, `wheel_rr`
- `pt_seat_driver`            the driver's seat cushion (left-hand drive)

Materials: Olive, Rusty, Steel, Black, Dark, Iron, Rubber, Glass, Lens,
LampRed, Padding, Cream.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import RX, RY, RZ, T, Part, fillet, reset, section_circle  # noqa: E402
from wrecks import door, rust, side_prism, wheel, write  # noqa: E402

ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> X


def uaz():
    reset()
    W = 0.88                 # half width
    AX = 1.19                # axles at z -+AX
    HUB_Y, R = 0.37, 0.39    # the hubs sit low on flat tyres
    ARCH = 0.46
    BELT, WING = 1.15, 1.08  # the doors' tops; the wings' and scuttle's
    ZW = -0.86               # the scuttle, where the nose ends
    ZA, ZB, ZC = -0.84, 0.06, 0.8   # front door, B pillar, the rear doors' end
    ZB2 = 0.11               # the rear doors' front
    FRONT, TAIL = -1.95, 1.78
    paint = "Olive"
    b = Part("body")

    # The nose: solid under the bonnet, flat sided, the wheel arches cut out
    # of it. Behind the rear doors, the tub's thin walls.
    nose = fillet([(FRONT, 0.55), (ZW, 0.55), (ZW, WING), (FRONT + 0.03, WING),
                   (FRONT, WING - 0.03)], [0.03, 0.0, 0.0, 0.03, 0.03], 2)
    b.prism(nose, -W, W, paint, axis="x", bevel=0.015, segs=2)
    wall = fillet([(ZC + 0.01, 0.55), (TAIL, 0.55), (TAIL, BELT), (ZC + 0.01, BELT)],
                  [0.0, 0.02, 0.02, 0.02], 2)
    for sx in (-1, 1):
        side_prism(b, wall, W - 0.05, W, paint, sx, bevel=0.01, segs=1)
        # The sills under the doors.
        b.box((0.06, 0.105, ZC - ZA + 0.01), (sx * (W - 0.03), 0.5925, (ZA + ZC) / 2), paint,
              bevel=0.01, segs=1)
    cut = Part("cut")
    for z in (-AX, AX):
        for sx in (-1, 1):
            cut.cyl(ARCH, *sorted((sx * 0.56, sx * 1.0)), "Dark", segs=24,
                    xf=T(0.0, HUB_Y, z) @ ALONG_X)
    # The engine bay under the bonnet; the grille's recess in the front.
    cut.box((1.08, 0.5, 0.96), (0.0, 1.0, -1.4), "Dark")
    cut.box((0.92, 0.36, 0.08), (0.0, 0.84, FRONT), "Dark")
    # Rust eaten into the arches' lips, the sills and the tub's sides.
    for z, y, w, h in ((-1.72, 0.62, 0.22, 0.12), (-0.95, 0.66, 0.16, 0.14), (1.62, 0.66, 0.3, 0.14),
                       (1.4, 0.98, 0.34, 0.16), (0.3, 0.58, 0.4, 0.06)):
        for sx in (-1, 1):
            rust(cut, sx * W, y, z * (1 if sx > 0 else 0.96), w, h)
    b.cut(cut, paint=True)
    # Everything else on the body goes in a second Part, joined after the
    # shell's cuts (open shapes like the arch lips would spoil them).
    shell, b = b, Part("body_detail")

    # The chassis: two rails, the leaf springs, the axles with their
    # differentials, the transfer box and the exhaust.
    for sx in (-1, 1):
        b.box((0.08, 0.14, TAIL - FRONT - 0.03), (sx * 0.42, 0.48, (FRONT + TAIL) / 2), "Iron",
              segs=1)
        for z in (-AX, AX):
            b.box((0.07, 0.05, 1.1), (sx * 0.42, 0.405, z), "Iron", segs=1)
    for z in (-AX, AX):
        b.cyl(0.045, -0.6, 0.6, "Iron", segs=10, xf=T(0.0, HUB_Y, z) @ ALONG_X)
        b.sphere(0.13, (0.1, HUB_Y, z), "Iron", segs=12, rings=6, scale=(1.0, 1.0, 0.8))
    b.box((0.3, 0.2, 0.5), (0.0, 0.48, -0.3), "Iron", bevel=0.02, segs=1)
    for z in (-0.75, 0.65):
        b.cyl(0.03, z - 0.4, z + 0.35, "Black", segs=8, xf=T(0.0, 0.42, 0.0))
    b.cyl(0.03, -0.6, 1.9, "Rusty", segs=8, xf=T(0.25, 0.53, 0.0))
    b.box((0.36, 0.14, 0.5), (-0.62, 0.48, 0.45), "Iron", bevel=0.02, segs=1)

    # The engine and its bits in the bay, the radiator behind the grille.
    b.box((0.5, 0.3, 0.66), (0.0, 0.9, -1.36), "Iron", bevel=0.02, segs=1)
    b.box((0.34, 0.08, 0.5), (0.0, 1.07, -1.36), "Rusty", bevel=0.01, segs=1)
    b.cyl(0.13, 0.98, 1.06, "Black", segs=16, xf=T(0.3, 0.0, -1.0) @ RX(-math.pi / 2))
    b.box((0.8, 0.34, 0.04), (0.0, 0.92, -1.84), "Dark", segs=1)
    b.box((0.22, 0.18, 0.2), (-0.36, 0.86, -0.98), "Black", bevel=0.01, segs=1)

    # The cabin: the floor, the B pillars the doors shut on, the dash and
    # the steering wheel (left-hand drive), the seats.
    b.box((2 * W - 0.04, 0.1, ZC - ZW), (0.0, 0.6, (ZW + ZC) / 2), "Dark", segs=1)
    for sx in (-1, 1):
        b.box((0.07, BELT - 0.645, ZB2 - ZB - 0.006), (sx * (W - 0.045), (BELT + 0.645) / 2,
                                                       (ZB + ZB2) / 2), paint, bevel=0.008, segs=1)
    b.box((2 * W - 0.16, 0.14, 0.2), (0.0, 0.99, ZW + 0.1), "Black", bevel=0.02, segs=1)
    for x in (-0.3, -0.48, 0.2):
        b.cyl(0.035, ZW + 0.19, ZW + 0.21, "Lens" if x < 0 else "Dark", segs=12,
              xf=T(x, 1.0, 0.0), chamfer=0.004)
    b.tube(0.2, 0.18, -0.012, 0.012, "Black", segs=24, xf=T(-0.4, 1.17, -0.5) @ RX(-1.15))
    b.cyl(0.022, 0.0, 0.34, "Black", segs=8, xf=T(-0.4, 1.17, -0.5) @ RX(-1.15 + math.pi))
    b.cyl(0.012, 0.65, 1.0, "Black", segs=6, xf=T(0.0, 0.0, -0.55) @ RX(-math.pi / 2 + 0.3))
    for x in (-0.4, 0.4):
        b.box((0.5, 0.14, 0.48), (x, 0.72, -0.25), "Padding", bevel=0.04, segs=1)
        b.box((0.5, 0.56, 0.12), (x, 1.04, 0.0), "Padding", bevel=0.04, segs=1,
              xf=RX(0.25, about=(x, 0.79, 0.0)) if x > 0 else None)
    b.box((1.5, 0.14, 0.44), (0.0, 0.72, 0.48), "Padding", bevel=0.04, segs=1)
    b.box((1.5, 0.5, 0.1), (0.0, 1.04, 0.74), "Padding", bevel=0.04, segs=1)

    # The tub behind: its floor, the wheel boxes over the rear arches, the
    # posts the tailgate shuts between.
    b.box((2 * W - 0.1, 0.11, TAIL - ZC - 0.05), (0.0, 0.605, (ZC + TAIL - 0.05) / 2), "Dark",
          segs=1)
    lo = math.degrees(math.asin((0.66 - HUB_Y) / (ARCH + 0.015)))
    for sx in (-1, 1):
        z0, z1 = sorted((sx * 0.56, sx * (W - 0.05)))
        b.lathe([(ARCH, z0), (ARCH + 0.015, z0), (ARCH + 0.015, z1), (ARCH, z1)], paint,
                segs=14, xf=T(0.0, HUB_Y, AX) @ ALONG_X, closed=True, start=lo,
                sweep=180.0 - 2 * lo)
        plate = [(AX - (ARCH + 0.015) * math.cos(math.radians(a)),
                  HUB_Y + (ARCH + 0.015) * math.sin(math.radians(a)))
                 for a in range(int(lo), int(180 - lo) + 1, 12)]
        side_prism(b, plate, 0.55, 0.56, paint, sx)
        b.box((0.05, BELT - 0.55, 0.06), (sx * (W - 0.075), (BELT + 0.55) / 2, TAIL - 0.03),
              paint, bevel=0.008, segs=1)
        # The front arches' lips, as far as the nose goes.
        z0, z1 = sorted((sx * (W - 0.01), sx * (W + 0.012)))
        b.lathe([(ARCH, z0), (ARCH + 0.018, z0), (ARCH + 0.018, z1), (ARCH, z1)], paint,
                segs=14, xf=T(0.0, HUB_Y, -AX) @ ALONG_X, closed=True, start=24.0,
                sweep=108.0)

    # The windscreen frame, upright, a shard or two of glass left in it.
    ws = RX(0.07, about=(0.0, WING, ZW + 0.03))
    b.box((1.66, 0.05, 0.06), (0.0, WING + 0.025, ZW + 0.03), paint, bevel=0.01, segs=1, xf=ws)
    b.box((1.66, 0.05, 0.04), (0.0, 1.72, ZW + 0.03), paint, bevel=0.01, segs=1, xf=ws)
    for sx in (-1, 1):
        b.box((0.05, 1.72 - WING, 0.05), (sx * 0.8, (1.72 + WING) / 2 + 0.01, ZW + 0.03),
              paint, bevel=0.01, segs=1, xf=ws)
    for prof in ([(-0.78, 1.1), (-0.5, 1.1), (-0.66, 1.26), (-0.78, 1.38)],
                 [(0.78, 1.69), (0.36, 1.69), (0.62, 1.58), (0.78, 1.46)]):
        b.prism(prof, ZW + 0.025, ZW + 0.031, "Glass", axis="z", xf=ws)
    # Wipers, the mirror on the driver's side.
    for x in (-0.35, 0.3):
        b.box((0.4, 0.012, 0.012), (0.0, 0.0, 0.0), "Black", segs=1,
              xf=ws @ T(x, WING + 0.07, ZW - 0.005) @ RZ(0.2))
    b.box((0.12, 0.03, 0.03), (-0.87, 1.4, ZW + 0.05), "Black", bevel=0.008, segs=1)
    b.box((0.03, 0.12, 0.09), (-0.94, 1.38, ZW + 0.05), "Black", bevel=0.01, segs=1,
          xf=RZ(0.5, about=(-0.94, 1.4, 0.0)))

    # The hoops the canvas hung on, the rearmost bent down where something
    # fell on it.
    hoop = fillet([(-0.82, BELT - 0.01), (-0.82, 1.94), (0.82, 1.94), (0.82, BELT - 0.01)],
                  [0.0, 0.14, 0.14, 0.0], 3, closed=False)
    for z in ((ZB + ZB2) / 2, 0.97):
        b.sweep(hoop, section_circle(0.018), paint, xf=T(0.0, 0.0, z) @ ALONG_X)
    bent = fillet([(-0.82, BELT - 0.01), (-0.82, 1.9), (-0.55, 1.88), (0.05, 1.66), (0.6, 1.84),
                   (0.82, 1.82), (0.82, BELT - 0.01)], [0.0, 0.1, 0.06, 0.12, 0.06, 0.1, 0.0], 3,
                  closed=False)
    b.sweep(bent, section_circle(0.018), paint,
            xf=RX(0.12, about=(0.0, BELT, 1.66)) @ T(0.0, 0.0, 1.66) @ ALONG_X)

    # Front: the slatted grille between round headlamps (one gone), the
    # sidelights, the bumper hanging off one bracket. Back: the tail lamps.
    for k in range(11):
        b.box((0.026, 0.38, 0.05), (-0.4 + k * 0.08, 0.84, FRONT + 0.03), paint, segs=1)
    for sx in (-1, 1):
        b.cyl(0.11, FRONT - 0.03, FRONT + 0.01, "Steel", segs=18, xf=T(sx * 0.62, 0.86, 0.0),
              chamfer=0.008)
        b.cyl(0.09, FRONT - 0.04, FRONT - 0.02, "Lens" if sx > 0 else "Dark", segs=18,
              xf=T(sx * 0.62, 0.86, 0.0), chamfer=0.006)
        b.box((0.1, 0.06, 0.03), (sx * 0.62, 0.66, FRONT - 0.005), "Cream", bevel=0.01, segs=1)
        b.box((0.07, 0.12, 0.02), (sx * (W - 0.075), 0.76, TAIL + 0.01), "LampRed", bevel=0.006,
              segs=1)
        b.box((0.12, 0.08, 0.1), (sx * 0.7, 0.55, TAIL + 0.04), paint, bevel=0.01, segs=1)
    b.box((1.86, 0.12, 0.08), (0.0, 0.5, FRONT - 0.05), paint, bevel=0.015, segs=1,
          xf=RZ(-0.07, about=(0.42, 0.5, 0.0)))
    b.box((0.52, 0.11, 0.012), (0.0, 0.0, 0.0), "Cream", segs=1,
          xf=RZ(-0.07, about=(0.42, 0.5, 0.0)) @ T(0.05, 0.5, FRONT - 0.096))

    # The lids: the flat bonnet over the engine bay between the wings, and
    # its latches; the tailgate with the spare wheel on it.
    bonnet = Part("bonnet", origin=(0.0, WING + 0.005, ZW - 0.02))
    bonnet.prism(fillet([(-0.66, 0.0), (0.66, 0.0), (0.66, 0.035), (-0.66, 0.035)], 0.012, 2),
                 FRONT + 0.01, ZW - 0.02, paint, axis="z", bevel=0.01, segs=1,
                 xf=T(0.0, WING + 0.005, 0.0))
    for sx in (-1, 1):
        bonnet.box((0.03, 0.06, 0.05), (sx * 0.66, WING + 0.02, -1.6), "Steel", bevel=0.006,
                   segs=1)
    bcut = Part("cut")
    rust(bcut, 0.2, WING + 0.04, -1.3, 0.5, 0.4, n=6, axis="y")
    rust(bcut, -0.4, WING + 0.04, -1.8, 0.3, 0.16, n=4, axis="y")
    bonnet.cut(bcut, paint=True)

    tg = Part("tailgate", origin=(0.0, 0.655, TAIL - 0.02))
    tg.prism(fillet([(-0.77, 0.66), (0.77, 0.66), (0.77, BELT - 0.01), (-0.77, BELT - 0.01)],
                    [0.01, 0.01, 0.03, 0.03], 2),
             TAIL - 0.04, TAIL, paint, axis="z", bevel=0.008, segs=1)
    tg.box((1.2, 0.03, 0.03), (0.0, BELT - 0.08, TAIL + 0.01), paint, bevel=0.008, segs=1)
    tcut = Part("cut")
    rust(tcut, 0.6, 0.75, TAIL, 0.2, 0.12, n=4)
    tg.cut(tcut, paint=True)
    # The spare, square to the tailgate on its carrier: the same wheel as
    # the road ones, pumped up, its axis along Z.
    sw, sr, srim, sy, sz = 0.24, 0.39, 0.25, 0.94, TAIL + 0.13
    tg.lathe([(srim - 0.01, -sw / 2), (sr - 0.03, -sw / 2), (sr, -sw / 2 + 0.04),
              (sr, sw / 2 - 0.04), (sr - 0.03, sw / 2), (srim - 0.01, sw / 2)], "Rubber",
             segs=24, xf=T(0.0, sy, sz), closed=True)
    tg.lathe([(0.0, sw / 2 - 0.035), (srim, sw / 2 - 0.035), (srim, -sw / 2 + 0.02),
              (0.0, -sw / 2 + 0.02)], "Rusty", segs=20, xf=T(0.0, sy, sz), closed=True)
    tg.cyl(0.04, TAIL - 0.005, sz + sw / 2 - 0.01, "Steel", segs=10, xf=T(0.0, sy, 0.0),
           chamfer=0.006)
    tg.box((0.5, 0.04, 0.02), (0.0, sy, TAIL + 0.01), "Steel", bevel=0.006, segs=1)

    parts = [("body", shell, b), bonnet, tg]
    for sx, s in ((-1, "l"), (1, "r")):
        fd = door(f"door_f{s}", sx, ZA, ZB, 0.655, BELT,
                  [(ZA + 0.03, BELT), (ZA + 0.03, 1.6), (ZB - 0.03, 1.6), (ZB - 0.03, BELT)],
                  paint, xs=(0.79, W))
        rd = door(f"door_r{s}", sx, ZB2, ZC, 0.655, BELT,
                  [(ZB2 + 0.03, BELT), (ZB2 + 0.03, 1.6), (ZC - 0.03, 1.6), (ZC - 0.03, BELT)],
                  paint, xs=(0.79, W))
        dcut = Part("cut")
        rust(dcut, sx * W, 0.72, -0.3 if sx < 0 else 0.5, 0.36, 0.1)
        (fd if sx < 0 else rd).cut(dcut, paint=True)
        parts += [fd, rd]
        for z, e in ((-AX, "f"), (AX, "r")):
            parts.append(wheel(f"wheel_{e}{s}", (sx * 0.72, HUB_Y, z), sx, r=R, w=0.24, rim=0.25))
    write("wreck_uaz", parts, {"pt_seat_driver": (-0.4, 0.79, -0.25)})


if __name__ == "__main__":
    uaz()
