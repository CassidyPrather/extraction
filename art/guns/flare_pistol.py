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
# Measured on a Type 57 (the Chinese copy) side view scaled to 220 mm, on
# the grip at the palm's height (docs/craft/guns/flare_pistol).
HINGE = (0.0, 0.028, -0.0755)   # forward, at the front of the frame's side plate
HZ, HY = HINGE[2], HINGE[1]
BREECH = -0.012                 # over the grip
MUZZLE = BREECH - 0.150         # a 150 mm barrel
STEP = -0.087                   # where the barrel's thick breech half ends
R_OUT = 0.0164        # the breech half
R_FWD = 0.0147        # the front half: a thin tube, 29.3 mm
R_BORE = 0.01335      # 26.7 mm: takes the 26.5 mm cartridge
SHIELD = BREECH - 0.0003  # the standing breech's face, just behind the barrel
FRAME_HW = 0.0125
PLATE_Y = 0.0155      # the side plate's bottom, 39.5 mm under the bore
LUG_Y = 0.0378        # the barrel's lug sits on the plate, a seam between
HAMMER_PIN = (0.0, 0.042, 0.018)
ON_BORE = T(0, BORE_Y, 0)

# --- Barrel (tips down about the hinge) ------------------------------------------------------
bar = Part("barrels", origin=HINGE)
# A plain smooth-bore tube, thicker over its breech half; the rim recess
# counterbored in the breech face.
bar.lathe([(R_BORE, MUZZLE + 0.0015), (R_BORE, BREECH - 0.0018), (0.0151, BREECH - 0.0018),
           (0.0151, BREECH), (R_OUT - 0.0006, BREECH), (R_OUT, BREECH - 0.0006),
           (R_OUT, STEP + 0.001), (R_FWD + 0.0004, STEP), (R_FWD, STEP - 0.0004),
           (R_FWD, MUZZLE + 0.0006), (R_FWD - 0.0004, MUZZLE),
           (R_BORE + 0.0006, MUZZLE)], "Blued", segs=32, xf=ON_BORE, closed=True)
# The lug under the breech half: a flat-sided block sitting on the frame's
# side plate, and a tongue at its front down round the hinge pin, inside
# the plate's fork, concentric with the pin so it turns there.
LUG_R = 0.0055
lug = ([(SHIELD - 0.0002, BORE_Y - 0.010), (SHIELD - 0.0002, LUG_Y), (HZ + 0.0075, LUG_Y)] +
       [(HZ + LUG_R * math.cos(math.radians(a)), HY + LUG_R * math.sin(math.radians(a)))
        for a in range(-30, -181, -25)] +
       [(HZ - LUG_R, LUG_Y + 0.004), (STEP + 0.0004, BORE_Y - 0.010)])
lug = fillet(lug, [0.0, 0.0012, 0.002] + [0.0] * 7 + [0.002, 0.0], 2)
bar.prism(lug, -0.0062, 0.0062, "Blued", axis="x", bevel=0.0007, segs=1)
# The lug's flat sides, nearly as wide as the frame, under the breech half.
bar.prism(fillet([(SHIELD - 0.0002, BORE_Y - 0.006), (SHIELD - 0.0002, LUG_Y),
                  (STEP + 0.0015, LUG_Y), (STEP + 0.0015, BORE_Y - 0.006)],
                 [0.0, 0.0012, 0.0012, 0.0], 2),
          -0.0112, 0.0112, "Blued", axis="x", bevel=0.0008, segs=1)
# Extractor: a sliding block in the right of the breech face, its claw
# over the rim recess.
bar.prism(fillet([(BREECH - 0.016, BORE_Y - 0.004), (BREECH - 0.0004, BORE_Y - 0.004),
                  (BREECH - 0.0004, BORE_Y + 0.004), (BREECH - 0.016, BORE_Y + 0.004)],
                 0.0012, 2), 0.0128, 0.0168, "Blued", axis="x", bevel=0.0006, segs=1)

# --- Frame --------------------------------------------------------------------------------------
frame = Part("frame")
# Side outline: the long side plate under the barrel's lug out to the
# hinge, no trigger guard (the trigger hangs bare under the plate), the
# grip raked back about 20 degrees to a flat butt, and the frame's top
# curving down behind the breech under the hammer's spur.
prof = [(SHIELD, 0.066), (SHIELD, LUG_Y), (-0.0865, LUG_Y), (-0.0865, PLATE_Y),
        (-0.0195, PLATE_Y),   # front strap, behind the trigger
        (-0.0135, 0.004), (-0.0075, -0.014), (0.0005, -0.030), (0.007, -0.050),
        (0.0105, -0.070), (0.016, -0.0795),
        (0.023, -0.0855),     # butt
        (0.046, -0.0855),
        (0.0565, -0.0755),
        (0.0585, -0.064),     # back strap
        (0.0575, -0.050), (0.0545, -0.038), (0.047, -0.020), (0.035, 0.0),
        (0.0285, 0.016), (0.0266, 0.030), (0.0285, 0.041),
        (0.029, 0.049), (0.022, 0.057), (0.010, 0.0645), (0.0, 0.0675)]
radii = [0.002, 0.0, 0.0015, 0.0025, 0.003, 0.008, 0.01, 0.01, 0.01, 0.01, 0.005, 0.005,
         0.004, 0.006, 0.006, 0.01, 0.01, 0.01, 0.01, 0.01, 0.006, 0.004, 0.004, 0.006,
         0.008, 0.006, 0.004]
frame.prism(fillet(prof, radii, 3), -FRAME_HW, FRAME_HW, "Blued", axis="x", bevel=0.0016,
            segs=2)
# The breech shield: a round collar as wide as the barrel's breech half,
# the hammer's housing narrowing behind it into the frame.
frame.lathe([(0.0, SHIELD), (0.0166, SHIELD), (0.0172, SHIELD + 0.0007), (0.0172, -0.0045),
             (0.0164, -0.0038), (0.0158, 0.0), (0.0140, 0.006), (0.0110, 0.012),
             (0.0, 0.016)], "Blued", segs=32, xf=ON_BORE)

cut = Part("cut")
# The fork in the side plate's front the barrel's tongue turns in.
cut.box((0.0136, 0.026, 0.024), (0.0, HY + 0.008, HZ + 0.004), "Blued")
# The hammer's slot through the top of the housing.
cut.box((0.0096, 0.05, 0.048), (0.0, 0.046 + 0.025, 0.022), "Blued")
# The "sight": a groove along the top of the shield.
cut.box((0.0024, 0.004, 0.016), (0.0, BORE_Y + 0.0172, -0.004), "Blued", bevel=0.0)
# Firing pin hole in the breech face.
cut.cyl(0.0021, SHIELD - 0.002, SHIELD + 0.003, "Blued", segs=12, xf=ON_BORE)
# The model and year struck on the right of the side plate.
RIGHT = T(FRAME_HW - 0.0006, 0, 0) @ RY(math.pi / 2)
text(cut, "СПШ-44", 0.0036, "Blued", RIGHT @ T(0.052, 0.0295, 0.0), depth=0.0012)
text(cut, "1944", 0.003, "Blued", RIGHT @ T(0.052, 0.0225, 0.0), depth=0.0012)
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

# Barrel latch: a flat lever on the left of the side plate, its serrated
# thumb piece in front of the grip; pressing it lets the barrel drop.
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

# --- Grip: panels on both sides, inset in the frame's straps -------------------------------------
gp = fillet([(-0.0175, 0.0245), (-0.0095, 0.002), (-0.0035, -0.016), (0.0035, -0.032),
             (0.0075, -0.052), (0.0115, -0.071), (0.017, -0.0785), (0.024, -0.081),
             (0.044, -0.081), (0.0515, -0.0765), (0.0545, -0.064), (0.054, -0.050),
             (0.0505, -0.039), (0.0435, -0.022), (0.032, -0.003), (0.0245, 0.0155),
             (0.0225, 0.0245)],
            [0.004, 0.008, 0.01, 0.01, 0.01, 0.006, 0.004, 0.004, 0.004, 0.004, 0.006, 0.01,
             0.01, 0.01, 0.01, 0.004, 0.003], 4)


def swell(y):
    return 0.0012 * math.exp(-((y + 0.03) / 0.03) ** 2)


grip = Part("grip")
grip.prism(gp, 0, 0, "Bakelite", axis="x", bevel=0.0026, segs=3,
           width=lambda z, y: (FRAME_HW - 0.0015, 0.0178 + swell(y)))
grip.prism(gp, 0, 0, "Bakelite", axis="x", bevel=0.0026, segs=3,
           width=lambda z, y: (-0.0178 - swell(y), -FRAME_HW + 0.0015))
# The grip screw through both panels, low in the front half.
for sx in (-1, 1):
    grip.sphere(0.0046, (sx * 0.0178, -0.034, 0.022), "Steel", segs=16, rings=8,
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
grip_check.checkering(grip_cut, grip_x, None, None, (-0.012, 0.052, -0.076, 0.019),
                      pitch=0.0024, depth=0.0005, mat="Bakelite")
grip.cut(grip_cut)

# Lanyard loop: a flat stirrup of wire under the butt, in the gun's plane.
lan = Part("lanyard")
lan.box((0.006, 0.006, 0.026), (0.0, -0.0868, 0.0335), "Steel", bevel=0.0012)
loop = fillet([(0.0215, -0.0875), (0.0215, -0.0925), (0.0455, -0.0925), (0.0455, -0.0875)],
              [0.0, 0.003, 0.003, 0.0], 3)
lan.sweep(loop, section_circle(0.0015, 8), "Steel", closed=False)

# --- Hammer (turns back about its pin) ---------------------------------------------------------
# Inside the frame behind the breech; only its grooved spur shows, over
# the frame's top at the back.
hammer = Part("hammer", origin=HAMMER_PIN)
hp = fillet([(0.0005, 0.050), (0.0005, 0.062), (0.006, 0.0675), (0.013, 0.0688),
             (0.024, 0.0655), (0.0345, 0.0632), (0.0395, 0.0615), (0.0405, 0.0565),
             (0.036, 0.0535), (0.028, 0.0535), (0.022, 0.049), (0.021, 0.040),
             (0.0165, 0.0355), (0.0105, 0.0385), (0.0055, 0.0455)],
            [0.0008, 0.002, 0.003, 0.004, 0.006, 0.003, 0.002, 0.0015, 0.002, 0.004, 0.003,
             0.003, 0.003, 0.003, 0.002], 3)
hammer.prism(hp, -0.0045, 0.0045, "Blued", axis="x", bevel=0.0009, segs=1)
# The broad spur you thumb, grooved across.
spur = fillet([(0.014, 0.0688), (0.024, 0.0655), (0.0345, 0.0632), (0.0395, 0.0615),
               (0.0405, 0.0565), (0.036, 0.0555), (0.024, 0.0605), (0.014, 0.0635)],
              [0.002, 0.004, 0.003, 0.002, 0.0015, 0.002, 0.004, 0.002], 2)
hammer.prism(spur, -0.0068, 0.0068, "Blued", axis="x", bevel=0.0013, segs=2)


def spur_top(z):
    pts = [(0.013, 0.0688), (0.024, 0.0655), (0.0345, 0.0632), (0.0395, 0.0615)]
    for (z0, y0), (z1, y1) in zip(pts, pts[1:]):
        if z0 <= z <= z1:
            return y0 + (y1 - y0) * (z - z0) / (z1 - z0)
    return pts[-1][1]


spur_cut = Part("cut")
for i in range(8):
    z = 0.0175 + i * 0.0023
    spur_cut.box((0.016, 0.0022, 0.0010), (0.0, spur_top(z), z), "Blued")
hammer.cut(spur_cut)

# --- Trigger ------------------------------------------------------------------------------------
# A flat blade hanging bare under the side plate, its finger piece turned
# forward and serrated at the foot. Its origin on its pin: the game turns
# it about it (`Moves::Trigger`).
trig = Part("trigger", origin=(0.0, 0.0205, -0.0195))
tp = fillet([(-0.0215, 0.019), (-0.0165, 0.019), (-0.0135, 0.004), (-0.0125, -0.006),
             (-0.012, -0.0135), (-0.0175, -0.0165), (-0.0255, -0.0165), (-0.0262, -0.0125),
             (-0.0228, -0.0108), (-0.0238, -0.0045), (-0.0243, 0.006), (-0.0232, 0.012)],
            [0.001, 0.001, 0.003, 0.003, 0.0015, 0.0015, 0.001, 0.001, 0.001, 0.003, 0.003,
             0.002], 2)
trig.prism(tp, -0.003, 0.003, "Blued", axis="x", bevel=0.0006, segs=1)
trig_cut = Part("cut")
for i in range(4):
    trig_cut.box((0.008, 0.0012, 0.0008), (0.0, -0.0167, -0.0245 + i * 0.0018), "Blued")
trig.cut(trig_cut)

# --- Build ----------------------------------------------------------------------------------------
join("barrels", bar, worn={})
join("static_frame", frame, pins, latch, grip, grip_check, lan, worn={})
make(hammer, worn={})
make(trig, worn={})

empty("pt_muzzle", (0.0, BORE_Y, MUZZLE))
empty("pt_sight", (0.0, BORE_Y + 0.0172, 0.004))
empty("pt_hinge", HINGE)
empty("pt_load_port", (0.0, BORE_Y, BREECH - 0.005))
empty("pt_port", (0.0, BORE_Y, BREECH - 0.005))

report()
export("assets/models/guns/flare_pistol.glb")
