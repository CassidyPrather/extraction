"""The machines' kit in Underground's Depot: the Custodian's sorting hall,
made, not scavenged -- white shell paint and light grey steel, black
stencil plates, orange tags. Writes assets/models/props/depot_<id>.glb.

Prop space: metres, Y up, origin on the floor under the centre, the front
(the face you walk up to) towards -Z.

- `depot_bin`        a wire bin, 1.2 x 0.9 x 0.8, open; its `lid` node is
                     hinged on the back top edge and modelled open (leaning
                     back 100 degrees: -100 degrees about local X closes
                     it). pt_contents_0..3 on the floor where items lie;
                     pt_label on the lid's plate (local -Z out of the plate,
                     Y up it) where the game writes the bin's stencil
- `depot_conveyor`   a roller conveyor 6.0 x 1.0 x 0.9 running along X;
                     each `roller_N` turns about its local Z (its axle);
                     pt_feed at the -X end, pt_drop at the +X end, on top
- `depot_tagger`     the tagging station, 0.7 x 1.3 x 0.6: a cabinet with
                     a slanted console, a glowing screen and the tag slot
                     (pt_slot, where a printed tag comes out)
- `depot_crate_long` a kept crate, 2.1 x 0.65 x 0.7, its `lid` (hinged on
                     the back top edge, shut) strapped and buckled;
                     pt_tag on the front, pt_top on the lid
- `depot_crate_small` the same family, 0.8 x 0.5 x 0.6: `lid`, pt_tag, and
                     pt_disc on the wire loop a collar disc hangs from
- `depot_crate_long_open`, `depot_crate_small_open`  the same crates opened:
                     hollow (zinc inside, the floor 6 cm up) with the lid
                     swung back 105 degrees on its hinge; pt_top is the
                     floor inside
- `depot_disc`       that brass disc, 0.04 across, hole at the top
                     (origin at its centre)
- `depot_intake_facade` a Custodian front for the kiosk at the ramp,
                     3.0 x 3.4 x 0.5, a 1.2 x 2.2 opening for its door: a
                     `shutter` node rolled up under its housing (origin at
                     the housing's bottom edge: scale its Y up to 22x to
                     close it), hazard stripes, pt_label over the opening
- `depot_sign`       a wall plate 0.9 x 0.4 x 0.03, origin at its centre
                     (its back on the wall at z +0.015): a blank face in a
                     black frame, pt_label at the face's centre
- `depot_cradle`     a walker's charging cradle, 2.2 x 2.6 x 2.0, its back
                     (+Z) to a wall: a plinth, two uprights and a gantry
                     with the charging arm, a cable to the wall; pt_dock
                     where the walker stands

Bins and crates stand 10-20 to a view, so they're kept under 600 and 400
tris; the stencils are plates the game writes on.

Materials: Shell, Zinc, Steel, Black, Orange, Rubber, Glow, Brass.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (RX, RY, RZ, T, Part, empty, export, fillet, make, report,  # noqa: E402
                    reset, section_circle)
from _kit import text  # noqa: E402

UP = RX(-math.pi / 2)        # cyl/lathe axis (Z) -> +Y
ALONG_X = RY(math.pi / 2)    # cyl/lathe axis (Z) -> +X


def wanted(name):
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    return not argv or name in argv


def write(name, parts, points):
    """Make the Parts, add the named points ((x, y, z) or ((x, y, z), rot)),
    and write models/props/<name>.glb."""
    for p in parts:
        make(p)
    for k, v in points.items():
        if isinstance(v[0], tuple):
            empty(k, v[0], v[1])
        else:
            empty(k, v)
    print(f"{name}:")
    report()
    export(f"assets/models/props/{name}.glb")


def wire(p, a, b, r=0.004, mat="Zinc"):
    """A straight wire from a to b: a three-sided rod (8 tris)."""
    ax, ay, az = a
    bx, by, bz = b
    dx, dy, dz = bx - ax, by - ay, bz - az
    L = math.sqrt(dx * dx + dy * dy + dz * dz)
    yaw = math.atan2(dx, dz)
    pitch = -math.asin(dy / L)
    p.cyl(r, 0.0, L, mat, segs=3, xf=T(ax, ay, az) @ RY(yaw) @ RX(pitch))


def bar(p, size, centre, mat="Shell"):
    """An unbevelled box (12 tris)."""
    p.box(size, centre, mat, segs=1)


# --- Bin ---------------------------------------------------------------------------------

def depot_bin():
    reset()
    W, H, D = 1.2, 0.9, 0.8
    b = Part("static_bin")
    f = 0.03
    # Corner posts, the top rim, a sheet floor on skids.
    for sx in (-1, 1):
        for sz in (-1, 1):
            bar(b, (f, H, f), (sx * (W - f) / 2, H / 2, sz * (D - f) / 2))
    y = H - f / 2
    for sz in (-1, 1):
        bar(b, (W - 2 * f, f, f), (0.0, y, sz * (D - f) / 2))
    for sx in (-1, 1):
        bar(b, (f, f, D - 2 * f), (sx * (W - f) / 2, y, 0.0))
    bar(b, (W - 2 * f, 0.008, D - 2 * f), (0.0, 0.06, 0.0), "Shell")
    for sz in (-1, 1):
        bar(b, (W, 0.035, 0.06), (0.0, 0.0175, sz * 0.3), "Steel")
    # The mesh: upright wires every 0.15 and two rows across.
    y0, y1 = 0.065, H - f
    xs = [-0.45 + 0.15 * k for k in range(7)]
    zs = [-0.3 + 0.15 * k for k in range(5)]
    for sz in (-1, 1):
        z = sz * (D - f) / 2
        for x in xs:
            wire(b, (x, y0, z), (x, y1, z))
    for sx in (-1, 1):
        x = sx * (W - f) / 2
        for z in zs:
            wire(b, (x, y0, z), (x, y1, z))
    for y in (0.33, 0.61):
        for sz in (-1, 1):
            z = sz * (D - f) / 2
            wire(b, (-(W - f) / 2, y, z), ((W - f) / 2, y, z))
        for sx in (-1, 1):
            x = sx * (W - f) / 2
            wire(b, (x, y, -(D - f) / 2), (x, y, (D - f) / 2))
    # An orange tag wired to the front rim.
    b.box((0.07, 0.1, 0.004), (0.0, 0.0, 0.0), "Orange", segs=1,
          xf=T(0.42, H - 0.09, -D / 2 - 0.006) @ RZ(0.12))

    # The lid: a white sheet with a black stencil plate, hinged on the back
    # top edge and raised (100 degrees, leaning back).
    hinge = (0.0, H, D / 2)
    lid = Part("lid", origin=hinge)
    OPEN = math.radians(100)
    tilt = RX(OPEN, about=hinge)
    lid.box((W + 0.01, 0.02, D), (0.0, H + 0.012, D / 2 - D / 2), "Shell", segs=1,
            xf=tilt)
    lid.box((0.7, 0.003, 0.32), (0.0, H + 0.0235, 0.02), "Black", segs=1, xf=tilt)
    for sx in (-1, 1):
        bar(lid, (0.08, 0.03, 0.03), (sx * 0.4, H + 0.005, D / 2 - 0.01), "Steel")
    # The label's centre on the plate, facing out of it.
    lab = tilt @ T(0.0, H + 0.025, 0.02)
    label_at = tuple(lab.to_translation())
    write("depot_bin", [b, lid], {
        **{f"pt_contents_{k}": (sx * 0.28, 0.065, sz * 0.18)
           for k, (sx, sz) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1)))},
        # The plate's outward normal is the lid's +Y: turned by the open
        # angle, it faces the front; the point's -Z is that normal.
        "pt_label": (label_at, RX(OPEN - math.pi / 2)),
    })


# --- Conveyor -----------------------------------------------------------------------------

def depot_conveyor():
    reset()
    L, W, H = 6.0, 1.0, 0.9
    R = 0.04
    yc = H - R
    s = Part("static_conveyor")
    # Side channels along the length, legs in pairs with feet and braces.
    for sz in (-1, 1):
        z = sz * (W / 2 - 0.03)
        bar(s, (L, 0.12, 0.05), (0.0, yc - 0.01, z))
        bar(s, (L, 0.02, 0.02), (0.0, H + 0.02, sz * (W / 2 - 0.01)), "Zinc")  # guide rail
    for x in (-2.7, -0.9, 0.9, 2.7):
        for sz in (-1, 1):
            z = sz * (W / 2 - 0.05)
            bar(s, (0.06, yc - 0.082, 0.06), (x, 0.012 + (yc - 0.082) / 2, z), "Zinc")
            bar(s, (0.14, 0.012, 0.12), (x, 0.006, z), "Steel")
        bar(s, (0.05, 0.05, W - 0.1), (x, 0.18, 0.0), "Zinc")
    # The drive at the +X end: a motor box under the rollers, a stencil
    # and an orange guard over the chain.
    bar(s, (0.5, 0.35, 0.4), (2.55, 0.45, 0.0), "Shell")
    s.cyl(0.11, -0.212, -0.19, "Zinc", segs=12, xf=T(2.55, 0.45, 0.0))
    bar(s, (0.42, 0.18, 0.02), (2.6, yc - 0.14, W / 2 + 0.005), "Orange")
    text(s, "C-2", 0.08, "Black", T(2.55, 0.48, -0.201) @ RY(math.pi), depth=0.002)
    # End stops.
    for sx in (-1, 1):
        bar(s, (0.02, 0.1, W - 0.06), (sx * (L / 2 + 0.01), H + 0.01, 0.0), "Zinc")
    parts = [s]
    n = int(L / 0.25)
    for k in range(n):
        x = -L / 2 + 0.125 + 0.25 * k
        r = Part(f"roller_{k}", origin=(x, yc, 0.0))
        r.cyl(R, -W / 2 + 0.06, W / 2 - 0.06, "Zinc", segs=10, xf=T(x, yc, 0.0),
              chamfer=0.004)
        parts.append(r)
    write("depot_conveyor", parts, {
        "pt_feed": (-L / 2 + 0.2, H + 0.005, 0.0),
        "pt_drop": (L / 2 - 0.2, H + 0.005, 0.0),
    })


# --- Tagger -------------------------------------------------------------------------------

def depot_tagger():
    reset()
    W, H, D = 0.7, 1.3, 0.6
    t = Part("static_tagger")
    # A plinth, the cabinet, and the console sloped back from the front.
    t.box((W, 0.06, D), (0.0, 0.03, 0.0), "Steel", bevel=0.006, segs=1)
    t.box((W - 0.04, 0.9, D - 0.04), (0.0, 0.51, 0.0), "Shell", bevel=0.012, segs=1)
    con = [(-0.27, 0.96), (0.28, 0.96), (0.28, 1.3), (0.1, 1.3), (-0.27, 1.04)]
    t.prism(fillet(con, [0.01, 0.01, 0.02, 0.02, 0.01], 2), -W / 2 + 0.02, W / 2 - 0.02,
            "Shell", axis="x", bevel=0.008, segs=1)
    # The screen on the slope, in a black bezel, and a row of keys.
    slope = math.atan2(1.3 - 1.04, 0.1 + 0.27)
    face = T(0.0, 1.17, -0.085) @ RX(-(math.pi / 2 - slope))
    t.box((0.44, 0.24, 0.01), (0.0, 0.0, 0.0), "Black", segs=1, xf=face @ RX(math.pi / 2))
    t.box((0.38, 0.19, 0.004), (0.0, 0.0, 0.0), "Glow", segs=1,
          xf=face @ T(0.0, 0.0064, 0.0) @ RX(math.pi / 2))
    for k in range(5):
        t.box((0.045, 0.012, 0.03), (0.0, 0.0, 0.0), "Black", segs=1,
              xf=T(-0.12 + 0.06 * k, 1.005, -0.24) @ RX(slope))
    # The tag slot in the cabinet's front, a tag half out of it; the reel
    # housing on the side.
    t.box((0.24, 0.035, 0.02), (0.0, 0.86, -D / 2 + 0.012), "Black", segs=1)
    t.box((0.16, 0.003, 0.09), (0.0, 0.86, -D / 2 - 0.035), "Orange", segs=1)
    t.cyl(0.13, 0.0, 0.06, "Zinc", segs=16, xf=T(W / 2 - 0.02, 0.62, 0.05) @ ALONG_X,
          chamfer=0.01)
    t.cyl(0.03, 0.06, 0.08, "Black", segs=10, xf=T(W / 2 - 0.02, 0.62, 0.05) @ ALONG_X)
    # A stencil on the front panel and a vent.
    text(t, "T-1", 0.07, "Black", T(0.0, 0.55, -D / 2 + 0.019) @ RY(math.pi), depth=0.002)
    for k in range(4):
        t.box((0.3, 0.012, 0.004), (0.0, 0.18 + 0.03 * k, -D / 2 + 0.019), "Black", segs=1)
    write("depot_tagger", [t], {"pt_slot": (0.0, 0.86, -D / 2 - 0.08)})


# --- Crates -------------------------------------------------------------------------------

def crate(name, W, H, D, points_extra, strap_z, disc=False, open_=False):
    """A white-painted steel crate: a body with skids and end handles, a
    black stencil plate on the front, and a lid hinged on the back top edge,
    its straps buckled across: shut, or (`open_`) the body hollow and the
    lid swung back past upright onto its hinge."""
    reset()
    LID = 0.08
    b = Part("static_crate")
    b.box((W, H - LID, D), (0.0, (H - LID) / 2 + 0.03, 0.0), "Shell", bevel=0.01, segs=1)
    if open_:
        # Hollow it: walls 2 cm thick, the floor 6 cm up, zinc inside.
        inside = Part("cut")
        inside.box((W - 0.04, H, D - 0.04), (0.0, 0.09 + H / 2, 0.0), "Zinc", segs=1)
        b.cut(inside, paint=True)
    for sx in (-1, 1):
        bar(b, (0.06, 0.03, D - 0.04), (sx * (W / 2 - 0.12), 0.015, 0.0), "Steel")
        bar(b, (0.012, 0.03, 0.18), (sx * (W / 2 + 0.006), H * 0.55, 0.0), "Black")
    bar(b, (min(0.6, W * 0.45), 0.12, 0.004), (-W * 0.18, H * 0.45, -D / 2 - 0.002), "Black")
    # The hook a tag hangs on, right of the plate.
    bar(b, (0.015, 0.03, 0.02), (W * 0.3, H * 0.62, -D / 2 - 0.008), "Steel")
    if disc:
        # The wire loop a collar disc hangs from.
        b.lathe([(0.012, -0.0015), (0.0135, 0.0), (0.012, 0.0015), (0.0105, 0.0)], "Zinc",
                segs=6, xf=T(W * 0.3, H * 0.4, -D / 2 - 0.004), closed=True)
    hinge = (0.0, H - LID + 0.03, D / 2)
    lid = Part("lid", origin=hinge)
    # Open, the lid turns 105 degrees back about its hinge.
    swing = RX(math.radians(105), about=hinge) if open_ else T()
    lid.box((W + 0.01, LID, D + 0.01), (0.0, H - LID / 2 + 0.03, 0.0), "Shell", bevel=0.01,
            segs=1, xf=swing)
    for z in strap_z:
        lid.box((W + 0.03, 0.006, 0.05), (0.0, H + 0.033, z), "Black", segs=1, xf=swing)
        lid.box((0.05, 0.012, 0.065), (W * 0.32, H + 0.039, z), "Steel", segs=1, xf=swing)
    write(name, [b, lid], {
        "pt_tag": (W * 0.3, H * 0.62 - 0.02, -D / 2 - 0.01),
        # Open, the top point is the crate's floor (what's in it).
        "pt_top": (0.0, 0.09, 0.0) if open_ else (0.0, H + 0.04, 0.0),
        **points_extra,
    })


def depot_crate_long(open_=False):
    crate("depot_crate_long" + ("_open" if open_ else ""), 2.1, 0.65 - 0.03, 0.7, {},
          (-0.18, 0.18), open_=open_)


def depot_crate_small(open_=False):
    W, H, D = 0.8, 0.5 - 0.03, 0.6
    crate("depot_crate_small" + ("_open" if open_ else ""), W, H, D,
          {"pt_disc": (W * 0.3, H * 0.4 - 0.014, -D / 2 - 0.004)}, (0.0,), disc=True,
          open_=open_)


def depot_disc():
    reset()
    d = Part("disc")
    d.lathe([(0.0035, -0.001), (0.019, -0.001), (0.02, 0.0), (0.019, 0.001), (0.0035, 0.001)],
            "Brass", segs=12, closed=True)
    write("depot_disc", [d], {"pt_hole": (0.0, 0.0, 0.0)})


# --- Intake facade and signs -------------------------------------------------------------

def depot_intake_facade():
    """A Custodian front over the kiosk's door end: two jambs and a header,
    a roller shutter's housing over the opening, hazard stripes on the
    jambs, a plate for the gate's name."""
    reset()
    W, H, D = 3.0, 3.4, 0.5
    OW, OH = 1.2, 2.2               # the clear opening (the door is 1.1 x 2.1)
    f = Part("static_facade")
    jw = (W - OW) / 2
    for sx in (-1, 1):
        bar(f, (jw, OH, D), (sx * (OW / 2 + jw / 2), OH / 2, 0.0))
        # Hazard stripes down the jamb's front, by the opening: an orange
        # strip and black bands across it at 45 degrees.
        x0, x1 = sx * OW / 2, sx * (OW / 2 + 0.15)
        f.box((0.15, OH - 0.1, 0.004), (sx * (OW / 2 + 0.075), (OH - 0.1) / 2 + 0.05,
              -D / 2 - 0.002), "Orange", segs=1)
        for k in range(7):
            y0 = 0.1 + 0.3 * k
            band = [(x0, y0), (x1, y0 + 0.15), (x1, y0 + 0.25), (x0, y0 + 0.1)]
            f.prism(band if sx > 0 else band[::-1], -D / 2 - 0.01, -D / 2 - 0.003, "Black",
                    axis="z")
    bar(f, (W, H - OH, D), (0.0, OH + (H - OH) / 2, 0.0))
    # The shutter's housing over the opening, standing out of the header.
    bar(f, (OW + 0.16, 0.36, 0.3), (0.0, OH + 0.18, -D / 2 - 0.15), "Zinc")
    # The name plate above, and a glowing strip under the housing.
    bar(f, (1.6, 0.36, 0.01), (0.0, OH + 0.72, -D / 2 - 0.005), "Black")
    bar(f, (OW, 0.02, 0.02), (0.0, OH - 0.01, -D / 2 - 0.2), "Glow")
    # The shutter, rolled up: a band of slats under the housing. Its
    # origin is the housing's bottom edge; scale its Y about that (up to
    # 22x) to drop it to the floor.
    sh = Part("shutter", origin=(0.0, OH, -D / 2 - 0.15))
    bar(sh, (OW - 0.02, 0.1, 0.03), (0.0, OH - 0.05, -D / 2 - 0.15), "Zinc")
    write("depot_intake_facade", [f, sh], {"pt_label": (0.0, OH + 0.72, -D / 2 - 0.011)})


def depot_sign():
    """A wall plate with a blank face in a black stencil frame."""
    reset()
    W, H, D = 0.9, 0.4, 0.03
    g = Part("static_sign")
    bar(g, (W - 0.02, H - 0.02, D - 0.01), (0.0, 0.0, 0.0))
    for sy in (-1, 1):
        bar(g, (W, 0.03, D), (0.0, sy * (H / 2 - 0.015), 0.0), "Black")
    for sx in (-1, 1):
        bar(g, (0.03, H - 0.06, D), (sx * (W / 2 - 0.015), 0.0, 0.0), "Black")
    write("depot_sign", [g], {"pt_label": (0.0, 0.0, -D / 2 + 0.0045)})


# --- Charging cradle ------------------------------------------------------------------------

def depot_cradle():
    reset()
    W, H, D = 2.2, 2.6, 2.0
    c = Part("static_cradle")
    # The plinth, a ramp at the front, tread plates where the feet stand,
    # an orange line round the bay.
    P = 0.14
    c.box((W, P, D - 0.3), (0.0, P / 2, 0.15), "Shell", bevel=0.015, segs=1)
    c.prism([(-D / 2, 0.0), (-D / 2 + 0.3, 0.0), (-D / 2 + 0.3, P), (-D / 2 + 0.29, P)],
            -W / 2, W / 2, "Zinc", axis="x")
    for sx in (-1, 1):
        for sz in (-1, 1):
            c.box((0.42, 0.006, 0.42), (sx * 0.62, P + 0.0035, 0.15 + sz * 0.42), "Black",
                  segs=1)
    for sx in (-1, 1):
        bar(c, (0.04, 0.004, D - 0.6), (sx * (W / 2 - 0.08), P + 0.0025, 0.05), "Orange")
    # Two uprights at the back and a gantry across.
    for sx in (-1, 1):
        x = sx * (W / 2 - 0.1)
        c.box((0.16, H - P - 0.012, 0.16), (x, P + (H - P - 0.012) / 2, D / 2 - 0.12), "Shell",
              bevel=0.01,
              segs=1)
    c.box((W, 0.2, 0.22), (0.0, H - 0.1, D / 2 - 0.12), "Shell", bevel=0.01, segs=1)
    # The charging arm: down from the gantry and forward, the connector
    # head over where the walker's back sits, with its contact glowing.
    arm = [(D / 2 - 0.12, H - 0.2), (D / 2 - 0.12, 2.15), (D / 2 - 0.3, 1.98), (0.25, 1.95)]
    c.sweep(arm, [(a, b) for a, b in section_circle(0.05, 8)], "Zinc")
    c.box((0.26, 0.18, 0.2), (0.0, 1.93, 0.17), "Shell", bevel=0.02, segs=1)
    c.box((0.14, 0.06, 0.012), (0.0, 1.86, 0.065), "Black", segs=1)
    c.box((0.1, 0.03, 0.006), (0.0, 1.86, 0.058), "Glow", segs=1)
    # Status lamps on the left upright; a stencil on the gantry.
    for k, m in enumerate(("Glow", "Orange")):
        c.cyl(0.03, 0.0, 0.02, m, segs=10,
              xf=T(-W / 2 + 0.1, 1.5 + 0.1 * k, D / 2 - 0.2) @ RY(math.pi))
    text(c, "C-04", 0.08, "Black", T(0.0, H - 0.1, D / 2 - 0.231) @ RY(math.pi), depth=0.003)
    # The cable: off the gantry's right end, looping down and back to the
    # wall (z = D / 2).
    cable = [(W / 2 - 0.1, H - 0.15, D / 2 - 0.04), (W / 2 - 0.02, H - 0.35, D / 2 - 0.02),
             (W / 2 + 0.05, H - 0.9, D / 2 - 0.05), (W / 2 + 0.06, 1.2, D / 2 - 0.08),
             (W / 2 + 0.04, 0.9, D / 2 - 0.02), (W / 2 - 0.05, 0.8, D / 2)]
    for a, b_ in zip(cable, cable[1:]):
        _cable_seg(c, a, b_, 0.028)
    c.box((0.16, 0.16, 0.03), (W / 2 - 0.05, 0.8, D / 2 - 0.015), "Black", bevel=0.01, segs=1)
    write("depot_cradle", [c], {"pt_dock": (0.0, P, 0.15)})


def _cable_seg(p, a, b, r):
    ax, ay, az = a
    bx, by, bz = b
    dx, dy, dz = bx - ax, by - ay, bz - az
    L = math.sqrt(dx * dx + dy * dy + dz * dz)
    yaw = math.atan2(dx, dz)
    pitch = -math.asin(dy / L)
    p.cyl(r, -r * 0.5, L + r * 0.5, "Rubber", segs=8, xf=T(ax, ay, az) @ RY(yaw) @ RX(pitch))


if __name__ == "__main__":
    for name, build in (("depot_bin", depot_bin), ("depot_conveyor", depot_conveyor),
                        ("depot_tagger", depot_tagger), ("depot_crate_long", depot_crate_long),
                        ("depot_crate_small", depot_crate_small), ("depot_disc", depot_disc),
                        ("depot_crate_long_open", lambda: depot_crate_long(True)),
                        ("depot_crate_small_open", lambda: depot_crate_small(True)),
                        ("depot_cradle", depot_cradle),
                        ("depot_intake_facade", depot_intake_facade),
                        ("depot_sign", depot_sign)):
        if wanted(name):
            build()
