"""Warning boards: what carries a warning where a track crosses into a hot
place, read from 10 m and more. Writes assets/models/props/<name>.glb:

- `warning_gate`      a plank field gate across a track between two posts,
                      a painted board across it, a chain and padlock, a
                      hazard lamp on the latch post, a trefoil plate
- `warning_hoarding`  a plank hoarding on two posts, braced behind
- `warning_enamel`    an enamel railway sign on a steel post, its rolled rim
                      chipped
- `warning_wall`      a concrete fence slab between two posts, a plain
                      patch for stencilled paint
- `warning_door`      a steel booth door in its frame and a stub of wall, a
                      plain panel at eye height for stencilled paint

Each stands on the ground at its origin and faces its reader on +Z. Its
words aren't modelled: the game draws them (`ScreenStyle::Sign`) on a
quad at the empty `text_face`, which sits on the face's plane at the
middle of its text area, +Z out toward the reader, +Y up. Nothing of the
model stands in front of that area. The areas (w x h, centre height) are
`world::notes::WARNING_BOARDS`, and
`world::notes::tests::warning_boards_keep_their_text_areas_clear` holds
the models to them. The gate also has an empty `lamp`, in front of its
lamp's red glass.

Grounds: the gate and hoarding are painted Cream inside a red-brown
(Red) border, the border's paint chipped to the board in places (never
the area's); the enamel is Enamel
inside a Black rolled rim; the wall's patch is its own Concrete; the
door's is a panel of flat grey paint (Pebble) over its rust-blotched
leaf.

Materials: Timber, Planks, Cream, Red, Iron, Rusty, Steel, Enamel, Black,
Concrete, Painted, Pebble, Yellow.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import UP, Wear, board, bolt, prop, rod  # noqa: E402
from common import RX, RY, RZ, T, Part, empty  # noqa: E402


def post(p, x, z, side_lo, side_hi, y0, y1, mat, cap=0.03):
    """A square post at (x, z) tapering from side_lo at y0 to side_hi near
    y1, its top chamfered to a point `cap` high (`outdoor.square_post`)."""
    r0, r1 = side_lo / math.sqrt(2), side_hi / math.sqrt(2)
    p.lathe([(0.0, y0), (r0, y0), (r1, y1 - cap), (r1 * 0.55, y1), (0.0, y1)], mat, segs=4,
            start=45.0, xf=T(x, 0.0, z) @ UP)


def painted_area(p, w, h, cy, z, ground, border, bw=0.05, depth=0.004, seed=None):
    """A text area w x h centred at (0, cy), its painted front on the plane
    z, in `ground`, inside a painted border bw wide in `border`. With a
    `seed`, the border's paint is chipped off in places (never the area's):
    gaps where the board shows `depth` under the paint."""
    p.box((w, h, depth), (0.0, cy, z - depth / 2), ground, segs=1)
    w_ = Wear(seed) if seed is not None else None

    def strip(x0, x1, y0, y1):
        """A border strip from (x0, y0) to (x1, y1), chipped along it."""
        along_x = (x1 - x0) >= (y1 - y0)
        length = (x1 - x0) if along_x else (y1 - y0)
        cuts = []
        if w_ is not None:
            t = w_.rng.uniform(0.05, 0.3)
            while t < length - 0.05:
                g = w_.rng.uniform(0.015, 0.04)
                cuts.append((t, t + g))
                t += g + w_.rng.uniform(0.15, 0.6)
        start = 0.0
        for c0, c1 in cuts + [(length, length)]:
            if c0 - start > 0.005:
                if along_x:
                    p.box((c0 - start, y1 - y0, depth), (x0 + (start + c0) / 2, (y0 + y1) / 2,
                                                        z - depth / 2), border, segs=1)
                else:
                    p.box((x1 - x0, c0 - start, depth), ((x0 + x1) / 2, y0 + (start + c0) / 2,
                                                        z - depth / 2), border, segs=1)
            start = c1

    for sy in (-1, 1):
        y0 = cy + h / 2 if sy > 0 else cy - h / 2 - bw
        strip(-w / 2 - bw, w / 2 + bw, y0, y0 + bw)
    for sx in (-1, 1):
        x0 = w / 2 if sx > 0 else -w / 2 - bw
        strip(x0, x0 + bw, cy - h / 2, cy + h / 2)


def trefoil(p, at, size, z):
    """A radiation plate: yellow, a black trefoil and hub standing 3.5 mm
    off it (raised, so the two faces don't fight), on +Z at z."""
    x, y = at
    p.box((size, size, 0.003), (x, y, z - 0.0015), "Yellow", bevel=0.002, segs=1)
    r_in, r_out = size * 0.09, size * 0.36
    for k in range(3):
        a0 = math.radians(60 + k * 120)
        arc = [a0 + math.radians(60) * i / 8 for i in range(9)]
        pts = [(r_in * math.cos(t), r_in * math.sin(t)) for t in arc[::-1]] + \
              [(r_out * math.cos(t), r_out * math.sin(t)) for t in arc]
        p.prism([(x + u, y + v) for u, v in pts], z + 0.0002, z + 0.0035, "Black", axis="z")
    p.cyl(size * 0.065, z + 0.0002, z + 0.0035, "Black", segs=16, xf=T(x, y, 0))


def chain(p, a, b, sag, links=14, r=0.012, wire=0.0028):
    """A hanging chain from a to b, sagging `sag` at its middle: links
    turned a quarter each to the last."""
    for i in range(links):
        t = (i + 0.5) / links
        x = a[0] + (b[0] - a[0]) * t
        y = a[1] + (b[1] - a[1]) * t - sag * 4 * t * (1 - t)
        z = a[2] + (b[2] - a[2]) * t
        turn = RY(math.pi / 2) if i % 2 else RX(0.0)
        p.tube(r, r - wire, -wire / 2, wire / 2, "Iron", segs=10, xf=T(x, y, z) @ turn)


# --- Plank gate ---------------------------------------------------------------------------

GATE_AREA = (2.4, 0.5, 1.1)


def warning_gate():
    w = Wear(91)
    p = Part("static")
    aw, ah, acy = GATE_AREA
    # Posts either side of the track, sunk 0.4.
    for x in (-1.72, 1.72):
        post(p, x, 0.0, 0.16, 0.14, -0.4, 1.75, "Timber", cap=0.05)
    # The leaf: two rails, end stiles, a brace, between the posts.
    zr = -0.01
    for y in (0.42, 1.52):
        board(p, (3.22, 0.12, 0.05), (0.0, y, zr), "Timber", w, bevel=0.006)
    for x in (-1.58, 1.58):
        board(p, (0.1, 1.24, 0.05), (x, 0.97, zr), "Timber", w, bevel=0.006)
    ang = math.atan2(1.0, 2.95)
    board(p, (3.1, 0.1, 0.03), (0.0, 0.97, zr - 0.004), "Timber", bevel=0.006,
          xf=T(0, 0.97, 0) @ RZ(ang) @ T(0, -0.97, 0))
    # The board across it, painted: cream inside a red-brown border.
    zb = 0.035
    # (Not wobbled: a board under paint is flat, or it shows through it.)
    board(p, (aw + 0.22, ah + 0.22, 0.035), (0.0, acy, zb - 0.0175), "Planks", bevel=0.005)
    zf = zb + 0.004
    painted_area(p, aw, ah, acy, zf, "Cream", "Red", seed=7)
    for sx in (-1, 1):
        for sy in (-1, 1):
            bolt(p, (sx * (aw / 2 + 0.08), acy + sy * (ah / 2 + 0.08), zb), (0, 0, 1), r=0.012,
                 h=0.008)
    # Hinge straps on the left post, a latch and a chain with its padlock
    # on the right.
    for y in (0.42, 1.52):
        p.box((0.42, 0.05, 0.008), (-1.5, y, zr + 0.03), "Iron", bevel=0.002, segs=1)
        rod(p, (-1.72, y - 0.06, 0.09), (-1.72, y + 0.06, 0.09), 0.012, "Iron")
    p.box((0.2, 0.04, 0.01), (1.62, 1.3, 0.025), "Iron", bevel=0.002, segs=1)
    chain(p, (1.56, 1.3, 0.05), (1.72, 1.24, 0.1), 0.14, links=9)
    p.box((0.045, 0.055, 0.02), (1.65, 1.09, 0.08), "Rusty", bevel=0.004, segs=1)
    p.tube(0.016, 0.012, -0.003, 0.003, "Steel", segs=12,
           xf=T(1.65, 1.128, 0.08) @ RX(math.pi / 2) @ RY(math.pi / 2))
    # The trefoil on the right post, beside the board, not on it.
    trefoil(p, (1.72, 0.78), 0.2, 0.0805)
    # The hazard lamp on the latch post's top: an iron hood over a red glass
    # facing the track.
    lx, ly = 1.72, 1.86
    p.cyl(0.07, ly - 0.08, ly + 0.08, "Iron", segs=16, xf=T(lx, 0.0, 0.0) @ UP)
    p.cyl(0.055, 0.0, 0.03, "Red", segs=16, xf=T(lx, ly, 0.065))
    p.box((0.16, 0.012, 0.11), (lx, ly + 0.09, 0.03), "Iron", bevel=0.003, segs=1)
    return [p]


# --- Hoarding -----------------------------------------------------------------------------

HOARDING_AREA = (2.0, 1.2, 1.6)


def warning_hoarding():
    w = Wear(92)
    p = Part("static")
    aw, ah, acy = HOARDING_AREA
    zp = -0.08
    for x in (-1.12, 1.12):
        post(p, x, zp, 0.14, 0.12, -0.5, 2.45, "Timber", cap=0.04)
        # A strut behind, from the post at 1.4 m into the ground 0.8 m back.
        top, foot = (x, 1.4, zp - 0.07), (x, -0.15, zp - 0.85)
        mid = tuple((a + b) / 2 for a, b in zip(top, foot))
        length = math.dist(top, foot)
        lean = math.atan2(top[2] - foot[2], top[1] - foot[1])
        board(p, (0.08, length, 0.06), mid, "Timber", bevel=0.005,
              xf=T(*mid) @ RX(lean) @ T(*(-v for v in mid)))
    # Battens on the posts' faces, the planks across them.
    for y in (0.98, 2.22):
        board(p, (2.4, 0.08, 0.035), (0.0, y, zp + 0.08), "Timber", w, bevel=0.005)
    n = 7
    for k in range(n):
        y = 0.92 + (k + 0.5) * (1.36 / n)
        # Flat under the paint (wobbled, a plank stood through it).
        board(p, (2.32 + w.j(0.02), 1.36 / n - 0.006, 0.025), (w.j(0.006), y, 0.024),
              "Planks", bevel=0.004)
    zf = 0.04
    painted_area(p, aw, ah, acy, zf, "Cream", "Red", bw=0.055, seed=11)
    for x in (-1.12, 1.12):
        for y in (0.98, 2.22):
            bolt(p, (x, y, 0.038), (0, 0, 1), r=0.01, h=0.006)
    return [p]


# --- Enamel railway sign -------------------------------------------------------------------

ENAMEL_AREA = (1.0, 0.6, 1.45)


def warning_enamel():
    p = Part("static")
    aw, ah, acy = ENAMEL_AREA
    # A steel pipe post, sunk 0.4, capped.
    p.cyl(0.03, -0.4, 1.84, "Painted", segs=14, xf=UP)
    p.cyl(0.034, 1.84, 1.86, "Iron", segs=14, xf=UP)
    # Two clamp bands and a bracket bar behind the plate.
    for y in (acy - 0.2, acy + 0.2):
        p.tube(0.036, 0.03, y - 0.02, y + 0.02, "Iron", segs=14, xf=UP)
        p.box((aw * 0.8, 0.035, 0.012), (0.0, y, 0.036), "Iron", bevel=0.002, segs=1)
    # The plate: enamel to its rolled rim.
    zf = 0.05
    p.box((aw + 0.02, ah + 0.02, 0.003), (0.0, acy, zf - 0.0015), "Enamel", segs=1)
    r = 0.0075
    for sy in (-1, 1):
        rod(p, (-(aw / 2 + r), acy + sy * (ah / 2 + r), zf - 0.002),
            ((aw / 2 + r), acy + sy * (ah / 2 + r), zf - 0.002), r, "Black", segs=8)
    for sx in (-1, 1):
        rod(p, (sx * (aw / 2 + r), acy - (ah / 2 + r), zf - 0.002),
            (sx * (aw / 2 + r), acy + (ah / 2 + r), zf - 0.002), r, "Black", segs=8)
    # Enamel chipped at the corners and along the rim: iron showing, rust
    # round it.
    w = Wear(93)
    for sx in (-1, 1):
        for sy in (-1, 1):
            cx, cy = sx * (aw / 2 + r), acy + sy * (ah / 2 + r)
            p.cyl(0.011 + w.j(0.004), zf + r - 0.002, zf + r - 0.001, "Rusty", segs=10,
                  xf=T(cx, cy, 0.0))
    for _ in range(6):
        x = w.rng.uniform(-aw / 2, aw / 2)
        sy = 1 if w.chance(0.5) else -1
        p.cyl(0.006 + w.j(0.002), zf + r - 0.002, zf + r - 0.001, "Iron", segs=8,
              xf=T(x, acy + sy * (ah / 2 + r), 0.0))
    return [p]


# --- Concrete fence slab ----------------------------------------------------------------

WALL_AREA = (1.6, 0.8, 1.6)


def warning_wall():
    p = Part("static")
    aw, ah, acy = WALL_AREA
    for x in (-1.32, 1.32):
        post(p, x, 0.0, 0.16, 0.14, -0.4, 2.35, "Concrete", cap=0.05)
    # The slab, 2.5 x 2.1, 0.1 through, its face at z 0.05.
    zf = 0.05
    p.box((2.5, 2.1, 0.1), (0.0, 1.1, 0.0), "Concrete", bevel=0.006, segs=1)
    # Cast ribs round the plain patch (none on it): a frame of them, and
    # a coping along the top.
    rib = 0.012
    # A frame of ribs round the patch, 6 cm clear of it, and two below.
    gap = 0.06
    # (The rails sunk 4 mm into the slab, the uprights on its face and 3.6
    # mm less proud than the rails, so no two faces share a plane.)
    for y in (acy - ah / 2 - gap - 0.025, acy + ah / 2 + gap + 0.025, 0.75, 0.3):
        p.box((2.3, 0.05, rib + 0.004), (0.0, y, zf + (rib - 0.004) / 2), "Concrete",
              bevel=0.004, segs=1)
    for x in (-(aw / 2 + gap + 0.025), aw / 2 + gap + 0.025):
        p.box((0.05, 1.86, rib * 0.7), (x, 1.15, zf + rib * 0.35), "Concrete", bevel=0.004,
              segs=1)
    p.box((2.56, 0.07, 0.16), (0.0, 2.185, 0.0), "Concrete", bevel=0.01, segs=1)
    return [p]


# --- Booth door ----------------------------------------------------------------------------

DOOR_AREA = (1.0, 0.6, 1.6)


def warning_door():
    p = Part("static")
    aw, ah, acy = DOOR_AREA
    # A stub of the booth's wall either side and over the door.
    zw = 0.0
    for x in (-0.86, 0.86):
        p.box((0.42, 2.6, 0.24), (x, 1.2, zw - 0.12), "Concrete", bevel=0.008, segs=1)
    p.box((1.3, 0.24, 0.24), (0.0, 2.38, zw - 0.12), "Concrete", bevel=0.008, segs=1)
    # The angle-iron frame round the opening, 1.3 x 2.26.
    for x in (-0.66, 0.66):
        p.box((0.05, 2.26, 0.05), (x, 1.13, 0.0), "Iron", bevel=0.003, segs=1)
    p.box((1.36, 0.05, 0.056), (0.0, 2.24, 0.0), "Iron", bevel=0.003, segs=1)
    # The leaf, 1.26 x 2.2, its face at z 0.03.
    zf = 0.03
    p.box((1.26, 2.2, 0.04), (0.0, 1.12, zf - 0.02), "Painted", bevel=0.004, segs=1)
    # A fresh panel of flat grey paint over the weathered leaf, for the
    # stencil: the leaf's own paint is blotched with rust, under the words
    # it fought them. 4 mm proud, so the two faces don't fight.
    p.box((aw + 0.12, ah + 0.12, 0.004), (0.0, acy, zf + 0.002), "Pebble", segs=1)
    # Pressed louvres low down and stiffeners, all off the plain panel.
    for k in range(6):
        y = 0.3 + k * 0.07
        p.box((0.7, 0.025, 0.012), (0.0, y, zf + 0.004), "Painted", bevel=0.003, segs=1,
              xf=T(0, y, zf) @ RX(-0.5) @ T(0, -y, -zf))
    p.box((1.18, 0.04, 0.012), (0.0, acy - ah / 2 - 0.1, zf + 0.006), "Painted", bevel=0.003,
          segs=1)
    # Hinges on the left, a handle and a hasp on the right.
    for y in (0.35, 1.12, 1.95):
        p.cyl(0.018, y - 0.06, y + 0.06, "Iron", segs=10, xf=T(-0.63, 0.0, zf + 0.01) @ UP)
    rod(p, (0.52, 1.02, zf + 0.04), (0.52, 1.18, zf + 0.04), 0.011, "Steel")
    for y in (1.02, 1.18):
        rod(p, (0.52, y, zf), (0.52, y, zf + 0.04), 0.009, "Steel")
    p.box((0.05, 0.12, 0.008), (0.58, 0.9, zf + 0.004), "Iron", bevel=0.002, segs=1)
    # A rusted kick plate at the foot, off the panel.
    p.box((1.16, 0.16, 0.008), (0.0, 0.12, zf + 0.002), "Rusty", bevel=0.002, segs=1)
    return [p]


def write_with(name, build, area, extra=None):
    """Build a carrier and mark its text area's middle (and any extras)."""
    def b():
        parts = build()
        aw, ah, acy = area
        z = FACE_Z[name]
        empty("text_face", (0.0, acy, z))
        for k, v in (extra or {}).items():
            empty(k, v)
        return parts
    prop(name, b)


# The plane each one's text area lies on (z), as built above.
FACE_Z = {
    "warning_gate": 0.039,
    "warning_hoarding": 0.04,
    "warning_enamel": 0.05,
    "warning_wall": 0.05,
    "warning_door": 0.034,
}

write_with("warning_gate", warning_gate, GATE_AREA, {"lamp": (1.72, 1.86, 0.11)})
write_with("warning_hoarding", warning_hoarding, HOARDING_AREA)
write_with("warning_enamel", warning_enamel, ENAMEL_AREA)
write_with("warning_wall", warning_wall, WALL_AREA)
write_with("warning_door", warning_door, DOOR_AREA)
