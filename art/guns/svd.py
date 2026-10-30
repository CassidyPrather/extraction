"""SVD: 7.62x54R semi-automatic marksman rifle, a laminated skeleton stock
with the thumbhole grip and the detachable cheek rest, slotted wooden
handguard, five-slot flash hider, the dovetail scope rail on the left of
the receiver, a detachable ten-round magazine. Writes
assets/models/guns/svd.glb.

Gun space: origin at the palm on the thumbhole stock's grip, -Z down the
bore (bore axis y = 0.058), +Y up, +X right, metres.

- `static`        stock and cheek rest, butt pad, receiver and cover, the
                  side rail, barrel, gas block, handguard and its rings,
                  sights, flash hider, guard, magazine catch, swivels
- `bolt_carrier`  carrier, bolt and the charging handle on the right;
                  slides back (+Z) 0.13 with the action (origin on the bore)
- `selector`      the big safety lever on the right, turns about X at its pivot
- `trigger`
- empties `pt_muzzle`, `pt_sight`, `pt_port`, `pt_support`, `pt_handle`,
  `magwell`, `pt_sling_front`, `pt_sling_rear`, `pt_rail`
"""

import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from common import (Part, RX, RY, RZ, T, area2, catmull, empty, export, fillet, join, main_guard,
                    make, report, section_rect)

main_guard()

BORE = 0.058
MUZZLE = -0.955
RX_HW = 0.0175          # receiver half width
RX_Z0, RX_Z1 = -0.27, 0.03
RX_Y0, RX_Y1 = 0.015, 0.080
TRAVEL = 0.13


def superellipse(yb, yt, wb, wt, power, n=24):
    """The cross-section `Part.stock` makes, as (x, y) points."""
    yc, h = (yb + yt) / 2, (yt - yb) / 2
    e = 2.0 / power
    out = []
    for k in range(n):
        th = 2 * math.pi * k / n
        c, s = math.cos(th), math.sin(th)
        w = wb + (wt - wb) * (s + 1) / 2
        out.append((math.copysign(abs(c) ** e, c) * w, yc + math.copysign(abs(s) ** e, s) * h))
    return out


# --- Receiver (milled, AK pattern, longer for the 7.62x54R) -----------------------
recv = Part("receiver")
recv_prof = fillet([(RX_Z0, RX_Y0 + 0.008), (RX_Z0 + 0.014, RX_Y0), (RX_Z1 - 0.004, RX_Y0),
                    (RX_Z1, RX_Y0 + 0.004), (RX_Z1, RX_Y1), (RX_Z0, RX_Y1)],
                   [0.002, 0.003, 0.002, 0.002, 0.0012, 0.0012], 2)
recv.prism(recv_prof, -RX_HW, RX_HW, "Blued", axis="x", bevel=0.0012, segs=2)
# Rails the cover rides on.
for sx in (-1, 1):
    recv.box((0.0025, 0.003, RX_Z1 - RX_Z0 - 0.06), (sx * (RX_HW + 0.0005), RX_Y1 - 0.0045, -0.11),
             "Blued", bevel=0.0006, segs=1)
rc = Part("cut")
# Ejection port and the charging handle's slot (right side).
rc.box((0.02, 0.019, 0.086), (RX_HW, 0.0725, -0.148), "Blued", bevel=0.002, segs=1)
rc.box((0.02, 0.0085, 0.16), (RX_HW, 0.0645, -0.137), "Blued")
# The milled lightening flats above the magazine well, both sides.
for sx in (-1, 1):
    rc.box((0.004, 0.022, 0.12), (sx * (RX_HW + 0.0012), 0.034, -0.205), "Blued", bevel=0.0015,
           segs=1)
# Magazine opening underneath.
rc.box((0.0285, 0.03, 0.09), (0, RX_Y0, -0.12), "Blued")
recv.cut(rc)

misc = Part("static_misc")
# Pins through the trigger group and trunnion, both sides.
for sx in (-1, 1):
    for z, y in ((-0.255, 0.03), (-0.255, 0.05), (-0.058, 0.026), (-0.03, 0.026), (0.006, 0.026),
                 (0.018, 0.05)):
        misc.sphere(0.0022, (sx * RX_HW, y, z), "Blued", segs=10, rings=5, scale=(0.45, 1, 1))

# The scope rail on the left: a dovetail plate riveted to the receiver.
rail = [(-0.0172, 0.0545), (-0.0212, 0.058), (-0.0262, 0.054), (-0.0262, 0.0765),
        (-0.0212, 0.0725), (-0.0172, 0.076)]
misc.prism(rail, -0.163, -0.035, "Blued", axis="z")
for z in (-0.15, -0.10, -0.048):
    misc.cyl(0.0022, -0.0268, -0.0255, "Steel", segs=10,
             xf=T(0, 0.0652, z) @ RY(math.pi / 2), chamfer=0.0005)
# The cut-out stop at the rail's rear the scope's clamp seats against.
misc.box((0.006, 0.026, 0.008), (-0.0215, 0.0655, -0.031), "Blued", bevel=0.001, segs=1)

# --- Receiver cover (smooth, the recoil spring button at the back) --------------------
cover_sec = fillet([(-0.018, 0.0775), (0.018, 0.0775), (0.018, 0.090), (0.0, 0.0975),
                    (-0.018, 0.090)], [0.0013, 0.0013, 0.007, 0.012, 0.007], 4)
misc.prism(cover_sec, -0.229, 0.034, "Blued", axis="z", bevel=0.0012, segs=1)
misc.cyl(0.0042, 0.034, 0.039, "Steel", segs=12, xf=T(0, 0.088, 0), chamfer=0.001)

# --- Trigger guard and magazine catch ----------------------------------------------------
guard_path = catmull([(-0.064, 0.016), (-0.067, 0.004), (-0.061, -0.009), (-0.048, -0.017),
                      (-0.028, -0.018), (-0.016, -0.012), (-0.010, -0.004)], samples=4,
                     closed=False)
misc.sweep(guard_path, section_rect(0.009, 0.003, 0.0010), "Blued")
misc.box((0.012, 0.003, 0.06), (0, 0.0155, -0.04), "Blued", bevel=0.0008, segs=1)
misc.prism(fillet([(-0.0755, 0.016), (-0.0705, 0.016), (-0.068, 0.002), (-0.0725, 0.0),
                   (-0.0755, 0.006)], 0.0012, 2), -0.0055, 0.0055, "Blued", axis="x",
           bevel=0.0006, segs=1)

# --- Barrel, gas block, sights, flash hider ----------------------------------------------
barrel = Part("barrel")
barrel.lathe([(0.0, -0.86), (0.0040, -0.86), (0.0040, -0.876), (0.0098, -0.876), (0.0098, -0.84),
              (0.0104, -0.70), (0.0114, -0.58), (0.0122, -0.52), (0.0132, -0.33),
              (0.0150, -0.305), (0.0150, -0.26), (0.0, -0.26)], "Blued", segs=20,
             xf=T(0, BORE, 0))
# Gas block with the piston cylinder, the regulator ahead of it
# (a sleeve on the barrel, the piston's cylinder over it, a web between).
barrel.cyl(0.0158, -0.566, -0.517, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.cyl(0.0122, -0.566, -0.517, "Blued", segs=18, xf=T(0, 0.084, 0), chamfer=0.0015)
barrel.prism([(-0.565, BORE), (-0.518, BORE), (-0.518, 0.084), (-0.565, 0.084)], -0.0095, 0.0095,
             "Blued", axis="x")
barrel.lathe([(0.0, -0.583), (0.0082, -0.583), (0.0092, -0.579), (0.0092, -0.565),
              (0.0, -0.565)], "Blued", segs=16, xf=T(0, 0.084, 0))
rg = Part("cut")
# The regulator's slot (set with a cartridge rim).
rg.box((0.022, 0.0028, 0.006), (0, 0.084, -0.583), "Blued")
barrel.cut(rg)
# Front sling swivel on the left of the gas block.
sling = [(-0.543 + 0.009 * math.cos(a), 0.052 - 0.009 * math.sin(a))
         for a in [math.pi * i / 8 for i in range(9)]]
barrel.sweep(sling, section_rect(0.0028, 0.0028, 0.0008), "Steel", xf=T(-0.0162, 0, 0))
# Front sight: a sleeve on the barrel, the tower, the post in its ring hood;
# the bayonet lug under it.
barrel.cyl(0.0128, -0.874, -0.838, "Blued", segs=20, xf=T(0, BORE, 0), chamfer=0.0015)
barrel.prism(fillet([(-0.871, 0.06), (-0.842, 0.06), (-0.846, 0.091), (-0.867, 0.091)],
                    0.0015, 2), -0.0065, 0.0065, "Blued", axis="x", bevel=0.0008, segs=1)
barrel.tube(0.0128, 0.0110, -0.870, -0.843, "Blued", segs=22, xf=T(0, 0.1, 0), chamfer=0.0006)
barrel.prism(fillet([(-0.860, 0.089), (-0.853, 0.089), (-0.8545, 0.106), (-0.8585, 0.106)],
                    0.0006, 1), -0.0013, 0.0013, "Blued", axis="x", bevel=0.0003, segs=1)
barrel.prism(fillet([(-0.872, BORE - 0.0105), (-0.838, BORE - 0.0105), (-0.842, BORE - 0.022),
                     (-0.868, BORE - 0.022)], 0.0015, 2), -0.0045, 0.0045, "Blued", axis="x",
             bevel=0.0008, segs=1)
# Rear sight: the block on the trunnion, its tangent leaf and slider, the
# notch plate; range marks along the leaf.
rs = fillet([(-0.289, 0.078), (-0.289, 0.096), (-0.279, 0.1005), (-0.237, 0.0985),
             (-0.233, 0.091), (-0.233, 0.078)], [0.001, 0.003, 0.003, 0.004, 0.003, 0.001], 2)
barrel.prism(rs, -0.0135, 0.0135, "Blued", axis="x", bevel=0.0012, segs=2)
barrel.prism([(-0.282, 0.0985), (-0.222, 0.1015), (-0.222, 0.1045), (-0.282, 0.1005)],
             -0.0065, 0.0065, "Blued", axis="x", bevel=0.0005, segs=1)
barrel.box((0.017, 0.006, 0.008), (0, 0.1008, -0.262), "Blued", bevel=0.001)
notch = [(-0.0065, 0.104), (0.0065, 0.104), (0.0065, 0.1088), (0.0011, 0.1088), (0.0011, 0.1074),
         (0.0, 0.1066), (-0.0011, 0.1074), (-0.0011, 0.1088), (-0.0065, 0.1088)]
barrel.prism(fillet(notch, [0.0005, 0.0005, 0.0005, 0.0003, 0.0, 0.0006, 0.0, 0.0003, 0.0005], 2),
             -0.225, -0.219, "Blued", axis="z")
for k in range(10):
    z = -0.2775 + k * 0.0052
    y = 0.0985 + (z + 0.282) / 0.06 * 0.003 + 0.0045
    w = 0.008 if k % 2 == 0 else 0.0045
    barrel.box((w, 0.0003, 0.0006), (-0.0065 + w / 2 + 0.0004, y + 0.00015, z), "Worn")

# Flash hider: a long tube with five slots, closed at the muzzle.
hider = Part("hider")
hider.tube(0.0118, 0.0084, MUZZLE, -0.872, "Blued", segs=25, xf=T(0, BORE, 0), chamfer=0.0012)
hc = Part("cut")
slot = fillet([(-0.0024, -0.946), (0.0024, -0.946), (0.0024, -0.884), (-0.0024, -0.884)],
              0.0023, 3)
for k in range(5):
    hc.prism(slot, BORE + 0.006, BORE + 0.016, "Blued", axis="y",
             xf=RZ(math.radians(36 + 72 * k), about=(0, BORE, 0)))
hider.cut(hc)

# --- Handguard: two wooden halves, six cooling slots a side, the rings --------------------
HG0, HG1 = -0.519, -0.287
HG_YB, HG_YT = 0.027, 0.099


def hg_w(z, yc, h):
    return (0.0215, 0.0175)


hg = Part("handguard")
hg.stock([(HG0 - 0.001, HG_YB), (HG1 + 0.001, HG_YB), (HG1 + 0.001, HG_YT), (HG0 - 0.001, HG_YT)],
         HG0, HG1, "Wood", hg_w, stations=6, segs=28, power=2.7)
hgc = Part("cut")
for sx in (-1, 1):
    for k in range(6):
        zc = -0.326 + k * -0.031
        hgc.box((0.012, 0.032, 0.0085), (sx * 0.0225, 0.064, zc), "Black", round_=0.0042,
                xf=RX(0.18, about=(0, 0.064, zc)))
    # The seam between the halves along the top.
hgc.box((0.0016, 0.012, 0.25), (0, HG_YT, (HG0 + HG1) / 2), "Black")
hg.cut(hgc, paint=True)

rings = Part("rings")
ring_sec = [(x * 1.07, 0.063 + (y - 0.063) * 1.06) for x, y in
            superellipse(HG_YB, HG_YT, 0.0215, 0.0175, 2.7, 28)]
rings.prism(ring_sec, HG1 - 0.002, HG1 + 0.012, "Blued", axis="z", bevel=0.0008, segs=1)
rings.prism(ring_sec, HG0 - 0.004, HG0 + 0.007, "Blued", axis="z", bevel=0.0008, segs=1)
# The rear ring's latch lever on the right.
rings.box((0.0035, 0.024, 0.007), (0.0245, 0.05, HG1 + 0.005), "Blued", bevel=0.0008,
          xf=RX(-0.3, about=(0.0245, 0.05, HG1 + 0.005)))

# --- Skeleton stock: laminated wood, the thumbhole behind the grip ------------------------
def comb_y(z):
    return 0.079 - (z - 0.022) / 0.233 * 0.015


outer = [
    (0.022, comb_y(0.022)), (0.255, comb_y(0.255)),      # comb, the heel under the pad
    (0.257, -0.083),                                      # toe
    (0.07, -0.0985), (0.042, -0.103), (0.016, -0.0985),   # lower bar, grip's foot
    (-0.0215, 0.007), (-0.0245, 0.0175), (0.022, 0.0175),  # grip front, under the receiver
]
outer = fillet(outer, [0.0, 0.0, 0.0, 0.03, 0.012, 0.012, 0.01, 0.0, 0.0], 4)
hole = fillet([(0.041, 0.030), (0.199, comb_y(0.199) - 0.032), (0.201, -0.054), (0.055, -0.066)],
              [0.011, 0.016, 0.016, 0.014], 5)


def sw(z):
    return 0.0185 + (z - 0.022) / 0.235 * 0.002


stock = Part("stock")
stock.prism(outer, 0, 0, "Wood", axis="x", bevel=0.007, segs=4,
            width=lambda z, y: (-sw(z), sw(z)))
# The thumbhole: a cutter flared at both faces, so the hole's edges are
# rounded like the outline's.
if area2(hole) < 0:
    hole = list(reversed(hole))
n = len(hole)
nrm = []
for i in range(n):
    a, b = hole[i - 1], hole[(i + 1) % n]
    tz, ty = b[0] - a[0], b[1] - a[1]
    ln = math.hypot(tz, ty)
    nrm.append((ty / ln, -tz / ln))      # outward from the hole (into the wood)
R = 0.0065
# (side, how far round the edge's quarter circle; None = clear outside the face)
steps = [(-1, None)] + [(-1, a) for a in (90, 60, 30, 0)] + [(1, a) for a in (0, 30, 60, 90)]
steps += [(1, None)]
loops = []
for side, a in steps:
    loop = []
    for (z, y), (nz, ny) in zip(hole, nrm):
        if a is None:
            x, off = side * (sw(z) + 0.006), R + 0.012
        else:
            t = math.radians(a)
            x = side * (sw(z) - R + 0.0005 + R * math.sin(t))
            off = R - R * math.cos(t)
        loop.append((x, y + ny * off, z + nz * off))
    loops.append(loop)
hcut = Part("cut")
hcut.loft(loops, "Wood")
stock.cut(hcut)
pad = Part("buttpad")
pad_prof = fillet([(0.2545, comb_y(0.255) + 0.0005), (0.2705, comb_y(0.255) + 0.0005),
                   (0.272, -0.0835), (0.2565, -0.0835)], [0.004, 0.004, 0.004, 0.004], 2)
pad.prism(pad_prof, -0.021, 0.021, "Rubber", axis="x", bevel=0.003, segs=2)
pc = Part("cut")
for k in range(9):
    y = -0.07 + k * 0.016
    pc.box((0.05, 0.003, 0.004), (0, y, 0.2725), "Rubber")
pad.cut(pc)

furn = Part("furniture")
# The rear sling swivel's plate and loop on the left of the butt.
for z0, y0, xs in ((0.222, -0.058, -0.0215),):
    furn.box((0.003, 0.024, 0.012), (xs + 0.0008, y0, z0), "Steel", bevel=0.0008, segs=1)
    loop = [(z0 + 0.010 * math.cos(a), y0 - 0.006 - 0.010 * math.sin(a))
            for a in [math.pi * i / 8 for i in range(9)]]
    furn.sweep(loop, section_rect(0.0028, 0.0028, 0.0008), "Steel", xf=T(xs - 0.0016, 0, 0))

# The detachable cheek rest, clamped on the comb.
CR0, CR1 = 0.078, 0.214
cr_top = catmull([(CR1, comb_y(CR1) + 0.004), (0.200, comb_y(0.2) + 0.013),
                  (0.15, comb_y(0.15) + 0.017), (0.10, comb_y(0.10) + 0.014),
                  (CR0, comb_y(CR0) + 0.004)], 4, closed=False)
cheek_out = [(CR0, comb_y(CR0) - 0.014), (CR1, comb_y(CR1) - 0.014)] + cr_top


def cheek_w(z, yc, h):
    return (0.0222, 0.0175)


cheek = Part("cheek")
cheek.stock(cheek_out, CR0 + 0.0005, CR1 - 0.0005, "Black", cheek_w, stations=22, segs=24,
            power=2.6)
# Its clamp latch on the left.
cheek.box((0.004, 0.012, 0.02), (-0.0222, comb_y(0.146) - 0.008, 0.146), "Steel", bevel=0.0012,
          segs=1)

# --- Moving parts ---------------------------------------------------------------------
carrier = Part("bolt_carrier", origin=(0.0, BORE, -0.2))
carrier.prism(fillet([(-0.0125, 0.059), (0.0125, 0.059), (0.0125, 0.0745), (0.0, 0.0785),
                      (-0.0125, 0.0745)], 0.002, 2), -0.232, -0.105, "Steel", axis="z",
              bevel=0.0015, segs=1)
carrier.cyl(0.0078, -0.25, -0.232, "Steel", segs=14, xf=T(0, BORE + 0.002, 0))
# The charging handle: arm out of the slot, a round knob.
carrier.box((0.026, 0.0068, 0.010), (0.0255, 0.0645, -0.2), "Steel", bevel=0.0015)
carrier.lathe([(0.0, 0.0), (0.0058, 0.0), (0.0072, 0.004), (0.0072, 0.009), (0.0058, 0.0135),
               (0.0, 0.014)], "Steel", segs=14,
              xf=T(0.0385, 0.0645, -0.2) @ RY(math.pi / 2))

PIVOT = (0.0188, 0.06, -0.006)
selector = Part("selector", origin=PIVOT)
sel = fillet([(-0.008, 0.0545), (-0.002, 0.0545), (-0.124, 0.0575), (-0.147, 0.0545),
              (-0.154, 0.0500), (-0.150, 0.0478), (-0.146, 0.053), (-0.126, 0.0545),
              (-0.126, 0.0685), (-0.006, 0.0665), (0.0, 0.064)],
             [0.0, 0.0, 0.004, 0.002, 0.002, 0.001, 0.003, 0.003, 0.004, 0.004, 0.003], 2)
selector.prism(sel, RX_HW + 0.0006, RX_HW + 0.0032, "Blued", axis="x", bevel=0.0006, segs=1)
selector.cyl(0.0058, RX_HW + 0.0005, RX_HW + 0.0036, "Blued", segs=14,
             xf=T(0, PIVOT[1], PIVOT[2]) @ RY(math.pi / 2), chamfer=0.0008)

trig = Part("trigger", origin=(0.0, 0.014, -0.039))
trig.sweep(catmull([(-0.039, 0.016), (-0.0425, 0.008), (-0.0425, 0.001), (-0.039, -0.005),
                    (-0.0345, -0.007)], 3, closed=False), section_rect(0.005, 0.0034, 0.0012),
           "Blued")

# --- Build ------------------------------------------------------------------------------
join("static", recv, misc, barrel, hider, hg, rings, stock, pad, furn, cheek, worn={})
for p in (carrier, selector, trig):
    make(p, worn={})

empty("pt_muzzle", (0.0, BORE, MUZZLE))
# The eye behind the notch, on the line over the front post.
empty("pt_sight", (0.0, 0.1068, 0.0))
empty("pt_port", (RX_HW, 0.0705, -0.148))
empty("pt_support", (0.0, 0.03, -0.40))
empty("pt_handle", (0.047, 0.0645, -0.2))
# The magazine's box centre sits under here, its top level with this point:
# its feed lips' centre lands at z -0.12.
empty("magwell", (0.0, 0.019, -0.124))
empty("pt_sling_front", (-0.0162, 0.043, -0.543))
empty("pt_sling_rear", (-0.0237, -0.074, 0.222))
# Where the PSO-1's clamp seats on the side rail's outer face.
empty("pt_rail", (-0.0262, 0.0652, -0.10))
report()
export("assets/models/guns/svd.glb")
