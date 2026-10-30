"""Nagant M1895: a seven-shot 7.62x38R revolver, solid frame, loaded and
unloaded one chamber at a time through a gate on the right. Writes
assets/models/guns/nagant.glb.

Gun space as in src/weapons/layout.rs (`nagant()`): origin at the palm
on the grip, -Z down the bore (bore axis y = BORE), +Y up, metres. The
cylinder's axis is CYL_R below the bore; the chamber in line with the
barrel is chamber 0 at the top.

- `static_frame`      frame, barrel, grip, guard, the ejector rod's
                      housing, sights
- `cylinder`          the cylinder; origin on its axis. Chamber k sits
                      k sevenths of a turn anticlockwise from the top, seen
                      from behind; the game turns it by -cylinder*2*pi/7
                      about Z, which brings chamber `cylinder` under the
                      hammer, and chamber (cylinder + GATE_OFFSET) % 7 to
                      the gate
- `cylinder_round_N`, `cylinder_case_N` (N 0..6)  a live round, or a
                      spent case, in chamber N; origins on the cylinder
                      axis so they turn with it; the game shows the one
                      the chamber holds
- `hammer`            origin on the hammer pin; a positive turn about +X
                      cocks it (the spur goes back)
- `gate`              the loading gate, origin on its hinge (upright, at
                      its front edge); open is a turn of +GATE_OPEN about
                      Y, swinging out to the right like a door
- `ejector_rod`       origin at its rest under the barrel; pushed back
                      along +Z by EJECT_TRAVEL through the chamber at the
                      gate
- `trigger`
- empties `pt_muzzle`, `pt_sight`, `pt_load_port` (the chamber at the
  gate, where a round goes in)
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "items"))
from common import (Part, RX, RY, RZ, T, catmull, empty, export, fillet, join, main_guard, make,
                    report, section_rect)
from _kit import text  # noqa: E402
from ammo import C762X38R, cartridge  # noqa: E402

main_guard()

BORE = 0.074
CYL_R = 0.0122                 # chamber circle radius
CYL_Y = BORE - CYL_R           # cylinder axis
CYL_Z0, CYL_Z1 = -0.059, -0.019  # cylinder front and rear faces
CYL_RAD = 0.0198               # cylinder outer radius
MUZZLE = -0.178
N = 7
GATE_OPEN = 1.4                # radians
EJECT_TRAVEL = 0.042
# The gate is two chambers clockwise of the top, seen from behind, on the
# right: with the cylinder at rest that's chamber 5.
GATE_OFFSET = 5
GATE_CH = GATE_OFFSET


def chamber_xy(k, a0=math.pi / 2):
    a = a0 + 2 * math.pi * k / N
    return CYL_R * math.cos(a), CYL_Y + CYL_R * math.sin(a)


# --- Frame ---------------------------------------------------------------------------
frame = Part("frame")
# Side profile (z, y): the solid frame round the cylinder window, the top
# strap, the recoil shield behind, down into the grip frame and guard.
fprof = [
    (-0.071, BORE + 0.0085), (0.006, BORE + 0.0085),       # top strap
    (0.012, BORE + 0.002), (0.012, 0.044),                 # recoil shield's back
    (0.004, 0.034), (0.009, 0.006),                        # backstrap under the hammer
    (0.0, 0.004), (-0.033, 0.004),                         # bottom, over the guard
    (-0.046, 0.03), (-0.071, 0.034),                       # front of the frame under the barrel
]
fprof = fillet(fprof, [0.003, 0.004, 0.003, 0.004, 0.008, 0.006, 0.006, 0.008, 0.012, 0.004], 6)
frame.prism(fprof, -0.0112, 0.0112, "Blued", axis="x", bevel=0.0012, segs=1)
fc = Part("cut")
# The cylinder window, through.
fc.box((0.04, 0.046, CYL_Z1 - CYL_Z0 + 0.002), (0, CYL_Y, (CYL_Z0 + CYL_Z1) / 2), "Blued",
       bevel=0.002, segs=1)
# The gate's recess in the recoil shield, right side, at the chamber it opens on.
gx, gy = chamber_xy(GATE_CH)
fc.box((0.012, 0.014, 0.034), (0.0112, gy, -0.004), "Blued", bevel=0.0015, segs=1)
# Stampings on the left of the frame: the arsenal's star, serial, year.
LEFT = T(-0.0112 + 0.0006, 0, 0) @ RY(-math.pi / 2)
text(fc, "ГА 2347", 0.0034, "Blued", LEFT @ T(0.022, 0.038, 0.0), depth=0.0012)
text(fc, "1932", 0.0034, "Blued", LEFT @ T(0.022, 0.031, 0.0), depth=0.0012)
# The side plate's outline on the right, a fine line cut round it.
plate = fillet([(-0.04, 0.012), (0.004, 0.012), (0.006, 0.04), (-0.03, 0.04)], 0.004, 3)
fc.sweep(plate + plate[:1], section_rect(0.0024, 0.0007), "Blued", xf=T(0.0112, 0, 0))
# The sighting groove down the top strap to the rear notch.
fc.box((0.0018, 0.0024, 0.066), (0, BORE + 0.0085, -0.033), "Blued")
# Screws on the side plate: domed heads, slotted; and the grip screw's
# heads through both panels.
screw = [(0.0, -0.0123), (0.0012, -0.0122), (0.0021, -0.0119), (0.0025, -0.0114),
         (0.0025, -0.0106), (0.0, -0.0106)]
for (z, y), a in zip(((-0.012, 0.02), (0.002, 0.03), (-0.03, 0.016)), (0.3, 1.2, -0.5)):
    frame.lathe(screw, "Steel", segs=16, xf=T(0, y, z) @ RY(math.pi / 2))
    fc.box((0.0014, 0.0007, 0.006), (-0.0124, y, z), "Steel", xf=RX(a, about=(0, y, z)))
for sx in (-1, 1):
    frame.lathe([(0.0, 0.0148), (0.0016, 0.0147), (0.0027, 0.0142), (0.003, 0.0136),
                 (0.003, 0.013), (0.0, 0.013)], "Steel", segs=18,
                xf=T(0, -0.003, 0.007) @ RY(sx * math.pi / 2))
    fc.box((0.0014, 0.0007, 0.007), (sx * 0.0149, -0.003, 0.007), "Steel",
           xf=RX(0.9, about=(0, -0.003, 0.007)))
frame.cut(fc)
# The trigger's pin, through the frame.
for sx in (-1, 1):
    frame.cyl(0.0017, 0.0, 0.0007, "Steel", segs=12, chamfer=0.0003,
              xf=T(sx * 0.0111, 0.012, -0.02) @ RY(sx * math.pi / 2))

static = Part("static_frame")
# Barrel: octagonal at the breech, round forward, screwed into the frame.
oct_ = [(0.0098 * math.cos(math.radians(22.5 + 45 * i)),
         BORE + 0.0098 * math.sin(math.radians(22.5 + 45 * i))) for i in range(8)]
static.prism(fillet(oct_, 0.0012, 3), -0.092, -0.068, "Blued", axis="z", bevel=0.0008, segs=2)
static.lathe([(0.0039, MUZZLE + 0.003), (0.0045, MUZZLE + 0.001), (0.0068, MUZZLE),
              (0.0078, MUZZLE + 0.0003), (0.0082, MUZZLE + 0.0018), (0.0082, -0.09),
              (0.0, -0.09)], "Blued", segs=40, xf=T(0, BORE, 0))
# The front sight blade near the muzzle; the rear notch in the top strap.
static.prism(fillet([(MUZZLE + 0.006, BORE + 0.006), (MUZZLE + 0.016, BORE + 0.006),
                     (MUZZLE + 0.014, BORE + 0.0135), (MUZZLE + 0.0075, BORE + 0.0135)],
                    0.0008, 1), -0.0012, 0.0012, "Blued", axis="x", bevel=0.0003, segs=1)
# The blade's base, a saddle on the barrel.
static.prism(fillet([(MUZZLE + 0.004, BORE + 0.0062), (MUZZLE + 0.019, BORE + 0.0062),
                     (MUZZLE + 0.017, BORE + 0.0088), (MUZZLE + 0.006, BORE + 0.0088)],
                    0.0007, 2), -0.0026, 0.0026, "Blued", axis="x", bevel=0.0005, segs=2)
# The rear sight: a hump on the top strap with a U notch.
rs = [(-0.0047, BORE + 0.0078), (0.0047, BORE + 0.0078), (0.0047, BORE + 0.0102),
      (0.0038, BORE + 0.0116), (0.0017, BORE + 0.0116), (0.0015, BORE + 0.0098),
      (0.0, BORE + 0.0091), (-0.0015, BORE + 0.0098), (-0.0017, BORE + 0.0116),
      (-0.0038, BORE + 0.0116), (-0.0047, BORE + 0.0102)]
static.prism(fillet(rs, [0, 0, 0.0006, 0.0006, 0.0003, 0.0006, 0.0, 0.0006, 0.0003, 0.0006,
                         0.0006], 3), 0.0, 0.0062, "Blued", axis="z", bevel=0.0003, segs=2)
# The ejector rod's housing under the barrel, a tube on a lug, and the
# cylinder axis pin's head at the front of the frame.
static.tube(0.0042, 0.0028, -0.165, -0.075, "Blued", segs=24, xf=T(0, CYL_Y - 0.004, 0),
            chamfer=0.0006)
static.box((0.006, 0.01, 0.008), (0, CYL_Y + 0.002, -0.163), "Blued", bevel=0.0015, segs=3)
# The lug's pin, across it.
static.cyl(0.0011, -0.0034, 0.0034, "Steel", segs=10, xf=T(0, CYL_Y + 0.004, -0.163) @ RY(math.pi / 2))
static.lathe([(0.0, -0.0735), (0.0028, -0.0735), (0.0036, -0.0728), (0.0036, -0.0712),
              (0.0040, -0.0708), (0.0040, -0.0618), (0.0036, -0.0612), (0.0, -0.0612)],
             "Steel", segs=24, xf=T(0, CYL_Y, 0))
# Trigger guard.
guard = catmull([(-0.033, 0.004), (-0.038, -0.006), (-0.034, -0.017), (-0.02, -0.021),
                 (-0.006, -0.017), (0.0, -0.008), (0.003, 0.002)], 8, closed=False)
static.sweep(guard, section_rect(0.0065, 0.003, 0.001, 3), "Blued")

# Grip: smooth walnut panels on a bird's-head frame, the lanyard ring at
# the butt. Below the heel of the palm the grip rakes back (RAKE metres
# back per metre down from RAKE_Y), as an M1895's does, to its 235 mm:
# the palm's part of it, and so the hold and the aim, stay put.
RAKE, RAKE_Y = 0.42, -0.025


def raked(pts):
    return [(z + RAKE * max(0.0, RAKE_Y - y), y) for z, y in pts]


grip_prof = catmull(raked([(0.0005, 0.006), (-0.008, -0.03), (-0.008, -0.06), (0.003, -0.08),
                           (0.02, -0.084), (0.031, -0.075), (0.031, -0.05), (0.022, -0.012),
                           (0.016, 0.006)]), samples=4)


def grip_w(z, y):
    swell = 0.0012 * math.exp(-((y + 0.04) / 0.03) ** 2)
    return (-0.0135 - swell, 0.0135 + swell)


grip = Part("grip")
grip.prism(grip_prof, 0, 0, "Wood", axis="x", bevel=0.003, segs=3, width=grip_w)
gc = Part("cut")
grip_check = Part("static_checkering")


def grip_x(z, y):
    zs = []
    n = len(grip_prof)
    for i in range(n):
        (z0, y0), (z1, y1) = grip_prof[i], grip_prof[(i + 1) % n]
        if (y0 - y) * (y1 - y) <= 0 and y0 != y1:
            zs.append(z0 + (z1 - z0) * (y - y0) / (y1 - y0))
    if len(zs) < 2 or not min(zs) + 0.005 < z < max(zs) - 0.005:
        return None
    return grip_w(z, y)[1]


grip_check.checkering(gc, grip_x, None, None, (-0.006, 0.046, -0.07, -0.012), pitch=0.0022,
                      depth=0.0005, mat="Wood")
grip.cut(gc)
# The grip's frame strap down the back, the butt cap, and the lanyard ring.
static.prism(fillet(raked([(0.016, 0.006), (0.031, -0.05), (0.031, -0.075), (0.02, -0.0845),
                           (0.019, -0.081), (0.027, -0.074), (0.027, -0.052), (0.013, 0.006)]),
                    [0.002, 0.006, 0.006, 0.003, 0.0, 0.004, 0.006, 0.002], 3),
             -0.004, 0.004, "Blued", axis="x")
BUTT_Z = 0.024 + RAKE * (RAKE_Y + 0.088)
static.sweep([(BUTT_Z + 0.0055 * math.cos(a), -0.088 + 0.0055 * math.sin(a))
              for a in [2 * math.pi * i / 24 for i in range(24)]],
             section_rect(0.0018, 0.0018, 0.0006, 3), "Steel", closed=True)
static.box((0.004, 0.006, 0.004), (0, -0.0835, BUTT_Z), "Steel", bevel=0.001, segs=3)
static.cyl(0.0012, -0.0028, 0.0028, "Steel", segs=10, xf=T(0, -0.0825, BUTT_Z) @ RY(math.pi / 2))

# --- Cylinder (turns about its axis) --------------------------------------------------
cyl = Part("cylinder", origin=(0.0, CYL_Y, 0.0))
cyl.lathe([(0.0035, CYL_Z0), (CYL_RAD - 0.001, CYL_Z0), (CYL_RAD, CYL_Z0 + 0.001),
           (CYL_RAD, CYL_Z1 - 0.0015), (CYL_RAD - 0.0015, CYL_Z1), (0.0035, CYL_Z1)],
          "Blued", segs=64, xf=T(0, CYL_Y, 0), closed=True)
cc = Part("cut")
for k in range(N):
    x, y = chamber_xy(k)
    # The chambers, through; the bolt stop notches round its rear edge.
    cc.cyl(0.0045, CYL_Z0 - 0.002, CYL_Z1 + 0.002, "Blued", segs=18, xf=T(x, y, 0))
    # A chamfer round each chamber's mouth.
    cc.lathe([(0.0, CYL_Z0 - 0.001), (0.0058, CYL_Z0 - 0.001), (0.0045, CYL_Z0 + 0.0008),
              (0.0, CYL_Z0 + 0.0008)], "Blued", segs=18, xf=T(x, y, 0))
    a = math.pi / 2 + 2 * math.pi * (k + 0.5) / N
    nx, ny = (CYL_RAD + 0.0005) * math.cos(a), CYL_Y + (CYL_RAD + 0.0005) * math.sin(a)
    cc.box((0.0028, 0.0028, 0.006), (nx, ny, CYL_Z1 - 0.007), "Blued", xf=RZ(a, about=(nx, ny, 0)))
cyl.cut(cc)
# The ratchet on its back face, its teeth the hand pushes on.
for k in range(N):
    a = math.pi / 2 + 2 * math.pi * k / N
    cyl.box((0.003, 0.0045, 0.002), (0.0, 0.0, 0.0), "Steel",
            xf=T(0, CYL_Y, CYL_Z1 + 0.0008) @ RZ(a) @ T(0.0055, 0, 0))

rounds = []
for k in range(N):
    x, y = chamber_xy(k)
    # The Nagant's round: the bullet sits inside the case, its crimped
    # mouth level with the cylinder's front face.
    live = Part(f"cylinder_round_{k}", origin=(0.0, CYL_Y, 0.0))
    cartridge(live, C762X38R, T(x, y, CYL_Z1 + 0.0004), segs=12)
    spent = Part(f"cylinder_case_{k}", origin=(0.0, CYL_Y, 0.0))
    case, _, _ = C762X38R
    spent.lathe([(r, z) for r, z in case], "Brass", segs=12, xf=T(x, y, CYL_Z1 + 0.0004))
    # A dented primer.
    spent.cyl(0.0016, -0.0003, 0.00005, "Copper", segs=8, xf=T(x, y, CYL_Z1 + 0.0004))
    rounds += [live, spent]

# --- Hammer, gate, ejector rod, trigger ----------------------------------------------------
HAMMER_PIN = (0.0, 0.052, 0.012)
hammer = Part("hammer", origin=HAMMER_PIN)
hp = fillet([(0.006, 0.046), (0.013, 0.047), (0.017, 0.058), (0.018, 0.071), (0.023, 0.08),
             (0.019, 0.083), (0.012, 0.075), (0.009, 0.064), (0.005, 0.057)],
            [0.0, 0.002, 0.003, 0.003, 0.003, 0.003, 0.002, 0.002, 0.0], 3)
hammer.prism(hp, -0.0032, 0.0032, "Blued", axis="x", bevel=0.0007, segs=2)
hc = Part("cut")
for i in range(4):
    y = 0.0738 + i * 0.002
    hc.box((0.01, 0.0011, 0.004), (0.0, y, 0.0205 + i * 0.0012), "Blued",
           xf=RX(0.6, about=(0, y, 0.0205)))
hammer.cut(hc)

GATE_HINGE = (0.0112 + 0.0018, gy, -0.019)
gate = Part("gate", origin=GATE_HINGE)
gate.prism(fillet([(-0.019, gy - 0.008), (0.011, gy - 0.008), (0.011, gy + 0.0072),
                   (-0.019, gy + 0.0072)], 0.002, 2), 0.0062, 0.0112 + 0.0018, "Blued", axis="x",
           bevel=0.0006, segs=1)
gate.box((0.0028, 0.004, 0.008), (0.0128, gy + 0.004, 0.006), "Blued", bevel=0.0008, segs=3)
gtc = Part("cut")
# Grooves across the thumb piece.
for i in range(4):
    gtc.box((0.0016, 0.006, 0.0007), (0.0142, gy + 0.004, 0.0034 + i * 0.0018), "Blued")
gate.cut(gtc)

ROD_Y = CYL_Y - 0.004
rod = Part("ejector_rod", origin=(0.0, ROD_Y, -0.12))
rod.cyl(0.0024, -0.168, -0.062, "Steel", segs=16, xf=T(0, ROD_Y, 0))
# The head, ringed for a grip.
head = [(0.0, -0.176), (0.0045, -0.176), (0.0055, -0.1725)]
for i in range(4):
    z = -0.1725 + i * 0.0016
    head += [(0.0055, z + 0.0004), (0.0049, z + 0.0008), (0.0055, z + 0.0012)]
head += [(0.0055, -0.166), (0.0, -0.165)]
rod.lathe(head, "Steel", segs=24, xf=T(0, ROD_Y, 0))

trig = Part("trigger", origin=(0.0, 0.006, -0.02))
trig.sweep(catmull([(-0.022, 0.004), (-0.026, -0.004), (-0.025, -0.012), (-0.021, -0.016)], 6,
                   closed=False), section_rect(0.0045, 0.003, 0.0011, 3), "Blued")

join("static_frame", frame, static, grip, grip_check, worn={})
for p in [cyl, hammer, gate, rod, trig]:
    make(p, worn={})
for p in rounds:
    make(p)

empty("pt_muzzle", (0.0, BORE, MUZZLE))
empty("pt_sight", (0.0, BORE + 0.012, 0.012))
empty("pt_load_port", (0.016, gy, -0.012))

report()
export("assets/models/guns/nagant.glb")
