"""Music in the houses: a wind-up horn gramophone and an upright piano.
Writes assets/models/props/<name>.glb for each; prop space as in _kit.py
(metres, +Y up, -Z the front, origin on the floor under the piece's
centre). Their moving parts are separate nodes:

- gramophone: a 0.32 x 0.16 x 0.32 m case for a table top. `platter`
  (origin on the spindle at the motor board, spins about Y), `tonearm`
  (origin on its swivel, swings about Y; modelled parked on its rest at
  the front right, a positive turn brings the soundbox in over the
  record), `horn` (fixed on the swivel's elbow, its bell to the front),
  `crank` (origin on its axle at the case's +X side, turns about local X).
- upright_piano: 1.45 x 1.2 x 0.58 m, the keys' tops at 0.72. `case`;
  `lid` (the fallboard, modelled open, standing on the name rail; origin
  on its hinge, a turn of -95 degrees about local X closes it over the
  keys); `key_21` .. `key_108` (MIDI numbers), each with its origin at the
  back of its top face, the pivot: a negative turn about local X presses
  its front down; `pedal_l`, `pedal_r` (origin on their pivot inside the
  plinth, a negative turn about X presses them); `music_desk`.
"""

import importlib.util
import math
import os
import sys

from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, ".."))
from _kit import ALONG_X, UP, prop, rod  # noqa: E402
from common import Part, RX, RY, S, T, fillet  # noqa: E402

import _kit  # noqa: E402

# The items' kit (stencil lettering, rings), loaded under its own name.
_spec = importlib.util.spec_from_file_location("items_kit",
                                               os.path.join(HERE, "..", "items", "_kit.py"))
items_kit = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(items_kit)

# Always under a building's roof.
_kit.AO_SKY = "indoors"


def rect(x0, x1, y0, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def spline3(points, samples=6):
    """A Catmull-Rom curve through 3D points, ends included."""
    pts = [Vector(p) for p in points]
    n = len(pts)
    out = []
    for i in range(n - 1):
        p0, p1, p2 = pts[max(i - 1, 0)], pts[i], pts[i + 1]
        p3 = pts[min(i + 2, n - 1)]
        for s in range(samples):
            t = s / samples
            t2, t3 = t * t, t * t * t
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2
                              + (-p0 + 3 * p1 - 3 * p2 + p3) * t3))
    out.append(pts[-1])
    return out


def frames_along(path):
    """Parallel-transported (u, v) frames along a 3D path, and tangents."""
    n = len(path)
    tans = [(path[min(i + 1, n - 1)] - path[max(i - 1, 0)]).normalized() for i in range(n)]
    u = tans[0].cross(Vector((1, 0, 0)))
    if u.length < 1e-6:
        u = tans[0].cross(Vector((0, 0, 1)))
    u.normalize()
    out = []
    for i in range(n):
        u = (u - tans[i] * u.dot(tans[i])).normalized()
        out.append((u.copy(), tans[i].cross(u).normalized()))
    return out, tans


def horn_shell(p, path, radius, wall, mat, sides=48, petals=8):
    """A thin flared horn along a 3D path: `radius(t)` at t 0..1, its wall
    `wall` thick, the petals' seams standing a little proud."""
    bm = p.bm
    n = len(path)
    frames, tans = frames_along(path)
    outer, inner = [], []
    for i in range(n):
        t = i / (n - 1)
        r = radius(t)
        uu, vv = frames[i]
        ro, ri = [], []
        for k in range(sides):
            a = 2 * math.pi * k / sides
            seam = 1.0 + 0.03 * max(0.0, math.cos(petals * a)) ** 16 * min(1.0, t * 3)
            d = uu * math.cos(a) + vv * math.sin(a)
            ro.append(bm.verts.new(path[i] + d * r * seam))
            ri.append(bm.verts.new(path[i] + d * (r - wall)))
        outer.append(ro)
        inner.append(ri)
    faces = []
    for i in range(n - 1):
        for k in range(sides):
            k1 = (k + 1) % sides
            faces.append(bm.faces.new((outer[i][k], outer[i][k1], outer[i + 1][k1],
                                       outer[i + 1][k])))
            faces.append(bm.faces.new((inner[i][k], inner[i + 1][k], inner[i + 1][k1],
                                       inner[i][k1])))
    for ro, ri in ((outer[0], inner[0]), (outer[-1], inner[-1])):
        for k in range(sides):
            k1 = (k + 1) % sides
            faces.append(bm.faces.new((ro[k], ri[k], ri[k1], ro[k1])))
    p._finish_piece([v for r_ in outer + inner for v in r_], faces, mat)
    return frames, tans


def tube_along(p, pts, r0, r1, mat, sides=14):
    """A round tube along a 3D polyline, tapering from r0 to r1, capped."""
    frames, _ = frames_along(pts)
    rings = []
    for i, c in enumerate(pts):
        r = r0 + (r1 - r0) * i / (len(pts) - 1)
        uu, vv = frames[i]
        rings.append([c + (uu * math.cos(2 * math.pi * k / sides) +
                           vv * math.sin(2 * math.pi * k / sides)) * r for k in range(sides)])
    p.loft(rings, mat)


# --- Gramophone ----------------------------------------------------------------------------

def gramophone():
    W, D = 0.32, 0.32
    top = 0.16
    case = Part("case")
    # Bun feet, a plinth, the box, a moulded lip round the motor board.
    for sx in (-1, 1):
        for sz in (-1, 1):
            case.lathe([(0.0, 0.0), (0.012, 0.0), (0.016, 0.006), (0.014, 0.012),
                        (0.0, 0.012)], "Wood", segs=12, xf=T(sx * 0.13, 0, sz * 0.13) @ UP)
    case.box((W + 0.008, 0.014, D + 0.008), (0.0, 0.019, 0.0), "Wood", bevel=0.003, segs=1)
    case.box((W, 0.12, D), (0.0, 0.086, 0.0), "Wood", bevel=0.002, segs=1)
    case.box((W + 0.01, 0.014, D + 0.01), (0.0, top - 0.007, 0.0), "Wood", bevel=0.003, segs=1)
    # A raised moulding frame on each side (5 mm proud).
    for k in range(4):
        for cx, cy, sx, sy in ((0.0, 0.135, 0.27, 0.01), (0.0, 0.037, 0.27, 0.01),
                               (-0.13, 0.086, 0.01, 0.088), (0.13, 0.086, 0.01, 0.088)):
            case.box((sx, sy, 0.005), (cx, cy, -D / 2 - 0.0025), "Wood", bevel=0.0015, segs=1,
                     xf=RY(k * math.pi / 2))
    # The maker's plate on the front.
    case.box((0.07, 0.022, 0.004), (0.0, 0.086, -D / 2 - 0.007), "Brass", bevel=0.001, segs=1)
    # Motor board fittings: the swivel's pillar at the back right, the
    # arm's rest at the front right, the speed lever, the needle cups.
    pv = (0.11, 0.245, 0.13)
    case.lathe([(0.0, top), (0.024, top), (0.024, top + 0.004), (0.014, top + 0.01),
                (0.011, top + 0.02), (0.011, pv[1] - 0.012), (0.0, pv[1] - 0.012)], "Cast",
               segs=20, xf=T(pv[0], 0, pv[2]) @ UP)
    rest = (pv[0], -0.03)
    case.cyl(0.008, top, top + 0.003, "Brass", segs=12, xf=T(rest[0], 0, rest[1]) @ UP)
    case.cyl(0.0035, top, 0.226, "Brass", segs=10, xf=T(rest[0], 0, rest[1]) @ UP)
    case.box((0.018, 0.005, 0.01), (rest[0], 0.2285, rest[1]), "Rubber", bevel=0.0015, segs=1)
    case.box((0.006, 0.004, 0.04), (-0.13, top + 0.002, -0.12), "Brass", bevel=0.001, segs=1)
    case.cyl(0.005, top + 0.004, top + 0.012, "Black", segs=10, xf=T(-0.13, 0, -0.14) @ UP)
    for z in (0.11, 0.135):
        case.tube(0.0075, 0.006, top, top + 0.012, "Brass", segs=12, xf=T(-0.14, 0, z) @ UP)
    # The crank's boss on the +X side.
    ca = (W / 2, 0.075, 0.05)
    case.tube(0.016, 0.006, 0.0, 0.006, "Brass", segs=16, xf=T(*ca) @ ALONG_X)
    # An iron stay from the case's back up under the horn.
    rod(case, (0.105, top - 0.01, D / 2 + 0.003), (0.105, 0.33, D / 2 + 0.003), 0.004, "Iron",
        segs=8)
    case.box((0.03, 0.03, 0.004), (0.105, top - 0.02, D / 2 + 0.002), "Iron", bevel=0.001,
             segs=1)
    parts = [case]

    # Platter: an iron disc under red felt, the spindle on top.
    pc = (-0.025, top, 0.0)
    plat = Part("platter", origin=pc)
    xf = T(pc[0], 0, pc[2]) @ UP
    plat.cyl(0.022, top, top + 0.004, "Cast", segs=20, xf=xf)
    plat.lathe([(0.0, top + 0.004), (0.123, top + 0.004), (0.125, top + 0.006),
                (0.125, top + 0.013), (0.0, top + 0.013)], "Cast", segs=48, xf=xf)
    plat.cyl(0.121, top + 0.013, top + 0.017, "Red", segs=48, xf=xf)
    plat.cyl(0.0036, top + 0.017, top + 0.03, "Steel", segs=10, xf=xf, chamfer=0.001)
    parts.append(plat)

    # Tonearm: a collar round the pillar's top, a tapered arm forward and
    # down to the soundbox, parked on its rest.
    arm = Part("tonearm", origin=pv)
    arm.tube(0.016, 0.0115, pv[1] - 0.012, pv[1] + 0.01, "Brass", segs=20,
             xf=T(pv[0], 0, pv[2]) @ UP)
    sb = Vector((pv[0], 0.226, -0.1))        # the soundbox's centre
    pts = spline3([(pv[0], pv[1], pv[2] - 0.01), (pv[0], 0.243, 0.04), (pv[0], 0.2375, -0.03),
                   (pv[0], 0.232, -0.065), (pv[0], sb.y + 0.002, sb.z + 0.024)], 5)
    tube_along(arm, pts, 0.011, 0.0065, "Brass")
    # Soundbox: a nickel drum on edge, its mica faces to the sides, the
    # needle clamp under it, the needle 1 mm over a record on the felt.
    arm.cyl(0.027, -0.006, 0.006, "Steel", segs=24, xf=T(*sb) @ ALONG_X, chamfer=0.002)
    arm.cyl(0.021, 0.006, 0.0075, "Pale", segs=24, xf=T(*sb) @ ALONG_X)
    arm.cyl(0.021, -0.0075, -0.006, "Pale", segs=24, xf=T(*sb) @ ALONG_X)
    arm.box((0.006, 0.022, 0.006), (sb.x, sb.y - 0.032, sb.z), "Steel", bevel=0.001, segs=1)
    arm.cyl(0.0007, 0.18, sb.y - 0.04, "Steel", segs=6, xf=T(sb.x, 0, sb.z) @ UP)
    parts.append(arm)

    # Horn: an elbow up out of the swivel, then a brass petal horn rising
    # over the back and flaring forward, a rolled bead round the bell.
    y_elbow = pv[1] + 0.01
    horn = Part("horn", origin=(pv[0], y_elbow, pv[2]))
    horn.cyl(0.0135, y_elbow, y_elbow + 0.02, "Brass", segs=20, xf=T(pv[0], 0, pv[2]) @ UP)
    hp = spline3([(pv[0], y_elbow + 0.02, pv[2]), (0.105, 0.33, 0.165), (0.085, 0.41, 0.15),
                  (0.055, 0.48, 0.095), (0.025, 0.53, 0.0), (0.0, 0.555, -0.1)], 5)
    R0, R1 = 0.0125, 0.15

    def rad(t):
        return R0 * math.exp(math.log(R1 / R0) * t ** 2.2)
    frames, tans = horn_shell(horn, hp, rad, 0.0015, "Brass", sides=40)
    uu, vv = frames[-1]
    m = Matrix((uu, vv, tans[-1])).transposed().to_4x4()
    m.translation = hp[-1] - tans[-1] * 0.001
    items_kit.ring(horn, R1 + 0.0015, 0.004, "Brass", xf=m, segs=40, sides=6)
    parts.append(horn)

    # The crank: an axle out of the boss, an arm, a turned wooden knob.
    cr = Part("crank", origin=ca)
    cr.cyl(0.0045, 0.0, 0.037, "Steel", segs=12, xf=T(*ca) @ ALONG_X)
    cr.box((0.006, 0.075, 0.012), (ca[0] + 0.037, ca[1] - 0.03, ca[2]), "Steel", bevel=0.002,
           segs=1)
    cr.cyl(0.0035, 0.0, 0.014, "Steel", segs=10,
           xf=T(ca[0] + 0.037, ca[1] - 0.06, ca[2]) @ ALONG_X)
    cr.lathe([(0.0, 0.0), (0.008, 0.0), (0.0095, 0.012), (0.0085, 0.03), (0.006, 0.036),
              (0.0, 0.037)], "Wood", segs=14, xf=T(ca[0] + 0.05, ca[1] - 0.06, ca[2]) @ ALONG_X)
    parts.append(cr)
    return parts


# --- Upright piano -------------------------------------------------------------------------

PIANO_W, PIANO_H, PIANO_D = 1.45, 1.2, 0.58
WHITE = (0, 2, 4, 5, 7, 9, 11)
PITCH = 0.023
KEY_TOP = 0.72
KEY_BOT = 0.698
Z_FRONT, Z_BACK = -0.215, -0.065
HEAD = 0.05
BLACK_W, BLACK_LEN, BLACK_UP = 0.0115, 0.095, 0.012
BLACK_SHIFT = {1: -0.0025, 3: 0.0025, 6: -0.003, 8: 0.0, 10: 0.003}
LID_OPEN = math.radians(95)
HINGE = (0.0, 0.76, -0.075)


def key_layout():
    """White keys' and black keys' centres (x) by MIDI number."""
    whites = [n for n in range(21, 109) if n % 12 in WHITE]
    x0 = -len(whites) * PITCH / 2
    wx = {n: x0 + (i + 0.5) * PITCH for i, n in enumerate(whites)}
    bx = {n: (wx[n - 1] + wx[n + 1]) / 2 + BLACK_SHIFT[n % 12]
          for n in range(21, 109) if n % 12 not in WHITE}
    return wx, bx


def outline(pts):
    """Drop repeated and collinear points."""
    out = []
    for p in pts:
        if not out or abs(p[0] - out[-1][0]) > 1e-7 or abs(p[1] - out[-1][1]) > 1e-7:
            out.append(p)
    m = len(out)
    keep = []
    for i in range(m):
        a, b, c = out[i - 1], out[i], out[(i + 1) % m]
        if abs((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])) > 1e-10:
            keep.append(b)
    return keep


def upright_piano():
    W, H, D = PIANO_W, PIANO_H, PIANO_D
    zb, zf = D / 2, -D / 2
    side_x = W / 2 - 0.035            # the sides' inner faces
    kx = 52 * PITCH / 2               # half the keyboard
    top = H - 0.025                   # under the top lid
    case = Part("case")
    # Sides, floor to lid; the arms (cheeks) carry on forward of them.
    for sx in (-1, 1):
        a, b = sorted((sx * side_x, sx * W / 2))
        case.prism(fillet(rect(-0.04, zb, 0.0, top), 0.008, 2), a, b, "Wood", axis="x",
                   bevel=0.004, segs=1)
    # Back, the top lid (overhanging), the upper and lower front panels
    # set 5 mm back from the sides' front edges.
    case.box((2 * side_x, top, 0.025), (0.0, top / 2, zb - 0.0125), "Wood")
    case.prism(fillet(rect(-0.06, zb + 0.012, top, H), 0.008, 2), -W / 2 - 0.012, W / 2 + 0.012,
               "Wood", axis="x", bevel=0.005, segs=2)
    case.box((2 * side_x, top - 0.76, 0.02), (0.0, (0.76 + top) / 2, -0.025), "Wood")
    case.box((2 * side_x, 0.66 - 0.1, 0.02), (0.0, 0.38, -0.025), "Wood")
    # Raised moulding frames on both panels (6 mm proud).
    for y0, y1 in ((1.0, 1.13), (0.17, 0.6)):
        for x0, x1 in ((-0.62, -0.02), (0.02, 0.62)):
            for cx, cy, w, h in (((x0 + x1) / 2, y0, x1 - x0 + 0.02, 0.02),
                                 ((x0 + x1) / 2, y1, x1 - x0 + 0.02, 0.02),
                                 (x0, (y0 + y1) / 2, 0.02, y1 - y0 - 0.02),
                                 (x1, (y0 + y1) / 2, 0.02, y1 - y0 - 0.02)):
                case.box((w, h, 0.006), (cx, cy, -0.038), "Wood", bevel=0.002, segs=1)
    # Plinth along the floor between the toes, proud of the knee panel.
    toe_x = side_x - 0.085
    case.box((2 * toe_x, 0.1, zb - 0.025 + 0.06), (0.0, 0.05, (-0.06 + zb - 0.025) / 2), "Wood",
             bevel=0.003, segs=1)
    # Keybed, key slip, the name rail over the keys' backs.
    case.box((2 * kx + 0.004, KEY_BOT - 0.66, -0.035 - Z_FRONT),
             (0.0, (0.66 + KEY_BOT) / 2, (Z_FRONT - 0.035) / 2), "Wood")
    case.box((2 * kx + 0.004, 0.049, 0.017), (0.0, 0.6795, Z_FRONT - 0.0085), "Wood",
             bevel=0.002, segs=1)
    case.box((2 * kx + 0.004, 0.025, 0.04), (0.0, 0.7475, -0.055), "Wood", bevel=0.002, segs=1)
    items_kit.text(case, "ПИАНИНО", 0.012, "Brass", T(0.0, 0.7475, -0.075) @ RY(math.pi),
                   depth=0.0008)
    for sx in (-1, 1):
        # The arm: a block from the keys' end out to the side's face.
        a, b = sorted((sx * (kx + 0.002), sx * W / 2))
        cheek = fillet(rect(Z_FRONT - 0.02, -0.04, 0.66, 0.775), 0.012, 3)
        case.prism(cheek, a, b, "Wood", axis="x", bevel=0.004, segs=1)
        # A turned column under it, standing on the toe.
        case.lathe([(0.0, 0.09), (0.03, 0.09), (0.03, 0.12), (0.022, 0.15), (0.026, 0.3),
                    (0.018, 0.5), (0.024, 0.6), (0.03, 0.62), (0.03, 0.66), (0.0, 0.66)], "Wood",
                   segs=16, xf=T(sx * (side_x - 0.04), 0, -0.17) @ UP)
        toe = fillet([(-0.04, 0.0), (-0.04, 0.09), (zf + 0.03, 0.09), (zf, 0.06), (zf, 0.0)],
                     0.01, 2)
        a, b = sorted((sx * toe_x, sx * W / 2))
        case.prism(toe, a, b, "Wood", axis="x", bevel=0.004, segs=1)
    # Felt-lined slots for the pedals in the plinth's front.
    for sx in (-1, 1):
        case.box((0.03, 0.026, 0.004), (sx * 0.06, 0.05, -0.062), "Black")
    parts = [case]

    # Fallboard, modelled open: built closed over the keys, turned up
    # about its hinge on the name rail's top front edge.
    lid = Part("lid", origin=HINGE)
    OPEN = RX(LID_OPEN, about=HINGE)
    lid.box((2 * kx, 0.012, 0.155), (0.0, 0.754, HINGE[2] - 0.0775), "Wood", bevel=0.002,
            segs=1, xf=OPEN)
    lid.box((2 * kx, 0.034, 0.014), (0.0, 0.743, HINGE[2] - 0.155 - 0.007), "Wood", bevel=0.003,
            segs=1, xf=OPEN)
    for sx in (-1, 1):
        lid.cyl(0.004, sx * 0.4, sx * 0.43, "Brass", segs=8,
                xf=T(0.0, HINGE[1], HINGE[2]) @ ALONG_X)
    parts.append(lid)

    # Keys: ivory whites cut round the blacks, ebony blacks raised.
    wx, bx = key_layout()
    g = 0.0005
    for n in range(21, 109):
        if n in wx:
            x = wx[n]
            xl, xr = x - PITCH / 2 + g, x + PITCH / 2 - g
            bl = bx[n - 1] + BLACK_W / 2 + 0.001 if (n - 1) in bx else xl
            br = bx[n + 1] - BLACK_W / 2 - 0.001 if (n + 1) in bx else xr
            zh = Z_FRONT + HEAD
            k = Part(f"key_{n}", origin=(x, KEY_TOP, Z_BACK))
            k.prism(outline([(xl, Z_FRONT), (xr, Z_FRONT), (xr, zh), (br, zh), (br, Z_BACK),
                             (bl, Z_BACK), (bl, zh), (xl, zh)]), KEY_BOT, KEY_TOP, "Cream",
                    axis="y")
        else:
            x = bx[n]
            yt = KEY_TOP + BLACK_UP
            k = Part(f"key_{n}", origin=(x, yt, Z_BACK))
            k.prism([(x - BLACK_W / 2, KEY_BOT + 0.004), (x + BLACK_W / 2, KEY_BOT + 0.004),
                     (x + BLACK_W / 2 - 0.0012, yt - 0.003), (x + 0.0042, yt),
                     (x - 0.0042, yt), (x - BLACK_W / 2 + 0.0012, yt - 0.003)],
                    Z_BACK - BLACK_LEN, Z_BACK, "Rotor", axis="z")
        parts.append(k)

    # Pedals: brass levers out of the plinth, pivoting inside it.
    for name, sx in (("pedal_l", -1), ("pedal_r", 1)):
        x = sx * 0.06
        pd = Part(name, origin=(x, 0.05, 0.0))
        pd.box((0.012, 0.01, 0.09), (x, 0.05, -0.045), "Brass")
        pd.lathe([(0.0, 0.0), (0.014, 0.0), (0.017, 0.03), (0.013, 0.07), (0.0, 0.08)], "Brass",
                 segs=16, xf=T(x, 0.05, -0.085) @ S(1.0, 0.3, 1.0) @ RX(math.pi))
        parts.append(pd)

    # Music desk: a ledge on the upper panel over the open fallboard.
    desk = Part("music_desk")
    desk.box((0.7, 0.014, 0.075), (0.0, 0.957, -0.035 - 0.0375), "Wood", bevel=0.003, segs=1)
    desk.box((0.7, 0.03, 0.012), (0.0, 0.979, -0.104), "Wood", bevel=0.003, segs=1)
    for sx in (-1, 1):
        desk.prism([(-0.035, 0.95), (-0.035, 0.91), (-0.056, 0.95)], sx * 0.3 - 0.01,
                   sx * 0.3 + 0.01, "Wood", axis="x")
    parts.append(desk)
    return parts


prop("gramophone", gramophone)
prop("upright_piano", upright_piano)
