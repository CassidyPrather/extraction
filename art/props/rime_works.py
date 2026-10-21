"""Rime Fell's works: the ore mill's machinery, the observatory's
instruments and the funicular's running gear, for `ore_mill`,
`observatory`, `funicular_upper` and `funicular_car` in src/world/fell.rs.
Each model fills its machine's blockout there (the colliders stay), its
origin on the floor under the middle of the machine's footprint, the
mill's own axes (+Z up the hill), measures as fell.rs's:

- `mill_thickener`    the thickener: a round concrete tank, dried slurry
                      in it, the bridge across with its rake drive, the
                      rake's shaft down; 4.4 across, 3.1 to the rail
- `mill_flotation`    the flotation bank: six cells in a row along X, the
                      froth gone to crust, the launder along their -Z
                      side, each cell's impeller drive on top; the
                      operators' catwalk along +Z on posts with its rail,
                      and the ship's ladder up to it at the -X end;
                      10.8 x 2.3 x 2.6
- `mill_conditioner`  the conditioner tank, its agitator's motor and
                      gearbox on a beam over it; 2.0 across, 2.6 tall
- `mill_pump`         a slurry pump and its motor on a bedplate; 0.8 x
                      0.7 x 0.6
- `mill_ball_mill`    the ball mill lying along X on its concrete piers:
                      the drum's bolted shell and conical heads, the
                      trunnions in their bearings, the girth gear in its
                      guard and the pinion, the feed chute up to the
                      crusher floor; 6.4 x 5.6 x 3.5
- `mill_ore_bin`      the ore bin up on its four posts: plank sides on
                      timber studs, iron bands, the grizzly's bars over
                      it, the steel hopper under it narrowing to the
                      chute; 5.0 x 5.7 x 6.0
- `mill_jaw_crusher`  the jaw crusher under the hopper: its cast frame,
                      the swing jaw on its pitman, the eccentric shaft
                      and two flywheels; 2.0 x 1.7 x 1.8

The observatory's (`obs_*`) and the funicular's (`fun_*`) say where they
stand in their docstrings.

Materials: Painted (the company's green), Rusty, Cast, Iron, Steel,
Concrete, Timber, Planks, GreyGreen (slurry), Pale (froth), Rubber.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _kit import ALONG_X, UP, Wear, board, bolt, prop, rod  # noqa: E402
from common import Part, RX, RY, RZ, T  # noqa: E402

ALONG_Z = RX(0.0)


def ring_bolts(p, centre, axis, r, n, mat="Iron", br=0.014):
    """`n` bolt heads round a circle of radius `r` about `axis` ("x", "y"
    or "z") at `centre`, facing along the axis."""
    cx, cy, cz = centre
    for k in range(n):
        a = 2 * math.pi * k / n
        c, s = r * math.cos(a), r * math.sin(a)
        if axis == "x":
            at, ax = (cx, cy + c, cz + s), (1, 0, 0)
        elif axis == "y":
            at, ax = (cx + c, cy, cz + s), (0, 1, 0)
        else:
            at, ax = (cx + c, cy + s, cz), (0, 0, 1)
        bolt(p, at, ax, r=br, h=0.012, mat=mat)


def rail(p, pts, h, r=0.022, mat="Painted"):
    """A handrail at height `h` over a polyline of (x, y, z) floor points,
    and its posts at each point."""
    for (a, b) in zip(pts, pts[1:]):
        rod(p, (a[0], a[1] + h, a[2]), (b[0], b[1] + h, b[2]), r, mat, segs=6)
        rod(p, (a[0], a[1] + h * 0.5, a[2]), (b[0], b[1] + h * 0.5, b[2]), r * 0.8, mat,
            segs=6)
    for q in pts:
        rod(p, q, (q[0], q[1] + h, q[2]), r * 1.1, mat, segs=6)


def grating(p, x0, x1, z0, z1, y, mat="Iron", pitch=0.04):
    """A steel grating's bearing bars along X between edge angles: a deck
    0.03 deep whose top is at `y`."""
    n = max(2, int((z1 - z0) / pitch))
    for k in range(n + 1):
        z = z0 + (z1 - z0) * k / n
        p.box((x1 - x0, 0.03, 0.006), ((x0 + x1) / 2, y - 0.015, z), mat)
    for x in (x0, x1):
        p.box((0.05, 0.05, z1 - z0), (x, y - 0.025, (z0 + z1) / 2), mat)
    for z in (z0, z1):
        p.box((x1 - x0, 0.05, 0.05), ((x0 + x1) / 2, y - 0.025, z), mat)


def motor(p, at, r, length, axis="x", mat="Painted"):
    """A ribbed electric motor, its shaft along `axis`, its feet down to
    `at`'s y less `r`."""
    x, y, z = at
    xf = T(x, y, z) @ (ALONG_X if axis == "x" else ALONG_Z)
    p.cyl(r, -length / 2, length / 2, mat, segs=16, xf=xf, chamfer=0.01)
    for k in range(1, 6):
        u = -length / 2 + length * k / 6
        p.cyl(r + 0.012, u - 0.006, u + 0.006, mat, segs=16, xf=xf)
    p.cyl(r * 0.25, length / 2, length / 2 + 0.08, "Steel", segs=8, xf=xf)
    p.box((0.14, 0.1, 0.12), (x, y + r + 0.03, z), "Cast", bevel=0.01, segs=1)
    if axis == "x":
        p.box((length * 0.8, 0.04, r * 1.6), (x, y - r - 0.01, z), "Cast")
    else:
        p.box((r * 1.6, 0.04, length * 0.8), (x, y - r - 0.01, z), "Cast")


# --- The concentrate shed ---------------------------------------------------------------------

def thickener():
    p = Part("static")
    R, H = 2.2, 1.5
    # The tank: a wall 0.2 thick, a chamfered coping, the dried slurry a
    # finger under its rim, cracked into plates.
    p.tube(R, R - 0.2, 0.0, H, "Concrete", segs=40, chamfer=0.02, xf=UP)
    p.tube(R + 0.03, R - 0.22, H - 0.08, H, "Concrete", segs=40, chamfer=0.01, xf=UP)
    p.cyl(R - 0.15, 0.0, H + 0.01, "GreyGreen", segs=40, xf=UP)
    w = Wear(41)
    for k in range(9):
        a = 2 * math.pi * k / 9 + w.j(0.2)
        rr = w.j(0.5) + 1.0
        p.box((0.5 + w.j(0.15), 0.012, 0.4 + w.j(0.1)),
              (rr * math.cos(a), H + 0.012, rr * math.sin(a)), "GreyGreen", bevel=0.004,
              segs=1)
    # The overflow launder round the rim, and its spout toward -X.
    p.tube(R + 0.15, R + 0.03, H - 0.22, H - 0.04, "Rusty", segs=40, xf=UP)
    p.box((0.4, 0.16, 0.25), (-R - 0.3, H - 0.14, 0.0), "Rusty", bevel=0.005, segs=1)
    # The bridge: two channels across X on their seats on the rim, the
    # grating between, a rail each side.
    for x in (-R + 0.2, R - 0.2):
        p.box((0.3, 0.3, 0.6), (x, H + 0.15, 0.0), "Painted", bevel=0.01, segs=1)
    for z in (-0.22, 0.22):
        p.box((2 * R, 0.3, 0.06), (0.0, H + 0.45, z), "Painted", bevel=0.004, segs=1)
    grating(p, -R, R, -0.2, 0.2, H + 0.6)
    for z in (-0.24, 0.24):
        pts = [(x, H + 0.6, z) for x in (-R + 0.1, -0.6, 0.6, R - 0.1)]
        rail(p, pts, 1.0, r=0.02)
    # The drive in the middle: the gear housing, its motor on top, the
    # rake's shaft down through the bridge into the tank and its arms
    # under the crust.
    p.cyl(0.33, 0.0, 0.5, "Painted", segs=20, chamfer=0.02, xf=T(0, H + 0.6, 0) @ UP)
    p.box((0.7, 0.06, 0.7), (0.0, H + 0.63, 0.0), "Cast", bevel=0.01, segs=1)
    motor(p, (0.0, H + 1.28, 0.0), 0.14, 0.4, axis="z")
    rod(p, (0.0, H + 0.6, 0.0), (0.0, 0.3, 0.0), 0.05, "Steel", segs=10)
    for a in (0.3, 0.3 + math.pi):
        rod(p, (0.0, 0.35, 0.0), (1.8 * math.cos(a), 0.15, 1.8 * math.sin(a)), 0.035, "Rusty",
            segs=6)
    return [p]


# --- The flotation floor ----------------------------------------------------------------------

FL_X = (-7.4, 3.4)
FL_Z = (-6.6, -5.0)
FL_H = 1.3
FL_CELLS = 6


def flotation():
    p = Part("static")
    x0, x1 = (v - (FL_X[0] + FL_X[1]) / 2 for v in FL_X)
    z0, z1 = (v - (FL_Z[0] + FL_Z[1]) / 2 for v in FL_Z)
    wc = (x1 - x0) / FL_CELLS
    w = Wear(77)
    for i in range(FL_CELLS):
        a, b = x0 + wc * i, x0 + wc * (i + 1)
        cx = (a + b) / 2
        # The cell: a steel tank, its top lip in angle, the froth gone to
        # a grey crust with a few domes in it.
        p.box((wc - 0.03, FL_H - 0.02, z1 - z0), (cx, (FL_H - 0.02) / 2, 0.0), "Painted",
              bevel=0.01, segs=1)
        p.box((wc, 0.05, 0.06), (cx, FL_H - 0.025, z0 + 0.03), "Painted")
        p.box((wc, 0.05, 0.06), (cx, FL_H - 0.025, z1 - 0.03), "Painted")
        p.box((wc - 0.2, 0.012, z1 - z0 - 0.2), (cx, FL_H + 0.0, 0.0), "Pale", bevel=0.004,
              segs=1)
        for k in range(4):
            p.sphere(0.05 + abs(w.j(0.03)), (cx + w.j(0.6), FL_H, w.j(0.5)), "Pale", segs=8,
                     rings=4, scale=(1, 0.35, 1))
        # Its impeller drive: a stand, the motor upright on it, the belt
        # guard, the shaft down into the cell.
        p.box((0.4, 0.4, 0.4), (cx, FL_H + 0.2, 0.0), "Cast", bevel=0.02, segs=1)
        p.cyl(0.12, 0.0, 0.25, "Painted", segs=14, chamfer=0.01,
              xf=T(cx, FL_H + 0.4, 0.0) @ UP)
        p.box((0.42, 0.08, 0.16), (cx, FL_H + 0.53, 0.06), "Painted", bevel=0.02, segs=1)
        # The air valve on its pipe, the pipe along the bank's top.
        rod(p, (cx + 0.5, FL_H, 0.3), (cx + 0.5, FL_H + 0.35, 0.3), 0.025, "Iron", segs=6)
        p.cyl(0.06, -0.01, 0.01, "Red", segs=10, xf=T(cx + 0.5, FL_H + 0.35, 0.3) @ UP)
        # The seam between cells.
        if i > 0:
            p.box((0.06, FL_H + 0.04, z1 - z0 + 0.04), (a, (FL_H + 0.04) / 2, 0.0), "Cast")
        for y in (0.3, 0.9):
            for x in (a + 0.15, b - 0.15):
                bolt(p, (x, y, z0 - 0.002), (0, 0, -1), r=0.012, mat="Iron")
    rod(p, (x0, FL_H + 0.35, 0.3), (x1, FL_H + 0.35, 0.3), 0.035, "Iron", segs=8)
    # The launder: a rusted trough along -Z, falling toward +X.
    p.box((x1 - x0, 0.04, 0.3), (0.0, 0.97, z0 - 0.15), "Rusty")
    p.box((x1 - x0, 0.25, 0.03), (0.0, 1.075, z0 - 0.285), "Rusty")
    for x in (x0 + 0.4, 0.0, x1 - 0.4):
        p.box((0.05, 0.3, 0.32), (x, 0.82, z0 - 0.16), "Iron")
    # The catwalk along +Z: its grating, posts down to the floor, the rail
    # on its open side and the toe board.
    deck = FL_H
    wz0, wz1 = z1, z1 + 0.8
    grating(p, x0, x1, wz0, wz1, deck)
    rz = wz1 - 0.04
    for x in (-7.3, -4.6, -1.6, 0.8, 3.3):
        x -= (FL_X[0] + FL_X[1]) / 2
        rod(p, (x, 0.0, rz), (x, deck + 1.0, rz), 0.03, "Painted", segs=6)
        rod(p, (x, 0.0, wz0 + 0.05), (x, deck - 0.05, wz0 + 0.05), 0.03, "Painted", segs=6)
    for h in (0.5, 1.0):
        rod(p, (x0, deck + h - 0.02, rz), (x1, deck + h - 0.02, rz), 0.022, "Painted", segs=6)
    p.box((x1 - x0, 0.1, 0.01), (0.0, deck + 0.05, rz), "Painted")
    # The ship's ladder up at the -X end: two stringers and its treads.
    steps = 6
    rise = deck / steps
    foot = x0 - (steps - 1) * 0.25
    for z in (wz0 + 0.03, wz1 - 0.03):
        rod(p, (foot, 0.0, z), (x0, deck, z), 0.03, "Iron", segs=4)
    for j in range(1, steps):
        xe = x0 - (steps - j) * 0.25
        p.box((0.24, 0.025, wz1 - wz0 - 0.08), (xe + 0.125, rise * j - 0.0125,
                                                (wz0 + wz1) / 2), "Iron")
    rod(p, (foot + 0.05, 1.0, wz1 - 0.02), (x0, deck + 1.0, wz1 - 0.02), 0.022, "Painted",
        segs=6)
    return [p]


def conditioner():
    p = Part("static")
    R, H = 1.0, 2.0
    p.cyl(R, 0.0, H, "Painted", segs=32, chamfer=0.02, xf=UP)
    for y in (0.5, 1.0, 1.5):
        p.tube(R + 0.015, R - 0.01, y - 0.03, y + 0.03, "Painted", segs=32, xf=UP)
    for y in (0.5, 1.0, 1.5):
        ring_bolts(p, (0.0, y, 0.0), "y", R + 0.012, 24)
    p.tube(R + 0.03, R - 0.04, H - 0.05, H, "Cast", segs=32, xf=UP)
    # The beam across its top and the drive on it.
    p.box((2 * R + 0.1, 0.16, 0.2), (0.0, H + 0.08, 0.0), "Painted", bevel=0.005, segs=1)
    p.box((0.45, 0.4, 0.45), (0.0, H + 0.2, 0.0), "Cast", bevel=0.02, segs=1)
    motor(p, (0.0, H + 0.56, 0.0), 0.13, 0.36, axis="z")
    # The feed pipe in, the drain valve out at the foot.
    rod(p, (0.8, H + 0.35, -0.3), (0.5, H - 0.05, -0.3), 0.05, "Iron", segs=8)
    rod(p, (-R, 0.25, 0.0), (-R - 0.3, 0.25, 0.0), 0.06, "Iron", segs=8)
    p.cyl(0.09, -0.04, 0.04, "Cast", segs=10, xf=T(-R - 0.2, 0.25, 0.0) @ ALONG_X)
    return [p]


def pump():
    p = Part("static")
    p.box((0.8, 0.08, 0.6), (0.0, 0.04, 0.0), "Cast", bevel=0.01, segs=1)
    # The volute on the -X end, its suction eye and discharge up.
    p.cyl(0.26, -0.1, 0.1, "Painted", segs=20, chamfer=0.02, xf=T(-0.18, 0.36, 0.0) @ ALONG_X)
    p.cyl(0.09, -0.3, -0.1, "Painted", segs=12, xf=T(-0.18, 0.36, 0.0) @ ALONG_X)
    rod(p, (-0.18, 0.55, 0.15), (-0.18, 0.7, 0.15), 0.07, "Painted", segs=12)
    p.cyl(0.11, -0.015, 0.015, "Cast", segs=14, xf=T(-0.18, 0.69, 0.15) @ UP)
    ring_bolts(p, (-0.18, 0.705, 0.15), "y", 0.085, 6)
    p.box((0.12, 0.28, 0.25), (0.0, 0.22, 0.0), "Cast", bevel=0.01, segs=1)
    motor(p, (0.22, 0.27, 0.0), 0.16, 0.34, axis="x")
    p.box((0.14, 0.16, 0.2), (0.02, 0.32, 0.0), "Yellow", bevel=0.01, segs=1)
    return [p]


# --- The ball mill floor ----------------------------------------------------------------------

BM_R, BM_LEN, BM_Y = 1.4, 5.0, 1.9


def ball_mill():
    """Origin on the floor under the drum's middle (mill (-1.5, f, 4.2))."""
    p = Part("static")
    R, L, Y = BM_R, BM_LEN, BM_Y
    shell = T(0, Y, 0) @ ALONG_X
    # The drum: its shell plates, the flanged courses bolted between, the
    # conical heads at each end (inside the collider's length).
    p.cyl(R - 0.01, -L / 2 + 0.25, L / 2 - 0.25, "Painted", segs=40, xf=shell)
    for u in (-L / 2 + 0.25, L / 2 - 0.25):
        sgn = 1 if u > 0 else -1
        p.lathe([(0.0, u), (R, u), (R, u + sgn * 0.04), (0.55, u + sgn * 0.25),
                 (0.0, u + sgn * 0.25)], "Painted", segs=40, xf=shell)
    for k in range(1, 4):
        u = -L / 2 + L * k / 4
        p.cyl(R + 0.02, u - 0.04, u + 0.04, "Cast", segs=40, xf=shell)
        for j in range(24):
            a = 2 * math.pi * j / 24
            for du in (-0.06, 0.06):
                bolt(p, (u + du, Y + R * math.cos(a), R * math.sin(a)),
                     (0, math.cos(a), math.sin(a)), r=0.016, h=0.014)
    # Liner bolts in rows down the shell.
    for j in range(16):
        a = 2 * math.pi * (j + 0.5) / 16
        for k in range(8):
            u = -L / 2 + 0.45 + (L - 0.9) * (k + 0.5) / 8
            if any(abs(u - (-L / 2 + L * m / 4)) < 0.1 for m in (1, 2, 3)):
                continue
            bolt(p, (u, Y + R * math.cos(a), R * math.sin(a)), (0, math.cos(a), math.sin(a)),
                 r=0.018, h=0.012)
    # The manhole on its side.
    p.box((0.6, 0.06, 0.45), (0.0, R + 0.01, 0.0), "Cast", bevel=0.01, segs=1,
          xf=T(-0.6, Y, 0) @ RX(1.2))
    # The trunnions and their bearings on the piers.
    for sgn in (-1, 1):
        x = sgn * (L / 2 + 0.3)
        p.cyl(0.5, -0.3, 0.3, "Steel", segs=24, xf=T(x, Y, 0) @ ALONG_X)
        p.box((0.9, 0.08, 1.6), (x, Y - 0.54, 0.0), "Cast", bevel=0.01, segs=1)
        p.box((0.7, 0.46, 1.3), (x, Y - 0.28, 0.0), "Cast", bevel=0.03, segs=1)
        p.cyl(0.58, -0.25, 0.25, "Cast", segs=24, xf=T(x, Y, 0) @ ALONG_X)
        for z in (-0.5, 0.5):
            bolt(p, (x - 0.25, Y - 0.04, z), (0, 1, 0), r=0.03, h=0.05)
            bolt(p, (x + 0.25, Y - 0.04, z), (0, 1, 0), r=0.03, h=0.05)
        # The pier: board-marked concrete, chamfered.
        p.box((0.9, Y - 0.58, 1.6), (x, (Y - 0.58) / 2, 0.0), "Concrete", bevel=0.03, segs=1)
    # The girth gear by the +X end and its guard; the pinion under it.
    gx = L / 2 - 0.35
    gear = T(gx, Y, 0) @ ALONG_X
    p.cyl(1.68, -0.11, 0.11, "Rusty", segs=64, xf=gear)
    for k in range(96):
        a = 2 * math.pi * k / 96
        p.box((0.2, 0.06, 0.05), (0.0, 1.71, 0.0), "Rusty", xf=T(gx, Y, 0) @ RX(a))
    p.lathe([(1.78, -0.18), (1.82, -0.18), (1.82, 0.18), (1.78, 0.18)], "Painted", segs=40,
            closed=True, sweep=200.0, start=-10.0, xf=gear)
    p.cyl(0.25, -0.15, 0.15, "Steel", segs=20, xf=T(gx, 0.95, 1.6))
    # The feed chute down from the crusher floor onto the feed end's
    # trunnion: square, in plate, flanged where its lengths meet.
    fx, c0, c1 = -L / 2 - 0.3, Y + 0.45, 5.7
    p.box((0.56, c1 - c0, 0.56), (fx, (c0 + c1) / 2, 0.0), "Rusty", bevel=0.01, segs=1)
    for y in (c0 + 0.05, (c0 + c1) / 2, c1 - 0.3):
        p.box((0.64, 0.05, 0.64), (fx, y, 0.0), "Iron", bevel=0.005, segs=1)
    return [p]


# --- The crusher floor ------------------------------------------------------------------------

BIN_X, BIN_Z = 5.0, 6.0
BIN_LO, BIN_HI = 3.2, 5.6


def ore_bin():
    """Origin on the floor under the bin's middle (mill (-6.8, f, 15.5))."""
    p = Part("static")
    hx, hz = BIN_X / 2, BIN_Z / 2
    w = Wear(53)
    # The posts at its corners, braced, on pads.
    for x in (-hx + 0.15, hx - 0.15):
        for z in (-hz + 0.15, hz - 0.15):
            board(p, (0.3, BIN_LO, 0.3), (x, BIN_LO / 2, z), "Timber", wear=w, bevel=0.01)
            p.box((0.5, 0.08, 0.5), (x, 0.04, z), "Concrete", bevel=0.01, segs=1)
    for z in (-hz + 0.15, hz - 0.15):
        for sgn in (-1, 1):
            a, b = (sgn * (hx - 0.15), 0.3, z), (sgn * 0.6, BIN_LO - 0.3, z)
            rod(p, a, b, 0.07, "Timber", segs=4)
    # The bin: planks on studs, two iron bands, a cap on the top edge.
    for L, at, along in ((BIN_X, -hz, "x"), (BIN_X, hz, "x"),
                         (BIN_Z, -hx, "z"), (BIN_Z, hx, "z")):
        n = int((BIN_HI - BIN_LO) / 0.2)
        for k in range(n):
            y = BIN_LO + (k + 0.5) * (BIN_HI - BIN_LO) / n
            if along == "x":
                board(p, (L, 0.19, 0.06), (0.0, y, at - math.copysign(0.03, at)), "Planks",
                      wear=w, bevel=0.003)
            else:
                board(p, (0.06, 0.19, L), (at - math.copysign(0.03, at), y, 0.0), "Planks",
                      wear=w, bevel=0.003)
        for t in range(int(L / 1.25) + 1):
            u = -L / 2 + 0.1 + (L - 0.2) * t / int(L / 1.25)
            if along == "x":
                p.box((0.12, BIN_HI - BIN_LO, 0.08), (u, (BIN_LO + BIN_HI) / 2,
                                                      at - math.copysign(0.1, at)), "Timber")
            else:
                p.box((0.08, BIN_HI - BIN_LO, 0.12), (at - math.copysign(0.1, at),
                                                      (BIN_LO + BIN_HI) / 2, u), "Timber")
    for y in (BIN_LO + 0.5, BIN_HI - 0.5):
        p.box((BIN_X + 0.04, 0.12, 0.02), (0.0, y, -hz - 0.01), "Rusty")
        p.box((BIN_X + 0.04, 0.12, 0.02), (0.0, y, hz + 0.01), "Rusty")
        p.box((0.02, 0.12, BIN_Z + 0.04), (-hx - 0.01, y, 0.0), "Rusty")
        p.box((0.02, 0.12, BIN_Z + 0.04), (hx + 0.01, y, 0.0), "Rusty")
    # Ore in it, a little short of the grizzly; the grizzly's bars over.
    p.box((BIN_X - 0.2, 0.05, BIN_Z - 0.2), (0.0, BIN_HI - 0.4, 0.0), "Pebble")
    for k in range(7):
        x = -hx + 0.6 + k * 0.6
        p.box((0.08, 0.08, BIN_Z - 0.6), (x, BIN_HI + 0.04, 0.0), "Iron", bevel=0.005, segs=1)
    for z in (-hz + 0.35, hz - 0.35):
        p.box((BIN_X - 0.4, 0.1, 0.1), (0.0, BIN_HI + 0.03, z), "Iron")
    # The hopper under it, in steel plate on angle, narrowing to the
    # chute; the chute's gate and its lever.
    top_w, top_d, y_top = 3.4, 4.0, BIN_LO
    neck, y_neck = 0.8, BIN_LO - 1.2
    pts = [(-top_w / 2, y_top, -top_d / 2), (top_w / 2, y_top, -top_d / 2),
           (top_w / 2, y_top, top_d / 2), (-top_w / 2, y_top, top_d / 2)]
    p.loft([[(x, y_top, z) for (x, _, z) in pts],
            [(math.copysign(neck / 2, x), y_neck, math.copysign(neck / 2, z))
             for (x, _, z) in pts]], "Rusty", caps=False)
    p.box((top_w + 0.06, 0.06, top_d + 0.06), (0.0, y_top - 0.03, 0.0), "Iron")
    p.box((neck, 0.8, neck), (0.0, y_neck - 0.4, 0.0), "Rusty", bevel=0.01, segs=1)
    p.box((neck + 0.1, 0.05, neck + 0.1), (0.0, y_neck - 0.79, 0.0), "Iron")
    rod(p, (neck / 2 + 0.05, y_neck - 0.6, 0.0), (neck / 2 + 0.7, y_neck - 0.2, 0.0), 0.02,
        "Iron", segs=6)
    p.sphere(0.04, (neck / 2 + 0.7, y_neck - 0.2, 0.0), "Black", segs=8, rings=5)
    return [p]


def jaw_crusher():
    """Origin on the floor under its middle (mill (-6.8, f, 15.5)); the
    flywheels on the shaft at z 0.4, 0.95 up, 0.9 either side."""
    p = Part("static")
    # The frame: two cast cheeks with ribs, the fixed jaw between at the
    # front, the swing jaw hung behind it, the toggle and the tension rod.
    for x in (-0.6, 0.6):
        p.box((0.2, 1.2, 1.8), (x, 0.6, 0.0), "Painted", bevel=0.03, segs=1)
        for z in (-0.6, 0.0, 0.6):
            p.box((0.06, 1.0, 0.1), (x * 1.18, 0.55, z), "Painted", bevel=0.01, segs=1)
    p.box((1.0, 1.1, 0.25), (0.0, 0.6, -0.75), "Cast", bevel=0.02, segs=1)
    p.box((1.0, 0.9, 0.2), (0.0, 0.6, -0.2), "Cast", xf=T(0, 0.6, -0.2) @ RX(-0.25)
          @ T(0, -0.6, 0.2), bevel=0.01, segs=1)
    p.box((0.6, 0.5, 0.35), (0.0, 0.95, 0.4), "Cast", bevel=0.04, segs=1)
    p.box((1.4, 0.15, 1.8), (0.0, 0.075, 0.0), "Cast", bevel=0.01, segs=1)
    rod(p, (0.0, 0.35, 0.3), (0.0, 0.35, 0.85), 0.04, "Steel", segs=8)
    # The shaft and the two flywheels, spoked.
    rod(p, (-1.0, 0.95, 0.4), (1.0, 0.95, 0.4), 0.08, "Steel", segs=12)
    for x in (-0.9, 0.9):
        fw = T(x, 0.95, 0.4) @ ALONG_X
        p.tube(0.75, 0.62, -0.08, 0.08, "Cast", segs=36, xf=fw)
        p.cyl(0.14, -0.1, 0.1, "Cast", segs=14, xf=fw)
        for k in range(6):
            a = 2 * math.pi * k / 6
            rod(p, (x, 0.95 + 0.12 * math.cos(a), 0.4 + 0.12 * math.sin(a)),
                (x, 0.95 + 0.64 * math.cos(a), 0.4 + 0.64 * math.sin(a)), 0.035, "Cast",
                segs=5)
        p.box((0.06, 0.12, 0.08), (x, 0.95, 0.4), "Steel")
    # The feed opening's chute lip and the discharge under, spilled ore.
    w = Wear(19)
    for k in range(10):
        p.sphere(0.05 + abs(w.j(0.04)), (w.j(0.6), 0.04, -0.5 + w.j(0.25)), "Pebble", segs=6,
                 rings=4, scale=(1, 0.7, 1))
    return [p]


# --- The observatory --------------------------------------------------------------------------

DOME_R, DRUM_H = 2.0, 1.2


def obs_dome():
    """Origin on the roof deck under the dome's middle (observatory (2.6,
    6.5, 0.9)): the rendered drum, the rail ring it turns on, the dome's
    zinc skin on its ribs, the shutter shut over its slot (along Z)."""
    p = Part("static")
    R, H = DOME_R, DRUM_H
    p.cyl(R, 0.0, H, "Plaster", segs=48, xf=UP)
    p.tube(R + 0.06, R - 0.02, 0.0, 0.12, "Concrete", segs=48, chamfer=0.02, xf=UP)
    p.tube(R + 0.04, R - 0.03, H - 0.02, H + 0.08, "Iron", segs=48, xf=UP)
    # The skin, a hair inside the collider's steps, and a lip over the
    # rail.
    prof = [(0.0, H + 0.08 + R - 0.01)]
    for k in range(17):
        a = math.pi / 2 * (1 - k / 16)
        prof.append(((R - 0.01) * math.cos(a), H + 0.08 + (R - 0.01) * math.sin(a)))
    prof = list(reversed(prof))
    prof = [(R + 0.06, H + 0.02), (R + 0.06, H + 0.1)] + prof[1:] + [(0.0, H + 0.08 + R - 0.01)]
    p.lathe(prof, "Zinc", segs=48, xf=UP)
    # The ribs: a flat bar down each seam, sixteen of them, skipping the
    # slot.
    for k in range(16):
        yaw = 2 * math.pi * (k + 0.5) / 16
        if abs(math.cos(yaw)) < 0.2:
            continue
        pts = []
        for j in range(13):
            a = math.pi / 2 * j / 12
            r = R + 0.005
            pts.append((r * math.cos(a) * math.cos(yaw), H + 0.08 + r * math.sin(a),
                        r * math.cos(a) * math.sin(yaw)))
        for a, b in zip(pts, pts[1:]):
            rod(p, a, b, 0.018, "Zinc", segs=4)
    # The shutter: two leaves shut over the slot, on rails either side,
    # from the lip over the crown and down the far side to the drum.
    for side in (-1, 1):
        for j in range(24):
            a0 = math.pi * j / 24
            a1 = math.pi * (j + 1) / 24
            for off, w, mat, lift in ((0.26, 0.05, "Iron", 0.05), (0.12, 0.24, "Painted", 0.03)):
                x = side * off
                r = math.sqrt(max(0.0, (R + lift) ** 2 - x * x))
                q0 = (x, H + 0.08 + r * math.sin(a0), r * math.cos(a0))
                q1 = (x, H + 0.08 + r * math.sin(a1), r * math.cos(a1))
                if mat == "Painted":
                    mid = ((q0[0] + q1[0]) / 2, (q0[1] + q1[1]) / 2, (q0[2] + q1[2]) / 2)
                    ln = math.dist(q0, q1) + 0.01
                    am = (a0 + a1) / 2
                    p.box((w, 0.02, ln), (0.0, 0.0, 0.0), mat,
                          xf=T(*mid) @ RX(-(am - math.pi / 2)))
                else:
                    rod(p, q0, q1, 0.025, mat, segs=4)
    # The door into the drum from the deck, toward -Z (shut), a step.
    p.box((0.7, 1.0, 0.06), (0.0, 0.55, -R - 0.01), "Painted", bevel=0.005, segs=1)
    p.box((0.8, 0.12, 0.3), (0.0, 0.06, -R - 0.15), "Concrete", bevel=0.01, segs=1)
    p.sphere(0.025, (0.25, 0.55, -R - 0.05), "Brass", segs=8, rings=5)
    return [p]


def obs_screen():
    """Origin on the roof under the instrument screen (observatory (-0.2,
    6.5, -1.9)): a louvred Stevenson screen on its four legs, its door to
    -Z, a double roof; the wire cage round it."""
    p = Part("static")
    for lx in (-0.4, 0.4):
        for lz in (-0.25, 0.25):
            p.box((0.05, 1.1, 0.05), (lx, 0.55, lz), "Timber")
            p.box((0.12, 0.04, 0.12), (lx, 0.02, lz), "Concrete")
    for lz in (-0.25, 0.25):
        p.box((0.85, 0.04, 0.04), (0.0, 0.5, lz), "Timber")
    # The box: corner posts, a floor, louvres on every side angled down
    # and out.
    y0, y1 = 1.1, 1.7
    for cx in (-0.43, 0.43):
        for cz in (-0.28, 0.28):
            p.box((0.04, y1 - y0, 0.04), (cx, (y0 + y1) / 2, cz), "Plaster")
    p.box((0.9, 0.03, 0.6), (0.0, y0 + 0.015, 0.0), "Plaster")
    n = 8
    for k in range(n):
        y = y0 + 0.05 + (y1 - y0 - 0.08) * (k + 0.5) / n
        for z in (-0.29, 0.29):
            p.box((0.82, 0.055, 0.008), (0.0, 0.0, 0.0), "Plaster",
                  xf=T(0.0, y, z) @ RX(math.copysign(0.7, -z)))
        for x in (-0.44, 0.44):
            p.box((0.008, 0.055, 0.52), (0.0, 0.0, 0.0), "Plaster",
                  xf=T(x, y, 0.0) @ RZ(math.copysign(0.7, x)))
    # The door's frame and hinges on the front, its brass catch.
    p.box((0.84, 0.03, 0.02), (0.0, y0 + 0.03, -0.3), "Plaster")
    p.box((0.84, 0.03, 0.02), (0.0, y1 - 0.03, -0.3), "Plaster")
    for y in (y0 + 0.12, y1 - 0.12):
        p.box((0.04, 0.06, 0.01), (-0.42, y, -0.305), "Iron")
    p.box((0.03, 0.05, 0.02), (0.41, (y0 + y1) / 2, -0.31), "Brass")
    # The roofs: a slab, and a second over it on spacers to shade it.
    p.box((0.96, 0.03, 0.66), (0.0, y1 + 0.015, 0.0), "Plaster")
    for x in (-0.35, 0.35):
        for z in (-0.22, 0.22):
            p.box((0.04, 0.03, 0.04), (x, y1 + 0.045, z), "Plaster")
    p.box((1.0, 0.025, 0.7), (0.0, y1 + 0.072, 0.0), "Plaster", bevel=0.004, segs=1,
          xf=T(0, y1 + 0.072, 0) @ RX(0.06) @ T(0, -(y1 + 0.072), 0))
    # The cage: angle posts, top and middle rails, wire mesh on the four
    # sides (a coarse grid), its gate toward -Z shut.
    cw, cd, ch = 1.6, 1.3, 2.0
    for sx in (-1, 1):
        for sz in (-1, 1):
            p.box((0.04, ch, 0.04), (sx * cw / 2, ch / 2, sz * cd / 2), "Iron")
    for y in (0.05, 1.0, ch - 0.02):
        for sz in (-1, 1):
            rod(p, (-cw / 2, y, sz * cd / 2), (cw / 2, y, sz * cd / 2), 0.012, "Iron", segs=4)
        for sx in (-1, 1):
            rod(p, (sx * cw / 2, y, -cd / 2), (sx * cw / 2, y, cd / 2), 0.012, "Iron", segs=4)
    for k in range(1, 11):
        x = -cw / 2 + cw * k / 11
        for sz in (-1, 1):
            rod(p, (x, 0.05, sz * cd / 2), (x, ch - 0.02, sz * cd / 2), 0.003, "Zinc", segs=3)
    for k in range(1, 9):
        z = -cd / 2 + cd * k / 9
        for sx in (-1, 1):
            rod(p, (sx * cw / 2, 0.05, z), (sx * cw / 2, ch - 0.02, z), 0.003, "Zinc", segs=3)
    for k in range(1, 14):
        y = 0.05 + (ch - 0.07) * k / 14
        for sz in (-1, 1):
            rod(p, (-cw / 2, y, sz * cd / 2), (cw / 2, y, sz * cd / 2), 0.003, "Zinc", segs=3)
        for sx in (-1, 1):
            rod(p, (sx * cw / 2, y, -cd / 2), (sx * cw / 2, y, cd / 2), 0.003, "Zinc", segs=3)
    return [p]


def obs_anemometer():
    """The anemometer's head, origin on the mast's axis at its top
    (observatory (1.6, 12.0, 3.6)): three cups on their arms round the
    spindle, the vane on its arm under them."""
    p = Part("static")
    p.cyl(0.06, -0.06, 0.0, "Painted", segs=12, xf=UP)
    p.cyl(0.035, 0.0, 0.22, "Brass", segs=10, xf=UP)
    p.cyl(0.06, 0.0, 0.04, "Brass", segs=12, xf=T(0, 0.08, 0) @ UP)
    for k in range(3):
        a = 2 * math.pi * k / 3
        d = (math.cos(a), math.sin(a))
        tip = (0.45 * d[0], 0.1, 0.45 * d[1])
        rod(p, (0.0, 0.1, 0.0), tip, 0.008, "Brass", segs=5)
        # A cup, its mouth facing round the circle.
        tang = (-d[1], d[0])
        yaw = math.atan2(tang[0], tang[1])
        p.lathe([(0.0, 0.0), (0.05, 0.015), (0.068, 0.045), (0.07, 0.06), (0.066, 0.06),
                 (0.063, 0.048), (0.045, 0.02), (0.0, 0.006)], "Brass", segs=14,
                xf=T(*tip) @ RY(yaw) @ T(0, 0, -0.03))
    # The vane: an arm on the mast under the cups, the fin at its tail and
    # the pointer at its head.
    p.cyl(0.03, -0.56, -0.44, "Painted", segs=10, xf=UP)
    rod(p, (0.25, -0.5, 0.0), (-0.65, -0.5, 0.0), 0.012, "Painted", segs=6)
    p.prism([(-0.15, -0.11), (0.15, -0.07), (0.15, 0.07), (-0.15, 0.11)], -0.01, 0.01,
            "Painted", axis="z", xf=T(-0.62, -0.5, 0.0))
    p.cyl(0.03, 0.0, 0.08, "Painted", segs=8, xf=T(0.25, -0.5, 0.0) @ ALONG_X)
    return [p]


def obs_radio():
    """The radio desk's set, origin on the table top under its middle, its
    front to -Z (the set 0.9 x 0.45 x 0.35): the receiver and the
    transmitter side by side in their grey steel cases, dials and a meter,
    the morse key and the headphones in front of it, the log."""
    p = Part("static")
    for x, w in ((-0.23, 0.44), (0.23, 0.44)):
        p.box((w, 0.45, 0.35), (x, 0.225, 0.0), "GreyGreen", bevel=0.01, segs=1)
        p.box((w - 0.04, 0.41, 0.01), (x, 0.225, -0.176), "Dark", bevel=0.003, segs=1)
        for (dx, dy) in ((-0.12, 0.07), (0.0, 0.07), (0.12, 0.07)):
            p.cyl(0.022, -0.03, 0.0, "Black", segs=12, xf=T(x + dx, dy + 0.0, -0.18))
        for (dx, dy) in ((-0.12, 0.2), (0.12, 0.2)):
            p.cyl(0.014, -0.02, 0.0, "Bakelite", segs=8, xf=T(x + dx, dy, -0.18))
        for dx in (-0.19, 0.19):
            for dy in (0.03, 0.42):
                bolt(p, (x + dx, dy, -0.182), (0, 0, -1), r=0.008, mat="Steel")
    # The receiver's tuning dial (lit glass) and the transmitter's meter.
    p.box((0.3, 0.12, 0.006), (-0.23, 0.32, -0.183), "Cream", bevel=0.004, segs=1)
    rod(p, (-0.3, 0.27, -0.188), (-0.28, 0.37, -0.188), 0.002, "Red", segs=3)
    p.cyl(0.07, -0.01, 0.0, "Cream", segs=20, xf=T(0.23, 0.31, -0.18))
    p.cyl(0.075, -0.012, -0.005, "Black", segs=20, xf=T(0.23, 0.31, -0.18))
    rod(p, (0.23, 0.27, -0.192), (0.27, 0.34, -0.192), 0.002, "Black", segs=3)
    # Vents along their tops, and the cable to the mast out of the back.
    for k in range(6):
        p.box((0.32, 0.004, 0.012), (0.0, 0.452, -0.1 + k * 0.04), "Dark")
    rod(p, (0.35, 0.3, 0.18), (0.35, 0.05, 0.42), 0.01, "Black", segs=5)
    # The morse key on its base, the headphones by it, the logbook open.
    p.box((0.08, 0.015, 0.16), (0.15, 0.0075, -0.3), "Bakelite", bevel=0.003, segs=1)
    rod(p, (0.15, 0.025, -0.36), (0.15, 0.03, -0.24), 0.006, "Brass", segs=5)
    p.cyl(0.012, 0.0, 0.015, "Black", segs=8, xf=T(0.15, 0.032, -0.37) @ UP)
    for x in (-0.28, -0.16):
        p.cyl(0.04, 0.0, 0.03, "Black", segs=14, xf=T(x, 0.015, -0.33) @ UP)
    p.lathe([(0.06, -0.003), (0.066, -0.003), (0.066, 0.003), (0.06, 0.003)], "Leather",
            segs=10, closed=True, sweep=180.0, xf=T(-0.22, 0.03, -0.33) @ RX(-0.2))
    p.box((0.21, 0.006, 0.16), (-0.05, 0.003, -0.32), "Leather")
    p.box((0.2, 0.01, 0.15), (-0.05, 0.011, -0.32), "Paper", bevel=0.002, segs=1)
    return [p]


def obs_barometer():
    """An aneroid barometer and a thermometer on their board, for the wall:
    the wall's face at z 0, origin at the dial's middle."""
    p = Part("static")
    p.box((0.42, 0.62, 0.02), (0.05, -0.08, -0.01), "Wood", bevel=0.005, segs=1)
    p.cyl(0.16, -0.06, -0.02, "Brass", segs=28, chamfer=0.006)
    p.cyl(0.14, -0.063, -0.06, "Cream", segs=28)
    for k in range(25):
        a = math.radians(-135 + 270 * k / 24)
        r0 = 0.115 if k % 4 else 0.1
        rod(p, (r0 * math.sin(a), r0 * math.cos(a), -0.064),
            (0.13 * math.sin(a), 0.13 * math.cos(a), -0.064), 0.0015, "Black", segs=3)
    rod(p, (0.0, 0.0, -0.066), (0.1 * math.sin(-0.5), 0.1 * math.cos(-0.5), -0.066), 0.003,
        "Black", segs=3)
    rod(p, (0.0, 0.0, -0.068), (0.09 * math.sin(0.4), 0.09 * math.cos(0.4), -0.068), 0.002,
        "Brass", segs=3)
    p.cyl(0.145, -0.07, -0.064, "Glass", segs=28)
    # The thermometer beside it, below.
    p.box((0.05, 0.3, 0.012), (0.2, -0.22, -0.026), "Cream", bevel=0.003, segs=1)
    rod(p, (0.2, -0.35, -0.034), (0.2, -0.1, -0.034), 0.003, "Glass", segs=5)
    rod(p, (0.2, -0.35, -0.035), (0.2, -0.22, -0.035), 0.0025, "Red", segs=5)
    p.sphere(0.008, (0.2, -0.355, -0.034), "Red", segs=8, rings=5)
    return [p]


def obs_bunk():
    """The double bunk's upper berth and its frame, origin on the floor
    under its middle (the lower berth is the room's bed), its long side
    along X: four posts, the upper's frame, mattress, a folded blanket,
    rungs up its foot."""
    p = Part("static")
    for x in (-0.95, 0.95):
        for z in (0.45, -0.45):
            p.box((0.06, 1.75, 0.06), (x, 0.875, z), "Timber", bevel=0.004, segs=1)
    p.box((1.95, 0.08, 0.06), (0.0, 1.35, 0.45), "Timber")
    p.box((1.95, 0.08, 0.06), (0.0, 1.35, -0.45), "Timber")
    p.box((0.06, 0.08, 0.9), (-0.95, 1.35, 0.0), "Timber")
    p.box((0.06, 0.08, 0.9), (0.95, 1.35, 0.0), "Timber")
    for k in range(7):
        x = -0.8 + k * 1.6 / 6
        p.box((0.08, 0.02, 0.84), (x, 1.32, 0.0), "Planks")
    p.box((1.85, 0.12, 0.84), (0.0, 1.45, 0.0), "Ticking", bevel=0.03, segs=2)
    p.box((0.5, 0.06, 0.8), (0.6, 1.54, 0.0), "DarkWool", bevel=0.02, segs=2)
    p.box((0.3, 0.08, 0.5), (-0.7, 1.55, 0.0), "Cream", bevel=0.03, segs=2)
    # The rail along its open side, rungs up its foot (+X).
    p.box((1.2, 0.05, 0.04), (-0.2, 1.65, -0.45), "Timber")
    for y in (0.35, 0.7, 1.05):
        rod(p, (0.95, y, 0.45), (0.95, y, -0.45), 0.018, "Timber", segs=6)
    return [p]


# --- The funicular ----------------------------------------------------------------------------

def fun_bull_wheel():
    """The top station's bull wheel, origin on the floor under the wheel's
    middle (station (-0.7, 0.15, -4.0)), its axle along X 4.85 up: the
    grooved rim of 2.4 m on six spokes, the rope in its groove round the
    bottom and away toward -X, the hub on its shaft, the shaft's plummer
    blocks on their concrete pedestals at x = +-0.9, and the chain case at
    x 1.95 down to the gearbox."""
    p = Part("static")
    Y, R = 4.85, 2.4
    ax = T(0, Y, 0) @ ALONG_X
    # The rim: a cast ring with a groove for the rope, flanged.
    p.lathe([(R - 0.11, -0.12), (R + 0.11, -0.12), (R + 0.11, -0.06), (R + 0.02, -0.03),
             (R + 0.02, 0.03), (R + 0.11, 0.06), (R + 0.11, 0.12), (R - 0.11, 0.12)],
            "Rusty", segs=64, closed=True, xf=ax)
    for k in range(20):
        a = 2 * math.pi * k / 20
        for sx in (-0.125, 0.125):
            bolt(p, (sx, Y + (R - 0.05) * math.sin(a), (R - 0.05) * math.cos(a)),
                 (math.copysign(1, sx), 0, 0), r=0.018, h=0.012)
    # The spokes: tapered I-sections from the hub to the rim.
    for k in range(6):
        a = 2 * math.pi * k / 6
        d = (math.sin(a), math.cos(a))
        for w0, t0, mat in ((0.2, 0.05, "Painted"), (0.06, 0.24, "Painted")):
            n = 6
            for j in range(n):
                r0 = 0.35 + (R - 0.45) * j / n
                r1 = 0.35 + (R - 0.45) * (j + 1) / n
                taper = 1.0 - 0.35 * (j + 0.5) / n
                mid = (r0 + r1) / 2
                p.box((t0, w0 * taper, r1 - r0 + 0.005), (0.0, 0.0, 0.0), mat,
                      xf=T(0.0, Y + mid * d[0], mid * d[1]) @ RX(-a))
    # The hub, the shaft through it, its keys and the plummer blocks.
    p.cyl(0.4, -0.25, 0.25, "Cast", segs=28, chamfer=0.03, xf=ax)
    ring_bolts(p, (0.26, Y, 0.0), "x", 0.3, 8)
    ring_bolts(p, (-0.26, Y, 0.0), "x", 0.3, 8)
    p.cyl(0.12, -1.6, 2.3, "Steel", segs=16, xf=ax)
    for x in (-0.9, 0.9):
        p.box((0.7, 4.73, 1.1), (x, 4.73 / 2, 0.0), "Concrete", bevel=0.03, segs=1)
        p.box((0.36, 0.22, 0.6), (x, Y - 0.12 - 0.0, 0.0), "Cast", bevel=0.02, segs=1)
        p.cyl(0.19, -0.16, 0.16, "Cast", segs=16, xf=T(x, Y, 0) @ ALONG_X)
        for z in (-0.24, 0.24):
            bolt(p, (x, Y - 0.0, z), (0, 1, 0), r=0.025, h=0.04)
        p.box((0.74, 0.06, 1.14), (x, 4.7, 0.0), "Concrete", bevel=0.01, segs=1)
    # The rope in the groove round the wheel's foot, where it leaves for
    # the line.
    p.lathe([(R + 0.02, -0.05), (R + 0.07, -0.05), (R + 0.07, 0.0), (R + 0.02, 0.0)], "Iron",
            segs=40, closed=True, sweep=200.0, start=80.0, xf=ax @ T(0, 0, 0.025))
    # The chain case: a steel box from the shaft down to the gearbox,
    # bolted at its seam, and its oil filler.
    cx = 1.95
    p.box((0.3, 5.15 - 0.8, 0.6), (cx, (0.8 + 5.15) / 2, 0.0), "Painted", bevel=0.02, segs=1)
    p.cyl(0.32, -0.16, 0.16, "Painted", segs=20, xf=T(cx, Y, 0) @ ALONG_X)
    for y in (1.2, 2.2, 3.2, 4.2):
        bolt(p, (cx + 0.155, y, 0.27), (1, 0, 0), r=0.014)
        bolt(p, (cx + 0.155, y, -0.27), (1, 0, 0), r=0.014)
    p.cyl(0.04, 0.0, 0.08, "Brass", segs=8, xf=T(cx, 5.0, 0.2) @ UP)
    return [p]


def fun_rope_reel():
    """The spare rope on its reel, lying on its flanges, origin on the
    floor under it (station (3.6, 0.15, 4.6)), its axle along X 0.9 up."""
    p = Part("static")
    ax = T(0, 0.9, 0) @ ALONG_X
    for x in (-0.5, 0.5):
        p.cyl(0.9, x - 0.04, x + 0.04, "Timber", segs=28, xf=ax)
        for k in range(8):
            a = 2 * math.pi * k / 8
            p.box((0.012, 0.08, 1.6), (x + math.copysign(0.046, x), 0.9, 0.0), "Iron",
                  xf=T(0, 0.9, 0) @ RX(a) @ T(0, -0.9, 0))
        p.cyl(0.12, x - 0.06, x + 0.06, "Iron", segs=12, xf=ax)
    # The rope wound on it, course on course.
    w = Wear(23)
    for k in range(12):
        x = -0.44 + 0.88 * (k + 0.5) / 12
        p.tube(0.7, 0.4, x - 0.035, x + 0.035, "Iron", segs=28, xf=ax @ RZ(w.j(0.3)))
    p.cyl(0.4, -0.46, 0.46, "Timber", segs=16, xf=ax)
    # The rope's end lashed down to the flange, its chocks.
    rod(p, (0.3, 1.6, -0.1), (0.46, 1.75, -0.3), 0.035, "Iron", segs=6)
    for z in (-0.75, 0.75):
        p.prism([(-0.15, 0.0), (0.15, 0.0), (0.0, 0.2)], -0.6, 0.6, "Timber", axis="x",
                xf=T(0, 0, z))
    return [p]


# The car on the grade: everything in the incline's own frame (y up from
# the rail heads along its normal, u along it, -u up the hill), placed by
# a turn of atan(GRADE) about X. Pieces fell.rs poses with `on_grade(x, y,
# z)` (z the world's) sit at u = (z - y sin a) / cos a.
GRADE = 0.20
GA = math.atan(GRADE)
CAR_LEN, CAR_HW, CAR_DECK = 7.64, 1.3, 0.9


def gu(y, z):
    return (z - y * math.sin(GA)) / math.cos(GA)


def fun_car_frame():
    """The funicular car's running gear and underframe, burnt with the
    body: the sole bars, the deck, the headstocks, two axles of flanged
    wheels in their axle boxes on leaf springs, the rail brake's jaws, the
    haul rope's socket at the top end with the rope's last metres."""
    p = Part("static")
    for x in (-0.8, 0.8):
        u = gu(0.65, 0.0)
        L = CAR_LEN - 0.1
        p.box((0.12, 0.3, L), (x, 0.65, u), "Rusty", bevel=0.005, segs=1)
        p.box((0.05, 0.26, L - 0.02), (x - math.copysign(0.04, x), 0.65, u), "Char")
        for k in range(14):
            bolt(p, (x + math.copysign(0.062, x), 0.72, u - L / 2 + 0.2 + k * (L - 0.4) / 13),
                 (math.copysign(1, x), 0, 0), r=0.012)
    p.box((2 * CAR_HW, 0.1, CAR_LEN), (0.0, CAR_DECK - 0.05, gu(CAR_DECK - 0.05, 0.0)), "Char")
    for k in range(9):
        u = gu(0.75, 0.0) - CAR_LEN / 2 + 0.3 + k * (CAR_LEN - 0.6) / 8
        p.box((1.48, 0.12, 0.08), (0.0, 0.76, u), "Rusty")
    for z in (-CAR_LEN / 2 + 0.08, CAR_LEN / 2 - 0.08):
        u = gu(0.65, z)
        p.box((2 * CAR_HW, 0.3, 0.16), (0.0, 0.65, u), "Painted", bevel=0.01, segs=1)
        for x in (-0.9, 0.9):
            p.cyl(0.09, 0.0, 0.12, "Cast", segs=12,
                  xf=T(x, 0.62, u + math.copysign(0.08, z)) @ RX(0.0 if z > 0 else math.pi))
    for z in (-2.6, 2.6):
        u = gu(0.32, z)
        axle = T(0, 0.32, u) @ ALONG_X
        p.cyl(0.05, -0.75, 0.75, "Steel", segs=10, xf=axle)
        for x in (-0.6, 0.6):
            s = math.copysign(1, x)
            wf = T(x, 0.32, u) @ ALONG_X
            # The wheel: a disc, its tread and the flange inboard.
            p.lathe([(0.06, -0.05 * s), (0.28, -0.05 * s), (0.28, 0.05 * s), (0.06, 0.05 * s)],
                    "Rusty", segs=28, closed=True, xf=wf)
            p.lathe([(0.27, -0.05 * s - 0.0), (0.33, -0.05 * s), (0.33, -0.03 * s),
                     (0.27, -0.03 * s)], "Rusty", segs=28, closed=True, xf=wf)
            p.cyl(0.12, -0.06, 0.06, "Cast", segs=12, xf=wf)
            # The axle box and its spring hung off the sole bar.
            p.box((0.16, 0.24, 0.32), (s * 0.8, 0.42, u), "Cast", bevel=0.015, segs=1)
            for k in range(4):
                y = 0.56 + 0.025 * k
                half = 0.45 - 0.08 * k
                p.box((0.08, 0.02, 2 * half), (s * 0.8, y, u), "Rusty")
            # The rail brake: jaws either side of the rail head behind the
            # wheel, on their arm.
            dz = math.copysign(0.42, z)
            p.box((0.22, 0.18, 0.2), (x, 0.08, u + dz), "Cast", bevel=0.01, segs=1)
            rod(p, (x, 0.18, u + dz), (s * 0.8, 0.5, u + dz * 0.6), 0.025, "Iron", segs=5)
    # The haul rope's socket at the top end, its thimble and the rope away
    # up the line, slack where it parted from nothing.
    u = gu(0.55, -CAR_LEN / 2 - 0.05)
    p.box((0.3, 0.25, 0.3), (0.0, 0.55, u), "Cast", bevel=0.02, segs=1)
    p.lathe([(0.06, -0.02), (0.1, -0.02), (0.1, 0.02), (0.06, 0.02)], "Steel", segs=16,
            closed=True, xf=T(0, 0.55, u - 0.25) @ RY(math.pi / 2))
    pts = [(0.0, 0.55, u - 0.35), (0.0, 0.3, u - 1.2), (0.05, 0.08, u - 2.4),
           (0.1, 0.03, u - 4.0)]
    for a, b in zip(pts, pts[1:]):
        rod(p, a, b, 0.025, "Iron", segs=6)
    return [p]


for name, build in (("mill_thickener", thickener), ("mill_flotation", flotation),
                    ("mill_conditioner", conditioner), ("mill_pump", pump),
                    ("mill_ball_mill", ball_mill), ("mill_ore_bin", ore_bin),
                    ("mill_jaw_crusher", jaw_crusher), ("obs_dome", obs_dome),
                    ("obs_screen", obs_screen), ("obs_anemometer", obs_anemometer),
                    ("obs_radio", obs_radio), ("obs_barometer", obs_barometer),
                    ("obs_bunk", obs_bunk), ("fun_bull_wheel", fun_bull_wheel),
                    ("fun_rope_reel", fun_rope_reel), ("fun_car_frame", fun_car_frame)):
    prop(name, build)
