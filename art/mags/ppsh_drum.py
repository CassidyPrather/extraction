"""PPSh-41 drum: 71 rounds of 7.62x25 on a spiral track, a feed tower on
top. Writes assets/models/mags/ppsh_drum.glb.

Item space: origin at the centre of the item box (0.072 x 0.18 x 0.127 m,
see assets/data/items.ron); the tower's top is the box's top (the gun's
`magwell` seats it there), the drum's axis runs across the gun (X).

A curved inspection window round the right face shows the spiral's outer
turns: the rounds in it are `round_N` for N from 6 up to 71, so the game
(show_mag_rounds) empties the window a few at a time as the drum runs
down, and shows the fill at a glance.

- `static_mag`  the drum shell, its front cover and winding key, the
                tower, the window frame
- `round_N`     the rounds seen in the window
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import RX, RY, RZ, T, Part, export, fillet, join, main_guard, make, report
from ammo import C762X25, cartridge  # noqa: E402
from _kit import text  # noqa: E402

main_guard()

TOP = 0.09
R = 0.062                       # drum radius
HW = 0.023                      # half its thickness
CY = TOP - 0.052 - R            # drum centre
ALONG_X = RY(math.pi / 2)

body = Part("body")
# The shell: a short cylinder across the gun, rolled rims on both faces.
body.lathe([(0.0, -HW), (R - 0.004, -HW), (R, -HW + 0.004), (R, HW - 0.004), (R - 0.004, HW),
            (0.0, HW)], "Blued", segs=48, xf=T(0, CY, 0) @ ALONG_X)
for sx in (-1, 1):
    body.lathe([(R - 0.0035, -0.0016), (R + 0.0012, -0.0012), (R + 0.0012, 0.0012),
                (R - 0.0035, 0.0016)], "Blued", segs=48,
               xf=T(sx * (HW - 0.0045), CY, 0) @ ALONG_X, closed=True)
# The left (outer) face: a pressed cover, its ribs, the winding key and
# its latch.
cover = T(-HW, CY, 0) @ RY(-math.pi / 2)
body.lathe([(0.0, 0.0), (R - 0.008, 0.0), (R - 0.01, 0.0035), (0.03, 0.005), (0.0, 0.006)],
           "Blued", segs=40, xf=cover)
for k in range(6):
    body.box((0.003, 0.016, 0.002), (0.0, 0.0, 0.0), "Blued",
             xf=cover @ RZ(math.pi * k / 3) @ T(0.0, 0.04, 0.0045))
body.lathe([(0.0, 0.004), (0.011, 0.004), (0.011, 0.008), (0.0085, 0.01), (0.0, 0.01)], "Steel",
           segs=20, xf=cover)
body.box((0.026, 0.006, 0.004), (0.0, 0.0, 0.011), "Steel", bevel=0.0015, xf=cover)
body.box((0.012, 0.006, 0.01), (0.0, 0.0, 0.0), "Blued", bevel=0.0015,
         xf=T(-HW - 0.002, CY - R + 0.004, 0.0))
# The feed tower: a box up from the drum's top, the lips at its top.
tower = fillet([(-0.0105, CY + R - 0.01), (0.0105, CY + R - 0.01), (0.0105, TOP - 0.004),
                (0.006, TOP), (-0.006, TOP), (-0.0105, TOP - 0.004)],
               [0.002, 0.002, 0.003, 0.002, 0.002, 0.003], 2)
body.prism(tower, -0.024, 0.014, "Blued", axis="z", bevel=0.0012, segs=1)
# The lips' plate, within the tower's length (it ran 5 mm past its back
# into the gun's housing: `capture::seated`).
body.box((0.022, 0.004, 0.026), (0.0, TOP - 0.006, 0.0), "Blued", bevel=0.001)
body.box((0.016, 0.01, 0.006), (0.0, TOP - 0.012, -0.018), "Blued", bevel=0.0015)

# The window: a curved slot round the right face, over the outer turn.
WIN_R0, WIN_R1 = R - 0.017, R - 0.006
A0, A1 = 200.0, 520.0           # degrees, round from the bottom front
cut = Part("cut")
n = 40
for i in range(n):
    a = math.radians(A0 + (A1 - A0) * (i + 0.5) / n)
    rr = (WIN_R0 + WIN_R1) / 2
    seg = 2 * math.pi * rr * (A1 - A0) / 360 / n + 0.0025
    cut.box((0.01, WIN_R1 - WIN_R0, seg), (0.0, 0.0, 0.0), "Blued",
            xf=T(HW, CY + rr * math.sin(a), rr * math.cos(a)) @ RX(-(a + math.pi / 2)))
body.cut(cut)
text(body, "71", 0.009, "Enamel", T(HW + 0.0003, CY - 0.018, 0.018) @ RY(math.pi / 2),
     depth=0.0004)

# The rounds seen in the window, lying across the drum, their heads out.
rounds = []
K = 14
for k in range(K):
    n_ = round(71 * (k + 1) / K)
    a = math.radians(A0 + (A1 - A0) * (k + 0.5) / K)
    rr = (WIN_R0 + WIN_R1) / 2
    r = Part(f"round_{n_}")
    cartridge(r, C762X25, T(HW - 0.0018, CY + rr * math.sin(a), rr * math.cos(a)) @ ALONG_X
, segs=10, base=True)
    rounds.append(r)

for r in rounds:
    make(r)
join("static_mag", body, worn={})
report()
export("assets/models/mags/ppsh_drum.glb")
