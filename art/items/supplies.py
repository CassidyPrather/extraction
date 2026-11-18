"""Medical and maintenance supplies: field dressing, regen injector, splint,
cleaning kit, gun oil, spare parts, multitool, fuel can. Writes
assets/models/items/<id>.glb for each (item space: see _kit.py). Things
without `dims` are a cube of side cbrt(bulk / 1000); each model rests on
that cube's floor.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from _kit import ALONG_X, LIFT, UP, centred, cube, item, ring, text, transform, tube_path, wanted
from common import Part, RX, RY, RZ, T, S, fillet, section_rect


def pillow(p, hx, hy, hz, y, mat, r=0.006):
    """A soft packet: a rounded slab, its edges pinched."""
    prof = fillet([(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)], r, 3)
    p.prism(prof, y - hy, y + hy, mat, axis="y", bevel=hy * 0.9, segs=3)


# --- Field dressing ---------------------------------------------------------------------------

def field_dressing():
    s = cube(0.1)
    p = Part("body")
    # A pale packet with a red tear-strip and a red cross on top: medical
    # is the lightest family, red on white (docs/craft/ITEM_READING.md).
    # The cross sits in a recess cut for it, flush, not on the wrap.
    y = -s / 2 + 0.012
    pillow(p, s / 2 - 0.001, 0.012, s / 2 - 0.003, y, "Pale", r=0.008)
    p.box((s - 0.01, 0.003, 0.004), (0.0, y, -s / 2 + 0.003), "Red")
    recess = Part("recess")
    recess.box((0.024, 0.009, 0.008), (0.0, y + 0.012, 0.0005), "Red")
    recess.box((0.008, 0.009, 0.024), (0.0, y + 0.012, 0.0005), "Red")
    p.cut(recess)
    label = Part("label")
    label.box((0.024, 0.0042, 0.008), (0.0, y + 0.0101, 0.0005), "Red")
    label.box((0.008, 0.0042, 0.024), (0.0, y + 0.0101, 0.0005), "Red")
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p, label])


# --- Regen injector ---------------------------------------------------------------------------

def regen_injector():
    """The injector pen (#102), lying along Z, its needle at -Z: two
    cartridge wells side by side down its back, open on top so you see
    into them (`items::pen::SLOTS`, each a cartridge's centre line), a
    steel catch over each well that closes on a seated cartridge's collar,
    a thumb eject button on top ahead of the wells, the red back cap, and
    a short needle in its shroud at the front."""
    p = Part("body")
    # The body: a dark olive block, its edges rounded, the wells cut down
    # into its top.
    # (Its bottom as far under the centre as the button stands over it:
    # the model is centred on its box, and the slots are from its centre.)
    top, bottom = 0.004, -0.011
    prof = fillet([(-0.016, bottom), (0.016, bottom), (0.016, top), (-0.016, top)],
                  [0.006, 0.006, 0.003, 0.003], 3)
    p.prism([(x, y) for x, y in prof], -0.052, 0.058, "Olive", axis="z", bevel=0.002, segs=2)
    wells = Part("wells")
    for x in SLOT_X:
        wells.cyl(0.0064, -0.0115, 0.0615, "Olive", segs=20, xf=T(x, SLOT_Y, 0.0))
    p.cut(wells)
    # The wells' floors, dark, so a cartridge reads against them.
    for x in SLOT_X:
        p.box((0.008, 0.0008, 0.072), (x, SLOT_Y - 0.0062, 0.025), "Black", segs=1)
    # The catches: a steel clip over each well, its bridge on the cartridge's
    # collar (which stands 4 mm proud of the top), standing on the body on
    # a post at the rim and one shared on the ridge between the wells.
    cy0, cy1 = SLOT_Y + 0.0064, SLOT_Y + 0.008
    for x in SLOT_X:
        sx = math.copysign(1.0, x)
        p.box((0.0152, cy1 - cy0, 0.003), (sx * 0.0076, (cy0 + cy1) / 2, 0.0), "Steel",
              bevel=0.0004, segs=1)
        p.box((0.0012, cy1 - 0.0032, 0.003), (sx * 0.0148, (cy1 + 0.0032) / 2, 0.0), "Steel",
              bevel=0.0003, segs=1)
    p.box((0.0018, cy1 - top + 0.0004, 0.003), (0.0, (cy1 + top - 0.0004) / 2, 0.0), "Steel",
          bevel=0.0003, segs=1)
    # The thumb eject button, ahead of the wells, ridged.
    p.box((0.010, 0.0062, 0.011), (0.0, top + 0.0031, -0.021), "Black", bevel=0.0015, segs=2)
    for k in range(3):
        p.box((0.0105, 0.0009, 0.0012), (0.0, top + 0.0066, -0.025 + k * 0.004), "Black",
              segs=1)
    # The red back cap.
    cap = fillet([(-0.0155, bottom + 0.0005), (0.0155, bottom + 0.0005), (0.0155, top + 0.0015),
                  (-0.0155, top + 0.0015)], [0.006, 0.006, 0.004, 0.004], 3)
    p.prism(cap, 0.058, 0.065, "Red", axis="z", bevel=0.0018, segs=2)
    # The nose: the body tapering to the needle's shroud, the needle short.
    p.lathe([(0.0, -0.052), (0.0085, -0.052), (0.0065, -0.056), (0.0042, -0.058), (0.0042, -0.0615),
             (0.0, -0.0615)], "Black", segs=18, xf=T(0.0, -0.0035, 0.0))
    p.cyl(0.0006, -0.065, -0.061, "Steel", segs=8, xf=T(0.0, -0.0035, 0.0))
    return [p]


# The wells' centre lines (`items::pen::SLOTS`, x and y; z 0.025).
SLOT_X = (0.0075, -0.0075)
SLOT_Y = 0.002


# --- Injector cartridges (#102) ---------------------------------------------------------------
# A glass cartridge along Z, its needle end (the septum) at -Z: the dose
# behind glass, a dark label sleeve round it with a long window on top so
# you read how full it is at arm's length, its grade in coloured bands
# round the back (REGEN green 1-3, STIM amber 1-2), steel caps, and a
# locking collar near the front that the pen's well closes on.
#
# The liquid is its own node, `fill`, its origin at its front end: the
# game scales it along Z by the cartridge's condition, how full it is
# (`items::pen`), so the window shows what's left.
CART_R, CART_HALF = 0.006, 0.035
WINDOW = (52.0, 128.0)  # the label's window, degrees round from +X (90: on top)


def cartridge(dose, bands):
    p = Part("body")
    # The glass, open at the ends to the caps.
    p.tube(0.0052, 0.0047, -0.029, 0.029, "Vial", segs=20)
    # The label sleeve, the window left out of it.
    w0, w1 = WINDOW
    p.lathe([(0.0057, -0.026), (0.0057, 0.026), (0.0053, 0.026), (0.0053, -0.026)], "Black",
            segs=18, start=w1, sweep=360.0 - (w1 - w0), closed=True)
    # The grade: bands round the back of the sleeve, in the dose's colour.
    for k in range(bands):
        z = 0.021 - k * 0.0045
        p.lathe([(0.00585, z - 0.0013), (0.00585, z + 0.0013), (0.0056, z + 0.0013),
                 (0.0056, z - 0.0013)], dose, segs=18, start=w1, sweep=360.0 - (w1 - w0),
                closed=True)
    # Window ticks: a quarter, a half, three quarters, along its edge.
    for t in (0.25, 0.5, 0.75):
        z = -0.026 + 0.052 * t
        p.box((0.0012, 0.0006, 0.0005), (0.0055 * math.cos(math.radians(w1 + 4)),
              0.0055 * math.sin(math.radians(w1 + 4)), z), "Steel", segs=1)
    # Steel caps: the septum's neck at the front, a ribbed back.
    p.lathe([(0.0, -0.035), (0.0026, -0.035), (0.0032, -0.0335), (0.0032, -0.032),
             (0.0053, -0.031), (0.0053, -0.028), (0.0, -0.028)], "Steel", segs=18)
    p.lathe([(0.0, 0.028), (0.0058, 0.028), (0.006, 0.0295), (0.006, 0.0335), (0.0056, 0.035),
             (0.0, 0.035)], "Steel", segs=18)
    p.tube(0.00605, 0.0055, 0.0302, 0.0312, "Black", segs=18)
    # The locking collar the well's catch closes on, near the front.
    p.tube(0.006, 0.0053, -0.0262, -0.0235, "Steel", segs=18)
    # The dose, front to back inside the glass: scaled by the game.
    front = -0.0275
    fill = Part("fill", origin=(0.0, 0.0, front))
    fill.cyl(0.0046, front, 0.0275, dose, segs=16)
    return [p, fill]


CARTRIDGES = (("regen_r1", "Regen", 1), ("regen_r2", "Regen", 2), ("regen_r3", "Regen", 3),
              ("stim_s1", "Stim", 1), ("stim_s2", "Stim", 2))


# --- Cleaning kit -----------------------------------------------------------------------------

def cleaning_kit():
    p = Part("body")
    # A dark canvas roll with two leather straps, the rod poking out one
    # end, a brush on it, and a little oiler tucked under the straps: tools
    # are dark with bare steel (docs/craft/ITEM_READING.md).
    roll = fillet([(-0.036, -0.022), (0.036, -0.022), (0.036, 0.016), (-0.036, 0.016)],
                  [0.012, 0.012, 0.02, 0.02], 4)
    p.prism(roll, -0.1, 0.1, "DarkWool", axis="z", bevel=0.006, segs=2)
    p.box((0.07, 0.003, 0.16), (0.0, 0.0175, 0.0), "DarkWool", bevel=0.0012, segs=1,
          xf=RX(0.02))
    for z in (-0.06, 0.055):
        strap = fillet([(-0.038, -0.024), (0.038, -0.024), (0.038, 0.019), (-0.038, 0.019)],
                       [0.013, 0.013, 0.021, 0.021], 4)
        p.prism(strap, z - 0.009, z + 0.009, "Leather", axis="z")
        p.box((0.012, 0.004, 0.014), (0.03, 0.019, z), "Steel", bevel=0.001, segs=1)
    p.cyl(0.0035, 0.08, 0.124, "Steel", segs=8, xf=T(0.018, 0.0, 0.0))
    p.cyl(0.0055, 0.112, 0.125, "Brass", segs=10, xf=T(0.018, 0.0, 0.0))
    p.cyl(0.003, -0.124, -0.09, "Steel", segs=8, xf=T(-0.012, -0.004, 0.0))
    p.lathe([(0.0, -0.125), (0.007, -0.122), (0.007, -0.114), (0.0, -0.114)], "Bakelite",
            segs=10, xf=T(-0.012, -0.004, 0.0))
    return [p]


# --- Gun oil --------------------------------------------------------------------------------

def gun_oil():
    s = cube(0.12)
    yb = -s / 2
    p = Part("body")
    # A small oil can: a round tin, a stepped shoulder, a spout and cap.
    p.lathe([(0.0, yb), (0.018, yb), (0.0195, yb + 0.002), (0.0195, yb + 0.026),
             (0.018, yb + 0.028), (0.009, yb + 0.033), (0.0045, yb + 0.034), (0.0, yb + 0.034)],
            "Paint", segs=20, xf=UP)
    p.tube(0.0198, 0.017, yb + 0.006, yb + 0.02, "Cream", segs=20, xf=UP)
    p.tube(0.0199, 0.017, yb + 0.011, yb + 0.015, "Red", segs=20, xf=UP)
    p.lathe([(0.0, yb + 0.033), (0.0035, yb + 0.033), (0.0022, yb + 0.044), (0.0, yb + 0.044)],
            "Brass", segs=10, xf=UP)
    p.cyl(0.003, yb + 0.044, yb + 0.049, "Black", segs=10, xf=UP, chamfer=0.0008)
    return [p]


# --- Spare parts -----------------------------------------------------------------------------

def spare_parts():
    s = cube(0.2)
    yb = -s / 2
    p = Part("body")
    # A small tin with its lid on, and on it a recoil spring, an extractor
    # and a few pins, tied with wire.
    tin = fillet([(-0.026, -0.02), (0.026, -0.02), (0.026, 0.02), (-0.026, 0.02)], 0.005, 2)
    p.prism(tin, yb, yb + 0.014, "Blued", axis="y", bevel=0.0015, segs=1)
    lid = fillet([(-0.0265, -0.0205), (0.0265, -0.0205), (0.0265, 0.0205), (-0.0265, 0.0205)],
                 0.0055, 2)
    p.prism(lid, yb + 0.014, yb + 0.02, "Blued", axis="y", bevel=0.0015, segs=1)
    # The label on the lid, LIFT thick from its face.
    p.box((0.03, LIFT, 0.018), (0.0, yb + 0.02 + LIFT / 2, 0.0), "Paper")
    # Coiled spring lying across the lid.
    # On the lid (its end coils stood 0.7 mm off it, 1.5 mm drawn).
    ys = yb + 0.0247
    for k in range(9):
        z = -0.016 + k * 0.004
        p.lathe([(0.0042, -0.0007), (0.0049, 0.0), (0.0042, 0.0007), (0.0035, 0.0)], "Steel",
                segs=10, xf=T(-0.008, ys, z) @ RX(0.25), closed=True)
    # Extractor: a little hooked bar.
    ext = [(0.0, 0.0), (0.016, 0.0), (0.018, 0.003), (0.016, 0.004), (0.004, 0.004), (0.0, 0.003)]
    p.prism([(x + 0.003, z - 0.01) for x, z in ext], yb + 0.0205, yb + 0.0235, "Blued", axis="y")
    for k in range(3):
        p.cyl(0.0012, -0.008, 0.008, "Steel", segs=6,
              xf=T(0.013 + k * 0.004, yb + 0.0215, 0.01) @ RY(0.3 * k))
    # Binding wire round the lot.
    for x in (-0.018, 0.018):
        p.box((0.0012, 0.0012, 0.042), (x, yb + 0.0205, 0.0), "Steel")
    # Sits on the floor of a box fitted to it (`dims` in items.ron).
    return centred([p])


# --- Multitool --------------------------------------------------------------------------------

def multitool():
    s = cube(0.1)
    yb = -s / 2
    p = Part("body")
    # A folding pliers tool lying closed: two steel handle channels with
    # black grip inlays, the pliers' jaws out at the front, the knife blade
    # swung half open.
    for sx in (-1, 1):
        p.box((0.0072, 0.006, 0.03), (sx * 0.0038, yb + 0.003, 0.007), "Steel", bevel=0.0008,
              segs=1)
        p.box((0.0012, 0.0036, 0.022), (sx * 0.0078, yb + 0.003, 0.008), "Black", bevel=0.0003,
              segs=1)
        jaw = [(0.0, -0.007), (0.0055, -0.007), (0.0035, -0.0175), (0.0014, -0.0222),
               (0.0, -0.0222)]
        p.prism([(sx * x, z) for x, z in jaw], yb + 0.001, yb + 0.005, "Steel", axis="y",
                xf=T(0.0, 0.0, -0.007) @ RY(sx * 0.07) @ T(0.0, 0.0, 0.007))
        for k in range(3):
            p.box((0.0016, 0.0042, 0.0008), (sx * 0.0012, yb + 0.003, -0.012 - k * 0.0025), "Dark",
                  xf=RY(sx * 0.07))
    p.cyl(0.0024, yb + 0.0002, yb + 0.0058, "Steel", segs=12, xf=T(0, 0, -0.0072) @ UP)
    for z in (0.0, 0.019):
        p.cyl(0.0014, yb + 0.0058, yb + 0.0064, "Steel", segs=8, xf=T(0.0038, 0, z) @ UP)
        p.cyl(0.0014, yb + 0.0058, yb + 0.0064, "Steel", segs=8, xf=T(-0.0038, 0, z) @ UP)
    blade = [(0.0, -0.0026), (0.0155, -0.0026), (0.0192, 0.0004), (0.0, 0.0028)]
    p.prism(blade, yb + 0.0062, yb + 0.007, "Steel", axis="y", xf=T(0.0038, 0, 0.019) @ RY(0.25))
    ring(p, 0.003, 0.0007, "Steel", xf=T(0.0, yb + 0.003, 0.0195) @ RX(math.pi / 2), segs=10,
         sides=4)
    # Modelled small; a real folding pliers tool is about 10 cm closed.
    return centred(transform([p], S(2.2, 2.2, 2.2)))


# --- Fuel can ---------------------------------------------------------------------------------

def fuel_can():
    p = Part("body")
    # A ten-litre olive jerrycan: pressed X ribs on both flanks, a welded
    # seam round its middle, three handles on top, a cam-lever spout cap on
    # the cut-off front corner, the fuel stencilled front and back.
    yb = -0.175
    prof = [(-0.145, yb), (0.145, yb), (0.145, yb + 0.33), (-0.1, yb + 0.33), (-0.145, yb + 0.29)]
    p.prism(fillet(prof, 0.015, 3), -0.075, 0.075, "Paint", axis="x", bevel=0.01, segs=2)
    seam = [(-0.1485, yb + 0.0035), (0.1485, yb + 0.0035), (0.1485, yb + 0.3335),
            (-0.1015, yb + 0.3335), (-0.1485, yb + 0.2915)]
    p.prism(fillet(seam, 0.017, 3), -0.002, 0.002, "Paint", axis="x")
    # X ribs: one bar corner to corner, the other in two halves either side.
    ang = math.atan2(0.25, 0.22)
    L = math.hypot(0.25, 0.22)
    yc = yb + 0.16
    for sx in (-1, 1):
        x = sx * 0.0775
        p.box((0.0055, 0.024, L), (x, 0.0, 0.0), "Paint", bevel=0.002, segs=1,
              xf=T(0, yc, 0) @ RX(ang))
        for sz in (-1, 1):
            p.box((0.0055, 0.024, L / 2 - 0.022), (x, 0.0, sz * (L / 4 + 0.011)), "Paint",
                  bevel=0.002, segs=1, xf=T(0, yc, 0) @ RX(-ang))
    # Handles on top.
    top = yb + 0.33
    for x in (-0.035, 0.0, 0.035):
        tube_path(p, [(x, top - 0.002, -0.005), (x, top + 0.011, 0.008), (x, top + 0.011, 0.1),
                      (x, top - 0.002, 0.113)], 0.0055, "Paint", sides=6, closed=False)
    # Spout and cap, square to the cut-off corner.
    xf = T(0.0, yb + 0.3078, -0.1205) @ RX(-0.835) @ UP
    p.cyl(0.016, -0.004, 0.009, "Steel", segs=16, xf=xf)
    p.lathe([(0.0, 0.009), (0.02, 0.009), (0.021, 0.011), (0.021, 0.019), (0.018, 0.022),
             (0.0, 0.022)], "Paint", segs=18, xf=xf)
    p.box((0.008, 0.006, 0.04), (0.0, 0.0, 0.0), "Steel", bevel=0.0015, segs=1,
          xf=xf @ T(0.0, 0.0, 0.025) @ RX(math.pi / 2) @ T(0.0, 0.0, 0.004))
    for sz, z in ((-1, -0.145), (1, 0.145)):
        text(p, "ДТ", 0.045, "Cream", T(0.0, yb + 0.13, z) @ (RY(math.pi) if sz < 0 else T()),
             depth=0.0035)
    return [p]


# --- Splint ---------------------------------------------------------------------------------------

def splint():
    """Two wooden slats lying along Z, three olive webbing straps round
    them with steel buckles: strapped either side of a broken shin."""
    L = 0.44
    y = -0.015
    p = Part("body")
    for x in (-0.021, 0.021):
        p.box((0.017, 0.011, L), (x, y + 0.0055, 0.0), "Wood", bevel=0.002, segs=1)
    for z in (-0.16, 0.0, 0.16):
        # The strap round both slats, LIFT proud of them all round (its
        # underside lay in the slats' undersides' plane).
        p.box((0.062, 0.011 + 2 * LIFT, 0.026), (0.0, y + 0.0055, z), "Olive", bevel=0.0012,
              segs=1)
        p.box((0.014, 0.004, 0.02), (0.0, y + 0.011 + LIFT + 0.0015, z), "Steel", bevel=0.0008,
              segs=1)
    return centred([p])


for item_id, fn in (("field_dressing", field_dressing), ("regen_injector", regen_injector),
                    ("splint", splint),
                    ("cleaning_kit", cleaning_kit), ("gun_oil", gun_oil),
                    ("spare_parts", spare_parts), ("multitool", multitool),
                    ("fuel_can", fuel_can)):
    if wanted(item_id):
        item(item_id, fn)
for item_id, dose, bands in CARTRIDGES:
    if wanted(item_id):
        item(item_id, lambda d=dose, b=bands: cartridge(d, b))
