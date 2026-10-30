"""The yards round the common houses: what made a house a home with a
garden, visual only (no colliders), placed by `world::clutter::yard` along
a house's sides and back by valley. Origins on the ground under the
thing's centre, its front to -Z; things that stand against a wall have
their back (+Z) to it.

- `clothes_line`       two T-posts 4 m apart, two lines, washing pegged
- `clothes_line_bare`  Cinder's: iron pipe posts, one line, a grey rag
- `garden_bed_cabbage` a raised plank bed, two rows of cabbages (one bolted)
- `garden_bed_roots`   a raised bed: potato haulm on a ridge, a row of leeks
- `bench_yard`         a plank bench for under the window
- `bench_charred`      the same, burnt, one seat board gone short
- `rain_barrel`        a hooped barrel on bearers, half a lid, a tap
- `woodpile_lean`      split logs stacked under a lean-to roof
- `washtub_stand`      a zinc tub on its stand, a washboard in it
- `chopping_block`     a stump with the axe in it, split pieces by it
- `beehive`            a painted hive on blocks, a brick on its lid
- `hen_coop`           a small coop on legs, its ramp, a nest box
- `handcart`           a two-wheeled cart, half loaded (Marrow's evacuees)
- `hand_pump`          a cast-iron yard pump on its slab, a bucket under it
- `dog_kennel`         Hana's dog's kennel (Marrow, her house only)
- `tub_iron`           Cinder's: an old iron bath used as a trough, sooted
- `coal_bunker`        Kettle's: a plank bunker, coal spilled at its hatch

Materials: Timber, Planks, Boards, Iron, Cast, Rusty, Zinc, Concrete, Brick,
Corrugated, Shingles, Hide (soil), Leaf, Olive, Yellow, Cream, Blue, Ticking,
Wool, DarkWool, Canvas, Char, Black.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import ALONG_X, UP, Wear, board, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, S, T  # noqa: E402

import _kit  # noqa: E402

# Out in the weather: the rain reaches what nothing of its own covers.
_kit.AO_DISTANCE = 0.35
_kit.AO_SKY = True


def blob(p, size, at, mat, segs=8, rings=5, xf=None):
    """A soft lump: a squashed sphere."""
    x = T(*at) @ S(*size)
    if xf is not None:
        x = xf @ x
    p.sphere(1.0, (0, 0, 0), mat, segs=segs, rings=rings, xf=x)


# --- The clothes line -------------------------------------------------------------------------

POST_TOP = 1.9
# Into the bars at the posts (x 2.0 less 0.03: it stopped 6 mm short).
LINE_HX = 1.98


def line_y(x, sag):
    """A line's height at x: from under the bars' tops, sagging in the middle."""
    y0 = POST_TOP - 0.055
    return y0 - sag * (1.0 - (x / LINE_HX) ** 2)


def line(p, z, sag, mat="Rope"):
    n = 8
    pts = [(-LINE_HX + 2 * LINE_HX * k / n) for k in range(n + 1)]
    for a, b in zip(pts, pts[1:]):
        rod(p, (a, line_y(a, sag), z), (b, line_y(b, sag), z), 0.004, mat, segs=4)


def hang(p, x, wid, hgt, z, sag, mat, dz=0.0, pegs=True):
    """A cloth `wid` x `hgt` pegged on the line at z (its top following the
    sag), `dz` out in front of the line for a piece laid over another."""
    slope = 2 * sag * x / LINE_HX ** 2
    top = line_y(x, sag) - 0.004
    xf = T(x, top, z + dz) @ RZ(math.atan(slope))
    p.box((wid, hgt, 0.008), (0, -hgt / 2, 0), mat, xf=xf)
    if pegs:
        for sx in (-1, 1):
            p.box((0.014, 0.065, 0.02), (sx * (wid / 2 - 0.03), -0.012, -dz), "Timber",
                  xf=xf)


def t_post(p, x, mat):
    p.box((0.08, POST_TOP + 0.3, 0.08), (x, (POST_TOP - 0.3) / 2, 0.0), mat, bevel=0.006,
          segs=1)
    p.box((0.06, 0.07, 0.62), (x, POST_TOP - 0.08, 0.0), mat, bevel=0.005, segs=1)
    for sz in (-1, 1):
        rod(p, (x, POST_TOP - 0.42, sz * 0.03), (x, POST_TOP - 0.12, sz * 0.22), 0.018, mat,
            segs=5)


def clothes_line():
    p = Part("static")
    for x in (-2.0, 2.0):
        t_post(p, x, "Timber")
    sag = 0.13
    zf, zb = -0.24, 0.24
    line(p, zf, sag)
    line(p, zb, sag)
    # The front line: a sheet, a shirt, a towel, a pair of socks.
    hang(p, -1.05, 1.1, 0.78, zf, sag, "Cream")
    hang(p, -0.05, 0.46, 0.62, zf, sag, "Blue")
    for sx in (-1, 1):
        # The shirt's sleeves hanging down its sides, in front of it.
        x = -0.05 + sx * 0.2
        top = line_y(x, sag) - 0.06
        p.box((0.13, 0.42, 0.008), (0, -0.21, 0), "Blue",
              xf=T(x, top, zf - 0.007) @ RZ(sx * 0.18))
    hang(p, 0.62, 0.42, 0.56, zf, sag, "Ticking")
    for x in (1.12, 1.3):
        hang(p, x, 0.09, 0.3, zf, sag, "Wool", pegs=False)
        p.box((0.014, 0.065, 0.02), (x, line_y(x, sag) - 0.016, zf), "Timber")
    # The back line: work trousers by their hems, a pillowcase.
    for x in (-0.95, -0.7):
        hang(p, x, 0.2, 0.86, zb, sag, "DarkWool", pegs=False)
        p.box((0.014, 0.065, 0.02), (x, line_y(x, sag) - 0.016, zb), "Timber")
    hang(p, 0.9, 0.62, 0.42, zb, sag, "Cream")
    return [p]


def clothes_line_bare():
    p = Part("static")
    for x in (-2.0, 2.0):
        p.cyl(0.032, -0.3, POST_TOP, "Rusty", segs=8, xf=T(x, 0, 0) @ UP)
        rod(p, (x, POST_TOP - 0.06, -0.3), (x, POST_TOP - 0.06, 0.3), 0.022, "Rusty", segs=6)
    sag = 0.22
    line(p, -0.24, sag, mat="Iron")
    hang(p, 0.55, 0.36, 0.3, -0.24, sag, "Wool", pegs=False)
    p.box((0.014, 0.065, 0.02), (0.6, line_y(0.6, sag) - 0.016, -0.24), "Char")
    return [p]


# --- Garden beds ------------------------------------------------------------------------------

BED_L, BED_D, BED_H = 2.4, 1.0, 0.28
SOIL = BED_H - 0.05


def raised_bed(p, w):
    for sz in (-1, 1):
        board(p, (BED_L, BED_H, 0.03), (0.0, BED_H / 2 - 0.02, sz * (BED_D / 2 - 0.015)),
              "Planks", w, bevel=0.004)
    for sx in (-1, 1):
        board(p, (0.03, BED_H - 0.01, BED_D - 0.062), (sx * (BED_L / 2 - 0.015),
              BED_H / 2 - 0.025, 0.0), "Planks", w, bevel=0.004)
    p.box((BED_L - 0.064, SOIL + 0.02, BED_D - 0.064), (0.0, SOIL / 2 - 0.01, 0.0), "Hide")


def garden_bed_cabbage():
    w = Wear(301)
    p = Part("static")
    raised_bed(p, w)
    for k, sz in enumerate((-1, 1)):
        for i in range(4):
            x = -0.84 + i * 0.56 + w.j(0.04)
            z = sz * 0.24 + w.j(0.03)
            if k == 1 and i == 2:
                # One gone to seed: a tall stalk, yellow flowers.
                blob(p, (0.16, 0.05, 0.16), (x, SOIL + 0.03, z), "Olive", segs=7, rings=4)
                rod(p, (x, SOIL, z), (x + 0.03, SOIL + 0.62, z), 0.014, "Olive", segs=5)
                blob(p, (0.07, 0.06, 0.07), (x + 0.03, SOIL + 0.66, z), "Yellow", segs=6,
                     rings=4)
                continue
            if k == 0 and i == 3:
                # One eaten to the stump.
                rod(p, (x, SOIL - 0.02, z), (x, SOIL + 0.09, z), 0.025, "Olive", segs=5)
                continue
            s = 1.0 + w.j(0.12)
            blob(p, (0.22 * s, 0.05, 0.22 * s), (x, SOIL + 0.035, z), "Olive", segs=7,
                 rings=4)
            blob(p, (0.14 * s, 0.11 * s, 0.14 * s), (x, SOIL + 0.09 * s, z), "Leaf")
    return [p]


def garden_bed_roots():
    w = Wear(303)
    p = Part("static")
    raised_bed(p, w)
    # The potatoes' ridge along the front, haulm sprawled on it.
    p.prism([(-0.4, SOIL - 0.01), (-0.08, SOIL - 0.01), (-0.18, SOIL + 0.08),
             (-0.3, SOIL + 0.08)], -BED_L / 2 + 0.06, BED_L / 2 - 0.06, "Hide", axis="x")
    for i in range(5):
        x = -0.9 + i * 0.45 + w.j(0.05)
        for k in range(3):
            a = 2 * math.pi * k / 3 + w.j(0.5)
            blob(p, (0.12, 0.09, 0.12), (x + 0.07 * math.cos(a), SOIL + 0.13,
                 -0.24 + 0.07 * math.sin(a)), "Olive" if (i + k) % 2 else "Leaf",
                 segs=6, rings=4)
    # A row of leeks at the back, their blades fanned.
    for i in range(8):
        x = -0.98 + i * 0.28 + w.j(0.03)
        z = 0.24 + w.j(0.02)
        p.cyl(0.02, SOIL - 0.02, SOIL + 0.14, "Cream", segs=5, xf=T(x, 0, z) @ UP)
        for k, a in enumerate((-0.35, 0.3)):
            p.box((0.035, 0.3, 0.004), (0, 0.15, 0), "Olive",
                  xf=T(x, SOIL + 0.12, z) @ RX(0.25 * (k * 2 - 1)) @ RZ(a + w.j(0.1)))
    return [p]


# --- Benches ----------------------------------------------------------------------------------

def bench(p, w, mat, charred=False):
    L = 1.5
    for k, z in enumerate((-0.085, 0.085)):
        if charred and k == 0:
            # Burnt back to a stub over the right leg, its end dropped.
            board(p, (0.86, 0.04, 0.16), (0.32, 0.43, z), mat, w, bevel=0.006, roll=0.04)
            continue
        board(p, (L + w.j(0.01), 0.04, 0.16), (w.j(0.01), 0.43, z), mat, w, bevel=0.006)
    for sx in (-1, 1):
        board(p, (0.05, 0.41, 0.3), (sx * 0.55, 0.205, 0.0), mat, w, bevel=0.005)
    board(p, (1.05, 0.06, 0.03), (0.0, 0.15, 0.0), mat, w, bevel=0.004)


def bench_yard():
    p = Part("static")
    bench(p, Wear(311), "Planks")
    return [p]


def bench_charred():
    p = Part("static")
    bench(p, Wear(313), "Char", charred=True)
    return [p]


# --- Rain barrel ------------------------------------------------------------------------------

def barrel_r(y):
    """The barrel's radius at y: 0.27 at its ends (0.12, 1.0), 0.3 at its belly."""
    t = (y - 0.56) / 0.44
    return 0.3 - 0.03 * t * t


def rain_barrel():
    w = Wear(321)
    p = Part("static")
    prof = [(0.0, 0.12)] + [(barrel_r(0.12 + 0.88 * k / 6), 0.12 + 0.88 * k / 6)
                             for k in range(7)] + [(0.0, 1.0)]
    p.lathe(prof, "Boards", segs=14, xf=UP)
    for y in (0.24, 0.88):
        r = barrel_r(y)
        p.tube(r + 0.008, r - 0.02, y - 0.022, y + 0.022, "Iron", segs=14,
               xf=T(0, 0, 0) @ UP)
    for sz in (-1, 1):
        board(p, (0.62, 0.12, 0.1), (0.0, 0.06, sz * 0.14), "Timber", w, bevel=0.006)
    # Half a lid, slid back.
    board(p, (0.46, 0.025, 0.24), (0.04, 1.0125, 0.1), "Planks", w, bevel=0.004)
    rod(p, (0.0, 0.22, -0.26), (0.0, 0.22, -0.36), 0.013, "Iron", segs=6)
    rod(p, (0.0, 0.22, -0.34), (0.0, 0.27, -0.34), 0.008, "Iron", segs=5)
    return [p]


# --- Woodpile ---------------------------------------------------------------------------------

def woodpile_lean():
    w = Wear(331)
    p = Part("static")
    front, back = -0.36, 0.33
    yf, yb = 1.15, 1.38
    for sx in (-1, 1):
        board(p, (0.08, yf, 0.08), (sx * 0.98, yf / 2, front), "Timber", w, bevel=0.006)
        board(p, (0.08, yb, 0.08), (sx * 0.98, yb / 2, back), "Timber", w, bevel=0.006)
    board(p, (2.06, 0.08, 0.08), (0.0, yf + 0.04, front), "Timber", w, bevel=0.006)
    board(p, (2.06, 0.08, 0.08), (0.0, yb + 0.04, back), "Timber", w, bevel=0.006)
    # The roof sheet on the two beams.
    a = math.atan2(yb - yf, back - front)
    run = math.hypot(yb - yf, back - front) + 0.22
    mid = ((front + back) / 2, (yf + yb) / 2 + 0.08 + 0.008 / math.cos(a))
    p.box((2.24, 0.016, run), (0, 0, 0), "Corrugated",
          xf=T(0.0, mid[1], mid[0]) @ RX(a))
    # Bearers, the logs on them, ends out.
    for sz in (-1, 1):
        board(p, (1.9, 0.05, 0.06), (0.0, 0.025, sz * 0.2), "Timber", w, bevel=0.004)
    r = 0.068
    rows = (12, 12, 11, 10, 7)
    for k, n in enumerate(rows):
        y = 0.05 + r + k * 0.121
        x0 = -0.77 + (0.07 if k % 2 else 0.0)
        for i in range(n):
            x = x0 + i * 0.14
            z0 = -0.31 + w.j(0.025)
            mat = "Timber" if w.chance(0.55) else "Planks"
            p.cyl(r - 0.002 + w.j(0.004), z0, z0 + 0.58 + w.j(0.03), mat, segs=5,
                  xf=T(x, y, 0) @ RZ(w.j(0.6)))
    return [p]


# --- Washtub ----------------------------------------------------------------------------------

def washtub_stand():
    w = Wear(341)
    p = Part("static")
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.045, 0.6, 0.045), (sx * 0.25, 0.3, sz * 0.25), "Timber", w,
                  bevel=0.004)
        board(p, (0.6, 0.04, 0.034), (0.0, 0.2, sx * 0.25), "Timber", w, bevel=0.004)
    for sz in (-1, 1):
        board(p, (0.62, 0.03, 0.09), (0.0, 0.585, sz * 0.12), "Planks", w, bevel=0.004)
    p.lathe([(0.0, 0.6), (0.26, 0.6), (0.33, 0.86), (0.345, 0.872), (0.322, 0.872),
             (0.25, 0.618), (0.0, 0.618)], "Zinc", segs=18, xf=UP)
    p.cyl(0.262, 0.7, 0.703, "Black", segs=18, xf=UP)
    for sx in (-1, 1):
        rod(p, (sx * 0.33, 0.82, -0.05), (sx * 0.37, 0.82, 0.0), 0.008, "Zinc", segs=5)
        rod(p, (sx * 0.37, 0.82, 0.0), (sx * 0.33, 0.82, 0.05), 0.008, "Zinc", segs=5)
    # The washboard leant in it.
    xf = T(0.05, 0.62, 0.12) @ RX(-0.35)
    for sx in (-1, 1):
        p.box((0.03, 0.6, 0.02), (sx * 0.15, 0.3, 0.0), "Timber", xf=xf)
    p.box((0.27, 0.035, 0.02), (0.0, 0.58, 0.0), "Timber", xf=xf)
    p.box((0.27, 0.3, 0.008), (0.0, 0.38, -0.004), "Zinc", xf=xf)
    return [p]


# --- Chopping block ---------------------------------------------------------------------------

def chopping_block():
    w = Wear(351)
    p = Part("static")
    p.cyl(0.24, -0.02, 0.45, "Hide", segs=10, xf=UP)
    p.cyl(0.226, 0.44, 0.458, "Timber", segs=10, xf=UP)
    # The axe, its blade in the top, the haft out over the side.
    xf = T(-0.04, 0.47, 0.02) @ RZ(-0.42)
    p.box((0.17, 0.1, 0.03), (0.0, -0.03, 0.0), "Iron", bevel=0.004, xf=xf)
    rod(p, (0.06, 0.465, 0.02), (0.52, 0.76, -0.12), 0.017, "Timber", segs=6)
    # Split pieces fallen by it.
    for k, (x, z, yaw) in enumerate(((0.37, -0.12, 0.3), (0.3, 0.2, -0.6),
                                      (-0.36, -0.08, 1.4))):
        L = 0.36 + w.j(0.04)
        p.prism([(-0.06, 0.0), (0.06, 0.0), (0.0, 0.09)], -L / 2, L / 2,
                "Timber" if k % 2 else "Planks", axis="x",
                xf=T(x, 0, z) @ RY(yaw))
    return [p]


# --- Beehive ----------------------------------------------------------------------------------

def beehive():
    w = Wear(361)
    p = Part("static")
    for sz in (-1, 1):
        p.box((0.44, 0.2, 0.15), (0.0, 0.1, sz * 0.15), "Concrete", bevel=0.01, segs=1)
    board(p, (0.46, 0.04, 0.52), (0.0, 0.22, 0.0), "Planks", w, bevel=0.004)
    board(p, (0.46, 0.015, 0.07), (0.0, 0.218, -0.285), "Planks", w, bevel=0.003)
    p.box((0.44, 0.26, 0.48), (0.0, 0.37, 0.0), "Cream", bevel=0.006, segs=1)
    p.box((0.45, 0.16, 0.49), (0.0, 0.58, 0.0), "Yellow", bevel=0.006, segs=1)
    p.box((0.3, 0.018, 0.008), (0.0, 0.252, -0.244), "Black")
    p.box((0.52, 0.06, 0.56), (0.0, 0.69, 0.0), "Zinc", bevel=0.006, segs=1)
    p.box((0.22, 0.065, 0.105), (0.05, 0.7525, 0.06), "Brick", bevel=0.006, segs=1,
          xf=T(0.05, 0, 0.06) @ RY(0.3) @ T(-0.05, 0, -0.06))
    return [p]


# --- Hen coop ---------------------------------------------------------------------------------

def hen_coop():
    w = Wear(371)
    p = Part("static")
    L, D, y0, y1 = 1.0, 0.65, 0.35, 0.9
    for sx in (-1, 1):
        for sz in (-1, 1):
            board(p, (0.05, y0 + 0.02, 0.05), (sx * 0.44, (y0 + 0.02) / 2, sz * 0.27),
                  "Timber", w, bevel=0.004)
    p.box((L, y1 - y0, D), (0.0, (y0 + y1) / 2, 0.0), "Planks", bevel=0.006, segs=1)
    p.prism([(-D / 2, y1), (D / 2, y1), (0.0, y1 + 0.22)], -L / 2, L / 2, "Planks",
            axis="x")
    a = math.atan2(0.22, D / 2)
    run = math.hypot(0.22, D / 2) + 0.1
    for s in (-1, 1):
        cz = s * (D / 4 + 0.03)
        cy = y1 + 0.11 + 0.012
        p.box((L + 0.12, 0.014, run), (0, 0, 0), "Corrugated",
              xf=T(0.0, cy, cz) @ RX(s * a))
    # The pop-hole and its ramp; the nest box on the end.
    p.box((0.2, 0.22, 0.008), (0.25, y0 + 0.13, -D / 2 - 0.004), "Black")
    ramp = math.hypot(0.4, y0 + 0.02)
    ang = math.atan2(y0 + 0.02, 0.4)
    xf = T(0.25, (y0 + 0.02) / 2, -D / 2 - 0.2) @ RX(-ang)
    p.box((0.18, 0.018, ramp), (0, 0, 0), "Planks", xf=xf)
    for k in range(4):
        p.box((0.16, 0.014, 0.018), (0, 0.016, -ramp / 2 + 0.07 + k * 0.09), "Timber",
              xf=xf)
    p.box((0.26, 0.28, 0.4), (L / 2 + 0.13, y0 + 0.2, 0.0), "Planks", bevel=0.005, segs=1)
    board(p, (0.3, 0.02, 0.44), (L / 2 + 0.14, y0 + 0.35, 0.0), "Boards", w, bevel=0.003,
          roll=-0.12)
    return [p]


# --- Handcart ---------------------------------------------------------------------------------

def handcart():
    w = Wear(381)
    p = Part("static")
    R, ay = 0.38, 0.38
    for sx in (-1, 1):
        x = sx * 0.47
        xf = T(x, ay, 0.0) @ ALONG_X
        p.tube(R, R - 0.035, -0.025, 0.025, "Timber", segs=16, xf=xf)
        p.cyl(0.055, -0.06, 0.06, "Timber", segs=8, xf=xf)
        for k in range(8):
            a = 2 * math.pi * k / 8 + 0.2
            c, s = math.cos(a), math.sin(a)
            rod(p, (x, ay + 0.05 * s, 0.05 * c), (x, ay + (R - 0.03) * s, (R - 0.03) * c),
                0.012, "Timber", segs=4)
    rod(p, (-0.5, ay, 0.0), (0.5, ay, 0.0), 0.02, "Iron", segs=6)
    for sx in (-1, 1):
        x = sx * 0.3
        board(p, (0.05, 0.06, 1.62), (x, 0.555, -0.22), "Timber", w, bevel=0.005)
        board(p, (0.06, 0.14, 0.08), (x, 0.455, 0.0), "Timber", w, bevel=0.005)
        board(p, (0.05, 0.53, 0.05), (x, 0.26, -0.48), "Timber", w, bevel=0.005)
    rod(p, (-0.33, 0.555, -1.0), (0.33, 0.555, -1.0), 0.018, "Timber", segs=6)
    board(p, (0.8, 0.03, 1.1), (0.0, 0.6, 0.05), "Planks", w, bevel=0.004)
    for sx in (-1, 1):
        board(p, (0.025, 0.22, 1.1), (sx * 0.4125, 0.695, 0.05), "Planks", w, bevel=0.004)
    for z in (-0.5125, 0.6125):
        board(p, (0.8, 0.22, 0.025), (0.0, 0.695, z), "Planks", w, bevel=0.004)
    # What was put on it and left: a sack, a bundle, a tin trunk.
    blob(p, (0.24, 0.16, 0.3), (-0.17, 0.76, 0.3), "Canvas", segs=8, rings=5)
    blob(p, (0.2, 0.12, 0.16), (0.16, 0.73, 0.38), "Wool", segs=8, rings=5)
    p.box((0.5, 0.28, 0.32), (0.05, 0.755, -0.25), "Painted", bevel=0.01, segs=1)
    return [p]


# --- Hand pump --------------------------------------------------------------------------------

def hand_pump():
    p = Part("static")
    p.box((0.8, 0.17, 0.8), (0.0, 0.035, 0.0), "Concrete", bevel=0.02, segs=1)
    top = 0.12
    p.lathe([(0.0, top), (0.13, top), (0.13, top + 0.04), (0.08, top + 0.07),
             (0.075, 0.95), (0.095, 1.0), (0.095, 1.08), (0.06, 1.12), (0.0, 1.13)],
            "Cast", segs=12, xf=UP)
    # The spout, out over the bucket.
    rod(p, (0.0, 0.84, -0.04), (0.0, 0.82, -0.26), 0.03, "Cast", segs=8)
    rod(p, (0.0, 0.83, -0.25), (0.0, 0.76, -0.3), 0.026, "Cast", segs=8)
    # The handle on its pivot, raised behind.
    rod(p, (0.0, 1.08, 0.05), (0.0, 1.11, 0.12), 0.02, "Cast", segs=6)
    rod(p, (0.0, 1.11, 0.1), (0.0, 1.24, 0.42), 0.018, "Cast", segs=6)
    rod(p, (0.0, 1.24, 0.42), (0.0, 1.2, 0.68), 0.018, "Cast", segs=6)
    p.sphere(0.03, (0.0, 1.2, 0.69), "Cast", segs=8, rings=5)
    # A bucket under it.
    bz = -0.28
    p.lathe([(0.0, top), (0.11, top), (0.135, top + 0.26), (0.142, top + 0.268),
             (0.126, top + 0.268), (0.103, top + 0.015), (0.0, top + 0.015)], "Zinc",
            segs=14, xf=T(0, 0, bz) @ UP)
    pts = [(0.14 * math.cos(math.pi * k / 6), top + 0.27 + 0.1 * math.sin(math.pi * k / 6))
           for k in range(7)]
    for a, b in zip(pts, pts[1:]):
        rod(p, (a[0], a[1], bz), (b[0], b[1], bz), 0.004, "Iron", segs=4)
    return [p]


# --- Dog kennel -------------------------------------------------------------------------------

def dog_kennel():
    w = Wear(391)
    p = Part("static")
    W, D, H = 0.9, 1.0, 0.6
    for sx in (-1, 1):
        board(p, (0.08, 0.05, D), (sx * 0.34, 0.025, 0.0), "Timber", w, bevel=0.004)
    p.box((W, H, D), (0.0, 0.05 + H / 2, 0.0), "Planks", bevel=0.006, segs=1)
    y1 = 0.05 + H
    p.prism([(-W / 2, y1), (W / 2, y1), (0.0, y1 + 0.32)], -D / 2, D / 2, "Planks", axis="z")
    a = math.atan2(0.32, W / 2)
    run = math.hypot(0.32, W / 2) + 0.09
    for s in (-1, 1):
        cx = s * (W / 4 + 0.035)
        p.box((run, 0.018, D + 0.14), (0, 0, 0), "Shingles",
              xf=T(cx, y1 + 0.16 + 0.016, 0.0) @ RZ(-s * a))
    # The doorway: dark, arched.
    arch = [(-0.17, 0.08)] + [(0.17 * math.cos(math.pi * k / 6),
                                0.42 + 0.17 * math.sin(math.pi * k / 6)) for k in range(7)]
    arch = [(0.17, 0.08)] + arch[1:] + [(-0.17, 0.08)]
    p.prism(arch, -D / 2 - 0.008, -D / 2, "Black", axis="z")
    # A name board over it, worn blank.
    p.box((0.3, 0.07, 0.008), (0.0, y1 + 0.09, -D / 2 - 0.004), "Cream")
    return [p]


# --- Cinder's iron tub ------------------------------------------------------------------------

def tub_iron():
    p = Part("static")
    L, D, y0, y1 = 1.5, 0.7, 0.12, 0.55
    for sz in (-1, 1):
        p.box((L, y1 - y0, 0.05), (0.0, (y0 + y1) / 2, sz * (D / 2 - 0.025)), "Cast",
              bevel=0.006, segs=1)
        z = sz * (D / 2 - 0.025)
        rod(p, (-L / 2 + 0.01, y1, z), (L / 2 - 0.01, y1, z), 0.032, "Rusty", segs=6)
    for sx in (-1, 1):
        p.box((0.05, y1 - y0 - 0.004, D - 0.1), (sx * (L / 2 - 0.025), (y0 + y1) / 2 - 0.002,
              0.0), "Cast", bevel=0.006, segs=1)
    p.box((L - 0.1, 0.04, D - 0.1), (0.0, y0 + 0.02, 0.0), "Cast", segs=1)
    # Black rainwater and soot standing in it.
    p.box((L - 0.104, 0.26, D - 0.104), (0.0, y0 + 0.17, 0.0), "Black")
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.sphere(1.0, (0, 0, 0), "Char", segs=6, rings=4,
                     xf=T(sx * (L / 2 - 0.12), 0.07, sz * (D / 2 - 0.1)) @ S(0.06, 0.07, 0.06))
    return [p]


# --- Kettle's coal bunker ---------------------------------------------------------------------

def coal_bunker():
    w = Wear(401)
    p = Part("static")
    hf, hb, d = 0.7, 0.9, 0.45
    p.prism([(-d, 0.0), (d, 0.0), (d, hb), (-d, hf)], -0.6, 0.6, "Planks", axis="x")
    for sx in (-1, 1):
        for sz, h in ((-1, hf), (1, hb)):
            board(p, (0.06, h + 0.02, 0.06), (sx * 0.6, (h + 0.02) / 2 - 0.02, sz * d),
                  "Timber", w, bevel=0.004)
    a = math.atan2(hb - hf, 2 * d)
    run = math.hypot(hb - hf, 2 * d) + 0.08
    off = 0.0125 / math.cos(a) + 0.03
    for sx in (-1, 1):
        p.box((0.62, 0.025, run), (0, 0, 0), "Boards",
              xf=T(sx * 0.31, (hf + hb) / 2 + off, 0.0) @ RX(-a) @ RZ(w.j(0.01)))
    # The hatch at the foot of the front, coal spilled out of it.
    p.box((0.36, 0.26, 0.016), (0.1, 0.15, -d - 0.008), "Boards", bevel=0.003, segs=1)
    for k, (x, z, s) in enumerate(((0.05, -0.5, 0.07), (0.16, -0.52, 0.06),
                                   (0.1, -0.6, 0.055), (-0.02, -0.58, 0.05),
                                   (0.22, -0.6, 0.045), (0.13, -0.68, 0.04))):
        p.sphere(1.0, (0, 0, 0), "Black", segs=5, rings=3,
                 xf=T(x, s * 0.5, z) @ S(s, s * 0.7, s))
    return [p]


for name, build in (("clothes_line", clothes_line), ("clothes_line_bare", clothes_line_bare),
                    ("garden_bed_cabbage", garden_bed_cabbage),
                    ("garden_bed_roots", garden_bed_roots),
                    ("bench_yard", bench_yard), ("bench_charred", bench_charred),
                    ("rain_barrel", rain_barrel), ("woodpile_lean", woodpile_lean),
                    ("washtub_stand", washtub_stand), ("chopping_block", chopping_block),
                    ("beehive", beehive), ("hen_coop", hen_coop), ("handcart", handcart),
                    ("hand_pump", hand_pump), ("dog_kennel", dog_kennel),
                    ("tub_iron", tub_iron), ("coal_bunker", coal_bunker)):
    prop(name, build)
