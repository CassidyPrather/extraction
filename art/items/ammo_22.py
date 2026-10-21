"""5.6 mm (.22 LR) rimfire ammunition for the TOZ-8M: one loose round and the
small card box of fifty. Writes assets/models/items/round_22lr.glb and
assets/models/items/ammo_22lr.glb (item space: see _kit.py).

The round: a short straight brass case with a wide rim and no primer (it
fires on the rim), a greased lead round-nosed bullet as wide as the case.
The box: a Soviet slide box like a big matchbox, a white sleeve printed
with a blue band and the calibre, its card tray of fifty standing on their
noses in five rows of ten pushed half out, the brass heads showing.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ON_TOP, UP, centred, item, panel, text, wanted
from common import Part, RY, T

# (r, z), the base at z 0 and the bullet to -z.
RIM, CASE, LENGTH = 0.00345, 0.00285, 0.025
CASE_PROF = [(0.0, 0.0), (RIM - 0.0002, 0.0), (RIM, -0.0002), (RIM, -0.0009), (CASE + 0.0002, -0.0011),
             (CASE, -0.0013), (CASE, -0.0154), (CASE - 0.0002, -0.0156), (0.0, -0.0156)]
# Heeled: the bullet's exposed part is the case's own width, a lube groove
# round it, then the round nose.
BULLET_PROF = [(0.0, -0.0150), (0.0028, -0.0150), (0.0028, -0.0166), (0.00255, -0.0170),
               (0.00255, -0.0176), (0.0028, -0.0180), (0.0028, -0.0192), (0.0026, -0.0210),
               (0.0021, -0.0228), (0.0013, -0.0242), (0.0005, -0.0249), (0.0, -LENGTH)]


def round_22lr():
    p = Part("body")
    xf = T(0.0, 0.0, LENGTH / 2)
    p.lathe(CASE_PROF, "Brass", segs=16, xf=xf)
    p.lathe(BULLET_PROF, "Zinc", segs=16, xf=xf)
    # The rim's crimp ring on the head, where the firing pin strikes.
    p.tube(RIM - 0.0006, RIM - 0.0011, -0.0002, 0.00008, "Tarnish", segs=16, xf=xf)
    return centred([p])


def ammo_22lr():
    p = Part("body")
    sx, sy = 0.075, 0.031            # the sleeve's width and height
    wall = 0.001
    sl0, sl1 = -0.010, 0.029         # the sleeve's front and back
    tr0, tr1 = -0.026, 0.013         # the tray's front and back
    # The sleeve: white card printed with a blue band, open at both ends.
    yt = sy - wall
    panel(p, "y", -sx / 2, sx / 2, sl0, sl1, yt, sy, "Paper",
          [(-sx / 2, sx / 2, 0.0195, 0.0255, "Blue")])
    p.box((sx, wall, sl1 - sl0), (0.0, wall / 2, (sl0 + sl1) / 2), "Paper", bevel=0.0003, segs=1)
    for s in (-1, 1):
        a, b = sorted((s * (sx / 2 - wall), s * sx / 2))
        panel(p, "x", sl0, sl1, wall, yt, a, b, "Paper", [(sl0, sl1, 0.020, 0.026, "Blue")])
    # The tray: kraft card, its sides only where it's out of the sleeve (in
    # it, the sleeve's own walls are what you see), standing on the table
    # in front and on the sleeve's floor inside.
    th = 0.024
    xi = sx / 2 - wall                # the tray's outer half width: the sleeve's inside
    p.box((2 * xi, 0.002, sl0 - tr0), (0.0, 0.001, (tr0 + sl0) / 2), "Rope", segs=1)
    for s in (-1, 1):
        p.box((wall, th - 0.002, sl0 - tr0), (s * (xi - wall / 2), 0.002 + (th - 0.002) / 2,
                                              (tr0 + sl0) / 2), "Rope", segs=1)
    for z in (tr0 + wall / 2, tr1 - wall / 2):
        p.box((2 * xi - 0.0002, th - 0.002, wall), (0.0, 0.002 + (th - 0.002) / 2, z), "Rope",
              segs=1)
    # The dark between the rounds, under their heads.
    head = 0.0295                     # the tops of the rims
    p.box((2 * xi - 0.002, 0.002, tr1 - tr0 - 2 * wall), (0.0, head - 0.0065, (tr0 + tr1) / 2),
          "Black", segs=1)
    # Fifty heads, five rows of ten: the rim and the top of the case.
    pitch_x, pitch_z = 0.0070, 0.0072
    head_prof = [(0.0, -0.0062), (CASE, -0.0062)] + CASE_PROF[1:6][::-1] + [(0.0, 0.0)]
    hidden_prof = [(0.0, -0.0062), (CASE, -0.0062), (CASE, -0.0011), (RIM, -0.0009), (RIM, 0.0),
                   (0.0, 0.0)]
    for i in range(10):
        for j in range(5):
            x = (i - 4.5) * pitch_x
            z = tr0 + wall + 0.0004 + RIM + j * pitch_z
            xf = T(x, head, z) @ UP
            if z + RIM > sl0:
                # Under the sleeve: seen only edge-on through its open back.
                p.lathe(hidden_prof, "Brass", segs=6, xf=xf)
            else:
                p.lathe(head_prof, "Brass", segs=10, xf=xf)
    # The calibre in black on the sleeve's top and its sides; the count.
    text(p, "5.6", 0.011, "Black", T(0.0, sy, 0.0075) @ ON_TOP, depth=0.0004)
    text(p, "50 ПАТРОНОВ", 0.0048, "Black", T(0.0, sy, -0.0045) @ ON_TOP, depth=0.0004)
    for s in (-1, 1):
        text(p, "5.6", 0.008, "Black", T(s * sx / 2, 0.0125, (sl0 + sl1) / 2) @ RY(s * 1.5707963),
             depth=0.0004)
    return centred([p])


if __name__ == "__main__":
    for item_id, fn in (("round_22lr", round_22lr), ("ammo_22lr", ammo_22lr)):
        if wanted(item_id):
            item(item_id, fn)
