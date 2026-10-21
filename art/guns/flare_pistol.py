"""SPSh-44: the Soviet 26 mm signal (flare) pistol, a single-shot break-open
with an exposed hammer. Writes assets/models/guns/flare_pistol.glb.

Gun space: origin at the palm on the grip, -Z down the bore (bore axis
y = 0.055), +Y up, metres. The `barrels` object (barrel and its hinge lug)
has its origin on the hinge pin under the breech, which the game tips it
down about (about 0.8 rad open). The `hammer` turns back about its pin
(its origin, local X) when it is thumbed to full cock.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, T, arc, empty, export, fillet, join, main_guard, make, report,
                    section_circle, section_rect, catmull)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

BORE_Y = 0.055
HINGE = (0.0, 0.032, -0.065)
HZ, HY = HINGE[2], HINGE[1]
BREECH, MUZZLE = -0.035, -0.19
R_OUT = 0.0185        # breech end of the barrel
R_FWD = 0.0165        # the rest of it
R_BORE = 0.01335      # 26.7 mm: takes the 26.5 mm cartridge
SHIELD = -0.0353      # the standing breech's face, just behind the barrel
FRAME_HW = 0.0125
HAMMER_PIN = (0.0, 0.06, 0.0)
ON_BORE = T(0, BORE_Y, 0)

# --- Barrel (tips down about the hinge) ------------------------------------------------------
bar = Part("barrels", origin=HINGE)
# A plain smooth-bore tube, thicker over the chamber, a raised band at the
# muzzle; the rim recess counterbored in the breech face.
bar.lathe([(R_BORE, MUZZLE + 0.0015), (R_BORE, BREECH - 0.0018), (0.0151, BREECH - 0.0018),
           (0.0151, BREECH), (R_OUT - 0.0006, BREECH), (R_OUT, BREECH - 0.0006),
           (R_OUT, -0.0735), (R_FWD + 0.0002, -0.0765), (R_FWD, -0.0775), (R_FWD, -0.1805),
           (R_FWD + 0.001, -0.1815), (R_FWD + 0.001, MUZZLE + 0.0006), (R_FWD + 0.0004, MUZZLE),
           (R_BORE + 0.0008, MUZZLE)], "Blued", segs=32, xf=ON_BORE, closed=True)
# The hinge lug under the breech: a fork tongue round the pin, its front
# concentric with it so it turns in the frame's knuckle.
LUG_R = 0.0055
lug = ([(SHIELD - 0.0002, 0.040), (SHIELD - 0.0002, 0.0295), (-0.046, 0.0268)] +
       [(HZ + LUG_R * math.cos(math.radians(a)), HY + LUG_R * math.sin(math.radians(a)))
        for a in range(270, 89, -20)] + [(-0.058, 0.0395)])
lug = fillet(lug, [0.0, 0.0012, 0.003] + [0.0] * 10 + [0.0], 2)
bar.prism(lug, -0.0062, 0.0062, "Blued", axis="x", bevel=0.0007, segs=1)
# Extractor: a sliding block in the right of the breech face, its claw
# over the rim recess.
bar.prism(fillet([(BREECH - 0.016, BORE_Y - 0.004), (BREECH - 0.0004, BORE_Y - 0.004),
                  (BREECH - 0.0004, BORE_Y + 0.004), (BREECH - 0.016, BORE_Y + 0.004)],
                 0.0012, 2), 0.0128, 0.0188, "Blued", axis="x", bevel=0.0006, segs=1)

# --- Frame --------------------------------------------------------------------------------------
frame = Part("frame")
# Side outline: under the barrel the knuckle round the hinge pin, the
# trigger opening, the raked grip frame, and the tang under the hammer.
knuckle = [(HZ + 0.0085 * math.cos(math.radians(a)), HY + 0.0085 * math.sin(math.radians(a)))
           for a in range(40, 266, 22)]
prof = ([(SHIELD, 0.066), (SHIELD, 0.0375)] + knuckle +
        [(-0.050, 0.0225), (-0.017, 0.0225),
         (-0.0145, 0.0195),   # front strap
         (-0.0005, -0.063),
         (0.0035, -0.068),    # butt
         (0.038, -0.0695),
         (0.0425, -0.064),
         (0.029, 0.022),      # back strap
         (0.0285, 0.040),
         (0.020, 0.0555),
         (0.008, 0.066)])
radii = ([0.002, 0.0015] + [0.0] * len(knuckle) +
         [0.003, 0.002, 0.003, 0.006, 0.005, 0.005, 0.005, 0.008, 0.006, 0.006, 0.004])
frame.prism(fillet(prof, radii, 3), -FRAME_HW, FRAME_HW, "Blued", axis="x", bevel=0.0016,
            segs=2)
# The breech shield: round, as wide as the barrel, the hammer's housing
# narrowing behind it to the frame's tang.
frame.lathe([(0.0, SHIELD), (0.0189, SHIELD), (0.0195, SHIELD + 0.0007), (0.0195, -0.0135),
             (0.0185, -0.008), (0.0158, -0.002), (0.012, 0.004), (0.0, 0.006)], "Blued", segs=32,
            xf=ON_BORE)

cut = Part("cut")
# The fork the barrel's lug drops into, open to the front.
cut.box((0.0152, 0.03, 0.05), (0.0, 0.0245 + 0.015, SHIELD - 0.025 + 0.0008), "Blued")
# The hammer's slot through the top of the housing.
cut.box((0.0096, 0.05, 0.06), (0.0, 0.046 + 0.025, -0.014 + 0.03), "Blued")
# The "sight": a groove along the top of the shield.
cut.box((0.0024, 0.004, 0.024), (0.0, BORE_Y + 0.0195, -0.0235), "Blued", bevel=0.0)
# Firing pin hole in the breech face.
cut.cyl(0.0021, SHIELD - 0.002, SHIELD + 0.003, "Blued", segs=12, xf=ON_BORE)
# The model and year struck on the right of the frame under the barrel.
RIGHT = T(FRAME_HW - 0.0006, 0, 0) @ RY(math.pi / 2)
text(cut, "СПШ-44", 0.0036, "Blued", RIGHT @ T(0.0465, 0.031, 0.0), depth=0.0012)
text(cut, "1944", 0.003, "Blued", RIGHT @ T(0.0465, 0.0255, 0.0), depth=0.0012)
frame.cut(cut)

pins = Part("pins")
# Hinge pin and hammer pin, their heads on both sides of the frame.
for (py, pz) in ((HY, HZ), (HAMMER_PIN[1], HAMMER_PIN[2])):
    pins.cyl(0.0028, -FRAME_HW - 0.0008, FRAME_HW + 0.0008, "Steel", segs=14,
             xf=T(0, py, pz) @ RY(math.pi / 2), chamfer=0.0004)
    for sx in (-1, 1):
        pins.cyl(0.0042, sx * (FRAME_HW - 0.0004), sx * (FRAME_HW + 0.0011), "Steel", segs=16,
                 xf=T(0, py, pz) @ RY(math.pi / 2), chamfer=0.0005)
# The firing pin's tip, just inside its hole.
pins.cyl(0.0016, SHIELD + 0.0006, SHIELD + 0.006, "Steel", segs=10, xf=ON_BORE)

# Trigger guard: a stamped loop from under the knuckle back to the strap.
guard = Part("guard")
guard_path = catmull([(-0.0555, 0.0238), (-0.0575, 0.014), (-0.054, 0.0035), (-0.045, -0.0032),
                      (-0.032, -0.0055), (-0.020, -0.0038), (-0.0105, 0.0012)], samples=4,
                     closed=False)
guard.sweep(guard_path, section_rect(0.0085, 0.0034, 0.0012), "Blued")

# Barrel latch: a flat lever on the left of the frame, its serrated thumb
# piece in front of the grip; pressing it lets the barrel drop.
latch = Part("latch")
lp = fillet([(-0.056, 0.0262), (-0.022, 0.0258), (-0.0175, 0.0255), (-0.0105, 0.0285),
             (-0.0095, 0.0345), (-0.013, 0.0368), (-0.019, 0.032), (-0.024, 0.0312),
             (-0.056, 0.0312)], [0.0022, 0.0, 0.002, 0.002, 0.0015, 0.0015, 0.002, 0.0, 0.0022], 3)
latch.prism(lp, -FRAME_HW - 0.0022, -FRAME_HW + 0.0003, "Blued", axis="x", bevel=0.0005, segs=1)
for i in range(4):
    z = -0.0168 + i * 0.0018
    latch.box((0.0012, 0.0045, 0.0007), (-FRAME_HW - 0.0022, 0.0315 + i * 0.0006, z), "Blued",
              xf=RX(0.6, about=(-FRAME_HW - 0.0022, 0.0315, z)))
latch.cyl(0.0024, -FRAME_HW - 0.0029, -FRAME_HW - 0.0016, "Steel", segs=12,
          xf=T(0, 0.0287, -0.046) @ RY(math.pi / 2), chamfer=0.0004)

# --- Grip: black Bakelite panels on both sides --------------------------------------------------
gp = fillet([(-0.0108, 0.0185), (0.0026, -0.0595), (0.006, -0.0645), (0.0355, -0.0658),
             (0.0385, -0.061), (0.0258, 0.0205), (0.0225, 0.0262), (-0.004, 0.0262)],
            [0.004, 0.004, 0.004, 0.004, 0.004, 0.004, 0.004, 0.004], 4)


def swell(y):
    return 0.0012 * math.exp(-((y + 0.02) / 0.03) ** 2)


grip = Part("grip")
grip.prism(gp, 0, 0, "Bakelite", axis="x", bevel=0.0026, segs=3,
           width=lambda z, y: (FRAME_HW - 0.0015, 0.0178 + swell(y)))
grip.prism(gp, 0, 0, "Bakelite", axis="x", bevel=0.0026, segs=3,
           width=lambda z, y: (-0.0178 - swell(y), -FRAME_HW + 0.0015))
# The grip screw through both panels, near the top.
for sx in (-1, 1):
    grip.sphere(0.0046, (sx * 0.0178, 0.0105, 0.012), "Steel", segs=16, rings=8,
                scale=(0.3, 1, 1))


def span(y):
    zs = []
    n = len(gp)
    for i in range(n):
        (z0, y0), (z1, y1) = gp[i], gp[(i + 1) % n]
        if (y0 - y) * (y1 - y) <= 0 and y0 != y1:
            zs.append(z0 + (z1 - z0) * (y - y0) / (y1 - y0))
    return (min(zs), max(zs)) if len(zs) >= 2 else None


def grip_x(z, y):
    s = span(y)
    if s is None or not s[0] + 0.0045 < z < s[1] - 0.0045:
        return None
    return 0.0178 + swell(y)


grip_cut = Part("cut")
grip_check = Part("checkering")
grip_check.checkering(grip_cut, grip_x, None, None, (-0.006, 0.036, -0.0585, 0.003),
                      pitch=0.0024, depth=0.0005, mat="Bakelite")
grip.cut(grip_cut)

# Lanyard ring: a swivel under the butt, the ring hanging behind.
lan = Part("lanyard")
lan.box((0.006, 0.006, 0.008), (0.0, -0.0705, 0.024), "Steel", bevel=0.0012)
lan.cyl(0.0014, -0.004, 0.004, "Steel", segs=10, xf=T(0, -0.0718, 0.024) @ RY(math.pi / 2))
ring = [(0.024 + 0.0095 * math.sin(a), -0.0815 + 0.0095 * math.cos(a))
        for a in [2 * math.pi * i / 18 for i in range(18)]]
lan.sweep(ring, section_circle(0.0014, 8), "Steel", closed=True)

# --- Hammer (turns back about its pin) ---------------------------------------------------------
hammer = Part("hammer", origin=HAMMER_PIN)
hp = fillet([(-0.0128, 0.0515), (-0.0128, 0.064), (-0.008, 0.070), (0.004, 0.079),
             (0.016, 0.087), (0.027, 0.0915), (0.0345, 0.0918), (0.0365, 0.088),
             (0.031, 0.0842), (0.021, 0.0782), (0.0145, 0.0695), (0.0115, 0.060),
             (0.0085, 0.0535), (0.0035, 0.0492), (-0.006, 0.0482)],
            [0.0008, 0.0012, 0.003, 0.006, 0.006, 0.004, 0.003, 0.0018, 0.003, 0.006, 0.006,
             0.005, 0.004, 0.003, 0.002], 3)
hammer.prism(hp, -0.0045, 0.0045, "Blued", axis="x", bevel=0.0009, segs=1)
# The broad spur you thumb, grooved across.
spur = fillet([(0.019, 0.0885), (0.027, 0.0915), (0.0345, 0.0918), (0.0365, 0.088),
               (0.031, 0.0842), (0.022, 0.0805)], [0.002, 0.003, 0.003, 0.0018, 0.003, 0.002], 2)
hammer.prism(spur, -0.0068, 0.0068, "Blued", axis="x", bevel=0.0013, segs=2)
spur_cut = Part("cut")
for i in range(6):
    z = 0.0225 + i * 0.0023
    y = 0.0898 + min(z - 0.0225, 0.0045) * 0.38 - max(0.0, z - 0.032) * 0.6
    spur_cut.box((0.016, 0.0022, 0.0010), (0.0, y, z), "Blued")
hammer.cut(spur_cut)

# --- Trigger ------------------------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, 0.0205, -0.027))
trig.sweep(catmull([(-0.0265, 0.0238), (-0.0292, 0.016), (-0.0305, 0.0095), (-0.0285, 0.0042),
                    (-0.0245, 0.0012)], samples=3, closed=False),
           section_rect(0.0052, 0.0034, 0.0012), "Blued")

# --- Build ----------------------------------------------------------------------------------------
join("barrels", bar, worn={})
join("static_frame", frame, pins, guard, latch, grip, grip_check, lan, worn={})
make(hammer, worn={})
make(trig, worn={})

empty("pt_muzzle", (0.0, BORE_Y, MUZZLE))
empty("pt_sight", (0.0, BORE_Y + 0.0195, -0.012))
empty("pt_hinge", HINGE)
empty("pt_load_port", (0.0, BORE_Y, -0.04))
empty("pt_port", (0.0, BORE_Y, -0.04))

report()
export("assets/models/guns/flare_pistol.glb")
