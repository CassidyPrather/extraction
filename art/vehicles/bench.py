"""The workbench in the truck bed: a plank top on painted steel angle legs,
braced, a shelf under it, a sheet-steel tray on top, a blue bench vise on
the front corner, a tool rack at the back, and steel plates round the
selector knob and the run lever. Writes assets/models/vehicles/bench.glb.

Bench space as in src/raid/bench.rs: origin at `BENCH_AT`, the bench
0.7 x 0.8 x 1.5 (X across the bed, Z along it), its top at y 0.4 and the
bed floor at y -0.4; -X faces the aisle. The tray's box (`TRAY_HALF`
0.26 x 0.08 x 0.42) sits on the top; the knob (BenchSelect) and lever
(BenchRun) are drawn by the game at (-0.33, 0.32, -0.55) and
(-0.33, 0.32, 0.6), and the display hangs on the canopy wall above.

- `bench`  everything (static)

Materials: Boards, Paint, Iron, Steel, Blue, Black, Rubber, Wood, Brass.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import RX, RY, RZ, T, Part, export, fillet, main_guard, make, report
from _kit import text  # noqa: E402

main_guard()
random.seed(7)

HX, HZ = 0.35, 0.75     # half the footprint
TOP = 0.4
FLOOR = -0.4
ALONG_X = RY(math.pi / 2)
UP = RX(-math.pi / 2)

b = Part("bench")

# --- Top: four thick planks along the bench, a steel nosing on the aisle edge
n = 4
pw = 2 * HX / n
for i in range(n):
    x = -HX + pw * (i + 0.5)
    b.box((pw - 0.006, 0.05, 2 * HZ + 0.02), (x, TOP - 0.025 + random.uniform(-0.002, 0.0),
                                                 0.0), "Boards", bevel=0.005, segs=1)
b.box((0.012, 0.06, 2 * HZ + 0.03), (-HX - 0.002, TOP - 0.032, 0.0), "Paint", bevel=0.003, segs=1)
# Bolt heads through the top into the frame.
for x in (-HX + 0.04, HX - 0.04):
    for z in (-HZ + 0.04, -0.2, 0.25, HZ - 0.04):
        b.cyl(0.008, TOP - 0.001, TOP + 0.003, "Iron", segs=8, xf=T(x, 0, z) @ UP)

# --- Frame: steel angle legs, rails round the top and the shelf, braces --
def angle(p, length, at, rot, w=0.045, t=0.005, mat="Paint"):
    """An L-section steel angle along local +Y from `at`, flanges along +X
    and +Z, then turned by `rot`."""
    xf = T(*at) @ rot
    p.box((w, length, t), (w / 2, length / 2, t / 2), mat, xf=xf)
    p.box((t, length, w), (t / 2, length / 2, w / 2 + t), mat, xf=xf)


LEG = TOP - 0.05 - FLOOR
for sx in (-1, 1):
    for sz in (-1, 1):
        # Flanges facing out, so the corner is the leg's outside edge.
        rot = RY({(-1, -1): 0.0, (1, -1): -math.pi / 2, (1, 1): math.pi,
                  (-1, 1): math.pi / 2}[(sx, sz)])
        angle(b, LEG, (sx * (HX - 0.02), FLOOR, sz * (HZ - 0.02)), rot)
        # Feet bolted to the bed.
        b.box((0.08, 0.008, 0.08), (sx * (HX - 0.04), FLOOR + 0.004, sz * (HZ - 0.04)), "Iron")
for y in (TOP - 0.08, FLOOR + 0.12):
    for sx in (-1, 1):
        b.box((0.008, 0.04, 2 * HZ - 0.06), (sx * (HX - 0.035), y, 0.0), "Paint")
    for sz in (-1, 1):
        b.box((2 * HX - 0.06, 0.04, 0.008), (0.0, y, sz * (HZ - 0.035)), "Paint")
# X braces across both ends.
for sz in (-1, 1):
    zb = sz * (HZ - 0.052)  # inside the end rails, clear of them
    h = (TOP - 0.1) - (FLOOR + 0.14)
    ln = math.hypot(2 * HX - 0.08, h)
    a = math.atan2(h, 2 * HX - 0.08)
    for k, sgn in enumerate((1, -1)):
        b.box((ln, 0.03, 0.006), (0.0, 0.0, 0.0), "Paint",
              xf=T(0.0, (TOP - 0.1 + FLOOR + 0.14) / 2, zb - sz * 0.012 * k) @ RZ(sgn * a))
# The shelf: slats on the lower rails.
for k in range(6):
    x = -HX + 0.08 + k * (2 * HX - 0.16) / 5
    b.box((0.08, 0.02, 2 * HZ - 0.08), (x, FLOOR + 0.15, 0.0), "Boards", bevel=0.003, segs=1)

# --- The aisle apron: a steel sheet under the top, with the plates the knob
# and the lever stand in.
b.box((0.006, 0.12, 2 * HZ - 0.06), (-HX + 0.03, TOP - 0.11, 0.0), "Paint")
for z, label in ((-0.55, "ВЫБОР"), (0.6, "ПУСК")):
    b.box((0.006, 0.16, 0.16), (-HX + 0.026, 0.32, z), "Iron", bevel=0.002, segs=1)
    # The plate's label, white stencil above the control.
    text(b, label, 0.016, "Enamel", T(-HX + 0.023, 0.385, z) @ RY(-math.pi / 2), depth=0.0008)
# Detents round the knob: five ticks.
for k in range(5):
    a = math.radians(-60 + 30 * k)
    b.box((0.004, 0.012, 0.004), (0.0, 0.0, 0.0), "Enamel",
          xf=T(-HX + 0.021, 0.32, -0.55) @ RX(a) @ T(0.0, 0.052, 0.0))
# The lever's slot.
b.box((0.004, 0.11, 0.022), (-HX + 0.021, 0.33, 0.6), "Black")

# --- Tray: sheet steel with rolled lips and two handles -------------------
TX, TZ = 0.27, 0.43
TY = TOP + 0.005
b.box((2 * TX, 0.006, 2 * TZ), (0.0, TY, 0.0), "Iron", bevel=0.002, segs=1)
for sx in (-1, 1):
    b.box((0.006, 0.05, 2 * TZ), (sx * TX, TY + 0.025, 0.0), "Iron", bevel=0.002, segs=1)
    b.cyl(0.006, -TZ, TZ, "Iron", segs=8, xf=T(sx * TX, TY + 0.05, 0.0))
for sz in (-1, 1):
    b.box((2 * TX - 0.012, 0.05, 0.006), (0.0, TY + 0.025, sz * TZ), "Iron", bevel=0.002, segs=1)
    b.cyl(0.006, -TX, TX, "Iron", segs=8, xf=T(0.0, TY + 0.05, sz * TZ) @ ALONG_X)
    # Wire handles.
    b.sweep([(sz * (TZ + 0.002), TY + 0.03), (sz * (TZ + 0.03), TY + 0.035),
             (sz * (TZ + 0.03), TY + 0.045), (sz * (TZ + 0.002), TY + 0.05)],
            [(0.005 * math.cos(a), 0.005 * math.sin(a)) for a in
             [2 * math.pi * i / 6 for i in range(6)]], "Steel",
            xf=T(0.0, 0.0, 0.0) @ T(-0.08, 0, 0))
    b.sweep([(sz * (TZ + 0.002), TY + 0.03), (sz * (TZ + 0.03), TY + 0.035),
             (sz * (TZ + 0.03), TY + 0.045), (sz * (TZ + 0.002), TY + 0.05)],
            [(0.005 * math.cos(a), 0.005 * math.sin(a)) for a in
             [2 * math.pi * i / 6 for i in range(6)]], "Steel", xf=T(0.08, 0, 0))

# --- Bench vise on the front corner (aisle side, -Z end) -------------------
VX, VZ = -HX + 0.075, -HZ + 0.1
vy = TOP
# Base and swivel plate.
b.cyl(0.055, vy, vy + 0.012, "Blue", segs=20, xf=T(VX, 0, VZ) @ UP, chamfer=0.003)
# Body: the fixed jaw block and the slide.
b.box((0.085, 0.06, 0.13), (VX, vy + 0.042, VZ + 0.01), "Blue", bevel=0.008, segs=2)
b.box((0.09, 0.075, 0.04), (VX, vy + 0.058, VZ - 0.05), "Blue", bevel=0.008, segs=2)
# Moving jaw, out toward the aisle end, its slide bar.
b.box((0.09, 0.075, 0.04), (VX, vy + 0.058, VZ - 0.115), "Blue", bevel=0.008, segs=2)
b.box((0.05, 0.035, 0.08), (VX, vy + 0.045, VZ - 0.075), "Iron", bevel=0.004, segs=1)
# Hardened jaw plates with cross-hatched faces.
for z in (VZ - 0.072, VZ - 0.093):
    b.box((0.092, 0.03, 0.006), (VX, vy + 0.08, z), "Iron", bevel=0.001, segs=1)
# Screw and T-handle with its ball ends.
b.cyl(0.009, VZ - 0.2, VZ - 0.13, "Steel", segs=12, xf=T(VX, vy + 0.035, 0.0))
b.cyl(0.006, -0.08, 0.08, "Steel", segs=10, xf=T(VX, vy + 0.035, VZ - 0.19) @ ALONG_X)
for sx in (-1, 1):
    b.sphere(0.011, (VX + sx * 0.082, vy + 0.035, VZ - 0.19), "Steel", segs=12, rings=6)
# Anvil face on the back of the fixed jaw.
b.box((0.06, 0.012, 0.05), (VX, vy + 0.078, VZ + 0.045), "Iron", bevel=0.002, segs=1)

# --- Tool rack at the back, under the display --------------------------------
RX_ = HX - 0.015
b.box((0.02, 0.26, 0.9), (RX_, TOP + 0.16, 0.12), "Boards", bevel=0.004, segs=1)
for z in (-0.355, 0.595):
    b.box((0.03, 0.3, 0.04), (RX_, TOP + 0.15, z), "Paint", bevel=0.004, segs=1)
# Spanners hanging on pegs, smallest to largest.
for k in range(5):
    z = -0.22 + k * 0.07
    L = 0.12 + k * 0.025
    b.cyl(0.004, RX_ - 0.03, RX_ - 0.01, "Iron", segs=6, xf=T(0, TOP + 0.24, z) @ ALONG_X)
    b.box((0.006, L, 0.014 + k * 0.002), (RX_ - 0.022, TOP + 0.24 - L / 2 + 0.01, z), "Steel",
          bevel=0.002, segs=1)
    b.cyl(0.013 + k * 0.002, -0.003, 0.003, "Steel", segs=12,
          xf=T(RX_ - 0.022, TOP + 0.24 - L + 0.01, z) @ ALONG_X)
# A hammer and a pair of pliers hung by their handles.
b.box((0.022, 0.24, 0.03), (RX_ - 0.03, TOP + 0.13, 0.25), "Wood", bevel=0.006, segs=1)
b.box((0.03, 0.03, 0.1), (RX_ - 0.03, TOP + 0.02, 0.25), "Iron", bevel=0.004, segs=1)
b.cyl(0.004, RX_ - 0.03, RX_ - 0.01, "Iron", segs=6, xf=T(0, TOP + 0.24, 0.25) @ ALONG_X)
for sz in (-1, 1):
    b.box((0.008, 0.16, 0.012), (RX_ - 0.024, TOP + 0.15, 0.4 + sz * 0.012), "Black",
          bevel=0.003, segs=1, xf=RX(sz * 0.08, about=(RX_ - 0.024, TOP + 0.24, 0.4)))
b.cyl(0.004, RX_ - 0.03, RX_ - 0.01, "Iron", segs=6, xf=T(0, TOP + 0.24, 0.4) @ ALONG_X)

# --- On the shelf: a tin of bolts and a coil of wire ----------------------
SY = FLOOR + 0.16
b.lathe([(0.0, SY), (0.055, SY), (0.055, SY + 0.11), (0.051, SY + 0.11), (0.051, SY + 0.006),
         (0.0, SY + 0.006)], "Iron", segs=20, xf=T(-0.12, 0, 0.45) @ UP)
for k in range(9):
    a = 2 * math.pi * k / 9
    r = 0.025 * (k % 3) / 2
    b.cyl(0.006, SY + 0.085, SY + 0.098, "Steel", segs=6,
          xf=T(-0.12 + r * math.cos(a), 0, 0.45 + r * math.sin(a)) @ UP)
for k in range(5):
    b.lathe([(0.075, -0.003), (0.079, 0.0), (0.075, 0.003), (0.071, 0.0)], "Copper", segs=24,
            xf=T(0.1, SY + 0.012 + k * 0.006, -0.35) @ UP, closed=True)

make(b, worn={"mats": ("Paint", "Blue"), "amount": 0.4})
report()
export("assets/models/vehicles/bench.glb")
