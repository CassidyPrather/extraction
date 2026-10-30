"""Makarov PM: blowback 9x18 pistol. Writes assets/models/guns/pm_pistol.glb.

Gun space as in src/weapons/layout.rs (`pm()`): origin at the palm on the
grip, -Z down the bore (bore axis y = 0.062), +Y up, metres.
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, T, catmull, empty, export, fillet, join, main_guard, make, report,
                    section_rect)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from _kit import text  # noqa: E402

main_guard()

BORE_Y = 0.062
# Where the magazine seats (its top, along its rake): the lips just under
# the slide.
MAGWELL = (0.0, 0.0445, -0.002)
SLIDE_Z0, SLIDE_Z1 = -0.132, 0.026  # muzzle end, rear
SLIDE_Y0, SLIDE_Y1 = 0.048, 0.077
SLIDE_HW = 0.014

# --- Slide (moves with the bolt) ---------------------------------------------
slide = Part("bolt_slide", origin=(0.0, BORE_Y, 0.0))
sec = fillet(
    [(-SLIDE_HW, SLIDE_Y0), (SLIDE_HW, SLIDE_Y0), (SLIDE_HW, SLIDE_Y1), (-SLIDE_HW, SLIDE_Y1)],
    [0.0024, 0.0024, 0.0105, 0.0105], 8)  # (fillets no tighter than the bevel: see _slab)
slide.prism(sec, SLIDE_Z0, SLIDE_Z1, "Blued", axis="z", bevel=0.0022, segs=3)
# Rear sight: a block with a square notch, dovetailed into the slide.
slide.prism(fillet([(0.012, 0.0755), (0.024, 0.0755), (0.0235, 0.0815), (0.0145, 0.0815)], 0.0008, 3),
            -0.007, 0.007, "Blued", axis="x", bevel=0.0005, segs=2)
# Front sight: a ramped blade.
slide.prism(fillet([(-0.1295, 0.0755), (-0.117, 0.0755), (-0.1215, 0.0805), (-0.1275, 0.0805)],
                   0.0006, 3), -0.0016, 0.0016, "Blued", axis="x", bevel=0.0004, segs=2)
# The extractor, let into the right of the slide behind the port, and its pin.
slide.prism(fillet([(-0.0302, 0.0662), (-0.015, 0.0665), (-0.0145, 0.0702), (-0.0302, 0.0705)],
                   [0.0004, 0.0012, 0.0012, 0.0004], 2),
            0.0134, 0.0146, "Blued", axis="x", bevel=0.0003, segs=1)
slide.cyl(0.0009, 0.0, 0.0011, "Steel", segs=10, xf=T(0.0139, 0.0684, -0.0185) @ RY(math.pi / 2))

cut = Part("cut")
# Muzzle opening around the barrel.
cut.cyl(0.0069, SLIDE_Z0 - 0.01, SLIDE_Z0 + 0.03, "Blued", segs=36, xf=T(0, BORE_Y, 0))
# Ejection port, right side.
cut.box((0.02, 0.016, 0.027), (0.016, 0.0705, -0.044), "Blued", bevel=0.0015, segs=1)
# Rear sight notch.
cut.box((0.0028, 0.004, 0.02), (0.0, 0.0818, 0.018), "Blued")
# Glare grooves across the sight's back face.
for y in (0.0786, 0.0799):
    cut.box((0.016, 0.0006, 0.0024), (0.0, y, 0.0242), "Blued")
# Grasping serrations, both sides, slightly raked.
for i in range(11):
    z = -0.001 + i * 0.0022
    for sx in (-1, 1):
        cut.box((0.0024, 0.019, 0.0011), (sx * SLIDE_HW, 0.0585, z), "Blued",
                xf=RX(-0.12, about=(0, 0.0585, z)))
# The serial and year engraved on the left of the slide.
# (0.6 mm deep: the exact boolean drops cuts much shallower than half a
# millimetre.)
LEFT = T(-SLIDE_HW + 0.0006, 0, 0) @ RY(-math.pi / 2)
text(cut, "ГТ 3051", 0.0032, "Blued", LEFT @ T(-0.083, 0.0585, 0.0), depth=0.0012)
text(cut, "1959", 0.0032, "Blued", LEFT @ T(-0.083, 0.0535, 0.0), depth=0.0012)
slide.cut(cut)

# --- Barrel (static; shows at the muzzle and through the port) ---------------
static = Part("static_frame")
static.tube(0.0064, 0.00465, -0.1305, -0.028, "Blued", segs=36, xf=T(0, BORE_Y, 0), chamfer=0.0007)
# Chamber hood, filling the port.
static.box((0.0125, 0.010, 0.026), (0.0, 0.0655, -0.042), "Blued", bevel=0.0015)

# --- Frame --------------------------------------------------------------------
frame_prof = [
    (0.024, 0.0485),   # rear top, under the slide
    (-0.118, 0.0485),  # front top
    (-0.121, 0.044),
    (-0.117, 0.0375),
    (-0.100, 0.0345),
    (-0.083, 0.0345),  # guard front attaches here
    (-0.022, 0.0345),  # top of the trigger opening
    (-0.019, 0.032),   # front strap begins
    (-0.0045, -0.046),  # front strap bottom
    (-0.002, -0.0505),
    (0.036, -0.0505),
    (0.038, -0.046),   # back strap bottom
    (0.022, 0.026),    # back strap top
    (0.027, 0.040),    # tang under the hammer
    (0.027, 0.0485),
]
frame_prof = fillet(frame_prof, [0.001, 0.002, 0.004, 0.003, 0.004, 0.0, 0.0, 0.004, 0.006, 0.002,
                                 0.002, 0.003, 0.006, 0.003, 0.002], 6)
frame = Part("frame")
frame.prism(frame_prof, -0.0112, 0.0112, "Blued", axis="x", bevel=0.0018, segs=3)
# The magazine's well up the grip, along the `magwell` seat (below): the
# frame and grip were solid, and the magazine stood through them (the 28
# October playtest, 0:08:31). Its body is 12.4 x 28.7 mm; 0.6 mm clear.
WELL = T(*MAGWELL) @ RX(-0.216)
well = Part("cut")
well.box((0.0136, 0.125, 0.0299), (0.0, -0.06, -0.0001), "Blued", xf=WELL)
frame.cut(well)
# Pin heads through the frame: the guard's hinge, the trigger bar's, the
# sear's; on the right, the slide stop's pivot.
for z, y, sides in ((-0.09, 0.040, (-1, 1)), (-0.03, 0.038, (-1, 1)), (0.014, 0.040, (-1, 1)),
                    (-0.038, 0.044, (1,))):
    for sx in sides:
        static.cyl(0.0014, 0.0, 0.0005, "Steel", segs=12, chamfer=0.0002,
                   xf=T(sx * 0.0111, y, z) @ RY(sx * math.pi / 2))

# Wraparound bakelite grip: sides and back strap, inset from the frame edges.
grip_prof = catmull([
    (-0.0135, 0.027), (-0.0105, 0.004), (-0.0065, -0.024), (-0.003, -0.0465),
    (0.018, -0.047), (0.0392, -0.0465), (0.0322, -0.012), (0.0255, 0.018), (0.0235, 0.029),
    (0.005, 0.0305),
], samples=4)


def grip_width(z, y):
    # A slight palm swell in the middle of the grip.
    swell = 0.0011 * math.exp(-((y + 0.008) / 0.03) ** 2)
    return (-0.0152 - swell, 0.0152 + swell)


grip = Part("grip")
grip.prism(grip_prof, 0, 0, "Bakelite", axis="x", bevel=0.0035, segs=3, width=grip_width)
# The star medallion near the top of each panel, a five-pointed star
# raised on it (its sides sunk in the dome).
star = [(0.007 + (0.0032 if i % 2 == 0 else 0.0013) * math.sin(math.pi * i / 5),
         0.017 + (0.0032 if i % 2 == 0 else 0.0013) * math.cos(math.pi * i / 5)) for i in range(10)]
for sx in (-1, 1):
    grip.sphere(0.0048, (sx * 0.0156, 0.017, 0.007), "Bakelite", segs=28, rings=14,
                scale=(0.25, 1, 1))
    grip.prism(star, *sorted((sx * 0.0164, sx * 0.0171)), "Bakelite", axis="x")


def grip_span(y):
    """Front and back of the grip outline at height y."""
    zs = []
    n = len(grip_prof)
    for i in range(n):
        (z0, y0), (z1, y1) = grip_prof[i], grip_prof[(i + 1) % n]
        if (y0 - y) * (y1 - y) <= 0 and y0 != y1:
            zs.append(z0 + (z1 - z0) * (y - y0) / (y1 - y0))
    return min(zs), max(zs)


# Checkered panels on both sides, inside a smooth border, under the star.
def grip_x(z, y):
    zf, zb = grip_span(y)
    if not zf + 0.004 < z < zb - 0.004:
        return None
    return grip_width(z, y)[1]


grip_cut = Part("cut")
grip_check = Part("static_checkering")
# The grip's screw, up the back strap, slotted.
gzb = grip_span(-0.025)[1]
grip.lathe([(0.0, gzb + 0.0012), (0.0017, gzb + 0.0011), (0.0027, gzb + 0.0006),
            (0.003, gzb), (0.003, gzb - 0.0015), (0.0, gzb - 0.0015)], "Steel", segs=18,
           xf=T(0, -0.025, 0))
grip_cut.box((0.006, 0.0007, 0.0014), (0.0, -0.025, gzb + 0.0012), "Steel")
grip_check.checkering(grip_cut, grip_x, None, None, (-0.008, 0.034, -0.0405, 0.009),
                      pitch=0.0019, depth=0.0005, mat="Bakelite")

# Trigger guard: a large rounded loop (it hinges down for stripping).
guard_path = catmull([
    (-0.084, 0.0355), (-0.0885, 0.026), (-0.086, 0.0135), (-0.0765, 0.0055), (-0.060, 0.0025),
    (-0.036, 0.003), (-0.022, 0.0065), (-0.0135, 0.0125),
], samples=8, closed=False)
static.sweep(guard_path, section_rect(0.0072, 0.0034, 0.0012, 3), "Blued")

# Hammer: a rounded spur behind the slide.
hammer = fillet([(0.0205, 0.0485), (0.0295, 0.0495), (0.0365, 0.0585), (0.0372, 0.0675),
                 (0.0335, 0.0725), (0.0285, 0.0705), (0.0245, 0.0615), (0.0215, 0.055)],
                [0, 0.002, 0.003, 0.003, 0.003, 0.002, 0.002, 0], 3)
# Its own object: the game turns it about its pin (the origin, an axis
# along X) when it cocks. Modelled down, against the slide; cocked is a
# positive turn about +X, the spur going back.
HAMMER_PIN = (0.0, 0.0515, 0.025)
hammer_p = Part("hammer", origin=HAMMER_PIN)
hammer_p.prism(hammer, -0.0034, 0.0034, "Blued", axis="x", bevel=0.0007, segs=2)
# Spur grooves, cut across the back of the spur.
spur_cut = Part("cut")
for i in range(4):
    y = 0.0605 + i * 0.0027
    spur_cut.box((0.01, 0.0012, 0.004), (0.0, y, 0.0372 - i * 0.0004), "Blued",
                 xf=RX(0.45, about=(0, y, 0.0372)))
hammer_p.cut(spur_cut)

# Slide stop lever, left side above the trigger.
static.prism(fillet([(-0.060, 0.041), (-0.034, 0.041), (-0.034, 0.047), (-0.058, 0.0465)], 0.0012, 2),
             -0.0122, -0.0108, "Blued", axis="x", bevel=0.0003, segs=1)
# Ridges on its thumb pad.
for i in range(4):
    static.box((0.0006, 0.0048, 0.0007), (-0.0123, 0.0438, -0.0565 + i * 0.0017), "Blued")
# Heel magazine catch, grooved across its back for the thumb.
catch = [(0.0375, -0.050), (0.0435, -0.050)]
for i in range(3):
    y = -0.0488 + i * 0.0016
    catch += [(0.0435, y), (0.0429, y + 0.0004), (0.0435, y + 0.0008)]
catch += [(0.0435, -0.044), (0.0375, -0.044)]
static.prism(catch, -0.005, 0.005, "Blued", axis="x")
# Lanyard loop on the left of the butt.
loop = [(0.031 + 0.0045 * math.cos(a), -0.044 + 0.0045 * math.sin(a))
        for a in [2 * math.pi * i / 24 for i in range(24)]]
static.sweep(loop, [(x, n) for x, n in section_rect(0.0016, 0.0016, 0.0005, 3)], "Steel",
             closed=True, xf=T(-0.0158, 0, 0))

# --- Trigger --------------------------------------------------------------------
trig = Part("trigger", origin=(0.0, 0.03, -0.035))
trig_path = catmull([(-0.0345, 0.0345), (-0.0385, 0.027), (-0.0395, 0.0195), (-0.0365, 0.0125),
                     (-0.0325, 0.0095)], samples=6, closed=False)
trig.sweep(trig_path, section_rect(0.0045, 0.0032, 0.0012, 3), "Blued")

# --- Safety / decocker (left rear of the slide, rotates about X) ---------------
PIVOT = (-0.0157, 0.0648, 0.007)
safety = Part("selector_safety", origin=PIVOT)
safety.cyl(0.0042, -0.0016, 0.0016, "Blued", segs=28, xf=T(*PIVOT) @ RY(math.pi / 2),
           chamfer=0.0006)
safety.prism(fillet([(0.006, 0.0615), (0.0215, 0.064), (0.0225, 0.0705), (0.007, 0.0685)],
                    [0.002, 0.0015, 0.0015, 0.002], 2),
             -0.0172, -0.0148, "Blued", axis="x", bevel=0.0005, segs=2)
for i in range(3):
    safety.box((0.0012, 0.0055, 0.0008), (-0.0176, 0.0665, 0.0165 + i * 0.0018), "Blued")

# --- Build ------------------------------------------------------------------------
for p in (slide, trig, safety):
    make(p, worn={})
grip_cut.box((0.0136, 0.125, 0.0299), (0.0, -0.06, -0.0001), "Bakelite", xf=WELL)
grip.cut(grip_cut)
join("static_frame", static, frame, grip, grip_check, worn={})
make(hammer_p, worn={})

# Named points (the layout's, for reference) and the modelled magazine seat.
empty("pt_muzzle", (0.0, BORE_Y, -0.13))
empty("pt_sight", (0.0, 0.079, 0.03))
empty("pt_port", (0.012, 0.068, -0.045))
empty("magwell", MAGWELL, RX(-0.216))

report()
export("assets/models/guns/pm_pistol.glb")
