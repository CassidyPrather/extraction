"""Music in the houses: a wind-up horn gramophone, an upright piano and a
chapel harmonium.
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
- harmonium: a reed organ, 1.2 x 1.3 x 0.6 m. `case`; `key_36` .. `key_96`,
  the piano's keys exactly (sizes, heights, pivots), the piano's layout
  moved left by `harmonium_offset()` (0.023 m) so the five octaves sit
  centred on the case; `stops` (the row of knobs over the keys);
  `pedal_l`, `pedal_r` (the bellows treadles, origin on their hinge pin
  at the back, a negative turn about X presses them); `music_desk`.
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
    # On the box's face inside the moulding (it stood 5 mm off it).
    case.box((0.07, 0.022, 0.004), (0.0, 0.086, -D / 2 - 0.0015), "Brass", bevel=0.001, segs=1)
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


def keyboard(lo, hi, dx=0.0):
    """Keys `key_<lo>` .. `key_<hi>` (MIDI numbers), the piano's layout
    moved `dx` along X: ivory whites cut round the blacks, ebony blacks
    raised. Each key's origin is at the back of its top face, the pivot."""
    wx, bx = key_layout()
    wx = {n: x + dx for n, x in wx.items() if lo <= n <= hi}
    bx = {n: x + dx for n, x in bx.items() if lo <= n <= hi}
    g = 0.0005
    parts = []
    for n in range(lo, hi + 1):
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
    return parts


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

    parts += keyboard(21, 108)

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


# --- Harmonium ----------------------------------------------------------------------------

HARM_W, HARM_H, HARM_D = 1.2, 1.3, 0.6
HARM_LO, HARM_HI = 36, 96


def harmonium_offset():
    """How far the piano's key layout is moved left so the harmonium's
    five octaves are centred on its case: x = x_piano - offset."""
    wx, _ = key_layout()
    return ((wx[HARM_LO] - PITCH / 2) + (wx[HARM_HI] + PITCH / 2)) / 2


def harmonium():
    W, H, D = HARM_W, HARM_H, HARM_D
    zb = D / 2
    so, si = W / 2 - 0.015, W / 2 - 0.045      # the sides' outer and inner faces
    kx = (HARM_HI - HARM_LO) // 12 * 7 * PITCH / 2 + PITCH / 2   # half the keyboard
    top = 1.1                                  # under the top lid
    case = Part("case")
    # Sides: the lower case, the arms either side of the keys, the upper
    # case set back above them.
    side = fillet([(-0.2, 0.0), (zb, 0.0), (zb, top), (-0.075, top), (-0.075, 0.8),
                   (-0.255, 0.8), (-0.255, 0.64), (-0.23, 0.57), (-0.2, 0.53)], 0.01, 2)
    for sx in (-1, 1):
        a, b = sorted((sx * si, sx * so))
        case.prism(side, a, b, "Wood", axis="x", bevel=0.004, segs=1)
        # A raised moulding frame on the side's outer face (6 mm proud).
        for cz, cy, sz, sy in ((0.05, 0.1, 0.32, 0.02), (0.05, 0.5, 0.32, 0.02),
                               (-0.1, 0.3, 0.02, 0.38), (0.2, 0.3, 0.02, 0.38)):
            case.box((0.006, sy, sz), (sx * (so + 0.003), cy, cz), "Wood", bevel=0.002, segs=1)
    # Back; the floor of the treadle well, its back (the bellows' front),
    # the shelf over it and the knee board; a shaped apron under that.
    case.box((2 * si, top, 0.02), (0.0, top / 2, zb - 0.015), "Wood")
    case.box((2 * si, 0.02, 0.2), (0.0, 0.01, -0.1), "Wood")
    case.box((2 * si, 0.29, 0.02), (0.0, 0.165, 0.01), "Wood")
    case.box((2 * si, 0.02, 0.17), (0.0, 0.3, -0.085), "Wood")
    case.box((2 * si, 0.35, 0.02), (0.0, 0.465, -0.18), "Wood")
    arch = [(-0.5 + t / 12, 0.21 + 0.04 * math.sin(math.pi * t / 12)) for t in range(1, 12)]
    case.prism([(-si, 0.29), (-si, 0.19), (-0.5, 0.21)] + arch +
               [(0.5, 0.21), (si, 0.19), (si, 0.29)], -0.2, -0.18, "Wood", axis="z")
    for x0, x1 in ((-0.52, -0.02), (0.02, 0.52)):
        for cx, cy, w, h in (((x0 + x1) / 2, 0.34, x1 - x0 + 0.02, 0.02),
                             ((x0 + x1) / 2, 0.59, x1 - x0 + 0.02, 0.02),
                             (x0, 0.465, 0.02, 0.23), (x1, 0.465, 0.02, 0.23)):
            case.box((w, h, 0.006), (cx, cy, -0.193), "Wood", bevel=0.002, segs=1)
    # Iron brackets on the well's floor for the treadles' hinge pins.
    for px in (-0.14, 0.14):
        for sx in (-1, 1):
            case.box((0.012, 0.03, 0.03), (px + sx * 0.116, 0.035, -0.02), "Iron", bevel=0.002,
                     segs=1)
    # Keybed, key slip, the cheeks at the keys' ends, the name rail over
    # the keys' backs, the stop board above it.
    case.box((2 * kx + 0.004, KEY_BOT - 0.64, -0.03 - Z_FRONT),
             (0.0, (0.64 + KEY_BOT) / 2, (Z_FRONT - 0.03) / 2), "Wood")
    case.box((2 * kx + 0.004, 0.049, 0.017), (0.0, 0.6795, Z_FRONT - 0.0085), "Wood",
             bevel=0.002, segs=1)
    for sx in (-1, 1):
        a, b = sorted((sx * (kx + 0.002), sx * si))
        case.prism(fillet(rect(Z_FRONT - 0.02, -0.03, 0.64, 0.77), 0.012, 3), a, b, "Wood",
                   axis="x", bevel=0.004, segs=1)
    case.box((2 * kx + 0.004, 0.025, 0.06), (0.0, 0.7475, -0.06), "Wood", bevel=0.002, segs=1)
    items_kit.text(case, "ФИСГАРМОНИЯ", 0.011, "Brass", T(0.0, 0.7475, -0.09) @ RY(math.pi),
                   depth=0.0008)
    case.box((2 * si, 0.08, 0.04), (0.0, 0.8, -0.05), "Wood", bevel=0.002, segs=1)
    # The upper case's front behind the desk, a raised panel each side of
    # the desk with a brass candle sconce on it.
    case.box((2 * si, top - 0.64, 0.02), (0.0, (0.64 + top) / 2, -0.02), "Wood")
    for sx in (-1, 1):
        x0, x1 = sorted((sx * 0.37, sx * 0.53))
        for cx, cy, w, h in (((x0 + x1) / 2, 0.87, x1 - x0 + 0.02, 0.02),
                             ((x0 + x1) / 2, 1.07, x1 - x0 + 0.02, 0.02),
                             (x0, 0.97, 0.02, 0.18), (x1, 0.97, 0.02, 0.18)):
            case.box((w, h, 0.006), (cx, cy, -0.033), "Wood", bevel=0.002, segs=1)
        c = sx * 0.45
        case.cyl(0.03, -0.034, -0.03, "Brass", segs=20, xf=T(c, 0.95, 0.0))
        rod(case, (c, 0.95, -0.032), (c, 0.95, -0.1), 0.005, "Brass", segs=8)
        case.lathe([(0.0, 0.0), (0.03, 0.0), (0.032, 0.004), (0.012, 0.008), (0.012, 0.03),
                    (0.016, 0.034), (0.0, 0.034)], "Brass", segs=20,
                   xf=T(c, 0.944, -0.1) @ UP)
    # The top lid, then the crest: a pointed board with a raised trefoil,
    # turned finials at its ends and on its point.
    case.prism(fillet(rect(-0.1, zb, top, top + 0.025), 0.008, 2), -W / 2, W / 2, "Wood",
               axis="x", bevel=0.005, segs=2)
    yt = top + 0.025
    crest = [(-0.5, yt), (0.5, yt), (0.5, yt + 0.04)]
    for i in range(1, 12):
        t = i / 12
        y = yt + 0.04 + 0.07 * t ** 1.4 + 0.01 * math.sin(3 * math.pi * t)
        crest.append((0.5 * (1 - t), y))
    crest.append((0.0, yt + 0.11))
    crest += [(-x, y) for x, y in crest[3:-1][::-1]]
    crest.append((-0.5, yt + 0.04))
    case.prism(crest, 0.2, 0.22, "Wood", axis="z")
    for a in (90, 210, 330):
        cx = 0.018 * math.cos(math.radians(a))
        cy = yt + 0.065 + 0.018 * math.sin(math.radians(a))
        items_kit.ring(case, 0.016, 0.004, "Wood", xf=T(cx, cy, 0.2), segs=24, sides=6)
    for sx in (-1, 1):
        items_kit.ring(case, 0.012, 0.004, "Wood", xf=T(sx * 0.3, yt + 0.03, 0.2), segs=20,
                       sides=6)
    fin = [(0.0, 0.0), (0.03, 0.0), (0.03, 0.012), (0.018, 0.02), (0.024, 0.045),
           (0.012, 0.07), (0.016, 0.08), (0.006, 0.1), (0.0, 0.11)]
    for sx in (-1, 1):
        case.lathe(fin, "Wood", segs=16, xf=T(sx * 0.53, yt, 0.21) @ UP)
    case.lathe([(0.0, 0.0), (0.012, 0.0), (0.014, 0.01), (0.008, 0.02), (0.011, 0.035),
                (0.005, 0.06), (0.0, H - yt - 0.1)], "Wood", segs=14,
               xf=T(0.0, yt + 0.1, 0.21) @ UP)
    parts = [case]

    parts += keyboard(HARM_LO, HARM_HI, -harmonium_offset())

    # Stops: a row of turned knobs with ivory faces out of the stop board.
    sp = (0.0, 0.8, -0.07)
    stops = Part("stops", origin=sp)
    for x in (-0.45, -0.36, -0.27, -0.18, -0.09, 0.09, 0.18, 0.27, 0.36, 0.45):
        xf = T(x, sp[1], sp[2]) @ RY(math.pi)
        stops.lathe([(0.0, -0.003), (0.006, -0.003), (0.006, 0.012), (0.01, 0.015),
                     (0.011, 0.022), (0.0095, 0.027), (0.0, 0.027)], "Wood", segs=16, xf=xf)
        stops.cyl(0.0075, 0.026, 0.031, "Cream", segs=16, xf=xf)
    parts.append(stops)

    # Treadles: carpeted boards hinged at the back of the well, their
    # fronts tilted up; a negative turn about X presses them.
    tilt = math.radians(18)
    for name, x in (("pedal_l", -0.14), ("pedal_r", 0.14)):
        hg = (x, 0.035, -0.02)
        pd = Part(name, origin=hg)
        up = RX(tilt, about=hg)
        pd.cyl(0.006, x - 0.11, x + 0.11, "Iron", segs=10, xf=T(0.0, hg[1], hg[2]) @ ALONG_X)
        pd.box((0.2, 0.014, 0.27), (x, hg[1], hg[2] - 0.135), "Wood", bevel=0.002, segs=1,
               xf=up)
        pd.box((0.17, 0.005, 0.23), (x, hg[1] + 0.0095, hg[2] - 0.14), "Red", xf=up)
        pd.box((0.2, 0.026, 0.01), (x, hg[1] + 0.006, hg[2] - 0.275), "Brass", bevel=0.002,
               segs=1, xf=up)
        parts.append(pd)

    # Music desk: a ledge on the stop board, a board leaning back on it.
    desk = Part("music_desk")
    desk.box((0.62, 0.014, 0.075), (0.0, 0.847, -0.0675), "Wood", bevel=0.003, segs=1)
    desk.prism([(-0.099, 0.854), (-0.087, 0.854), (-0.04, 1.03), (-0.052, 1.03)], -0.3, 0.3,
               "Wood", axis="x")
    desk.box((0.62, 0.03, 0.012), (0.0, 0.866, -0.099), "Wood", bevel=0.003, segs=1)
    parts.append(desk)
    return parts


prop("gramophone", gramophone)
prop("upright_piano", upright_piano)
prop("harmonium", harmonium)
