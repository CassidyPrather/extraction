"""Music: a portable cassette player, a compact cassette and a 78 rpm
record. Writes assets/models/items/<id>.glb for each (item space: see
_kit.py; the cassette and record are centred on their bounds).

The parts are separate nodes the game moves:

- cassette_player: a chunky 1980s Eastern-bloc portable recorder, 0.150 x
  0.090 x 0.040 m body, its cassette door on the front (+Z, toward the
  viewer). `door` turns about its local X on its bottom-edge hinge
  (positive opens it, the top tipping toward +Z); `door_window` (the clear
  pane) is the door's child, its origin on the same hinge. Four piano keys
  on the top face, 34 mm apart (`btn_rew`, `btn_ff`, `btn_play`,
  `btn_stop`, left to right), each with its origin at the centre of its top
  face, pushed down along -Y. `reel_l`, `reel_r` are the drive spindles in
  the well, origin on their axis at the well floor, spun about local Z.
  `cell_door` is the battery cover on the back, origin at the centre of its
  outer face (it slides off along -Y). The cassette stands in the well
  behind the door, long axis along X, open edge down (-Y), side A toward
  +Z (`WELL`, `SEAT` below). Built on the body's centre rather than its
  bounds' (which run from the feet at y -0.0465 to the keys' tops at
  +0.051), so the keys sit at X -0.051, -0.017, +0.017, +0.051 exactly.
- cassette: the same frame -- long axis X, open (tape) edge at -Y, side A's
  label on +Z, side B's on -Z. `hub_l`, `hub_r` spin about local Z.
- record: a 10-inch shellac 78 lying flat (+Y up), `label` both sides.
"""

import math
import os
import sys

import bmesh
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, panel, text, wanted  # noqa: E402
import common  # noqa: E402
from common import (Part, RY, RZ, T, export, fillet, make, parent, report,  # noqa: E402
                    reset)


def shift_centred(parts):
    """Move Parts (geometry, cutters and origins) so their joint bounds are
    centred on the origin; returns the offset applied."""
    vs = [v.co for p in parts for v in p.bm.verts]
    lo = Vector((min(v.x for v in vs), min(v.y for v in vs), min(v.z for v in vs)))
    hi = Vector((max(v.x for v in vs), max(v.y for v in vs), max(v.z for v in vs)))
    d = -(lo + hi) / 2
    for p in parts:
        for q in [p] + p.cutters:
            for v in q.bm.verts:
                v.co += d
        p.origin = p.origin + d
    return d


def write(item_id, parts, children=(), centre=True):
    """Make the Parts, parent (child, parent) name pairs, write the .glb.
    With `centre`, first move everything to centre it on its bounds."""
    reset()
    d = shift_centred(parts) if centre else Vector()
    objs = {p.name: make(p) for p in parts}
    for child, par in children:
        parent(objs[child], objs[par])
    print(f"{item_id}:")
    for p in parts:
        o = p.origin
        print(f"  origin {p.name:12s} ({o.x:+.4f}, {o.y:+.4f}, {o.z:+.4f})")
    report()
    common.FOLDS.clear()
    export(f"assets/models/items/{item_id}.glb")
    return d


def rect(x0, x1, y0, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


# --- Cassette player ------------------------------------------------------------------------

PW, PH, PD = 0.150, 0.090, 0.040        # body
WELL_X = 0.0515                          # the cassette well, half width
WELL_Y0, WELL_Y1 = -0.039, 0.0275
WELL_FLOOR = 0.0025                      # z of the well's floor
WELL = (0.0, (WELL_Y0 + WELL_Y1) / 2, (WELL_FLOOR + PD / 2) / 2)
# The cassette's centre when it sits in the well (on its open edge, side A
# toward the door).
SEAT = (0.0, WELL_Y0 + 0.0319 + 0.0001, WELL_FLOOR + 0.006)
HUB_DX, HUB_DY = 0.02125, 0.0035         # hubs from the cassette's centre
KEYS = (("btn_rew", -0.051), ("btn_ff", -0.017), ("btn_play", 0.017), ("btn_stop", 0.051))
KEY_W, KEY_D, KEY_UP = 0.028, 0.018, 0.006
JACK = T(0.0, 0.0, -0.0145)              # headphone socket, on top behind the keys


def cassette_player():
    top = PH / 2
    front = PD / 2
    p = Part("body")
    outline = fillet(rect(-PW / 2, PW / 2, -PH / 2, PH / 2), 0.009, 4)
    p.prism(outline, -PD / 2, PD / 2, "Rotor", axis="z", bevel=0.003, segs=2)
    cut = Part("cut")
    # The well behind the door.
    cut.box((2 * WELL_X, WELL_Y1 - WELL_Y0, 0.03), (0.0, (WELL_Y0 + WELL_Y1) / 2,
                                                    WELL_FLOOR + 0.015), "Rotor")
    # Slots for the keys.
    for _, x in KEYS:
        cut.box((KEY_W + 0.0008, 0.014, KEY_D + 0.0008), (x, top - 0.005, 0.0), "Rotor")
    # Battery cover's pocket on the back (3.5 mm: shallower, the cover's
    # face and the pocket's floor would z-fight).
    cut.box((0.1004, 0.0604, 0.0074), (0.0, -0.005, -PD / 2), "Rotor")
    # The thumbwheel's slot in the +X side.
    cut.box((0.03, 0.0228, 0.0058), (PW / 2, 0.012, 0.0), "Rotor")
    # Headphone socket on top, behind the keys.
    cut.cyl(0.0022, top - 0.008, top + 0.002, "Rotor", segs=12, xf=JACK @ UP)
    p.cut(cut)
    # Stencils, painted into the plastic: the maker's name over the door,
    # the speaker's slots on the -X side, the socket's ring.
    paint = Part("paint")
    text(paint, "ЛЕСНИК-205", 0.0062, "Enamel", T(-0.028, 0.0365, front - 0.0005), depth=0.001)
    for k in range(4):
        paint.box((0.007, 0.0012, 0.001), (0.036 + k * 0.0085, 0.0365, front), "LampRed"
                  if k == 0 else "Enamel")
    for k in range(7):
        y = -0.03 + k * 0.006
        paint.box((0.0016, 0.0024, 0.024), (-PW / 2, y, 0.0), "Black")
    paint.box((0.0016, 0.026, 0.0024), (-PW / 2, -0.012, -0.0135), "Black")
    paint.box((0.0016, 0.026, 0.0024), (-PW / 2, -0.012, 0.0135), "Black")
    p.cut(paint, paint=True)
    # In the well: the head, the pinch roller and capstan (they reach into
    # the cassette's openings), a pad of felt under the cassette.
    sz = SEAT[2]
    p.box((0.011, 0.0055, 0.006), (0.0, WELL_Y0 + 0.00275, sz), "Steel", bevel=0.0008, segs=1)
    p.box((0.016, 0.003, 0.008), (0.0, WELL_Y0 + 0.0015, sz - 0.001), "Dark")
    p.cyl(0.0028, sz - 0.003, sz + 0.003, "Rubber", segs=14, xf=T(0.0175, WELL_Y0 + 0.0035, 0))
    p.cyl(0.0009, WELL_FLOOR, sz + 0.003, "Steel", segs=8, xf=T(0.0235, WELL_Y0 + 0.0035, 0))
    p.cyl(0.0035, WELL_FLOOR, sz - 0.003, "Dark", segs=10, xf=T(0.0175, WELL_Y0 + 0.0035, 0))
    p.cyl(0.0024, WELL_FLOOR, sz - 0.003, "Dark", segs=10, xf=T(-0.0175, WELL_Y0 + 0.0035, 0))
    # The headphone socket's ring, the thumbwheel, rubber feet underneath.
    p.tube(0.0034, 0.0022, top - 0.004, top + 0.0008, "Steel", segs=14, xf=JACK @ UP)
    wheel = []
    for k in range(48):
        a = 2 * math.pi * k / 48
        r = 0.0095 if k % 2 == 0 else 0.0088
        wheel.append((PW / 2 - 0.0075 + r * math.cos(a), 0.012 + r * math.sin(a)))
    p.prism(wheel, -0.0024, 0.0024, "Pebble", axis="z")
    p.cyl(0.0016, -0.0028, 0.0028, "Steel", segs=10, xf=T(PW / 2 - 0.0075, 0.012, 0))
    for sx in (-1, 1):
        p.box((0.02, 0.0015, 0.03), (sx * 0.05, -PH / 2 - 0.0005, 0.0), "Rubber", bevel=0.0006,
              segs=1)
    # A wrist-strap lug at the top left corner.
    p.box((0.004, 0.012, 0.012), (-PW / 2 - 0.0015, top - 0.012, -0.008), "Steel", bevel=0.001,
          segs=1)
    parts = [p]

    # The door, hinged on its bottom edge, the pane its child.
    hinge = (0.0, WELL_Y0 + 0.0005, front - 0.0015)
    door = Part("door", origin=hinge)
    dx0, dx1, dy0, dy1 = -WELL_X + 0.0005, WELL_X - 0.0005, WELL_Y0 + 0.0005, WELL_Y1 - 0.0005
    win = (-0.034, 0.034, SEAT[1] - 0.014, SEAT[1] + 0.017)
    door.prism(fillet(rect(dx0, dx1, dy0, dy1), 0.003, 2), front - 0.003, front, "Steel",
               axis="z", bevel=0.0008, segs=1)
    hole = Part("hole")
    hole.prism(fillet(rect(*win), 0.004, 3), front - 0.004, front + 0.001, "Steel", axis="z")
    stripes = Part("stripes")
    # Two black pinstripes and the deck's markings, painted on.
    stripes.box((dx1 - dx0 - 0.01, 0.0012, 0.001), (0.0, dy1 - 0.0045, front), "Black")
    stripes.box((dx1 - dx0 - 0.01, 0.0012, 0.001), (0.0, dy0 + 0.0045, front), "Black")
    text(stripes, "STEREO", 0.0032, "Black", T(0.03, dy1 - 0.0085, front - 0.0005), depth=0.001)
    text(stripes, "60", 0.0032, "Black", T(-0.038, dy1 - 0.0085, front - 0.0005), depth=0.001)
    door.cut(hole)
    door.cut(stripes, paint=True)
    # A thumb catch at the top, the hinge knuckles at the bottom.
    door.box((0.024, 0.003, 0.0035), (0.0, dy1 - 0.0005, front - 0.0002), "Dark", bevel=0.0008,
             segs=1)
    for sx in (-1, 1):
        door.cyl(0.0016, sx * 0.03, sx * 0.045, "Steel", segs=10,
                 xf=T(0.0, hinge[1], hinge[2]) @ RY(math.pi / 2))
    pane = Part("door_window", origin=hinge)
    pane.prism(fillet(rect(win[0] + 0.0003, win[1] - 0.0003, win[2] + 0.0003, win[3] - 0.0003),
                      0.0037, 3), front - 0.0028, front - 0.0004, "Clear", axis="z")
    parts += [door, pane]

    # The keys: chunky piano keys standing 6 mm proud, stamped symbols.
    def tri(x, z, s, point):
        return [(x - point * s * 0.55, z - s * 0.6), (x + point * s * 0.6, z),
                (x - point * s * 0.55, z + s * 0.6)]

    for name, x in KEYS:
        ky = top + KEY_UP
        k = Part(name, origin=(x, ky, 0.0))
        mat = "Red" if name == "btn_stop" else "Pebble"
        ink = "Enamel" if name == "btn_stop" else "Black"
        prof = fillet(rect(x - KEY_W / 2, x + KEY_W / 2, -KEY_D / 2, KEY_D / 2), 0.002, 2)
        k.prism(prof, ky - 0.016, ky, mat, axis="y", bevel=0.0012, segs=2)
        stamp = Part("stamp")
        s = 0.006
        if name == "btn_play":
            shapes = [tri(x, 0.0, s, 1)]
        elif name == "btn_ff":
            shapes = [tri(x - 0.003, 0.0, s * 0.8, 1), tri(x + 0.0035, 0.0, s * 0.8, 1)]
        elif name == "btn_rew":
            shapes = [tri(x + 0.003, 0.0, s * 0.8, -1), tri(x - 0.0035, 0.0, s * 0.8, -1)]
        else:
            shapes = [rect(x - 0.0035, x + 0.0035, -0.0035, 0.0035)]
        for sh in shapes:
            stamp.prism(sh, ky - 0.0006, ky + 0.001, ink, axis="y")
        k.cut(stamp, paint=True)
        parts.append(k)

    # The drive spindles, seen through the window.
    for name, sx in (("reel_l", -1), ("reel_r", 1)):
        c = (sx * HUB_DX, SEAT[1] + HUB_DY, WELL_FLOOR)
        r = Part(name, origin=c)
        xf = T(c[0], c[1], 0.0)
        r.cyl(0.0068, WELL_FLOOR, WELL_FLOOR + 0.0012, "Dark", segs=20, xf=xf)
        r.cyl(0.0031, WELL_FLOOR, WELL_FLOOR + 0.0115, "Enamel", segs=16, xf=xf, chamfer=0.0006)
        for j in range(6):
            a = 2 * math.pi * j / 6
            r.box((0.0026, 0.0012, 0.008), (0.0, 0.0, 0.0), "Enamel",
                  xf=xf @ T(0.0028 * math.cos(a), 0.0028 * math.sin(a), WELL_FLOOR + 0.0062)
                  @ RZ(a))
        parts.append(r)

    # The battery cover: a plug in its pocket, ridges to push it by.
    cz = -PD / 2 + 0.0003
    cell = Part("cell_door", origin=(0.0, -0.005, cz))
    cell.prism(fillet(rect(-0.05, 0.05, -0.035, 0.025), 0.004, 3), cz, cz + 0.0035, "Rotor",
               axis="z", bevel=0.0008, segs=1)
    for j in range(5):
        cell.box((0.03, 0.0012, 0.0012), (0.0, -0.03 + j * 0.0028, cz - 0.0002), "Rotor",
                 bevel=0.0004, segs=1)
    marks = Part("marks")
    text(marks, "4x1.5V", 0.005, "Enamel", T(0.0, 0.008, cz + 0.0005) @ RY(math.pi),
         depth=0.001)
    cell.cut(marks, paint=True)
    parts.append(cell)
    return parts


# --- Cassette ------------------------------------------------------------------------------

CW, CH, CT = 0.1004, 0.0638, 0.012


def cassette():
    """Built standing up: long axis X, open edge at -Y, side A on +Z."""
    s = Part("shell")
    s.prism(fillet(rect(-CW / 2, CW / 2, -CH / 2, CH / 2), 0.0025, 2), -CT / 2, CT / 2, "Rotor",
            axis="z", bevel=0.0006, segs=1)
    lab = (-0.046, 0.046, -0.012, 0.028)        # the label's area on each face
    win = (-0.0115, 0.0115, -0.0025, 0.0095)    # the window
    cut = Part("cut")
    for sz in (-1, 1):
        # The label's pocket (3.4 mm: the label is a plug in it, so its face
        # and the pocket's floor can't z-fight).
        cut.box((lab[1] - lab[0], lab[3] - lab[2], 0.0034 + 0.004),
                ((lab[0] + lab[1]) / 2, (lab[2] + lab[3]) / 2, sz * (CT / 2 - 0.0034 + 0.0038)),
                "Rotor")
    for sx in (-1, 1):
        cut.cyl(0.0066, -0.01, 0.01, "Rotor", segs=24, xf=T(sx * HUB_DX, HUB_DY, 0.0))
        # Capstan holes either side of the head opening.
        cut.cyl(0.0013, -0.01, 0.01, "Rotor", segs=10, xf=T(sx * 0.0255, -0.0245, 0.0))
    # The openings along the bottom edge: the head's in the middle, the
    # pinch rollers' and erase head's either side.
    for x, w in ((0.0, 0.013), (-0.0175, 0.0075), (0.0175, 0.0075), (-0.033, 0.0045),
                 (0.033, 0.0045)):
        cut.box((w, 0.011, 0.0062), (x, -CH / 2, 0.0), "Rotor")
    s.cut(cut)
    # The window: sunk past the label's floor, smoky.
    glass = Part("glass")
    for sz in (-1, 1):
        glass.box((win[1] - win[0], win[3] - win[2], 0.002),
                  ((win[0] + win[1]) / 2, (win[2] + win[3]) / 2, sz * (CT / 2 - 0.0039)), "Lens")
    # Screws in the shell's lower corners and middle.
    for sz in (-1, 1):
        for x, y in ((-0.045, -0.027), (0.045, -0.027), (0.0, -0.0215)):
            glass.cyl(0.0011, sz * CT / 2 - 0.0006, sz * CT / 2 + 0.0006, "Steel", segs=8,
                      xf=T(x, y, 0.0))
    s.cut(glass, paint=True)

    # The tape across the openings.
    tape = Part("tape")
    tape.box((0.08, 0.0004, 0.0038), (0.0, -CH / 2 + 0.0015, 0.0), "Bakelite")

    # Labels: a plug in each pocket, its face 0.2 mm under the shell's.
    def label(name, base, band, letter):
        L = Part(name)
        t1 = CT / 2 - 0.0002
        panel(L, "z", lab[0], lab[1], lab[2], lab[3], t1 - 0.0034, t1, base,
              [(lab[0], lab[1], 0.0235, 0.0262, band),
               (lab[0], lab[1], -0.0098, -0.0072, band)])
        holes = Part("holes")
        holes.box((win[1] - win[0] + 0.004, win[3] - win[2] + 0.004, 0.01),
                  ((win[0] + win[1]) / 2, (win[2] + win[3]) / 2, t1), base)
        for sx in (-1, 1):
            holes.cyl(0.0076, t1 - 0.01, t1 + 0.01, base, segs=24, xf=T(sx * HUB_DX, HUB_DY, 0))
        L.cut(holes)
        ink = Part("ink")
        text(ink, letter, 0.0065, "Black", T(-0.039, 0.017, t1 - 0.0004), depth=0.001)
        for y in (0.0135, 0.0185):
            ink.box((0.064, 0.0005, 0.001), (0.006, y, t1), "Blue")
        text(ink, "МК-60", 0.0028, "Black", T(0.034, -0.0056, t1 - 0.0004), depth=0.001)
        L.cut(ink, paint=True)
        return L

    la = label("label_a", "Cream", "Red", "A")
    lb = label("label_b", "Paper", "Blue", "В")
    # Side B: the same label turned over onto -Z (about Y, so the open
    # edge stays at -Y).
    for q in [lb] + lb.cutters:
        bmesh.ops.transform(q.bm, matrix=RY(math.pi), verts=q.bm.verts)

    hubs = []
    for name, sx in (("hub_l", -1), ("hub_r", 1)):
        c = (sx * HUB_DX, HUB_DY, 0.0)
        h = Part(name, origin=c)
        h.tube(0.0061, 0.0043, -CT / 2 + 0.0008, CT / 2 - 0.0008, "Enamel", segs=24,
               xf=T(c[0], c[1], 0.0))
        for j in range(6):
            a = 2 * math.pi * j / 6 + math.pi / 6
            h.box((0.0016, 0.0012, CT - 0.004), (0.0, 0.0, 0.0), "Enamel",
                  xf=T(c[0] + 0.0039 * math.cos(a), c[1] + 0.0039 * math.sin(a), 0.0) @ RZ(a))
        hubs.append(h)
    return [s, la, lb, tape] + hubs


# --- 78 rpm record -------------------------------------------------------------------------

def record(colour="Red"):
    R, RL, RH = 0.127, 0.0435, 0.0036
    t = 0.001
    disc = Part("disc")
    # Lathe profile (r, height): a rounded rim, the grooved field, the
    # smooth run-out round the label a hair lower.
    # Shallow V rings between the tracks catch the light, so the grooved
    # field reads as bands.
    gaps = (0.074, 0.093, 0.109)
    face = [(RL, t - 0.0001), (0.056, t - 0.0001), (0.058, t)]
    for g in gaps:
        face += [(g - 0.0005, t), (g, t - 0.00018), (g + 0.0005, t)]
    face += [(0.1225, t), (0.124, t + 0.00005), (0.1262, t - 0.0003), (R, 0.0003)]
    prof = [(r, -h) for r, h in face] + [(r, h) for r, h in reversed(face)]
    disc.lathe(prof, "Rotor", segs=64, closed=True)
    lab = Part("label")
    rings = [(RH, 0.026, colour), (0.026, 0.0275, "Cream"), (0.0275, RL, colour)]
    for r0, r1, mat in rings:
        lab.lathe([(r0, -t - 0.0001), (r1, -t - 0.0001), (r1, t + 0.0001), (r0, t + 0.0001)],
                  mat, segs=32, closed=True)
    ink = Part("ink")
    for sgn in (1, -1):
        text(ink, "78", 0.007, "Cream", (RY(math.pi) if sgn < 0 else T()) @
             T(0.0, 0.015, t + 0.0001 - 0.0004), depth=0.001)
        text(ink, "ПЛАСТИНКА", 0.0035, "Cream",
             (RY(math.pi) if sgn < 0 else T()) @ T(0.0, -0.016, t + 0.0001 - 0.0004),
             depth=0.001)
    lab.cut(ink, paint=True)
    parts = [disc, lab]
    # Lathe and text are built round Z (the disc's face toward +Z); lay it
    # flat, face up.
    for p in parts:
        for q in [p] + p.cutters:
            bmesh.ops.transform(q.bm, matrix=UP, verts=q.bm.verts)
    return parts


if __name__ == "__main__":
    if wanted("cassette_player"):
        # Built on the body's centre, not the bounds' (the keys stand 6 mm
        # over the top), so the keys sit at their X exactly.
        d = write("cassette_player", cassette_player(), children=[("door_window", "door")],
                  centre=False)
        print(f"  well centre ({WELL[0] + d.x:+.4f}, {WELL[1] + d.y:+.4f}, {WELL[2] + d.z:+.4f}),"
              f" cassette seat ({SEAT[0] + d.x:+.4f}, {SEAT[1] + d.y:+.4f}, {SEAT[2] + d.z:+.4f})")
    if wanted("cassette"):
        write("cassette", cassette())
    # Each tape you find is a cassette (the game's item ids,
    # extraction_core::music::tapes).
    for tid in ("tape_practice", "tape_hymns", "tape_lodge", "tape_duty", "tape_birthday",
                "tape_bells"):
        if wanted(tid):
            write(tid, cassette())
    if wanted("record"):
        write("record", record())
    # The records you find: one label colour each.
    for rid, colour in (("record_augustin", "Red"), ("record_entertainer", "Blue"),
                        ("record_susanna", "Olive")):
        if wanted(rid):
            write(rid, record(colour))
