"""Car wrecks by the Zone's roads and in its yards, for Destruction to strip
and World models to place. Writes assets/models/props/wreck_<name>.glb.

Prop space: metres, +Y up from the ground, -Z the front, the origin on the
ground under the middle of the wheelbase. Each sits low on flat tyres.

- `wreck_lada`  a boxy saloon, faded blue, 4.1 x 1.6 x 1.4

Objects (each one node; its origin is its pivot):

- `body`                      the shell, chassis, engine and interior
                              (static)
- `door_fl`, `door_fr`,       origin on the front hinge line at the door's
  `door_rl`, `door_rr`        mid height; they swing about Y. A front door
                              is the `car_door` item's size
- `bonnet`                    origin on its rear hinge; lifts about X
- `boot`                      origin on its front hinge; lifts about X
- `wheel_fl`, `wheel_fr`,     origin at the hub
  `wheel_rl`, `wheel_rr`
- `pt_seat_driver`            the driver's seat cushion (left-hand drive)

Materials: Blue, Rusty, Steel, Black, Dark, Iron, Rubber, Glass, Lens,
LampRed, Padding, Leather.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import RX, RY, RZ, T, Part, empty, export, fillet, join, make, report, reset  # noqa: E402

ALONG_X = RY(math.pi / 2)   # lathe/cyl axis (Z) -> X


def wanted(name):
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or name in argv


def write(name, parts, points):
    for p in parts:
        if isinstance(p, tuple):
            join(*p)
        else:
            make(p)
    for k, v in points.items():
        empty(k, v)
    print(f"{name}:")
    report()
    export(f"assets/models/props/{name}.glb")


def side_prism(p, profile, x0, x1, mat, sx, **kw):
    """A side-view (z, y) profile extruded between |x| x0 and x1 on side sx."""
    a, b = sorted((sx * x0, sx * x1))
    p.prism(profile, a, b, mat, axis="x", **kw)


def rust(cut, x, y, z, w, h, n=5, axis="x"):
    """A blotch of rust eaten 5 mm into a panel (a cutter, painted):
    overlapping turned scraps round (y, z) on the face at x, or round
    (x, z) on a top face at height y with axis "y"."""
    rnd = random.Random(int(abs(x * 1000 + y * 100 + z * 10)))
    for _ in range(n):
        a = rnd.uniform(-0.6, 0.6)
        sw, sh = w * rnd.uniform(0.35, 0.7), h * rnd.uniform(0.35, 0.8)
        du, dv = rnd.uniform(-w, w) * 0.3, rnd.uniform(-h, h) * 0.3
        if axis == "x":
            cut.box((0.01, sh, sw), (0.0, 0.0, 0.0), "Rusty",
                    xf=T(x, y + dv, z + du) @ RX(a))
        else:
            cut.box((sh, 0.01, sw), (0.0, 0.0, 0.0), "Rusty",
                    xf=T(x + dv, y, z + du) @ RY(a))


def wheel(name, hub, sx, r=0.29, w=0.17, rim=0.19, mat="Rusty"):
    """A road wheel on a flat tyre: the tyre round a pressed steel rim, a
    hub cap; facing out on side sx."""
    q = Part(name, origin=hub)
    x, y, z = hub
    q.lathe([(rim - 0.01, -w / 2), (r - 0.02, -w / 2), (r, -w / 2 + 0.03), (r, w / 2 - 0.03),
             (r - 0.02, w / 2), (rim - 0.01, w / 2)], "Rubber", segs=24,
            xf=T(x, y, z) @ ALONG_X, closed=True)
    q.lathe([(0.0, sx * (w / 2 - 0.035)), (rim, sx * (w / 2 - 0.035)),
             (rim, -sx * (w / 2 - 0.02)), (0.0, -sx * (w / 2 - 0.02))], mat, segs=20,
            xf=T(x, y, z) @ ALONG_X, closed=True)
    q.cyl(0.07, sx * (w / 2 - 0.035), sx * (w / 2 - 0.015), "Steel", segs=14,
          xf=T(x, y, z) @ ALONG_X, chamfer=0.006)
    return q


def door(name, sx, z0, z1, y0, y1, frame, paint, xs=(0.72, 0.8)):
    """A door on side sx between z0 (front, hinged) and z1: the skin over a
    dark inner frame, the trim panel inside, the handle, and a window frame
    `frame` (a side-view polyline above the belt line y1), empty but for a
    shard of glass."""
    xi, xo = xs
    q = Part(name, origin=(sx * xo, (y0 + y1) / 2, z0))
    skin = fillet([(z0, y0), (z1, y0), (z1, y1), (z0, y1)], [0.03, 0.03, 0.01, 0.01], 2)
    side_prism(q, skin, xo - 0.025, xo, paint, sx, bevel=0.006, segs=1)
    side_prism(q, skin, xi + 0.01, xo - 0.02, "Dark", sx)
    trim = fillet([(z0 + 0.04, y0 + 0.04), (z1 - 0.04, y0 + 0.04), (z1 - 0.04, y1 - 0.03),
                   (z0 + 0.04, y1 - 0.03)], 0.02, 2)
    side_prism(q, trim, xi, xi + 0.02, "Padding", sx, bevel=0.005, segs=1)
    # The pull handle near the back of the skin, a crease along it.
    q.box((0.012, 0.025, 0.12), (sx * (xo + 0.006), y1 - 0.07, z1 - 0.13), "Steel", bevel=0.004,
          segs=1)
    q.box((0.006, 0.01, z1 - z0 - 0.1), (sx * (xo + 0.002), y0 + 0.3, (z0 + z1) / 2), paint)
    # The window frame, a channel round the empty opening.
    fw = 0.025
    for k, ((za, ya), (zb, yb)) in enumerate(zip(frame, frame[1:])):
        L = math.hypot(zb - za, yb - ya)
        a = math.atan2(yb - ya, zb - za)
        # (Alternate pieces a little thinner, so their faces don't meet in
        # one plane where they overlap at the corners.)
        t = 0.03 - 0.008 * (k % 2)
        q.box((t, fw, L + fw), (0.0, 0.0, 0.0), paint, bevel=0.004, segs=1,
              xf=T(sx * (xo - 0.015), (ya + yb) / 2, (za + zb) / 2)
              @ RX(-a))
    zm = (z0 + z1) / 2
    q.prism([(zm - 0.08, y1 + 0.01), (zm + 0.05, y1 + 0.01), (zm - 0.02, y1 + 0.11)],
            *sorted((sx * (xo - 0.02), sx * (xo - 0.016))), "Glass", axis="x")
    return q


# --- The saloon ------------------------------------------------------------------------------

def lada():
    reset()
    W = 0.8                  # half width
    AX = 1.21                # axles at z -+AX
    HUB_Y, R = 0.27, 0.29    # the hubs sit low on flat tyres
    BELT, ROOF = 0.82, 1.36
    ZA, ZB, ZC = -0.965, 0.065, 0.955   # front door, rear door, the doors' ends
    paint = "Blue"
    b = Part("body")

    # The nose and tail: solid over the floor, their tops under the bonnet
    # and boot lid, the wheel arches cut out of their sides.
    nose = fillet([(-2.05, 0.3), (ZA - 0.005, 0.3), (ZA - 0.005, BELT), (-1.98, BELT),
                   (-2.05, 0.74)], [0.04, 0.0, 0.0, 0.04, 0.04], 2)
    tail = fillet([(ZC + 0.005, 0.3), (2.05, 0.3), (2.05, 0.78), (1.99, BELT + 0.01),
                   (ZC + 0.005, BELT + 0.01)], [0.0, 0.04, 0.04, 0.04, 0.0], 2)
    for prof in (nose, tail):
        b.prism(prof, -W, W, paint, axis="x", bevel=0.02, segs=2)
    cut = Part("cut")
    for z in (-AX, AX):
        for sx in (-1, 1):
            cut.cyl(0.37, *sorted((sx * 0.5, sx * 0.9)), "Dark", segs=24,
                    xf=T(0.0, HUB_Y, z) @ ALONG_X)
    # The engine bay and the boot well under their lids.
    cut.box((1.44, 0.5, 0.95), (0.0, BELT, -1.48), "Dark")
    cut.box((1.44, 0.45, 0.9), (0.0, BELT, 1.5), "Dark")
    # Rust eaten into the sills and arches' lips, painted into the cut.
    for z, y, w, h in ((-1.62, 0.36, 0.3, 0.1), (1.6, 0.4, 0.42, 0.14), (1.85, 0.6, 0.18, 0.12),
                       (-2.0, 0.4, 0.12, 0.1)):
        for sx in (-1, 1):
            rust(cut, sx * W, y, z * (1 if sx > 0 else 0.97), w, h)
    b.cut(cut, paint=True)
    # Everything else on the body goes in a second Part, joined after the
    # shell's cuts (open shapes like the arch lips would spoil them).
    shell, b = b, Part("body_detail")
    # The engine and its bits in the bay; the spare wheel in the boot.
    b.box((0.5, 0.36, 0.55), (0.0, 0.52, -1.42), "Iron", bevel=0.02, segs=1)
    b.box((0.36, 0.08, 0.3), (0.12, 0.74, -1.5), "Rusty", bevel=0.01, segs=1)
    b.cyl(0.13, 0.7, 0.78, "Black", segs=16, xf=T(-0.3, 0.0, -1.3) @ RX(-math.pi / 2))
    b.box((1.2, 0.3, 0.06), (0.0, 0.5, -1.92), "Black", segs=1)
    b.cyl(0.25, -0.08, 0.0, "Rubber", segs=20, xf=T(0.0, 0.48, 1.55) @ RX(-math.pi / 2))

    # The middle: the floor pan and sills under the doors, the bulkheads
    # the doors shut against.
    b.box((2 * W - 0.04, 0.1, ZC - ZA), (0.0, 0.27, (ZA + ZC) / 2), "Dark", segs=1)
    for sx in (-1, 1):
        b.box((0.06, 0.1, ZC - ZA + 0.01), (sx * (W - 0.03), 0.255, (ZA + ZC) / 2), paint,
              bevel=0.01, segs=1)
    # The cabin: pillars round the doors' window frames, the roof, the
    # windscreen and rear window mostly gone.
    def strut(a, c, sx, t=0.08, mat=paint):
        (za, ya), (zb, yb) = a, c
        L = math.hypot(zb - za, yb - ya)
        ang = math.atan2(yb - ya, zb - za)
        b.box((0.07, t, L + t), (0.0, 0.0, 0.0), mat, bevel=0.015, segs=1,
              xf=T(sx * (W - 0.075), (ya + yb) / 2, (za + zb) / 2) @ RX(-ang))
    for sx in (-1, 1):
        strut((ZA - 0.02, BELT), (-0.5, ROOF - 0.02), sx)
        strut((ZB, BELT), (ZB, ROOF - 0.02), sx, t=0.1)
        strut((0.8, ROOF - 0.02), (1.27, BELT + 0.02), sx)
    roof = fillet([(-0.56, ROOF - 0.05), (0.86, ROOF - 0.05), (0.84, ROOF + 0.01), (-0.54, ROOF + 0.01)],
                  0.02, 2)
    b.prism(roof, -W + 0.035, W - 0.035, paint, axis="x", bevel=0.02, segs=2)
    # (A dent in the roof, the glass that's left.)
    b.box((0.7, 0.04, 0.5), (0.2, ROOF - 0.03, 0.3), "Rusty", bevel=0.02, segs=1)
    for (za, ya), (zb, yb) in ((((ZA - 0.02, BELT + 0.01), (-0.5, ROOF - 0.03))),
                               (((0.8, ROOF - 0.03), (1.27, BELT + 0.03)))):
        glass = [(za, ya), (za + (zb - za) * 0.35, ya + (yb - ya) * 0.35),
                 (za + (zb - za) * 0.2, ya + (yb - ya) * 0.3), (za + (zb - za) * 0.1,
                                                                ya + (yb - ya) * 0.15)]
        for x0, x1 in ((-0.62, -0.1), (0.25, 0.6)):
            b.prism([(z + 0.01, y) for z, y in glass], x0, x1, "Glass", axis="x")
    # The dash, the steering wheel (left-hand drive), the seats.
    b.box((2 * W - 0.2, 0.18, 0.25), (0.0, 0.78, ZA + 0.2), "Black", bevel=0.03, segs=1)
    b.tube(0.19, 0.17, -0.012, 0.012, "Black", segs=24,
           xf=T(-0.38, 0.88, ZA + 0.42) @ RX(-1.1))
    b.cyl(0.02, 0.0, 0.3, "Black", segs=8, xf=T(-0.38, 0.88, ZA + 0.42) @ RX(-1.1 + math.pi))
    for x in (-0.36, 0.36):
        b.box((0.48, 0.14, 0.5), (x, 0.42, -0.12), "Padding", bevel=0.04, segs=1)
        b.box((0.48, 0.6, 0.13), (x, 0.74, 0.16), "Padding", bevel=0.04, segs=1,
              xf=RX(-0.2, about=(x, 0.48, 0.16)) if x > 0 else None)
    b.box((1.36, 0.14, 0.5), (0.0, 0.42, 0.62), "Leather", bevel=0.04, segs=1)
    b.box((1.36, 0.5, 0.13), (0.0, 0.72, 0.9), "Leather", bevel=0.04, segs=1)

    # Front: the grille between twin round headlamps, the bumper hanging
    # off one bracket. Back: tail lamps and its bumper.
    b.box((0.8, 0.16, 0.03), (0.0, 0.62, -2.05), "Black", segs=1)
    for sx in (-1, 1):
        for x in (0.54, 0.7):
            b.cyl(0.065, -2.075, -2.04, "Lens" if x < 0.6 or sx < 0 else "Dark", segs=16,
                  xf=T(sx * x, 0.64, 0.0), chamfer=0.006)
        b.box((0.24, 0.1, 0.03), (sx * 0.62, 0.62, 2.06), "LampRed", bevel=0.01, segs=1)
    b.box((1.62, 0.07, 0.06), (0.0, 0.0, 0.0), "Steel", bevel=0.02, segs=1,
          xf=T(0.05, 0.36, -2.12) @ RZ(-0.12))
    b.box((1.6, 0.07, 0.06), (0.0, 0.4, 2.11), "Steel", bevel=0.02, segs=1)

    # Grille slats, the number plates, a mirror left on the driver's door
    # pillar, wipers, the chrome belt strip, the exhaust, arch lips.
    for k in range(5):
        b.box((0.78, 0.012, 0.02), (0.0, 0.565 + k * 0.027, -2.065), "Steel", segs=1)
    b.box((0.52, 0.11, 0.012), (0.0, 0.42, -2.157), "Cream", segs=1)
    b.box((0.52, 0.11, 0.012), (0.0, 0.6, 2.06), "Cream", segs=1)
    b.box((0.04, 0.03, 0.08), (-W - 0.02, BELT + 0.04, ZA + 0.06), "Black", bevel=0.008, segs=1)
    b.box((0.12, 0.08, 0.025), (-W - 0.08, BELT + 0.08, ZA + 0.06), "Black", bevel=0.01, segs=1)
    for x in (-0.3, 0.2):
        b.box((0.42, 0.012, 0.012), (0.0, 0.0, 0.0), "Black", segs=1,
              xf=T(x, BELT + 0.04, ZA - 0.04) @ RZ(0.15))
    for sx in (-1, 1):
        b.box((0.008, 0.012, 2.05 - 0.12), (sx * (W + 0.004), BELT - 0.035, -0.05), "Steel",
              segs=1)
        for z in (-AX, AX):
            z0, z1 = sorted((sx * 0.75, sx * 0.808))
            b.lathe([(0.36, z0), (0.378, z0), (0.378, z1), (0.36, z1)], paint, segs=14,
                    xf=T(0.0, HUB_Y, z) @ ALONG_X, closed=True, start=0.0, sweep=180.0)
    b.cyl(0.028, 1.6, 2.15, "Rusty", segs=10, xf=T(0.45, 0.25, 0.0))

    # The lids: the bonnet over the engine bay, the boot lid.
    bonnet = Part("bonnet", origin=(0.0, BELT + 0.01, ZA - 0.02))
    bonnet.prism(fillet([(-W + 0.04, 0.0), (W - 0.04, 0.0), (W - 0.04, 0.035),
                         (-W + 0.04, 0.035)], 0.012, 2),
                 -2.0, ZA - 0.02, paint, axis="z", bevel=0.01, segs=1,
                 xf=T(0.0, BELT + 0.005, 0.0))
    bcut = Part("cut")
    rust(bcut, 0.25, BELT + 0.04, -1.45, 0.5, 0.4, n=6, axis="y")
    rust(bcut, -0.5, BELT + 0.04, -1.9, 0.3, 0.2, n=4, axis="y")
    bonnet.cut(bcut, paint=True)
    boot = Part("boot", origin=(0.0, BELT + 0.02, 1.27))
    boot.prism(fillet([(-W + 0.04, 0.0), (W - 0.04, 0.0), (W - 0.04, 0.035),
                       (-W + 0.04, 0.035)], 0.012, 2),
               1.27, 1.99, paint, axis="z", bevel=0.01, segs=1, xf=T(0.0, BELT + 0.015, 0.0))

    parts = [("body", shell, b), bonnet, boot]
    for sx, s in ((-1, "l"), (1, "r")):
        parts.append(door(f"door_f{s}", sx, ZA, ZB - 0.035, 0.335, BELT,
                          [(ZA + 0.02, BELT), (-0.5, ROOF - 0.05), (ZB - 0.05, ROOF - 0.05),
                           (ZB - 0.05, BELT)], paint))
        parts.append(door(f"door_r{s}", sx, ZB + 0.035, ZC, 0.335, BELT,
                          [(ZB + 0.05, BELT), (ZB + 0.05, ROOF - 0.05), (0.78, ROOF - 0.05),
                           (ZC - 0.02, BELT + 0.15)], paint))
        for z, e in ((-AX, "f"), (AX, "r")):
            parts.append(wheel(f"wheel_{e}{s}", (sx * 0.68, HUB_Y, z), sx))
    write("wreck_lada", parts, {"pt_seat_driver": (-0.36, 0.49, -0.12)})


if __name__ == "__main__":
    for name, build in (("wreck_lada", lada),):
        if wanted(name):
            build()
